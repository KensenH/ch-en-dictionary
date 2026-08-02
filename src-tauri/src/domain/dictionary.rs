use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Word {
    pub id: i32,
    pub traditional: String,
    pub simplified: String,
    pub pinyin: String,
    pub definition: String,
}

pub trait DictionaryRepoTrait {
    fn search_by_hanzi(&self, query: &str) -> Result<Vec<Word>>;
    fn search_by_other(&self, query: &str) -> Result<Vec<Word>>;
}

pub trait DictionaryCommandTrait {
    fn search(&self, query: &str) -> Result<Vec<Word>>;
}
