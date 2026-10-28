//! Every engine Flick knows: the built-in catalog, and engines people add
//! themselves.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::modes::suggest::Engine;

/// Where the search text goes in an engine's address.
pub const QUERY: &str = "{query}";

/// A custom engine's id is this followed by its shortcut.
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
    /// Typed before a space to switch to this engine, e.g. `"gh"`.
    pub shortcut: &'static str,
    pub target: Target,
}

const fn web(
    id: &'static str,
    name: &'static str,
    group: Group,
    shortcut: &'static str,
    url: &'static str,
    suggest: Option<Engine>,
) -> EngineSpec {
    EngineSpec {
        id,
        name,
        group,
        shortcut,
        target: Target::Web { url, suggest },
    }
}

use Group::*;

/// Every built-in engine, grouped in list order. Shortcuts are 2-3 letters
/// and never a common word, so ordinary searches do not switch engines.
#[rustfmt::skip]
pub const CATALOG: &[EngineSpec] = &[
    EngineSpec { id: "files", name: "Files", group: OnThisPc, shortcut: "fi", target: Target::Files },
    web("google", "Google", WebSearch, "gg", "https://www.google.com/search?q={query}", Some(Engine::Google)),
    web("duckduckgo", "DuckDuckGo", WebSearch, "ddg", "https://duckduckgo.com/?q={query}", Some(Engine::DuckDuckGo)),
    web("bing", "Bing", WebSearch, "bi", "https://www.bing.com/search?q={query}", None),
    web("brave", "Brave", WebSearch, "brv", "https://search.brave.com/search?q={query}", None),
    web("chatgpt", "ChatGPT", Ai, "gpt", "https://chatgpt.com/?q={query}", None),
    web("claude", "Claude", Ai, "cl", "https://claude.ai/new?q={query}", None),
    web("perplexity", "Perplexity", Ai, "pp", "https://www.perplexity.ai/search?q={query}", None),
    web("youtube", "YouTube", VideoMusic, "yt", "https://www.youtube.com/results?search_query={query}", Some(Engine::YouTube)),
    web("spotify", "Spotify", VideoMusic, "sp", "https://open.spotify.com/search/{query}", None),
    web("twitch", "Twitch", VideoMusic, "tw", "https://www.twitch.tv/search?term={query}", None),
    web("wikipedia", "Wikipedia", Knowledge, "wp", "https://en.wikipedia.org/w/index.php?search={query}", Some(Engine::Wikipedia)),
    web("maps", "Maps", Knowledge, "mp", "https://www.google.com/maps/search/{query}", None),
    web("translate", "Translate", Knowledge, "tr", "https://translate.google.com/?sl=auto&op=translate&text={query}", None),
    web("github", "GitHub", Code, "gh", "https://github.com/search?q={query}", None),
    web("stackoverflow", "Stack Overflow", Code, "st", "https://stackoverflow.com/search?q={query}", None),
    web("mdn", "MDN", Code, "mdn", "https://developer.mozilla.org/search?q={query}", None),
    web("docsrs", "docs.rs", Code, "rs", "https://docs.rs/releases/search?query={query}", None),
    web("amazon", "Amazon", Shopping, "amz", "https://www.amazon.com/s?k={query}", None),
    web("ebay", "eBay", Shopping, "eb", "https://www.ebay.com/sch/i.html?_nkw={query}", None),
    web("reddit", "Reddit", Social, "rd", "https://www.reddit.com/search/?q={query}", None),
    web("x", "X", Social, "xx", "https://x.com/search?q={query}", None),
    web("linkedin", "LinkedIn", Social, "li", "https://www.linkedin.com/search/results/all/?keywords={query}", None),
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

/// An engine someone added: a name, an address with `{query}`, a shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomEngine {
    pub name: String,
    pub url: String,
    pub shortcut: String,
}

impl CustomEngine {
    pub fn id(&self) -> String {
        format!("{CUSTOM_PREFIX}{}", self.shortcut)
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
    ShortcutShape,
    /// The name of the engine that already has the shortcut.
    ShortcutTaken(String),
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CustomError::NameEmpty => write!(f, "Give it a name"),
            CustomError::NameTooLong => write!(f, "Keep the name to 20 characters"),
            CustomError::NotHttps => write!(f, "The address must start with https://"),
            CustomError::NoQuery => write!(f, "Put {QUERY} where the search text goes"),
            CustomError::BadAddress => write!(f, "That is not a web address"),
            CustomError::ShortcutShape => write!(f, "Shortcut: 2-5 letters or digits, no spaces"),
            CustomError::ShortcutTaken(name) => write!(f, "{name} already uses that shortcut"),
        }
    }
}

/// Checks a new custom engine against the rules and the shortcuts in use,
/// returning it trimmed, with a lowercase shortcut.
pub fn check_custom(
    name: &str,
    url: &str,
    shortcut: &str,
    existing: &[CustomEngine],
) -> Result<CustomEngine, CustomError> {
    let (name, url, shortcut) = (name.trim(), url.trim(), shortcut.trim().to_lowercase());
    if name.is_empty() {
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
    let shape = (2..=5).contains(&shortcut.len())
        && shortcut
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    if !shape {
        return Err(CustomError::ShortcutShape);
    }
    let taken = CATALOG
        .iter()
        .find(|e| e.shortcut == shortcut)
        .map(|e| e.name.to_owned())
        .or_else(|| {
            existing
                .iter()
                .find(|c| c.shortcut == shortcut)
                .map(|c| c.name.clone())
        });
    if let Some(owner) = taken {
        return Err(CustomError::ShortcutTaken(owner));
    }
    Ok(CustomEngine {
        name: name.to_owned(),
        url: url.to_owned(),
        shortcut,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_and_shortcuts_are_unique() {
        let ids: HashSet<_> = CATALOG.iter().map(|e| e.id).collect();
        let shortcuts: HashSet<_> = CATALOG.iter().map(|e| e.shortcut).collect();
        assert_eq!(ids.len(), CATALOG.len());
        assert_eq!(shortcuts.len(), CATALOG.len());
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
    fn shortcuts_are_short_and_not_common_words() {
        for engine in CATALOG {
            let n = engine.shortcut.len();
            assert!((2..=3).contains(&n), "{}", engine.shortcut);
            assert!(
                !["so", "am", "be", "go", "is", "it", "on", "to", "an"].contains(&engine.shortcut)
            );
        }
    }

    #[test]
    fn catalog_has_the_agreed_engines_in_group_order() {
        assert_eq!(CATALOG.len(), 23);
        let groups: Vec<_> = CATALOG.iter().map(|e| e.group).collect();
        let mut sorted = groups.clone();
        sorted.sort_by_key(|g| GROUPS.iter().position(|x| x == g));
        assert_eq!(groups, sorted);
        assert_eq!(find("github").unwrap().shortcut, "gh");
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
            shortcut: "tv".into(),
        }]
    }

    #[test]
    fn a_good_custom_engine_is_cleaned_up() {
        let c = check_custom(
            " Crates ",
            " https://crates.io/search?q={query} ",
            "CR",
            &mine(),
        )
        .unwrap();
        assert_eq!(c.name, "Crates");
        assert_eq!(c.url, "https://crates.io/search?q={query}");
        assert_eq!(c.shortcut, "cr");
        assert_eq!(c.id(), "custom:cr");
    }

    #[test]
    fn custom_engine_mistakes_are_named() {
        let ok_url = "https://a.com/?q={query}";
        assert_eq!(
            check_custom("", ok_url, "ab", &[]),
            Err(CustomError::NameEmpty)
        );
        assert_eq!(
            check_custom(&"x".repeat(21), ok_url, "ab", &[]),
            Err(CustomError::NameTooLong)
        );
        assert_eq!(
            check_custom("A", "http://a.com/?q={query}", "ab", &[]),
            Err(CustomError::NotHttps)
        );
        assert_eq!(
            check_custom("A", "https://a.com/?q=", "ab", &[]),
            Err(CustomError::NoQuery)
        );
        assert_eq!(
            check_custom("A", "https://{query}", "ab", &[]),
            Err(CustomError::BadAddress)
        );
        assert_eq!(
            check_custom("A", ok_url, "a", &[]),
            Err(CustomError::ShortcutShape)
        );
        assert_eq!(
            check_custom("A", ok_url, "a b", &[]),
            Err(CustomError::ShortcutShape)
        );
        assert_eq!(
            check_custom("A", ok_url, "abcdef", &[]),
            Err(CustomError::ShortcutShape)
        );
        assert_eq!(
            check_custom("A", ok_url, "gh", &[]),
            Err(CustomError::ShortcutTaken("GitHub".into()))
        );
        assert_eq!(
            check_custom("A", ok_url, "tv", &mine()),
            Err(CustomError::ShortcutTaken("Thingiverse".into()))
        );
    }

    #[test]
    fn mistakes_read_as_plain_words() {
        assert_eq!(
            CustomError::ShortcutTaken("GitHub".into()).to_string(),
            "GitHub already uses that shortcut"
        );
        assert_eq!(
            CustomError::NoQuery.to_string(),
            "Put {query} where the search text goes"
        );
    }
}
