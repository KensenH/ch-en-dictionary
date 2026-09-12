use crate::domain::dictionary::{DictionaryRepoTrait, Word};
use anyhow::Result;
use rusqlite::params_from_iter;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct DictionaryRepo {
    conn: Mutex<Connection>,
}

impl DictionaryRepoTrait for DictionaryRepo {
    fn search_by_hanzi(&self, query: &str) -> Result<Vec<Word>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut query_str = String::from(
            "SELECT
                id,
                traditional_character,
                simplified_character,
                pinyin,
                definition
             FROM cedict_dictionary",
        );

        if !query.is_empty() {
            query_str.push_str(
                " WHERE traditional_character LIKE $1 || '%' OR simplified_character LIKE $1 || '%'"
            );
        }

        if query.is_empty() {
            query_str.push_str(" ORDER BY w_cd DESC, id ASC LIMIT 20");
        } else {
            query_str.push_str(
                " ORDER BY (traditional_character = $1 OR simplified_character = $1) DESC,
                  w_cd DESC, id ASC LIMIT 20",
            );
        }

        let mut stmt = conn.prepare(query_str.as_str())?;

        let rows = if query.is_empty() {
            stmt.query_map([], map_word)?
        } else {
            stmt.query_map(params_from_iter([query]), map_word)?
        };

        let result: Vec<Word> = rows.collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }

    fn search_by_other(&self, query: &str) -> Result<Vec<Word>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut query_str = String::from(
            "SELECT
            cd.id,
            cd.traditional_character,
            cd.simplified_character,
            cd.pinyin,
            cd.definition,
            cd.w_cd
            FROM cedict_dictionary_fts
            JOIN cedict_dictionary cd
            ON cd.id = cedict_dictionary_fts.rowid
        ",
        );

        if !query.is_empty() {
            query_str.push_str(" WHERE cedict_dictionary_fts MATCH $1");
        }

        if query.is_empty() {
            query_str.push_str(" ORDER BY cd.w_cd DESC, cd.id ASC LIMIT 20");
        } else {
            // A complete slash-delimited translation beats an incidental mention.
            // Popularity orders direct translations; FTS relevance orders other matches.
            query_str.push_str(
                " ORDER BY
                  (instr('/' || lower(replace(replace(cd.definition, '!', ''), '?', '')) || '/', '/' || $2 || '/') > 0) DESC,
                  CASE WHEN instr('/' || lower(replace(replace(cd.definition, '!', ''), '?', '')) || '/', '/' || $2 || '/') > 0
                       THEN coalesce(cd.w_cd, 0) END DESC,
                  rank ASC, cd.w_cd DESC, cd.id ASC LIMIT 20",
            );
        }

        let mut stmt = conn.prepare(query_str.as_str())?;

        let fts_query = escape_fts_query(query);
        let rows = if query.is_empty() {
            stmt.query_map([], map_word)?
        } else {
            stmt.query_map(
                params_from_iter([fts_query.as_str(), query.to_lowercase().as_str()]),
                map_word,
            )?
        };

        let result: Vec<Word> = rows.collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }
}

fn map_word(row: &rusqlite::Row<'_>) -> rusqlite::Result<Word> {
    Ok(Word {
        id: row.get(0)?,
        traditional: row.get(1)?,
        simplified: row.get(2)?,
        pinyin: row.get(3)?,
        definition: row.get(4)?,
    })
}

fn escape_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('\"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

impl DictionaryRepo {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> DictionaryRepo {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE cedict_dictionary (
                    id INTEGER PRIMARY KEY,
                    traditional_character TEXT NOT NULL,
                    simplified_character TEXT NOT NULL,
                    pinyin TEXT NOT NULL,
                    definition TEXT NOT NULL,
                    w_cd INTEGER
                );
                CREATE VIRTUAL TABLE cedict_dictionary_fts USING fts5(
                    pinyin,
                    definition,
                    content='cedict_dictionary',
                    content_rowid='id'
                );
                INSERT INTO cedict_dictionary
                    (id, traditional_character, simplified_character, pinyin, definition, w_cd)
                VALUES (1, '你好', '你好', 'ni3 hao3', 'hello', 100);
                INSERT INTO cedict_dictionary_fts(cedict_dictionary_fts) VALUES ('rebuild');
                ",
            )
            .unwrap();

        DictionaryRepo::new(connection)
    }

    fn add_entries(repo: &DictionaryRepo, entries: &str) {
        repo.conn
            .lock()
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO cedict_dictionary
             (id, traditional_character, simplified_character, pinyin, definition, w_cd)
             VALUES {entries};
             INSERT INTO cedict_dictionary_fts(cedict_dictionary_fts) VALUES ('rebuild');"
            ))
            .unwrap();
    }

    #[test]
    fn exact_hanzi_precedes_popular_prefix_matches() {
        let repo = test_repo();
        add_entries(
            &repo,
            "(2, '書', '书', 'shu1', 'book', NULL),
                            (3, '書店', '书店', 'shu1 dian4', 'bookshop', 9000)",
        );
        for query in ["书", "書"] {
            let words = repo.search_by_hanzi(query).unwrap();
            assert_eq!(words[0].simplified, "书");
            assert_eq!(words[1].simplified, "书店");
        }
    }

    #[test]
    fn direct_translation_precedes_incidental_mentions_even_without_frequency() {
        let repo = test_repo();
        add_entries(
            &repo,
            "(2, '回', '回', 'hui2', 'chapter of a classic book', 9000),
                            (3, '書', '书', 'shu1', 'letter/book/document', NULL)",
        );
        let words = repo.search_by_other("BOOK").unwrap();
        assert_eq!(words[0].simplified, "书");
        assert_eq!(words[1].simplified, "回");
    }

    #[test]
    fn direct_greeting_ignores_terminal_punctuation() {
        let repo = test_repo();
        add_entries(
            &repo,
            "(2, '喂', '喂', 'wei2', 'hello (when answering the phone)', 9000),
                            (3, '您好', '您好', 'nin2 hao3', 'Hello!/Hi!', 200)",
        );
        assert_eq!(repo.search_by_other("hello").unwrap()[0].simplified, "您好");
    }

    #[test]
    fn popularity_orders_equally_direct_translations() {
        let repo = test_repo();
        add_entries(
            &repo,
            "(2, '蘋', '苹', 'ping2', 'apple', 28),
                            (3, '蘋果', '苹果', 'ping2 guo3', 'apple/classifier note', 446)",
        );
        assert_eq!(repo.search_by_other("apple").unwrap()[0].simplified, "苹果");
    }

    #[test]
    fn relevance_precedes_popularity_for_other_matches() {
        let repo = test_repo();
        add_entries(&repo, "(2, '甲', '甲', 'jia3', 'a book', NULL),
                            (3, '乙', '乙', 'yi3', 'an incidental mention of a book in a long explanatory note', 9000)");
        assert_eq!(repo.search_by_other("book").unwrap()[0].simplified, "甲");
    }

    #[test]
    fn empty_search_keeps_popularity_order() {
        let repo = test_repo();
        add_entries(&repo, "(2, '書', '书', 'shu1', 'book', 9000)");
        assert_eq!(repo.search_by_hanzi("").unwrap()[0].simplified, "书");
        assert_eq!(repo.search_by_other("").unwrap()[0].simplified, "书");
    }

    #[test]
    fn searches_hanzi_by_prefix() {
        let repo = test_repo();

        let words = repo.search_by_hanzi("你").unwrap();

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].definition, "hello");
    }

    #[test]
    fn searches_other_terms_without_allowing_fts_syntax_errors() {
        let repo = test_repo();

        let words = repo.search_by_other("\"hello").unwrap();

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].traditional, "你好");
    }
}
