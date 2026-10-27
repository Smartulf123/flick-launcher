# Mac and Linux versions, hotkey picker and automatic builds: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flick runs on Windows, macOS and Ubuntu, anyone can pick their own hotkey inside Flick, and GitHub builds and publishes all downloads from a version tag.

**Architecture:** Every operating-system call moves behind `crates/flick-app/src/platform/` (one folder per OS, chosen by `cfg`), so the rest of the app is OS-agnostic. Logic that can be pure (hotkey text, autostart file contents, socket messages, GNOME settings, folder choice) lives in plain functions with unit tests that run on all three systems in GitHub Actions. Packaging uses cargo-packager in a tag-triggered workflow.

**Tech Stack:** Rust 1.95 (edition 2024), eframe/egui 0.37 (glow), global-hotkey 0.9, tray-icon 0.26 (+ gtk 0.18 on Linux), windows-rs 0.62 and winreg (Windows only), dirs 6 (macOS and Linux), GitHub Actions, cargo-packager.

**Spec:** `docs/superpowers/specs/2026-10-27-cross-platform-design.md`

## Global Constraints

- All code is Rust; no webview or JavaScript. No new dependency may compile C on Windows (system libraries linked on Linux and macOS, e.g. GTK, OpenSSL, are fine).
- Windows behavior stays exactly as today unless a task says otherwise; all existing tests keep passing.
- Default hotkeys: Windows `"Alt+Space"`, macOS `"Alt+Space"` (shown as Option+Space), Linux `"Ctrl+Alt+Space"`.
- Folders, exactly: config and log `%APPDATA%\flick` / `~/Library/Application Support/flick` / `$XDG_CONFIG_HOME/flick`; favorites `<config dir>\backgrounds` on Windows and macOS, `$XDG_DATA_HOME/flick/backgrounds` on Linux; cache `%APPDATA%\flick\cache` / `~/Library/Caches/flick` / `$XDG_CACHE_HOME/flick`.
- Start-at-login labels: "Start with Windows", "Open at Login", "Start at login".
- Bundle identifier `io.github.smartulf123.flick`; LaunchAgent file `~/Library/LaunchAgents/io.github.smartulf123.flick.plist`; Linux autostart file `~/.config/autostart/flick.desktop`.
- Release asset names: `Flick-windows-x64.exe`, `Flick-macos.dmg`, `flick_<version>_amd64.deb`, `Flick-linux-x86_64.AppImage`.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` stay clean.
- Never drive the user's desktop with synthetic keystrokes to test; UI is checked by hand by the user or testers.
- Tests never touch the real startup entry, registry Run key, LaunchAgents or autostart folder: they use temp dirs or test keys.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; repo-local git identity is `Kasper Aune Dahl <262791272+Smartulf123@users.noreply.github.com>`.
- Rust lives in `~/.cargo/bin` (prefix `export PATH="$HOME/.cargo/bin:$PATH"` in Git Bash).

## Review Focus

1. Flick run from an AppImage: autostart and the GNOME shortcut must point at `$APPIMAGE`, not the temporary mount path the binary runs from (test in Task 7).
2. An executable path with spaces (e.g. `/home/a b/Apps/flick`) must be quoted correctly in the `.desktop` `Exec=` line and the GNOME shortcut command (tests in Tasks 7 and 8).
3. A crash leaves the socket file behind: the next start must take over instead of thinking Flick is running (test in Task 5).
4. A hotkey typed by hand in `config.toml` that the picker cannot produce (`"Super+Space"`, `"shift+alt+KeyQ"`) must still display sensibly and stay working (test in Task 1).
5. GNOME already has other custom shortcuts, or none (`@as []`): Flick's entry is added once without removing others (test in Task 8).

---

### Task 1: Hotkey text and picker logic (pure)

**Files:**
- Create: `crates/flick-app/src/hotkey_text.rs`, `crates/flick-app/src/hotkey_picker.rs`
- Modify: `crates/flick-app/src/main.rs` (declare modules)

**Interfaces:**
- Produces:
  - `pub enum Os { Windows, Mac, Linux }` with `pub const CURRENT: Os` (from `cfg!(target_os)`), in `hotkey_text.rs`.
  - `pub enum ComboError { NeedsModifier, UnsupportedKey }` and `pub fn combo_text(modifiers: egui::Modifiers, key: egui::Key) -> Result<String, ComboError>`: canonical order `Ctrl+Alt+Shift+Super+<Key>`; `modifiers.mac_cmd` maps to `Super`; keys allowed: A–Z (`"K"`), 0–9 (`"1"`), `"Space"`, F1–F24 (`"F5"`); F-keys alone are valid.
  - `pub fn display(text: &str, os: Os) -> String`: tokens split on `+`, joined with `" + "`; case-insensitive: ctrl/control→`Ctrl`; alt/option→`Alt` (`Option` on Mac); shift→`Shift`; super/cmd/command/meta→`Win` / `Cmd` / `Super`; `KeyX`→`X`; `DigitN`→`N`; other tokens with first letter upper-cased.
  - `pub fn hint(err: ComboError, os: Os) -> &'static str`: NeedsModifier → `"Add Ctrl, Alt or Shift"` (Windows, Linux) / `"Add Cmd, Option, Ctrl or Shift"` (Mac); UnsupportedKey → `"Use a letter, a number, Space or F1 to F24"`.
  - In `hotkey_picker.rs`: `pub struct Capture { pub candidate: Option<String>, pub message: Option<&'static str> }` (Default), `pub enum Step { Stay, Cancel, Confirm(String) }`, `impl Capture { pub fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers, os: Os) -> Step }`: Escape → `Cancel`; Enter → `Confirm(candidate)` if any, else `Stay` with message `"Press the keys you want to use"`; any other key → `combo_text`; Ok sets `candidate` and clears `message`; Err keeps the old candidate and sets `message = Some(hint(..))`.
- Decided here: Win/Super cannot be captured on Windows and Linux (egui does not report it there); it can still be set by hand in `config.toml`. The spec's "Add Ctrl, Alt, Shift or Win" message becomes the per-OS `hint` above.

- [ ] **Step 1: Write the failing tests** (in each file's `mod tests`):
  - `combo_text(Modifiers::CTRL | Modifiers::SHIFT, Key::K) == Ok("Ctrl+Shift+K")`; `combo_text(Modifiers::ALT, Key::Space) == Ok("Alt+Space")`; `combo_text(Modifiers::NONE, Key::F5) == Ok("F5")`; `combo_text(Modifiers::NONE, Key::K) == Err(NeedsModifier)`; `combo_text(Modifiers::CTRL, Key::Enter) == Err(UnsupportedKey)`; `combo_text(Modifiers { mac_cmd: true, ..Default::default() }, Key::Space) == Ok("Super+Space")`.
  - `round_trips_through_the_hotkey_parser`: for every combo of {Ctrl, Alt, Shift, mac_cmd} × {A, Z, Num0, Num9, Space, F1, F24}, `crate::hotkey::parse(&combo_text(..).unwrap())` is `Ok`.
  - `display("Alt+Space", Os::Mac) == "Option + Space"`; `display("Super+Space", Os::Windows) == "Win + Space"`; `display("shift+alt+KeyQ", Os::Linux) == "Shift + Alt + Q"`; `display("Ctrl+Digit1", Os::Windows) == "Ctrl + 1"`.
  - Picker: Esc → `Cancel`; Enter with nothing → `Stay` with a message; `K` alone → message, no candidate; Ctrl+K then Enter → `Confirm("Ctrl+K")`; Ctrl+K then plain J → candidate stays `"Ctrl+K"`.
- [ ] **Step 2: Run** `cargo test -p flick-app hotkey_` → Expected: FAIL, functions not found.
- [ ] **Step 3: Implement** the functions above.
- [ ] **Step 4: Run** `cargo test -p flick-app hotkey_` → Expected: PASS.
- [ ] **Step 5: Commit** `feat(hotkey): hotkey text conversion and picker logic`.

### Task 2: Change the hotkey from inside Flick (Windows)

**Files:**
- Modify: `crates/flick-app/src/hotkey.rs`, `crates/flick-app/src/ui.rs`, `crates/flick-app/src/tray.rs`, `crates/flick-app/src/main.rs`, `README.md`

**Interfaces:**
- Consumes: Task 1 (`Capture`, `Step`, `display`, `Os::CURRENT`).
- Produces:
  - `pub struct Hotkeys` in `hotkey.rs` with `pub fn new(on_press: impl Fn() + Send + Sync + 'static) -> anyhow::Result<Self>` (sets the global event handler once), `pub fn change(&mut self, text: &str) -> anyhow::Result<()>` (parse; register the new one first; on success unregister the old one and remember the new; on failure the old one stays and the error says `"<display text> is already used by another program"`), `pub fn pause(&self)`, `pub fn resume(&self)`, `pub fn current(&self) -> Option<&str>`. The old `register` function is removed; `main.rs` calls `Hotkeys::new(..)` then `change(&config.hotkey)` and logs/tooltips the error as today.
  - `Services.hotkey: Option<Hotkeys>`.
  - `TrayCommand::ChangeHotkey`; menu item `"Change hotkey..."` added to the tray menu (after Background) and to the gear/right-click bar menu (after None, with a separator).
  - `Tray::set_tooltip(&self, text: &str)`.
  - `FlickApp` field `capture: Option<Capture>`.

- [ ] **Step 1: Write the failing test** in `hotkey.rs`: `change_keeps_the_old_hotkey_when_the_new_one_is_bad`: `Hotkeys::new(|| {})`, `change("Ctrl+Alt+Shift+F24")` is Ok, `change("Banana+Space")` is Err, `current() == Some("Ctrl+Alt+Shift+F24")`. The test is `#[cfg(windows)]`: registering system hotkeys on the macOS and Linux CI runners has no desktop session.
- [ ] **Step 2: Run** `cargo test -p flick-app change_keeps` → Expected: FAIL, `Hotkeys` not found.
- [ ] **Step 3: Implement `Hotkeys`**, the menu items and `TrayCommand::ChangeHotkey`.
- [ ] **Step 4: Wire the capture view in `ui.rs`:** `ChangeHotkey` sets `capture = Some(Capture::default())`, pauses the hotkey and shows the bar. While `capture` is `Some`, key presses go to `Capture::key` instead of the launcher, and the bar's card shows: title `"Press the keys you want to use"`, under it `"Now: <display(current)>"`, the candidate in large text (or the message in the muted color), and `"Enter to keep · Esc to cancel"`. `Confirm(text)` → `hotkeys.change(&text)`: Ok → save `config.hotkey`, `tray.set_tooltip(&tray::tooltip(&text, None))`, close capture, resume; Err → show the error as the message, stay. `Cancel` and hiding the bar → close capture, resume.
- [ ] **Step 5: Run** `cargo test --workspace` and clippy → Expected: all pass, no warnings.
- [ ] **Step 6: README:** in "Using it", add a row `| Gear → **Change hotkey...** | Pick your own hotkey |`; in Settings, `hotkey` row says it can also be set with Change hotkey.
- [ ] **Step 7: Commit** `feat: change the hotkey from inside Flick`.
- [ ] **Step 8: Hand check (stop and ask the user):** build release, restart Flick, ask the user to try Change hotkey (valid key, a plain letter, Esc, a key taken by another app, restart keeps it). Wait for the answer. If approved: bump workspace version to 0.1.2, commit, push, `gh release create v0.1.2 target/release/flick.exe` with short notes (this release is still built locally; `release.yml` comes in Task 9). Ask before publishing.

### Task 3: Platform layer on Windows (no behavior change)

**Files:**
- Create: `crates/flick-app/src/platform/mod.rs`, `platform/message.rs`, `platform/windows/{mod.rs, paths.rs, autostart.rs, instance.rs, window.rs, shell_icons.rs, menu.rs}`
- Move (git mv, then adjust): `paths.rs`, `autostart.rs`, `single_instance.rs`, `window.rs`, `shell_icons.rs` into `platform/windows/`
- Modify: `main.rs`, `ui.rs`, `hotkey.rs`, `tray.rs`, `icons.rs`, `crates/flick-app/Cargo.toml`

**Interfaces (what every OS folder must provide, re-exported by `platform/mod.rs`):**
- `platform::paths::{config_path() -> PathBuf, log_path() -> PathBuf, backgrounds_dir() -> PathBuf, cache_dir() -> PathBuf, default_roots() -> Vec<PathBuf>, pictures_dir() -> PathBuf}`
- `platform::autostart::{LABEL: &str, sync(first_run: bool), toggle(), is_enabled() -> bool}`
- `platform::instance::{acquire(on_message: impl Fn(Message) + Send + 'static) -> Option<Guard>, send(msg: Message) -> bool}`: `None` means another Flick runs. On Windows `on_message` is never called and `send` returns `false`.
- `platform::BarWindow`: `Clone + Send + Sync`, with `from_creation_context(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self>`, `show_and_focus(&self)`, `hide(&self)`, `work_area(&self) -> Option<[i32; 4]>`, `round_corners(&self)`.
- `platform::show_native_menu(tray: &tray::Tray, window: &BarWindow) -> bool`: Windows pops up the native bar menu and returns `true`.
- `platform::IconCache` with today's `get(&mut self, ctx, path, is_dir) -> Option<&(TextureHandle, String)>`.
- `platform::message::{Message { Toggle, Show }, Message::parse(line: &str) -> Option<Message>, Message::as_line(self) -> &'static str}` (lines `"toggle"`, `"show"`; parse trims whitespace, case-sensitive).
- `icons::kind_label(path: &Path, is_dir: bool) -> String`: `"Folder"` for folders, else the extension upper-cased (`"PDF"`), else `"File"`. `ui::row_icon` uses it when `IconCache::get` returns `None` (today it returns an empty label).
- `TrayCommand::ToggleStartWithWindows` renamed `ToggleStartAtLogin`; the tray item text comes from `platform::autostart::LABEL`.
- `Cargo.toml`: `windows` and `winreg` move to `[target.'cfg(windows)'.dependencies]`.

- [ ] **Step 1: Write the failing tests:** `message.rs`: `parse("toggle\n") == Some(Toggle)`, `parse("show") == Some(Show)`, `parse("Toggle") == None`, `parse("") == None`, and `as_line` round-trips. `icons.rs`: `kind_label("C:/a/report.pdf", false) == "PDF"`, `kind_label("C:/a/Makefile", false) == "File"`, `kind_label("C:/a/photos", true) == "Folder"`.
- [ ] **Step 2: Run** `cargo test -p flick-app message kind_label` → Expected: FAIL.
- [ ] **Step 3: Move the modules and implement the interfaces** above on Windows. Bodies are today's code; `BarWindow` is today's `WindowHandle`; `show_native_menu` holds today's `show_bar_menu` call. `ui.rs` and `main.rs` call only `platform::…`.
- [ ] **Step 4: Run** `cargo test --workspace`, clippy, fmt → Expected: same test count as before plus the new ones, all pass.
- [ ] **Step 5: Hand check (ask the user):** release build, restart Flick; hotkey, search, files, gear menu, tray, gallery behave as before. Wait for the answer.
- [ ] **Step 6: Commit** `refactor: move Windows calls into platform/windows`.

### Task 4: macOS and Linux build, and CI on all three systems

**Files:**
- Create: `platform/macos/{mod.rs, paths.rs, window.rs, autostart.rs, instance.rs}`, `platform/linux/{mod.rs, paths.rs, window.rs, autostart.rs, instance.rs, tray_thread.rs}`, `platform/unix_paths.rs`, `crates/flick-app/src/bar_menu.rs`, `.github/workflows/ci.yml`
- Modify: `crates/flick-app/Cargo.toml`, `crates/flick-core/src/config.rs`, `main.rs`, `ui.rs`, `tray.rs`

**Interfaces:**
- Consumes: Task 3 interfaces (implemented here for macOS and Linux; `autostart` and `instance` may be stubs until Tasks 5 and 7: `is_enabled() == false`, `acquire` always `Some`, `send` returns `false`).
- Produces:
  - `unix_paths::default_roots_from(candidates: [Option<PathBuf>; 3]) -> Vec<PathBuf>`: keeps the candidates that are existing directories, in order.
  - macOS/Linux `paths` from `dirs`: `config_dir()`, `data_dir()` (Linux) and `cache_dir()` joined with `flick` per Global Constraints; `default_roots()` = `default_roots_from([dirs::desktop_dir(), dirs::document_dir(), dirs::download_dir()])`; `pictures_dir()` = `dirs::picture_dir()` else `dirs::home_dir()`.
  - macOS/Linux `BarWindow` holds an `egui::Context`; `show_and_focus` sends `ViewportCommand::Visible(true)` and `ViewportCommand::Focus` and requests a repaint; `hide` sends `Visible(false)`; `work_area` returns `None` (callers fall back to the monitor size, as today); `round_corners` does nothing.
  - `platform::show_native_menu` returns `false` on macOS and Linux (and `ui.rs` calls it only when `TrayHandle::local()` is `Some`); otherwise `ui.rs` opens `bar_menu::show(ui: &mut egui::Ui, open: &mut bool) -> Option<TrayCommand>`, an egui popup at the pointer with the same four items (Browse backgrounds..., Choose your own picture..., None, Change hotkey...).
  - `platform::tray::start(build: impl FnOnce() -> anyhow::Result<tray::Tray> + Send + 'static) -> anyhow::Result<TrayHandle>`; `TrayHandle::with(&self, f: impl FnOnce(&tray::Tray) + Send + 'static)` and `TrayHandle::local(&self) -> Option<&tray::Tray>`. Windows and macOS: build runs now on the calling thread, `with` runs `f` immediately, `local` is `Some`. Linux (`tray_thread.rs`): a thread runs `gtk::init()`, `build()`, then `gtk::main()`; `with` sends `f` over an `std::sync::mpsc` channel drained by a `glib::timeout_add_local(100 ms)` on that thread; `local` is `None`. `Services.tray` becomes `Option<TrayHandle>`.
  - macOS: `NativeOptions.event_loop_builder` sets `ActivationPolicy::Accessory` (via `eframe::winit::platform::macos::EventLoopBuilderExtMacOS`); the tray icon uses `with_icon_as_template(true)`.
  - `flick_core::config::default_hotkey() -> &'static str` per Global Constraints; `Config::default()` uses it.
- `Cargo.toml`: `[target.'cfg(unix)'.dependencies] dirs = "6"`; `[target.'cfg(target_os = "linux")'.dependencies] gtk = "0.18"`; eframe gains feature `"x11"` (and not `"wayland"`, so Linux always runs on X11/XWayland).

- [ ] **Step 1: Write the failing tests:** `default_roots_from` with a temp dir holding `Desktop` and `Downloads` but no `Documents` returns those two in order, and `[None, None, None]` returns empty. In flick-core, `defaults_match_spec` asserts `c.hotkey == default_hotkey()` and `default_hotkey()` is `"Ctrl+Alt+Space"` under `cfg!(target_os = "linux")`, else `"Alt+Space"`. Gate Windows-only tests with `#[cfg(windows)]` (known folders, shell icons, registry, mutex).
- [ ] **Step 2: Run** `cargo test --workspace` → Expected: FAIL (functions missing).
- [ ] **Step 3: Implement** the interfaces above.
- [ ] **Step 4: Write `.github/workflows/ci.yml`:** on push and pull_request to `main`; job `test` with matrix `[windows-latest, macos-latest, ubuntu-22.04]`, `dtolnay/rust-toolchain@stable` with clippy and rustfmt, `Swatinem/rust-cache@v2`; on Ubuntu first `sudo apt-get update && sudo apt-get install -y libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libssl-dev pkg-config`; steps `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and on Ubuntu only `cargo fmt --all --check`. Job `preview` only on `workflow_dispatch`: builds release binaries on macOS (both `aarch64-apple-darwin` and `x86_64-apple-darwin`, joined with `lipo -create -output flick-macos flick-arm flick-x86`) and Ubuntu 22.04 (`flick-linux`) and uploads each with `actions/upload-artifact@v4`.
- [ ] **Step 5: Run locally** `cargo test --workspace` and clippy on Windows → Expected: pass. Commit `feat: build on macOS and Linux; CI on all three systems`, push.
- [ ] **Step 6: Check CI:** `gh run watch` on the push run → Expected: all three `test` jobs green. Fix any platform compile error (it is this task's code) and push again until green.

### Task 5: One Flick at a time and `flick --toggle` on macOS and Linux

**Files:**
- Create: `crates/flick-app/src/platform/unix_instance.rs`
- Modify: `platform/macos/instance.rs`, `platform/linux/instance.rs`, `main.rs`, `ui.rs`

**Interfaces:**
- Consumes: `Message` (Task 3), `platform::paths::cache_dir` (Task 4).
- Produces:
  - `unix_instance::acquire_in(dir: &Path, on_message: impl Fn(Message) + Send + 'static) -> std::io::Result<Option<Guard>>`: creates `dir`, opens `dir/flick.lock` and `File::try_lock()`s it; locked elsewhere → `Ok(None)`; got it → removes a leftover `dir/flick.sock`, binds a `UnixListener` there, starts a thread that reads one line per connection and calls `on_message` for each parsed `Message`. `Guard` keeps the lock file and removes the socket on drop.
  - The macOS and Linux `instance::acquire` wrap `acquire_in`: `Ok(Some)` → `Some(guard)`; `Ok(None)` → `None`; `Err(e)` → log the error and return `Some(Guard::detached())` (Flick runs without the single-instance check rather than not at all).
  - `unix_instance::send_in(dir: &Path, msg: Message) -> bool`: connects to `dir/flick.sock`, writes `msg.as_line()` plus `\n`; `false` when nothing listens.
  - The socket folder: macOS `cache_dir()`; Linux `$XDG_RUNTIME_DIR/flick` if set, else `cache_dir()`.
  - `main.rs`: if the first argument is `--toggle`, call `platform::instance::send(Message::Toggle)` and exit 0 without starting the UI or logging. Otherwise `acquire`; on `None`, `send(Message::Show)` and exit.
  - Messages reach the UI through `Arc<Mutex<Vec<Message>>>` plus `ctx.request_repaint()` (via the existing `wake` `OnceLock`). `FlickApp` gains `visible: bool`, set wherever it shows or hides the bar; in `logic`, `Toggle` hides when visible and shows otherwise, `Show` shows.

- [ ] **Step 1: Write the failing tests** (`#[cfg(unix)]`, temp dirs): `second_acquire_in_the_same_dir_is_refused`; `send_reaches_the_first_instance` (a channel in `on_message` receives `Toggle`); `a_leftover_socket_does_not_block_the_next_start` (create a plain file `flick.sock`, no lock held → `acquire_in` returns `Some` and `send_in` then works); `send_without_a_running_flick_returns_false`.
- [ ] **Step 2: Run** on Windows they are skipped; push and check the macOS and Ubuntu CI jobs → Expected: FAIL (missing functions) on those jobs.
- [ ] **Step 3: Implement** the interfaces above.
- [ ] **Step 4: Run** `cargo test --workspace` locally and push → Expected: CI green on all three.
- [ ] **Step 5: Commit** `feat: single instance and flick --toggle on macOS and Linux`.

### Task 6: Skeleton checkpoint with the testers (stop here)

**Files:** `docs/testing.md` (create; first version), `.superpowers` ledger rulings.

- [ ] **Step 1:** Write `docs/testing.md` with a "Skeleton" section: macOS: download the `flick-macos` artifact, `chmod +x flick-macos`, `xattr -d com.apple.quarantine flick-macos`, run it from Terminal; Option+Space shows a focused bar; Esc hides; menu-bar icon appears and Quit works. Ubuntu: `sudo apt install libayatana-appindicator3-1 libxdo3`, `chmod +x flick-linux`, run it; in Settings → Keyboard → Custom Shortcuts bind `/full/path/flick-linux --toggle` to Ctrl+Alt+Space; the shortcut shows a bar you can type in straight away (no "Flick is ready" notice); again hides it; tray icon menu works.
- [ ] **Step 2:** Run the `preview` workflow (`gh workflow run ci.yml` then `gh run watch`), give the user the artifact links and the checklist, and **stop until the user reports back.**
- [ ] **Step 3: Settle the two spec risks from the report**, each recorded as a ledger `Ruling:`:
  - Bar does not appear or update while hidden (macOS or Linux): replace that OS's `BarWindow::show_and_focus`/`hide` with direct window calls through the raw window handle: macOS `objc2-app-kit` (`NSApplication::activate`, `NSWindow::makeKeyAndOrderFront`, `orderOut`), Linux `x11rb` (map, raise, `SetInputFocus`); retest.
  - GNOME shows "Flick is ready" instead of focusing: pass the toggle process's `XDG_ACTIVATION_TOKEN` (or `DESKTOP_STARTUP_ID`) in the message line (`toggle <token>`; extend `Message::parse` and its tests) and set it as `DESKTOP_STARTUP_ID` before showing; if that fails too, the README states "On Ubuntu, choose **Ubuntu on Xorg** at the login screen" and Task 8 still installs the shortcut.
- [ ] **Step 4: Commit** fixes and `docs/testing.md`; push; CI green.

### Task 7: Start at login on macOS and Linux

**Files:**
- Create: `crates/flick-app/src/platform/unix_autostart.rs`
- Modify: `platform/macos/autostart.rs`, `platform/linux/autostart.rs`

**Interfaces:**
- Produces:
  - `unix_autostart::self_command(appimage: Option<&str>, exe: &Path) -> PathBuf`: `appimage` when set and non-empty, else `exe`.
  - `unix_autostart::launch_agent_plist(exe: &Path) -> String`: a plist with `Label` `io.github.smartulf123.flick`, `ProgramArguments` `[exe]`, `RunAtLoad` true; exe XML-escaped.
  - `unix_autostart::desktop_entry(exe: &Path) -> String`: `[Desktop Entry]`, `Type=Application`, `Name=Flick`, `Exec=<quoted exe>`, `X-GNOME-Autostart-enabled=true`; Exec quoting per the Desktop Entry spec (wrap in double quotes; escape `"`, `` ` ``, `$`, `\` with a backslash).
  - `unix_autostart::Entry { file: PathBuf, contents: fn(&Path) -> String }` with `is_enabled()`, `enable(exe)`, `disable()` (missing file is fine), `points_to(exe)` (file contents equal `contents(exe)`).
  - macOS `LABEL = "Open at Login"`, file `~/Library/LaunchAgents/io.github.smartulf123.flick.plist`; Linux `LABEL = "Start at login"`, file `$XDG_CONFIG_HOME/autostart/flick.desktop`. `sync` and `toggle` behave like Windows (enable on first run; repoint if the exe moved), using `self_command(std::env::var("APPIMAGE"), current_exe)`.
- [ ] **Step 1: Write the failing tests:** `self_command(Some("/home/a/Flick.AppImage"), "/tmp/.mount_x/usr/bin/flick") == "/home/a/Flick.AppImage"` and `self_command(Some(""), exe) == exe`; `desktop_entry("/home/a b/flick")` contains `Exec="/home/a b/flick"`; a path with `$` gets `\$`; `launch_agent_plist("/Applications/Flick & Co.app/Contents/MacOS/flick")` contains `Flick &amp; Co`; an `Entry` in a temp dir: `enable` → `is_enabled` and `points_to` true; `disable` twice is Ok.
- [ ] **Step 2: Run** (CI on macOS/Ubuntu, since these are `#[cfg(unix)]`; the pure string functions are not gated and also run locally) → Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** locally and in CI → Expected: PASS on all three.
- [ ] **Step 5: Commit** `feat: start at login on macOS and Linux`.

### Task 8: GNOME shortcut on Wayland, and the hotkey picker there

**Files:**
- Create: `crates/flick-app/src/platform/linux/gnome.rs`, `platform/linux/session.rs`
- Modify: `hotkey.rs`, `platform/mod.rs`, `platform/linux/mod.rs`, `tray.rs`, `main.rs`

**Interfaces:**
- Consumes: `self_command` (Task 7), `Hotkeys` (Task 2), `Message::Toggle` (Task 5).
- Produces:
  - `session::detect(var: impl Fn(&str) -> Option<String>) -> Session` with `pub enum Session { X11, Wayland { gnome: bool } }`: Wayland when `XDG_SESSION_TYPE == "wayland"` or `WAYLAND_DISPLAY` is set; `gnome` when `XDG_CURRENT_DESKTOP` contains `GNOME` (case-insensitive, `:`-separated list).
  - `gnome::accelerator(text: &str) -> Option<String>`: `"Ctrl+Alt+Space"` → `"<Control><Alt>space"`, `"Ctrl+Shift+K"` → `"<Control><Shift>k"`, `"Super+1"` → `"<Super>1"`, `"F5"` → `"F5"`; `None` if `hotkey::parse` rejects the text.
  - `gnome::PATH = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/flick/"`.
  - `gnome::with_flick(list: &str) -> Option<String>`: parses gsettings' printed list (`@as []` or `['a', 'b']`); returns the list with `PATH` appended, or `None` if already present.
  - `gnome::commands(exe: &Path, accel: &str, list: &str) -> Vec<Vec<String>>`: the `gsettings set` argument lists for `name` `Flick`, `command` `'<quoted exe>' --toggle` (single-quote shell quoting), `binding` `accel`, then the list update if `with_flick` returns `Some`.
  - `gnome::apply(text: &str) -> anyhow::Result<()>`: runs `gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings`, then each command; error if `gsettings` is missing or a command fails.
  - `platform::system_shortcut() -> Option<SystemShortcut>` with `pub enum SystemShortcut { Gnome, Manual }`: Linux Wayland only; `None` elsewhere.
  - `Hotkeys` gains a backend: when `system_shortcut()` is `Some(Gnome)`, `change` calls `gnome::apply` instead of registering (pause/resume do nothing); `Some(Manual)` → `change` saves only, and the tray tooltip reads `"Flick: bind flick --toggle to a key in your system settings"`.
- [ ] **Step 1: Write the failing tests** for `detect` (X11; Wayland via either variable; `ubuntu:GNOME` → gnome true; `KDE` → false), `accelerator` (the four cases and `"Banana+X"` → None), `with_flick` (`@as []` → `['<PATH>']`; `['/a/']` → `['/a/', '<PATH>']`; already containing PATH → None), `commands` for exe `/home/a b/flick` (command argument `'/home/a b/flick' --toggle`).
- [ ] **Step 2: Run** in CI (these are `#[cfg(target_os = "linux")]`) → Expected: FAIL on Ubuntu.
- [ ] **Step 3: Implement**, and call `hotkeys.change(&config.hotkey)` at start as today so GNOME's shortcut is written or updated on each start.
- [ ] **Step 4: Run** CI → Expected: green on all three.
- [ ] **Step 5: Commit** `feat: GNOME shortcut on Wayland`; ask the Ubuntu tester to retest the hotkey and Change hotkey (now without binding it by hand).

### Task 9: Packaging, release workflow, docs and v0.2.0

**Files:**
- Create: `.github/workflows/release.yml`, `crates/flick-app/assets/icon.png`, `crates/flick-app/packaging/Info.plist`
- Modify: `crates/flick-app/Cargo.toml`, `crates/flick-app/src/tray.rs` (test that writes the icon), `README.md`, `docs/testing.md`, `Cargo.toml` (version)

**Interfaces:**
- Consumes: `tray::circle_rgba(size)`.
- Produces:
  - `icon.png`: 1024 × 1024 from `circle_rgba(1024)`, written once by an `#[ignore]` test `write_app_icon` and committed.
  - `Info.plist` fragment: `LSUIElement` true.
  - `[package.metadata.packager]` in `crates/flick-app/Cargo.toml`: `product-name = "Flick"`, `identifier = "io.github.smartulf123.flick"`, `icons = ["assets/icon.png"]`, `binaries = [{ path = "flick", main = true }]`, `macos.info-plist-path = "packaging/Info.plist"`, `deb.depends = ["libgtk-3-0", "libayatana-appindicator3-1", "libxdo3", "libssl3"]`.
  - `release.yml` on push of tags `v*`, `permissions: contents: write`:
    - `windows` (windows-latest): `cargo build --release -p flick-app`, rename to `Flick-windows-x64.exe`, upload artifact.
    - `macos` (macos-latest): build both Apple targets, `lipo` into `target/release/flick` (replacing the host build), `cargo install cargo-packager --locked`, `cargo packager --release --formats dmg`, `codesign --force --deep -s - Flick.app` before the dmg if packager does not already ad-hoc sign, rename the dmg to `Flick-macos.dmg`, upload.
    - `linux` (ubuntu-22.04): apt packages as in CI, `cargo packager --release --formats deb,appimage`, rename to `flick_<version>_amd64.deb` and `Flick-linux-x86_64.AppImage`, upload.
    - `publish` (needs all three): download artifacts, `gh release create "$GITHUB_REF_NAME" <files> --title "Flick ${GITHUB_REF_NAME#v}" --notes-file .github/release-notes.md --generate-notes`.
  - `.github/release-notes.md`: install steps per system (Windows: SmartScreen More info → Run anyway; macOS: open the dmg, drag Flick to Applications, first launch → System Settings → Privacy & Security → Open Anyway; Ubuntu: double-click the `.deb`; other Linux: `chmod +x` the AppImage and run it).
- [ ] **Step 1:** Generate and commit `icon.png` (`cargo test -p flick-app write_app_icon -- --ignored`); add packager config and `Info.plist`.
- [ ] **Step 2:** Write `release.yml` and `.github/release-notes.md`. Validate with a pre-release tag: push `v0.2.0-rc.1` → Expected: the workflow builds all four files and creates a release; fix failures and repeat with `rc.2`, … Delete rc releases and tags when done (`gh release delete <tag> --cleanup-tag --yes`), asking the user first.
- [ ] **Step 3: README:** "Download" becomes per system (Windows, macOS, Ubuntu, other Linux) with the asset names and first-launch steps; "Needs Windows 10 or 11" becomes per-system requirements; the hotkey table shows each system's default; settings paths per system; a short "Ubuntu on Wayland" note on the automatic GNOME shortcut and `flick --toggle` for other desktops; "Building from source" lists the Ubuntu apt packages. `docs/testing.md` gets the full hand-check list per system from the spec's Testing section.
- [ ] **Step 4: Run** `cargo test --workspace` and clippy locally; push; CI green.
- [ ] **Step 5: Release (ask the user first):** bump the workspace version to 0.2.0, commit `release: 0.2.0`, tag `v0.2.0`, push the tag → Expected: the release page lists the four assets. Ask the testers to install each download as a new user would and go through `docs/testing.md`.
