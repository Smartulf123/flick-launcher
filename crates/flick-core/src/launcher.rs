//! The search bar's state: input, active engine, results and selection.
//! The app turns key presses into `Intent`s and carries out the returned `Effect`s.

use crate::mode::{Action, ResultItem};
use crate::registry::{ModeRegistry, Modes};

/// A key press the search bar cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Up,
    Down,
    Enter,
    Tab,
    ShiftTab,
    Escape,
    /// Backspace while the search box is empty.
    BackspaceOnEmpty,
}

/// Something the app must do in response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Run(Action),
    Hide,
}

pub struct Launcher {
    registry: ModeRegistry,
    /// Registry indices of the bar's engines, in Tab order.
    chips: Vec<usize>,
    shortcuts: Vec<(String, usize)>,
    default_mode: usize,
    active: usize,
    max_results: usize,
    input: String,
    results: Vec<ResultItem>,
    selected: usize,
    /// The engine and shortcut text from before the last shortcut, so
    /// Backspace in the empty box can go back.
    before_shortcut: Option<(usize, String)>,
}

impl Launcher {
    pub fn new(modes: Modes, max_results: usize) -> Self {
        Self {
            registry: modes.registry,
            chips: modes.chips,
            shortcuts: modes.shortcuts,
            default_mode: modes.default,
            active: modes.default,
            max_results,
            input: String::new(),
            results: Vec::new(),
            selected: 0,
            before_shortcut: None,
        }
    }

    /// Stores new input, re-runs the search and selects the first result.
    /// A shortcut and a space at the start ("gh rust") switch to that
    /// engine and are dropped from the text.
    pub fn set_input(&mut self, text: &str) {
        let text = match self.shortcut_in(text) {
            Some((target, word, rest)) => {
                self.before_shortcut = Some((self.active, word.to_owned()));
                self.active = target;
                rest
            }
            None => text,
        };
        self.input = text.to_owned();
        self.selected = 0;
        self.run_query();
    }

    /// The engine a leading "shortcut " names, the shortcut, and the rest.
    fn shortcut_in<'t>(&self, text: &'t str) -> Option<(usize, &'t str, &'t str)> {
        let (word, rest) = text.split_once(' ')?;
        let target = self
            .shortcuts
            .iter()
            .find(|(s, _)| !word.is_empty() && s.eq_ignore_ascii_case(word))?
            .1;
        Some((target, word, rest))
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn results(&self) -> &[ResultItem] {
        &self.results
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Selects result `index`, clamped to the last result (for mouse hover and click).
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.results.len().saturating_sub(1));
    }

    /// Name of the engine that answers the input.
    pub fn active_mode_name(&self) -> &str {
        self.registry.mode(self.active).name()
    }

    pub fn handle(&mut self, intent: Intent) -> Vec<Effect> {
        match intent {
            Intent::Up => self.selected = self.selected.saturating_sub(1),
            Intent::Down => {
                self.selected = (self.selected + 1).min(self.results.len().saturating_sub(1));
            }
            Intent::Tab => {
                let next = self.active_chip().map_or(0, |c| (c + 1) % self.chips.len());
                self.select_chip(next);
            }
            Intent::ShiftTab => {
                let n = self.chips.len();
                let prev = self.active_chip().map_or(n - 1, |c| (c + n - 1) % n);
                self.select_chip(prev);
            }
            Intent::BackspaceOnEmpty => {
                if self.input.is_empty()
                    && let Some((engine, word)) = self.before_shortcut.take()
                {
                    self.active = engine;
                    self.input = word;
                    self.selected = 0;
                    self.run_query();
                }
            }
            Intent::Enter => {
                let action = match self.results.get(self.selected) {
                    Some(item) if item.action != Action::None => item.action.clone(),
                    _ => return Vec::new(),
                };
                self.reset();
                return vec![Effect::Run(action), Effect::Hide];
            }
            Intent::Escape => {
                self.reset();
                return vec![Effect::Hide];
            }
        }
        Vec::new()
    }

    /// Re-runs the current search, keeping the selection in range
    /// (for example after the file index finishes building).
    pub fn refresh(&mut self) {
        self.run_query();
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
    }

    /// Back to an empty bar in the default engine.
    pub fn reset(&mut self) {
        self.input.clear();
        self.results.clear();
        self.selected = 0;
        self.active = self.default_mode;
        self.before_shortcut = None;
    }

    /// The bar's engine names, in Tab order.
    pub fn chips(&self) -> Vec<&str> {
        self.chips
            .iter()
            .map(|&i| self.registry.mode(i).name())
            .collect()
    }

    /// Position (in `chips()`) of the engine that answers the input; `None`
    /// while an engine reached by shortcut is in use.
    pub fn active_chip(&self) -> Option<usize> {
        self.chips.iter().position(|&i| i == self.active)
    }

    /// The engine in use when it is not on the bar (reached by shortcut).
    pub fn visiting(&self) -> Option<&str> {
        match self.active_chip() {
            Some(_) => None,
            None => Some(self.active_mode_name()),
        }
    }

    /// Makes chip `index` the active engine and searches the same text there.
    /// Panics if `index` is out of range.
    pub fn select_chip(&mut self, index: usize) {
        self.active = self.chips[index];
        self.before_shortcut = None;
        self.selected = 0;
        self.run_query();
    }

    fn run_query(&mut self) {
        self.results = self
            .registry
            .mode(self.active)
            .query(&self.input, self.max_results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{FileEntry, FileIndex, SharedIndex};
    use crate::mode::Action;
    use crate::modes::{FilesMode, WebSearchMode};
    use crate::registry::{ModeRegistry, Modes};
    use std::path::PathBuf;

    const YOUTUBE: usize = 1;
    const FILES: usize = 2;

    fn launcher() -> Launcher {
        let entries = ["report.txt", "resume.pdf", "notes.md"]
            .iter()
            .map(|n| FileEntry {
                path: PathBuf::from(format!(r"C:\test\{n}")),
                name: (*n).to_owned(),
                is_dir: false,
            })
            .collect();
        let index = SharedIndex::default();
        *index.write().unwrap() = Some(FileIndex { entries });

        // Google, YouTube and Files on the bar; GitHub only by shortcut.
        let mut r = ModeRegistry::new();
        r.register(Box::new(WebSearchMode::google()));
        r.register(Box::new(WebSearchMode::youtube()));
        r.register(Box::new(FilesMode::new(index)));
        r.register(Box::new(WebSearchMode::builtin("github")));
        let shortcuts = [("gg", 0), ("yt", 1), ("fi", 2), ("gh", 3)]
            .map(|(k, i)| (k.to_owned(), i))
            .to_vec();
        let modes = Modes {
            registry: r,
            chips: vec![0, 1, 2],
            shortcuts,
            default: 0,
        };
        Launcher::new(modes, 8)
    }

    #[test]
    fn a_shortcut_and_space_switch_engines_and_drop_the_shortcut() {
        let mut l = launcher();
        l.set_input("yt lofi");
        assert_eq!((l.active_mode_name(), l.input()), ("YouTube", "lofi"));
        assert_eq!(l.active_chip(), Some(YOUTUBE));
        assert_eq!(l.results()[0].title, "Search YouTube for \"lofi\"");
    }

    #[test]
    fn shortcuts_reach_engines_that_are_not_on_the_bar() {
        let mut l = launcher();
        l.set_input("GH egui");
        assert_eq!((l.active_mode_name(), l.input()), ("GitHub", "egui"));
        assert_eq!(l.active_chip(), None);
        assert_eq!(l.visiting(), Some("GitHub"));
    }

    #[test]
    fn only_a_whole_first_word_followed_by_a_space_is_a_shortcut() {
        let mut l = launcher();
        for text in ["gh", "ghost town", "go gh now", " gh x"] {
            l.set_input(text);
            assert_eq!((l.active_mode_name(), l.input()), ("Google", text));
        }
    }

    #[test]
    fn backspace_in_the_empty_box_goes_back_with_the_shortcut_typed() {
        let mut l = launcher();
        l.set_input("gh ");
        assert_eq!((l.active_mode_name(), l.input()), ("GitHub", ""));
        l.handle(Intent::BackspaceOnEmpty);
        assert_eq!((l.active_mode_name(), l.input()), ("Google", "gh"));
        // Nothing to go back to now.
        l.set_input("");
        l.handle(Intent::BackspaceOnEmpty);
        assert_eq!(l.active_mode_name(), "Google");
    }

    #[test]
    fn tab_from_a_shortcut_engine_returns_to_the_bar() {
        let mut l = launcher();
        l.set_input("gh egui");
        l.handle(Intent::Tab);
        assert_eq!((l.active_mode_name(), l.input()), ("Google", "egui"));
        l.set_input("gh egui");
        l.handle(Intent::ShiftTab);
        assert_eq!(l.active_mode_name(), "Files");
        assert_eq!(l.visiting(), None);
    }

    #[test]
    fn hiding_after_a_shortcut_returns_to_the_default_engine() {
        let mut l = launcher();
        l.set_input("gh egui");
        assert_eq!(l.handle(Intent::Escape), vec![Effect::Hide]);
        assert_eq!(l.active_mode_name(), "Google");
        l.handle(Intent::BackspaceOnEmpty);
        assert_eq!(l.active_mode_name(), "Google");
    }

    #[test]
    fn enter_runs_selected_then_hides_and_resets() {
        let mut l = launcher();
        l.select_chip(YOUTUBE);
        l.set_input("lofi");
        assert_eq!(
            l.handle(Intent::Enter),
            vec![
                Effect::Run(Action::OpenUrl(
                    "https://www.youtube.com/results?search_query=lofi".into()
                )),
                Effect::Hide
            ]
        );
        assert_eq!((l.input(), l.active_mode_name()), ("", "Google"));
    }

    #[test]
    fn enter_with_nothing_runnable_does_nothing() {
        let mut l = launcher();
        assert!(l.handle(Intent::Enter).is_empty());
        l.select_chip(FILES);
        l.set_input("zzzz");
        assert!(l.handle(Intent::Enter).is_empty());
    }

    #[test]
    fn tab_cycles_and_keeps_the_text() {
        let mut l = launcher();
        l.set_input("lofi");
        l.handle(Intent::Tab);
        assert_eq!((l.input(), l.active_mode_name()), ("lofi", "YouTube"));
        l.handle(Intent::Tab);
        assert_eq!(l.active_mode_name(), "Files");
        l.handle(Intent::ShiftTab);
        assert_eq!(l.active_mode_name(), "YouTube");
    }

    #[test]
    fn arrows_move_and_clamp() {
        let mut l = launcher();
        l.select_chip(FILES);
        l.set_input("e");
        assert_eq!(l.results().len(), 3);
        l.handle(Intent::Up);
        assert_eq!(l.selected(), 0);
        for _ in 0..5 {
            l.handle(Intent::Down);
        }
        assert_eq!(l.selected(), 2);
        l.set_input("es");
        assert_eq!(l.selected(), 0);
    }

    #[test]
    fn select_picks_a_row_and_clamps() {
        let mut l = launcher();
        l.select_chip(FILES);
        l.set_input("e");
        l.select(1);
        assert_eq!(l.selected(), 1);
        l.select(99);
        assert_eq!(l.selected(), 2);
    }

    #[test]
    fn lists_the_bar_engines_in_tab_order() {
        assert_eq!(launcher().chips(), vec!["Google", "YouTube", "Files"]);
    }

    #[test]
    fn select_mode_switches_and_searches_again() {
        let mut l = launcher();
        l.set_input("notes");
        l.select_chip(FILES);
        assert_eq!(l.active_chip(), Some(FILES));
        assert_eq!((l.input(), l.active_mode_name()), ("notes", "Files"));
        assert_eq!(l.results()[0].title, "notes.md");
    }

    #[test]
    fn escape_hides_and_resets() {
        let mut l = launcher();
        l.select_chip(FILES);
        l.set_input("rust");
        assert_eq!(l.handle(Intent::Escape), vec![Effect::Hide]);
        assert_eq!((l.input(), l.active_chip()), ("", Some(0)));
    }
}
