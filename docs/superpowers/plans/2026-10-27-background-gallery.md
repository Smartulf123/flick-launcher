# Background Gallery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the wallpaper/Windows-pictures/theme options with six built-in public-domain backgrounds, a searchable Wallhaven gallery inside Flick, and "Choose your own picture", with Kanagawa as the only look.

**Architecture:** `flick-core` only learns a `builtin:<id>` background setting. `flick-app` gains `builtin_backgrounds.rs` (embedded pictures), `wallhaven.rs` (pure URL/JSON code), `gallery_loader.rs` (worker threads for search, thumbnails, downloads), and `gallery.rs` (gallery state and drawing). `ui.rs` switches between `View::Bar` and `View::Gallery` inside the same window.

**Tech Stack:** Rust 2024; egui/eframe 0.37 (glow); `ureq` 3 with native TLS and the Windows certificate store (already used for suggestions); `image` 0.25 (jpeg, png, webp); `serde_json`; `rfd`.

**Spec:** `docs/superpowers/specs/2026-10-27-background-gallery-design.md`

## Global Constraints

- Rust only. No new C/C++-building dependencies. Network only through `ureq` with `TlsProvider::NativeTls` and `RootCerts::PlatformVerifier`.
- Wallhaven requests always carry `purity=100`; nothing in the UI or config can change that.
- Built-in ids, in menu order: `great-wave`, `red-fuji`, `starry-night`, `impression-sunrise`, `earthrise`, `pillars-of-creation`. Default background setting: `builtin:great-wave`.
- Downloads go to `%APPDATA%\flick\backgrounds\wallhaven-<id>.jpg`, scaled to at most 1280 px wide (`background::MAX_WIDTH`).
- Gallery window size `960 x 640`; bar width stays `640`.
- Kanagawa colors stay exactly as today's `PRESETS[0]`.
- After every task: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass.

## Review Focus

1. No internet: the Online tab says "Can't reach wallhaven.cc" and Built-in/Downloaded still work. Test in Task 6.
2. Searching again (or typing fast) while a search is still loading: old results must never mix into new ones. Test in Task 6.
3. A download that fails or is not a picture: the background stays as it was, no crash, an error line in the gallery. Test in Task 5.
4. An old config (`background = "wallpaper"`, `theme = "dracula"`): Flick starts with the Great Wave and keeps every other setting. Test in Task 1.
5. Wallhaven ever returning a non-safe picture: it is dropped before display. Test in Task 4.

---

### Task 1: Background setting understands built-ins

**Files:** Modify `crates/flick-core/src/config.rs`.

**Interfaces:**
- Produces: `BackgroundSource::Builtin(String)` (variants now `Builtin(String)`, `None`, `File(PathBuf)`; `Wallpaper` removed); `BackgroundSource::from_setting` / `to_setting` round-trip `builtin:<id>`; `pub const DEFAULT_BUILTIN: &str = "great-wave";`; `Appearance` loses `theme`; default `background = "builtin:great-wave"`.

- [ ] **Step 1: Failing tests** (replace the old `background_setting_is_read` and `appearance_defaults`):

```rust
#[test] fn builtin_backgrounds_round_trip() {
    let s = BackgroundSource::from_setting("builtin:starry-night");
    assert_eq!(s, BackgroundSource::Builtin("starry-night".into()));
    assert_eq!(s.to_setting(), "builtin:starry-night");
}
#[test] fn old_wallpaper_setting_becomes_the_default_builtin() {
    assert_eq!(BackgroundSource::from_setting("wallpaper"), BackgroundSource::Builtin("great-wave".into()));
}
#[test] fn none_and_paths_still_work() {
    assert_eq!(BackgroundSource::from_setting("none"), BackgroundSource::None);
    assert_eq!(BackgroundSource::from_setting(r"C:\Pics\a.jpg"), BackgroundSource::File(r"C:\Pics\a.jpg".into()));
}
#[test] fn appearance_defaults() {
    let a = Config::default().appearance;
    assert_eq!((a.background.as_str(), a.tint), ("builtin:great-wave", 0.55));
}
#[test] fn old_config_with_theme_still_loads() {
    // file: [appearance]\ntheme = "dracula"\nbackground = "wallpaper"\n  + hotkey = "Ctrl+Space" at top
    // -> no error, hotkey "Ctrl+Space", background setting reads as Builtin("great-wave")
}
```

- [ ] **Step 2:** `cargo test -p flick-core --lib config`. Expected: compile errors (no `Builtin`).
- [ ] **Step 3:** Implement. `from_setting`: trim; `"" | "none"` → `None`; `"wallpaper"` → `Builtin(DEFAULT_BUILTIN)`; `strip_prefix("builtin:")` → `Builtin`; else `File`. Remove `Appearance::theme` (serde ignores the unknown key in old files).
- [ ] **Step 4:** `cargo test -p flick-core --lib`. Expected: all pass. App code will not compile until Task 3; that is expected, do not run workspace checks until then.
- [ ] **Step 5:** Commit `feat(core): builtin background setting, theme setting removed`.

### Task 2: Built-in pictures

**Files:** Create `crates/flick-app/assets/backgrounds/{great-wave,red-fuji,starry-night,impression-sunrise,earthrise,pillars-of-creation}.jpg`, `crates/flick-app/assets/backgrounds/CREDITS.md`, `crates/flick-app/src/builtin_backgrounds.rs`. Modify `crates/flick-app/src/background.rs`, `main.rs` (`mod`).

**Interfaces:**
- Consumes: `BackgroundSource` (Task 1).
- Produces: `pub struct Builtin { pub id: &'static str, pub title: &'static str, pub credit: &'static str, pub bytes: &'static [u8] }`; `pub const BUILTINS: [Builtin; 6]` (menu order from Global Constraints); `pub fn find(id: &str) -> Option<&'static Builtin>`. In `background.rs`: `pub fn decode(bytes: &[u8]) -> anyhow::Result<ColorImage>` (format guessed, scaled to `MAX_WIDTH`; `load(path)` reads the file and calls it), `pub fn load_source(source: &BackgroundSource) -> anyhow::Result<Option<ColorImage>>`. Remove `wallpaper_path`, `resolve`, `windows_pictures_dir`.

- [ ] **Step 1: Assets.** Take the 1280 px files from a local `background-candidates` folder (`great-wave`, `red-fuji`, `starry-night`, `impression-sunrise`, `earthrise`, and `pillars-hubble` saved as `pillars-of-creation`) and re-save each as JPEG quality 85 (System.Drawing, as used before). Expected: six files, each under 600 KB. `CREDITS.md`: one line per picture with title, artist/source, "Public domain, via Wikimedia Commons" and its Commons page URL from its `picks.tsv`.
- [ ] **Step 2: Failing tests** in `builtin_backgrounds.rs`:

```rust
#[test] fn six_builtins_in_menu_order() {
    let ids: Vec<_> = BUILTINS.iter().map(|b| b.id).collect();
    assert_eq!(ids, ["great-wave","red-fuji","starry-night","impression-sunrise","earthrise","pillars-of-creation"]);
}
#[test] fn every_builtin_decodes() {
    for b in &BUILTINS { let img = crate::background::decode(b.bytes).unwrap(); assert!(img.size[0] <= 1280 && img.size[1] > 0, "{}", b.id); }
}
#[test] fn find_by_id() { assert_eq!(find("earthrise").unwrap().title, "Earthrise"); assert!(find("nope").is_none()); }
```
and in `background.rs`:
```rust
#[test] fn builtin_source_loads() { assert!(load_source(&BackgroundSource::Builtin("great-wave".into())).unwrap().is_some()); }
#[test] fn unknown_builtin_is_an_error() { assert!(load_source(&BackgroundSource::Builtin("nope".into())).is_err()); }
#[test] fn none_loads_nothing() { assert!(load_source(&BackgroundSource::None).unwrap().is_none()); }
```
- [ ] **Step 3:** `cargo test -p flick-app builtin` fails (missing module). Implement with `include_bytes!("../assets/backgrounds/<id>.jpg")`. Titles: "The Great Wave off Kanagawa", "Red Fuji", "The Starry Night", "Impression, Sunrise", "Earthrise", "Pillars of Creation". Credits: "Katsushika Hokusai", "Katsushika Hokusai", "Vincent van Gogh", "Claude Monet", "NASA, Apollo 8", "NASA, ESA, Hubble Heritage Team".
- [ ] **Step 4:** Tests from Step 2 pass (run after Task 3 if the app crate does not yet compile; record that in the ledger).
- [ ] **Step 5:** Commit `feat(app): six built-in backgrounds`.

### Task 3: One look, simpler menu

**Files:** Modify `theme.rs`, `tray.rs`, `ui.rs`, `main.rs`.

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces: `theme::KANAGAWA: Palette` (fields `background, border, text, muted, accent, selected`; `id`/`name` removed); `PRESETS`, `palette()`, `is_known()` removed; `card_fill(&Palette, f32)` and `apply(ctx, &Palette)` unchanged. `tray::BackgroundChoice { Browse, ChooseFile, None }`; `TrayCommand { ToggleStartWithWindows, Background(BackgroundChoice), ReloadIndex, Quit }`; `Tray::show_background(&BackgroundSource)` ticks only **None**; bar menu (gear / right-click) = the three Background items at top level; tray menu = Start with Windows, Background ▸, Reload index, separator, Quit. `ui::load_picture(ctx, &BackgroundSource) -> Option<TextureHandle>` uses `background::load_source`.

- [ ] **Step 1: Tests.** In `theme.rs` keep `card_fill_uses_tint_as_alpha` against `KANAGAWA` (alpha 140 at 0.55; `card_fill(&KANAGAWA, 1.0) == KANAGAWA.background`); delete the preset tests.
- [ ] **Step 2:** Implement the menu and wiring. `set_background` handles `Browse` by calling `self.open_gallery(ctx)` (a stub that logs "gallery not yet" until Task 7), `ChooseFile` via `background::pick_picture(&paths::pictures_dir())`, `None`. Remove `set_theme` and the theme lookup in `main.rs`.
- [ ] **Step 3:** Workspace checks pass (all three commands). Expected: test count drops by the removed preset tests only.
- [ ] **Step 4:** Commit `feat(app): Kanagawa only; Background menu is Browse, Choose your own, None`.

### Task 4: Wallhaven search address and reply

**Files:** Create `crates/flick-app/src/wallhaven.rs`.

**Interfaces:**
- Produces: `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct Categories { pub general: bool, pub anime: bool, pub people: bool }` with `Default` = all true and `fn mask(self) -> String` ("110" etc.); `pub fn search_url(query: &str, categories: Categories, page: u32) -> String`; `#[derive(Clone, Debug, PartialEq)] pub struct Wallpaper { pub id: String, pub thumb_url: String, pub full_url: String, pub page_url: String, pub resolution: String }`; `pub struct SearchPage { pub wallpapers: Vec<Wallpaper>, pub current_page: u32, pub last_page: u32 }`; `pub fn parse_search(json: &str) -> anyhow::Result<SearchPage>`; `pub fn download_path(dir: &Path, id: &str) -> Option<PathBuf>`.

- [ ] **Step 1: Failing tests:**

```rust
#[test] fn query_searches_by_relevance_safe_only() {
    assert_eq!(search_url("anime girl", Categories { general: false, anime: true, people: false }, 1),
        "https://wallhaven.cc/api/v1/search?q=anime+girl&categories=010&purity=100&sorting=relevance&page=1");
}
#[test] fn empty_query_shows_the_toplist() {
    assert_eq!(search_url("  ", Categories::default(), 2),
        "https://wallhaven.cc/api/v1/search?categories=111&purity=100&sorting=toplist&page=2");
}
#[test] fn parses_results_and_paging() { /* fixture: 2 sfw items + meta {current_page:1,last_page:9}
    -> 2 wallpapers; first: id "mlzoy1", thumb_url = thumbs.small, full_url = path, page_url = url, resolution "3840x2160" */ }
#[test] fn non_safe_results_are_dropped() { /* fixture: one "sfw", one "sketchy" -> only the sfw one */ }
#[test] fn bad_json_is_an_error() { assert!(parse_search("<html>").is_err()); }
#[test] fn download_names_are_safe() {
    assert_eq!(download_path(Path::new(r"C:\bg"), "mlzoy1"), Some(PathBuf::from(r"C:\bg\wallhaven-mlzoy1.jpg")));
    assert_eq!(download_path(Path::new(r"C:\bg"), r"..\evil"), None);
    assert_eq!(download_path(Path::new(r"C:\bg"), ""), None);
}
```
Fixture JSON shape (from the live API): `{"data":[{"id":"mlzoy1","url":"https://wallhaven.cc/w/mlzoy1","purity":"sfw","resolution":"3840x2160","path":"https://w.wallhaven.cc/full/ml/wallhaven-mlzoy1.png","thumbs":{"small":"https://th.wallhaven.cc/small/ml/mlzoy1.jpg"}}],"meta":{"current_page":1,"last_page":9}}`.
- [ ] **Step 2:** Run, see them fail. **Step 3:** Implement (`url::Url::parse_with_params`, `serde_json::Value`; ids must be non-empty ASCII alphanumeric). `url` and `serde_json` become `flick-app` dependencies. **Step 4:** Pass. **Step 5:** Commit `feat(app): wallhaven search address and reply parsing`.

### Task 5: Background workers for search, thumbnails and downloads

**Files:** Create `crates/flick-app/src/http.rs` (move the agent setup out of `suggest_fetch.rs`: `pub fn agent() -> ureq::Agent`), `crates/flick-app/src/gallery_loader.rs`.

**Interfaces:**
- Consumes: Task 4.
- Produces:
  - `pub enum Job { Search { generation: u64, url: String }, Thumb { generation: u64, id: String, url: String }, Download { wallpaper: Wallpaper, dir: PathBuf } }`
  - `pub enum Done { Search { generation: u64, result: anyhow::Result<SearchPage> }, Thumb { generation: u64, id: String, image: anyhow::Result<ColorImage> }, Downloaded { id: String, result: anyhow::Result<PathBuf> } }`
  - `pub struct Loader` with `pub fn start(on_update: impl Fn() + Send + Sync + 'static) -> Loader` (4 workers, real network), `fn start_with(fetch: impl Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync + 'static, on_update) -> Loader` (tests), `pub fn send(&self, job: Job)`, `pub fn take_done(&self) -> Vec<Done>`.
  - Downloads: fetch bytes, `background::decode` (scales to 1280), save as JPEG to `wallhaven::download_path`, creating the folder; thumbnails decode without scaling.

- [ ] **Step 1: Failing tests** (fake `fetch` returning bytes of a 4x2 PNG made with `image`, or an error):

```rust
#[test] fn thumbnail_jobs_come_back_decoded() { /* send Thumb{generation:3,id:"a"}, wait for on_update, take_done -> Thumb{generation:3,id:"a",image:Ok(size [4,2])} */ }
#[test] fn download_saves_a_jpeg_and_returns_its_path() { /* tempdir; Download -> Ok(path) where path == dir/wallhaven-<id>.jpg, file exists, background::load(path) ok */ }
#[test] fn failed_download_reports_an_error_and_writes_nothing() { /* fetch Err -> Downloaded{result:Err}, dir has no files */ }
#[test] fn non_picture_download_is_an_error() { /* fetch Ok(b"<html>") -> Err, no file */ }
```
- [ ] **Step 2–4:** fail, implement (workers share `Arc<Mutex<mpsc::Receiver<Job>>>`; results in `Arc<Mutex<Vec<Done>>>`; `on_update` after each push), pass. `suggest_fetch::start` uses `http::agent()`; its tests still pass.
- [ ] **Step 5:** Commit `feat(app): gallery loader for search, thumbnails and downloads`.

### Task 6: Gallery state and drawing

**Files:** Create `crates/flick-app/src/gallery.rs`.

**Interfaces:**
- Consumes: Tasks 2, 4, 5.
- Produces: `pub enum Tab { Builtin, Downloaded, Online }`; `pub enum Pick { Source(BackgroundSource), Download(Wallpaper) }`; `pub struct Gallery` with `pub fn new(downloads_dir: PathBuf) -> Self`, `pub fn start_search(&mut self) -> Job` (bumps `generation`, clears results, page 1, `loading = true`, `error = None`), `pub fn load_more(&mut self) -> Option<Job>` (next page when `page < last_page` and not loading), `pub fn apply(&mut self, ctx: &egui::Context, done: Done) -> Vec<Job>` (stale generations ignored; new results append and return Thumb jobs; search errors set `error = Some("Can't reach wallhaven.cc".into())`), `pub fn show(&mut self, ui: &mut egui::Ui, palette: &Palette, current: &BackgroundSource) -> GalleryAction` with `pub enum GalleryAction { None, Back, Pick(Pick), Jobs(Vec<Job>) }`; `pub fn list_downloaded(dir: &Path) -> Vec<PathBuf>` (`wallhaven-*.jpg`, newest first); accessors `pub fn results(&self) -> &[Wallpaper]`, `pub fn error(&self) -> Option<&str>`, `pub fn is_loading(&self) -> bool`.

- [ ] **Step 1: Failing tests** (state only, no drawing; build `Done` values by hand):

```rust
#[test] fn new_search_ignores_late_results_from_the_old_one() {
    let mut g = Gallery::new(tmp()); g.start_search(); g.start_search(); // generation 2
    g.apply(&ctx(), Done::Search { generation: 1, result: Ok(page(&["old"], 1, 1)) });
    assert!(g.results().is_empty());
    let jobs = g.apply(&ctx(), Done::Search { generation: 2, result: Ok(page(&["new"], 1, 3)) });
    assert_eq!(ids(&g), ["new"]); assert_eq!(jobs.len(), 1); // one Thumb job
}
#[test] fn offline_shows_cant_reach_and_stops_loading() {
    let mut g = Gallery::new(tmp()); g.start_search();
    g.apply(&ctx(), Done::Search { generation: 1, result: Err(anyhow::anyhow!("dns")) });
    assert_eq!(g.error(), Some("Can't reach wallhaven.cc")); assert!(!g.is_loading());
}
#[test] fn load_more_asks_for_the_next_page_and_appends() { /* page 1 of 3 loaded -> load_more() is Some(Search url ending "&page=2"); after its Done, results are page1 + page2 */ }
#[test] fn no_more_pages_after_the_last() { /* page 3 of 3 -> load_more() is None */ }
#[test] fn downloaded_lists_only_flick_downloads_newest_first() { /* tempdir with wallhaven-a.jpg, wallhaven-b.jpg (b written later), notes.txt -> [b, a] */ }
```
- [ ] **Step 2:** Fail. **Step 3:** Implement state, then drawing:
  - Header row: **Back** button, tab chips **Built-in / Downloaded / Search online**.
  - Online tab: search box (Enter or the **Search** button runs `start_search`), toggles **General / Anime / People**, the error line if any, grid, **Load more** while `page < last_page`, "Loading..." while loading.
  - Grid: 3 columns of 290 x 170 cards; the picture fills the card with `background::cover_uv`; a card that matches `current` gets a 2 px `palette.accent` outline; hover shows resolution and "wallhaven.cc" for online cards plus an **Open page** link (`open::that_detached(page_url)`).
  - Built-in cards use the six built-ins, decoded once per gallery opening; Downloaded cards load each file once.
  - Textures are owned by `Gallery`, so dropping it frees them.
- [ ] **Step 4:** Pass. **Step 5:** Commit `feat(app): background gallery view`.

### Task 7: Gallery inside the bar's window

**Files:** Modify `ui.rs`, `main.rs`, `paths.rs` (`pub fn backgrounds_dir() -> PathBuf` = `%APPDATA%\flick\backgrounds`, with a test that it ends in `flick\backgrounds`).

**Interfaces:**
- Consumes: Tasks 3, 5, 6.
- Produces: `enum View { Bar, Gallery(Gallery) }` in `FlickApp`; `fn open_gallery(&mut self, ctx)` (window to 960 x 640, kept on screen with `placement::keep_on_screen`, Online tab starts the toplist search); `fn close_gallery(&mut self, ctx)` (back to the bar size, gallery dropped); `Services.loader: Loader`.

- [ ] **Step 1:** Wire it: while `View::Gallery`, `ui()` draws the gallery instead of the cards, `read_intents` is skipped except Escape (= Back), and click-away hiding is off. `Pick::Source(s)` → existing `set_background` path with `s`; `Pick::Download(w)` → `loader.send(Job::Download{..})`, and a later `Done::Downloaded{Ok(path)}` → set `BackgroundSource::File(path)`; `Err` → the gallery's error line "Couldn't download that picture". `logic()` drains `loader.take_done()` into the gallery each frame.
- [ ] **Step 2:** Workspace checks pass.
- [ ] **Step 3: By hand** (user): gear → Browse backgrounds → Built-in: click Starry Night (background changes, outline moves) → Search online: type "anime", Enter → thumbnails appear → Load more → pick one (downloads, becomes the background, appears under Downloaded) → Esc returns to the bar → restart Flick: background kept. Wi-Fi off: Search online shows "Can't reach wallhaven.cc", Built-in still works.
- [ ] **Step 4:** Commit `feat(app): open the gallery from the gear menu`.

### Task 8: Docs and release

**Files:** Modify `README.md`.

- [ ] **Step 1:** README: replace the theme/background config rows with `appearance.background` (`builtin:<id>`, a path, or `none`) and `appearance.tint`; describe the gear menu and gallery; a **Credits** section listing the six built-ins (from `CREDITS.md`) and "Online backgrounds come from wallhaven.cc and belong to their creators; Flick downloads one only when you pick it."
- [ ] **Step 2:** `cargo build --release`; record exe size (expected about 2 MB larger than 6.3 MB) and idle memory with the gallery closed.
- [ ] **Step 3:** Commit `docs: background gallery and credits`.
