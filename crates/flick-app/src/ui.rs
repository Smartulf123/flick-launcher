//! The search bar window: draws the launcher state and feeds it key presses.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Key, Modifiers, RichText, TextEdit, ViewportCommand};
use flick_core::launcher::{Effect, Intent, Launcher};
use flick_core::mode::ResultItem;
use global_hotkey::GlobalHotKeyManager;
use tray_icon::TrayIcon;

use crate::actions::run_action;
use crate::icons::Icon;
use crate::theme;
use crate::window::WindowHandle;

pub const WIDTH: f32 = 640.0;
/// Height before the first frame measures the real content.
pub const START_HEIGHT: f32 = 120.0;

/// The parts of the app that run while the bar is hidden.
pub struct Background {
    /// Set by the indexing thread when a new file index is ready.
    pub index_updated: Arc<AtomicBool>,
    /// Set by the tray's Quit item.
    pub quit: Arc<AtomicBool>,
    /// Set by the hotkey handler each time it shows the bar.
    pub shown: Arc<AtomicBool>,
    pub window: WindowHandle,
    /// Keeps the hotkey registered; `None` if registration failed.
    pub hotkey: Option<GlobalHotKeyManager>,
    /// Keeps the tray icon alive; `None` if it could not be created.
    pub tray: Option<TrayIcon>,
}

#[derive(Debug, PartialEq, Eq)]
enum Visibility {
    Shown,
    Hidden,
}

pub struct FlickApp {
    launcher: Launcher,
    background: Background,
    /// Keys held back one frame because text was typed before them.
    pending: Vec<Intent>,
    height: f32,
    was_focused: bool,
}

impl FlickApp {
    pub fn new(launcher: Launcher, background: Background) -> Self {
        Self {
            launcher,
            background,
            pending: Vec::new(),
            height: START_HEIGHT,
            was_focused: false,
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
        self.background.window.hide();
    }

    /// Draws chips, search box, results and the hint line. Returns the
    /// effects of a mouse click on a result.
    fn draw(&mut self, ui: &mut egui::Ui) -> Vec<Effect> {
        let mut clicked_mode = None;
        let mut clicked_row = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let active = self.launcher.effective_mode();
            for (i, (name, keyword)) in self.launcher.modes().into_iter().enumerate() {
                if mode_chip(ui, name, keyword, i == active).clicked() {
                    clicked_mode = Some(i);
                }
            }
        });
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            icon_slot(ui, Icon::Search, 20.0, theme::MUTED);
            let placeholder = theme::placeholder(self.launcher.effective_mode_name());
            let mut text = self.launcher.input().to_owned();
            let response = ui.add(
                TextEdit::singleline(&mut text)
                    .font(egui::TextStyle::Heading)
                    .frame(egui::Frame::NONE)
                    .hint_text(RichText::new(placeholder).color(theme::MUTED))
                    .desired_width(f32::INFINITY),
            );
            // This, not key consumption, is what keeps focus here after Tab.
            response.request_focus();
            if response.changed() {
                self.launcher.set_input(&text);
            }
        });

        let selected = self.launcher.selected();
        // Moving the mouse over a row selects it; a resting pointer does not
        // fight the arrow keys.
        let pointer_moved = ui.input(|i| i.pointer.is_moving());
        let mut hovered_row = None;
        if !self.launcher.results().is_empty() {
            ui.add_space(8.0);
        }
        for (i, item) in self.launcher.results().iter().enumerate() {
            let response = result_row(ui, item, i == selected);
            if response.clicked() {
                clicked_row = Some(i);
            } else if pointer_moved && response.hovered() && i != selected {
                hovered_row = Some(i);
            }
        }

        ui.add_space(8.0);
        let line_y = ui.cursor().top();
        ui.painter().hline(
            ui.max_rect().x_range(),
            line_y,
            egui::Stroke::new(1.0, theme::BORDER),
        );
        ui.add_space(6.0);
        let typing = !self.launcher.input().trim().is_empty();
        ui.label(
            RichText::new(theme::footer(self.launcher.results().len(), typing))
                .small()
                .color(theme::MUTED),
        );

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
    fn fit_height(&mut self, ctx: &egui::Context, content_height: f32) {
        let height = content_height.ceil();
        if (height - self.height).abs() >= 1.0 {
            self.height = height;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(WIDTH, height)));
        }
    }
}

/// Allocates a square of `size` and draws `icon` in it.
fn icon_slot(ui: &mut egui::Ui, icon: Icon, size: f32, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    icon.paint(ui.painter(), rect.shrink(1.0), color);
}

/// A mode chip: icon, name and its keyword. Clickable.
fn mode_chip(ui: &mut egui::Ui, name: &str, keyword: &str, active: bool) -> egui::Response {
    let (fill, stroke, text) = if active {
        (theme::SELECTED, theme::ACCENT, theme::TEXT)
    } else {
        (egui::Color32::TRANSPARENT, theme::BORDER, theme::MUTED)
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
                ui.label(RichText::new(keyword).size(12.0).color(theme::MUTED));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One result: icon, title and a one-line path, cut short instead of wrapping.
fn result_row(ui: &mut egui::Ui, item: &ResultItem, selected: bool) -> egui::Response {
    let fill = if selected {
        theme::SELECTED
    } else {
        egui::Color32::TRANSPARENT
    };
    let icon_color = if selected {
        theme::ACCENT
    } else {
        theme::MUTED
    };
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icon_slot(ui, Icon::for_kind(item.kind), 18.0, icon_color);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add(
                        egui::Label::new(RichText::new(&item.title).color(theme::TEXT)).truncate(),
                    );
                    if !item.subtitle.is_empty() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&item.subtitle).small().color(theme::MUTED),
                            )
                            .truncate(),
                        );
                    }
                });
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl eframe::App for FlickApp {
    /// Runs even while hidden: picks up a finished index and hides the bar
    /// when the user clicks into another window.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.background.quit.load(Ordering::Relaxed) {
            log::info!("quit from tray");
            // Remove the tray icon now so Windows does not leave a ghost icon.
            self.background.tray = None;
            self.background.hotkey = None;
            std::process::exit(0);
        }
        if self.background.shown.swap(false, Ordering::Relaxed) {
            // Every show starts empty, whatever happened before the last hide.
            self.launcher.reset();
            self.pending.clear();
        }
        if self.background.index_updated.swap(false, Ordering::Relaxed) {
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
        let card = egui::Frame::NONE
            .fill(theme::BACKGROUND)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| self.draw(ui));
        self.fit_height(&ctx, card.response.rect.height());
        for effect in card.inner {
            if self.apply(effect) == Visibility::Hidden {
                return;
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
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
