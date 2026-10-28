//! Web engines (Google, YouTube, GitHub, custom ones...): a row that
//! searches what you typed, then (for engines that have them, when
//! suggestions are on) the engine's suggestions as further rows.

use crate::catalog::{self, CustomEngine, EngineSpec, Target};
use crate::mode::{Action, ResultItem, ResultKind, SearchMode};
use crate::modes::suggest::{Engine, Suggestions};

/// A mode that fills the text into an address with `{query}`.
pub struct WebSearchMode {
    id: String,
    name: String,
    url: String,
    engine: Option<Engine>,
    suggestions: Option<Suggestions>,
}

impl WebSearchMode {
    /// A built-in web engine; `None` for one that is not a website.
    pub fn from_spec(spec: &EngineSpec) -> Option<Self> {
        let Target::Web { url, suggest } = spec.target else {
            return None;
        };
        Some(Self {
            id: spec.id.to_owned(),
            name: spec.name.to_owned(),
            url: url.to_owned(),
            engine: suggest,
            suggestions: None,
        })
    }

    /// An engine someone added; it has no suggestions.
    pub fn custom(engine: &CustomEngine) -> Self {
        Self {
            id: engine.id(),
            name: engine.name.clone(),
            url: engine.url.clone(),
            engine: None,
            suggestions: None,
        }
    }

    /// The catalog's engine with this id. Panics if it is not a website.
    pub fn builtin(id: &str) -> Self {
        catalog::find(id)
            .and_then(Self::from_spec)
            .unwrap_or_else(|| panic!("no web engine {id}"))
    }

    pub fn google() -> Self {
        Self::builtin("google")
    }

    pub fn youtube() -> Self {
        Self::builtin("youtube")
    }

    /// Shows the engine's suggestions under the search row (for engines
    /// that have them).
    pub fn with_suggestions(mut self, suggestions: Suggestions) -> Self {
        self.suggestions = Some(suggestions);
        self
    }

    fn search_url(&self, text: &str) -> String {
        catalog::fill(&self.url, text)
    }
}

impl SearchMode for WebSearchMode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn query(&self, text: &str, max_results: usize) -> Vec<ResultItem> {
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        let url = self.search_url(text);
        let mut rows = vec![ResultItem {
            title: format!("Search {} for \"{text}\"", self.name),
            subtitle: url.clone(),
            score: 0,
            action: Action::OpenUrl(url),
            kind: ResultKind::Web,
        }];

        if let (Some(suggestions), Some(engine)) = (&self.suggestions, self.engine) {
            let list = suggestions.cache.get(engine, text).or_else(|| {
                suggestions.source.request(engine, text);
                // Until they arrive, keep the last suggestions for what was
                // typed before, so the list does not shrink and grow per key.
                suggestions.cache.best_prefix(engine, text)
            });
            rows.extend(
                list.unwrap_or_default()
                    .into_iter()
                    .filter(|s| !s.eq_ignore_ascii_case(text))
                    .map(|s| {
                        let url = self.search_url(&s);
                        ResultItem {
                            title: s,
                            subtitle: String::new(),
                            score: 0,
                            action: Action::OpenUrl(url),
                            kind: ResultKind::Web,
                        }
                    }),
            );
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
            Action::OpenUrl("https://www.google.com/search?q=rust%20%26%20egui".into())
        );
        assert_eq!(r[0].title, "Search Google for \"rust & egui\"");
    }

    #[test]
    fn youtube_url() {
        let r = WebSearchMode::youtube().query("lofi beats", 8);
        assert_eq!(
            r[0].action,
            Action::OpenUrl("https://www.youtube.com/results?search_query=lofi%20beats".into())
        );
    }

    #[test]
    fn non_ascii_is_encoded() {
        let r = WebSearchMode::google().query("blåbær 🍓", 8);
        assert_eq!(
            r[0].action,
            Action::OpenUrl(
                "https://www.google.com/search?q=bl%C3%A5b%C3%A6r%20%F0%9F%8D%93".into()
            )
        );
    }

    #[test]
    fn every_catalog_website_becomes_a_mode() {
        let gh = WebSearchMode::builtin("github");
        assert_eq!((gh.id(), gh.name()), ("github", "GitHub"));
        assert_eq!(
            gh.query("egui", 8)[0].action,
            Action::OpenUrl("https://github.com/search?q=egui".into())
        );
        assert!(WebSearchMode::from_spec(crate::catalog::find("files").unwrap()).is_none());
    }

    #[test]
    fn custom_engines_search_their_address() {
        let mode = WebSearchMode::custom(&crate::catalog::CustomEngine {
            name: "Crates".into(),
            url: "https://crates.io/search?q={query}".into(),
        });
        assert_eq!((mode.id(), mode.name()), ("custom:crates", "Crates"));
        let r = mode.query("serde json", 8);
        assert_eq!(r[0].title, "Search Crates for \"serde json\"");
        assert_eq!(
            r[0].action,
            Action::OpenUrl("https://crates.io/search?q=serde%20json".into())
        );
    }

    #[test]
    fn engines_without_type_ahead_ask_for_nothing() {
        let (mode, _, recorder) = with_suggestions(WebSearchMode::builtin("github"));
        assert_eq!(mode.query("egui", 8).len(), 1);
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn wikipedia_asks_wikipedia_for_suggestions() {
        let (mode, _, recorder) = with_suggestions(WebSearchMode::builtin("wikipedia"));
        mode.query("rust", 8);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [(Engine::Wikipedia, "rust".to_owned())]
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
            Action::OpenUrl("https://www.google.com/search?q=rust%20game".into())
        );
        assert_eq!(r[2].kind, ResultKind::Web);
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn while_waiting_the_previous_suggestions_stay() {
        // "ru" has suggestions; "rus" is still on its way.
        let (mode, cache, recorder) = with_suggestions(WebSearchMode::google());
        cache.insert(Engine::Google, "ru", vec!["rust".into(), "ruby".into()]);
        let titles: Vec<_> = mode.query("rus", 8).into_iter().map(|i| i.title).collect();
        assert_eq!(titles, ["Search Google for \"rus\"", "rust", "ruby"]);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [(Engine::Google, "rus".to_owned())]
        );
    }

    #[test]
    fn exact_suggestions_win_over_older_ones() {
        let (mode, cache, _) = with_suggestions(WebSearchMode::google());
        cache.insert(Engine::Google, "ru", vec!["ruby".into()]);
        cache.insert(Engine::Google, "rus", vec!["rust".into()]);
        assert_eq!(mode.query("rus", 8)[1].title, "rust");
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
