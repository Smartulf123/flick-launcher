//! The search bar window: a background picture with two floating cards
//! (search, results), driven by key presses and tray commands.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Align, Color32, Key, Layout, Modifiers, RichText, TextEdit, TextureHandle,
    TextureOptions, ViewportCommand, vec2,
};
use flick_core::config::{self, BackgroundSource, Config};
use flick_core::launcher::{Effect, Intent, Launcher};
use flick_core::mode::{Action, ResultItem, ResultKind};
use global_hotkey::GlobalHotKeyManager;

use crate::actions::run_action;
use crate::icons::Icon;
use crate::shell_icons::IconCache;
use crate::theme::{self, PRESETS, Palette};
use crate::tray::{BackgroundChoice, Tray, TrayCommand};
use crate::window::WindowHandle;
use crate::{autostart, background, paths, placement};

pub const WIDTH: f32 = 640.0;
/// Height before the first frame measures the real content.
pub const START_HEIGHT: f32 = 120.0;
/// Space around and between the cards, where the picture shows.
const GAP: i8 = 10;
const ICON_SIZE: f32 = 20.0;

/// Tray clicks waiting for the event-loop thread.
pub type TrayQueue = Arc<Mutex<Vec<TrayCommand>>>;

/// The parts of the app that keep running while the bar is hidden.
pub struct Services {
    /// Set when results may have changed: a new file index, or suggestions arrived.
    pub results_changed: Arc<AtomicBool>,
    /// Set by the hotkey handler each time it shows the bar.
    pub shown: Arc<AtomicBool>,
    pub window: WindowHandle,
    /// Keeps the hotkey registered; `None` if registration failed.
    pub hotkey: Option<GlobalHotKeyManager>,
    /// Keeps the tray icon alive; `None` if it could not be created.
    pub tray: Option<Tray>,
    pub tray_queue: TrayQueue,
    /// Starts a file index rebuild.
    pub reindex: Box<dyn Fn()>,
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
    /// The search box was right-clicked; open the Theme and Background menu.
    wants_bar_menu: bool,
}

/// Loads the picture for `source` as a texture; `None` for no picture or on error.
pub fn load_picture(ctx: &egui::Context, source: &BackgroundSource) -> Option<TextureHandle> {
    let path = background::resolve(source)?;
    match background::load(&path) {
        Ok(image) => Some(ctx.load_texture("background", image, TextureOptions::LINEAR)),
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
        let palette = theme::palette(&settings.config.appearance.theme);
        Self {
            launcher,
            services,
            settings,
            palette,
            picture,
            icons: IconCache::default(),
            pending: Vec::new(),
            height: START_HEIGHT,
            was_focused: false,
            positioned: false,
            wants_bar_menu: false,
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
        self.services.window.hide();
    }

    fn save_settings(&self) {
        if let Err(e) = config::save(&self.settings.path, &self.settings.config) {
            log::error!("could not save {}: {e}", self.settings.path.display());
        }
    }

    fn handle_tray(&mut self, ctx: &egui::Context, command: TrayCommand) {
        match command {
            TrayCommand::ToggleStartWithWindows => autostart::toggle(),
            TrayCommand::SetTheme(index) => self.set_theme(ctx, index),
            TrayCommand::Background(choice) => self.set_background(ctx, choice),
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

    fn set_theme(&mut self, ctx: &egui::Context, index: usize) {
        let Some(palette) = PRESETS.get(index) else {
            return;
        };
        self.palette = palette;
        theme::apply(ctx, palette);
        if let Some(tray) = &self.services.tray {
            tray.show_theme(index);
        }
        self.settings.config.appearance.theme = palette.id.to_owned();
        self.save_settings();
        log::info!("theme: {}", palette.name);
    }

    fn set_background(&mut self, ctx: &egui::Context, choice: BackgroundChoice) {
        let current = BackgroundSource::from_setting(&self.settings.config.appearance.background);
        let source = match choice {
            BackgroundChoice::Wallpaper => Some(BackgroundSource::Wallpaper),
            BackgroundChoice::None => Some(BackgroundSource::None),
            BackgroundChoice::WindowsPictures => {
                background::pick_picture(&background::windows_pictures_dir())
                    .map(BackgroundSource::File)
            }
            BackgroundChoice::ChooseFile => {
                background::pick_picture(&paths::pictures_dir()).map(BackgroundSource::File)
            }
        };
        // Cancelled picker, or a picture that will not load: keep what we had.
        let applied = source.and_then(|source| {
            let picture = load_picture(ctx, &source);
            let wants_picture = source != BackgroundSource::None;
            (picture.is_some() || !wants_picture).then_some((source, picture))
        });
        let shown = match applied {
            Some((source, picture)) => {
                self.picture = picture;
                self.settings.config.appearance.background = source.to_setting();
                self.save_settings();
                log::info!("background: {}", source.to_setting());
                source
            }
            None => current,
        };
        // The menu flips a tick on click; set the ticks to what is really shown.
        if let Some(tray) = &self.services.tray {
            tray.show_background(&shown);
        }
    }

    /// Draws the two cards. Returns the effects of a mouse click on a result.
    fn draw(&mut self, ui: &mut egui::Ui) -> Vec<Effect> {
        let p = self.palette;
        let fill = if self.picture.is_some() {
            theme::card_fill(p, self.settings.config.appearance.clamped_tint())
        } else {
            p.background
        };
        let card = egui::Frame::NONE
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, p.border))
            .corner_radius(14)
            .inner_margin(egui::Margin::symmetric(14, 10));

        let mut clicked_mode = None;
        let mut clicked_row = None;
        let mut hovered_row = None;

        card.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let active = self.launcher.effective_mode();
                for (i, (name, keyword)) in self.launcher.modes().into_iter().enumerate() {
                    if mode_chip(ui, p, name, keyword, i == active).clicked() {
                        clicked_mode = Some(i);
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                icon_slot(ui, Icon::Search, ICON_SIZE, p.muted);
                let placeholder = theme::placeholder(self.launcher.effective_mode_name());
                let mut text = self.launcher.input().to_owned();
                let response = ui.add(
                    TextEdit::singleline(&mut text)
                        .font(egui::TextStyle::Heading)
                        .frame(egui::Frame::NONE)
                        .hint_text(RichText::new(placeholder).color(p.muted))
                        .desired_width(f32::INFINITY),
                );
                // This, not key consumption, is what keeps focus here after Tab.
                response.request_focus();
                if response.secondary_clicked() {
                    // Opened at the start of the next frame, with the bar drawn.
                    self.wants_bar_menu = true;
                    ui.ctx().request_repaint();
                }
                if response.changed() {
                    self.launcher.set_input(&text);
                }
            });
        });

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
            self.launcher.select_mode(i);
        }
        if let Some(i) = clicked_row {
            self.launcher.select(i);
            return self.launcher.handle(Intent::Enter);
        }
        Vec::new()
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

/// Allocates a square of `size` and draws `icon` in it.
fn icon_slot(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    icon.paint(ui.painter(), rect.shrink(1.0), color);
}

/// A mode chip: icon, name and its keyword. Clickable.
fn mode_chip(
    ui: &mut egui::Ui,
    p: &Palette,
    name: &str,
    keyword: &str,
    active: bool,
) -> egui::Response {
    let (fill, stroke, text) = if active {
        (p.selected, p.accent, p.text)
    } else {
        (Color32::TRANSPARENT, p.border, p.muted)
    };
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
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(theme::chip_tip(keyword))
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
                    String::new()
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
        if !self.positioned
            && let Some(monitor) = ctx.input(|i| i.viewport().monitor_size)
        {
            // Open a bit above the middle so the list has room to grow.
            let at = placement::initial_position(monitor, WIDTH);
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(at));
            self.positioned = true;
        }
        if self.services.shown.swap(false, Ordering::Relaxed) {
            // Every show starts empty, whatever happened before the last hide.
            self.launcher.reset();
            self.pending.clear();
        }
        if self.services.results_changed.swap(false, Ordering::Relaxed) {
            self.launcher.refresh();
        }
        let focused = ctx.input(|i| i.viewport().focused) == Some(true);
        if self.was_focused && !focused {
            self.hide();
        }
        self.was_focused = focused;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let keys = read_intents(&ctx);
        self.pending.extend(keys.intents);
        if keys.after_text {
            // Let the text box take this frame's typing first; run the keys next frame.
            ctx.request_repaint();
        } else {
            for intent in std::mem::take(&mut self.pending) {
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
        match &self.picture {
            Some(picture) => {
                let uv = background::cover_uv(picture.size(), window.size());
                ui.painter().image(picture.id(), window, uv, Color32::WHITE);
            }
            None => {
                ui.painter().rect_filled(window, 0, self.palette.background);
            }
        }

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
        // Right-click on the bar: the Theme and Background menu, right here.
        if (mover.secondary_clicked() || std::mem::take(&mut self.wants_bar_menu))
            && let Some(tray) = &self.services.tray
        {
            tray.show_bar_menu(self.services.window.raw());
            // The pop-up takes focus while open; give it back to the search box.
            self.services.window.show_and_focus();
        }

        let content = egui::Frame::NONE
            .inner_margin(egui::Margin::same(GAP))
            .show(ui, |ui| self.draw(ui));
        self.fit_height(&ctx, content.response.rect.height());
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
pub fn read_intents(ctx: &egui::Context) -> FrameKeys {
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
            } => match intent_for(*key, *modifiers) {
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
        let ctx = egui::Context::default();
        let raw = RawInput {
            events,
            ..Default::default()
        };
        let mut out = FrameKeys::default();
        ctx.run_ui(raw, |ui| out = read_intents(ui.ctx()))
            .textures_delta
            .clear();
        out
    }

    fn intents_for(events: Vec<Event>) -> Vec<Intent> {
        keys_for(events).intents
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
            read_intents(ui.ctx());
            still_there = ui.ctx().input(|i| i.key_pressed(Key::Tab));
        });
        output.textures_delta.clear();
        assert!(!still_there);
    }
}
