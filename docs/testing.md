# Testing Flick by hand

Automatic tests run on Windows, macOS and Ubuntu for every push (GitHub Actions, workflow **CI**). They cannot press keys or look at the screen, so these checks are done by a person.

## Skeleton check (macOS and Ubuntu)

A first build with the window, the hotkey and the tray icon. Start at login and installers come later.

### Getting the program

1. Open the latest **CI** run on the `feat/cross-platform` branch: <https://github.com/Smartulf123/flick-launcher/actions/workflows/ci.yml?query=branch%3Afeat%2Fcross-platform>
2. Under **Artifacts**, download `flick-macos` or `flick-linux` (you need to be signed in to GitHub; the repo owner can download it and pass the file on).
3. Unzip it. The file inside is the whole program.

### macOS

In Terminal, in the folder with the file:

```bash
chmod +x flick-macos
xattr -d com.apple.quarantine flick-macos
./flick-macos
```

Check:

- [ ] A small circle appears in the menu bar (white or black to match the menu bar). There is no Dock icon.
- [ ] **Option+Space** shows the bar in the middle of the screen, and you can type straight away (no click needed).
- [ ] Typing shows Google suggestions; Enter opens the browser.
- [ ] Tab switches to YouTube and Files; Files finds something on your Desktop.
- [ ] **Esc** hides the bar; clicking another window hides it too.
- [ ] The gear opens the menu inside the bar (Browse backgrounds, Choose your own picture, None, Change hotkey).
- [ ] **Change hotkey...** works: press a new combination, Enter, and the new keys show the bar.
- [ ] Running `./flick-macos` a second time does not start a second Flick; it shows the bar.
- [ ] The menu-bar icon's menu works, and **Quit** quits.

### Ubuntu (GNOME, Wayland: the normal Ubuntu login)

In a terminal, in the folder with the file:

```bash
chmod +x flick-linux
./flick-linux
```

Then bind a key to it (this becomes automatic in a later step): **Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts → +**

- Name: `Flick`
- Command: the full path to the file followed by `--toggle`, for example `/home/you/Downloads/flick-linux --toggle`
- Shortcut: **Ctrl+Alt+Space**

Check:

- [ ] A small circle appears in the top bar's tray area.
- [ ] **Ctrl+Alt+Space** shows the bar in the middle of the screen, and you can type straight away. **Important:** note if GNOME shows a "Flick is ready" notice instead of the bar, or if the bar appears but typing goes to the previous window.
- [ ] Pressing **Ctrl+Alt+Space** again hides it.
- [ ] Typing shows Google suggestions; Enter opens the browser.
- [ ] Tab switches to YouTube and Files; Files finds something in your home folders.
- [ ] **Esc** hides the bar; clicking another window hides it too.
- [ ] The gear opens the menu inside the bar.
- [ ] Running `./flick-linux` a second time does not start a second Flick.
- [ ] The tray icon's menu works, and **Quit** quits.

### What to send back

For each item that fails: what you did, what happened instead, and a screenshot if it is about the look. Also say which macOS or Ubuntu version you used.
