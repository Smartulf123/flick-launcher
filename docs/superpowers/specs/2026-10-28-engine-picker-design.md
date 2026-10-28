# Engine picker — design

Date: 2026-10-28. Status: decided (the owner asked to build without further questions; decisions below marked *ruling* were made by Claude).

## Goal

People choose which search engines appear as chips in the bar (1 to 6, default 5), order them by dragging, add their own engines, and reach any engine in the catalog with a short shortcut such as `gh rust`. Apps mode (launching installed programs) is a separate, later project and is not part of this design.

## Agreed with the owner

- Picker lives on the chip row (option B): hover a chip for a remove badge, drag chips to reorder, a "+" chip opens an engine list in the results area, filtered by what is typed in the search box. At 6 chips adding is off; the last chip cannot be removed.
- "Add your own" is a small form in the same space: name, search address with `{query}`, shortcut.
- Shortcuts work for every engine in the catalog, not only the chosen chips.
- Default chips: Google, YouTube, Files, Wikipedia, ChatGPT (*ruling*: in that order, so existing users keep their first three in the same place).
- Approach: one generic web engine fed by a built-in catalog table; custom engines are extra rows stored in the settings file.

## Catalog

| Group | Engine | id | Shortcut | Address | Type-ahead |
|---|---|---|---|---|---|
| On this PC | Files | files | fi | (local) | |
| Web search | Google | google | gg | https://www.google.com/search?q={query} | Google |
| | DuckDuckGo | duckduckgo | ddg | https://duckduckgo.com/?q={query} | DuckDuckGo |
| | Bing | bing | bi | https://www.bing.com/search?q={query} | |
| | Brave | brave | brv | https://search.brave.com/search?q={query} | |
| AI | ChatGPT | chatgpt | gpt | https://chatgpt.com/?q={query} | |
| | Claude | claude | cl | https://claude.ai/new?q={query} | |
| | Perplexity | perplexity | pp | https://www.perplexity.ai/search?q={query} | |
| Video & music | YouTube | youtube | yt | https://www.youtube.com/results?search_query={query} | YouTube |
| | Spotify | spotify | sp | https://open.spotify.com/search/{query} | |
| | Twitch | twitch | tw | https://www.twitch.tv/search?term={query} | |
| Knowledge | Wikipedia | wikipedia | wp | https://en.wikipedia.org/w/index.php?search={query} | Wikipedia |
| | Maps | maps | mp | https://www.google.com/maps/search/{query} | |
| | Translate | translate | tr | https://translate.google.com/?sl=auto&op=translate&text={query} | |
| Code | GitHub | github | gh | https://github.com/search?q={query} | |
| | Stack Overflow | stackoverflow | st | https://stackoverflow.com/search?q={query} | |
| | MDN | mdn | mdn | https://developer.mozilla.org/search?q={query} | |
| | docs.rs | docsrs | rs | https://docs.rs/releases/search?query={query} | |
| Shopping | Amazon | amazon | amz | https://www.amazon.com/s?k={query} | |
| | eBay | ebay | eb | https://www.ebay.com/sch/i.html?_nkw={query} | |
| Social | Reddit | reddit | rd | https://www.reddit.com/search/?q={query} | |
| | X | x | xx | https://x.com/search?q={query} | |
| | LinkedIn | linkedin | li | https://www.linkedin.com/search/results/all/?keywords={query} | |

Rulings:
- Shortcuts are 2–3 letters and never a common word, so "so good" or "b movies" never switch engines. Built-in shortcuts are fixed.
- English/US sites for Wikipedia and Amazon; other countries are a custom engine away.
- Google Maps and Google Translate are named "Maps" and "Translate" so six chips fit the bar.
- `{query}` is replaced by the trimmed text, percent-encoded (only `A–Z a–z 0–9 - . _ ~` stay as they are; a space becomes `%20`). This is valid both in a query string and in a path (Spotify, Maps).
- Type-ahead is added for Wikipedia (`https://en.wikipedia.org/w/api.php?action=opensearch&limit=8&search=…`) and DuckDuckGo (`https://duckduckgo.com/ac/?type=list&q=…`); both answer in the same `["query", ["suggestion", …]]` shape Google uses. Bing and Amazon are left out: their services need keys or change often.

## Settings file

```toml
engines = ["google", "youtube", "files", "wikipedia", "chatgpt"]

[[custom_engines]]
name = "Thingiverse"
url = "https://www.thingiverse.com/search?q={query}"
shortcut = "tv"
```

- A custom engine's id is `custom:<shortcut>`.
- On load the chip list is cleaned: unknown ids and repeats dropped, cut to 6; an empty result falls back to the default list. *Ruling*: a settings file from 0.2.0 (no `engines` line) gets the default list, so existing users see Wikipedia and ChatGPT added after their three.
- `default_mode` (the engine the bar opens in) stays; if it is not one of the chips, the first chip is used.
- Custom engine rules: name 1–20 characters; address starts with `https://`, contains `{query}`, and is a valid address once `{query}` is filled in; shortcut 2–5 lowercase letters or digits, not used by any other engine.

## Behaviour in the bar

- Tab / Shift+Tab cycle through the chips only. From an engine reached by shortcut, Tab goes to the first chip and Shift+Tab to the last.
- Shortcut: when the text starts with a shortcut followed by a space, the bar switches to that engine and the shortcut is removed from the text. An engine that is not a chip shows as an extra outlined chip while in use. Backspace in the empty box right after a switch goes back to the previous engine with the shortcut text restored (without the space). Hiding the bar returns to the default engine.
- Chip row: chips wrap to a second line if they do not fit. Hovering a chip shows a small × badge in its corner (not on the last chip); clicking the badge removes it. Dragging a chip and releasing it moves it; a line marks where it will land.
- "+" chip: opens the list in the results area (scrolls after about 12 rows). With an empty search box: groups with headers, then "Your own" (custom engines, each with a delete button), then "+ Add your own…". With text: a flat list of engines whose name or shortcut contains it. Engines already on the bar are greyed with ✓. Arrow keys and Enter, or a click, add the selected engine at the end. Esc closes the list (the bar stays open). At 6 chips the list says "6 of 6: remove an engine first" and nothing can be added.
- Form: three fields, Tab moves between them, Enter adds, Esc goes back to the list. Errors show under the fields in plain words. Adding a custom engine also puts it on the bar when there is room.
- Every change is saved to the settings file at once and the bar is rebuilt with the new engines.

## Structure

- `flick-core/src/catalog.rs` (new): the catalog table, groups, lookup by id and shortcut, custom-engine checks, and the `{query}` filling.
- `flick-core/src/config.rs`: `engines`, `custom_engines`, the cleaned chip list, limits (`MIN_ENGINES = 1`, `MAX_ENGINES = 6`) and add / remove / move operations; `build_registry_with` registers every engine and returns the chips and shortcuts.
- `flick-core/src/modes/web.rs`: one `WebSearchMode` built from an id, name, address template and optional type-ahead engine.
- `flick-core/src/modes/suggest.rs`: Wikipedia and DuckDuckGo type-ahead.
- `flick-core/src/launcher.rs`: chips, Tab over chips, shortcuts, Backspace-back.
- `flick-app/src/engine_picker.rs` (new): picker state, list building and filtering, the form, and drawing; `ui.rs` routes keys and the search box to it while open.
- `flick-app/src/ui.rs`: chip row with remove badge, dragging, the "+" chip and the extra chip.

## Testing

Unit tests for the catalog (unique ids and shortcuts, every address has `{query}`), URL filling, config cleaning and limits, custom-engine checks, launcher shortcuts and Tab, type-ahead addresses, the picker's list and filter, and the drop position while dragging. The look and the mouse interactions are checked by the owner by hand.
