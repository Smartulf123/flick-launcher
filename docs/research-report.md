# A Rust keyboard launcher for Windows 11

Checked on 26 October 2026, for your own use (and as a way to learn Rust).

## Summary
**Building for yourself: tools that already exist and what to build with**

Four launchers already put a hotkey search bar on Windows with Rust inside, but none matches your exact mix. [look](https://github.com/kunkka19xx/look) (1.2k stars) is the closest all-Rust, no-webview option, yet it has no mode or plugin system and treats web search as a Ctrl+Enter action. [Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher), your model, already does every feature, just in C#. Recommended stack: egui/eframe for the window, global-hotkey and tray-icon for the background process, nucleo for fuzzy matching, walkdir first and the Everything IPC crate or an NTFS reader later for files, and `open` to hand URLs to Brave.

Confidence in this check is low because GitHub rate-limited part of the search, so a small Rust launcher may have been missed.

## Tools that might save you time
| Project | Type | Stars or users | Last update | License | Match | What it does |
|---|---|---|---|---|---|---|
| [look](https://github.com/kunkka19xx/look) | repo | 1.2k stars | today | GPL-3.0 | direct | Alt+Space launcher for apps, files and folders; Rust core, GPU-drawn gpui UI on Windows, no browser engine. No plugin model. |
| [ZeroLaunch-rs](https://github.com/ghost-him/ZeroLaunch-rs) | repo | 575 stars | 2 days ago | GPL-3.0 | direct | Alt+Space launcher with typo-tolerant fuzzy ranking and prefix plugins (including Everything file search). UI is Tauri + Vue. |
| [ProtonSearch](https://github.com/PranshulSoni/protonsearch) | repo | 147 stars | 2 days ago | MIT | direct | Native Rust/Win32 Alt+Space launcher with site-search quicklinks and file search via SQLite FTS5; much broader scope (OCR, Git, clipboard, AI). |
| [Velocmd](https://github.com/YashvardhanG/Velocmd) | repo | 24 stars | August 2026 | GPL-3.0 | direct | Rust in-memory file indexer (jwalk, windows-rs) with /files, /folders and /google chips; Tauri UI. |
| [PowerToys (Command Palette)](https://github.com/microsoft/PowerToys) | repo | 140k stars | today | MIT | partial | Microsoft's free launcher for apps, files and web search, inside a large .NET suite. |
| [Wox](https://github.com/Wox-launcher/Wox) | repo | 27.6k stars | today | GPL-3.0 | partial | Most-starred open-source launcher, native GPU UI, plugin store; about 150 MB memory per its README. |
| [Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher) | repo | 15.9k stars | yesterday | MIT | partial | Your reference: hotkey bar, web searches, Everything-backed file search, plugins. Written in C#. |
| [Everything](https://www.voidtools.com/) | app | widely used | n/a | freeware | partial | Instant file-name search over NTFS, with an SDK. Not a launcher, but the hardest part of your Files mode. |
| [Raycast for Windows](https://www.raycast.com/windows) | app | public beta | n/a | proprietary | partial | Polished launcher with quicklinks and its own file indexer; closed source, account-based. |
| [Takeoff](https://github.com/akiraeng/takeoff-launcher) | repo | 39 stars | September 2026 | MIT | partial | C++/Win32 launcher at about 10 MB RAM with g/yt keywords and Tab to switch engines. Apps only, no files. |

Also worth reading for structure, though Linux-only: [anyrun](https://github.com/anyrun-org/anyrun) (Rust, plugins need four functions), [zlaunch](https://github.com/zortax/zlaunch) (Rust + GPUI with Google/YouTube search, MIT) and [Onagre](https://github.com/onagre-launcher/onagre) (Rust + iced with prefix-routed plugins, MIT). [Listary](https://listary.com/) is another free file-first Windows launcher.

## Gaps
1. No all-Rust, native-UI Windows launcher found combines Tab or prefix-switched Google and YouTube modes, fuzzy file search and a pluggable mode trait in one small app. look lacks a plugin model, ProtonSearch uses full-text search rather than fuzzy matching, and ZeroLaunch-rs and Velocmd render in a webview. Evidence: [look](https://github.com/kunkka19xx/look), [ProtonSearch](https://github.com/PranshulSoni/protonsearch), [ZeroLaunch-rs](https://github.com/ghost-him/ZeroLaunch-rs), [Velocmd](https://github.com/YashvardhanG/Velocmd). Checked: their READMEs.
2. The popular open launchers are heavier than your memory target. Wox states about 150 MB, and Flow Launcher has an open bug with 12 upvotes about [triggering excessive events](https://github.com/Flow-Launcher/Flow.Launcher/issues/3817). [Takeoff](https://github.com/akiraeng/takeoff-launcher) shows about 10 MB is reachable natively. Checked: Wox README, Flow Launcher issues, Takeoff README.

## Recommended stack
| Layer | Top pick | Popularity | License | Alternative | Why it fits this project |
|---|---|---|---|---|---|
| Native GUI | [egui + eframe](https://github.com/emilk/egui) | 31.5k stars | Apache-2.0 | [iced](https://github.com/iced-rs/iced) | Pure Rust, repaints only on input so an idle launcher uses no CPU; immediate mode is the easiest model while learning. |
| Windows bindings | [windows-rs](https://github.com/microsoft/windows-rs) | 12.9k stars | Apache-2.0 | none maintained | Microsoft's official Win32/COM/WinRT bindings, for window styles, icons and raw volume access. |
| Hotkey and tray | [global-hotkey](https://github.com/tauri-apps/global-hotkey) + [tray-icon](https://github.com/tauri-apps/tray-icon) | 266 and 413 stars | Apache-2.0 | none needed | Standalone Rust crates (no Tauri web UI) that share one event-loop model. |
| File search | [walkdir](https://github.com/BurntSushi/walkdir) | 1.6k stars | Unlicense/MIT | [ib-everything](https://github.com/Chaoses-Ib/ib-everything) | Index Documents, Desktop and Downloads first; switch to the Everything IPC crate for whole-PC search. |
| Fuzzy matching | [nucleo](https://github.com/helix-editor/nucleo) | 1.5k stars | MPL-2.0 | [frizbee](https://github.com/saghen/frizbee) | fzf-style scoring that ranks abbreviations like "bdgt" well; about 6x faster than skim per its README. |
| Config | [toml](https://github.com/toml-rs/toml) + serde | 1.1k stars | Apache-2.0 | [confy](https://github.com/rust-cli/confy) | Typed Config struct; toml_edit can rewrite the hotkey without losing the user's comments. |
| Testing | [insta](https://github.com/mitsuhiko/insta) | 3.0k stars | Apache-2.0 | [cargo-nextest](https://github.com/nextest-rs/nextest) | Snapshot tests for "query to ranked results" and prefix parsing. |
| Packaging | [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) | 498 stars | Apache-2.0 | [dist](https://github.com/axodotdev/cargo-dist) | NSIS and MSI installers plus an updater crate; dist fits later for tag-driven GitHub Releases. |

Watch out for:
- egui: its README does not cover borderless, always-on-top or centered windows. Check eframe's viewport options on docs.rs, and measure startup and memory with the wgpu and glow renderers.
- iced: its README calls it experimental, and the API changes between releases, so AI-written code often targets an old version.
- global-hotkey: create the manager on the event-loop thread. Alt+Space collides with the Windows window menu and PowerToys, so keep the hotkey configurable.
- nucleo: MPL-2.0 only applies to nucleo's own files if you modify them; using it as a dependency is unrestricted.

## Skills for your coding agent
| Skill | Helps with | Installs or stars | Why |
|---|---|---|---|
| [rust-best-practices](https://github.com/apollographql/skills/tree/HEAD/skills/rust-best-practices) | idiomatic Rust | 21k installs | Borrowing over cloning, no unwrap outside tests, thiserror/anyhow, Clippy. Keeps the agent from teaching you bad habits. |
| [egui-agent-skills](https://github.com/gregorycarnegie/egui-agent-skills) | egui code and UI design | 22 installs | Fixes egui 0.36 APIs models get wrong, plus layout, colours and keyboard use for egui screens. Unofficial and very new. |
| [Superpowers](https://github.com/obra/superpowers) (test-driven-development) | workflow | 395k installs | Red/green/refactor for prefix parsing, mode switching and ranking. Already in your Claude Code skill list. |

## Building blocks for the features
- **Fast whole-PC file index:** [ntfs](https://github.com/ColinFinck/ntfs), Use. Safe-Rust NTFS reader up to Windows 11, the all-Rust route to an Everything-style index. Watch out: you write the index and USN live updates yourself, and raw volume access usually needs admin rights. (603 stars, Apache-2.0)
- **Fast whole-PC file index:** [OmniSearch](https://github.com/Eul45/omni-search), Learn from. Shows the full flow: MFT scan, then a USN journal watcher to stay live. Watch out: the engine is C++, so port the ideas. (609 stars, MIT)
- **Fast whole-PC file index:** [ib-everything](https://github.com/Chaoses-Ib/ib-everything), Use. Rust port of the Everything IPC SDK, sync and async, no DLL. Watch out: needs Everything running; one maintainer. (19 stars, MIT)
- **Opening URLs and files:** [open](https://github.com/Byron/open-rs), Use. `open::that()` sends search URLs to Brave as the default browser and files to their associated app. (405 stars, MIT)
- **Opening URLs and files:** [webbrowser](https://github.com/amodm/webbrowser-rs), Use. URL-only, guarantees a browser opens; a hardened mode refuses non-http(s) links. (329 stars, Apache-2.0)
- **Pluggable search modes:** [ZeroLaunch-rs](https://github.com/ghost-him/ZeroLaunch-rs), Learn from. Its README documents prefix-triggered inline plugins, full-window panel plugins and a Rust Plugin trait (init, query, execute_action). Watch out: GPL-3.0 app; its plugin SDK crates are Apache-2.0. (575 stars, GPL-3.0)
- **Pluggable search modes:** [look](https://github.com/kunkka19xx/look), Learn from. docs/architecture.md shows how a Rust core and a native gpui shell fit together on Windows. Watch out: GPL-3.0, read for ideas only. (1.2k stars, GPL-3.0)
- **Native popup window:** [Slint](https://github.com/slint-ui/slint), Use. Third GUI option, designed for minimal memory. Watch out: GPL-3.0 unless you take its royalty-free license, and a second markup language to learn. (24.2k stars, GPL-3.0 or royalty-free)

## Products worth studying
- [Flow Launcher](https://flowlauncher.com): look at the action-keyword flow, where a keyword plus a space switches results to one plugin. Install it, try web search and Everything file search, and read its MIT-licensed plugin docs for the provider contract.
- [Takeoff](https://github.com/akiraeng/takeoff-launcher): look at Tab switching between g and yt engines, and how a Win32 window stays near 10 MB. Run it, measure memory in Task Manager, and read the MIT-licensed C++ for hotkey and show/hide handling.
- [Raycast for Windows](https://www.raycast.com/windows): look at the root search bar, how app, file and quicklink results mix, and its focus and dismiss behaviour. Use the free tier and screenshot the flows.

Learn the patterns and make them your own; code, images, icons and a look-alike design stay the owner's, and many apps' terms forbid reverse engineering.

## Next step
Spend an evening with Flow Launcher and look side by side, and write down what you would change. If you still want your own, start with the smallest loop: an eframe window that a global hotkey shows and Esc hides, with the Google mode, then add YouTube, then Files.

## Sources
https://github.com/kunkka19xx/look
https://github.com/ghost-him/ZeroLaunch-rs
https://github.com/PranshulSoni/protonsearch
https://github.com/YashvardhanG/Velocmd
https://github.com/microsoft/PowerToys
https://www.computerworld.com/article/3956444/windows-start-menu-command-palette.html
https://github.com/Wox-launcher/Wox
https://github.com/Flow-Launcher/Flow.Launcher
https://github.com/Flow-Launcher/Flow.Launcher/issues/3817
https://flowlauncher.com
https://www.voidtools.com/
https://www.raycast.com/windows
https://listary.com/
https://github.com/akiraeng/takeoff-launcher
https://github.com/Xoshbin/asyar
https://github.com/anyrun-org/anyrun
https://github.com/zortax/zlaunch
https://github.com/onagre-launcher/onagre
https://github.com/emilk/egui
https://github.com/iced-rs/iced
https://github.com/slint-ui/slint
https://github.com/microsoft/windows-rs
https://github.com/tauri-apps/global-hotkey
https://github.com/tauri-apps/tray-icon
https://github.com/BurntSushi/walkdir
https://github.com/Chaoses-Ib/ib-everything
https://github.com/ColinFinck/ntfs
https://github.com/Eul45/omni-search
https://github.com/helix-editor/nucleo
https://github.com/saghen/frizbee
https://github.com/toml-rs/toml
https://github.com/rust-cli/confy
https://github.com/mitsuhiko/insta
https://github.com/nextest-rs/nextest
https://github.com/crabnebula-dev/cargo-packager
https://github.com/axodotdev/cargo-dist
https://github.com/Byron/open-rs
https://github.com/amodm/webbrowser-rs
https://github.com/apollographql/skills/tree/HEAD/skills/rust-best-practices
https://github.com/gregorycarnegie/egui-agent-skills
https://github.com/obra/superpowers
