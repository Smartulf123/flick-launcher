//! The global hotkey that shows the search bar.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;
use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::window::WindowHandle;

/// Parses a hotkey such as `"Alt+Space"` or `"Ctrl+Space"`.
pub fn parse(hotkey: &str) -> anyhow::Result<HotKey> {
    hotkey
        .parse::<HotKey>()
        .with_context(|| format!("hotkey {hotkey:?} is not valid"))
}

/// Registers `hotkey` system-wide. Pressing it sets `shown` (so the app
/// resets the bar), then shows and focuses `window`.
///
/// Must run on the event-loop thread (the eframe creation closure), and the
/// returned manager must be kept alive for the hotkey to stay registered.
pub fn register(
    hotkey: &str,
    window: WindowHandle,
    shown: Arc<AtomicBool>,
    ctx: egui::Context,
) -> anyhow::Result<GlobalHotKeyManager> {
    let hotkey = parse(hotkey)?;
    let manager = GlobalHotKeyManager::new().context("could not start the hotkey manager")?;
    manager
        .register(hotkey)
        .context("the hotkey is already used by another program")?;
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state == HotKeyState::Pressed {
            shown.store(true, Ordering::Relaxed);
            window.show_and_focus();
            ctx.request_repaint();
        }
    }));
    Ok(manager)
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
    fn rejects_unknown_keys() {
        let err = parse("Banana+Space").unwrap_err().to_string();
        assert!(err.contains("Banana+Space"), "{err}");
    }
}
