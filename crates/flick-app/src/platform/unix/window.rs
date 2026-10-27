//! Showing and hiding the bar through the window library (winit, via
//! egui's viewport commands). On Linux the window always runs on X11
//! (XWayland under Wayland), so it can be placed and kept on top.

use eframe::egui::{self, ViewportCommand};

/// The search bar's window. Cheap to clone and safe to use from the
/// hotkey's thread.
#[derive(Clone)]
pub struct BarWindow(egui::Context);

impl BarWindow {
    pub fn from_creation_context(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        Ok(Self(cc.egui_ctx.clone()))
    }

    pub fn show_and_focus(&self) {
        self.0.send_viewport_cmd(ViewportCommand::Visible(true));
        self.0.send_viewport_cmd(ViewportCommand::Focus);
        self.0.request_repaint();
    }

    pub fn hide(&self) {
        self.0.send_viewport_cmd(ViewportCommand::Visible(false));
        self.0.request_repaint();
    }

    /// Not known here; callers use the whole monitor instead.
    pub fn work_area(&self) -> Option<[i32; 4]> {
        None
    }

    /// Square corners on macOS and Linux.
    pub fn round_corners(&self) {}
}
