//! The global hotkey that shows the search bar.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;
use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::hotkey_text::{Os, display};
use crate::window::WindowHandle;

/// Parses a hotkey such as `"Alt+Space"` or `"Ctrl+Space"`.
pub fn parse(hotkey: &str) -> anyhow::Result<HotKey> {
    hotkey
        .parse::<HotKey>()
        .with_context(|| format!("hotkey {hotkey:?} is not valid"))
}

/// What pressing the hotkey does: sets `shown` (so the app resets the bar),
/// then shows and focuses `window`.
pub fn on_press(
    window: WindowHandle,
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
    manager: GlobalHotKeyManager,
    current: Option<(HotKey, String)>,
}

impl Hotkeys {
    /// Starts with no hotkey registered; `change` registers one.
    pub fn new(on_press: impl Fn() + Send + Sync + 'static) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new().context("could not start the hotkey manager")?;
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                on_press();
            }
        }));
        Ok(Self {
            manager,
            current: None,
        })
    }

    /// Switches to `text`. The new hotkey is registered before the old one
    /// is released, so a refused hotkey leaves the old one working.
    pub fn change(&mut self, text: &str) -> anyhow::Result<()> {
        let hotkey = parse(text)?;
        if self.current.as_ref().is_some_and(|(old, _)| *old == hotkey) {
            return Ok(());
        }
        self.manager.register(hotkey).with_context(|| {
            format!(
                "{} is already used by another program",
                display(text, Os::CURRENT)
            )
        })?;
        if let Some((old, _)) = self.current.replace((hotkey, text.to_owned())) {
            let _ = self.manager.unregister(old);
        }
        Ok(())
    }

    /// The hotkey in use, as written in the settings.
    pub fn current(&self) -> Option<&str> {
        self.current.as_ref().map(|(_, text)| text.as_str())
    }

    /// Stops the hotkey firing while a new one is being picked.
    pub fn pause(&self) {
        if let Some((hotkey, _)) = &self.current {
            let _ = self.manager.unregister(*hotkey);
        }
    }

    pub fn resume(&self) {
        if let Some((hotkey, _)) = &self.current {
            match self.manager.register(*hotkey) {
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
