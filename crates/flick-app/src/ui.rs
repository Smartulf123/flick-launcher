//! The search bar window: a background picture with two floating cards
//! (search, results), driven by key presses and tray commands.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Align, Color32, Key, Layout, Modifiers, RichText, TextEdit, TextureHandle,
    TextureOptions, ViewportCommand, vec2,
};
use flick_core::config::{self, BackgroundSource, Config, MIN_ENGINES};
use flick_core::launcher::{Effect, Intent, Launcher};
use flick_core::mode::{Action, ResultItem, ResultKind};

use crate::actions::run_action;
use crate::bar_menu;
use crate::engine_picker::{self, Picker, PickerAction};
use crate::gallery::{Gallery, GalleryAction, Pick, Picks};
use crate::gallery_loader::{Done, Job, Loader};
use crate::hotkey::Hotkeys;
use crate::hotkey_picker::{Capture, Step};
use crate::hotkey_text::{Os, display};
use crate::icons::Icon;
use crate::platform::message::Message;
use crate::platform::{self, BarWindow, IconCache, autostart, paths};
use crate::theme::{self, KANAGAWA, Palette};
use crate::tray::{self, BackgroundChoice, Tray, TrayCommand};
use crate::wallhaven::{self, Wallpaper};
use crate::{background, placement};

pub const WIDTH: f32 = 640.0;
/// Height before the first frame measures the real content.
pub const START_HEIGHT: f32 = 120.0;
/// About the bar's height with a full list of results. The background is
/// fitted to this height once, so it stays put while results come and go.
const FULL_HEIGHT: f32 = 520.0;
/// Space around and between the cards, where the picture shows.
const GAP: i8 = 10;
const ICON_SIZE: f32 = 20.0;
/// The window's size while the background gallery is open.
const GALLERY_WIDTH: f32 = 960.0;
const GALLERY_HEIGHT: f32 = 640.0;

/// Tray clicks waiting for the event-loop thread.
pub type TrayQueue = Arc<Mutex<Vec<TrayCommand>>>;

/// Messages from a second `flick` (such as `flick --toggle`), waiting for
/// the event-loop thread.
pub type MessageQueue = Arc<Mutex<Vec<Message>>>;

/// The parts of the app that keep running while the bar is hidden.
pub struct Services {
    /// Set when results may have changed: a new file index, or suggestions arrived.
    pub results_changed: Arc<AtomicBool>,
    /// Set by the hotkey handler each time it shows the bar.
    pub shown: Arc<AtomicBool>,
    pub messages: MessageQueue,
    pub window: BarWindow,
    /// The system-wide hotkey; `None` if the hotkey manager would not start.
    pub hotkey: Option<Hotkeys>,
    /// Keeps the tray icon alive; `None` if it could not be created.
    pub tray: Option<Tray>,
    pub tray_queue: TrayQueue,
    /// Starts a file index rebuild.
    pub reindex: Box<dyn Fn()>,
    /// A fresh launcher for these settings (after the bar's engines change).
    pub build_launcher: Box<dyn Fn(&Config) -> Launcher>,
    /// Background workers for the gallery (search, thumbnails, downloads).
    pub loader: Loader,
}

/// The config as loaded, and where to save changes made from the tray.
pub struct Settings {
    pub config: Config,
    pub path: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
enum Visibility {
    Shown,
    Hidden,
}

pub struct FlickApp {
    launcher: Launcher,
    services: Services,
    settings: Settings,
    palette: &'static Palette,
    picture: Option<TextureHandle>,
    icons: IconCache,
    /// Keys held back one frame because text was typed before them.
    pending: Vec<Intent>,
    height: f32,
    was_focused: bool,
    /// Whether the window has been moved to its opening spot yet.
    positioned: bool,
    /// The search box was right-clicked; open the background menu.
    wants_bar_menu: bool,
    /// The background gallery, while it is open.
    gallery: Option<Gallery>,
    /// Background choices in order; only the latest may be applied.
    picks: Picks,
    /// Where the bar was when the gallery opened, to go back there.
    bar_position: Option<egui::Pos2>,
    /// The "Change hotkey" view, while it is open.
    capture: Option<Capture>,
    /// Whether the bar is on screen, for `flick --toggle`.
    visible: bool,
    /// The gear menu drawn inside the bar (where there is no system pop-up).
    menu_open: bool,
    /// A row of that menu was clicked this frame.
    menu_choice: Option<TrayCommand>,
    /// The "+" engine list, while it is open.
    picker: Option<Picker>,
    /// The chip being dragged to a new place, by position on the bar.
    dragging: Option<usize>,
    /// Flick changed the typed text itself (a shortcut, Backspace back);
    /// put the cursor at its end.
    cursor_to_end: bool,
}

/// Loads the picture for `source` as a texture; `None` for no picture or on error.
pub fn load_picture(ctx: &egui::Context, source: &BackgroundSource) -> Option<TextureHandle> {
    match background::load_source(source) {
        Ok(image) => {
            image.map(|image| ctx.load_texture("background", image, TextureOptions::LINEAR))
        }
        Err(e) => {
            log::error!("background picture: {e:#}");
            None
        }
    }
}

impl FlickApp {
    pub fn new(
        launcher: Launcher,
        services: Services,
        settings: Settings,
        picture: Option<TextureHandle>,
    ) -> Self {
        Self {
            launcher,
            services,
            settings,
            palette: &KANAGAWA,
            picture,
            icons: IconCache::default(),
            pending: Vec::new(),
            height: START_HEIGHT,
            was_focused: false,
            positioned: false,
            wants_bar_menu: false,
            gallery: None,
            picks: Picks::default(),
            bar_position: None,
            capture: None,
            visible: false,
            menu_open: false,
            menu_choice: None,
            picker: None,
            dragging: None,
            cursor_to_end: false,
        }
    }

    fn apply(&mut self, effect: Effect) -> Visibility {
        match effect {
            Effect::Run(action) => {
                if let Err(e) = run_action(&action) {
                    log::error!("could not open {action:?}: {e}");
                }
                Visibility::Shown
            }
            Effect::Hide => {
                self.hide();
                Visibility::Hidden
            }
        }
    }

    fn hide(&mut self) {
        self.launcher.reset();
        self.pending.clear();
        self.was_focused = false;
        self.close_capture();
        self.menu_open = false;
        self.picker = None;
        self.dragging = None;
        self.visible = false;
        self.services.window.hide();
    }

    /// Opens the "Change hotkey" view, with the hotkey paused meanwhile.
    fn open_capture(&mut self, ctx: &egui::Context) {
        self.close_gallery(ctx);
        self.picker = None;
        self.launcher.reset();
        self.pending.clear();
        if let Some(hotkeys) = &self.services.hotkey {
            hotkeys.pause();
        }
        self.capture = Some(Capture::default());
        self.visible = true;
        self.services.window.show_and_focus();
    }

    /// Leaves the "Change hotkey" view; the hotkey in use works again.
    fn close_capture(&mut self) {
        if self.capture.take().is_some()
            && let Some(hotkeys) = &self.services.hotkey
        {
            hotkeys.resume();
        }
    }

    /// Switches to the picked hotkey and saves it, or explains why not.
    fn confirm_hotkey(&mut self, text: String) {
        let result = match &mut self.services.hotkey {
            Some(hotkeys) => hotkeys.change(&text),
            None => Err(anyhow::anyhow!("the hotkey manager is not running")),
        };
        match result {
            Ok(()) => {
                log::info!("hotkey: {text}");
                self.settings.config.hotkey = text.clone();
                self.save_settings();
                if let Some(tray) = &self.services.tray {
                    let shortcut = self
                        .services
                        .hotkey
                        .as_ref()
                        .and_then(Hotkeys::system_shortcut);
                    tray.set_tooltip(&tray::status(&text, None, shortcut));
                }
                self.close_capture();
            }
            Err(e) => {
                log::warn!("hotkey {text}: {e:#}");
                if let Some(capture) = &mut self.capture {
                    capture.error = Some(format!("{e:#}"));
                }
            }
        }
    }

    /// Changes the bar's engines, saves the settings and rebuilds the bar
    /// with them, keeping what was typed.
    fn change_engines(&mut self, change: impl FnOnce(&mut Config)) {
        change(&mut self.settings.config);
        self.save_settings();
        let text = self.launcher.input().to_owned();
        self.launcher = (self.services.build_launcher)(&self.settings.config);
        if self.picker.is_none() && !text.is_empty() {
            self.launcher.set_input(&text);
        }
    }

    fn picker_action(&mut self, action: PickerAction) {
        match action {
            PickerAction::Add(id) => self.change_engines(|c| {
                if let Err(e) = c.add_engine(&id) {
                    log::warn!("could not add {id}: {e:?}");
                }
            }),
            PickerAction::AddCustom(engine) => {
                log::info!("added engine {} ({})", engine.name, engine.shortcut);
                self.change_engines(|c| c.add_custom(engine));
            }
            PickerAction::DeleteCustom(id) => self.change_engines(|c| c.remove_custom(&id)),
            PickerAction::Close => self.picker = None,
        }
    }

    fn save_settings(&self) {
        if let Err(e) = config::save(&self.settings.path, &self.settings.config) {
            log::error!("could not save {}: {e}", self.settings.path.display());
        }
    }

    fn handle_tray(&mut self, ctx: &egui::Context, command: TrayCommand) {
        match command {
            TrayCommand::ToggleStartAtLogin => autostart::toggle(),
            TrayCommand::Background(choice) => self.set_background(ctx, choice),
            TrayCommand::ChangeHotkey => self.open_capture(ctx),
            TrayCommand::ReloadIndex => (self.services.reindex)(),
            TrayCommand::Quit => {
                log::info!("quit from tray");
                // Remove the tray icon now so Windows does not leave a ghost icon.
                self.services.tray = None;
                self.services.hotkey = None;
                std::process::exit(0);
            }
        }
    }

    fn set_background(&mut self, ctx: &egui::Context, choice: BackgroundChoice) {
        match choice {
            BackgroundChoice::Browse => self.open_gallery(ctx),
            BackgroundChoice::ChooseFile => {
                if let Some(path) = background::pick_picture(&paths::pictures_dir()) {
                    self.picks.next();
                    self.use_background(ctx, BackgroundSource::File(path));
                }
            }
            BackgroundChoice::None => {
                self.picks.next();
                self.use_background(ctx, BackgroundSource::None);
            }
        }
        // The menu flips a tick on click; set the ticks to what is really shown.
        if let Some(tray) = &self.services.tray {
            let shown = BackgroundSource::from_setting(&self.settings.config.appearance.background);
            tray.show_background(&shown);
        }
    }

    /// Shows `source` and saves it. A picture that will not load leaves the
    /// current background as it is and returns false.
    fn use_background(&mut self, ctx: &egui::Context, source: BackgroundSource) -> bool {
        let picture = load_picture(ctx, &source);
        if picture.is_none() && source != BackgroundSource::None {
            return false;
        }
        self.picture = picture;
        self.settings.config.appearance.background = source.to_setting();
        self.save_settings();
        log::info!("background: {}", source.to_setting());
        true
    }

    /// The usable area of the window's monitor, in egui points.
    fn work_area(&self, ctx: &egui::Context) -> Option<egui::Rect> {
        let [left, top, right, bottom] = self.services.window.work_area()?;
        let scale = ctx
            .input(|i| i.viewport().native_pixels_per_point)
            .unwrap_or_else(|| ctx.pixels_per_point());
        Some(egui::Rect::from_min_max(
            egui::pos2(left as f32, top as f32) / scale,
            egui::pos2(right as f32, bottom as f32) / scale,
        ))
    }

    /// Turns the window into the gallery: larger, centered on where the bar
    /// was, inside the same monitor, and shown (it may be opened from the
    /// tray). It is an ordinary window while open, so a browser opened from
    /// "Open page" can come in front of it.
    fn open_gallery(&mut self, ctx: &egui::Context) {
        if self.gallery.is_some() {
            return;
        }
        self.gallery = Some(Gallery::new(
            paths::backgrounds_dir(),
            paths::cache_dir(),
            self.services.loader.generations(),
        ));
        self.picker = None;
        self.launcher.reset();
        self.pending.clear();

        let outer = ctx.input(|i| i.viewport().outer_rect);
        self.bar_position = outer.map(|r| r.min);
        let wanted = vec2(GALLERY_WIDTH, GALLERY_HEIGHT);
        let work = self.work_area(ctx);
        let size = work.map_or(wanted, |w| placement::fit_size(w, wanted, 16.0));
        self.height = size.y;
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        if let (Some(outer), Some(work)) = (outer, work) {
            let centered = egui::pos2(outer.left() - (size.x - WIDTH) / 2.0, outer.top());
            let at = placement::place_in(work.shrink(16.0), size, centered);
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(at));
        }
        self.visible = true;
        self.services.window.show_and_focus();
    }

    /// Back to the search bar, where it was before the gallery opened.
    fn close_gallery(&mut self, ctx: &egui::Context) {
        if self.gallery.take().is_none() {
            return;
        }
        // Searches and thumbnails still queued are no longer wanted.
        self.services.loader.generations().next();
        // Previews only live while the gallery is open; keep the one in use.
        let in_use =
            match BackgroundSource::from_setting(&self.settings.config.appearance.background) {
                BackgroundSource::File(path) => Some(path),
                _ => None,
            };
        for stale in crate::gallery::stale_previews(&paths::cache_dir(), in_use.as_deref()) {
            let _ = std::fs::remove_file(stale);
        }
        self.height = START_HEIGHT;
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(WIDTH, START_HEIGHT)));
        if let Some(at) = self.bar_position.take() {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(at));
        }
    }

    /// A finished search, thumbnail or download from the loader.
    fn handle_loaded(&mut self, ctx: &egui::Context, done: Done) {
        match done {
            Done::Downloaded { id, pick, result } => {
                let ok = match (result, pick) {
                    // A favorite, or an older preview: saved, not shown.
                    (Ok(_), None) => true,
                    (Ok(_), Some(pick)) if !self.picks.is_latest(pick) => true,
                    (Ok(path), Some(_)) => self.use_background(ctx, BackgroundSource::File(path)),
                    (Err(e), _) => {
                        log::error!("download of wallhaven {id}: {e:#}");
                        false
                    }
                };
                if let Some(gallery) = &mut self.gallery {
                    if ok {
                        gallery.note_saved();
                    } else {
                        gallery.note_download_failed();
                    }
                }
            }
            other => {
                if let Some(gallery) = &mut self.gallery {
                    for job in gallery.apply(ctx, other) {
                        self.services.loader.send(job);
                    }
                }
            }
        }
    }

    /// The gallery view: drawn instead of the bar while it is open.
    fn gallery_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.close_gallery(ctx);
            return;
        }
        // Backspace in the empty box goes back from an engine reached by shortcut.
        if self.picker.is_none()
            && self.launcher.input().is_empty()
            && ctx.input(|i| i.key_pressed(Key::Backspace))
        {
            self.launcher.handle(Intent::BackspaceOnEmpty);
            self.cursor_to_end = true;
        }

        let window = ui.max_rect();
        self.paint_background(ui, window);
        let mover = ui.interact(
            window,
            ui.scope_id().with("move gallery"),
            egui::Sense::drag(),
        );
        if mover.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let p = self.palette;
        let current = BackgroundSource::from_setting(&self.settings.config.appearance.background);
        let Some(gallery) = self.gallery.as_mut() else {
            return;
        };
        let action = egui::Frame::NONE
            .inner_margin(egui::Margin::same(GAP))
            .show(ui, |ui| {
                egui::Frame::NONE
                    .fill(theme::card_fill(p, 0.85))
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(14)
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height(ui.available_height());
                        gallery.show(ui, p, &current)
                    })
                    .inner
            })
            .inner;
        match action {
            GalleryAction::None => {}
            GalleryAction::Back => self.close_gallery(ctx),
            GalleryAction::Pick(Pick::Source(source)) => {
                self.picks.next();
                if !self.use_background(ctx, source) {
                    log::error!("could not open the picked background");
                }
            }
            GalleryAction::Pick(Pick::Download(wallpaper)) => {
                let pick = self.picks.next();
                self.services.loader.send(Job::Download {
                    wallpaper,
                    dir: paths::cache_dir(),
                    pick: Some(pick),
                });
            }
            GalleryAction::Favorite(wallpaper) => self.add_favorite(wallpaper),
            GalleryAction::Unfavorite(path) => {
                self.remove_favorite(&path);
                if let Some(gallery) = &mut self.gallery {
                    gallery.note_saved();
                }
            }
            GalleryAction::Jobs(jobs) => {
                for job in jobs {
                    self.services.loader.send(job);
                }
            }
        }
    }

    /// Keeps an online picture under Favorites: copies the preview if there
    /// is one, else downloads it without changing the background.
    fn add_favorite(&mut self, wallpaper: Wallpaper) {
        let favorites = paths::backgrounds_dir();
        let cached = wallhaven::download_path(&paths::cache_dir(), &wallpaper.id)
            .filter(|path| path.is_file());
        let target = wallhaven::download_path(&favorites, &wallpaper.id);
        if let (Some(cached), Some(target)) = (cached, target) {
            // Move the preview over; if it is the background, follow it.
            let moved = std::fs::create_dir_all(&favorites)
                .and_then(|()| std::fs::rename(&cached, &target));
            match moved {
                Ok(()) => {
                    let background = &mut self.settings.config.appearance.background;
                    if *background == BackgroundSource::File(cached).to_setting() {
                        *background = BackgroundSource::File(target).to_setting();
                        self.save_settings();
                    }
                }
                Err(e) => log::error!("could not save favorite: {e}"),
            }
            if let Some(gallery) = &mut self.gallery {
                gallery.note_saved();
            }
        } else {
            self.services.loader.send(Job::Download {
                wallpaper,
                dir: favorites,
                pick: None,
            });
        }
    }

    /// Removes a picture from Favorites. If it is the background in use, it
    /// moves to the cache instead, so the background keeps working.
    fn remove_favorite(&mut self, path: &std::path::Path) {
        let in_use = BackgroundSource::from_setting(&self.settings.config.appearance.background)
            == BackgroundSource::File(path.to_path_buf());
        let result = match (in_use, path.file_name()) {
            (true, Some(name)) => {
                let moved = paths::cache_dir().join(name);
                std::fs::create_dir_all(paths::cache_dir())
                    .and_then(|()| std::fs::rename(path, &moved))
                    .map(|()| {
                        self.settings.config.appearance.background =
                            BackgroundSource::File(moved).to_setting();
                        self.save_settings();
                    })
            }
            _ => std::fs::remove_file(path),
        };
        if let Err(e) = result {
            log::error!("could not remove favorite {}: {e}", path.display());
        }
    }

    /// The background picture (cropped to fill), or the plain theme color.
    fn paint_background(&self, ui: &egui::Ui, window: egui::Rect) {
        match &self.picture {
            Some(picture) => {
                let uv = background::anchored_uv(picture.size(), window.size(), FULL_HEIGHT);
                ui.painter().image(picture.id(), window, uv, Color32::WHITE);
            }
            None => {
                ui.painter().rect_filled(window, 0, self.palette.background);
            }
        }
    }

    /// A floating card: see-through over a picture, solid without one.
    fn card(&self) -> egui::Frame {
        let p = self.palette;
        let fill = if self.picture.is_some() {
            theme::card_fill(p, self.settings.config.appearance.clamped_tint())
        } else {
            p.background
        };
        egui::Frame::NONE
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, p.border))
            .corner_radius(14)
            .inner_margin(egui::Margin::symmetric(14, 10))
    }

    /// The "Change hotkey" view: takes every key press, shows the pick.
    fn capture_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let presses = take_key_presses(ctx);
        for (key, modifiers) in presses {
            let Some(capture) = &mut self.capture else {
                break;
            };
            match capture.key(key, modifiers, Os::CURRENT) {
                Step::Stay => {}
                Step::Cancel => self.close_capture(),
                Step::Confirm(text) => self.confirm_hotkey(text),
            }
        }
        let window = ui.max_rect();
        self.paint_background(ui, window);
        let Some(capture) = &self.capture else {
            return;
        };
        let p = self.palette;
        // One row, like the search box: the keys (or a grey prompt) on the
        // left, a short note on the right.
        let keys = match capture.candidate.as_deref() {
            Some(text) => RichText::new(display(text, Os::CURRENT))
                .text_style(egui::TextStyle::Heading)
                .color(p.text),
            None => {
                let prompt = match self.services.hotkey.as_ref().and_then(Hotkeys::current) {
                    Some(now) => format!("Press a new hotkey (now {})", display(now, Os::CURRENT)),
                    None => "Press a new hotkey".to_owned(),
                };
                theme::hint(&prompt, p.muted)
            }
        };
        let note = capture
            .error
            .as_deref()
            .or(capture.message)
            .unwrap_or_else(|| {
                crate::hotkey_picker::note(
                    self.services
                        .hotkey
                        .as_ref()
                        .and_then(Hotkeys::system_shortcut),
                )
            });
        let card = self.card();
        let content = egui::Frame::NONE
            .inner_margin(egui::Margin::same(GAP))
            .show(ui, |ui| {
                card.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icon_slot(ui, Icon::Settings, ICON_SIZE, p.muted);
                        ui.label(keys);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(note).small().color(p.muted));
                        });
                    });
                });
            });
        self.fit_height(ctx, content.response.rect.height());
    }

    /// Draws the two cards. Returns the effects of a mouse click on a result.
    fn draw(&mut self, ui: &mut egui::Ui) -> Vec<Effect> {
        let p = self.palette;
        let card = self.card();

        let mut clicked_mode = None;
        let mut chip_change = None;
        let mut clicked_row = None;
        let mut hovered_row = None;

        card.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // The chips wrap to a second line rather than run under the gear.
                let row = vec2(ui.available_width() - 34.0, 0.0);
                let wrap = Layout::left_to_right(Align::Center).with_main_wrap(true);
                ui.allocate_ui_with_layout(row, wrap, |ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    chip_change = self.chip_row(ui, p, &mut clicked_mode);
                });
                // Settings: the same Theme and Background menu as right-click.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if settings_button(ui, p).clicked() {
                        self.wants_bar_menu = true;
                        ui.ctx().request_repaint();
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                icon_slot(ui, Icon::Search, ICON_SIZE, p.muted);
                let picking = self.picker.is_some();
                let placeholder = if picking {
                    "Filter engines\u{2026}".to_owned()
                } else {
                    theme::placeholder(self.launcher.active_mode_name())
                };
                let mut text = match &self.picker {
                    Some(picker) => picker.filter.clone(),
                    None => self.launcher.input().to_owned(),
                };
                let response = ui.add(
                    TextEdit::singleline(&mut text)
                        .font(egui::TextStyle::Heading)
                        .frame(egui::Frame::NONE)
                        .hint_text(theme::hint(&placeholder, p.muted))
                        .desired_width(f32::INFINITY),
                );
                // This, not key consumption, is what keeps focus here after Tab.
                // The form's own fields keep the focus while it is open.
                if !self.picker.as_ref().is_some_and(Picker::form_open) {
                    response.request_focus();
                }
                if response.secondary_clicked() {
                    // Opened at the start of the next frame, with the bar drawn.
                    self.wants_bar_menu = true;
                    ui.ctx().request_repaint();
                }
                if response.changed() {
                    match &mut self.picker {
                        Some(picker) => picker.set_filter(&text),
                        None => {
                            self.launcher.set_input(&text);
                            // A shortcut was taken out of the text.
                            self.cursor_to_end |= self.launcher.input() != text;
                        }
                    }
                }
                if std::mem::take(&mut self.cursor_to_end) {
                    let end = self.launcher.input().chars().count();
                    if let Some(mut state) =
                        egui::text_edit::TextEditState::load(ui.ctx(), response.id)
                    {
                        let cursor = egui::text::CCursor::new(end);
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                        state.store(ui.ctx(), response.id);
                    }
                }
            });
        });

        if let Some(change) = chip_change {
            self.apply_chip_change(change);
        }

        // The engine list takes the place of the results while open.
        if self.picker.is_some() {
            ui.add_space(GAP as f32 * 0.8);
            let mut action = None;
            card.show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(picker) = &mut self.picker {
                    action = picker.draw(ui, p, &self.settings.config);
                }
            });
            if let Some(action) = action {
                self.picker_action(action);
            }
            return Vec::new();
        }

        // The in-bar gear menu takes the place of the results while open.
        if self.menu_open {
            ui.add_space(GAP as f32 * 0.8);
            card.show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.menu_choice = bar_menu::rows(ui, p);
            });
            return Vec::new();
        }

        // No results, no second card: the bar is just the search card.
        let has_results = !self.launcher.results().is_empty();
        if has_results {
            ui.add_space(GAP as f32 * 0.8);
        }

        if has_results {
            card.show(ui, |ui| {
                ui.set_width(ui.available_width());
                let selected = self.launcher.selected();
                // Moving the mouse over a row selects it; a resting pointer does
                // not fight the arrow keys.
                let pointer_moved = ui.input(|i| i.pointer.is_moving());
                // All results (at most max_results) are shown; the window
                // slides up if needed rather than the list scrolling.
                for (i, item) in self.launcher.results().iter().enumerate() {
                    let response = result_row(ui, p, &mut self.icons, item, i == selected);
                    if response.clicked() {
                        clicked_row = Some(i);
                    } else if pointer_moved && response.hovered() && i != selected {
                        hovered_row = Some(i);
                    }
                }
            });
        }

        if let Some(i) = hovered_row {
            self.launcher.select(i);
        }
        if let Some(i) = clicked_mode {
            self.launcher.select_chip(i);
        }
        if let Some(i) = clicked_row {
            self.launcher.select(i);
            return self.launcher.handle(Intent::Enter);
        }
        Vec::new()
    }

    /// The bar's chips, the shortcut engine in use and the "+" chip. Sets
    /// `clicked` for a chip click; returns a removal or a move to save.
    fn chip_row(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        clicked: &mut Option<usize>,
    ) -> Option<ChipChange> {
        let names: Vec<String> = self
            .launcher
            .chips()
            .into_iter()
            .map(String::from)
            .collect();
        let active = self.launcher.active_chip();
        let removable = names.len() > MIN_ENGINES;
        let pointer = ui.ctx().pointer_interact_pos();
        let mut rects = Vec::with_capacity(names.len());
        let mut change = None;
        let mut released = None;
        for (i, name) in names.iter().enumerate() {
            let response = mode_chip(ui, p, name, Some(i) == active);
            rects.push(response.rect);
            let badge = remove_badge(response.rect);
            let on_badge = pointer.is_some_and(|pos| badge.contains(pos));
            if removable && response.hovered() && self.dragging.is_none() {
                let painter = ui.painter();
                painter.circle_filled(
                    badge.center(),
                    7.0,
                    if on_badge { p.warning } else { p.border },
                );
                painter.text(
                    badge.center(),
                    egui::Align2::CENTER_CENTER,
                    "\u{d7}",
                    egui::FontId::proportional(12.0),
                    p.text,
                );
            }
            if response.clicked() {
                if removable && on_badge {
                    change = Some(ChipChange::Remove(i));
                } else {
                    *clicked = Some(i);
                }
            }
            if response.drag_started() {
                self.dragging = Some(i);
            }
            if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            if response.drag_stopped() {
                released = self.dragging.take().zip(pointer);
            }
        }
        // Worked out once every chip has its place for this frame.
        if let Some((from, pos)) = released {
            let centers: Vec<f32> = rects.iter().map(|r| r.center().x).collect();
            let to = engine_picker::drop_index(&centers, from, pos.x);
            if to != from {
                change = Some(ChipChange::Move(from, to));
            }
        }
        // While dragging, a line shows where the chip will land.
        if let (Some(from), Some(pos)) = (self.dragging, pointer) {
            let centers: Vec<f32> = rects.iter().map(|r| r.center().x).collect();
            let to = engine_picker::drop_index(&centers, from, pos.x);
            let others: Vec<&egui::Rect> = rects
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != from)
                .map(|(_, r)| r)
                .collect();
            let x = match others.get(to) {
                Some(r) => r.left() - 3.0,
                None => others
                    .last()
                    .map_or(rects[from].left(), |r| r.right() + 3.0),
            };
            let (top, bottom) = (rects[from].top(), rects[from].bottom());
            ui.painter().line_segment(
                [egui::pos2(x, top), egui::pos2(x, bottom)],
                egui::Stroke::new(2.0, p.accent),
            );
        }
        if let Some(name) = self.launcher.visiting() {
            visiting_chip(ui, p, name);
        }
        if plus_chip(ui, p, self.picker.is_some()).clicked() {
            self.menu_open = false;
            self.picker = match self.picker {
                Some(_) => None,
                None => Some(Picker::default()),
            };
        }
        change
    }

    fn apply_chip_change(&mut self, change: ChipChange) {
        let Some(id) = self.chip_id(match change {
            ChipChange::Remove(i) | ChipChange::Move(i, _) => i,
        }) else {
            return;
        };
        match change {
            ChipChange::Remove(_) => self.change_engines(|c| {
                if let Err(e) = c.remove_engine(&id) {
                    log::warn!("could not remove {id}: {e:?}");
                }
            }),
            ChipChange::Move(from, to) => self.change_engines(|c| c.move_engine(from, to)),
        }
    }

    /// The settings id of the chip at `position` on the bar.
    fn chip_id(&self, position: usize) -> Option<String> {
        self.settings.config.chips().get(position).cloned()
    }

    /// Resizes the window to the height the content actually used.
    /// Resizes the window to the height the content actually used, sliding
    /// it up if the new height would run past the bottom of the screen.
    fn fit_height(&mut self, ctx: &egui::Context, content_height: f32) {
        let height = content_height.ceil();
        if (height - self.height).abs() < 1.0 {
            return;
        }
        self.height = height;
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(WIDTH, height)));
        let (outer, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        if let (Some(outer), Some(monitor)) = (outer, monitor)
            && let Some(top) = placement::keep_on_screen(outer.top(), height, monitor.y)
        {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(
                outer.left(),
                top,
            )));
        }
    }
}

/// A change made on the chip row, by position on the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChipChange {
    Remove(usize),
    Move(usize, usize),
}

/// Allocates a square of `size` and draws `icon` in it.
fn icon_slot(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    icon.paint(ui.painter(), rect.shrink(1.0), color);
}

/// A mode chip: icon and name. Clickable.
fn mode_chip(ui: &mut egui::Ui, p: &Palette, name: &str, active: bool) -> egui::Response {
    // Every chip has its own dark backing and bright text; the active one
    // stands out by its fill and accent outline.
    let (fill, stroke) = if active {
        (p.selected, p.accent)
    } else {
        (theme::chip_fill(p), p.border)
    };
    let text = p.text;
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.horizontal(|ui| {
                icon_slot(ui, Icon::for_mode(name), 14.0, text);
                ui.label(RichText::new(name).size(13.0).color(text));
            });
        })
        .response
        .interact(egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The engine reached by shortcut, shown after the chips while in use.
fn visiting_chip(ui: &mut egui::Ui, p: &Palette, name: &str) {
    egui::Frame::NONE
        .fill(theme::chip_fill(p))
        .stroke(egui::Stroke::new(1.0, p.accent))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.horizontal(|ui| {
                icon_slot(ui, Icon::for_mode(name), 14.0, p.accent);
                ui.label(RichText::new(name).size(13.0).color(p.accent));
            });
        })
        .response
        .on_hover_text("Reached by shortcut; Tab goes back to your engines");
}

/// The "+" at the end of the chips; opens and closes the engine list.
fn plus_chip(ui: &mut egui::Ui, p: &Palette, open: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::click());
    let fill = if open || response.hovered() {
        p.selected
    } else {
        theme::chip_fill(p)
    };
    ui.painter().rect_filled(rect, 8, fill);
    ui.painter().rect_stroke(
        rect,
        8,
        egui::Stroke::new(1.0, p.border),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(17.0),
        p.text,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Add or find engines")
}

/// The remove mark in a hovered chip's top-right corner.
fn remove_badge(chip: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(chip.right_top() + vec2(-3.0, 3.0), vec2(14.0, 14.0))
}

/// The gear at the end of the chip row; opens Theme and Background.
fn settings_button(ui: &mut egui::Ui, p: &Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::click());
    let fill = if response.hovered() {
        p.selected
    } else {
        theme::chip_fill(p)
    };
    ui.painter().rect_filled(rect, 8, fill);
    Icon::Settings.paint(ui.painter(), rect.shrink(5.0), p.text);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Theme and background")
}

/// Draws a result's icon and returns its kind label ("Google", "Microsoft
/// Word Document", ...). Files and folders get their real Windows icon.
fn row_icon(
    ui: &mut egui::Ui,
    p: &Palette,
    icons: &mut IconCache,
    item: &ResultItem,
    selected: bool,
) -> String {
    let fallback = if selected { p.accent } else { p.muted };
    match (&item.kind, &item.action) {
        (ResultKind::File | ResultKind::Folder, Action::OpenPath(path)) => {
            let is_dir = item.kind == ResultKind::Folder;
            match icons.get(ui.ctx(), path, is_dir) {
                Some((texture, type_name)) => {
                    let (id, type_name) = (texture.id(), type_name.clone());
                    ui.add(egui::Image::new((id, vec2(ICON_SIZE, ICON_SIZE))));
                    type_name
                }
                None => {
                    icon_slot(ui, Icon::for_kind(item.kind), ICON_SIZE, fallback);
                    crate::icons::kind_label(path, is_dir)
                }
            }
        }
        (ResultKind::Web, Action::OpenUrl(url)) => {
            let icon = Icon::for_url(url);
            icon_slot(ui, icon, ICON_SIZE, fallback);
            match icon {
                Icon::Google => "Google".into(),
                Icon::YouTube => "YouTube".into(),
                _ => "Web".into(),
            }
        }
        _ => {
            icon_slot(ui, Icon::for_kind(item.kind), ICON_SIZE, fallback);
            String::new()
        }
    }
}

/// One result: icon, title, a one-line path, and the kind label on the right.
fn result_row(
    ui: &mut egui::Ui,
    p: &Palette,
    icons: &mut IconCache,
    item: &ResultItem,
    selected: bool,
) -> egui::Response {
    let fill = if selected {
        p.selected
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let label = row_icon(ui, p, icons, item, selected);
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !label.is_empty() {
                        ui.label(RichText::new(label).small().color(p.muted));
                    }
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.add(
                            egui::Label::new(RichText::new(&item.title).color(p.text)).truncate(),
                        );
                        if !item.subtitle.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&item.subtitle).small().color(p.muted),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl eframe::App for FlickApp {
    /// Runs even while hidden: tray commands, a finished index, a fresh
    /// show, and hiding when the user clicks into another window.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let commands = std::mem::take(
            &mut *self
                .services
                .tray_queue
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        for command in commands {
            self.handle_tray(ctx, command);
        }
        for done in self.services.loader.take_done() {
            self.handle_loaded(ctx, done);
        }
        if !self.positioned
            && let Some(monitor) = ctx.input(|i| i.viewport().monitor_size)
        {
            // Open a bit above the middle so the list has room to grow.
            let at = placement::initial_position(monitor, WIDTH);
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(at));
            self.positioned = true;
        }
        let messages = std::mem::take(
            &mut *self
                .services
                .messages
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        for message in messages {
            match message_action(message, self.visible, self.capture.is_some()) {
                Some(BarAction::Hide) => self.hide(),
                // Shown like the hotkey shows it, reset below.
                Some(BarAction::Show) => {
                    self.services.shown.store(true, Ordering::Relaxed);
                    self.services.window.show_and_focus();
                }
                None => {}
            }
        }
        if self.services.shown.swap(false, Ordering::Relaxed) {
            self.visible = true;
            // The hotkey always brings back the search bar.
            self.close_gallery(ctx);
            // Every show starts empty, whatever happened before the last hide.
            self.launcher.reset();
            self.picker = None;
            self.pending.clear();
        }
        if self.services.results_changed.swap(false, Ordering::Relaxed) {
            self.launcher.refresh();
        }
        let focused = ctx.input(|i| i.viewport().focused) == Some(true);
        // The gallery stays open when another window is clicked.
        if self.gallery.is_none() && self.was_focused && !focused {
            self.hide();
        }
        self.was_focused = focused;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.capture.is_some() {
            self.capture_ui(ui, &ctx);
            return;
        }
        if self.gallery.is_some() {
            self.gallery_ui(ui, &ctx);
            return;
        }
        // The form's fields move with Tab, so it is left to them.
        let form_open = self.picker.as_ref().is_some_and(Picker::form_open);
        let keys = read_intents(&ctx, form_open);
        self.pending.extend(keys.intents);
        if keys.after_text {
            // Let the text box take this frame's typing first; run the keys next frame.
            ctx.request_repaint();
        } else {
            for intent in std::mem::take(&mut self.pending) {
                // Esc closes the in-bar menu first.
                if intent == Intent::Escape && std::mem::take(&mut self.menu_open) {
                    continue;
                }
                // The open engine list takes the keys; Esc closes it, not the bar.
                if let Some(picker) = &mut self.picker {
                    if let Some(action) = picker.handle(intent, &self.settings.config) {
                        self.picker_action(action);
                    }
                    continue;
                }
                for effect in self.launcher.handle(intent) {
                    if self.apply(effect) == Visibility::Hidden {
                        // Drop the rest of this frame, typing included, so
                        // nothing leaks into the next show.
                        return;
                    }
                }
            }
        }

        let window = ui.max_rect();
        self.paint_background(ui, window);

        // Dragging anywhere that is not a text box or a row moves the window.
        // Added before the cards, so the cards' own widgets win their clicks.
        let mover = ui.interact(
            window,
            ui.scope_id().with("move window"),
            egui::Sense::click_and_drag(),
        );
        if mover.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        // Right-click on the bar or the gear: the background menu, as a
        // system pop-up where there is one, else inside the bar.
        if mover.secondary_clicked() || std::mem::take(&mut self.wants_bar_menu) {
            let native = self
                .services
                .tray
                .as_ref()
                .is_some_and(|tray| platform::show_native_menu(tray, &self.services.window));
            if native {
                // The pop-up takes focus while open; give it back to the search box.
                self.services.window.show_and_focus();
            } else {
                self.menu_open = !self.menu_open;
            }
        }

        let content = egui::Frame::NONE
            .inner_margin(egui::Margin::same(GAP))
            .show(ui, |ui| self.draw(ui));
        self.fit_height(&ctx, content.response.rect.height());
        if let Some(command) = self.menu_choice.take() {
            self.menu_open = false;
            self.handle_tray(&ctx, command);
        }
        for effect in content.inner {
            if self.apply(effect) == Visibility::Hidden {
                return;
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.background.to_normalized_gamma_f32()
    }
}

/// What a message from a second `flick` does to the bar.
#[derive(Debug, PartialEq, Eq)]
enum BarAction {
    Show,
    Hide,
}

/// `None` while a new hotkey is being picked: the old one is paused
/// there, and on GNOME Wayland pressing it runs `flick --toggle`.
fn message_action(message: Message, visible: bool, capturing: bool) -> Option<BarAction> {
    if capturing {
        return None;
    }
    match message {
        Message::Toggle if visible => Some(BarAction::Hide),
        Message::Toggle | Message::Show => Some(BarAction::Show),
    }
}

/// Every key pressed this frame, with its modifiers; typed text is dropped
/// too, so nothing reaches the search box underneath.
fn take_key_presses(ctx: &egui::Context) -> Vec<(Key, Modifiers)> {
    ctx.input_mut(|input| {
        let mut presses = Vec::new();
        input.events.retain(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                presses.push((*key, *modifiers));
                false
            }
            egui::Event::Text(_) | egui::Event::Key { .. } => false,
            _ => true,
        });
        presses
    })
}

/// The launcher's keys pressed in one frame, in press order.
#[derive(Debug, Default)]
pub struct FrameKeys {
    pub intents: Vec<Intent>,
    /// Text was typed before the first key. The text box only sees that
    /// text later in the frame, so the keys must wait one frame.
    pub after_text: bool,
}

/// Takes the launcher's keys out of the input queue before any widget sees
/// them, so the text box never loses focus to Tab.
/// With `keep_tab`, Tab is left for the focused widget (to move between
/// a form's fields).
pub fn read_intents(ctx: &egui::Context, keep_tab: bool) -> FrameKeys {
    ctx.input_mut(|input| {
        let mut keys = FrameKeys::default();
        let mut seen_text = false;
        input.events.retain(|event| match event {
            egui::Event::Text(_) => {
                seen_text = true;
                true
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => match intent_for(*key, *modifiers).filter(|_| !(keep_tab && *key == Key::Tab)) {
                Some(intent) => {
                    if keys.intents.is_empty() {
                        keys.after_text = seen_text;
                    }
                    keys.intents.push(intent);
                    false
                }
                None => true,
            },
            _ => true,
        });
        keys
    })
}

fn intent_for(key: Key, modifiers: Modifiers) -> Option<Intent> {
    if modifiers.ctrl || modifiers.alt || modifiers.command {
        return None;
    }
    match key {
        Key::Tab if modifiers.shift => Some(Intent::ShiftTab),
        Key::Tab => Some(Intent::Tab),
        Key::ArrowUp => Some(Intent::Up),
        Key::ArrowDown => Some(Intent::Down),
        Key::Enter => Some(Intent::Enter),
        Key::Escape => Some(Intent::Escape),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_show_and_hide_the_bar() {
        assert_eq!(
            message_action(Message::Toggle, true, false),
            Some(BarAction::Hide)
        );
        assert_eq!(
            message_action(Message::Toggle, false, false),
            Some(BarAction::Show)
        );
        assert_eq!(
            message_action(Message::Show, true, false),
            Some(BarAction::Show)
        );
    }

    #[test]
    fn picking_a_hotkey_is_not_interrupted_by_the_old_one() {
        // On GNOME Wayland the old hotkey runs `flick --toggle`.
        assert_eq!(message_action(Message::Toggle, true, true), None);
        assert_eq!(message_action(Message::Show, true, true), None);
    }
    use eframe::egui::{self, Event, Key, Modifiers, RawInput};

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn keys_for(events: Vec<Event>) -> FrameKeys {
        keys_keeping_tab(events, false)
    }

    fn keys_keeping_tab(events: Vec<Event>, keep_tab: bool) -> FrameKeys {
        let ctx = egui::Context::default();
        let raw = RawInput {
            events,
            ..Default::default()
        };
        let mut out = FrameKeys::default();
        ctx.run_ui(raw, |ui| out = read_intents(ui.ctx(), keep_tab))
            .textures_delta
            .clear();
        out
    }

    fn intents_for(events: Vec<Event>) -> Vec<Intent> {
        keys_for(events).intents
    }

    #[test]
    fn a_form_keeps_tab_for_its_fields() {
        let keys = keys_keeping_tab(
            vec![
                press(Key::Tab, Modifiers::NONE),
                press(Key::Enter, Modifiers::NONE),
            ],
            true,
        );
        assert_eq!(keys.intents, vec![Intent::Enter]);
    }

    #[test]
    fn intents_keep_press_order() {
        assert_eq!(
            intents_for(vec![
                press(Key::ArrowDown, Modifiers::NONE),
                press(Key::ArrowUp, Modifiers::NONE),
            ]),
            vec![Intent::Down, Intent::Up]
        );
    }

    #[test]
    fn key_after_typed_text_is_flagged() {
        // "rust" + Enter rolled into one frame: the `t` reaches the text box
        // after this function runs, so Enter must wait a frame.
        let keys = keys_for(vec![
            Event::Text("t".into()),
            press(Key::Enter, Modifiers::NONE),
        ]);
        assert_eq!(keys.intents, vec![Intent::Enter]);
        assert!(keys.after_text);
    }

    #[test]
    fn key_alone_is_not_flagged() {
        assert!(!keys_for(vec![press(Key::Enter, Modifiers::NONE)]).after_text);
    }

    #[test]
    fn shift_tab_is_not_read_as_tab() {
        assert_eq!(
            intents_for(vec![press(Key::Tab, Modifiers::SHIFT)]),
            vec![Intent::ShiftTab]
        );
        assert_eq!(
            intents_for(vec![press(Key::Tab, Modifiers::NONE)]),
            vec![Intent::Tab]
        );
    }

    #[test]
    fn navigation_keys_become_intents() {
        let got = intents_for(vec![
            press(Key::ArrowDown, Modifiers::NONE),
            press(Key::ArrowUp, Modifiers::NONE),
            press(Key::Enter, Modifiers::NONE),
            press(Key::Escape, Modifiers::NONE),
        ]);
        for want in [Intent::Down, Intent::Up, Intent::Enter, Intent::Escape] {
            assert!(got.contains(&want), "{want:?} missing from {got:?}");
        }
    }

    #[test]
    fn tab_is_consumed_so_egui_cannot_move_focus() {
        let ctx = egui::Context::default();
        let raw = RawInput {
            events: vec![press(Key::Tab, Modifiers::NONE)],
            ..Default::default()
        };
        let mut still_there = true;
        let mut output = ctx.run_ui(raw, |ui| {
            // What actually keeps focus in the text box is `request_focus()` in
            // `draw`; this only checks that no widget sees the Tab press.
            read_intents(ui.ctx(), false);
            still_there = ui.ctx().input(|i| i.key_pressed(Key::Tab));
        });
        output.textures_delta.clear();
        assert!(!still_there);
    }
}
