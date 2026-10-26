//! Google and YouTube: one result that opens a search URL in the default browser.

use crate::mode::{Action, ResultItem, SearchMode};

/// A mode that turns the query into one search URL.
pub struct WebSearchMode {
    id: &'static str,
    name: &'static str,
    base_url: &'static str,
    param: &'static str,
}

impl WebSearchMode {
    pub fn google() -> Self {
        Self {
            id: "google",
            name: "Google",
            base_url: "https://www.google.com/search",
            param: "q",
        }
    }

    pub fn youtube() -> Self {
        Self {
            id: "youtube",
            name: "YouTube",
            base_url: "https://www.youtube.com/results",
            param: "search_query",
        }
    }
}

impl SearchMode for WebSearchMode {
    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &str {
        self.name
    }

    fn query(&self, text: &str, _max_results: usize) -> Vec<ResultItem> {
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        let Ok(url) = url::Url::parse_with_params(self.base_url, &[(self.param, text)]) else {
            return Vec::new();
        };
        vec![ResultItem {
            title: format!("Search {} for \"{text}\"", self.name),
            subtitle: url.to_string(),
            score: 0,
            action: Action::OpenUrl(url.into()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{Action, SearchMode};

    #[test]
    fn google_url() {
        let r = WebSearchMode::google().query("rust & egui", 8);
        assert_eq!(r.len(), 1);
        assert_eq!(
            r[0].action,
            Action::OpenUrl("https://www.google.com/search?q=rust+%26+egui".into())
        );
        assert_eq!(r[0].title, "Search Google for \"rust & egui\"");
    }

    #[test]
    fn youtube_url() {
        let r = WebSearchMode::youtube().query("lofi beats", 8);
        assert_eq!(
            r[0].action,
            Action::OpenUrl("https://www.youtube.com/results?search_query=lofi+beats".into())
        );
    }

    #[test]
    fn non_ascii_is_encoded() {
        let r = WebSearchMode::google().query("blåbær 🍓", 8);
        assert_eq!(
            r[0].action,
            Action::OpenUrl("https://www.google.com/search?q=bl%C3%A5b%C3%A6r+%F0%9F%8D%93".into())
        );
    }

    #[test]
    fn blank_query_gives_nothing() {
        assert!(WebSearchMode::google().query("", 8).is_empty());
        assert!(WebSearchMode::youtube().query("   ", 8).is_empty());
    }

    #[test]
    fn surrounding_spaces_are_trimmed() {
        let r = WebSearchMode::google().query("  rust  ", 8);
        assert_eq!(
            r[0].action,
            Action::OpenUrl("https://www.google.com/search?q=rust".into())
        );
    }
}
