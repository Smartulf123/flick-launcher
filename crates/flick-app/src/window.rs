//! Showing and hiding the search bar through Win32.
//!
//! All visibility changes go through here, never `ViewportCommand::Visible`,
//! so the hotkey handler can show the window while egui is idle.

use anyhow::bail;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SW_HIDE, SW_SHOW,
    SetForegroundWindow, ShowWindow,
};

/// The search bar's window, as a plain number so it can cross threads.
#[derive(Debug, Clone, Copy)]
pub struct WindowHandle(isize);

impl WindowHandle {
    pub fn from_creation_context(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        match cc.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => Ok(Self(handle.hwnd.get())),
            other => bail!("expected a Win32 window, got {other:?}"),
        }
    }

    /// The raw window handle, for APIs that take it as a number.
    pub fn raw(self) -> isize {
        self.0
    }

    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }

    /// Shows the window and moves keyboard focus to it. Windows refuses
    /// `SetForegroundWindow` from background processes in some cases; the
    /// fallback borrows the foreground thread's input state for one call.
    pub fn show_and_focus(self) {
        let hwnd = self.hwnd();
        // SAFETY: plain Win32 calls on a window this process owns; a stale
        // handle makes them fail harmlessly.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            if SetForegroundWindow(hwnd).as_bool() {
                return;
            }
            let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let this_thread = GetCurrentThreadId();
            if foreground_thread != 0 && foreground_thread != this_thread {
                let _ = AttachThreadInput(this_thread, foreground_thread, true);
                let _ = BringWindowToTop(hwnd);
                let _ = SetForegroundWindow(hwnd);
                let _ = AttachThreadInput(this_thread, foreground_thread, false);
            }
        }
    }

    /// Asks Windows 11 to round the window's corners. Earlier Windows
    /// versions ignore the request and keep square corners.
    pub fn round_corners(self) {
        let preference = DWMWCP_ROUND;
        // SAFETY: the pointer and size describe `preference`, which outlives the call.
        let result = unsafe {
            DwmSetWindowAttribute(
                self.hwnd(),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                std::ptr::from_ref(&preference).cast(),
                size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            )
        };
        if let Err(e) = result {
            log::info!("rounded corners not available: {e}");
        }
    }

    pub fn hide(self) {
        // SAFETY: see `show_and_focus`.
        unsafe {
            let _ = ShowWindow(self.hwnd(), SW_HIDE);
        }
    }
}
