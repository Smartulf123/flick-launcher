//! Google and YouTube: a row that searches what you typed, then (when
//! suggestions are on) the engine's suggestions as further rows.

use crate::mode::{Action, ResultItem, ResultKind, SearchMode};
use crate::modes::suggest::{Engine, Suggestions};

/// A mode that turns the query into search URLs.
pub struct WebSearchMode {
    id: &'static str,
    name: &'static str,
    base_url: &'static str,
    param: &'static str,
    engine: Engine,
    suggestions: Option<Suggestions>,
}

impl WebSearchMode {
    pub fn google() -> Self {
        Self {
            id: "google",
            name: "Google",
            base_url: "https://www.google.com/search",
            param: "q",
            engine: Engine::Google,
            suggestions: None,
        }
    }

    pub fn youtube() -> Self {
        Self {
            id: "youtube",
            name: "YouTube",
            base_url: "https://www.youtube.com/results",
            param: "search_query",
            engine: Engine::YouTube,
            suggestions: None,
        }
    }

    /// Shows the engine's suggestions under the search row.
    pub fn with_suggestions(mut self, suggestions: Suggestions) -> Self {
        self.suggestions = Some(suggestions);
        self
    }

    fn search_url(&self, text: &str) -> Option<String> {
        url::Url::parse_with_params(self.base_url, &[(self.param, text)])
            .ok()
            .map(String::from)
    }
}

impl SearchMode for WebSearchMode {
    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &str {
        self.name
    }

    fn query(&self, text: &str, max_results: usize) -> Vec<ResultItem> {
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        let Some(url) = self.search_url(text) else {
            return Vec::new();
        };
        let mut rows = vec![ResultItem {
            title: format!("Search {} for \"{text}\"", self.name),
            subtitle: url.clone(),
            score: 0,
            action: Action::OpenUrl(url),
            kind: ResultKind::Web,
        }];

        if let Some(suggestions) = &self.suggestions {
            match suggestions.cache.get(self.engine, text) {
                Some(list) => rows.extend(
                    list.into_iter()
                        .filter(|s| !s.eq_ignore_ascii_case(text))
                        .filter_map(|s| {
                            let url = self.search_url(&s)?;
                            Some(ResultItem {
                                title: s,
                                subtitle: String::new(),
                                score: 0,
                                action: Action::OpenUrl(url),
                                kind: ResultKind::Web,
                            })
                        }),
                ),
                None => suggestions.source.request(self.engine, text),
            }
        }
        rows.truncate(max_results.max(1));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{Action, ResultKind, SearchMode};

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

    use crate::modes::suggest::{Engine, SuggestionCache, SuggestionSource, Suggestions};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Recorder(Mutex<Vec<(Engine, String)>>);

    impl SuggestionSource for Recorder {
        fn request(&self, engine: Engine, query: &str) {
            self.0.lock().unwrap().push((engine, query.to_owned()));
        }
    }

    fn with_suggestions(mode: WebSearchMode) -> (WebSearchMode, SuggestionCache, Arc<Recorder>) {
        let cache = SuggestionCache::default();
        let recorder = Arc::new(Recorder::default());
        let mode = mode.with_suggestions(Suggestions {
            cache: cache.clone(),
            source: recorder.clone(),
        });
        (mode, cache, recorder)
    }

    #[test]
    fn asks_for_suggestions_when_none_are_cached() {
        let (mode, _, recorder) = with_suggestions(WebSearchMode::google());
        assert_eq!(mode.query("  rus ", 8).len(), 1);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [(Engine::Google, "rus".to_owned())]
        );
    }

    #[test]
    fn cached_suggestions_follow_the_search_row() {
        let (mode, cache, recorder) = with_suggestions(WebSearchMode::google());
        cache.insert(
            Engine::Google,
            "rus",
            vec!["Rus".into(), "rust".into(), "rust game".into()],
        );
        let r = mode.query("rus", 8);
        let titles: Vec<_> = r.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["Search Google for \"rus\"", "rust", "rust game"]);
        assert_eq!(
            r[2].action,
            Action::OpenUrl("https://www.google.com/search?q=rust+game".into())
        );
        assert_eq!(r[2].kind, ResultKind::Web);
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn suggestions_respect_max_results() {
        let (mode, cache, _) = with_suggestions(WebSearchMode::google());
        cache.insert(
            Engine::Google,
            "a",
            vec!["ab".into(), "ac".into(), "ad".into()],
        );
        assert_eq!(mode.query("a", 2).len(), 2);
    }

    #[test]
    fn youtube_asks_youtube_for_suggestions() {
        let (mode, _, recorder) = with_suggestions(WebSearchMode::youtube());
        mode.query("lofi", 8);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [(Engine::YouTube, "lofi".to_owned())]
        );
    }

    #[test]
    fn blank_query_asks_for_nothing() {
        let (mode, _, recorder) = with_suggestions(WebSearchMode::google());
        assert!(mode.query("   ", 8).is_empty());
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn web_results_are_web_kind() {
        assert_eq!(
            WebSearchMode::youtube().query("cats", 8)[0].kind,
            ResultKind::Web
        );
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
