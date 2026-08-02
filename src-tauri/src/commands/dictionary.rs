use crate::domain::dictionary::Word;
use crate::domain::dictionary::{DictionaryCommandTrait, DictionaryRepoTrait};
use anyhow::Result;

pub struct DictionaryCommand<R: DictionaryRepoTrait> {
    repo: R,
}

impl<R: DictionaryRepoTrait> DictionaryCommandTrait for DictionaryCommand<R> {
    fn search(&self, query: &str) -> Result<Vec<Word>> {
        let query = query.trim();

        if Self::contains_hanzi(query) || query.is_empty() {
            self.repo.search_by_hanzi(query)
        } else {
            self.repo.search_by_other(query)
        }
    }
}

impl<R: DictionaryRepoTrait> DictionaryCommand<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    fn contains_hanzi(s: &str) -> bool {
        s.chars().any(|character| {
            ('\u{3400}'..='\u{4DBF}').contains(&character)
                || ('\u{4E00}'..='\u{9FFF}').contains(&character)
                || ('\u{20000}'..='\u{2FA1F}').contains(&character)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RecordingRepo {
        route: Mutex<Option<&'static str>>,
    }

    impl DictionaryRepoTrait for RecordingRepo {
        fn search_by_hanzi(&self, _query: &str) -> Result<Vec<Word>> {
            *self.route.lock().unwrap() = Some("hanzi");
            Ok(Vec::new())
        }

        fn search_by_other(&self, _query: &str) -> Result<Vec<Word>> {
            *self.route.lock().unwrap() = Some("other");
            Ok(Vec::new())
        }
    }

    #[test]
    fn routes_hanzi_queries_to_hanzi_search() {
        let repo = RecordingRepo {
            route: Mutex::new(None),
        };
        let command = DictionaryCommand::new(repo);

        command.search("  你好  ").unwrap();

        assert_eq!(*command.repo.route.lock().unwrap(), Some("hanzi"));
    }

    #[test]
    fn routes_non_hanzi_queries_to_full_text_search() {
        let repo = RecordingRepo {
            route: Mutex::new(None),
        };
        let command = DictionaryCommand::new(repo);

        command.search(" hello ").unwrap();

        assert_eq!(*command.repo.route.lock().unwrap(), Some("other"));
    }

    #[test]
    fn treats_empty_queries_as_hanzi_searches() {
        let repo = RecordingRepo {
            route: Mutex::new(None),
        };
        let command = DictionaryCommand::new(repo);

        command.search("   ").unwrap();

        assert_eq!(*command.repo.route.lock().unwrap(), Some("hanzi"));
    }
}
