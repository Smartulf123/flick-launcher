//! The tray icon: shows the hotkey, and offers start at login,
//! Background, Change hotkey, Reload index and Quit.

use anyhow::Context as _;
use flick_core::config::BackgroundSource;
use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::platform;

pub const ICON_SIZE: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundChoice {
    /// Opens the background gallery.
    Browse,
    /// Opens the file picker for a picture of your own.
    ChooseFile,
    None,
}

/// What the tray menu asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// The start-at-login checkbox was clicked; the menu has already
    /// flipped its tick.
    ToggleStartAtLogin,
    Background(BackgroundChoice),
    /// Opens the "Change hotkey" view in the bar.
    ChangeHotkey,
    ReloadIndex,
    Quit,
}

/// Tooltip where the person binds the key themselves (Wayland, not GNOME).
pub const MANUAL_SHORTCUT_TIP: &str = "Flick: bind flick --toggle to a key in your system settings";

/// The tooltip for this system: how to set up the key where the person
/// binds it themselves, else the hotkey or why it is not working.
pub fn status(
    hotkey: &str,
    hotkey_error: Option<&str>,
    shortcut: Option<crate::platform::SystemShortcut>,
) -> String {
    match shortcut {
        Some(crate::platform::SystemShortcut::Manual) => MANUAL_SHORTCUT_TIP.to_owned(),
        _ => tooltip(hotkey, hotkey_error),
    }
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

/// The tray icon and the menu items whose ticks the app updates.
/// Menu items are not thread-safe: keep this on the event-loop thread.
pub struct Tray {
    icon: TrayIcon,
    no_background: CheckMenuItem,
    /// The background choices, shown by the gear or a right-click on the bar.
    /// Only Windows pops it up natively; macOS and Linux draw it in the bar.
    #[cfg_attr(not(windows), allow(dead_code))]
    bar_menu: Menu,
}

impl Tray {
    /// The gear menu's items (background choices and Change hotkey).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn bar_menu(&self) -> &Menu {
        &self.bar_menu
    }

    /// Updates the hover text, e.g. after the hotkey changed.
    pub fn set_tooltip(&self, text: &str) {
        if let Err(e) = self.icon.set_tooltip(Some(text)) {
            log::error!("could not update the tray tooltip: {e}");
        }
    }

    /// Ticks None when there is no picture.
    pub fn show_background(&self, source: &BackgroundSource) {
        self.no_background
            .set_checked(*source == BackgroundSource::None);
    }
}

/// Builds the tray icon and menu. `on_command` runs on the event-loop thread
/// when a menu item is clicked.
pub fn build(
    status: &str,
    start_at_login: bool,
    background: &BackgroundSource,
    on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
) -> anyhow::Result<Tray> {
    let mut commands: Vec<(MenuId, TrayCommand)> = Vec::new();

    let autostart = CheckMenuItem::new(platform::autostart::LABEL, true, start_at_login, None);
    commands.push((autostart.id().clone(), TrayCommand::ToggleStartAtLogin));

    let browse = MenuItem::new("Browse backgrounds...", true, None);
    let choose = MenuItem::new("Choose your own picture...", true, None);
    let no_background = CheckMenuItem::new("None", true, false, None);
    for (id, choice) in [
        (browse.id(), BackgroundChoice::Browse),
        (choose.id(), BackgroundChoice::ChooseFile),
        (no_background.id(), BackgroundChoice::None),
    ] {
        commands.push((id.clone(), TrayCommand::Background(choice)));
    }
    let background_menu = Submenu::new("Background", true);
    background_menu.append(&browse)?;
    background_menu.append(&choose)?;
    background_menu.append(&no_background)?;

    let change_hotkey = MenuItem::new("Change hotkey...", true, None);
    commands.push((change_hotkey.id().clone(), TrayCommand::ChangeHotkey));

    let bar_menu = Menu::new();
    bar_menu.append(&browse)?;
    bar_menu.append(&choose)?;
    bar_menu.append(&no_background)?;
    bar_menu.append(&PredefinedMenuItem::separator())?;
    bar_menu.append(&change_hotkey)?;

    let reload = MenuItem::new("Reload index", true, None);
    let quit = MenuItem::new("Quit", true, None);
    commands.push((reload.id().clone(), TrayCommand::ReloadIndex));
    commands.push((quit.id().clone(), TrayCommand::Quit));

    let menu = Menu::new();
    menu.append(&autostart)?;
    menu.append(&background_menu)?;
    menu.append(&change_hotkey)?;
    menu.append(&reload)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some((_, command)) = commands.iter().find(|(id, _)| *id == event.id) {
            on_command(*command);
        }
    }));

    let icon = Icon::from_rgba(circle_rgba(ICON_SIZE), ICON_SIZE, ICON_SIZE)?;
    let builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(status);
    let icon = platform::with_tray_icon(icon, builder)
        .build()
        .context("could not create the tray icon")?;

    let tray = Tray {
        icon,
        no_background,
        bar_menu,
    };
    tray.show_background(background);
    Ok(tray)
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
    fn status_explains_binding_the_key_by_hand() {
        use crate::platform::SystemShortcut;
        assert_eq!(
            status("Ctrl+Alt+Space", None, Some(SystemShortcut::Manual)),
            MANUAL_SHORTCUT_TIP
        );
        assert_eq!(
            status("Ctrl+Alt+Space", None, Some(SystemShortcut::Gnome)),
            "Flick (Ctrl+Alt+Space)"
        );
        assert_eq!(status("Alt+Space", None, None), "Flick (Alt+Space)");
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
