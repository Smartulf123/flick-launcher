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

    /// Name of the mode that answers the input.
    pub fn active_mode_name(&self) -> &str {
        self.registry.mode(self.active).name()
    }

    pub fn handle(&mut self, intent: Intent) -> Vec<Effect> {
        match intent {
            Intent::Up => self.selected = self.selected.saturating_sub(1),
            Intent::Down => {
                self.selected = (self.selected + 1).min(self.results.len().saturating_sub(1));
            }
            Intent::Tab => self.select_mode(self.registry.next(self.active)),
            Intent::ShiftTab => self.select_mode(self.registry.prev(self.active)),
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

    /// Every mode's name, in Tab order.
    pub fn modes(&self) -> Vec<&str> {
        (0..self.registry.len())
            .map(|i| self.registry.mode(i).name())
            .collect()
    }

    /// Index (into `modes()`) of the mode that answers the input.
    pub fn active_mode(&self) -> usize {
        self.active
    }

    /// Makes `index` the active mode and searches the same text there.
    /// Panics if `index` is out of range.
    pub fn select_mode(&mut self, index: usize) {
        assert!(index < self.registry.len(), "no mode {index}");
        self.active = index;
        let text = std::mem::take(&mut self.input);
        self.set_input(&text);
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
    use crate::registry::ModeRegistry;
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

        let mut r = ModeRegistry::new();
        r.register(Box::new(WebSearchMode::google()));
        r.register(Box::new(WebSearchMode::youtube()));
        r.register(Box::new(FilesMode::new(index)));
        Launcher::new(r, 0, 8)
    }

    #[test]
    fn typing_a_mode_name_does_not_switch_modes() {
        let mut l = launcher();
        l.set_input("yt lofi");
        assert_eq!(l.active_mode_name(), "Google");
        assert_eq!(l.results()[0].title, "Search Google for \"yt lofi\"");
    }

    #[test]
    fn enter_runs_selected_then_hides_and_resets() {
        let mut l = launcher();
        l.select_mode(YOUTUBE);
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
        l.select_mode(FILES);
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
        l.select_mode(FILES);
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
        l.select_mode(FILES);
        l.set_input("e");
        l.select(1);
        assert_eq!(l.selected(), 1);
        l.select(99);
        assert_eq!(l.selected(), 2);
    }

    #[test]
    fn lists_mode_names_in_tab_order() {
        assert_eq!(launcher().modes(), vec!["Google", "YouTube", "Files"]);
    }

    #[test]
    fn select_mode_switches_and_searches_again() {
        let mut l = launcher();
        l.set_input("notes");
        l.select_mode(FILES);
        assert_eq!(l.active_mode(), FILES);
        assert_eq!((l.input(), l.active_mode_name()), ("notes", "Files"));
        assert_eq!(l.results()[0].title, "notes.md");
    }

    #[test]
    fn escape_hides_and_resets() {
        let mut l = launcher();
        l.select_mode(FILES);
        l.set_input("rust");
        assert_eq!(l.handle(Intent::Escape), vec![Effect::Hide]);
        assert_eq!((l.input(), l.active_mode()), ("", 0));
    }
}
