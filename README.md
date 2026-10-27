# Flick

**A fast keyboard launcher for Windows 11, written in Rust.**

Press **Alt+Space** anywhere and a small search bar appears in the middle of your screen. Type, press Enter, and it's gone. Flick searches three places:

- **Google**: Enter opens a Google search in your default browser, with suggestions as you type.
- **YouTube**: the same, for YouTube search results.
- **Files**: fuzzy search over your own Desktop, Documents and Downloads folders as you type (`bdgt` finds `budget_2026.xlsx`). Files and folders show their real Windows icons.

It runs quietly in the tray, starts with Windows, and sits on a background picture you choose: one of six classic paintings and space photos that come with it, a picture of your own, or any wallpaper you find through the built-in online gallery.

Flick was inspired by [Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher) (modes you can switch between) and [look](https://github.com/kunkka19xx/look) (a launcher that looks good).

## Download

1. Download **`flick.exe`** from the [latest release](../../releases/latest).
2. Move it somewhere it can stay, for example a new folder `C:\Tools\Flick`. Flick adds itself to your startup apps from wherever it is, so don't run it straight from Downloads if you tidy that folder.
3. Double-click it. Nothing appears yet; that's normal. Press **Alt+Space**.

Windows may show "Windows protected your PC" the first time, because the program isn't code-signed. Click **More info**, then **Run anyway**.

Needs Windows 10 or 11 (64-bit). Nothing else to install.

To remove Flick: right-click its tray icon, untick **Start with Windows**, choose **Quit**, and delete `flick.exe` and the folder `%APPDATA%\flick`.

## Using it

| Do this | To |
|---|---|
| **Alt+Space** | Show or hide the bar |
| Type, then **Enter** | Search, or open the selected file |
| **Tab** / **Shift+Tab** | Switch between Google, YouTube and Files |
| Type `g `, `yt ` or `f ` (with the space) | Jump straight to that mode |
| **↑** / **↓** | Pick a result |
| **Esc**, or click elsewhere | Hide the bar |
| Drag the edges of the bar | Move it |

You can also click a mode chip or a result with the mouse; hover a chip to see its shortcut. The gear at the top right (or right-clicking the bar) opens the background menu. The tray icon has **Start with Windows**, **Background**, **Reload index** (scans your folders again right away) and **Quit**.

### Your files

Out of the box Flick indexes the Desktop, Documents and Downloads folders of whoever is signed in to Windows, including OneDrive-synced ones. To search other folders, list them under `files.roots` in the settings file (see [Settings](#settings)), then restart Flick (**Quit** in the tray menu, then start `flick.exe` again). The index refreshes itself every 30 minutes.

### Backgrounds

Click the gear and choose:

- **Browse backgrounds...** opens a gallery with three tabs:
  - **Built-in**: six public-domain pictures that ship with Flick.
  - **Favorites**: pictures you kept from the online search.
  - **Search online**: search [wallhaven.cc](https://wallhaven.cc) for wallpapers (safe-for-work results only, always). Click a picture to try it as your background; tap the heart in its corner to keep it under Favorites, and tap again to remove it. More pictures load as you scroll.
- **Choose your own picture...** uses any JPG, PNG or WebP on your PC.
- **None** shows a plain dark background.

## Settings

Flick creates `%APPDATA%\flick\config.toml` the first time it runs. Open it in any text editor; changes apply the next time Flick starts (**Quit** in the tray menu, then start `flick.exe` again).

| Setting | Default | What it does |
|---|---|---|
| `hotkey` | `"Alt+Space"` | The key that shows the bar. Modifiers: `Alt`, `Ctrl`, `Shift`, `Super` (the Windows key), e.g. `"Ctrl+Space"`. |
| `default_mode` | `"google"` | Mode the bar opens in: `google`, `youtube` or `files`. |
| `max_results` | `8` | Most results shown at once. |
| `suggestions` | `true` | Google and YouTube suggestions while you type. |
| `keywords.google` / `.youtube` / `.files` | `"g"` / `"yt"` / `"f"` | The prefixes that jump to a mode. |
| `files.roots` | `[]` | Folders to search. Empty means your Desktop, Documents and Downloads. Example: `roots = ['C:\Users\you\Projects', 'D:\Photos']` |
| `files.exclude_dir_names` | `.git`, `node_modules`, `target`, `$RECYCLE.BIN` | Folder names skipped, with everything inside them. |
| `files.reindex_minutes` | `30` | How often the file index is rebuilt. `0` turns it off. |
| `appearance.background` | `"builtin:great-wave"` | Set from the gear menu: `builtin:<name>`, `none`, or a path to a picture. |
| `appearance.tint` | `0.55` | How dark the bar is over the picture, from `0` (clear) to `1` (solid). |

Choosing a background from the menu saves it into this file (comments you added are not kept). A broken settings file never stops Flick: it uses the defaults and writes what went wrong to `%APPDATA%\flick\flick.log`.

## Privacy

Flick has no account, no telemetry and no tracking. It only goes online for these:

- **Search suggestions**: what you type in Google or YouTube mode is sent to Google's suggestion service, as a browser's search box does. Set `suggestions = false` to turn this off.
- **The online gallery**: searches and pictures come from wallhaven.cc, only while the gallery's online tab is open.

Your file index stays in memory on your PC and is never sent anywhere.

## Building from source

Needs [Rust](https://rustup.rs) 1.95 or newer and the Visual Studio C++ build tools.

```bash
git clone https://github.com/Smartulf123/flick-launcher.git
cd flick-launcher
cargo build --release
```

The program is `target\release\flick.exe`. `cargo test --workspace` runs the tests.

### How it's built

Every line is Rust: no Electron, Tauri, webview or JavaScript. The window is [egui/eframe](https://github.com/emilk/egui) with the OpenGL renderer, and Windows APIs go through Microsoft's [windows](https://github.com/microsoft/windows-rs) crate.

- `crates/flick-core`: the modes, prefix parsing, file index, fuzzy matching ([nucleo](https://github.com/helix-editor/nucleo)), settings and the launcher's state machine. No UI code, fully unit-tested.
- `crates/flick-app`: the window, hotkey, tray icon, background gallery and Windows calls.

It is light: a single 10 MB program that uses no CPU while hidden and about 80 MB of memory with a 150,000-file index (most of that is the index).

### Adding a mode

1. Add a file in `crates/flick-core/src/modes/` with a type that implements `SearchMode` (`id`, `name`, `query`).
2. Register it in `config::build_registry` and give it a keyword in `Keywords`.

Each mode returns results with a title, subtitle, score and an action (`OpenUrl`, `OpenPath` or `None`). Ideas: apps, Windows settings, a calculator, GitHub.

## Credits

Built-in backgrounds, all public domain via Wikimedia Commons (sources in [`CREDITS.md`](crates/flick-app/assets/backgrounds/CREDITS.md)):

- *The Great Wave off Kanagawa* and *Fine Wind, Clear Morning (Red Fuji)*, Katsushika Hokusai
- *The Starry Night*, Vincent van Gogh
- *Impression, Sunrise*, Claude Monet
- *Earthrise*, NASA, Apollo 8 (Bill Anders)
- *Pillars of Creation*, NASA, ESA and the Hubble Heritage Team

Online backgrounds come from [wallhaven.cc](https://wallhaven.cc) and belong to their creators. Flick downloads one only when you choose it, for your own use, and never bundles or uploads them. The colours are the [Kanagawa](https://github.com/rebelot/kanagawa.nvim) palette.

## License

[MIT](LICENSE)
