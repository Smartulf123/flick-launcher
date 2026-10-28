#![windows_subsystem = "windows"]

mod actions;
mod background;
mod bar_menu;
mod builtin_backgrounds;
mod chip_layout;
mod engine_picker;
mod favicons;
mod gallery;
mod gallery_loader;
mod hotkey;
mod hotkey_picker;
mod hotkey_text;
mod http;
mod icons;
mod placement;
mod platform;
mod suggest_fetch;
mod theme;
mod tray;
mod ui;
mod wallhaven;

use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui;
use flick_core::config::{self, BackgroundSource};

use crate::platform::message::Message;
use flick_core::apps::SharedApps;
use flick_core::index::{self, SharedIndex};
use flick_core::launcher::Launcher;
use flick_core::modes::suggest::{SuggestionCache, Suggestions};

/// Starts index rebuilds and tells the UI when one finishes.
#[derive(Clone)]
struct Reindexer {
    shared: SharedIndex,
    apps: SharedApps,
    roots: Vec<PathBuf>,
    excludes: Vec<String>,
    updated: Arc<AtomicBool>,
    ctx: egui::Context,
}

impl Reindexer {
    /// Rebuilds the file index and, alongside it, the app list. The handle
    /// is the file index's.
    fn run(&self) -> std::thread::JoinHandle<()> {
        self.scan_apps();
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

    /// Lists the installed apps on its own thread, then swaps the list in.
    fn scan_apps(&self) {
        let (apps, updated, ctx) = (self.apps.clone(), self.updated.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let started = Instant::now();
            let list = platform::apps::scan();
            log::info!("found {} apps in {:.1?}", list.len(), started.elapsed());
            *apps.write().unwrap_or_else(|e| e.into_inner()) = Some(list);
            updated.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        });
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
    let path = platform::paths::log_path();
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
    // `flick --toggle` (bound to a key where Flick cannot grab one itself)
    // only tells the running Flick, then exits.
    if std::env::args().nth(1).as_deref() == Some("--toggle") {
        platform::instance::send(Message::Toggle);
        return Ok(());
    }
    // The window's context is filled in once it exists, so background
    // threads (and the instance socket) can wake it.
    let wake: Arc<OnceLock<egui::Context>> = Arc::default();
    // What a second `flick` asked for, handled by the bar on its thread.
    let messages: ui::MessageQueue = Arc::default();
    let on_message = {
        let (messages, wake) = (messages.clone(), wake.clone());
        move |msg| {
            messages.lock().unwrap_or_else(|e| e.into_inner()).push(msg);
            if let Some(ctx) = wake.get() {
                ctx.request_repaint();
            }
        }
    };
    let Some(_instance) = platform::instance::acquire(on_message) else {
        platform::instance::send(Message::Show);
        return Ok(());
    };
    start_logging();

    let config_path = platform::paths::config_path();
    let first_run = !config_path.exists();
    let (config, error) = config::load_or_create(&config_path);
    if let Some(e) = error {
        log::error!("{}: {e}; using defaults", config_path.display());
    }
    platform::autostart::sync(first_run);

    // "Results may have changed" (index rebuilt, suggestions arrived).
    let results_changed = Arc::new(AtomicBool::new(false));
    let notify = {
        let (flag, wake) = (results_changed.clone(), wake.clone());
        move || {
            flag.store(true, Ordering::Relaxed);
            if let Some(ctx) = wake.get() {
                ctx.request_repaint();
            }
        }
    };

    let suggestion_cache = SuggestionCache::default();
    let suggestions = Suggestions {
        cache: suggestion_cache.clone(),
        source: suggest_fetch::start(suggestion_cache, notify.clone()),
    };

    let shared = SharedIndex::default();
    let apps = SharedApps::default();
    // Also used to rebuild the bar after its engines change.
    let build_launcher = {
        let (shared, apps) = (shared.clone(), apps.clone());
        move |config: &config::Config| {
            let modes = config::build_modes_with(
                config,
                shared.clone(),
                apps.clone(),
                Some(suggestions.clone()),
            );
            Launcher::new(modes, config.max_results)
        }
    };
    let launcher = build_launcher(&config);

    let roots = if config.files.roots.is_empty() {
        platform::paths::default_roots()
    } else {
        config.files.roots.clone()
    };
    log::info!("indexing {roots:?}");

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Flick")
            .with_inner_size([ui::WIDTH, ui::START_HEIGHT])
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_visible(false),
        centered: true,
        ..Default::default()
    };
    platform::configure(&mut options);

    eframe::run_native(
        "Flick",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let _ = wake.set(ctx.clone());
            let appearance = &config.appearance;
            theme::apply(&ctx, &theme::KANAGAWA);
            let background_source = BackgroundSource::from_setting(&appearance.background);
            let picture = ui::load_picture(&ctx, &background_source);
            let window = platform::BarWindow::from_creation_context(cc)?;
            window.round_corners();
            let shown = Arc::new(AtomicBool::new(false));
            let mut hotkeys =
                hotkey::Hotkeys::new(hotkey::on_press(window.clone(), shown.clone(), ctx.clone()));
            let hotkey_error = match &mut hotkeys {
                Ok(hotkeys) => hotkeys.change(&config.hotkey).err(),
                Err(e) => Some(anyhow::anyhow!("{e:#}")),
            }
            .map(|e| format!("{e:#}"));
            if let Some(e) = &hotkey_error {
                log::error!("hotkey {:?}: {e}", config.hotkey);
            }

            let reindexer = Reindexer {
                shared,
                apps,
                roots,
                excludes: config.files.exclude_dir_names.clone(),
                updated: results_changed.clone(),
                ctx: ctx.clone(),
            };
            reindexer.run();
            reindexer.clone().every(config.files.reindex_minutes);

            // Tray clicks are queued and handled by the app on this thread.
            let tray_queue = ui::TrayQueue::default();
            let tray = {
                let (queue, ctx) = (tray_queue.clone(), ctx.clone());
                tray::build(
                    &tray::status(
                        &config.hotkey,
                        hotkey_error.as_deref(),
                        hotkeys
                            .as_ref()
                            .ok()
                            .and_then(hotkey::Hotkeys::system_shortcut),
                    ),
                    platform::autostart::is_enabled(),
                    &background_source,
                    move |command| {
                        queue
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push(command);
                        ctx.request_repaint();
                    },
                )
            }
            .inspect_err(|e| log::error!("{e:#}"))
            .ok();

            let services = ui::Services {
                results_changed,
                shown,
                messages,
                window,
                hotkey: hotkeys.ok(),
                tray,
                tray_queue,
                reindex: Box::new(move || {
                    reindexer.run();
                }),
                build_launcher: Box::new(build_launcher),
                loader: gallery_loader::Loader::start({
                    let ctx = ctx.clone();
                    move || ctx.request_repaint()
                }),
                logos: favicons::Favicons::start(platform::paths::cache_dir().join("favicons"), {
                    let ctx = ctx.clone();
                    move || ctx.request_repaint()
                }),
            };
            let settings = ui::Settings {
                config: config.clone(),
                path: config_path.clone(),
            };
            Ok(Box::new(ui::FlickApp::new(
                launcher, services, settings, picture,
            )))
        }),
    )
}
