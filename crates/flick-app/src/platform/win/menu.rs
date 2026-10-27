//! The gear menu as a native Windows pop-up.

use tray_icon::menu::ContextMenu;

use super::BarWindow;
use crate::tray::Tray;

/// Pops up the background choices at the mouse pointer, over the bar.
/// Blocks until the menu closes; a choice arrives as a normal tray command.
pub fn show_native_menu(tray: &Tray, window: &BarWindow) -> bool {
    // SAFETY: the handle is the bar's own live window, owned by this thread.
    unsafe {
        tray.bar_menu()
            .show_context_menu_for_hwnd(window.raw(), None);
    }
    true
}
