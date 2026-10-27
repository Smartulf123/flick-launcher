# Flick: Mac and Linux versions, a hotkey picker, and automatic builds (design)

Status: proposed, waiting for review. Builds on `main` at v0.1.1 (Windows only).

## Goal

Anyone can download Flick for Windows, macOS or Ubuntu Linux and get the same launcher: a hotkey shows the bar, it searches Google, YouTube and their own Desktop, Documents and Downloads, it starts at login, and it lives in the tray or menu bar. Anyone can choose their own hotkey from inside Flick. GitHub builds and publishes all downloads when a version tag is pushed.

Windows keeps working exactly as today. Everything stays Rust (no webview, no JavaScript).

People can test by hand on Windows (the author), a Mac and Ubuntu (GNOME on Wayland, the Ubuntu default).

## What the user gets

| | Windows | macOS | Ubuntu / Linux |
|---|---|---|---|
| Download | `Flick-windows-x64.exe` | `Flick-macos.dmg` with `Flick.app` (Apple Silicon and Intel in one) | `flick_<version>_amd64.deb` (Ubuntu 22.04+), `Flick-linux-x86_64.AppImage` (other distributions) |
| Default hotkey | Alt+Space | Option+Space | Ctrl+Alt+Space |
| Lives in | tray | menu bar, no Dock icon | tray (AppIndicator) |
| Start at login menu item | "Start with Windows" | "Open at Login" | "Start at login" |
| First launch warning (unsigned) | SmartScreen: More info, Run anyway | Privacy & Security: Open Anyway | none |

The default hotkey is only a starting point: **Change hotkey...** in the gear and tray menus sets any combination.

## Hotkey picker (all systems)

- **Change hotkey...** in the gear menu and the tray menu switches the bar to a capture view: "Press the keys you want to use", with the current hotkey under it.
- Pressing a combination shows it ("Ctrl + Shift + K"). **Enter** keeps it, **Esc** cancels and returns to the search bar. Pressing another combination replaces the shown one.
- Valid: at least one modifier (Ctrl, Alt, Shift, Win/Super; Cmd and Option on macOS) plus one key, or F1 to F24 alone. Anything else shows "Add Ctrl, Alt, Shift or Win" and is not accepted.
- While capturing, the registered global hotkey is paused so it does not fire.
- On Enter: the new hotkey is registered at once (no restart) and saved to `hotkey` in `config.toml`; the old one is released; the tray tooltip updates. If the system refuses ("already used by another program"), the message shows in the capture view and the old hotkey stays.
- On GNOME Wayland, "registering" means rewriting Flick's GNOME custom shortcut (below).
- Display names per system: macOS shows Cmd and Option; Windows and Linux show Win/Super and Alt. The stored text is the same on every system, e.g. `"Ctrl+Shift+K"` or `"Super+Space"` (Cmd is stored as `Super`), and must parse back with the existing `hotkey::parse`; a unit test checks the round trip for letters, digits, F-keys, Space and every modifier.

## Platform layer

`crates/flick-app/src/platform/` with `mod.rs`, `windows.rs`, `macos.rs`, `linux.rs`, chosen by `#[cfg(target_os)]`. The rest of the app calls only `platform::` functions; no other file checks the operating system. Today's `paths.rs`, `autostart.rs`, `single_instance.rs`, `window.rs` and `shell_icons.rs` move into `platform/windows.rs` (or stay as Windows-only submodules of it) without behavior changes.

| Function | Windows | macOS | Linux |
|---|---|---|---|
| Config and log folder | `%APPDATA%\flick` | `~/Library/Application Support/flick` | `$XDG_CONFIG_HOME/flick` (`~/.config/flick`) |
| Favorites (`backgrounds`) | `%APPDATA%\flick\backgrounds` | `~/Library/Application Support/flick/backgrounds` | `$XDG_DATA_HOME/flick/backgrounds` (`~/.local/share/flick/backgrounds`) |
| Preview cache | `%APPDATA%\flick\cache` | `~/Library/Caches/flick` | `$XDG_CACHE_HOME/flick` (`~/.cache/flick`) |
| Default search folders | Known Folders: Desktop, Documents, Downloads (OneDrive-aware) | `~/Desktop`, `~/Documents`, `~/Downloads` | XDG user dirs (localized names), missing ones skipped |
| Pictures folder (file picker start) | Known Folder Pictures | `~/Pictures` | XDG Pictures dir, else home |
| Start at login | HKCU Run entry (today) | `~/Library/LaunchAgents/io.github.smartulf123.flick.plist` | `~/.config/autostart/flick.desktop` |
| One copy running | named mutex (today) | lock file + Unix socket in the cache folder | lock file + Unix socket in `$XDG_RUNTIME_DIR` (else the cache folder) |
| Show and focus | Win32 (today) | egui viewport commands: Visible, Focus; app activation policy Accessory | egui viewport commands under X11/XWayland |
| Hide | Win32 (today) | viewport Visible(false) | viewport Visible(false) |
| Monitor work area | `GetMonitorInfoW` (today) | whole monitor from egui (no menu-bar exclusion) | whole monitor from egui |
| Rounded corners | DWM (today) | none (square) | none (square) |
| File icons and type names | shell icons, "Microsoft Word Document" (today) | Flick's own File/Folder icons; type = extension in capitals ("PDF"), "Folder" | same as macOS |

Folder lookups use the `dirs` crate on macOS and Linux. Opening files and links already uses the `open` crate on every system.

### `flick --toggle` and the single instance

- The first Flick holds the lock and listens on the socket (macOS, Linux). Messages are one line: `toggle` or `show`.
- `flick --toggle` sends `toggle` and exits. Plain `flick` while one is running sends `show` and exits (today on Windows a second launch just exits; it keeps doing that).
- A stale socket file from a crash is removed when the lock is free.
- Windows keeps its mutex; `--toggle` is not needed there.

## Showing the bar on each system

- **Windows:** unchanged.
- **macOS:** global-hotkey's Carbon hotkey (no Accessibility permission). The bundle's `Info.plist` sets `LSUIElement` so there is no Dock icon. The menu-bar icon is a template image. Showing sends `ViewportCommand::Visible(true)` and `Focus` from the hotkey callback and requests a repaint.
- **Linux, X11 session:** global-hotkey registers the key as on Windows.
- **Linux, Wayland session:** eframe is started on its X11 backend (XWayland), so window position, always-on-top and skip-taskbar work as on X11. Global keys cannot be grabbed, so:
  - On GNOME (`XDG_CURRENT_DESKTOP` contains `GNOME`), Flick writes a custom shortcut through `gsettings` under `org.gnome.settings-daemon.plugins.media-keys` named "Flick", binding the configured hotkey to `<path to flick> --toggle`. It is written at start when missing or different, and when the hotkey picker saves.
  - On other Wayland desktops the tray tooltip says "Bind flick --toggle to a key in your system settings", and the README explains it for KDE.

**Known risks, settled at the skeleton checkpoint before other work depends on them:**

1. GNOME's focus-stealing prevention may show "Flick is ready" instead of focusing the bar when it is shown from `--toggle`. Remedies to try in order: pass the activation token the toggle process received (`XDG_ACTIVATION_TOKEN`/`DESKTOP_STARTUP_ID`) through the socket and use it when showing; failing that, document the "Ubuntu on Xorg" login session as the supported Linux setup.
2. macOS may not run Flick's update loop while the window is hidden. Remedy: drive show/hide from the hotkey callback directly (as Windows does with Win32), using winit on the main thread.

## Builds and releases

- **`.github/workflows/ci.yml`** on every push and pull request to `main`: a matrix of `windows-latest`, `macos-latest` and `ubuntu-22.04` runs `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`; Ubuntu also runs `cargo fmt --all --check`. Ubuntu installs `libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libssl-dev pkg-config`. Tests that need Windows (shell icons, known folders, the registry) are `#[cfg(windows)]`.
- **`.github/workflows/release.yml`** on a pushed tag `v*`:
  - Windows: `cargo build --release` (static C runtime, as today), uploaded as `Flick-windows-x64.exe`.
  - macOS: release builds for `aarch64-apple-darwin` and `x86_64-apple-darwin`, joined with `lipo`, packaged by cargo-packager into `Flick.app` and `Flick-macos.dmg`, ad-hoc signed (`codesign -s -`) so Apple Silicon runs it.
  - Linux on `ubuntu-22.04`: cargo-packager makes the `.deb` (depends on `libgtk-3-0`, `libayatana-appindicator3-1`, `libxdo3`, `libssl3`) and the AppImage.
  - A final job creates the GitHub release for the tag with all four files, install notes per system, and GitHub's generated change list.
- cargo-packager settings live in `crates/flick-app/Cargo.toml` (`[package.metadata.packager]`): product name Flick, identifier `io.github.smartulf123.flick`, icon `crates/flick-app/assets/icon.png`.
- `assets/icon.png` (1024 x 1024) is the tray icon's circle, rendered once by the same drawing code and committed.
- The workspace version is the release version; a release is: bump version, commit, tag `vX.Y.Z`, push.

## Order of work

1. Hotkey picker on Windows. Released on its own (v0.1.2) after the author tries it.
2. Platform layer refactor, Windows only, no behavior change. Author checks Windows still works.
3. `ci.yml` on all three systems (Mac and Linux files may start as minimal stubs that compile).
4. Skeleton on macOS and Linux: window, hotkey or `--toggle`, show and hide, tray. **Checkpoint:** the Mac and Ubuntu testers try a CI-built binary; risks 1 and 2 are settled here.
5. The rest on macOS and Linux: folders, start at login, single instance, generic file icons, GNOME shortcut, hotkey picker.
6. Packaging, `release.yml`, README install sections per system, `docs/testing.md`. Release v0.2.0; testers install the downloads as a new user would.

## Testing

- Unit tests (all systems, in CI): hotkey text to and from egui key presses and the validity rule; autostart file contents (plist, `.desktop`); folder selection from given home/XDG values; socket message parsing and stale-socket handling; GNOME shortcut `gsettings` arguments; the Wayland/X11/GNOME detection from given environment values.
- Existing tests keep passing on Windows; Windows-only ones are gated.
- `docs/testing.md`: a hand-check list per system for testers: the hotkey shows a focused bar; a Google search opens the browser; a file on the Desktop is found and opens; tray/menu-bar items work; start at login works after a restart; the gallery loads, previews and keeps favorites; Change hotkey works and survives a restart; a second launch does not start a second copy.

## Out of scope

- Code signing and notarization (Apple Developer, Windows certificate).
- ARM Linux and ARM Windows builds; Flatpak, Snap, Homebrew, winget.
- Real file icons on macOS and Linux; rounded corners there.
- Auto-update.
