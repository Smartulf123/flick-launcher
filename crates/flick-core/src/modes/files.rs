//! Files: fuzzy search over the file index by file name.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

use crate::index::{FileEntry, SharedIndex};
use crate::mode::{Action, ResultItem, SearchMode};

/// Title of the single row shown before the first index build finishes.
pub const INDEXING_TITLE: &str = "Indexing files...";

pub struct FilesMode {
    index: SharedIndex,
}

impl FilesMode {
    pub fn new(index: SharedIndex) -> Self {
        Self { index }
    }
}

/// Lets nucleo match on the file name while keeping the whole entry.
struct ByName<'a>(&'a FileEntry);

impl AsRef<str> for ByName<'_> {
    fn as_ref(&self) -> &str {
        &self.0.name
    }
}

impl SearchMode for FilesMode {
    fn id(&self) -> &str {
        "files"
    }

    fn name(&self) -> &str {
        "Files"
    }

    fn query(&self, text: &str, max_results: usize) -> Vec<ResultItem> {
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        let guard = self.index.read().unwrap_or_else(|p| p.into_inner());
        let Some(index) = guard.as_ref() else {
            return vec![ResultItem {
                title: INDEXING_TITLE.into(),
                subtitle: String::new(),
                score: 0,
                action: Action::None,
            }];
        };

        let pattern = Pattern::parse(text, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut matches = pattern.match_list(index.entries.iter().map(ByName), &mut matcher);
        matches.sort_by(|(a, sa), (b, sb)| sb.cmp(sa).then(a.0.name.len().cmp(&b.0.name.len())));

        matches
            .into_iter()
            .take(max_results)
            .map(|(ByName(entry), score)| ResultItem {
                title: entry.name.clone(),
                subtitle: entry
                    .path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                score,
                action: Action::OpenPath(entry.path.clone()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{FileEntry, FileIndex, SharedIndex};
    use crate::mode::{Action, ResultItem, SearchMode};
    use std::path::PathBuf;

    fn mode_with(names: &[&str]) -> FilesMode {
        let entries = names
            .iter()
            .map(|n| FileEntry {
                path: PathBuf::from(format!(r"C:\test\{n}")),
                name: (*n).to_owned(),
                is_dir: false,
            })
            .collect();
        let shared = SharedIndex::default();
        *shared.write().unwrap() = Some(FileIndex { entries });
        FilesMode::new(shared)
    }

    #[test]
    fn abbreviation_finds_budget_first() {
        let r = mode_with(&["notes.txt", "big_data.txt", "budget_2026.xlsx"]).query("bdgt", 8);
        assert_eq!(r[0].title, "budget_2026.xlsx");
        assert!(
            r.iter()
                .all(|i| i.title != "notes.txt" && i.title != "big_data.txt")
        );
        assert_eq!(
            r[0].action,
            Action::OpenPath(r"C:\test\budget_2026.xlsx".into())
        );
        assert_eq!(r[0].subtitle, r"C:\test");
    }

    #[test]
    fn case_and_accents_are_ignored() {
        assert_eq!(
            mode_with(&["Blåbær.txt"]).query("bla", 8)[0].title,
            "Blåbær.txt"
        );
    }

    #[test]
    fn respects_max_results() {
        assert_eq!(mode_with(&["a1", "a2", "a3", "a4"]).query("a", 2).len(), 2);
    }

    #[test]
    fn blank_query_gives_nothing() {
        assert!(mode_with(&["a.txt"]).query("  ", 8).is_empty());
    }

    #[test]
    fn shows_indexing_row_before_first_build() {
        let r = FilesMode::new(SharedIndex::default()).query("x", 8);
        assert_eq!(
            r,
            vec![ResultItem {
                title: INDEXING_TITLE.into(),
                subtitle: String::new(),
                score: 0,
                action: Action::None,
            }]
        );
    }

    #[test]
    fn ranking_snapshot() {
        let r = mode_with(&[
            "report.docx",
            "rust_book.pdf",
            "Rust",
            "trust.txt",
            "ruby.rb",
        ])
        .query("rust", 8);
        insta::assert_debug_snapshot!(r.iter().map(|i| &i.title).collect::<Vec<_>>());
    }
}
