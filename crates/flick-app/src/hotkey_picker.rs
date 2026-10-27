//! The "Change hotkey" view's state: which combination was pressed, and
//! what Enter and Esc do.

use eframe::egui::{Key, Modifiers};

use crate::hotkey_text::{Os, combo_text, hint};
use crate::platform::SystemShortcut;

/// The note beside the keys: how to finish. Where the person binds the
/// key themselves, saving alone changes nothing, so it says so.
pub fn note(shortcut: Option<SystemShortcut>) -> &'static str {
    match shortcut {
        Some(SystemShortcut::Manual) => {
            "Enter, then bind flick --toggle to it in your system settings"
        }
        _ => "Enter to save \u{b7} Esc to cancel",
    }
}

/// The combination picked so far, and what to tell the person.
#[derive(Debug, Default)]
pub struct Capture {
    pub candidate: Option<String>,
    pub message: Option<&'static str>,
    /// Why the last confirmed pick was refused (e.g. already in use).
    pub error: Option<String>,
}

/// What a key press means for the view.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Stay,
    Cancel,
    Confirm(String),
}

impl Capture {
    pub fn key(&mut self, key: Key, modifiers: Modifiers, os: Os) -> Step {
        match key {
            Key::Escape => Step::Cancel,
            Key::Enter => match &self.candidate {
                Some(text) => Step::Confirm(text.clone()),
                None => {
                    self.message = Some("Press the keys you want to use");
                    Step::Stay
                }
            },
            _ => {
                match combo_text(modifiers, key) {
                    Ok(text) => {
                        self.candidate = Some(text);
                        self.message = None;
                        self.error = None;
                    }
                    Err(err) => self.message = Some(hint(err, os)),
                }
                Step::Stay
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey_text::Os;
    use eframe::egui::{Key, Modifiers};

    fn press(c: &mut Capture, key: Key, modifiers: Modifiers) -> Step {
        c.key(key, modifiers, Os::Windows)
    }

    #[test]
    fn the_note_says_how_to_finish() {
        use crate::platform::SystemShortcut;
        assert_eq!(note(None), "Enter to save \u{b7} Esc to cancel");
        assert_eq!(
            note(Some(SystemShortcut::Gnome)),
            "Enter to save \u{b7} Esc to cancel"
        );
        assert_eq!(
            note(Some(SystemShortcut::Manual)),
            "Enter, then bind flick --toggle to it in your system settings"
        );
    }

    #[test]
    fn escape_cancels() {
        let mut c = Capture::default();
        assert_eq!(press(&mut c, Key::Escape, Modifiers::NONE), Step::Cancel);
    }

    #[test]
    fn enter_with_nothing_pressed_asks_for_keys() {
        let mut c = Capture::default();
        assert_eq!(press(&mut c, Key::Enter, Modifiers::NONE), Step::Stay);
        assert_eq!(c.message, Some("Press the keys you want to use"));
    }

    #[test]
    fn a_plain_letter_is_refused_with_a_hint() {
        let mut c = Capture::default();
        assert_eq!(press(&mut c, Key::K, Modifiers::NONE), Step::Stay);
        assert_eq!(c.candidate, None);
        assert_eq!(c.message, Some("Add Ctrl, Alt or Shift"));
    }

    #[test]
    fn a_valid_combination_is_kept_with_enter() {
        let mut c = Capture::default();
        assert_eq!(press(&mut c, Key::K, Modifiers::CTRL), Step::Stay);
        assert_eq!(c.candidate.as_deref(), Some("Ctrl+K"));
        assert_eq!(c.message, None);
        assert_eq!(
            press(&mut c, Key::Enter, Modifiers::NONE),
            Step::Confirm("Ctrl+K".into())
        );
    }

    #[test]
    fn a_bad_press_keeps_the_last_good_one() {
        let mut c = Capture::default();
        press(&mut c, Key::K, Modifiers::CTRL);
        press(&mut c, Key::J, Modifiers::NONE);
        assert_eq!(c.candidate.as_deref(), Some("Ctrl+K"));
        assert!(c.message.is_some());
    }
}
