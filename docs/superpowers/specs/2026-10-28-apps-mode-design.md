# Apps mode

## Goal

Find and start an installed program from the bar: type `spot`, press Enter, Spotify opens. It works like Files: a fuzzy match on the name, with the program's own icon where the system gives one. It is one more engine in the catalog ("Apps", in the "On this PC" group), so it goes on the bar like any other.

What the owner said: Apps mode comes after 0.3.0 and may take ChatGPT's place among the default five. The rest of this document is decided here, without a round of questions, as the owner prefers; the rulings are marked.

## What counts as an app

**Windows.** Everything in Start's "All apps" list, which Windows exposes as the shell folder `shell:AppsFolder`. That covers desktop programs (from Start menu shortcuts) and Store apps (Calculator, Settings, Photos, Spotify from the Store), which a scan of the Start menu folders would miss. For each item Flick keeps its display name and its parsing name (a Store app id like `Microsoft.WindowsCalculator_8wekyb3d8bbwe!App`, or a program path).

*Ruling*: these are left out: names containing "uninstall", and items whose parsing name is a file path whose extension is not `exe`, `msc` or `appref-ms` (readme files, help pages and web links that installers put in Start). When two items share a name (ignoring case), the first is kept.

**macOS.** `*.app` bundles in `/Applications`, `/Applications/Utilities`, `/System/Applications`, `/System/Applications/Utilities` and `~/Applications`, one folder deep (Flick does not look inside bundles). The name is the bundle name without `.app`.

**Linux.** `.desktop` files in `~/.local/share/applications`, each `$XDG_DATA_DIRS/applications` (default `/usr/local/share:/usr/share`), Flatpak's `exports/share/applications` (user and system) and `/var/lib/snapd/desktop/applications`. A file counts when its `[Desktop Entry]` group has `Type=Application`, a `Name=` and an `Exec=`, and neither `NoDisplay=true` nor `Hidden=true`. The plain `Name=` is used (not translated names). A file in an earlier folder hides one with the same file name in a later folder, as the desktop does.

## Searching

- An `AppsMode` in `flick-core` searches a shared app list the same way `FilesMode` searches the file index: nucleo fuzzy match on the name, best score first, shorter name first on a tie, at most `max_results`.
- An empty search box shows nothing. Before the first scan finishes, one row says "Finding apps...".
- Each result: the name as the title, no subtitle, "App" as the label on the right, and the program's icon (Windows) or a generic app icon (macOS, Linux; real icons there are a later step).

## Starting an app

A new `Action::Launch(String)`; the string means something per system, and `actions.rs` hands it to the platform:

- Windows: opens `shell:AppsFolder\<parsing name>` with the shell, which starts desktop and Store apps alike.
- macOS: `open <path to the .app>`.
- Linux: `gio launch <path to the .desktop file>` (part of GLib, on every GNOME/Ubuntu desktop). If `gio` is missing, the error is logged.

The bar hides after Enter, as for every other result.

## When the list is built

On a background thread at start, and again whenever the file index is rebuilt (every `reindex_minutes`, and "Rebuild index" in the menu), so newly installed apps show up without a restart. The old list stays searchable until the new one is ready. A scan that fails leaves an empty list and logs why.

## Where the code goes

- `flick-core/src/apps.rs` (new): `AppEntry { name, launch }`, `SharedApps`, the Windows filter rules, the `.desktop` parser and the macOS/Linux folder scans (plain file reading, so it is tested on every system).
- `flick-core/src/modes/apps.rs` (new): `AppsMode`.
- `flick-core/src/mode.rs`: `Action::Launch`, `ResultKind::App`.
- `flick-core/src/catalog.rs`: `Target::Apps`; the "apps" engine, after Files. `config::build_modes_with` registers it with the shared list.
- `flick-app/src/platform/win/apps.rs` (new): the `shell:AppsFolder` listing, through the Windows shell (COM), and its icons.
- `flick-app/src/platform/{win,unix}`: `apps::scan()` and `launch_app()`, exported from `platform`.
- `flick-app/src/icons.rs`: an `App` icon for macOS, Linux and when Windows has none.
- `flick-app/src/main.rs`: the scan runs alongside the file index.

## Default engines

*Ruling*: the default five become Google, YouTube, Files, Apps and Wikipedia; ChatGPT leaves the defaults but stays in the list. This applies to new settings files; anyone who already has an `engines` line keeps theirs and can add Apps with **+**.

## Testing

Unit tests: the Windows filter (uninstallers, documents and duplicates dropped, Store ids kept), `.desktop` parsing (hidden, no-display, non-application, missing fields, `[Desktop Action]` groups ignored), earlier folders hiding later ones, the macOS scan on a temporary folder of fake `.app` folders, `AppsMode` ranking and its "Finding apps..." row, the catalog entry, and that `Action::Launch` reaches the platform. The `shell:AppsFolder` listing and starting real apps are checked by hand on Windows; macOS and Linux stay "preview", as in 0.2.0.
