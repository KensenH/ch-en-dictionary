package main

import (
	"bufio"
	"context"
	"database/sql"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
	"unicode"

	log "github.com/sirupsen/logrus"
	"github.com/xuri/excelize/v2"

	_ "github.com/mattn/go-sqlite3"
)

const (
	dbPath        = "./data/cedict_dictionary.db"
	cedictSource  = "./source/cedict_ts.u8"
	subtlexSource = "./source/SUBTLEX-CH-WF.xlsx"
	subtlexSheet  = "SUBTLEX-CH-WF"
	batchSize     = 2000
)

func main() {
	start := time.Now()
	ctx := context.Background()

	db, err := openDB(dbPath)
	if err != nil {
		log.Fatalln(err)
	}
	defer db.Close()

	if err := createCedictTable(db); err != nil {
		log.Fatalln(err)
	}
	wordCount, err := populateCedict(ctx, db, cedictSource, batchSize)
	if err != nil {
		log.Fatalln(err)
	}
	if err := createCedictIndexes(db); err != nil {
		log.Fatalln(err)
	}
	if err := createCedictFTS(db); err != nil {
		log.Fatalln(err)
	}
	if err := createSubtlexTable(db); err != nil {
		log.Fatalln(err)
	}
	if err := populateSubtlex(ctx, db, subtlexSource, subtlexSheet, batchSize); err != nil {
		log.Fatalln(err)
	}
	if err := migrateSubtlexToCedict(db); err != nil {
		log.Fatalln(err)
	}

	if err := vacuum(db); err != nil {
		log.Fatalln(err)
	}

	log.Infof("Done. Inserted %d words.", wordCount)
	log.Infof("%s", time.Since(start))
}

func openDB(path string) (*sql.DB, error) {
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return nil, fmt.Errorf("remove existing database: %w", err)
	}

	db, err := sql.Open("sqlite3", path)
	if err != nil {
		return nil, err
	}
	for _, pragma := range []string{
		`PRAGMA journal_mode = OFF`,
		`PRAGMA synchronous = OFF`,
		`PRAGMA temp_store = MEMORY`,
	} {
		if _, err := db.Exec(pragma); err != nil {
			db.Close()
			return nil, err
		}
	}
	return db, nil
}

func createCedictTable(db *sql.DB) error {
	_, err := db.Exec(`
		CREATE TABLE IF NOT EXISTS cedict_dictionary (
			id INTEGER PRIMARY KEY,
			traditional_character TEXT,
			simplified_character TEXT,
			pinyin TEXT,
			pinyin_plain TEXT,
			pinyin_without_space TEXT,
			w_cd INTEGER,
			definition TEXT
		)
	`)
	return err
}

func populateCedict(ctx context.Context, db *sql.DB, sourcePath string, batchSize int) (int, error) {
	file, err := os.Open(sourcePath)
	if err != nil {
		return 0, err
	}
	defer file.Close()

	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return 0, err
	}
	defer tx.Rollback()

	scanner := bufio.NewScanner(file)
	scanner.Split(bufio.ScanLines)

	var batchArgs []interface{}
	var batchCount int
	var wordCount int
	lineNumber := 0

	log.Infof("Scanning file...")
	for scanner.Scan() {
		lineNumber++
		line := scanner.Text()
		if strings.HasPrefix(line, "#") {
			continue
		}

		traditionalCharacter, simplifiedCharacter, pinyin, definition, err := parseCedictLine(line)
		if err != nil {
			return 0, fmt.Errorf("parse CEDICT line %d: %w", lineNumber, err)
		}
		pinyinPlain := alphaOnly(pinyin)
		pinyinWithoutSpace := strings.ReplaceAll(pinyinPlain, " ", "")

		batchArgs = append(batchArgs,
			traditionalCharacter, simplifiedCharacter,
			pinyin, pinyinPlain, pinyinWithoutSpace, definition,
		)
		batchCount++

		if batchCount == batchSize {
			if err := execCedictBatch(tx, batchArgs, batchCount); err != nil {
				return 0, err
			}
			batchArgs = batchArgs[:0]
			batchCount = 0
			wordCount += batchSize
		}
	}

	if err := scanner.Err(); err != nil {
		return 0, err
	}

	if batchCount > 0 {
		if err := execCedictBatch(tx, batchArgs, batchCount); err != nil {
			return 0, err
		}
		wordCount += batchCount
	}

	return wordCount, tx.Commit()
}

func parseCedictLine(line string) (string, string, string, string, error) {
	firstSpace := strings.IndexByte(line, ' ')
	if firstSpace <= 0 {
		return "", "", "", "", fmt.Errorf("missing traditional/simplified separator")
	}

	traditionalCharacter := line[:firstSpace]
	remaining := strings.TrimSpace(line[firstSpace+1:])
	secondSpace := strings.IndexByte(remaining, ' ')
	if secondSpace <= 0 {
		return "", "", "", "", fmt.Errorf("missing simplified/pinyin separator")
	}

	simplifiedCharacter := remaining[:secondSpace]
	remaining = strings.TrimSpace(remaining[secondSpace+1:])
	pinyinStart := strings.IndexByte(remaining, '[')
	pinyinEnd := strings.IndexByte(remaining, ']')
	if pinyinStart < 0 || pinyinEnd <= pinyinStart {
		return "", "", "", "", fmt.Errorf("missing pinyin brackets")
	}

	pinyin := remaining[pinyinStart+1 : pinyinEnd]
	definition := strings.TrimSpace(remaining[pinyinEnd+1:])
	if len(definition) < 2 || !strings.HasPrefix(definition, "/") || !strings.HasSuffix(definition, "/") {
		return "", "", "", "", fmt.Errorf("definition is not enclosed in slashes")
	}

	return traditionalCharacter, simplifiedCharacter, pinyin, definition[1 : len(definition)-1], nil
}

func execCedictBatch(tx *sql.Tx, batchArgs []interface{}, batchCount int) error {
	placeholders := valuePlaceholders(batchCount, 6)
	log.Infof("Inserting batch of %d words...", batchCount)
	_, err := tx.Exec(fmt.Sprintf(
		`INSERT INTO cedict_dictionary
			(traditional_character, simplified_character, pinyin,
			pinyin_plain, pinyin_without_space, definition) VALUES %s`,
		placeholders), batchArgs...)
	return err
}

// createCedictIndexes creates indexes for:
// - Hanzi prefix: traditional_character LIKE $1||'%' OR simplified_character LIKE $1||'%'
// - Order by frequency: ORDER BY w_cd DESC (composite indexes avoid separate sort)
func createCedictIndexes(db *sql.DB) error {
	for _, stmt := range []string{
		`CREATE INDEX idx_traditional ON cedict_dictionary(traditional_character)`,
		`CREATE INDEX idx_simplified ON cedict_dictionary(simplified_character)`,
		`CREATE INDEX idx_traditional_w_cd ON cedict_dictionary(traditional_character, w_cd)`,
		`CREATE INDEX idx_simplified_w_cd ON cedict_dictionary(simplified_character, w_cd)`,
	} {
		if _, err := db.Exec(stmt); err != nil {
			return err
		}
	}
	return nil
}

// createCedictFTS builds FTS5 for pinyin/definition search.
// Query: SELECT ... FROM cedict_dictionary d JOIN cedict_dictionary_fts fts ON d.id = fts.rowid WHERE fts MATCH $1
// Order by relevance: ORDER BY fts.rank (or bm25(fts) for FTS5)
func createCedictFTS(db *sql.DB) error {
	if _, err := db.Exec(`
		CREATE VIRTUAL TABLE cedict_dictionary_fts
		USING fts5(
			pinyin,
			pinyin_plain,
			pinyin_without_space,
			definition,
			content='cedict_dictionary',
			content_rowid='id'
		)
	`); err != nil {
		return err
	}
	_, err := db.Exec(`INSERT INTO cedict_dictionary_fts(cedict_dictionary_fts) VALUES ('rebuild')`)
	return err
}

func createSubtlexTable(db *sql.DB) error {
	_, err := db.Exec(`
		CREATE TABLE subtlex (
			word TEXT,
			w_cd INTEGER
		)
	`)
	return err
}

func populateSubtlex(ctx context.Context, db *sql.DB, sourcePath, sheetName string, batchSize int) error {
	f, err := excelize.OpenFile(sourcePath)
	if err != nil {
		return err
	}
	defer f.Close()

	rows, err := f.Rows(sheetName)
	if err != nil {
		return err
	}
	defer rows.Close()

	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	var batchArgs []interface{}
	var batchCount int

	for rows.Next() {
		cell, err := rows.Columns()
		if err != nil {
			return err
		}
		if len(cell) < 5 {
			continue
		}

		word := cell[0]
		wcd, err := strconv.Atoi(cell[4])
		if err != nil {
			continue
		}

		batchArgs = append(batchArgs, word, wcd)
		batchCount++

		if batchCount == batchSize {
			if err := execSubtlexBatch(tx, batchArgs, batchCount); err != nil {
				return err
			}
			batchArgs = batchArgs[:0]
			batchCount = 0
		}
	}

	if batchCount > 0 {
		if err := execSubtlexBatch(tx, batchArgs, batchCount); err != nil {
			return err
		}
	}

	return tx.Commit()
}

func execSubtlexBatch(tx *sql.Tx, batchArgs []interface{}, batchCount int) error {
	placeholders := valuePlaceholders(batchCount, 2)
	log.Infof("Inserting subtlex batch of %d...", batchCount)
	_, err := tx.Exec(fmt.Sprintf(`INSERT INTO subtlex (word, w_cd) VALUES %s`, placeholders), batchArgs...)
	return err
}

func valuePlaceholders(rows, cols int) string {
	row := "(" + strings.Repeat("?, ", cols-1) + "?)"
	var sb strings.Builder
	for i := 0; i < rows; i++ {
		if i > 0 {
			sb.WriteString(",")
		}
		sb.WriteString(row)
	}
	return sb.String()
}

func migrateSubtlexToCedict(db *sql.DB) error {
	log.Infof("Migrating SUBTLEX data to cedict_dictionary.w_cd...")
	if _, err := db.Exec(`CREATE INDEX idx_subtlex_word ON subtlex(word)`); err != nil {
		return err
	}
	_, err := db.Exec(`
		UPDATE cedict_dictionary
		SET w_cd = subtlex.w_cd
		FROM subtlex
		WHERE subtlex.word = cedict_dictionary.simplified_character
	`)
	if err != nil {
		return err
	}
	log.Infof("Dropping subtlex table...")
	_, err = db.Exec(`DROP TABLE subtlex`)
	return err
}

func vacuum(db *sql.DB) error {
	_, err := db.Exec(`VACUUM`)
	return err
}

func alphaOnly(s string) string {
	result := make([]rune, 0, len(s))
	for _, r := range s {
		if unicode.IsLetter(r) || r == ' ' {
			result = append(result, r)
		}
	}
	return string(result)
}
