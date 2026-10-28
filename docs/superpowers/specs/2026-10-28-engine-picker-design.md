# Engine picker — design

Date: 2026-10-28. Status: decided (the owner asked to build without further questions; decisions below marked *ruling* were made by Claude).

## Goal

People choose which search engines appear as chips in the bar (1 to 5, default 5), order them by dragging, add their own engines. Apps mode (launching installed programs) is a separate, later project and is not part of this design.

## Agreed with the owner

- Picker lives on the chip row (option B): hover a chip for a remove badge, drag chips to reorder, a "+" chip opens an engine list in the results area, filtered by what is typed in the search box. At 5 chips adding is off; the last chip cannot be removed.
- "Add your own" is a small form in the same space: name and search address with `{query}`.
- Default chips: Google, YouTube, Files, Wikipedia, ChatGPT (*ruling*: in that order, so existing users keep their first three in the same place).
- Approach: one generic web engine fed by a built-in catalog table; custom engines are extra rows stored in the settings file.

## Catalog

| Group | Engine | id | Address | Type-ahead |
|---|---|---|---|---|
| On this PC | Files | files | (local) | |
| Web search | Google | google | https://www.google.com/search?q={query} | Google |
| | DuckDuckGo | duckduckgo | https://duckduckgo.com/?q={query} | DuckDuckGo |
| | Bing | bing | https://www.bing.com/search?q={query} | |
| | Brave | brave | https://search.brave.com/search?q={query} | |
| AI | ChatGPT | chatgpt | https://chatgpt.com/?q={query} | |
| | Claude | claude | https://claude.ai/new?q={query} | |
| | Perplexity | perplexity | https://www.perplexity.ai/search?q={query} | |
| Video & music | YouTube | youtube | https://www.youtube.com/results?search_query={query} | YouTube |
| | Spotify | spotify | https://open.spotify.com/search/{query} | |
| | Twitch | twitch | https://www.twitch.tv/search?term={query} | |
| Knowledge | Wikipedia | wikipedia | https://en.wikipedia.org/w/index.php?search={query} | Wikipedia |
| | Maps | maps | https://www.google.com/maps/search/{query} | |
| | Translate | translate | https://translate.google.com/?sl=auto&op=translate&text={query} | |
| Code | GitHub | github | https://github.com/search?q={query} | |
| | Stack Overflow | stackoverflow | https://stackoverflow.com/search?q={query} | |
| | MDN | mdn | https://developer.mozilla.org/search?q={query} | |
| | docs.rs | docsrs | https://docs.rs/releases/search?query={query} | |
| Shopping | Amazon | amazon | https://www.amazon.com/s?k={query} | |
| | eBay | ebay | https://www.ebay.com/sch/i.html?_nkw={query} | |
| Social | Reddit | reddit | https://www.reddit.com/search/?q={query} | |
| | X | x | https://x.com/search?q={query} | |
| | LinkedIn | linkedin | https://www.linkedin.com/search/results/all/?keywords={query} | |

Rulings:
- English/US sites for Wikipedia and Amazon; other countries are a custom engine away.
- Google Maps and Google Translate are named "Maps" and "Translate" so the chips fit the bar.
- `{query}` is replaced by the trimmed text, percent-encoded (only `A–Z a–z 0–9 - . _ ~` stay as they are; a space becomes `%20`). This is valid both in a query string and in a path (Spotify, Maps).
- Type-ahead is added for Wikipedia (`https://en.wikipedia.org/w/api.php?action=opensearch&limit=8&search=…`) and DuckDuckGo (`https://duckduckgo.com/ac/?type=list&q=…`); both answer in the same `["query", ["suggestion", …]]` shape Google uses. Bing and Amazon are left out: their services need keys or change often.

## Settings file

```toml
engines = ["google", "youtube", "files", "wikipedia", "chatgpt"]

[[custom_engines]]
name = "Thingiverse"
url = "https://www.thingiverse.com/search?q={query}"
```

- A custom engine's id is `custom:` and its name in lowercase, with a dash for each run of other characters (`custom:thingiverse`).
- On load the chip list is cleaned: unknown ids and repeats dropped, cut to 5; an empty result falls back to the default list. *Ruling*: a settings file from 0.2.0 (no `engines` line) gets the default list, so existing users see Wikipedia and ChatGPT added after their three.
- `default_mode` (the engine the bar opens in) stays; if it is not one of the chips, the first chip is used.
- Custom engine rules: name 1–20 characters; address starts with `https://`, contains `{query}`, and is a valid address once `{query}` is filled in; no two of the person's engines share an id.

## Behaviour in the bar

- Tab / Shift+Tab cycle through the chips. Hiding the bar returns to the default engine.
- Chip row: chips wrap to a second line if they do not fit. Hovering a chip shows a small × badge in its corner (not on the last chip); clicking the badge removes it. Dragging a chip and releasing it moves it; a line marks where it will land.
- "+" chip: opens the list in the results area, as tall as a full page of results (`max_results` rows); it scrolls beyond that, with an always-visible scrollbar. Only the arrow keys scroll the selection into view, so the mouse wheel is never pulled back. With an empty search box: groups with headers, then "Your own" (custom engines, each with a delete button), then "+ Add your own…". With text: a flat list of engines whose name contains it. Engines already on the bar are greyed with ✓. Arrow keys and Enter, or a click, add the selected engine at the end. Esc closes the list (the bar stays open). At 5 chips the list says "5 of 5: remove an engine first" and nothing can be added.
- Form: three fields, Tab moves between them, Enter adds, Esc goes back to the list. Errors show under the fields in plain words. Adding a custom engine also puts it on the bar when there is room.
- Every change is saved to the settings file at once and the bar is rebuilt with the new engines.

## Structure

- `flick-core/src/catalog.rs` (new): the catalog table, groups, lookup by id, custom-engine checks, and the `{query}` filling.
- `flick-core/src/config.rs`: `engines`, `custom_engines`, the cleaned chip list, limits (`MIN_ENGINES = 1`, `MAX_ENGINES = 6`) and add / remove / move operations; `build_registry_with` registers every engine and returns the chips.
- `flick-core/src/modes/web.rs`: one `WebSearchMode` built from an id, name, address template and optional type-ahead engine.
- `flick-core/src/modes/suggest.rs`: Wikipedia and DuckDuckGo type-ahead.
- `flick-core/src/launcher.rs`: chips and Tab over chips.
- `flick-app/src/engine_picker.rs` (new): picker state, list building and filtering, the form, and drawing; `ui.rs` routes keys and the search box to it while open.
- `flick-app/src/ui.rs`: chip row with remove badge, dragging, the "+" chip and the extra chip.

## Testing

Unit tests for the catalog (unique ids, every address has `{query}`), URL filling, config cleaning and limits, custom-engine checks, launcher Tab, type-ahead addresses, the picker's list and filter, and the drop position while dragging. The look and the mouse interactions are checked by the owner by hand.

## Change after the first hand check

The owner asked for shortcuts to go: no shortcut labels in the list, no typed shortcuts, and no shortcut field in the "Add your own" form (they had asked for shortcut labels to be removed once before, for 0.1). The list was also made as tall as a full page of results, with a wider scrollbar that is always shown, and it no longer jumps back to the selected row while the mouse wheel scrolls it.

After trying it, the owner lowered the most engines on the bar from six to five: six made the chip row crowded around the remove badges and the "+" chip. With the default five, the bar starts full, so adding an engine means taking one off first.
