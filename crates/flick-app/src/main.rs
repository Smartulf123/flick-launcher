#![windows_subsystem = "windows"]

mod actions;
mod hotkey;
mod paths;
mod single_instance;
mod tray;
mod ui;
mod window;

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eframe::egui;
use flick_core::config::{self, Config};
use flick_core::index::{self, SharedIndex};
use flick_core::launcher::Launcher;

/// Starts index rebuilds and tells the UI when one finishes.
#[derive(Clone)]
struct Reindexer {
    shared: SharedIndex,
    roots: Vec<PathBuf>,
    excludes: Vec<String>,
    updated: Arc<AtomicBool>,
    ctx: egui::Context,
}

impl Reindexer {
    fn run(&self) -> std::thread::JoinHandle<()> {
        let (shared, updated, ctx) = (self.shared.clone(), self.updated.clone(), self.ctx.clone());
        let started = Instant::now();
        index::spawn_rebuild(
            self.shared.clone(),
            self.roots.clone(),
            self.excludes.clone(),
            move || {
                let count = shared
                    .read()
                    .map(|g| g.as_ref().map_or(0, |i| i.entries.len()))
                    .unwrap_or(0);
                log::info!("indexed {count} entries in {:.1?}", started.elapsed());
                updated.store(true, Ordering::Relaxed);
                ctx.request_repaint();
            },
        )
    }

    /// Rebuilds every `minutes` minutes, forever, on its own thread.
    fn every(self, minutes: u64) {
        if minutes == 0 {
            return;
        }
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(minutes * 60));
                let _ = self.run().join();
            }
        });
    }
}

fn start_logging() {
    let path = paths::log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // One log per run: the file is replaced at each start.
    if let Ok(file) = File::create(&path) {
        let _ = simplelog::WriteLogger::init(
            log::LevelFilter::Info,
            simplelog::Config::default(),
            file,
        );
    }
}

fn main() -> eframe::Result {
    let Some(_instance) = single_instance::acquire() else {
        return Ok(());
    };
    start_logging();

    let config_path = paths::config_path();
    let (config, error) = config::load_or_create(&config_path);
    if let Some(e) = error {
        log::error!("{}: {e}; using defaults", config_path.display());
    }

    let shared = SharedIndex::default();
    let (registry, default_mode) = config::build_registry(&config, shared.clone())
        .or_else(|e| {
            log::error!("bad keywords in config, using defaults: {e}");
            config::build_registry(&Config::default(), shared.clone())
        })
        .expect("default keywords are valid");
    if registry.index_of(&config.default_mode).is_none() {
        log::warn!(
            "unknown default_mode {:?}, using the first mode",
            config.default_mode
        );
    }
    let launcher = Launcher::new(registry, default_mode, config.max_results);

    let roots = if config.files.roots.is_empty() {
        paths::default_roots()
    } else {
        config.files.roots.clone()
    };
    log::info!("indexing {roots:?}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Flick")
            .with_inner_size([ui::WIDTH, 64.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_visible(false),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Flick",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let window = window::WindowHandle::from_creation_context(cc)?;
            let shown = Arc::new(AtomicBool::new(false));
            let hotkey = hotkey::register(&config.hotkey, window, shown.clone(), ctx.clone());
            let hotkey_error = hotkey.as_ref().err().map(|e| format!("{e:#}"));
            if let Some(e) = &hotkey_error {
                log::error!("hotkey {:?}: {e}", config.hotkey);
            }

            let reindexer = Reindexer {
                shared,
                roots,
                excludes: config.files.exclude_dir_names.clone(),
                updated: Arc::new(AtomicBool::new(false)),
                ctx: ctx.clone(),
            };
            reindexer.run();
            reindexer.clone().every(config.files.reindex_minutes);

            let quit = Arc::new(AtomicBool::new(false));
            let tray = {
                let (reindexer, quit, ctx) = (reindexer.clone(), quit.clone(), ctx.clone());
                tray::build(
                    &tray::tooltip(&config.hotkey, hotkey_error.as_deref()),
                    move |command| match command {
                        tray::TrayCommand::ReloadIndex => {
                            reindexer.run();
                        }
                        tray::TrayCommand::Quit => {
                            quit.store(true, Ordering::Relaxed);
                            ctx.request_repaint();
                        }
                    },
                )
            }
            .inspect_err(|e| log::error!("{e:#}"))
            .ok();

            Ok(Box::new(ui::FlickApp::new(
                launcher,
                ui::Background {
                    index_updated: reindexer.updated,
                    quit,
                    shown,
                    window,
                    hotkey: hotkey.ok(),
                    tray,
                },
            )))
        }),
    )
}
