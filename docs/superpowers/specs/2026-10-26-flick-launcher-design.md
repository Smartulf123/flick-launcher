# Flick: a Rust keyboard launcher for Windows 11 (design)

Baseline research: [report.md](../../../check-reports/2026-10-26-rust-keyboard-launcher-windows/report.md)

"Flick" is a working name; renaming the crates later is a find-and-replace.

## What it is

A background app for Windows 11, written entirely in Rust. A global hotkey shows a small centered search bar. Three search modes:

1. **Google**: Enter opens a Google search in the default browser (Brave).
2. **YouTube**: Enter opens YouTube search results in the default browser.
3. **Files**: fuzzy-searches files and folders as you type ("bdgt" finds "budget_2026.xlsx"). Arrow keys pick a result, Enter opens it.

Tab cycles modes (Shift+Tab goes back). A prefix also picks the mode: `g `, `yt `, `f `. Esc or clicking away hides the bar. Minimal design, appears instantly, very low memory use. More modes (apps, settings, GitHub) can be added later without touching the existing ones.

## Hard requirement: Rust only

- All source code is Rust. No Electron, no Tauri app, no webview, no HTML/CSS/JS, no C/C++ sources.
- Dependencies are Rust crates from crates.io. `global-hotkey` and `tray-icon` are published by the Tauri org but are standalone Rust crates that pull in no web UI; that is allowed.
- Windows APIs are called through Microsoft's `windows` crate (windows-rs).

## Design: what we borrow

**From look** ([docs/architecture.md](https://github.com/kunkka19xx/look/blob/HEAD/docs/architecture.md)): a Rust core that knows nothing about the UI, plus a thin native shell that talks to it in-process. look keeps matching, ranking, indexing and query parsing in core crates so they are testable without a window. Flick does the same with two crates:

- `flick-core`: modes, query parsing, file index, fuzzy matching, config, and the launcher state machine. Pure Rust, no UI or Win32 dependencies, fully unit-tested.
- `flick-app`: the egui/eframe window, hotkey, tray icon and Win32 calls. Thin; it turns key presses into `Intent`s and carries out the `Effect`s the core returns.

**From Flow Launcher** ([plugin docs](https://github.com/Flow-Launcher/docs)): each plugin has a user-configurable *action keyword* and implements `Query(query) -> List<Result>`, where each result carries a title, subtitle, score and action. Flick's `SearchMode` trait is the same contract in Rust, and the keywords live in the config file, not in the mode code.

Not borrowed: look's SQLite store, AI features and macOS shell; Flow's out-of-process plugins (JSON-RPC, Python, Node). Flick modes are Rust types compiled into the app.

## Stack (from the report)

egui + eframe 0.37 (glow renderer), windows-rs, global-hotkey, tray-icon, walkdir, nucleo-matcher, toml + serde, open, thiserror (core errors), anyhow (app), insta + tempfile (tests).

## Behaviour decisions

- Default hotkey `Alt+Space`, configurable. If registration fails (another app owns it), Flick still runs, logs the error, and shows it in the tray tooltip.
- On each show: input empty, default mode active (config `default_mode`, default `google`).
- A prefix counts only when it is the whole first word followed by a space: `g rust` is Google, `gaming` is not. Keywords match case-insensitively.
- Tab moves to the next mode after the *effective* one and strips a typed prefix, keeping the search text.
- Enter runs the selected result's action, then hides and resets. Enter with no runnable result does nothing.
- Web modes produce one result; an empty or all-space query produces none.
- Files: index of the user's Desktop, Documents and Downloads (resolved with Windows Known Folders, so OneDrive-redirected folders work), or the roots listed in config. Files and folders both. Excluded folder names: `.git`, `node_modules`, `target`, `$RECYCLE.BIN`. Index built in the background at startup and every 30 minutes; the old index stays searchable while a new one builds. Before the first build finishes, Files mode shows a single "Indexing files..." row.
- Matching is on the file name, ranked by nucleo score; at most 8 results (config `max_results`).
- Config file: `%APPDATA%\flick\config.toml`, created with defaults when missing. A broken file never stops the app: defaults are used and the error is logged.
- One instance only: a second launch exits immediately.
- Window: borderless, always on top, no taskbar button, 640 px wide, centered on the primary monitor.

## Later (not in the first plan)

Apps mode, Windows settings mode, GitHub mode, start at login, an NTFS MFT/USN or Everything-backed index for whole-PC search, frecency ranking, a settings UI, and an installer with cargo-packager.
