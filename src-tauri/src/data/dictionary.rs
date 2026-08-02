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

        query_str.push_str(" ORDER BY w_cd DESC LIMIT 20");

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

        query_str.push_str(" ORDER BY cd.w_cd DESC, rank ASC LIMIT 20");

        let mut stmt = conn.prepare(query_str.as_str())?;

        let fts_query = escape_fts_query(query);
        let rows = if query.is_empty() {
            stmt.query_map([], map_word)?
        } else {
            stmt.query_map(params_from_iter([fts_query.as_str()]), map_word)?
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
                    w_cd INTEGER NOT NULL
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
