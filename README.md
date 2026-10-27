# Flick

A keyboard launcher for Windows 11, written entirely in Rust. Press a hotkey and a small search bar appears in the middle of the screen:

- **Google**: Enter opens a Google search in your default browser.
- **YouTube**: Enter opens YouTube search results.
- **Files**: fuzzy search over your Desktop, Documents and Downloads as you type (`bdgt` finds `budget_2026.xlsx`). Arrow keys pick, Enter opens.

The mode chips at the top show where you are. Tab and Shift+Tab switch modes, typing a prefix (`g `, `yt `, `f `) jumps to one, and you can click a chip or a result (hover a chip to see its prefix). In Google and YouTube mode, suggestions appear under your search as you type. Drag the cards' edges or the gap between them to move the window; the top 8 results are always shown in full, and the window slides up if needed to stay on the screen. Esc or clicking elsewhere hides the bar. Flick lives in the tray (Start with Windows, Background, Reload index, Quit).

## Backgrounds

Click the gear at the top right of the bar (or right-click the bar, or use the tray's Background menu):

- **Browse backgrounds...** opens the gallery inside the bar's window. **Built-in** has six public-domain pictures that ship with Flick; **Downloaded** has pictures you picked before; **Search online** searches [wallhaven.cc](https://wallhaven.cc) (safe-for-work only, always), and more pictures load as you scroll. Click a picture to use it; Esc or **Back** returns to the bar.
- **Choose your own picture...** uses any JPG, PNG or WebP on your PC.
- **None** shows the plain Kanagawa background.

Pictures picked online are downloaded once, scaled to 1280 px wide, and kept in `%APPDATA%\flick\backgrounds`.

## Rust only

Every source file is Rust. No Electron, Tauri, webview or JavaScript. The window is [egui/eframe](https://github.com/emilk/egui) with the OpenGL (glow) renderer, and Windows APIs go through Microsoft's [windows](https://github.com/microsoft/windows-rs) crate.

The code is split the way [look](https://github.com/kunkka19xx/look) splits its launcher:

- `crates/flick-core`: modes, prefix parsing, the file index, fuzzy matching ([nucleo](https://github.com/helix-editor/nucleo)), config and the launcher state machine. No UI code; fully unit-tested.
- `crates/flick-app`: the window, global hotkey, tray icon and Win32 calls. It turns key presses into `Intent`s and carries out the `Effect`s the core returns.

## Build and run

Needs Rust 1.95 or newer and the Visual Studio C++ build tools (for the linker).

```bash
cargo build --release
```

Run `target\release\flick.exe`. Nothing appears until you press the hotkey (Alt+Space by default). The first run adds Flick to your startup list, so it is ready after every restart; untick **Start with Windows** in the tray menu to stop that.

## Configuration

`%APPDATA%\flick\config.toml` is created with these defaults on first run:

| Key | Default | Meaning |
|---|---|---|
| `hotkey` | `"Alt+Space"` | Global hotkey. Modifiers: `Alt`, `Ctrl`, `Shift`, `Super` (Windows key). |
| `default_mode` | `"google"` | Mode shown when the bar opens: `google`, `youtube` or `files`. |
| `max_results` | `8` | Most results shown at once. |
| `suggestions` | `true` | Google and YouTube suggestions while you type. This sends what you type to Google, as a browser search box does; set `false` to turn it off. |
| `keywords.google` / `.youtube` / `.files` | `"g"` / `"yt"` / `"f"` | Prefixes that jump to a mode. |
| `files.roots` | `[]` | Folders to index. Empty means Desktop, Documents and Downloads (OneDrive folders included). |
| `files.exclude_dir_names` | `.git`, `node_modules`, `target`, `$RECYCLE.BIN` | Folder names skipped with their contents. |
| `files.reindex_minutes` | `30` | How often the index is rebuilt. `0` turns it off. |
| `appearance.background` | `"builtin:great-wave"` | `builtin:<id>` (`great-wave`, `red-fuji`, `starry-night`, `impression-sunrise`, `earthrise`, `pillars-of-creation`), `none`, or a path to a JPG, PNG or WebP. Set from the gear menu. |
| `appearance.tint` | `0.55` | How dark the cards are over the picture, from `0` (clear) to `1` (solid). |

Choosing a background saves it into this file, rewriting it (hand-written comments are not kept).

A broken config file never stops Flick: it falls back to the defaults and writes the error to `%APPDATA%\flick\flick.log` (replaced at each start).

## Adding a mode

1. Add a file in `crates/flick-core/src/modes/` with a type that implements `SearchMode` (`id`, `name`, `query`).
2. Register it in `config::build_registry` and add its keyword to `Keywords`.

That is the whole contract, modelled on Flow Launcher's plugins: each mode returns results with a title, subtitle, score and an `Action` (`OpenUrl`, `OpenPath` or `None`).

## Measured (release build, 26 October 2026)

| | |
|---|---|
| `flick.exe` size | 6.3 MB |
| Index | 147,671 files and folders built in 1.3 to 2.7 s |
| Memory, idle and hidden, with that index | 64.6 MB private working set |
| Memory with an empty index | 29.5 MB |
| CPU while idle | 0 (no repaints while hidden) |

The index accounts for most of the memory (about 240 bytes per entry). A more compact index layout is the next memory win.

## Credits

Built-in backgrounds, all public domain via Wikimedia Commons (details in `crates/flick-app/assets/backgrounds/CREDITS.md`):

- *The Great Wave off Kanagawa* and *Fine Wind, Clear Morning (Red Fuji)*, Katsushika Hokusai
- *The Starry Night*, Vincent van Gogh
- *Impression, Sunrise*, Claude Monet
- *Earthrise*, NASA, Apollo 8 (Bill Anders)
- *Pillars of Creation*, NASA, ESA and the Hubble Heritage Team

Online backgrounds come from [wallhaven.cc](https://wallhaven.cc) and belong to their creators. Flick downloads one only when you pick it, for your own background, and never bundles or uploads them.

## Later

Apps, Windows settings and GitHub modes; an NTFS MFT/USN or Everything-backed index for whole-PC search; frecency ranking; an installer with cargo-packager.
