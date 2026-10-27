//! The tray icon: shows the hotkey, and offers Start with Windows, Reload index and Quit.

use anyhow::Context as _;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub const ICON_SIZE: u32 = 32;

/// What the tray menu asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// The checkbox was clicked; Windows has already flipped its tick.
    ToggleStartWithWindows,
    ReloadIndex,
    Quit,
}

/// Tooltip text: the hotkey, or why it is not working.
pub fn tooltip(hotkey: &str, hotkey_error: Option<&str>) -> String {
    match hotkey_error {
        None => format!("Flick ({hotkey})"),
        Some(_) => format!("Flick: hotkey {hotkey} unavailable, see log"),
    }
}

/// A filled circle on a transparent square, drawn in code so the app ships no image files.
pub fn circle_rgba(size: u32) -> Vec<u8> {
    let center = (size as f32 - 1.0) / 2.0;
    let radius = size as f32 / 2.0 - 1.0;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - center, y as f32 - center);
            let inside = dx * dx + dy * dy <= radius * radius;
            rgba.extend_from_slice(&[0x4f, 0x8c, 0xff, if inside { 255 } else { 0 }]);
        }
    }
    rgba
}

/// Builds the tray icon. `on_command` runs on the event-loop thread when a
/// menu item is clicked. Keep the returned icon alive.
pub fn build(
    status: &str,
    start_with_windows: bool,
    on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
) -> anyhow::Result<TrayIcon> {
    let autostart = CheckMenuItem::new("Start with Windows", true, start_with_windows, None);
    let reload = MenuItem::new("Reload index", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append(&autostart)?;
    menu.append(&reload)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    let autostart_id = autostart.id().clone();
    let (reload_id, quit_id) = (reload.id().clone(), quit.id().clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == autostart_id {
            on_command(TrayCommand::ToggleStartWithWindows);
        } else if event.id == reload_id {
            on_command(TrayCommand::ReloadIndex);
        } else if event.id == quit_id {
            on_command(TrayCommand::Quit);
        }
    }));

    let icon = Icon::from_rgba(circle_rgba(ICON_SIZE), ICON_SIZE, ICON_SIZE)?;
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(status)
        .with_icon(icon)
        .build()
        .context("could not create the tray icon")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_names_the_hotkey() {
        assert_eq!(tooltip("Alt+Space", None), "Flick (Alt+Space)");
    }

    #[test]
    fn tooltip_reports_a_failed_hotkey() {
        assert_eq!(
            tooltip("Super+Space", Some("already used")),
            "Flick: hotkey Super+Space unavailable, see log"
        );
    }

    #[test]
    fn icon_is_an_opaque_circle_on_transparent() {
        let rgba = circle_rgba(ICON_SIZE);
        let size = ICON_SIZE as usize;
        assert_eq!(rgba.len(), size * size * 4);
        let alpha = |x: usize, y: usize| rgba[(y * size + x) * 4 + 3];
        assert_eq!(alpha(size / 2, size / 2), 255);
        assert_eq!(alpha(0, 0), 0);
    }
}
