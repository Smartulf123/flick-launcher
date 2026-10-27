//! Hotkeys as text: turning a key press into the text stored in the
//! settings file ("Ctrl+Shift+K"), and showing that text the way each
//! system names its keys ("Option + Space" on a Mac).

use eframe::egui::{Key, Modifiers};

/// Which system's key names to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

impl Os {
    pub const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else {
        Os::Windows
    };
}

/// Why a key press cannot be a hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComboError {
    /// A letter, digit or Space on its own would fire while typing.
    NeedsModifier,
    /// Only letters, digits, Space and F1 to F24 are offered.
    UnsupportedKey,
}

const LETTERS: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
];
const DIGITS: [Key; 10] = [
    Key::Num0,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];
const F_KEYS: [Key; 24] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
    Key::F13,
    Key::F14,
    Key::F15,
    Key::F16,
    Key::F17,
    Key::F18,
    Key::F19,
    Key::F20,
    Key::F21,
    Key::F22,
    Key::F23,
    Key::F24,
];

/// The settings text for a pressed combination, modifiers in a fixed
/// order: `Ctrl+Alt+Shift+Super+<key>`. Cmd on a Mac is stored as Super.
pub fn combo_text(modifiers: Modifiers, key: Key) -> Result<String, ComboError> {
    let is_f_key = F_KEYS.contains(&key);
    if !(is_f_key || LETTERS.contains(&key) || DIGITS.contains(&key) || key == Key::Space) {
        return Err(ComboError::UnsupportedKey);
    }
    let held = [
        (modifiers.ctrl, "Ctrl"),
        (modifiers.alt, "Alt"),
        (modifiers.shift, "Shift"),
        (modifiers.mac_cmd, "Super"),
    ];
    let mut parts: Vec<&str> = held.iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect();
    if parts.is_empty() && !is_f_key {
        return Err(ComboError::NeedsModifier);
    }
    parts.push(key.name());
    Ok(parts.join("+"))
}

/// Settings text as people read it on `os`: "Ctrl + Shift + K",
/// "Option + Space". Also tidies hand-written text like "shift+alt+KeyQ".
pub fn display(text: &str, os: Os) -> String {
    text.split('+')
        .map(|token| {
            let lower = token.trim().to_ascii_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => "Ctrl".to_owned(),
                "alt" | "option" if os == Os::Mac => "Option".to_owned(),
                "alt" | "option" => "Alt".to_owned(),
                "shift" => "Shift".to_owned(),
                "super" | "cmd" | "command" | "meta" => match os {
                    Os::Windows => "Win",
                    Os::Mac => "Cmd",
                    Os::Linux => "Super",
                }
                .to_owned(),
                _ => key_name(token.trim()),
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// "KeyQ" → "Q", "Digit1" → "1", "space" → "Space".
fn key_name(token: &str) -> String {
    let bare = token
        .strip_prefix("Key")
        .or_else(|| token.strip_prefix("Digit"))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(token);
    let mut chars = bare.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// What to tell someone whose key press was refused.
pub fn hint(err: ComboError, os: Os) -> &'static str {
    match (err, os) {
        (ComboError::NeedsModifier, Os::Mac) => "Add Cmd, Option, Ctrl or Shift",
        (ComboError::NeedsModifier, _) => "Add Ctrl, Alt or Shift",
        (ComboError::UnsupportedKey, _) => "Use a letter, a number, Space or F1 to F24",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Key, Modifiers};

    fn mac_cmd() -> Modifiers {
        Modifiers {
            mac_cmd: true,
            ..Default::default()
        }
    }

    #[test]
    fn key_presses_become_settings_text() {
        assert_eq!(
            combo_text(Modifiers::CTRL | Modifiers::SHIFT, Key::K),
            Ok("Ctrl+Shift+K".into())
        );
        assert_eq!(
            combo_text(Modifiers::ALT, Key::Space),
            Ok("Alt+Space".into())
        );
        assert_eq!(combo_text(Modifiers::NONE, Key::F5), Ok("F5".into()));
        assert_eq!(combo_text(mac_cmd(), Key::Space), Ok("Super+Space".into()));
    }

    #[test]
    fn a_plain_key_needs_a_modifier() {
        assert_eq!(
            combo_text(Modifiers::NONE, Key::K),
            Err(ComboError::NeedsModifier)
        );
    }

    #[test]
    fn only_letters_digits_space_and_f_keys_are_allowed() {
        assert_eq!(
            combo_text(Modifiers::CTRL, Key::Enter),
            Err(ComboError::UnsupportedKey)
        );
    }

    #[test]
    fn round_trips_through_the_hotkey_parser() {
        let mods = [
            Modifiers::CTRL,
            Modifiers::ALT,
            Modifiers::SHIFT,
            mac_cmd(),
            Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
        ];
        let keys = [
            Key::A,
            Key::Z,
            Key::Num0,
            Key::Num9,
            Key::Space,
            Key::F1,
            Key::F24,
        ];
        for m in mods {
            for k in keys {
                let text = combo_text(m, k).unwrap();
                assert!(crate::hotkey::parse(&text).is_ok(), "{text}");
            }
        }
    }

    #[test]
    fn shown_with_each_systems_key_names() {
        assert_eq!(display("Alt+Space", Os::Mac), "Option + Space");
        assert_eq!(display("Super+Space", Os::Windows), "Win + Space");
        assert_eq!(display("Super+Space", Os::Mac), "Cmd + Space");
        assert_eq!(display("Super+Space", Os::Linux), "Super + Space");
        assert_eq!(display("shift+alt+KeyQ", Os::Linux), "Shift + Alt + Q");
        assert_eq!(display("Ctrl+Digit1", Os::Windows), "Ctrl + 1");
    }

    #[test]
    fn hints_name_the_keys_of_each_system() {
        assert_eq!(
            hint(ComboError::NeedsModifier, Os::Windows),
            "Add Ctrl, Alt or Shift"
        );
        assert_eq!(
            hint(ComboError::NeedsModifier, Os::Mac),
            "Add Cmd, Option, Ctrl or Shift"
        );
        assert_eq!(
            hint(ComboError::UnsupportedKey, Os::Linux),
            "Use a letter, a number, Space or F1 to F24"
        );
    }
}
