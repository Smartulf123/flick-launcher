//! Wallhaven (wallhaven.cc) search: the request address and reading the
//! reply. Safe-for-work only, always. Fetching lives in `gallery_loader`.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde_json::Value;

const SEARCH: &str = "https://wallhaven.cc/api/v1/search";

/// Which Wallhaven categories to search.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Categories {
    pub general: bool,
    pub anime: bool,
    pub people: bool,
}

impl Default for Categories {
    fn default() -> Self {
        Self {
            general: true,
            anime: true,
            people: true,
        }
    }
}

impl Categories {
    /// Wallhaven's three-digit on/off mask: general, anime, people.
    pub fn mask(self) -> String {
        [self.general, self.anime, self.people]
            .iter()
            .map(|on| if *on { '1' } else { '0' })
            .collect()
    }
}

/// The search address. `purity=100` (safe for work) is always sent; an
/// empty query lists the most popular pictures.
pub fn search_url(query: &str, categories: Categories, page: u32) -> String {
    let query = query.trim();
    let mask = categories.mask();
    let page = page.to_string();
    let mut params: Vec<(&str, &str)> = Vec::new();
    if !query.is_empty() {
        params.push(("q", query));
    }
    params.push(("categories", &mask));
    params.push(("purity", "100"));
    params.push((
        "sorting",
        if query.is_empty() {
            "toplist"
        } else {
            "relevance"
        },
    ));
    params.push(("page", &page));
    url::Url::parse_with_params(SEARCH, &params)
        .map(String::from)
        .unwrap_or_default()
}

/// One search result.
#[derive(Clone, Debug, PartialEq)]
pub struct Wallpaper {
    pub id: String,
    pub thumb_url: String,
    pub full_url: String,
    /// The picture's page on wallhaven.cc (credit to the uploader).
    pub page_url: String,
    pub resolution: String,
}

pub struct SearchPage {
    pub wallpapers: Vec<Wallpaper>,
    pub current_page: u32,
    pub last_page: u32,
}

/// Reads a search reply, keeping only safe-for-work pictures.
pub fn parse_search(json: &str) -> anyhow::Result<SearchPage> {
    let reply: Value = serde_json::from_str(json).context("Wallhaven sent something unexpected")?;
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let wallpapers = reply
        .get("data")
        .and_then(Value::as_array)
        .context("Wallhaven reply has no results list")?
        .iter()
        .filter(|w| w.get("purity").and_then(Value::as_str) == Some("sfw"))
        .filter_map(|w| {
            Some(Wallpaper {
                id: text(w, "id")?,
                thumb_url: text(w.get("thumbs")?, "small")?,
                full_url: text(w, "path")?,
                page_url: text(w, "url")?,
                resolution: text(w, "resolution").unwrap_or_default(),
            })
        })
        .collect();
    let meta = reply.get("meta");
    let number = |key: &str| {
        meta.and_then(|m| m.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(1) as u32
    };
    Ok(SearchPage {
        wallpapers,
        current_page: number("current_page"),
        last_page: number("last_page"),
    })
}

/// Where a downloaded picture is saved; `None` for an id that is not plain
/// letters and digits (so a strange reply can never write outside `dir`).
pub fn download_path(dir: &Path, id: &str) -> Option<PathBuf> {
    let safe = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric());
    safe.then(|| dir.join(format!("wallhaven-{id}.jpg")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = r#"{"data":[
        {"id":"mlzoy1","url":"https://wallhaven.cc/w/mlzoy1","purity":"sfw","resolution":"3840x2160",
         "path":"https://w.wallhaven.cc/full/ml/wallhaven-mlzoy1.png","thumbs":{"small":"https://th.wallhaven.cc/small/ml/mlzoy1.jpg"}},
        {"id":"9oo8k1","url":"https://wallhaven.cc/w/9oo8k1","purity":"sfw","resolution":"4096x2304",
         "path":"https://w.wallhaven.cc/full/9o/wallhaven-9oo8k1.jpg","thumbs":{"small":"https://th.wallhaven.cc/small/9o/9oo8k1.jpg"}}
      ],"meta":{"current_page":1,"last_page":9}}"#;

    #[test]
    fn query_searches_by_relevance_safe_only() {
        assert_eq!(
            search_url(
                "anime girl",
                Categories {
                    general: false,
                    anime: true,
                    people: false
                },
                1
            ),
            "https://wallhaven.cc/api/v1/search?q=anime+girl&categories=010&purity=100&sorting=relevance&page=1"
        );
    }

    #[test]
    fn empty_query_shows_the_toplist() {
        assert_eq!(
            search_url("  ", Categories::default(), 2),
            "https://wallhaven.cc/api/v1/search?categories=111&purity=100&sorting=toplist&page=2"
        );
    }

    #[test]
    fn parses_results_and_paging() {
        let page = parse_search(REPLY).unwrap();
        assert_eq!((page.current_page, page.last_page), (1, 9));
        assert_eq!(page.wallpapers.len(), 2);
        assert_eq!(
            page.wallpapers[0],
            Wallpaper {
                id: "mlzoy1".into(),
                thumb_url: "https://th.wallhaven.cc/small/ml/mlzoy1.jpg".into(),
                full_url: "https://w.wallhaven.cc/full/ml/wallhaven-mlzoy1.png".into(),
                page_url: "https://wallhaven.cc/w/mlzoy1".into(),
                resolution: "3840x2160".into(),
            }
        );
    }

    #[test]
    fn non_safe_results_are_dropped() {
        let reply = REPLY.replacen(r#""purity":"sfw""#, r#""purity":"sketchy""#, 1);
        let page = parse_search(&reply).unwrap();
        let ids: Vec<_> = page.wallpapers.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["9oo8k1"]);
    }

    #[test]
    fn bad_json_is_an_error() {
        assert!(parse_search("<html>").is_err());
    }

    #[test]
    fn download_names_are_safe() {
        assert_eq!(
            download_path(Path::new(r"C:\bg"), "mlzoy1"),
            Some(PathBuf::from(r"C:\bg\wallhaven-mlzoy1.jpg"))
        );
        assert_eq!(download_path(Path::new(r"C:\bg"), r"..\evil"), None);
        assert_eq!(download_path(Path::new(r"C:\bg"), ""), None);
    }
}
