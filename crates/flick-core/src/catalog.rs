//! Every engine Flick knows: the built-in catalog.

use crate::modes::suggest::Engine;

/// Where the search text goes in an engine's address.
pub const QUERY: &str = "{query}";

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
    /// The installed programs.
    Apps,
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
    EngineSpec { id: "apps", name: "Apps", group: OnThisPc, target: Target::Apps },
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

/// The site whose logo stands for an engine or result with this address:
/// its host, except Google Maps, which lives under Google's own host.
pub fn icon_domain(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    if host == "www.google.com" && parsed.path().starts_with("/maps") {
        return Some("maps.google.com".into());
    }
    Some(host)
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
        assert!(find("stackoverflow").is_none());
        let groups: Vec<_> = CATALOG.iter().map(|e| e.group).collect();
        let mut sorted = groups.clone();
        sorted.sort_by_key(|g| GROUPS.iter().position(|x| x == g));
        assert_eq!(groups, sorted);
        assert_eq!(find("github").unwrap().name, "GitHub");
        assert_eq!(find("files").unwrap().target, Target::Files);
        assert_eq!(find("apps").unwrap().target, Target::Apps);
        assert!(find("nope").is_none());
    }

    #[test]
    fn logos_come_from_the_site_and_maps_has_its_own() {
        assert_eq!(
            icon_domain("https://www.google.com/search?q={query}").as_deref(),
            Some("www.google.com")
        );
        assert_eq!(
            icon_domain("https://www.google.com/maps/search/{query}").as_deref(),
            Some("maps.google.com")
        );
        assert_eq!(
            icon_domain("https://claude.ai/new?q=rust%20egui").as_deref(),
            Some("claude.ai")
        );
        assert_eq!(
            icon_domain("https://WWW.Amazon.com/s?k=x").as_deref(),
            Some("www.amazon.com")
        );
        assert_eq!(icon_domain("not an address"), None);
        assert_eq!(icon_domain("file:///C:/x.txt"), None);
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
}
