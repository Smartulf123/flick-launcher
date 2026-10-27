# Flick: background gallery (design)

Status: proposed, waiting for review. Builds on the redesign already on `feat/flick-v1`.

## Goal

Make the background easy to customize without hunting for files: a few good pictures built into Flick, a big searchable online library (with anime) inside Flick, and "use my own picture". Also simplify: one look (Kanagawa) instead of six themes.

## What the user gets

- **Gear menu** (and tray / right-click, same menu):
  - **Browse backgrounds...**: opens the gallery.
  - **Choose your own picture...**: a file picker for any JPG, PNG or WebP.
  - **None**: plain dark background.
  - The **Theme** submenu, **My desktop wallpaper** and **Windows pictures...** are removed.
- **Gallery view**: the bar's own window grows to about 960 x 640 and shows the gallery; **Esc** or the **Back** button returns to the search bar. Three sections, as tabs at the top:
  1. **Built-in**: six public-domain pictures that ship inside Flick (work offline).
  2. **Downloaded**: pictures you picked from the online library before (stored on your PC, work offline).
  3. **Search online**: a search box ("anime girl", "mountains", "cyberpunk city"), category toggles **General / Anime / People**, a grid of thumbnails, and **Load more** for the next page. Empty search shows the most popular pictures.
- Clicking any picture sets it as the background immediately and saves the choice. The current background has an accent outline.
- Each online picture shows "wallhaven.cc" and its resolution on hover, and has an **Open page** link to its Wallhaven page (credit to the uploader).

## Built-in pictures

All public domain on Wikimedia Commons (licenses checked through the Commons API on 27 October 2026), scaled to 1280 px wide, about 2 MB in total, embedded in `flick.exe`:

| Picture | Artist / source |
|---|---|
| The Great Wave off Kanagawa (default) | Katsushika Hokusai |
| Fine Wind, Clear Morning (Red Fuji) | Katsushika Hokusai |
| The Starry Night | Vincent van Gogh |
| Impression, Sunrise | Claude Monet |
| Earthrise | NASA, Apollo 8 (Bill Anders) |
| Pillars of Creation | NASA, ESA, Hubble Heritage Team |

Credits go in `crates/flick-app/assets/backgrounds/CREDITS.md` and the README.

## Online library: Wallhaven

- API: `https://wallhaven.cc/api/v1/search` with `q`, `categories` (3-bit mask General/Anime/People), `purity=100` (safe for work only, always; no setting to change it), `sorting=relevance` when there is a query and `toplist` when there is not, and `page`. No account or API key.
- Rate limit is 45 requests a minute; one search per page and one thumbnail per picture stay well under it.
- Thumbnails (`thumbs.small`, about 300 x 200) are fetched by 4 background workers and kept in memory only while the gallery is open; textures are freed when it closes.
- Choosing a picture downloads `path` (the full image), scales it to 1280 px wide, and saves it as `%APPDATA%\flick\backgrounds\wallhaven-<id>.jpg`. The config's `appearance.background` then holds that path.
- Network goes through the same `ureq` + Windows TLS setup as suggestions. No internet: the Search online tab shows "Can't reach wallhaven.cc"; Built-in and Downloaded still work.
- Licensing: Wallhaven pictures belong to their creators. Flick downloads one only when you choose it, for your own background, and never bundles or uploads them.

## Settings and migration

- `appearance.background` accepts `builtin:<id>` (for example `builtin:great-wave`), a file path, or `none`. Default: `builtin:great-wave`.
- Old values: `wallpaper` becomes `builtin:great-wave`; an existing file path keeps working.
- `appearance.theme` is ignored and dropped on the next save; Kanagawa is the only palette.

## How it is built (all Rust)

- `flick-core` stays unchanged except the background setting parse (`BackgroundSource::Builtin(id)`), with tests.
- `flick-app`:
  - `builtin_backgrounds.rs`: the six pictures via `include_bytes!`, with id, title and credit.
  - `wallhaven.rs`: search URL building and JSON parsing (pure, unit-tested), plus the fetch calls.
  - `gallery.rs`: the gallery view state (tab, query, categories, results, pages, thumbnail textures) and drawing.
  - `ui.rs`: a `View::Bar | View::Gallery` switch; while the gallery is open, click-away hiding is off and the window is the larger size.
  - `theme.rs`: the six presets reduced to the Kanagawa palette; the Theme menu is removed from `tray.rs`.
- Tests: URL building and parsing (including safe-for-work always on), background setting parse and migration, built-in list integrity (ids unique, every picture decodes), download file naming, and the gallery's paging state. The look itself is checked by eye.

## Out of scope

Other online libraries, NSFW content, uploading pictures anywhere, picture cropping or position controls, blur.
