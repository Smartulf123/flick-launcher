//! macOS and Linux. They share almost everything; what differs is chosen
//! inside each file.

pub mod autostart;
mod icons;
pub mod instance;
mod window;

pub use icons::IconCache;
pub use window::BarWindow;

use crate::tray::Tray;

/// There is no system pop-up for the gear menu here; the bar shows its own.
pub fn show_native_menu(_tray: &Tray, _window: &BarWindow) -> bool {
    false
}

/// The menu-bar icon is a template on macOS, so it turns white or black
/// with the menu bar.
pub fn with_tray_icon(
    icon: tray_icon::Icon,
    builder: tray_icon::TrayIconBuilder,
) -> tray_icon::TrayIconBuilder {
    #[cfg(target_os = "macos")]
    return builder.with_icon_templated(icon);
    #[cfg(not(target_os = "macos"))]
    builder.with_icon(icon)
}

/// Linux: GNOME gets a shortcut from Flick, other Wayland desktops one from
/// the person. macOS and other X11 desktops let Flick grab the key itself.
pub fn system_shortcut() -> Option<super::SystemShortcut> {
    #[cfg(target_os = "linux")]
    return super::gnome::shortcut_for(|name| std::env::var(name).ok());
    #[cfg(not(target_os = "linux"))]
    None
}

/// Points the desktop's shortcut at `flick --toggle` for the hotkey `text`.
pub fn apply_system_shortcut(shortcut: super::SystemShortcut, text: &str) -> anyhow::Result<()> {
    match shortcut {
        #[cfg(target_os = "linux")]
        super::SystemShortcut::Gnome => {
            let program = autostart::program()
                .ok_or_else(|| anyhow::anyhow!("cannot find Flick's own path"))?;
            super::gnome::apply(text, &program)
        }
        // Nothing Flick can write; the tray tip explains it.
        _ => {
            let _ = text;
            Ok(())
        }
    }
}

/// macOS: Flick is a menu-bar app, with no Dock icon or app menu.
pub fn configure(options: &mut eframe::NativeOptions) {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = options;
}
