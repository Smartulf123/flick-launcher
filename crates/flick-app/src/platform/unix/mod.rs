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
