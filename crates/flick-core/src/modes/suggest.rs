//! Search suggestions for Google and YouTube: the request address, reading
//! the reply, and a shared cache. Fetching is the app's job (behind
//! `SuggestionSource`), so this module never touches the network.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Whose suggestions to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    Google,
    YouTube,
}

/// Fetches suggestions in the background and stores them in the cache.
/// `request` must return at once; it may be called on every keystroke.
pub trait SuggestionSource: Send + Sync {
    fn request(&self, engine: Engine, query: &str);
}

/// Most entries the cache keeps before starting over.
pub const CACHE_LIMIT: usize = 256;

/// Suggestion lists by engine and exact query, shared between the UI and
/// the fetcher.
#[derive(Clone, Default)]
pub struct SuggestionCache(Arc<Mutex<CacheMap>>);

type CacheMap = HashMap<(Engine, String), Vec<String>>;

impl SuggestionCache {
    fn lock(&self) -> std::sync::MutexGuard<'_, CacheMap> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn get(&self, engine: Engine, query: &str) -> Option<Vec<String>> {
        self.lock().get(&(engine, query.to_owned())).cloned()
    }

    /// Suggestions for the longest earlier text that `query` starts with
    /// (for "rus": "ru", else "r"), to show while fresh ones load.
    pub fn best_prefix(&self, engine: Engine, query: &str) -> Option<Vec<String>> {
        self.lock()
            .iter()
            .filter(|((e, q), list)| {
                *e == engine && !list.is_empty() && query.starts_with(q.as_str())
            })
            .max_by_key(|((_, q), _)| q.len())
            .map(|(_, list)| list.clone())
    }

    pub fn insert(&self, engine: Engine, query: &str, suggestions: Vec<String>) {
        let mut map = self.lock();
        if map.len() >= CACHE_LIMIT {
            map.clear();
        }
        map.insert((engine, query.to_owned()), suggestions);
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// What a web mode needs to show suggestions.
#[derive(Clone)]
pub struct Suggestions {
    pub cache: SuggestionCache,
    pub source: Arc<dyn SuggestionSource>,
}

/// The suggestion address browsers use. It is not an official, documented
/// API: if Google changes it, suggestions stop and searching still works.
pub fn suggest_url(engine: Engine, query: &str) -> String {
    let mut params = vec![("client", "firefox"), ("ie", "utf-8"), ("oe", "utf-8")];
    if engine == Engine::YouTube {
        params.push(("ds", "yt"));
    }
    params.push(("q", query));
    url::Url::parse_with_params("https://suggestqueries.google.com/complete/search", &params)
        .map(String::from)
        .unwrap_or_default()
}

/// Reads `["query", ["suggestion", ...], ...]`; anything else gives nothing.
pub fn parse_suggestions(json: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(parts)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    match parts.get(1) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_suggestion_list() {
        let json = r#"["rust",["rust","rust game","rustlings"],[],{"google:suggestsubtypes":[]}]"#;
        assert_eq!(parse_suggestions(json), ["rust", "rust game", "rustlings"]);
    }

    #[test]
    fn reads_non_ascii_suggestions() {
        assert_eq!(
            parse_suggestions(r#"["bl",["blåbær","blåbærsyltetøy"]]"#),
            ["blåbær", "blåbærsyltetøy"]
        );
    }

    #[test]
    fn bad_replies_give_no_suggestions() {
        assert!(parse_suggestions("<html>rate limited</html>").is_empty());
        assert!(parse_suggestions(r#"{"q":"x"}"#).is_empty());
        assert!(parse_suggestions(r#"["x"]"#).is_empty());
    }

    #[test]
    fn request_address_per_engine() {
        assert_eq!(
            suggest_url(Engine::Google, "æ ø"),
            "https://suggestqueries.google.com/complete/search?client=firefox&ie=utf-8&oe=utf-8&q=%C3%A6+%C3%B8"
        );
        assert_eq!(
            suggest_url(Engine::YouTube, "lofi"),
            "https://suggestqueries.google.com/complete/search?client=firefox&ie=utf-8&oe=utf-8&ds=yt&q=lofi"
        );
    }

    #[test]
    fn cache_keeps_lists_per_engine_and_query() {
        let cache = SuggestionCache::default();
        assert_eq!(cache.get(Engine::Google, "rust"), None);
        cache.insert(Engine::Google, "rust", vec!["rust game".into()]);
        assert_eq!(
            cache.get(Engine::Google, "rust"),
            Some(vec!["rust game".into()])
        );
        assert_eq!(cache.get(Engine::YouTube, "rust"), None);
    }

    #[test]
    fn cache_stays_small() {
        let cache = SuggestionCache::default();
        for i in 0..(CACHE_LIMIT + 1) {
            cache.insert(Engine::Google, &format!("q{i}"), vec![]);
        }
        assert!(cache.len() <= CACHE_LIMIT);
        assert!(
            cache
                .get(Engine::Google, &format!("q{CACHE_LIMIT}"))
                .is_some()
        );
    }
}
