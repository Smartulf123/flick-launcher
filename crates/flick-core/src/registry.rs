//! Registered search modes, their action keywords, and prefix parsing.

use crate::mode::SearchMode;

/// Why a mode could not be registered.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("an action keyword cannot be empty")]
    EmptyKeyword,
    #[error("action keyword {0:?} contains whitespace")]
    KeywordHasWhitespace(String),
    #[error("action keyword {0:?} is used by two modes")]
    DuplicateKeyword(String),
}

/// The result of reading the search bar's text.
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedQuery<'a> {
    /// Index of the mode that should answer.
    pub mode: usize,
    /// The text to search for, without the keyword prefix.
    pub text: &'a str,
    /// Whether a keyword prefix chose the mode.
    pub prefixed: bool,
}

/// The modes in Tab order, each with its lowercase action keyword.
#[derive(Default)]
pub struct ModeRegistry {
    entries: Vec<(String, Box<dyn SearchMode>)>,
}

impl ModeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        keyword: &str,
        mode: Box<dyn SearchMode>,
    ) -> Result<(), RegistryError> {
        if keyword.is_empty() {
            return Err(RegistryError::EmptyKeyword);
        }
        if keyword.chars().any(char::is_whitespace) {
            return Err(RegistryError::KeywordHasWhitespace(keyword.to_owned()));
        }
        let keyword = keyword.to_lowercase();
        if self.entries.iter().any(|(k, _)| *k == keyword) {
            return Err(RegistryError::DuplicateKeyword(keyword));
        }
        self.entries.push((keyword, mode));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The mode at `index`. Panics if `index >= len()`.
    pub fn mode(&self, index: usize) -> &dyn SearchMode {
        self.entries[index].1.as_ref()
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|(_, m)| m.id() == id)
    }

    /// Picks the mode for `input`: a leading `<keyword><space>` wins, otherwise `active`.
    pub fn parse<'a>(&self, input: &'a str, active: usize) -> ParsedQuery<'a> {
        if let Some((first, rest)) = input.split_once(' ')
            && let Some(mode) = self
                .entries
                .iter()
                .position(|(k, _)| k.eq_ignore_ascii_case(first))
        {
            return ParsedQuery {
                mode,
                text: rest,
                prefixed: true,
            };
        }
        ParsedQuery {
            mode: active,
            text: input,
            prefixed: false,
        }
    }

    pub fn next(&self, index: usize) -> usize {
        (index + 1) % self.len()
    }

    pub fn prev(&self, index: usize) -> usize {
        (index + self.len() - 1) % self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::ResultItem;

    struct Dummy(&'static str);

    impl SearchMode for Dummy {
        fn id(&self) -> &str {
            self.0
        }
        fn name(&self) -> &str {
            self.0
        }
        fn query(&self, _text: &str, _max_results: usize) -> Vec<ResultItem> {
            Vec::new()
        }
    }

    fn registry() -> ModeRegistry {
        let mut r = ModeRegistry::new();
        r.register("g", Box::new(Dummy("google"))).unwrap();
        r.register("yt", Box::new(Dummy("youtube"))).unwrap();
        r.register("f", Box::new(Dummy("files"))).unwrap();
        r
    }

    #[test]
    fn prefix_selects_mode() {
        let r = registry();
        assert_eq!(
            r.parse("g rust", 2),
            ParsedQuery {
                mode: 0,
                text: "rust",
                prefixed: true
            }
        );
        assert_eq!(
            r.parse("yt lofi beats", 0),
            ParsedQuery {
                mode: 1,
                text: "lofi beats",
                prefixed: true
            }
        );
        assert_eq!(
            r.parse("f bdgt", 0),
            ParsedQuery {
                mode: 2,
                text: "bdgt",
                prefixed: true
            }
        );
    }

    #[test]
    fn keywords_are_case_insensitive() {
        assert_eq!(registry().parse("YT x", 0).mode, 1);
    }

    #[test]
    fn no_prefix_uses_active_mode() {
        assert_eq!(
            registry().parse("rust", 1),
            ParsedQuery {
                mode: 1,
                text: "rust",
                prefixed: false
            }
        );
    }

    #[test]
    fn word_starting_like_keyword_is_not_a_prefix() {
        let r = registry();
        for input in ["gaming", "fyi", "ytd stats", "g"] {
            let p = r.parse(input, 2);
            assert_eq!((p.mode, p.text, p.prefixed), (2, input, false), "{input}");
        }
    }

    #[test]
    fn cycling_wraps_both_ways() {
        let r = registry();
        assert_eq!((r.next(0), r.next(1), r.next(2)), (1, 2, 0));
        assert_eq!((r.prev(0), r.prev(2)), (2, 1));
    }

    #[test]
    fn bad_keywords_are_rejected() {
        let mut r = registry();
        assert_eq!(
            r.register("g", Box::new(Dummy("x"))),
            Err(RegistryError::DuplicateKeyword("g".into()))
        );
        assert_eq!(
            r.register("G", Box::new(Dummy("x"))),
            Err(RegistryError::DuplicateKeyword("g".into()))
        );
        assert_eq!(
            r.register("", Box::new(Dummy("x"))),
            Err(RegistryError::EmptyKeyword)
        );
        assert_eq!(
            r.register("a b", Box::new(Dummy("x"))),
            Err(RegistryError::KeywordHasWhitespace("a b".into()))
        );
    }
}
