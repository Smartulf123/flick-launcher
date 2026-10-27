//! No system file icons on macOS and Linux: results use Flick's own File
//! and Folder icons, with the type taken from the extension.

use std::path::Path;

use eframe::egui::{self, TextureHandle};

#[derive(Default)]
pub struct IconCache;

impl IconCache {
    /// Always `None`: the caller falls back to Flick's own icons.
    pub fn get(
        &mut self,
        _ctx: &egui::Context,
        _path: &Path,
        _is_dir: bool,
    ) -> Option<&(TextureHandle, String)> {
        None
    }
}
