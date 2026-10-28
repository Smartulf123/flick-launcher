//! The "+" list on the chip row: every engine, grouped or filtered by what
//! is typed, to put on the bar; and the small form for adding your own.

use eframe::egui::style::ScrollStyle;
use eframe::egui::{self, Align2, FontId, RichText, ScrollArea, Sense, TextEdit, vec2};
use flick_core::catalog::{self, CATALOG, CustomEngine, GROUPS};
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
        /// Added by the person, so it can be deleted.
        custom: bool,
    },
    AddOwn,
}

/// The list for `filter`: with no filter, every group under its heading,
/// then the person's own engines; with a filter, the engines whose name
/// contains it. "Add your own" always comes last.
pub fn rows(config: &Config, filter: &str) -> Vec<Row> {
    let chips = config.chips();
    let filter = filter.trim().to_lowercase();
    let matches = |name: &str| filter.is_empty() || name.to_lowercase().contains(&filter);
    let engine = |id: &str, name: &str, custom: bool| Row::Engine {
        id: id.to_owned(),
        name: name.to_owned(),
        on_bar: chips.iter().any(|c| c == id),
        custom,
    };

    let mut out = Vec::new();
    if filter.is_empty() {
        for group in GROUPS {
            out.push(Row::Heading(group.label()));
            out.extend(
                CATALOG
                    .iter()
                    .filter(|e| e.group == group)
                    .map(|e| engine(e.id, e.name, false)),
            );
        }
        if !config.custom_engines.is_empty() {
            out.push(Row::Heading("Your own"));
        }
    } else {
        out.extend(
            CATALOG
                .iter()
                .filter(|e| matches(e.name))
                .map(|e| engine(e.id, e.name, false)),
        );
    }
    out.extend(
        config
            .custom_engines
            .iter()
            .filter(|c| matches(&c.name))
            .map(|c| engine(&c.id(), &c.name, true)),
    );
    out.push(Row::AddOwn);
    out
}

/// Whether the bar already holds the most engines it can.
pub fn is_full(config: &Config) -> bool {
    config.chips().len() >= MAX_ENGINES
}

/// The rows the arrow keys can land on: engines that can still be added,
/// and "Add your own".
pub fn selectable(rows: &[Row], full: bool) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| match row {
            Row::Engine { on_bar, .. } => !on_bar && !full,
            Row::AddOwn => true,
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
    AddCustom(CustomEngine),
    DeleteCustom(String),
    Close,
}

/// The "+" list while it is open.
#[derive(Debug, Default)]
pub struct Picker {
    /// What is typed in the search box, used to filter the list.
    pub filter: String,
    /// Position in `selectable()`.
    pub selected: usize,
    /// The "Add your own" form, while it is open.
    pub form: Option<Form>,
    /// The selection moved by key; scroll it into view once. (Doing it
    /// every frame would undo every turn of the mouse wheel.)
    reveal: bool,
}

/// The fields of the "Add your own" form.
#[derive(Debug, Default)]
pub struct Form {
    pub name: String,
    pub url: String,
    /// Why the last Add was refused, in plain words.
    pub error: Option<String>,
    /// Put the cursor in the name field on the next frame.
    focus_name: bool,
}

impl Picker {
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_owned();
        self.selected = 0;
        self.reveal = true;
    }

    /// A key from the search bar while the picker is open.
    pub fn handle(&mut self, intent: Intent, config: &Config) -> Option<PickerAction> {
        if self.form.is_some() {
            match intent {
                Intent::Escape => self.form = None,
                Intent::Enter => return self.submit(config),
                _ => {}
            }
            return None;
        }
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
                let row = picks.get(self.selected).map(|&i| rows[i].clone())?;
                return self.choose(&row);
            }
            Intent::Escape => return Some(PickerAction::Close),
            Intent::Tab | Intent::ShiftTab => {}
        }
        None
    }

    fn choose(&mut self, row: &Row) -> Option<PickerAction> {
        match row {
            Row::Engine { id, .. } => Some(PickerAction::Add(id.clone())),
            Row::AddOwn => {
                self.form = Some(Form {
                    focus_name: true,
                    ..Form::default()
                });
                None
            }
            Row::Heading(_) => None,
        }
    }

    /// Checks the form; a good engine closes it, a refused one shows why.
    fn submit(&mut self, config: &Config) -> Option<PickerAction> {
        let form = self.form.as_mut()?;
        match catalog::check_custom(&form.name, &form.url, &config.custom_engines) {
            Ok(engine) => {
                self.form = None;
                Some(PickerAction::AddCustom(engine))
            }
            Err(e) => {
                form.error = Some(e.to_string());
                None
            }
        }
    }

    /// Whether the form is open (its fields take Tab and the focus).
    pub fn form_open(&self) -> bool {
        self.form.is_some()
    }

    /// Draws the list or the form in the results card, the list as tall
    /// as `height` (the card's height with a full page of results).
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        config: &Config,
        logos: &mut Favicons,
        height: f32,
    ) -> Option<PickerAction> {
        if self.form.is_some() {
            return self.draw_form(ui, p, config);
        }
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
                    format!("{MAX_ENGINES} of {MAX_ENGINES}: remove an engine first (hover a chip, then \u{d7})"),
                    p.warning,
                )
            } else {
                (
                    format!("Add an engine \u{b7} {} of {MAX_ENGINES}", config.chips().len()),
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
        let mut open_form = false;
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
                        Row::AddOwn => {
                            ui.add_space(6.0);
                            let response =
                                text_row(ui, p, None, "+ Add your own\u{2026}", p.accent, selected);
                            if reveal {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                open_form = true;
                            }
                        }
                    }
                }
            });
        if open_form {
            self.choose(&Row::AddOwn);
        }
        action
    }

    fn draw_form(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        config: &Config,
    ) -> Option<PickerAction> {
        let mut add = false;
        let form = self.form.as_mut()?;
        ui.label(
            RichText::new("Add your own engine")
                .size(13.0)
                .color(p.text),
        );
        ui.add_space(4.0);
        let name = field(ui, p, "Name", &mut form.name, "Thingiverse");
        if std::mem::take(&mut form.focus_name) {
            name.request_focus();
        }
        field(
            ui,
            p,
            "Search address \u{2014} {query} is where your text goes",
            &mut form.url,
            "https://www.example.com/search?q={query}",
        );
        if let Some(error) = &form.error {
            ui.label(RichText::new(error).size(12.0).color(p.warning));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Tab to move \u{b7} Esc to go back")
                    .size(12.0)
                    .color(p.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let button =
                    egui::Button::new(RichText::new("Add").color(p.background)).fill(p.accent);
                if ui
                    .add(button)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    add = true;
                }
            });
        });
        if add { self.submit(config) } else { None }
    }
}

/// A labelled one-line text field.
fn field(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    text: &mut String,
    hint: &str,
) -> egui::Response {
    ui.label(RichText::new(label).size(11.0).color(p.muted));
    let response = ui.add(
        TextEdit::singleline(text)
            .hint_text(RichText::new(hint).color(p.muted))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(2.0);
    response
}

/// A full-width clickable row of text, after a small mark if given.
fn text_row(
    ui: &mut egui::Ui,
    p: &Palette,
    mark: Option<&Mark>,
    text: &str,
    color: egui::Color32,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 4, p.selected);
    }
    let mut left = rect.left() + 8.0;
    if let Some(mark) = mark {
        let icon =
            egui::Rect::from_min_size(egui::pos2(left, rect.center().y - 8.0), vec2(16.0, 16.0));
        mark.paint(ui.painter(), icon, color);
        left = icon.right() + 8.0;
    }
    ui.painter().text(
        egui::pos2(left, rect.center().y),
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
        Row::AddOwn => "+ Add your own\u{2026}".into(),
    }
}

/// An engine in the list. Returns Add when clicked (if it can be added) and
/// DeleteCustom when a custom engine's delete mark is clicked.
fn engine_row(
    ui: &mut egui::Ui,
    p: &Palette,
    row: &Row,
    mark: &Mark,
    selected: bool,
    reveal: bool,
    full: bool,
) -> Option<PickerAction> {
    let Row::Engine {
        id, on_bar, custom, ..
    } = row
    else {
        return None;
    };
    let addable = !on_bar && !full;
    let color = if addable { p.text } else { p.muted };
    let response = text_row(ui, p, Some(mark), &row_label(row), color, selected);
    if reveal {
        response.scroll_to_me(None);
    }
    // On the bar already: a check on the right, left of any delete mark.
    if *on_bar {
        let room = if *custom { 40.0 } else { 16.0 };
        let c = response.rect.right_center() - vec2(room, 0.0);
        let stroke = egui::Stroke::new(1.8, p.accent);
        ui.painter()
            .line_segment([c + vec2(-5.0, 0.0), c + vec2(-1.5, 3.5)], stroke);
        ui.painter()
            .line_segment([c + vec2(-1.5, 3.5), c + vec2(5.0, -4.0)], stroke);
    }
    let delete = custom.then(|| {
        let r = response.rect;
        egui::Rect::from_center_size(r.right_center() - vec2(16.0, 0.0), vec2(18.0, 18.0))
    });
    if let Some(mark) = delete {
        let hot = response.hover_pos().is_some_and(|pos| mark.contains(pos));
        ui.painter().text(
            mark.center(),
            Align2::CENTER_CENTER,
            "\u{d7}",
            FontId::proportional(15.0),
            if hot { p.warning } else { p.muted },
        );
    }
    if !response.clicked() {
        return None;
    }
    let at = response.interact_pointer_pos();
    match delete {
        Some(mark) if at.is_some_and(|pos| mark.contains(pos)) => {
            Some(PickerAction::DeleteCustom(id.clone()))
        }
        _ if addable => Some(PickerAction::Add(id.clone())),
        _ => None,
    }
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
                Row::AddOwn => "+".into(),
            })
            .collect()
    }

    fn crates() -> CustomEngine {
        CustomEngine {
            name: "Crates".into(),
            url: "https://crates.io/search?q={query}".into(),
        }
    }

    #[test]
    fn without_a_filter_every_group_is_listed_then_add_your_own() {
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
        assert_eq!(list.len(), 8 + CATALOG.len() + 1);
        assert_eq!(list.last().unwrap(), "+");
    }

    #[test]
    fn own_engines_get_their_own_heading() {
        let mut c = Config::default();
        c.remove_engine("wikipedia").unwrap();
        c.add_custom(crates());
        let list = names(&rows(&c, ""));
        assert_eq!(
            list[list.len() - 3..],
            ["# Your own", "Crates \u{2713}", "+"]
        );
    }

    #[test]
    fn every_row_label_is_in_the_bar_font() {
        // A character the font lacks is drawn as an empty box.
        let mut c = Config::default();
        c.remove_engine("wikipedia").unwrap();
        c.add_custom(crates());
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
    fn a_filter_matches_names() {
        let c = Config::default();
        assert_eq!(names(&rows(&c, "git")), ["GitHub", "+"]);
        assert_eq!(names(&rows(&c, "gh")), ["+"]);
        assert_eq!(names(&rows(&c, " YOU ")), ["YouTube \u{2713}", "+"]);
        assert_eq!(names(&rows(&c, "zzz")), ["+"]);
    }

    #[test]
    fn only_addable_engines_and_add_your_own_are_selectable() {
        let c = Config::default();
        let list = rows(&c, "");
        let picks = selectable(&list, false);
        assert_eq!(names(&[list[picks[0]].clone()]), ["DuckDuckGo"]);
        assert_eq!(selectable(&list, true), [list.len() - 1]);
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
    fn escape_closes_the_list_and_leaves_the_form() {
        let c = Config::default();
        let mut picker = Picker::default();
        picker.set_filter("zzz");
        assert_eq!(picker.handle(Intent::Enter, &c), None);
        assert!(picker.form_open());
        assert_eq!(picker.handle(Intent::Escape, &c), None);
        assert!(!picker.form_open());
        assert_eq!(picker.handle(Intent::Escape, &c), Some(PickerAction::Close));
    }

    #[test]
    fn the_form_explains_refusals_and_adds_good_engines() {
        let c = Config::default();
        let mut picker = Picker {
            form: Some(Form::default()),
            ..Picker::default()
        };
        assert_eq!(picker.handle(Intent::Enter, &c), None);
        assert_eq!(
            picker.form.as_ref().unwrap().error.as_deref(),
            Some("Give it a name")
        );
        let form = picker.form.as_mut().unwrap();
        form.name = "Crates".into();
        form.url = "https://crates.io/search?q={query}".into();
        assert_eq!(
            picker.handle(Intent::Enter, &c),
            Some(PickerAction::AddCustom(crates()))
        );
        assert!(!picker.form_open());
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
