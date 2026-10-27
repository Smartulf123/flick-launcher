# Flick Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Flick, an all-Rust Windows 11 keyboard launcher: a global hotkey shows a centered search bar with Google, YouTube and fuzzy file-search modes, built so more modes can be added later.

**Architecture:** look's split, done in Rust: a pure `flick-core` crate (modes, query parsing, file index, fuzzy matching, config, and a launcher state machine that turns `Intent`s into `Effect`s) and a thin `flick-app` crate (eframe window, hotkey, tray, Win32). Flow Launcher's plugin contract becomes the `SearchMode` trait, with action keywords (`g`, `yt`, `f`) in the config file.

**Tech Stack:** Rust 2024 edition (rustc 1.95 or newer), egui/eframe 0.37 (glow renderer), windows-rs, global-hotkey, tray-icon, walkdir, nucleo-matcher, serde + toml, open, url, thiserror, anyhow, log + simplelog; tests with insta and tempfile.

**Spec:** `docs/superpowers/specs/2026-10-26-flick-launcher-design.md` (research baseline: `check-reports/2026-10-26-rust-keyboard-launcher-windows/report.md`)

## Global Constraints

- Rust only: every source file is `.rs` (plus `Cargo.toml`/`.toml`/`.md`). No Tauri app, webview, HTML/CSS/JS, Node tooling or C/C++ sources. Win32 calls go through the `windows` crate.
- Project folder: the repository root. All commands below run from there.
- Workspace: `crates/flick-core` (lib, no UI, no `windows`/`eframe` deps) and `crates/flick-app` (bin `flick`, Windows-only).
- `rust-version = "1.95"` and `edition = "2024"` in the workspace (egui 0.37 requires 1.95).
- eframe: `default-features = false, features = ["default_fonts", "glow", "winit/default"]`.
- Errors: `thiserror` enums in core, `anyhow` in the app. No `unwrap()`/`expect()` outside tests and `main` setup.
- After every task: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all pass.
- Defaults (from the spec): hotkey `Alt+Space`; default mode `google`; keywords `g`, `yt`, `f`; `max_results = 8`; excluded folder names `.git`, `node_modules`, `target`, `$RECYCLE.BIN`; re-index every 30 minutes; window 640 px wide; config at `%APPDATA%\flick\config.toml`.
- URLs: Google `https://www.google.com/search?q=<query>`, YouTube `https://www.youtube.com/results?search_query=<query>`, query form-encoded (space becomes `+`).

## Review Focus

1. A word that merely starts like a keyword (`gaming`, `fyi`, `ytd`) must not switch modes; only `<keyword><space>` does. Test in Task 1.
2. The hotkey may already be taken (PowerToys uses Alt+Space-style shortcuts). Flick must keep running, log the error and say so in the tray tooltip, not crash. Checked in Task 9.
3. When the hotkey shows the bar from the background, it must take keyboard focus so typing works immediately. Checked in Task 8.
4. Tab must cycle modes, never move egui focus out of the text box. Checked in Task 7.
5. Non-ASCII input (Norwegian letters, emoji) must be URL-encoded correctly and match file names. Tests in Tasks 2 and 4.

---

### Task 0: Toolchain setup (one-time, by the user)

- [ ] **Step 1:** Install Rust: run `winget install Rustlang.Rustup`, open a new terminal, then `rustup default stable`.
- [ ] **Step 2:** Confirm the MSVC linker exists: Visual Studio 2022 is installed; in Visual Studio Installer make sure the "Desktop development with C++" workload (or Build Tools with it) is ticked. Rust uses it only to link; no C++ code is written.
- [ ] **Step 3:** Verify. Run `rustc --version` and `cargo --version`. Expected: rustc 1.95.0 or newer.
- [ ] **Step 4:** In Claude Code, add the agent skills from the report: rust-best-practices (apollographql/skills) and egui-agent-skills. The egui skill targets 0.36; tell the agent the project uses 0.37 (`App::ui` plus optional `App::logic`).

### Task 1: Workspace, mode contract and prefix parsing

**Files:**
- Create: `Cargo.toml` (workspace), `.gitignore` (`/target`), `crates/flick-core/Cargo.toml`, `crates/flick-core/src/lib.rs`, `crates/flick-core/src/mode.rs`, `crates/flick-core/src/registry.rs`
- Test: `crates/flick-core/src/registry.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces (`mode.rs`):
  - `pub enum Action { OpenUrl(String), OpenPath(std::path::PathBuf), None }` (derive Debug, Clone, PartialEq, Eq)
  - `pub struct ResultItem { pub title: String, pub subtitle: String, pub score: u32, pub action: Action }` (Debug, Clone, PartialEq, Eq)
  - `pub trait SearchMode: Send + Sync { fn id(&self) -> &str; fn name(&self) -> &str; fn query(&self, text: &str, max_results: usize) -> Vec<ResultItem>; }`
- Produces (`registry.rs`):
  - `pub struct ModeRegistry` with `new() -> Self`, `register(&mut self, keyword: &str, mode: Box<dyn SearchMode>) -> Result<(), RegistryError>`, `len(&self) -> usize`, `mode(&self, index: usize) -> &dyn SearchMode`, `index_of(&self, id: &str) -> Option<usize>`, `parse<'a>(&self, input: &'a str, active: usize) -> ParsedQuery<'a>`, `next(&self, index: usize) -> usize`, `prev(&self, index: usize) -> usize`
  - `pub struct ParsedQuery<'a> { pub mode: usize, pub text: &'a str, pub prefixed: bool }`
  - `pub enum RegistryError { EmptyKeyword, KeywordHasWhitespace(String), DuplicateKeyword(String) }` (thiserror)

- [ ] **Step 1: Create the workspace.** `git init`, root `Cargo.toml` with `[workspace] members = ["crates/*"]`, `resolver = "3"`, `[workspace.package] edition = "2024"`, `rust-version = "1.95"`. Core deps: `thiserror`. Copy the spec and this plan into `docs/`.

- [ ] **Step 2: Write the failing tests** in `registry.rs`, using a test-only `struct Dummy(&'static str)` implementing `SearchMode` (id and name return the string, `query` returns empty). Registry order: google `g`, youtube `yt`, files `f`.

```rust
#[test] fn prefix_selects_mode() {
    let r = registry();
    assert_eq!(r.parse("g rust", 2), ParsedQuery { mode: 0, text: "rust", prefixed: true });
    assert_eq!(r.parse("yt lofi beats", 0), ParsedQuery { mode: 1, text: "lofi beats", prefixed: true });
    assert_eq!(r.parse("f bdgt", 0), ParsedQuery { mode: 2, text: "bdgt", prefixed: true });
}
#[test] fn keywords_are_case_insensitive() { assert_eq!(registry().parse("YT x", 0).mode, 1); }
#[test] fn no_prefix_uses_active_mode() {
    assert_eq!(registry().parse("rust", 1), ParsedQuery { mode: 1, text: "rust", prefixed: false });
}
#[test] fn word_starting_like_keyword_is_not_a_prefix() {
    let r = registry();
    for input in ["gaming", "fyi", "ytd stats", "g"] {
        let p = r.parse(input, 2);
        assert_eq!((p.mode, p.text, p.prefixed), (2, input, false), "{input}");
    }
}
#[test] fn cycling_wraps_both_ways() {
    let r = registry();
    assert_eq!((r.next(0), r.next(1), r.next(2)), (1, 2, 0));
    assert_eq!((r.prev(0), r.prev(2)), (2, 1));
}
#[test] fn bad_keywords_are_rejected() {
    let mut r = registry();
    assert_eq!(r.register("g", Box::new(Dummy("x"))), Err(RegistryError::DuplicateKeyword("g".into())));
    assert_eq!(r.register("G", Box::new(Dummy("x"))), Err(RegistryError::DuplicateKeyword("g".into())));
    assert_eq!(r.register("", Box::new(Dummy("x"))), Err(RegistryError::EmptyKeyword));
    assert_eq!(r.register("a b", Box::new(Dummy("x"))), Err(RegistryError::KeywordHasWhitespace("a b".into())));
}
```

- [ ] **Step 3: Run to see them fail.** `cargo test -p flick-core registry` . Expected: compile errors (types not defined).
- [ ] **Step 4: Implement** `mode.rs` and `registry.rs`. Keywords are stored lowercased; `parse` splits at the first space with `split_once(' ')` and compares the first part with `eq_ignore_ascii_case`. Text after the prefix is returned as typed (not trimmed).
- [ ] **Step 5: Run.** `cargo test -p flick-core` . Expected: 6 passed. Then the three Global Constraints checks.
- [ ] **Step 6: Commit.** `git add -A && git commit -m "feat(core): mode contract and prefix parsing"`

### Task 2: Google and YouTube modes

**Files:**
- Create: `crates/flick-core/src/modes/mod.rs`, `crates/flick-core/src/modes/web.rs`
- Modify: `crates/flick-core/src/lib.rs` (add `pub mod modes;`), `crates/flick-core/Cargo.toml` (add `url`)

**Interfaces:**
- Consumes: `SearchMode`, `ResultItem`, `Action` (Task 1)
- Produces: `pub struct WebSearchMode` with `pub fn google() -> Self` and `pub fn youtube() -> Self`; ids `"google"` / `"youtube"`, names `"Google"` / `"YouTube"`.

- [ ] **Step 1: Write the failing tests** in `web.rs`:

```rust
#[test] fn google_url() {
    let r = WebSearchMode::google().query("rust & egui", 8);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].action, Action::OpenUrl("https://www.google.com/search?q=rust+%26+egui".into()));
    assert_eq!(r[0].title, "Search Google for \"rust & egui\"");
}
#[test] fn youtube_url() {
    let r = WebSearchMode::youtube().query("lofi beats", 8);
    assert_eq!(r[0].action, Action::OpenUrl("https://www.youtube.com/results?search_query=lofi+beats".into()));
}
#[test] fn non_ascii_is_encoded() {
    let r = WebSearchMode::google().query("blåbær 🍓", 8);
    assert_eq!(r[0].action, Action::OpenUrl("https://www.google.com/search?q=bl%C3%A5b%C3%A6r+%F0%9F%8D%93".into()));
}
#[test] fn blank_query_gives_nothing() {
    assert!(WebSearchMode::google().query("", 8).is_empty());
    assert!(WebSearchMode::youtube().query("   ", 8).is_empty());
}
#[test] fn surrounding_spaces_are_trimmed() {
    let r = WebSearchMode::google().query("  rust  ", 8);
    assert_eq!(r[0].action, Action::OpenUrl("https://www.google.com/search?q=rust".into()));
}
```

- [ ] **Step 2: Run to see them fail.** `cargo test -p flick-core web` . Expected: compile error.
- [ ] **Step 3: Implement** `WebSearchMode { id, name, base_url, param }`. Build the URL with `url::Url::parse_with_params(base_url, &[(param, text.trim())])`. Subtitle: the URL. Score: 0.
- [ ] **Step 4: Run.** `cargo test -p flick-core` . Expected: all pass, plus the Global Constraints checks.
- [ ] **Step 5: Commit.** `feat(core): google and youtube modes`

### Task 3: File index

**Files:**
- Create: `crates/flick-core/src/index.rs`
- Modify: `lib.rs` (`pub mod index;`), `crates/flick-core/Cargo.toml` (`walkdir`; dev: `tempfile`)

**Interfaces:**
- Produces:
  - `pub struct FileEntry { pub path: PathBuf, pub name: String, pub is_dir: bool }`
  - `pub struct FileIndex { pub entries: Vec<FileEntry> }` with `pub fn build(roots: &[PathBuf], exclude_dir_names: &[String]) -> FileIndex`
  - `pub type SharedIndex = std::sync::Arc<std::sync::RwLock<Option<FileIndex>>>;`
  - `pub fn spawn_rebuild(shared: SharedIndex, roots: Vec<PathBuf>, exclude_dir_names: Vec<String>, on_done: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()>`

- [ ] **Step 1: Write the failing tests** with a `tempfile::tempdir()` holding `docs/budget_2026.xlsx`, `docs/notes.txt`, `node_modules/pkg/index.js`, `.git/HEAD`:

```rust
#[test] fn indexes_files_and_folders_by_name() {
    let idx = FileIndex::build(&[root.clone()], &excludes());
    let names: Vec<_> = idx.entries.iter().map(|e| (e.name.as_str(), e.is_dir)).collect();
    assert!(names.contains(&("budget_2026.xlsx", false)));
    assert!(names.contains(&("docs", true)));
}
#[test] fn excluded_folders_are_skipped_with_contents() {
    let idx = FileIndex::build(&[root.clone()], &excludes()); // [".git", "node_modules"]
    assert!(idx.entries.iter().all(|e| e.name != "index.js" && e.name != "node_modules" && e.name != "HEAD"));
}
#[test] fn roots_themselves_are_not_entries() { /* no entry has path == root */ }
#[test] fn missing_root_is_ignored() {
    let idx = FileIndex::build(&[root.join("does-not-exist"), root.clone()], &excludes());
    assert!(idx.entries.iter().any(|e| e.name == "notes.txt"));
}
#[test] fn rebuild_fills_shared_index_and_calls_back() {
    let shared: SharedIndex = Default::default();
    let (tx, rx) = std::sync::mpsc::channel();
    spawn_rebuild(shared.clone(), vec![root.clone()], excludes(), move || tx.send(()).unwrap()).join().unwrap();
    rx.try_recv().unwrap();
    assert!(shared.read().unwrap().as_ref().unwrap().entries.len() >= 3);
}
```

- [ ] **Step 2: Run to see them fail.** `cargo test -p flick-core index`
- [ ] **Step 3: Implement.** `WalkDir::new(root).min_depth(1)` with `filter_entry` dropping directories whose name is in the exclude list; skip `Err` entries silently (permission denied must not stop the walk). `spawn_rebuild` builds first, then swaps the result in under the write lock, so the old index stays readable during a build.
- [ ] **Step 4: Run.** `cargo test -p flick-core` . Expected: all pass.
- [ ] **Step 5: Commit.** `feat(core): walkdir file index with background rebuild`

### Task 4: Files mode with fuzzy matching

**Files:**
- Create: `crates/flick-core/src/modes/files.rs`, snapshot dir `crates/flick-core/src/modes/snapshots/`
- Modify: `modes/mod.rs`, `crates/flick-core/Cargo.toml` (`nucleo-matcher`; dev: `insta`)

**Interfaces:**
- Consumes: `SharedIndex`, `FileIndex`, `FileEntry` (Task 3); `SearchMode` (Task 1)
- Produces: `pub struct FilesMode` with `pub fn new(index: SharedIndex) -> Self`; id `"files"`, name `"Files"`; `pub const INDEXING_TITLE: &str = "Indexing files...";`

- [ ] **Step 1: Write the failing tests.** Helper `mode_with(names: &[&str])` builds a `FilesMode` over a `SharedIndex` containing entries `C:\test\<name>`.

```rust
#[test] fn abbreviation_finds_budget_first() {
    let r = mode_with(&["notes.txt", "big_data.txt", "budget_2026.xlsx"]).query("bdgt", 8);
    assert_eq!(r[0].title, "budget_2026.xlsx");
    assert!(r.iter().all(|i| i.title != "notes.txt" && i.title != "big_data.txt"));
    assert_eq!(r[0].action, Action::OpenPath(r"C:\test\budget_2026.xlsx".into()));
    assert_eq!(r[0].subtitle, r"C:\test");
}
#[test] fn case_and_accents_are_ignored() {
    assert_eq!(mode_with(&["Blåbær.txt"]).query("bla", 8)[0].title, "Blåbær.txt");
}
#[test] fn respects_max_results() { assert_eq!(mode_with(&["a1","a2","a3","a4"]).query("a", 2).len(), 2); }
#[test] fn blank_query_gives_nothing() { assert!(mode_with(&["a.txt"]).query("  ", 8).is_empty()); }
#[test] fn shows_indexing_row_before_first_build() {
    let r = FilesMode::new(SharedIndex::default()).query("x", 8);
    assert_eq!(r, vec![ResultItem { title: INDEXING_TITLE.into(), subtitle: String::new(), score: 0, action: Action::None }]);
}
#[test] fn ranking_snapshot() {
    let r = mode_with(&["report.docx", "rust_book.pdf", "Rust", "trust.txt", "ruby.rb"]).query("rust", 8);
    insta::assert_debug_snapshot!(r.iter().map(|i| &i.title).collect::<Vec<_>>());
}
```

- [ ] **Step 2: Run to see them fail.** `cargo test -p flick-core files`
- [ ] **Step 3: Implement.** Under the read lock: `Pattern::parse(text.trim(), CaseMatching::Ignore, Normalization::Smart)` with a `Matcher::new(Config::DEFAULT)`, `match_list` over entry names, keep the top `max_results`, and break score ties by shorter name. Map each to `ResultItem { title: name, subtitle: parent dir, score, action: OpenPath(path) }`.
- [ ] **Step 4: Run and review the snapshot.** `cargo test -p flick-core`, then `cargo insta review` (install once with `cargo install cargo-insta`). Accept only if `Rust` and `rust_book.pdf` rank above `trust.txt` and `ruby.rb` is absent.
- [ ] **Step 5: Commit.** `feat(core): fuzzy files mode with nucleo`

### Task 5: Launcher state machine

**Files:**
- Create: `crates/flick-core/src/launcher.rs`
- Modify: `lib.rs`

**Interfaces:**
- Consumes: `ModeRegistry`, `ParsedQuery` (Task 1); `ResultItem`, `Action`
- Produces:
  - `pub enum Intent { Up, Down, Enter, Tab, ShiftTab, Escape }`
  - `pub enum Effect { Run(Action), Hide }` (Debug, PartialEq)
  - `pub struct Launcher` with `new(registry: ModeRegistry, default_mode: usize, max_results: usize) -> Self`, `set_input(&mut self, text: &str)`, `input(&self) -> &str`, `results(&self) -> &[ResultItem]`, `selected(&self) -> usize`, `effective_mode_name(&self) -> &str`, `handle(&mut self, intent: Intent) -> Vec<Effect>`, `refresh(&mut self)`, `reset(&mut self)`

- [ ] **Step 1: Write the failing tests.** Registry: real `WebSearchMode::google()` (`g`), `WebSearchMode::youtube()` (`yt`), and a `FilesMode` over three fixed entries `C:\test\report.txt`, `C:\test\resume.pdf`, `C:\test\notes.md` (`f`). Default mode 0, `max_results` 8.

```rust
#[test] fn prefix_shows_effective_mode() {
    let mut l = launcher(); l.set_input("yt lofi");
    assert_eq!(l.effective_mode_name(), "YouTube");
}
#[test] fn enter_runs_selected_then_hides_and_resets() {
    let mut l = launcher(); l.set_input("yt lofi");
    assert_eq!(l.handle(Intent::Enter), vec![
        Effect::Run(Action::OpenUrl("https://www.youtube.com/results?search_query=lofi".into())), Effect::Hide]);
    assert_eq!((l.input(), l.effective_mode_name()), ("", "Google"));
}
#[test] fn enter_with_nothing_runnable_does_nothing() {
    let mut l = launcher();
    assert!(l.handle(Intent::Enter).is_empty());           // empty input
    l.set_input("f zzzz"); assert!(l.handle(Intent::Enter).is_empty()); // no matches
}
#[test] fn tab_cycles_and_strips_prefix() {
    let mut l = launcher(); l.set_input("yt lofi");
    l.handle(Intent::Tab);
    assert_eq!((l.input(), l.effective_mode_name()), ("lofi", "Files"));
    l.handle(Intent::ShiftTab);
    assert_eq!(l.effective_mode_name(), "YouTube");
}
#[test] fn arrows_move_and_clamp() {
    let mut l = launcher(); l.set_input("f e"); // matches all 3 fixture entries
    l.handle(Intent::Up); assert_eq!(l.selected(), 0);
    for _ in 0..5 { l.handle(Intent::Down); }
    assert_eq!(l.selected(), 2);
    l.set_input("f es"); assert_eq!(l.selected(), 0); // new input resets selection
}
#[test] fn escape_hides_and_resets() {
    let mut l = launcher(); l.set_input("g rust");
    assert_eq!(l.handle(Intent::Escape), vec![Effect::Hide]);
    assert_eq!(l.input(), "");
}
```

- [ ] **Step 2: Run to see them fail.** `cargo test -p flick-core launcher`
- [ ] **Step 3: Implement.** `set_input` stores the text, parses it, queries the effective mode and resets `selected` to 0. `Tab`/`ShiftTab` set the active mode to `next`/`prev` of the effective mode and replace the input with `parsed.text`. `refresh` re-runs the current query keeping `selected` clamped (used when the index finishes). `reset` clears input, results and selection and restores the default mode.
- [ ] **Step 4: Run.** `cargo test -p flick-core` . Expected: all pass.
- [ ] **Step 5: Commit.** `feat(core): launcher state machine`

### Task 6: Config file

**Files:**
- Create: `crates/flick-core/src/config.rs`
- Modify: `lib.rs`, `crates/flick-core/Cargo.toml` (`serde` with `derive`, `toml`)

**Interfaces:**
- Produces:
  - `#[serde(default)] pub struct Config { pub hotkey: String, pub default_mode: String, pub max_results: usize, pub keywords: Keywords, pub files: FilesConfig }` (Serialize, Deserialize, Debug, Clone, PartialEq, Default impl = the spec defaults)
  - `pub struct Keywords { pub google: String, pub youtube: String, pub files: String }`
  - `pub struct FilesConfig { pub roots: Vec<PathBuf>, pub exclude_dir_names: Vec<String>, pub reindex_minutes: u64 }`
  - `pub fn load_or_create(path: &Path) -> (Config, Option<ConfigError>)`
  - `pub fn build_registry(config: &Config, index: SharedIndex) -> Result<(ModeRegistry, usize), RegistryError>` (the `usize` is the default-mode index; unknown `default_mode` falls back to 0)
  - `pub enum ConfigError { Io(std::io::Error), Parse(toml::de::Error) }` (thiserror)

- [ ] **Step 1: Write the failing tests** (tempdir paths):

```rust
#[test] fn missing_file_is_created_with_defaults() {
    let (c, err) = load_or_create(&dir.join("flick/config.toml"));
    assert!(err.is_none()); assert_eq!(c, Config::default());
    assert!(dir.join("flick/config.toml").exists());
}
#[test] fn defaults_match_spec() {
    let c = Config::default();
    assert_eq!((c.hotkey.as_str(), c.default_mode.as_str(), c.max_results), ("Alt+Space", "google", 8));
    assert_eq!((c.keywords.google.as_str(), c.keywords.youtube.as_str(), c.keywords.files.as_str()), ("g", "yt", "f"));
    assert_eq!(c.files.exclude_dir_names, [".git", "node_modules", "target", "$RECYCLE.BIN"]);
    assert_eq!(c.files.reindex_minutes, 30);
}
#[test] fn partial_file_keeps_other_defaults() { /* file: hotkey = "Ctrl+Space" -> hotkey changed, max_results == 8 */ }
#[test] fn broken_file_gives_defaults_and_error() {
    /* file: "hotkey = " -> (Config::default(), Some(ConfigError::Parse(_))), file left untouched */
}
#[test] fn unknown_default_mode_falls_back_to_first() {
    let c = Config { default_mode: "bing".into(), ..Config::default() };
    assert_eq!(build_registry(&c, Default::default()).unwrap().1, 0);
}
#[test] fn registry_uses_configured_keywords() {
    let mut c = Config::default(); c.keywords.youtube = "y".into();
    let (r, _) = build_registry(&c, Default::default()).unwrap();
    assert_eq!(r.parse("y cats", 0).mode, 1);
}
```

- [ ] **Step 2: Run to see them fail.** `cargo test -p flick-core config`
- [ ] **Step 3: Implement.** Create parent folders, write `toml::to_string_pretty(&Config::default())` when the file is missing. Registration order: google, youtube, files.
- [ ] **Step 4: Run.** `cargo test -p flick-core` . Expected: all pass.
- [ ] **Step 5: Commit.** `feat(core): toml config with defaults`

### Task 7: The search window

**Files:**
- Create: `crates/flick-app/Cargo.toml` (bin `flick`; deps `flick-core`, `eframe` per Global Constraints, `open`, `anyhow`), `crates/flick-app/src/main.rs`, `crates/flick-app/src/ui.rs`, `crates/flick-app/src/actions.rs`

**Interfaces:**
- Consumes: `Launcher`, `Intent`, `Effect`, `Action` (Task 5); `load_or_create`, `build_registry` (Task 6); `spawn_rebuild`, `SharedIndex` (Task 3)
- Produces: `pub struct FlickApp` implementing `eframe::App` (`ui` required, `logic` optional, eframe 0.37); `pub fn run_action(action: &Action) -> std::io::Result<()>` in `actions.rs`; `fn read_intents(ctx: &egui::Context) -> Vec<Intent>` in `ui.rs`

- [ ] **Step 1: Window setup in `main.rs`.** `#![windows_subsystem = "windows"]`. `NativeOptions { viewport: ViewportBuilder::default().with_inner_size([640.0, 64.0]).with_decorations(false).with_always_on_top().with_taskbar(false).with_resizable(false), centered: true, ..Default::default() }`. For this task, roots come from `Config.files.roots` or, if empty, `%USERPROFILE%\Desktop`, `Documents`, `Downloads` (Task 9 replaces this with Known Folders). Start `spawn_rebuild` with `on_done` calling `ctx.request_repaint()`.
- [ ] **Step 2: Key handling in `read_intents`.** Inside `ctx.input_mut`, call `consume_key` for Tab (no modifiers), Shift+Tab, ArrowUp, ArrowDown, Enter and Escape, before the `TextEdit` is drawn, so egui never uses Tab for focus.
- [ ] **Step 3: Drawing in `FlickApp::ui`.** A row with a mode label (`effective_mode_name()`) and a single-line `TextEdit` that requests focus every frame; on change call `launcher.set_input`. Below it, one row per result (title, dim subtitle), the selected row highlighted. Resize the viewport height to `64 + 40 * results.len()` with `ViewportCommand::InnerSize`. Effects: `Run(a)` calls `run_action` (log failures, never panic); `Hide` closes the app for now (Task 8 changes it to hide).
- [ ] **Step 4: `run_action`.** `OpenUrl(u)` and `OpenPath(p)` call `open::that_detached`; `None` does nothing.
- [ ] **Step 5: Verify by hand.** `cargo run -p flick-app`. Expected: a borderless bar centered on screen, no taskbar button. Type `rust`, Enter: Brave opens a Google search. Run again, type `yt lofi`: label reads YouTube; Enter opens YouTube. Run again, Tab twice: label reads Files, focus stays in the text box; type a few letters of a file in Documents, arrow down, Enter: the file opens. Esc closes the window.
- [ ] **Step 6: Commit.** `feat(app): egui search window`

### Task 8: Background process, hotkey and focus

**Files:**
- Create: `crates/flick-app/src/hotkey.rs`, `crates/flick-app/src/window.rs`
- Modify: `main.rs`, `ui.rs`, `crates/flick-app/Cargo.toml` (`global-hotkey`, `windows` with features `Win32_Foundation`, `Win32_UI_WindowsAndMessaging`, `Win32_UI_Input_KeyboardAndMouse`; `raw-window-handle` matching eframe's version)

**Interfaces:**
- Consumes: `FlickApp` (Task 7)
- Produces: `window.rs`: `pub struct WindowHandle(HWND)` (`Send + Sync` wrapper) with `pub fn from_creation_context(cc: &eframe::CreationContext) -> anyhow::Result<Self>`, `pub fn show_and_focus(&self)`, `pub fn hide(&self)`; `hotkey.rs`: `pub fn register(hotkey: &str, window: WindowHandle, ctx: egui::Context) -> anyhow::Result<GlobalHotKeyManager>`

Decision: all show/hide goes through Win32 (`ShowWindow` + `SetForegroundWindow`), never `ViewportCommand::Visible`, because eframe stops calling `ui` for a hidden viewport, so the app could not show itself again from inside the egui loop.

- [ ] **Step 1: `window.rs`.** Get the HWND from `cc.window_handle()` (`RawWindowHandle::Win32`). `show_and_focus`: `ShowWindow(SW_SHOW)` then `SetForegroundWindow`. `hide`: `ShowWindow(SW_HIDE)`.
- [ ] **Step 2: `hotkey.rs`.** Parse with `hotkey.parse::<HotKey>()`, create the `GlobalHotKeyManager` inside the eframe creation closure (event-loop thread, as the crate's README requires), register, and `GlobalHotKeyEvent::set_event_handler(Some(...))`: on `HotKeyState::Pressed`, `show_and_focus()` then `ctx.request_repaint()`. Keep the manager alive in `FlickApp`.
- [ ] **Step 3: Start hidden and hide instead of exit.** Start with `.with_visible(false)`. `Effect::Hide` calls `launcher.reset()` then `window.hide()`. Click-away: in `logic`, if the viewport was focused and `ctx.input(|i| i.viewport().focused) == Some(false)`, do the same.
- [ ] **Step 4: Verify by hand** with `cargo run -p flick-app`. Expected: nothing visible at start. Alt+Space shows the bar; typing works at once without clicking (Review Focus 3). Esc hides it. Alt+Space again shows it with empty input and Google mode, 20 times in a row. Clicking on the desktop hides it. Open Notepad, focus it, press Alt+Space: the bar appears in front and takes the keyboard.
- [ ] **Step 5: Commit.** `feat(app): global hotkey with win32 show and hide`

### Task 9: Tray, single instance, logging, known folders

**Files:**
- Create: `crates/flick-app/src/tray.rs`, `crates/flick-app/src/single_instance.rs`, `crates/flick-app/src/paths.rs`
- Modify: `main.rs`, `crates/flick-app/Cargo.toml` (`tray-icon`, `log`, `simplelog`; `windows` features `Win32_System_Threading`, `Win32_UI_Shell`, `Win32_System_Com`)

**Interfaces:**
- Consumes: `hotkey::register` (Task 8), `spawn_rebuild` (Task 3), `load_or_create` (Task 6)
- Produces: `paths.rs`: `pub fn config_path() -> PathBuf` (`%APPDATA%\flick\config.toml`), `pub fn log_path() -> PathBuf` (`%APPDATA%\flick\flick.log`), `pub fn default_roots() -> Vec<PathBuf>`; `single_instance.rs`: `pub fn acquire() -> Option<InstanceGuard>`; `tray.rs`: `pub fn build(status: &str) -> anyhow::Result<TrayIcon>` with menu items Reload index and Quit

- [ ] **Step 1: Single instance.** `CreateMutexW` named `Local\FlickLauncher`; if `GetLastError()` is `ERROR_ALREADY_EXISTS`, `main` returns immediately.
- [ ] **Step 2: Logging.** `simplelog::WriteLogger` to `log_path()` at Info level, started first in `main`. Log config errors, hotkey failures, index build time and entry count, and `run_action` failures.
- [ ] **Step 3: Known folders.** `default_roots` uses `SHGetKnownFolderPath` for `FOLDERID_Desktop`, `FOLDERID_Documents` and `FOLDERID_Downloads`, so OneDrive-redirected folders are found. Use them when `config.files.roots` is empty.
- [ ] **Step 4: Tray.** Icon built from an RGBA buffer drawn in code (a 32x32 filled circle), so there are no image assets. Tooltip `Flick (Alt+Space)` using the configured hotkey, or `Flick: hotkey <x> unavailable, see log` when registration failed. Reload index calls `spawn_rebuild`; Quit ends the app. Also re-run `spawn_rebuild` every `reindex_minutes` from a background thread.
- [ ] **Step 5: Verify by hand.** `cargo run -p flick-app`. Expected: tray icon with the tooltip; a second `cargo run` exits at once and the hotkey still works once. Set `hotkey = "Super+Space"` (the Windows key plus Space, already taken by Windows; the parser has no `Win` name) in the config and restart: Flick keeps running, the tooltip and `flick.log` report the failure (Review Focus 2). Repeat with `hotkey = "Banana+Space"`: same result, logged as a parse error. Restore the hotkey. Put a file in your (OneDrive) Desktop, choose Reload index, then find it in Files mode. Quit removes the tray icon and ends the process.
- [ ] **Step 6: Commit.** `feat(app): tray, single instance, logging, known folders`

### Task 10: Release check and README

**Files:**
- Create: `README.md`
- Modify: `Cargo.toml` (`[profile.release] lto = true, codegen-units = 1, strip = true, opt-level = 3`)

- [ ] **Step 1: Build.** `cargo build --release`. Expected: `target\release\flick.exe` builds with no warnings.
- [ ] **Step 2: Measure.** Start `flick.exe`, wait for the index log line, then read memory in Task Manager (Details tab, "Memory (active private working set)") when idle and hidden, and after showing the bar 10 times. Record both numbers, the index size and the build time from the log in the README. Target for this plan: under 50 MB idle. If it is over, try `with_visible(false)` startup without the first frame and check the renderer before adding features.
- [ ] **Step 3: Latency check.** Press the hotkey 10 times; the bar must appear without a visible delay. In Files mode, typing in an index of your real folders must not lag. If it does, log the query time per keystroke and note it for the "Later" index work.
- [ ] **Step 4: README.** What Flick is, the Rust-only rule, how to build (`cargo build --release`), the config file with every key and its default, how to add a mode (implement `SearchMode`, register it in `build_registry`, add a keyword to `Keywords`), and the measured numbers.
- [ ] **Step 5: Commit.** `docs: readme and release profile`

---

## Later (separate plans)

From the spec: apps mode, Windows settings mode, GitHub mode, start at login, an NTFS MFT/USN index (ntfs crate) or Everything IPC (ib-everything) for whole-PC search, frecency ranking, a settings window, and an installer with cargo-packager. Each new mode is one file in `flick-core/src/modes/` plus one line in `build_registry`.
