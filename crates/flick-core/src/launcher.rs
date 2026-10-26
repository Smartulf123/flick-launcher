//! The search bar's state: input, active mode, results and selection.
//! The app turns key presses into `Intent`s and carries out the returned `Effect`s.

use crate::mode::{Action, ResultItem};
use crate::registry::ModeRegistry;

/// A key press the search bar cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Up,
    Down,
    Enter,
    Tab,
    ShiftTab,
    Escape,
}

/// Something the app must do in response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Run(Action),
    Hide,
}

pub struct Launcher {
    registry: ModeRegistry,
    default_mode: usize,
    active: usize,
    max_results: usize,
    input: String,
    results: Vec<ResultItem>,
    selected: usize,
}

impl Launcher {
    pub fn new(registry: ModeRegistry, default_mode: usize, max_results: usize) -> Self {
        Self {
            registry,
            default_mode,
            active: default_mode,
            max_results,
            input: String::new(),
            results: Vec::new(),
            selected: 0,
        }
    }

    /// Stores new input, re-runs the search and selects the first result.
    pub fn set_input(&mut self, text: &str) {
        self.input = text.to_owned();
        self.selected = 0;
        self.run_query();
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

    /// The mode that answers the current input (a prefix overrides the active mode).
    pub fn effective_mode_name(&self) -> &str {
        self.registry.mode(self.effective_mode()).name()
    }

    pub fn handle(&mut self, intent: Intent) -> Vec<Effect> {
        match intent {
            Intent::Up => self.selected = self.selected.saturating_sub(1),
            Intent::Down => {
                self.selected = (self.selected + 1).min(self.results.len().saturating_sub(1));
            }
            Intent::Tab => self.select_mode(self.registry.next(self.effective_mode())),
            Intent::ShiftTab => self.select_mode(self.registry.prev(self.effective_mode())),
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

    /// Back to an empty bar in the default mode.
    pub fn reset(&mut self) {
        self.input.clear();
        self.results.clear();
        self.selected = 0;
        self.active = self.default_mode;
    }

    /// Every mode's name and keyword, in Tab order.
    pub fn modes(&self) -> Vec<(&str, &str)> {
        (0..self.registry.len())
            .map(|i| (self.registry.mode(i).name(), self.registry.keyword(i)))
            .collect()
    }

    /// Index (into `modes()`) of the mode that answers the current input.
    pub fn effective_mode(&self) -> usize {
        self.registry.parse(&self.input, self.active).mode
    }

    /// Makes `index` the active mode, dropping any typed prefix but keeping
    /// the search text. Panics if `index` is out of range.
    pub fn select_mode(&mut self, index: usize) {
        assert!(index < self.registry.len(), "no mode {index}");
        let text = self
            .registry
            .parse(&self.input, self.active)
            .text
            .to_owned();
        self.active = index;
        self.set_input(&text);
    }

    fn run_query(&mut self) {
        let parsed = self.registry.parse(&self.input, self.active);
        self.results = self
            .registry
            .mode(parsed.mode)
            .query(parsed.text, self.max_results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{FileEntry, FileIndex, SharedIndex};
    use crate::mode::Action;
    use crate::modes::{FilesMode, WebSearchMode};
    use crate::registry::ModeRegistry;
    use std::path::PathBuf;

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

        let mut r = ModeRegistry::new();
        r.register("g", Box::new(WebSearchMode::google())).unwrap();
        r.register("yt", Box::new(WebSearchMode::youtube()))
            .unwrap();
        r.register("f", Box::new(FilesMode::new(index))).unwrap();
        Launcher::new(r, 0, 8)
    }

    #[test]
    fn prefix_shows_effective_mode() {
        let mut l = launcher();
        l.set_input("yt lofi");
        assert_eq!(l.effective_mode_name(), "YouTube");
    }

    #[test]
    fn enter_runs_selected_then_hides_and_resets() {
        let mut l = launcher();
        l.set_input("yt lofi");
        assert_eq!(
            l.handle(Intent::Enter),
            vec![
                Effect::Run(Action::OpenUrl(
                    "https://www.youtube.com/results?search_query=lofi".into()
                )),
                Effect::Hide
            ]
        );
        assert_eq!((l.input(), l.effective_mode_name()), ("", "Google"));
    }

    #[test]
    fn enter_with_nothing_runnable_does_nothing() {
        let mut l = launcher();
        assert!(l.handle(Intent::Enter).is_empty());
        l.set_input("f zzzz");
        assert!(l.handle(Intent::Enter).is_empty());
    }

    #[test]
    fn tab_cycles_and_strips_prefix() {
        let mut l = launcher();
        l.set_input("yt lofi");
        l.handle(Intent::Tab);
        assert_eq!((l.input(), l.effective_mode_name()), ("lofi", "Files"));
        l.handle(Intent::ShiftTab);
        assert_eq!(l.effective_mode_name(), "YouTube");
    }

    #[test]
    fn arrows_move_and_clamp() {
        let mut l = launcher();
        l.set_input("f e");
        assert_eq!(l.results().len(), 3);
        l.handle(Intent::Up);
        assert_eq!(l.selected(), 0);
        for _ in 0..5 {
            l.handle(Intent::Down);
        }
        assert_eq!(l.selected(), 2);
        l.set_input("f es");
        assert_eq!(l.selected(), 0);
    }

    #[test]
    fn select_picks_a_row_and_clamps() {
        let mut l = launcher();
        l.set_input("f e");
        l.select(1);
        assert_eq!(l.selected(), 1);
        l.select(99);
        assert_eq!(l.selected(), 2);
    }

    #[test]
    fn lists_modes_with_keywords_in_tab_order() {
        assert_eq!(
            launcher().modes(),
            vec![("Google", "g"), ("YouTube", "yt"), ("Files", "f")]
        );
    }

    #[test]
    fn effective_mode_follows_prefix() {
        let mut l = launcher();
        assert_eq!(l.effective_mode(), 0);
        l.set_input("f notes");
        assert_eq!(l.effective_mode(), 2);
    }

    #[test]
    fn select_mode_switches_and_strips_prefix() {
        let mut l = launcher();
        l.set_input("yt notes");
        l.select_mode(2);
        assert_eq!((l.input(), l.effective_mode_name()), ("notes", "Files"));
        assert_eq!(l.results()[0].title, "notes.md");
    }

    #[test]
    fn escape_hides_and_resets() {
        let mut l = launcher();
        l.set_input("g rust");
        assert_eq!(l.handle(Intent::Escape), vec![Effect::Hide]);
        assert_eq!(l.input(), "");
    }
}
