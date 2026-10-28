//! Every engine Flick knows: the built-in catalog, and engines people add
//! themselves.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::modes::suggest::Engine;

/// Where the search text goes in an engine's address.
pub const QUERY: &str = "{query}";

/// A custom engine's id is this followed by its name in lowercase.
pub const CUSTOM_PREFIX: &str = "custom:";

/// The headings the engine list is grouped under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    OnThisPc,
    WebSearch,
    Ai,
    VideoMusic,
    Knowledge,
    Code,
    Shopping,
    Social,
}

/// The groups in the order the list shows them.
pub const GROUPS: [Group; 8] = [
    Group::OnThisPc,
    Group::WebSearch,
    Group::Ai,
    Group::VideoMusic,
    Group::Knowledge,
    Group::Code,
    Group::Shopping,
    Group::Social,
];

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::OnThisPc => "On this PC",
            Group::WebSearch => "Web search",
            Group::Ai => "AI",
            Group::VideoMusic => "Video & music",
            Group::Knowledge => "Knowledge",
            Group::Code => "Code",
            Group::Shopping => "Shopping",
            Group::Social => "Social",
        }
    }
}

/// What an engine searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The file index on this computer.
    Files,
    /// A website: `url` with `{query}` where the text goes, and whose
    /// suggestions to show while typing, if any.
    Web {
        url: &'static str,
        suggest: Option<Engine>,
    },
}

/// One built-in engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineSpec {
    /// Stable name in the settings file, e.g. `"github"`.
    pub id: &'static str,
    /// Shown on the chip and in the list.
    pub name: &'static str,
    pub group: Group,
    pub target: Target,
}

const fn web(
    id: &'static str,
    name: &'static str,
    group: Group,
    url: &'static str,
    suggest: Option<Engine>,
) -> EngineSpec {
    EngineSpec {
        id,
        name,
        group,
        target: Target::Web { url, suggest },
    }
}

use Group::*;

/// Every built-in engine, grouped in list order.
#[rustfmt::skip]
pub const CATALOG: &[EngineSpec] = &[
    EngineSpec { id: "files", name: "Files", group: OnThisPc, target: Target::Files },
    web("google", "Google", WebSearch, "https://www.google.com/search?q={query}", Some(Engine::Google)),
    web("duckduckgo", "DuckDuckGo", WebSearch, "https://duckduckgo.com/?q={query}", Some(Engine::DuckDuckGo)),
    web("bing", "Bing", WebSearch, "https://www.bing.com/search?q={query}", None),
    web("brave", "Brave", WebSearch, "https://search.brave.com/search?q={query}", None),
    web("chatgpt", "ChatGPT", Ai, "https://chatgpt.com/?q={query}", None),
    web("claude", "Claude", Ai, "https://claude.ai/new?q={query}", None),
    web("perplexity", "Perplexity", Ai, "https://www.perplexity.ai/search?q={query}", None),
    web("youtube", "YouTube", VideoMusic, "https://www.youtube.com/results?search_query={query}", Some(Engine::YouTube)),
    web("spotify", "Spotify", VideoMusic, "https://open.spotify.com/search/{query}", None),
    web("twitch", "Twitch", VideoMusic, "https://www.twitch.tv/search?term={query}", None),
    web("wikipedia", "Wikipedia", Knowledge, "https://en.wikipedia.org/w/index.php?search={query}", Some(Engine::Wikipedia)),
    web("maps", "Maps", Knowledge, "https://www.google.com/maps/search/{query}", None),
    web("translate", "Translate", Knowledge, "https://translate.google.com/?sl=auto&op=translate&text={query}", None),
    web("github", "GitHub", Code, "https://github.com/search?q={query}", None),
    web("stackoverflow", "Stack Overflow", Code, "https://stackoverflow.com/search?q={query}", None),
    web("mdn", "MDN", Code, "https://developer.mozilla.org/search?q={query}", None),
    web("docsrs", "docs.rs", Code, "https://docs.rs/releases/search?query={query}", None),
    web("amazon", "Amazon", Shopping, "https://www.amazon.com/s?k={query}", None),
    web("ebay", "eBay", Shopping, "https://www.ebay.com/sch/i.html?_nkw={query}", None),
    web("reddit", "Reddit", Social, "https://www.reddit.com/search/?q={query}", None),
    web("x", "X", Social, "https://x.com/search?q={query}", None),
    web("linkedin", "LinkedIn", Social, "https://www.linkedin.com/search/results/all/?keywords={query}", None),
];

/// The built-in engine with this id.
pub fn find(id: &str) -> Option<&'static EngineSpec> {
    CATALOG.iter().find(|e| e.id == id)
}

/// `template` with every `{query}` replaced by `text`, percent-encoded so
/// it is safe in a query string and in a path alike.
pub fn fill(template: &str, text: &str) -> String {
    template.replace(QUERY, &encode(text))
}

/// Keeps `A-Z a-z 0-9 - . _ ~`; every other byte becomes `%XX`.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// An engine someone added: a name and an address with `{query}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomEngine {
    pub name: String,
    pub url: String,
}

impl CustomEngine {
    /// `custom:` and the name in lowercase, with a dash for each run of
    /// anything but letters and digits ("Thingi Verse!" is `custom:thingi-verse`).
    pub fn id(&self) -> String {
        let mut slug = String::new();
        for c in self.name.trim().chars().flat_map(char::to_lowercase) {
            if c.is_alphanumeric() {
                slug.push(c);
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        format!("{CUSTOM_PREFIX}{}", slug.trim_end_matches('-'))
    }
}

/// Why a custom engine was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomError {
    NameEmpty,
    NameTooLong,
    NotHttps,
    NoQuery,
    BadAddress,
    /// One of the person's own engines already has this name.
    NameTaken(String),
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CustomError::NameEmpty => write!(f, "Give it a name"),
            CustomError::NameTooLong => write!(f, "Keep the name to 20 characters"),
            CustomError::NotHttps => write!(f, "The address must start with https://"),
            CustomError::NoQuery => write!(f, "Put {QUERY} where the search text goes"),
            CustomError::BadAddress => write!(f, "That is not a web address"),
            CustomError::NameTaken(name) => write!(f, "You already have an engine called {name}"),
        }
    }
}

/// Checks a new custom engine against the rules and the person's other
/// engines, returning it trimmed.
pub fn check_custom(
    name: &str,
    url: &str,
    existing: &[CustomEngine],
) -> Result<CustomEngine, CustomError> {
    let (name, url) = (name.trim(), url.trim());
    if !name.chars().any(char::is_alphanumeric) {
        return Err(CustomError::NameEmpty);
    }
    if name.chars().count() > 20 {
        return Err(CustomError::NameTooLong);
    }
    if !url.starts_with("https://") {
        return Err(CustomError::NotHttps);
    }
    if !url.contains(QUERY) {
        return Err(CustomError::NoQuery);
    }
    match url::Url::parse(&fill(url, "test")) {
        Ok(parsed) if parsed.host_str().is_some_and(|h| h.contains('.')) => {}
        _ => return Err(CustomError::BadAddress),
    }
    let engine = CustomEngine {
        name: name.to_owned(),
        url: url.to_owned(),
    };
    if let Some(same) = existing.iter().find(|c| c.id() == engine.id()) {
        return Err(CustomError::NameTaken(same.name.clone()));
    }
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<_> = CATALOG.iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), CATALOG.len());
    }

    #[test]
    fn every_web_address_has_a_query_slot_and_https() {
        for engine in CATALOG {
            if let Target::Web { url, .. } = engine.target {
                assert!(url.starts_with("https://"), "{}", engine.id);
                assert!(url.contains(QUERY), "{}", engine.id);
            }
        }
    }

    #[test]
    fn catalog_has_the_agreed_engines_in_group_order() {
        assert_eq!(CATALOG.len(), 23);
        let groups: Vec<_> = CATALOG.iter().map(|e| e.group).collect();
        let mut sorted = groups.clone();
        sorted.sort_by_key(|g| GROUPS.iter().position(|x| x == g));
        assert_eq!(groups, sorted);
        assert_eq!(find("github").unwrap().name, "GitHub");
        assert_eq!(find("files").unwrap().target, Target::Files);
        assert!(find("nope").is_none());
    }

    #[test]
    fn filling_encodes_for_query_strings_and_paths() {
        assert_eq!(
            fill("https://www.google.com/search?q={query}", "rust & egui"),
            "https://www.google.com/search?q=rust%20%26%20egui"
        );
        assert_eq!(
            fill("https://open.spotify.com/search/{query}", "lo/fi?"),
            "https://open.spotify.com/search/lo%2Ffi%3F"
        );
        assert_eq!(
            fill("https://x/?q={query}", "blå-bær_~."),
            "https://x/?q=bl%C3%A5-b%C3%A6r_~."
        );
        assert_eq!(
            fill("https://x/?a={query}&b={query}", "1 2"),
            "https://x/?a=1%202&b=1%202"
        );
    }

    fn mine() -> Vec<CustomEngine> {
        vec![CustomEngine {
            name: "Thingiverse".into(),
            url: "https://www.thingiverse.com/search?q={query}".into(),
        }]
    }

    #[test]
    fn a_good_custom_engine_is_cleaned_up() {
        let c = check_custom(" Crates ", " https://crates.io/search?q={query} ", &mine()).unwrap();
        assert_eq!(c.name, "Crates");
        assert_eq!(c.url, "https://crates.io/search?q={query}");
        assert_eq!(c.id(), "custom:crates");
    }

    #[test]
    fn custom_ids_come_from_the_name() {
        let id = |name: &str| {
            CustomEngine {
                name: name.into(),
                url: String::new(),
            }
            .id()
        };
        assert_eq!(id("Thingi Verse!"), "custom:thingi-verse");
        assert_eq!(id("  Blå  bær "), "custom:blå-bær");
        assert_eq!(id("A.B"), "custom:a-b");
    }

    #[test]
    fn custom_engine_mistakes_are_named() {
        let ok_url = "https://a.com/?q={query}";
        assert_eq!(check_custom("", ok_url, &[]), Err(CustomError::NameEmpty));
        assert_eq!(
            check_custom(" !? ", ok_url, &[]),
            Err(CustomError::NameEmpty)
        );
        assert_eq!(
            check_custom(&"x".repeat(21), ok_url, &[]),
            Err(CustomError::NameTooLong)
        );
        assert_eq!(
            check_custom("A", "http://a.com/?q={query}", &[]),
            Err(CustomError::NotHttps)
        );
        assert_eq!(
            check_custom("A", "https://a.com/?q=", &[]),
            Err(CustomError::NoQuery)
        );
        assert_eq!(
            check_custom("A", "https://{query}", &[]),
            Err(CustomError::BadAddress)
        );
        assert_eq!(
            check_custom("thingiverse", ok_url, &mine()),
            Err(CustomError::NameTaken("Thingiverse".into()))
        );
    }

    #[test]
    fn mistakes_read_as_plain_words() {
        assert_eq!(
            CustomError::NameTaken("Crates".into()).to_string(),
            "You already have an engine called Crates"
        );
        assert_eq!(
            CustomError::NoQuery.to_string(),
            "Put {query} where the search text goes"
        );
    }
}
