//! The background gallery: built-in pictures, your downloads, and a
//! Wallhaven search, drawn inside the bar's own window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText, Sense, TextureHandle, TextureOptions, vec2};
use flick_core::config::BackgroundSource;

use crate::background;
use crate::builtin_backgrounds::{BUILTINS, Builtin};
use crate::gallery_loader::{Done, Job};
use crate::theme::Palette;
use crate::wallhaven::{self, Categories, Wallpaper};

const COLUMNS: usize = 3;
const CARD_HEIGHT: f32 = 170.0;
/// Width that gallery thumbnails are scaled to (keeps their memory small).
const THUMB_WIDTH: u32 = 360;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Builtin,
    Downloaded,
    Online,
}

/// What the user chose in the gallery.
pub enum Pick {
    /// Use this right away.
    Source(BackgroundSource),
    /// Download this Wallhaven picture first.
    Download(Wallpaper),
}

pub enum GalleryAction {
    None,
    Back,
    Pick(Pick),
    Jobs(Vec<Job>),
}

pub struct Gallery {
    tab: Tab,
    query: String,
    categories: Categories,
    generation: u64,
    results: Vec<Wallpaper>,
    page: u32,
    last_page: u32,
    loading: bool,
    error: Option<String>,
    downloading: Option<String>,
    downloads_dir: PathBuf,
    online_thumbs: HashMap<String, TextureHandle>,
    builtin_thumbs: Option<Vec<Option<TextureHandle>>>,
    downloaded: Option<Vec<(PathBuf, Option<TextureHandle>)>>,
}

impl Gallery {
    pub fn new(downloads_dir: PathBuf) -> Self {
        Self {
            tab: Tab::Builtin,
            query: String::new(),
            categories: Categories::default(),
            generation: 0,
            results: Vec::new(),
            page: 0,
            last_page: 0,
            loading: false,
            error: None,
            downloading: None,
            downloads_dir,
            online_thumbs: HashMap::new(),
            builtin_thumbs: None,
            downloaded: None,
        }
    }

    pub fn results(&self) -> &[Wallpaper] {
        &self.results
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// A fresh search for the current text and categories; older replies
    /// still on their way will be ignored.
    pub fn start_search(&mut self) -> Job {
        self.generation += 1;
        self.results.clear();
        self.online_thumbs.clear();
        self.page = 0;
        self.last_page = 0;
        self.loading = true;
        self.error = None;
        Job::Search {
            generation: self.generation,
            url: wallhaven::search_url(&self.query, self.categories, 1),
        }
    }

    /// The next page, if there is one and nothing is loading.
    pub fn load_more(&mut self) -> Option<Job> {
        if self.loading || self.page == 0 || self.page >= self.last_page {
            return None;
        }
        self.loading = true;
        Some(Job::Search {
            generation: self.generation,
            url: wallhaven::search_url(&self.query, self.categories, self.page + 1),
        })
    }

    /// Takes in a finished search or thumbnail; returns thumbnail jobs for
    /// new results. Results from a replaced search are dropped.
    pub fn apply(&mut self, ctx: &egui::Context, done: Done) -> Vec<Job> {
        match done {
            Done::Search { generation, result } if generation == self.generation => {
                self.loading = false;
                match result {
                    Ok(page) => {
                        self.page = page.current_page;
                        self.last_page = page.last_page;
                        let jobs = page
                            .wallpapers
                            .iter()
                            .map(|w| Job::Thumb {
                                generation,
                                id: w.id.clone(),
                                url: w.thumb_url.clone(),
                            })
                            .collect();
                        self.results.extend(page.wallpapers);
                        jobs
                    }
                    Err(e) => {
                        log::info!("wallhaven search: {e:#}");
                        self.error = Some("Can't reach wallhaven.cc".into());
                        Vec::new()
                    }
                }
            }
            Done::Thumb {
                generation,
                id,
                image: Ok(image),
            } if generation == self.generation => {
                let texture =
                    ctx.load_texture(format!("wallhaven-{id}"), image, TextureOptions::LINEAR);
                self.online_thumbs.insert(id, texture);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A download finished; it now shows under Downloaded.
    pub fn note_downloaded(&mut self) {
        self.downloading = None;
        self.downloaded = None;
    }

    pub fn note_download_failed(&mut self) {
        self.downloading = None;
        self.error = Some("Couldn't download that picture".into());
    }

    /// Draws the gallery. `current` is the background in use (outlined).
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        current: &BackgroundSource,
    ) -> GalleryAction {
        let mut action = GalleryAction::None;
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                action = GalleryAction::Back;
            }
            ui.add_space(12.0);
            for (tab, label) in [
                (Tab::Builtin, "Built-in"),
                (Tab::Downloaded, "Downloaded"),
                (Tab::Online, "Search online"),
            ] {
                if ui.selectable_label(self.tab == tab, label).clicked() && self.tab != tab {
                    self.tab = tab;
                    if tab == Tab::Online && self.generation == 0 {
                        action = GalleryAction::Jobs(vec![self.start_search()]);
                    }
                }
            }
        });
        ui.add_space(8.0);

        if self.tab == Tab::Online
            && let Some(jobs) = self.search_bar(ui, p)
        {
            action = GalleryAction::Jobs(jobs);
        }
        if let Some(msg) = self
            .downloading
            .as_ref()
            .map(|_| "Downloading...")
            .or(self.error.as_deref())
        {
            ui.label(RichText::new(msg).color(p.muted));
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let picked = match self.tab {
                    Tab::Builtin => self.builtin_grid(ui, p, current),
                    Tab::Downloaded => self.downloaded_grid(ui, p, current),
                    Tab::Online => self.online_grid(ui, p, current),
                };
                if let Some(pick) = picked {
                    if let Pick::Download(w) = &pick {
                        self.downloading = Some(w.id.clone());
                        self.error = None;
                    }
                    action = GalleryAction::Pick(pick);
                }
                if self.tab == Tab::Online && self.page > 0 && self.page < self.last_page {
                    ui.add_space(8.0);
                    let label = if self.loading {
                        "Loading..."
                    } else {
                        "Load more"
                    };
                    if ui.button(label).clicked()
                        && let Some(job) = self.load_more()
                    {
                        action = GalleryAction::Jobs(vec![job]);
                    }
                } else if self.tab == Tab::Online && self.loading {
                    ui.label(RichText::new("Loading...").color(p.muted));
                }
            });
        action
    }

    fn search_bar(&mut self, ui: &mut egui::Ui, p: &Palette) -> Option<Vec<Job>> {
        let mut search = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text(RichText::new("Search wallhaven.cc, e.g. anime girl").color(p.muted))
                    .desired_width(360.0),
            );
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                search = true;
            }
            if ui.button("Search").clicked() {
                search = true;
            }
            ui.add_space(12.0);
            for (on, label) in [
                (&mut self.categories.general, "General"),
                (&mut self.categories.anime, "Anime"),
                (&mut self.categories.people, "People"),
            ] {
                if ui.toggle_value(on, label).changed() {
                    search = true;
                }
            }
        });
        ui.add_space(6.0);
        search.then(|| vec![self.start_search()])
    }

    fn builtin_grid(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        current: &BackgroundSource,
    ) -> Option<Pick> {
        let ctx = ui.ctx().clone();
        let thumbs = self.builtin_thumbs.get_or_insert_with(|| {
            BUILTINS
                .iter()
                .map(|b| thumbnail(&ctx, b.id, b.bytes))
                .collect()
        });
        let cards: Vec<(&Builtin, Option<&TextureHandle>)> = BUILTINS
            .iter()
            .zip(thumbs.iter().map(Option::as_ref))
            .collect();
        let mut picked = None;
        grid(ui, cards.len(), |ui, i, size| {
            let (b, texture) = cards[i];
            let is_current = *current == BackgroundSource::Builtin(b.id.into());
            let response = card(ui, p, size, texture, is_current, Some((b.title, b.credit)));
            if response.clicked() {
                picked = Some(Pick::Source(BackgroundSource::Builtin(b.id.into())));
            }
        });
        picked
    }

    fn downloaded_grid(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        current: &BackgroundSource,
    ) -> Option<Pick> {
        let ctx = ui.ctx().clone();
        let dir = self.downloads_dir.clone();
        let files = self.downloaded.get_or_insert_with(|| {
            list_downloaded(&dir)
                .into_iter()
                .map(|path| {
                    let texture = std::fs::read(&path)
                        .ok()
                        .and_then(|bytes| thumbnail(&ctx, &path.to_string_lossy(), &bytes));
                    (path, texture)
                })
                .collect()
        });
        if files.is_empty() {
            ui.label(
                RichText::new("Pictures you pick under Search online show up here.").color(p.muted),
            );
            return None;
        }
        let mut picked = None;
        grid(ui, files.len(), |ui, i, size| {
            let (path, texture) = &files[i];
            let is_current = *current == BackgroundSource::File(path.clone());
            if card(ui, p, size, texture.as_ref(), is_current, None).clicked() {
                picked = Some(Pick::Source(BackgroundSource::File(path.clone())));
            }
        });
        picked
    }

    fn online_grid(
        &mut self,
        ui: &mut egui::Ui,
        p: &Palette,
        current: &BackgroundSource,
    ) -> Option<Pick> {
        let mut picked = None;
        let results = &self.results;
        let thumbs = &self.online_thumbs;
        let dir = &self.downloads_dir;
        grid(ui, results.len(), |ui, i, size| {
            let w = &results[i];
            let saved = wallhaven::download_path(dir, &w.id).filter(|path| path.is_file());
            let is_current = saved
                .as_ref()
                .is_some_and(|path| *current == BackgroundSource::File(path.clone()));
            let response = card(ui, p, size, thumbs.get(&w.id), is_current, None);
            let response = response.on_hover_ui(|ui| {
                ui.label(format!("{}  from wallhaven.cc", w.resolution));
                ui.hyperlink_to("Open page", &w.page_url);
            });
            if response.clicked() {
                picked = Some(match saved {
                    Some(path) => Pick::Source(BackgroundSource::File(path)),
                    None => Pick::Download(w.clone()),
                });
            }
        });
        picked
    }
}

/// Lays out `count` cards in rows of `COLUMNS`, calling `draw(ui, index, size)`.
fn grid(ui: &mut egui::Ui, count: usize, mut draw: impl FnMut(&mut egui::Ui, usize, egui::Vec2)) {
    let gap = 10.0;
    let width = ((ui.available_width() - gap * (COLUMNS as f32 - 1.0)) / COLUMNS as f32).floor();
    let size = vec2(width, CARD_HEIGHT);
    for row in (0..count).collect::<Vec<_>>().chunks(COLUMNS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &i in row {
                draw(ui, i, size);
            }
        });
        ui.add_space(gap);
    }
}

/// One picture card: the picture cropped to fill, an accent outline when
/// it is the current background, and an optional title strip.
fn card(
    ui: &mut egui::Ui,
    p: &Palette,
    size: egui::Vec2,
    texture: Option<&TextureHandle>,
    is_current: bool,
    caption: Option<(&str, &str)>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    match texture {
        Some(t) => {
            painter.image(
                t.id(),
                rect,
                background::cover_uv(t.size(), rect.size()),
                Color32::WHITE,
            );
        }
        None => {
            painter.rect_filled(rect, 10, p.border);
        }
    }
    if let Some((title, credit)) = caption {
        let strip =
            egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 40.0), rect.max);
        painter.rect_filled(strip, 0, Color32::from_black_alpha(150));
        painter.text(
            strip.left_top() + vec2(10.0, 5.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(14.0),
            p.text,
        );
        painter.text(
            strip.left_top() + vec2(10.0, 22.0),
            egui::Align2::LEFT_TOP,
            credit,
            egui::FontId::proportional(11.0),
            p.muted,
        );
    }
    if response.hovered() {
        painter.rect_filled(rect, 10, Color32::from_white_alpha(18));
    }
    let outline = if is_current {
        egui::Stroke::new(3.0, p.accent)
    } else {
        egui::Stroke::new(1.0, p.border)
    };
    ui.painter()
        .rect_stroke(rect, 10, outline, egui::StrokeKind::Inside);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small texture for a gallery card.
fn thumbnail(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let picture = image::load_from_memory(bytes).ok()?;
    let height = (picture.height() as u64 * THUMB_WIDTH as u64 / picture.width().max(1) as u64)
        .max(1) as u32;
    let small = picture.thumbnail(THUMB_WIDTH, height).to_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [small.width() as usize, small.height() as usize],
        small.as_raw(),
    );
    Some(ctx.load_texture(format!("gallery-{name}"), image, TextureOptions::LINEAR))
}

/// Flick's downloaded pictures in `dir`, newest first.
pub fn list_downloaded(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("wallhaven-") && name.ends_with(".jpg")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    files.into_iter().map(|(_, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gallery_loader::Done;
    use crate::wallhaven::SearchPage;

    fn ctx() -> egui::Context {
        egui::Context::default()
    }

    fn tmp() -> PathBuf {
        std::env::temp_dir().join("flick-gallery-test-unused")
    }

    fn page(ids: &[&str], current: u32, last: u32) -> SearchPage {
        SearchPage {
            wallpapers: ids
                .iter()
                .map(|id| Wallpaper {
                    id: (*id).into(),
                    thumb_url: format!("thumb-{id}"),
                    full_url: format!("full-{id}"),
                    page_url: format!("page-{id}"),
                    resolution: "1920x1080".into(),
                })
                .collect(),
            current_page: current,
            last_page: last,
        }
    }

    fn ids(g: &Gallery) -> Vec<&str> {
        g.results().iter().map(|w| w.id.as_str()).collect()
    }

    #[test]
    fn new_search_ignores_late_results_from_the_old_one() {
        let mut g = Gallery::new(tmp());
        g.start_search();
        g.start_search(); // generation 2
        g.apply(
            &ctx(),
            Done::Search {
                generation: 1,
                result: Ok(page(&["old"], 1, 1)),
            },
        );
        assert!(g.results().is_empty());
        let jobs = g.apply(
            &ctx(),
            Done::Search {
                generation: 2,
                result: Ok(page(&["new"], 1, 3)),
            },
        );
        assert_eq!(ids(&g), ["new"]);
        assert!(matches!(
            jobs.as_slice(),
            [Job::Thumb { generation: 2, .. }]
        ));
    }

    #[test]
    fn offline_shows_cant_reach_and_stops_loading() {
        let mut g = Gallery::new(tmp());
        g.start_search();
        g.apply(
            &ctx(),
            Done::Search {
                generation: 1,
                result: Err(anyhow::anyhow!("dns")),
            },
        );
        assert_eq!(g.error(), Some("Can't reach wallhaven.cc"));
        assert!(!g.is_loading());
    }

    #[test]
    fn load_more_asks_for_the_next_page_and_appends() {
        let mut g = Gallery::new(tmp());
        g.start_search();
        g.apply(
            &ctx(),
            Done::Search {
                generation: 1,
                result: Ok(page(&["a"], 1, 3)),
            },
        );
        let Some(Job::Search { generation, url }) = g.load_more() else {
            panic!("expected a search")
        };
        assert!(url.ends_with("&page=2"), "{url}");
        assert!(
            g.load_more().is_none(),
            "no second request while one is loading"
        );
        g.apply(
            &ctx(),
            Done::Search {
                generation,
                result: Ok(page(&["b"], 2, 3)),
            },
        );
        assert_eq!(ids(&g), ["a", "b"]);
    }

    #[test]
    fn no_more_pages_after_the_last() {
        let mut g = Gallery::new(tmp());
        g.start_search();
        g.apply(
            &ctx(),
            Done::Search {
                generation: 1,
                result: Ok(page(&["a"], 3, 3)),
            },
        );
        assert!(g.load_more().is_none());
    }

    #[test]
    fn downloaded_lists_only_flick_downloads_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let older = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        for name in ["wallhaven-a.jpg", "wallhaven-b.jpg", "notes.txt"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::File::options()
            .write(true)
            .open(dir.path().join("wallhaven-a.jpg"))
            .unwrap()
            .set_modified(older)
            .unwrap();
        let names: Vec<_> = list_downloaded(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["wallhaven-b.jpg", "wallhaven-a.jpg"]);
    }

    #[test]
    fn missing_download_folder_lists_nothing() {
        assert!(list_downloaded(&tmp().join("nope")).is_empty());
    }
}
