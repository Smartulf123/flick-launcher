#![windows_subsystem = "windows"]

mod actions;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use flick_core::config::{self, Config};
use flick_core::index::{self, SharedIndex};
use flick_core::launcher::Launcher;

fn env_path(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_default()
}

fn main() -> eframe::Result {
    let config_path = env_path("APPDATA").join("flick").join("config.toml");
    let (config, error) = config::load_or_create(&config_path);
    if let Some(e) = error {
        eprintln!("{}: {e}", config_path.display());
    }

    let shared = SharedIndex::default();
    let (registry, default_mode) = config::build_registry(&config, shared.clone())
        .or_else(|e| {
            eprintln!("bad keywords in config, using defaults: {e}");
            config::build_registry(&Config::default(), shared.clone())
        })
        .expect("default keywords are valid");
    let launcher = Launcher::new(registry, default_mode, config.max_results);

    let roots = if config.files.roots.is_empty() {
        let home = env_path("USERPROFILE");
        ["Desktop", "Documents", "Downloads"]
            .map(|d| home.join(d))
            .to_vec()
    } else {
        config.files.roots.clone()
    };
    let excludes = config.files.exclude_dir_names.clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Flick")
            .with_inner_size([ui::WIDTH, 64.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Flick",
        options,
        Box::new(move |cc| {
            let index_updated = Arc::new(AtomicBool::new(false));
            let flag = index_updated.clone();
            let ctx = cc.egui_ctx.clone();
            index::spawn_rebuild(shared, roots, excludes, move || {
                flag.store(true, Ordering::Relaxed);
                ctx.request_repaint();
            });
            Ok(Box::new(ui::FlickApp::new(launcher, index_updated)))
        }),
    )
}
