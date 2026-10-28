//! Fetches search suggestions on a background thread, waiting
//! for a pause in typing so one request goes out per pause, not per key.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flick_core::modes::suggest::{
    Engine, SuggestionCache, SuggestionSource, parse_suggestions, suggest_url,
};

/// How long typing must pause before suggestions are fetched.
const PAUSE: Duration = Duration::from_millis(150);
const TIMEOUT: Duration = Duration::from_secs(3);

/// The app's `SuggestionSource`: hands requests to the fetching thread.
pub struct Fetcher {
    requests: Mutex<mpsc::Sender<(Engine, String)>>,
}

impl SuggestionSource for Fetcher {
    fn request(&self, engine: Engine, query: &str) {
        let _ = self
            .requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send((engine, query.to_owned()));
    }
}

/// Starts the real fetcher: HTTPS through Windows' own TLS (no C code in
/// the build). `on_update` runs after new suggestions land in `cache`.
pub fn start(cache: SuggestionCache, on_update: impl Fn() + Send + 'static) -> Arc<Fetcher> {
    let agent = crate::http::agent(TIMEOUT);
    start_with(
        cache,
        move |engine, query| {
            let body = crate::http::get_bytes(&agent, &suggest_url(engine, query))?;
            Ok(parse_suggestions(&String::from_utf8_lossy(&body)))
        },
        on_update,
    )
}

/// The fetching loop with the network call passed in, so tests can run it.
fn start_with(
    cache: SuggestionCache,
    fetch: impl Fn(Engine, &str) -> anyhow::Result<Vec<String>> + Send + 'static,
    on_update: impl Fn() + Send + 'static,
) -> Arc<Fetcher> {
    let (tx, rx) = mpsc::channel::<(Engine, String)>();
    std::thread::spawn(move || {
        while let Ok(mut job) = rx.recv() {
            // Keep taking newer text until typing pauses.
            while let Ok(newer) = rx.recv_timeout(PAUSE) {
                job = newer;
            }
            let (engine, query) = job;
            if cache.get(engine, &query).is_some() {
                continue;
            }
            match fetch(engine, &query) {
                Ok(list) => {
                    cache.insert(engine, &query, list);
                    on_update();
                }
                Err(e) => {
                    log::info!("no suggestions for {query:?}: {e:#}");
                    // Remember the miss so an offline PC is not asked on every frame.
                    cache.insert(engine, &query, Vec::new());
                }
            }
        }
    });
    Arc::new(Fetcher {
        requests: Mutex::new(tx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn a_burst_of_keys_fetches_only_the_last_text() {
        let cache = SuggestionCache::default();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (done_tx, done_rx) = mpsc::channel();
        let seen = calls.clone();
        let fetcher = start_with(
            cache.clone(),
            move |engine, query| {
                seen.lock().unwrap().push((engine, query.to_owned()));
                Ok(vec![format!("{query} suggestion")])
            },
            move || done_tx.send(()).unwrap(),
        );
        for q in ["r", "ru", "rus"] {
            fetcher.request(Engine::Google, q);
        }
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(*calls.lock().unwrap(), [(Engine::Google, "rus".to_owned())]);
        assert_eq!(
            cache.get(Engine::Google, "rus"),
            Some(vec!["rus suggestion".to_owned()])
        );
    }

    /// Talks to Google: run by hand with `cargo test -- --ignored live`.
    #[test]
    #[ignore]
    fn live_google_suggestions() {
        let cache = SuggestionCache::default();
        let (done_tx, done_rx) = mpsc::channel();
        let fetcher = start(cache.clone(), move || done_tx.send(()).unwrap());
        fetcher.request(Engine::Google, "rust");
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let list = cache.get(Engine::Google, "rust").unwrap();
        println!("LIVE {list:?}");
        assert!(!list.is_empty());
    }

    #[test]
    fn a_failed_fetch_is_remembered_as_no_suggestions() {
        let cache = SuggestionCache::default();
        let fetcher = start_with(cache.clone(), |_, _| anyhow::bail!("offline"), || {});
        fetcher.request(Engine::YouTube, "lofi");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while cache.get(Engine::YouTube, "lofi").is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(cache.get(Engine::YouTube, "lofi"), Some(vec![]));
    }
}
