//! The global hotkey that shows the search bar.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;
use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::hotkey_text::{Os, display};
use crate::platform::{self, BarWindow, SystemShortcut};

/// Parses a hotkey such as `"Alt+Space"` or `"Ctrl+Space"`.
pub fn parse(hotkey: &str) -> anyhow::Result<HotKey> {
    hotkey
        .parse::<HotKey>()
        .with_context(|| format!("hotkey {hotkey:?} is not valid"))
}

/// What pressing the hotkey does: sets `shown` (so the app resets the bar),
/// then shows and focuses `window`.
pub fn on_press(
    window: BarWindow,
    shown: Arc<AtomicBool>,
    ctx: egui::Context,
) -> impl Fn() + Send + Sync + 'static {
    move || {
        shown.store(true, Ordering::Relaxed);
        window.show_and_focus();
        ctx.request_repaint();
    }
}

/// The system-wide hotkey, which can be changed while Flick runs.
///
/// Must be created on the event-loop thread (the eframe creation closure)
/// and kept alive for the hotkey to stay registered.
pub struct Hotkeys {
    backend: Backend,
    current: Option<(HotKey, String)>,
}

enum Backend {
    /// Flick grabs the key itself (Windows, macOS, Linux on X11).
    Global(GlobalHotKeyManager),
    /// Linux on Wayland: the desktop runs `flick --toggle` on the key.
    System(SystemShortcut),
}

impl Hotkeys {
    /// Starts with no hotkey registered; `change` registers one. Where
    /// apps cannot grab keys, the desktop's own shortcut is used instead.
    pub fn new(on_press: impl Fn() + Send + Sync + 'static) -> anyhow::Result<Self> {
        if let Some(shortcut) = platform::system_shortcut() {
            return Ok(Self::system(shortcut));
        }
        let manager = GlobalHotKeyManager::new().context("could not start the hotkey manager")?;
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                on_press();
            }
        }));
        Ok(Self {
            backend: Backend::Global(manager),
            current: None,
        })
    }

    /// Hotkeys handled by the desktop's shortcut settings.
    pub fn system(shortcut: SystemShortcut) -> Self {
        Self {
            backend: Backend::System(shortcut),
            current: None,
        }
    }

    /// `Some` when the desktop, not Flick, catches the key.
    pub fn system_shortcut(&self) -> Option<SystemShortcut> {
        match self.backend {
            Backend::Global(_) => None,
            Backend::System(shortcut) => Some(shortcut),
        }
    }

    /// Switches to `text`. The new hotkey is registered before the old one
    /// is released, so a refused hotkey leaves the old one working.
    pub fn change(&mut self, text: &str) -> anyhow::Result<()> {
        let hotkey = parse(text)?;
        if self.current.as_ref().is_some_and(|(old, _)| *old == hotkey) {
            return Ok(());
        }
        match &self.backend {
            Backend::Global(manager) => {
                manager.register(hotkey).with_context(|| {
                    format!(
                        "{} is already used by another program",
                        display(text, Os::CURRENT)
                    )
                })?;
                if let Some((old, _)) = &self.current {
                    let _ = manager.unregister(*old);
                }
            }
            Backend::System(shortcut) => platform::apply_system_shortcut(*shortcut, text)?,
        }
        self.current = Some((hotkey, text.to_owned()));
        Ok(())
    }

    /// The hotkey in use, as written in the settings.
    pub fn current(&self) -> Option<&str> {
        self.current.as_ref().map(|(_, text)| text.as_str())
    }

    /// Stops the hotkey firing while a new one is being picked.
    pub fn pause(&self) {
        if let (Backend::Global(manager), Some((hotkey, _))) = (&self.backend, &self.current) {
            let _ = manager.unregister(*hotkey);
        }
    }

    pub fn resume(&self) {
        if let (Backend::Global(manager), Some((hotkey, _))) = (&self.backend, &self.current) {
            match manager.register(*hotkey) {
                Ok(()) | Err(global_hotkey::Error::AlreadyRegistered(_)) => {}
                Err(e) => log::error!("could not restore the hotkey: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    #[test]
    fn parses_configured_hotkeys() {
        assert_eq!(
            parse("Alt+Space").unwrap(),
            HotKey::new(Some(Modifiers::ALT), Code::Space)
        );
        assert_eq!(
            parse("Ctrl+Space").unwrap(),
            HotKey::new(Some(Modifiers::CONTROL), Code::Space)
        );
    }

    #[test]
    fn a_manual_system_shortcut_only_remembers_the_hotkey() {
        // Wayland outside GNOME: the person binds `flick --toggle` themselves.
        let mut hotkeys = Hotkeys::system(SystemShortcut::Manual);
        hotkeys.change("Ctrl+Alt+K").unwrap();
        assert!(hotkeys.change("Banana+Space").is_err());
        assert_eq!(hotkeys.current(), Some("Ctrl+Alt+K"));
        assert_eq!(hotkeys.system_shortcut(), Some(SystemShortcut::Manual));
        // Pausing has nothing to pause.
        hotkeys.pause();
        hotkeys.resume();
    }

    /// Registers real system hotkeys, so it needs a desktop session; the
    /// macOS and Linux CI runners have none.
    #[cfg(windows)]
    #[test]
    fn change_keeps_the_old_hotkey_when_the_new_one_is_bad() {
        let mut hotkeys = Hotkeys::new(|| {}).unwrap();
        hotkeys.change("Ctrl+Alt+Shift+F24").unwrap();
        assert!(hotkeys.change("Banana+Space").is_err());
        assert_eq!(hotkeys.current(), Some("Ctrl+Alt+Shift+F24"));
    }

    #[test]
    fn rejects_unknown_keys() {
        let err = parse("Banana+Space").unwrap_err().to_string();
        assert!(err.contains("Banana+Space"), "{err}");
    }
}
