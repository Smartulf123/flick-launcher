//! Apps: fuzzy search over the installed programs by name.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

use crate::apps::{AppEntry, SharedApps};
use crate::mode::{Action, ResultItem, ResultKind, SearchMode};

/// Title of the single row shown before the first app scan finishes.
pub const FINDING_APPS: &str = "Finding apps...";

pub struct AppsMode {
    apps: SharedApps,
}

impl AppsMode {
    pub fn new(apps: SharedApps) -> Self {
        Self { apps }
    }
}

/// Lets nucleo match on the app name while keeping the whole entry.
struct ByName<'a>(&'a AppEntry);

impl AsRef<str> for ByName<'_> {
    fn as_ref(&self) -> &str {
        &self.0.name
    }
}

impl SearchMode for AppsMode {
    fn id(&self) -> &str {
        "apps"
    }

    fn name(&self) -> &str {
        "Apps"
    }

    fn query(&self, text: &str, max_results: usize) -> Vec<ResultItem> {
        let text = text.trim();
        if text.is_empty() {
            return Vec::new();
        }
        let guard = self.apps.read().unwrap_or_else(|p| p.into_inner());
        let Some(apps) = guard.as_ref() else {
            return vec![ResultItem {
                title: FINDING_APPS.into(),
                subtitle: String::new(),
                score: 0,
                action: Action::None,
                kind: ResultKind::Info,
            }];
        };

        let pattern = Pattern::parse(text, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut matches = pattern.match_list(apps.iter().map(ByName), &mut matcher);
        matches.sort_by(|(a, sa), (b, sb)| sb.cmp(sa).then(a.0.name.len().cmp(&b.0.name.len())));

        matches
            .into_iter()
            .take(max_results)
            .map(|(ByName(app), score)| ResultItem {
                title: app.name.clone(),
                subtitle: String::new(),
                score,
                action: Action::Launch(app.launch.clone()),
                kind: ResultKind::App,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(names: Option<&[&str]>) -> AppsMode {
        let list = names.map(|names| {
            names
                .iter()
                .map(|n| AppEntry {
                    name: (*n).to_owned(),
                    launch: format!("id:{n}"),
                })
                .collect()
        });
        AppsMode::new(std::sync::Arc::new(std::sync::RwLock::new(list)))
    }

    #[test]
    fn apps_mode_ranks_by_name_and_launches() {
        let m = mode(Some(&["Speedtest", "Paint", "Spotify"]));
        let r = m.query("spot", 8);
        assert_eq!(r[0].title, "Spotify");
        assert_eq!(r[0].subtitle, "");
        assert_eq!(r[0].kind, ResultKind::App);
        assert_eq!(r[0].action, Action::Launch("id:Spotify".into()));
        assert!(r.iter().all(|i| i.title != "Paint"));
        assert_eq!(m.query("p", 1).len(), 1);
    }

    #[test]
    fn a_tie_goes_to_the_shorter_name() {
        let m = mode(Some(&["Notepad++", "Notepad"]));
        assert_eq!(m.query("notepad", 8)[0].title, "Notepad");
    }

    #[test]
    fn apps_mode_waits_for_the_first_scan() {
        let r = mode(None).query("spot", 8);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, FINDING_APPS);
        assert_eq!(
            (r[0].action.clone(), r[0].kind),
            (Action::None, ResultKind::Info)
        );
    }

    #[test]
    fn an_empty_box_shows_nothing() {
        assert!(mode(Some(&["Paint"])).query("  ", 8).is_empty());
        assert!(mode(None).query("", 8).is_empty());
    }
}
