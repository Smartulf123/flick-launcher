//! The search bar window: draws the launcher state and feeds it key presses.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Key, Modifiers, RichText, TextEdit, ViewportCommand};
use flick_core::launcher::{Effect, Intent, Launcher};
use global_hotkey::GlobalHotKeyManager;

use crate::actions::run_action;
use crate::window::WindowHandle;

pub const WIDTH: f32 = 640.0;
const BAR_HEIGHT: f32 = 64.0;
const ROW_HEIGHT: f32 = 40.0;

pub struct FlickApp {
    launcher: Launcher,
    /// Set by the indexing thread when a new file index is ready.
    index_updated: Arc<AtomicBool>,
    window: WindowHandle,
    /// Keeps the hotkey registered; `None` if registration failed.
    _hotkey: Option<GlobalHotKeyManager>,
    height: f32,
    was_focused: bool,
}

impl FlickApp {
    pub fn new(
        launcher: Launcher,
        index_updated: Arc<AtomicBool>,
        window: WindowHandle,
        hotkey: Option<GlobalHotKeyManager>,
    ) -> Self {
        Self {
            launcher,
            index_updated,
            window,
            _hotkey: hotkey,
            height: BAR_HEIGHT,
            was_focused: false,
        }
    }

    fn apply(&mut self, effect: Effect) {
        match effect {
            Effect::Run(action) => {
                if let Err(e) = run_action(&action) {
                    eprintln!("could not open {action:?}: {e}");
                }
            }
            Effect::Hide => self.hide(),
        }
    }

    fn hide(&mut self) {
        self.launcher.reset();
        self.was_focused = false;
        self.window.hide();
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.launcher.effective_mode_name()).strong());
            let mut text = self.launcher.input().to_owned();
            let response = ui.add(
                TextEdit::singleline(&mut text)
                    .font(egui::TextStyle::Heading)
                    .frame(egui::Frame::NONE)
                    .hint_text("Search...")
                    .desired_width(f32::INFINITY),
            );
            response.request_focus();
            if response.changed() {
                self.launcher.set_input(&text);
            }
        });

        let selected = self.launcher.selected();
        for (i, item) in self.launcher.results().iter().enumerate() {
            let fill = if i == selected {
                ui.visuals().selection.bg_fill
            } else {
                egui::Color32::TRANSPARENT
            };
            egui::Frame::NONE
                .fill(fill)
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&item.title).strong());
                    ui.label(RichText::new(&item.subtitle).small().weak());
                });
        }
    }

    fn fit_height(&mut self, ctx: &egui::Context) {
        let height = BAR_HEIGHT + ROW_HEIGHT * self.launcher.results().len() as f32;
        if height != self.height {
            self.height = height;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(WIDTH, height)));
        }
    }
}

impl eframe::App for FlickApp {
    /// Runs even while hidden: picks up a finished index and hides the bar
    /// when the user clicks into another window.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.index_updated.swap(false, Ordering::Relaxed) {
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
        for intent in read_intents(&ctx) {
            for effect in self.launcher.handle(intent) {
                self.apply(effect);
            }
        }
        egui::Frame::central_panel(ui.style()).show(ui, |ui| self.draw(ui));
        self.fit_height(&ctx);
    }
}

/// Takes the launcher's keys out of the input queue before any widget sees
/// them, so the text box never loses focus to Tab. Shift+Tab goes first
/// because a plain-Tab match also accepts Shift.
pub fn read_intents(ctx: &egui::Context) -> Vec<Intent> {
    const KEYS: [(Modifiers, Key, Intent); 6] = [
        (Modifiers::SHIFT, Key::Tab, Intent::ShiftTab),
        (Modifiers::NONE, Key::Tab, Intent::Tab),
        (Modifiers::NONE, Key::ArrowUp, Intent::Up),
        (Modifiers::NONE, Key::ArrowDown, Intent::Down),
        (Modifiers::NONE, Key::Enter, Intent::Enter),
        (Modifiers::NONE, Key::Escape, Intent::Escape),
    ];
    ctx.input_mut(|input| {
        let mut intents = Vec::new();
        for (modifiers, key, intent) in KEYS {
            while input.consume_key(modifiers, key) {
                intents.push(intent);
            }
        }
        intents
    })
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

    fn intents_for(events: Vec<Event>) -> Vec<Intent> {
        let ctx = egui::Context::default();
        let raw = RawInput {
            events,
            ..Default::default()
        };
        let mut out = Vec::new();
        ctx.run_ui(raw, |ui| out = read_intents(ui.ctx()))
            .textures_delta
            .clear();
        out
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
            read_intents(ui.ctx());
            still_there = ui.ctx().input(|i| i.key_pressed(Key::Tab));
        });
        output.textures_delta.clear();
        assert!(!still_there);
    }
}
