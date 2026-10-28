//! The "+" list on the chip row: every engine, grouped or filtered by what
//! is typed, to put on the bar.

use eframe::egui::style::ScrollStyle;
use eframe::egui::{self, Align2, FontId, RichText, ScrollArea, Sense, vec2};
use flick_core::catalog::{CATALOG, GROUPS};
use flick_core::config::{Config, MAX_ENGINES};
use flick_core::launcher::Intent;

use crate::favicons::{Favicons, Mark};
use crate::theme::Palette;

const ROW_HEIGHT: f32 = 24.0;

/// One line of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Heading(&'static str),
    Engine {
        id: String,
        name: String,
        /// Already one of the bar's chips.
        on_bar: bool,
    },
}

/// The list for `filter`: with no filter, every group under its heading;
/// with a filter, the engines whose name contains it.
pub fn rows(config: &Config, filter: &str) -> Vec<Row> {
    let chips = config.chips();
    let filter = filter.trim().to_lowercase();
    let engine = |id: &str, name: &str| Row::Engine {
        id: id.to_owned(),
        name: name.to_owned(),
        on_bar: chips.iter().any(|c| c == id),
    };

    let mut out = Vec::new();
    if filter.is_empty() {
        for group in GROUPS {
            out.push(Row::Heading(group.label()));
            out.extend(
                CATALOG
                    .iter()
                    .filter(|e| e.group == group)
                    .map(|e| engine(e.id, e.name)),
            );
        }
    } else {
        out.extend(
            CATALOG
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&filter))
                .map(|e| engine(e.id, e.name)),
        );
    }
    out
}

/// Whether the bar already holds the most engines it can.
pub fn is_full(config: &Config) -> bool {
    config.chips().len() >= MAX_ENGINES
}

/// The rows the arrow keys can land on: engines that can still be added.
pub fn selectable(rows: &[Row], full: bool) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| match row {
            Row::Engine { on_bar, .. } => !on_bar && !full,
            Row::Heading(_) => false,
        })
        .map(|(i, _)| i)
        .collect()
}

/// What the picker wants done to the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerAction {
    /// Put this engine on the bar.
    Add(String),
    /// An engine was picked up to be carried onto the bar.
    Lift {
        id: String,
        name: String,
    },
    Close,
}

/// Rows that can be picked up and carried onto the bar: engines that are
/// not on it yet (also when it is full, to swap one in).
pub fn liftable(row: &Row) -> bool {
    matches!(row, Row::Engine { on_bar: false, .. })
}

/// The "+" list while it is open.
#[derive(Debug, Default)]
pub struct Picker {
    /// What is typed in the search box, used to filter the list.
    pub filter: String,
    /// Position in `selectable()`.
    pub selected: usize,
    /// The selection moved by key; scroll it into view once. (Doing it
    /// every frame would undo every turn of the mouse wheel.)
    reveal: bool,
}

impl Picker {
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_owned();
        self.selected = 0;
        self.reveal = true;
    }

    /// A key from the search bar while the picker is open.
    pub fn handle(&mut self, intent: Intent, config: &Config) -> Option<PickerAction> {
        let rows = rows(config, &self.filter);
        let picks = selectable(&rows, is_full(config));
        match intent {
            Intent::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.reveal = true;
            }
            Intent::Down => {
                self.selected = (self.selected + 1).min(picks.len().saturating_sub(1));
                self.reveal = true;
            }
            Intent::Enter => {
                return match picks.get(self.selected).map(|&i| &rows[i]) {
                    Some(Row::Engine { id, .. }) => Some(PickerAction::Add(id.clone())),
                    _ => None,
                };
            }
            Intent::Escape => return Some(PickerAction::Close),
            Intent::Tab | Intent::ShiftTab => {}
        }
        None
    }

    /// Draws the list in the results card, as tall as `height` (the card's
    /// height with a full page of results).
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        config: &Config,
        logos: &mut Favicons,
        height: f32,
    ) -> Option<PickerAction> {
        let rows = rows(config, &self.filter);
        let full = is_full(config);
        let picks = selectable(&rows, full);
        self.selected = self.selected.min(picks.len().saturating_sub(1));
        let selected_row = picks.get(self.selected).copied();
        let reveal = std::mem::take(&mut self.reveal);
        let top = ui.cursor().top();

        ui.horizontal(|ui| {
            let (text, color) = if full {
                (
                    format!(
                        "{MAX_ENGINES} of {MAX_ENGINES}: drag an engine onto a chip to swap it in"
                    ),
                    p.warning,
                )
            } else {
                (
                    format!(
                        "Add an engine \u{b7} {} of {MAX_ENGINES}",
                        config.chips().len()
                    ),
                    p.muted,
                )
            };
            ui.label(RichText::new(text).size(12.0).color(color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Esc to close").size(12.0).color(p.muted));
            });
        });
        ui.add_space(4.0);

        let list_height = (height - (ui.cursor().top() - top)).max(ROW_HEIGHT * 4.0);
        // A bar that is always there and wide enough to grab.
        ui.spacing_mut().scroll = ScrollStyle {
            bar_width: 10.0,
            handle_min_length: 32.0,
            dormant_handle_opacity: 0.6,
            dormant_background_opacity: 0.25,
            ..ScrollStyle::solid()
        };
        let mut action = None;
        ScrollArea::vertical()
            .max_height(list_height)
            // Without this the list fits the window as it is now, and the
            // window (which grows to fit what is drawn) never grows.
            .min_scrolled_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, row) in rows.iter().enumerate() {
                    let selected = Some(i) == selected_row;
                    let reveal = selected && reveal;
                    match row {
                        Row::Heading(label) => {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(label.to_uppercase())
                                    .size(11.0)
                                    .color(p.muted),
                            );
                        }
                        Row::Engine { id, name, .. } => {
                            let mark = Mark::for_engine(logos, ui.ctx(), config, id, name);
                            if let Some(a) = engine_row(ui, p, row, &mark, selected, reveal, full) {
                                action = Some(a);
                            }
                        }
                    }
                }
            });
        action
    }
}

/// A full-width clickable row of text, after a small mark.
fn text_row(
    ui: &mut egui::Ui,
    p: &Palette,
    mark: &Mark,
    text: &str,
    color: egui::Color32,
    selected: bool,
    sense: Sense,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), sense);
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 4, p.selected);
    }
    let icon = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.center().y - 8.0),
        vec2(16.0, 16.0),
    );
    mark.paint(ui.painter(), icon, color);
    ui.painter().text(
        egui::pos2(icon.right() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        color,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The text drawn for a row.
fn row_label(row: &Row) -> String {
    match row {
        Row::Heading(label) => label.to_uppercase(),
        Row::Engine { name, .. } => name.clone(),
    }
}

/// An engine in the list. Returns Add when clicked (if it can be added) and
/// Lift when it is picked up to be carried onto the bar.
fn engine_row(
    ui: &mut egui::Ui,
    p: &Palette,
    row: &Row,
    mark: &Mark,
    selected: bool,
    reveal: bool,
    full: bool,
) -> Option<PickerAction> {
    let Row::Engine { id, on_bar, .. } = row else {
        return None;
    };
    let addable = !on_bar && !full;
    let color = if addable { p.text } else { p.muted };
    let sense = if liftable(row) {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let response = text_row(ui, p, mark, &row_label(row), color, selected, sense);
    if response.drag_started() {
        return Some(PickerAction::Lift {
            id: id.clone(),
            name: row_label(row),
        });
    }
    if reveal {
        response.scroll_to_me(None);
    }
    // On the bar already: a check on the right.
    if *on_bar {
        let c = response.rect.right_center() - vec2(16.0, 0.0);
        let stroke = egui::Stroke::new(1.8, p.accent);
        ui.painter()
            .line_segment([c + vec2(-5.0, 0.0), c + vec2(-1.5, 3.5)], stroke);
        ui.painter()
            .line_segment([c + vec2(-1.5, 3.5), c + vec2(5.0, -4.0)], stroke);
    }
    (response.clicked() && addable).then(|| PickerAction::Add(id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Row::Heading(h) => format!("# {h}"),
                Row::Engine {
                    name, on_bar: true, ..
                } => format!("{name} \u{2713}"),
                Row::Engine { name, .. } => name.clone(),
            })
            .collect()
    }

    #[test]
    fn without_a_filter_every_group_is_listed() {
        let list = names(&rows(&Config::default(), ""));
        assert_eq!(
            list[..5],
            [
                "# On this PC",
                "Files \u{2713}",
                "Apps \u{2713}",
                "# Web search",
                "Google \u{2713}"
            ]
        );
        assert_eq!(list.iter().filter(|n| n.starts_with('#')).count(), 8);
        assert_eq!(list.len(), 8 + CATALOG.len());
    }

    #[test]
    fn every_row_label_is_in_the_bar_font() {
        // A character the font lacks is drawn as an empty box.
        let c = Config::default();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.ctx().fonts_mut(|fonts| {
                for row in rows(&c, "") {
                    let label = row_label(&row);
                    let font = FontId::proportional(13.0);
                    assert!(fonts.has_glyphs(&font, &label), "{label:?}");
                }
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn engines_off_the_bar_can_be_picked_up_even_when_it_is_full() {
        let c = Config::default();
        let list = rows(&c, "");
        let lift = |name: &str| {
            let row = list.iter().find(|r| row_label(r) == name).unwrap();
            liftable(row)
        };
        assert!(lift("Reddit"));
        assert!(!lift("Google"));
        assert!(!liftable(&Row::Heading("AI")));
    }

    #[test]
    fn a_filter_matches_names() {
        let c = Config::default();
        assert_eq!(names(&rows(&c, "git")), ["GitHub"]);
        assert!(names(&rows(&c, "gh")).is_empty());
        assert_eq!(names(&rows(&c, " YOU ")), ["YouTube \u{2713}"]);
        assert!(names(&rows(&c, "zzz")).is_empty());
    }

    #[test]
    fn only_addable_engines_are_selectable() {
        let c = Config::default();
        let list = rows(&c, "");
        let picks = selectable(&list, false);
        assert_eq!(names(&[list[picks[0]].clone()]), ["DuckDuckGo"]);
        assert!(selectable(&list, true).is_empty());
    }

    #[test]
    fn enter_adds_the_selected_engine() {
        // Room for one more: the default five fill the bar.
        let mut c = Config::default();
        c.remove_engine("wikipedia").unwrap();
        let mut picker = Picker::default();
        assert_eq!(
            picker.handle(Intent::Enter, &c),
            Some(PickerAction::Add("duckduckgo".into()))
        );
        picker.handle(Intent::Down, &c);
        assert_eq!(
            picker.handle(Intent::Enter, &c),
            Some(PickerAction::Add("bing".into()))
        );
        picker.set_filter("hub");
        assert_eq!(
            picker.handle(Intent::Enter, &c),
            Some(PickerAction::Add("github".into()))
        );
    }

    #[test]
    fn enter_with_nothing_to_add_does_nothing_and_escape_closes() {
        let c = Config::default();
        let mut picker = Picker::default();
        picker.set_filter("zzz");
        assert_eq!(picker.handle(Intent::Enter, &c), None);
        assert_eq!(picker.handle(Intent::Escape, &c), Some(PickerAction::Close));
    }

    #[test]
    fn only_keys_scroll_the_selection_into_view() {
        // Revealing on every frame would fight the mouse wheel.
        let c = Config::default();
        let mut picker = Picker::default();
        assert!(!std::mem::take(&mut picker.reveal));
        picker.handle(Intent::Down, &c);
        assert!(std::mem::take(&mut picker.reveal));
        assert!(!picker.reveal);
        picker.handle(Intent::Up, &c);
        assert!(std::mem::take(&mut picker.reveal));
        picker.set_filter("g");
        assert!(picker.reveal);
    }

    #[test]
    fn the_list_is_full_height_even_in_a_short_window() {
        // The window grows to fit what is drawn, so the list must not
        // shrink to the room the window has before it grows.
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(640.0, 120.0),
            )),
            ..Default::default()
        };
        let mut used = 0.0;
        let mut output = ctx.run_ui(raw, |ui| {
            let top = ui.cursor().top();
            let mut logos = Favicons::with_fetch(
                std::env::temp_dir().join("flick-test-logos"),
                |_| anyhow::bail!("offline"),
                || {},
            );
            Picker::default().draw(
                ui,
                &crate::theme::KANAGAWA,
                &Config::default(),
                &mut logos,
                400.0,
            );
            used = ui.min_rect().bottom() - top;
        });
        output.textures_delta.clear();
        assert!(used >= 399.0, "{used}");
    }
}
