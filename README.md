# Flick

**A fast keyboard launcher for Windows, macOS and Linux, written in Rust.**

Press **Alt+Space** (Option+Space on a Mac, Ctrl+Alt+Space on Linux) anywhere and a small search bar appears in the middle of your screen. Type, press Enter, and it's gone. You choose which search engines sit on the bar (up to five); out of the box:

- **Google**: Enter opens a Google search in your default browser, with suggestions as you type.
- **YouTube**: the same, for YouTube search results.
- **Files**: fuzzy search over your own Desktop, Documents and Downloads folders as you type (`bdgt` finds `budget_2026.xlsx`). On Windows, files and folders show their real Windows icons.
- **Apps**: find a program you have installed and start it (`spot` starts Spotify). On Windows this is everything in Start's app list, Store apps included, with their own icons; on a Mac the apps in Applications; on Linux the apps in your app menu.
- **Wikipedia**.

Swap them for any of 24 engines (DuckDuckGo, ChatGPT, Claude, Spotify, GitHub, Maps, Amazon, Reddit and more) or add your own.

It runs quietly in the tray (the menu bar on a Mac), starts when you log in, and sits on a background picture you choose: one of six classic paintings and space photos that come with it, a picture of your own, or any wallpaper you find through the built-in online gallery.

## Download

All downloads are on the [latest release](../../releases/latest) page. None of them need anything else installed.

### Windows 10 or 11

1. Download **`Flick-windows-x64.exe`**.
2. Move it somewhere it can stay, for example a new folder `C:\Tools\Flick`. Flick adds itself to your startup apps from wherever it is, so don't run it straight from Downloads if you tidy that folder.
3. Double-click it. Nothing appears yet; that's normal. Press **Alt+Space**.

Windows may show "Windows protected your PC" the first time, because the program isn't code-signed. Click **More info**, then **Run anyway**.

To remove Flick: right-click its tray icon, untick **Start with Windows**, choose **Quit**, and delete the program and the folder `%APPDATA%\flick`.

### macOS 11 or newer (preview)

1. Download **`Flick-macos.dmg`** (one download for Apple Silicon and Intel Macs), open it and drag **Flick** to **Applications**.
2. Open Flick. macOS blocks it the first time because it isn't notarized by Apple: open **System Settings → Privacy & Security** and click **Open Anyway**.
3. A small circle appears in the menu bar; there is no Dock icon. Press **Option+Space**.

To remove Flick: untick **Open at Login** in its menu-bar menu, choose **Quit**, delete Flick from Applications, and delete `~/Library/Application Support/flick`.

### Ubuntu 22.04 or newer (preview)

1. Download **`flick_<version>_amd64.deb`** and double-click it to install (or run `sudo apt install ./flick_*_amd64.deb`).
2. Start **Flick** from the app list. A small circle appears in the top bar.
3. Press **Ctrl+Alt+Space**.

On Ubuntu's normal (Wayland) session apps can't grab a key themselves, so Flick adds a shortcut named "Flick" to **Settings → Keyboard → Custom Shortcuts** that runs `flick --toggle`. On other Wayland desktops (such as KDE Plasma) bind `flick --toggle` to a key in your system's shortcut settings; Flick's tray tip reminds you.

To remove Flick: untick **Start at login**, choose **Quit**, run `sudo apt remove flick`, and delete `~/.config/flick`, `~/.local/share/flick` and `~/.cache/flick`.

### Other Linux (preview)

Download **`Flick-linux-x86_64.AppImage`**, make it runnable (`chmod +x Flick-linux-x86_64.AppImage`) and run it.

The macOS and Linux versions are new and have not been tried by hand on every setup yet. If something doesn't work, please [open an issue](../../issues).

## Using it

| Do this | To |
|---|---|
| **Alt+Space** (Mac: Option+Space, Linux: Ctrl+Alt+Space) | Show or hide the bar |
| Type, then **Enter** | Search, or open the selected file |
| **Tab** / **Shift+Tab** | Switch between the engines on the bar |
| **↑** / **↓** | Pick a result |
| **Esc**, or click elsewhere | Hide the bar |
| Drag the edges of the bar | Move it |
| Gear → **Change hotkey...** | Pick your own hotkey: press the keys, then Enter |

You can also click an engine chip or a result with the mouse. The gear at the top right (or right-clicking the bar) opens the background menu. The tray icon has **Start with Windows** (**Open at Login** on a Mac, **Start at login** on Linux), **Background**, **Change hotkey...**, **Reload index** (scans your folders again right away) and **Quit**. On a Mac and on Linux the gear opens the same choices inside the bar.

### Search engines

Click **+** at the end of the engine chips to see every engine, grouped; type to filter the list, and click one (or pick it with the arrows and Enter) to put it on the bar. Hover a chip and click its **×** to take it off, or drag it to a new place. The bar holds one to five engines; to swap one in when it is full, take one off first. **+ Add your own...** at the bottom of the list adds any site that has a search address: give it a name, the address with `{query}` where your text goes (for example `https://www.thingiverse.com/search?q={query}`).

| Group | Engines |
|---|---|
| On this PC | Files, Apps |
| Web search | Google, DuckDuckGo, Bing, Brave |
| AI | ChatGPT, Claude, Perplexity |
| Video & music | YouTube, Spotify, Twitch |
| Knowledge | Wikipedia, Maps, Translate |
| Code | GitHub, Stack Overflow, MDN, docs.rs |
| Shopping | Amazon, eBay |
| Social | Reddit, X, LinkedIn |

Google, YouTube, Wikipedia and DuckDuckGo show suggestions as you type. Wikipedia and Amazon use their English and US sites; add your own country's as a custom engine.

### Your files

Out of the box Flick indexes the Desktop, Documents and Downloads folders of whoever is signed in (on Windows including OneDrive-synced ones; on Linux whatever your system calls those folders, in any language). To search other folders, list them under `files.roots` in the settings file (see [Settings](#settings)), then restart Flick (**Quit** in the tray menu, then start it again). The index refreshes itself every 30 minutes.

### Backgrounds

Click the gear and choose:

- **Browse backgrounds...** opens a gallery with three tabs:
  - **Built-in**: six public-domain pictures that ship with Flick.
  - **Favorites**: pictures you kept from the online search.
  - **Search online**: search [wallhaven.cc](https://wallhaven.cc) for wallpapers (safe-for-work results only, always). Click a picture to try it as your background; tap the heart in its corner to keep it under Favorites, and tap again to remove it. More pictures load as you scroll.
- **Choose your own picture...** uses any JPG, PNG or WebP on your PC.
- **None** shows a plain dark background.

## Settings

Flick creates `config.toml` the first time it runs: in `%APPDATA%\flick` on Windows, `~/Library/Application Support/flick` on a Mac and `~/.config/flick` on Linux. Open it in any text editor; changes apply the next time Flick starts (**Quit** in the tray menu, then start it again).

| Setting | Default | What it does |
|---|---|---|
| `hotkey` | `"Alt+Space"` (Linux: `"Ctrl+Alt+Space"`) | The key that shows the bar. Easiest to set with **Change hotkey...** in the gear or tray menu; by hand, modifiers are `Alt` (Option on a Mac), `Ctrl`, `Shift`, `Super` (the Windows key, or Cmd on a Mac), e.g. `"Ctrl+Space"`. |
| `default_mode` | `"google"` | Engine the bar opens in, by its id (`google`, `files`, `github`, ...). If it is not on the bar, the first engine is used. |
| `engines` | `["google", "youtube", "files", "apps", "wikipedia"]` | The engines on the bar, in order. Easiest to change with **+** on the bar. |
| `custom_engines` | none | Engines you added: `name` and `url` (with `{query}`). |
| `max_results` | `8` | Most results shown at once. |
| `suggestions` | `true` | Suggestions while you type, for Google, YouTube, Wikipedia and DuckDuckGo. |
| `files.roots` | `[]` | Folders to search. Empty means your Desktop, Documents and Downloads. Example: `roots = ['C:\Users\you\Projects', 'D:\Photos']` |
| `files.exclude_dir_names` | `.git`, `node_modules`, `target`, `$RECYCLE.BIN` | Folder names skipped, with everything inside them. |
| `files.reindex_minutes` | `30` | How often the file index is rebuilt. `0` turns it off. |
| `appearance.background` | `"builtin:great-wave"` | Set from the gear menu: `builtin:<name>`, `none`, or a path to a picture. |
| `appearance.tint` | `0.55` | How dark the bar is over the picture, from `0` (clear) to `1` (solid). |

Choosing a background or changing the engines on the bar saves it into this file (comments you added are not kept). A broken settings file never stops Flick: it uses the defaults and writes what went wrong to `flick.log` in the same folder.

## Privacy

Flick has no account, no telemetry and no tracking. It only goes online for these:

- **Search suggestions**: what you type in Google or YouTube is sent to Google's suggestion service, in Wikipedia to Wikipedia's and in DuckDuckGo to DuckDuckGo's, as a browser's search box does. Other engines get nothing until you press Enter. Set `suggestions = false` to turn this off.
- **The online gallery**: searches and pictures come from wallhaven.cc, only while the gallery's online tab is open.

Your file index stays in memory on your PC and is never sent anywhere.

## Building from source

Needs [Rust](https://rustup.rs) 1.95 or newer. On Windows also the Visual Studio C++ build tools; on Ubuntu `sudo apt install pkg-config libssl-dev`.

```bash
git clone https://github.com/Smartulf123/flick-launcher.git
cd flick-launcher
cargo build --release
```

The program is `target/release/flick` (`flick.exe` on Windows). `cargo test --workspace` runs the tests. [`docs/testing.md`](docs/testing.md) lists the checks done by hand on each system.

### How it's built

Every line is Rust: no Electron, Tauri, webview or JavaScript. The window is [egui/eframe](https://github.com/emilk/egui) with the OpenGL renderer. Everything that differs between Windows, macOS and Linux lives in `crates/flick-app/src/platform/`; Windows APIs go through Microsoft's [windows](https://github.com/microsoft/windows-rs) crate, and the Linux tray talks D-Bus in pure Rust, so no GTK is needed.

- `crates/flick-core`: the modes, file index, fuzzy matching ([nucleo](https://github.com/helix-editor/nucleo)), settings and the launcher's state machine. No UI code, fully unit-tested.
- `crates/flick-app`: the window, hotkey, tray icon, background gallery and the per-system code.

It is light: a single program of about 10 MB that uses no CPU while hidden and about 80 MB of memory with a 150,000-file index (most of that is the index).

### Adding an engine

A website is one line in `CATALOG` in `crates/flick-core/src/catalog.rs`: id, name, group and the search address with `{query}`.

Anything else is a mode:

1. Add a file in `crates/flick-core/src/modes/` with a type that implements `SearchMode` (`id`, `name`, `query`).
2. Give it a `Target` in the catalog and register it in `config::build_modes_with`; it can then go on the bar.

Each mode returns results with a title, subtitle, score and an action (`OpenUrl`, `OpenPath` or `None`). Ideas: apps, Windows settings, a calculator.

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
