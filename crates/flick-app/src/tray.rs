//! The tray icon: shows the hotkey, and offers Start with Windows, Theme,
//! Background, Reload index and Quit.

use anyhow::Context as _;
use flick_core::config::BackgroundSource;
use tray_icon::menu::{
    CheckMenuItem, ContextMenu, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::theme::PRESETS;

pub const ICON_SIZE: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundChoice {
    Wallpaper,
    /// Opens the picker in Windows' own wallpaper folder.
    WindowsPictures,
    /// Opens the picker anywhere.
    ChooseFile,
    None,
}

/// What the tray menu asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// The checkbox was clicked; Windows has already flipped its tick.
    ToggleStartWithWindows,
    /// Index into `theme::PRESETS`.
    SetTheme(usize),
    Background(BackgroundChoice),
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

/// The tray icon and the menu items whose ticks the app updates.
/// Menu items are not thread-safe: keep this on the event-loop thread.
pub struct Tray {
    _icon: TrayIcon,
    themes: Vec<CheckMenuItem>,
    wallpaper: CheckMenuItem,
    no_background: CheckMenuItem,
    /// Theme and Background, shown when the bar itself is right-clicked.
    bar_menu: Menu,
}

impl Tray {
    /// Pops up Theme and Background at the mouse pointer, over `hwnd`.
    /// Blocks until the menu closes; a choice arrives as a normal tray command.
    pub fn show_bar_menu(&self, hwnd: isize) {
        // SAFETY: `hwnd` is the bar's own live window, owned by this thread.
        unsafe {
            self.bar_menu.show_context_menu_for_hwnd(hwnd, None);
        }
    }

    /// Ticks the theme at `index` and unticks the rest.
    pub fn show_theme(&self, index: usize) {
        for (i, item) in self.themes.iter().enumerate() {
            item.set_checked(i == index);
        }
    }

    /// Ticks Wallpaper or None to match `source`; a picture file ticks neither.
    pub fn show_background(&self, source: &BackgroundSource) {
        self.wallpaper
            .set_checked(*source == BackgroundSource::Wallpaper);
        self.no_background
            .set_checked(*source == BackgroundSource::None);
    }
}

/// Builds the tray icon and menu. `on_command` runs on the event-loop thread
/// when a menu item is clicked.
pub fn build(
    status: &str,
    start_with_windows: bool,
    theme_index: usize,
    background: &BackgroundSource,
    on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
) -> anyhow::Result<Tray> {
    let mut commands: Vec<(MenuId, TrayCommand)> = Vec::new();

    let autostart = CheckMenuItem::new("Start with Windows", true, start_with_windows, None);
    commands.push((autostart.id().clone(), TrayCommand::ToggleStartWithWindows));

    let themes: Vec<CheckMenuItem> = PRESETS
        .iter()
        .enumerate()
        .map(|(i, p)| CheckMenuItem::new(p.name, true, i == theme_index, None))
        .collect();
    for (i, item) in themes.iter().enumerate() {
        commands.push((item.id().clone(), TrayCommand::SetTheme(i)));
    }
    let theme_menu = Submenu::new("Theme", true);
    for item in &themes {
        theme_menu.append(item)?;
    }

    let wallpaper = CheckMenuItem::new("My desktop wallpaper", true, false, None);
    let windows_pictures = MenuItem::new("Windows pictures...", true, None);
    let choose = MenuItem::new("Choose a picture...", true, None);
    let no_background = CheckMenuItem::new("None", true, false, None);
    for (id, choice) in [
        (wallpaper.id(), BackgroundChoice::Wallpaper),
        (windows_pictures.id(), BackgroundChoice::WindowsPictures),
        (choose.id(), BackgroundChoice::ChooseFile),
        (no_background.id(), BackgroundChoice::None),
    ] {
        commands.push((id.clone(), TrayCommand::Background(choice)));
    }
    let background_menu = Submenu::new("Background", true);
    background_menu.append(&wallpaper)?;
    background_menu.append(&windows_pictures)?;
    background_menu.append(&choose)?;
    background_menu.append(&no_background)?;

    let reload = MenuItem::new("Reload index", true, None);
    let quit = MenuItem::new("Quit", true, None);
    commands.push((reload.id().clone(), TrayCommand::ReloadIndex));
    commands.push((quit.id().clone(), TrayCommand::Quit));

    let menu = Menu::new();
    menu.append(&autostart)?;
    menu.append(&theme_menu)?;
    menu.append(&background_menu)?;

    let bar_menu = Menu::new();
    bar_menu.append(&theme_menu)?;
    bar_menu.append(&background_menu)?;
    menu.append(&reload)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some((_, command)) = commands.iter().find(|(id, _)| *id == event.id) {
            on_command(*command);
        }
    }));

    let icon = Icon::from_rgba(circle_rgba(ICON_SIZE), ICON_SIZE, ICON_SIZE)?;
    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(status)
        .with_icon(icon)
        .build()
        .context("could not create the tray icon")?;

    let tray = Tray {
        _icon: icon,
        themes,
        wallpaper,
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
    fn icon_is_an_opaque_circle_on_transparent() {
        let rgba = circle_rgba(ICON_SIZE);
        let size = ICON_SIZE as usize;
        assert_eq!(rgba.len(), size * size * 4);
        let alpha = |x: usize, y: usize| rgba[(y * size + x) * 4 + 3];
        assert_eq!(alpha(size / 2, size / 2), 255);
        assert_eq!(alpha(0, 0), 0);
    }
}
