//! Linux on Wayland: apps cannot grab a global key there, so on GNOME
//! Flick adds a custom keyboard shortcut that runs `flick --toggle`. This
//! file decides when that applies and what to tell `gsettings`.

use std::path::Path;

/// GNOME's keyboard-shortcut settings.
pub const SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
/// Flick's own custom shortcut in those settings.
pub const PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/flick/";

/// What kind of desktop session Flick runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    /// The hotkey is grabbed directly, as on Windows.
    X11,
    /// No global keys for apps; `gnome` says whether Flick can add a
    /// GNOME shortcut itself.
    Wayland { gnome: bool },
}

/// Reads the session from environment variables (`var` returns one).
pub fn detect(var: impl Fn(&str) -> Option<String>) -> Session {
    let wayland = var("XDG_SESSION_TYPE").is_some_and(|t| t.eq_ignore_ascii_case("wayland"))
        || var("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty());
    if !wayland {
        return Session::X11;
    }
    let gnome = var("XDG_CURRENT_DESKTOP")
        .is_some_and(|desktops| desktops.split(':').any(|d| d.eq_ignore_ascii_case("gnome")));
    Session::Wayland { gnome }
}

/// A hotkey from the settings ("Ctrl+Alt+Space") as GNOME writes it
/// ("<Control><Alt>space"); `None` if Flick would not accept the hotkey.
pub fn accelerator(text: &str) -> Option<String> {
    crate::hotkey::parse(text).ok()?;
    let mut out = String::new();
    for token in text.split('+').map(str::trim) {
        let modifier = match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some("<Control>"),
            "alt" | "option" => Some("<Alt>"),
            "shift" => Some("<Shift>"),
            "super" | "cmd" | "command" | "meta" => Some("<Super>"),
            _ => None,
        };
        match modifier {
            Some(m) => out.push_str(m),
            None => {
                let key = token
                    .strip_prefix("Key")
                    .or_else(|| token.strip_prefix("Digit"))
                    .filter(|rest| !rest.is_empty())
                    .unwrap_or(token);
                // F-keys keep their capital; letters and Space are lowercase.
                let is_f_key = key.len() > 1
                    && key.starts_with(['F', 'f'])
                    && key[1..].chars().all(|c| c.is_ascii_digit());
                if is_f_key {
                    out.push_str(&key.to_ascii_uppercase());
                } else {
                    out.push_str(&key.to_ascii_lowercase());
                }
            }
        }
    }
    Some(out)
}

/// The custom-shortcut list as `gsettings get` prints it, with Flick's
/// entry added; `None` if it is already there.
pub fn with_flick(list: &str) -> Option<String> {
    let list = list.trim();
    let mut items: Vec<String> = if list.starts_with("@as") {
        Vec::new()
    } else {
        list.trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|item| item.trim().trim_matches('\'').to_owned())
            .filter(|item| !item.is_empty())
            .collect()
    };
    if items.iter().any(|item| item == PATH) {
        return None;
    }
    items.push(PATH.to_owned());
    let quoted: Vec<String> = items.iter().map(|item| format!("'{item}'")).collect();
    Some(format!("[{}]", quoted.join(", ")))
}

/// `exe` quoted for the shell GNOME runs the shortcut with.
fn shell_quote(exe: &Path) -> String {
    format!("'{}'", exe.to_string_lossy().replace('\'', r"'\''"))
}

/// A GVariant string literal, as `gsettings set` reads its value.
fn gvariant_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', r"\\").replace('"', "\\\""))
}

/// The `gsettings` argument lists that make Flick's shortcut run
/// `exe --toggle` on `accel`, given the current shortcut `list`.
pub fn commands(exe: &Path, accel: &str, list: &str) -> Vec<Vec<String>> {
    let schema = format!("{SCHEMA}.custom-keybinding:{PATH}");
    let set =
        |key: &str, value: String| vec!["set".to_owned(), schema.clone(), key.to_owned(), value];
    let command = format!("{} --toggle", shell_quote(exe));
    let mut cmds = vec![
        set("name", "'Flick'".to_owned()),
        set("command", gvariant_string(&command)),
        set("binding", format!("'{accel}'")),
    ];
    if let Some(list) = with_flick(list) {
        cmds.push(vec![
            "set".to_owned(),
            SCHEMA.to_owned(),
            "custom-keybindings".to_owned(),
            list,
        ]);
    }
    cmds
}

/// Writes (or updates) Flick's GNOME shortcut for the hotkey `text`.
#[cfg(target_os = "linux")]
pub fn apply(text: &str, exe: &Path) -> anyhow::Result<()> {
    use anyhow::{Context as _, bail};
    use std::process::Command;

    let accel = accelerator(text).with_context(|| format!("{text:?} is not a valid hotkey"))?;
    let out = Command::new("gsettings")
        .args(["get", SCHEMA, "custom-keybindings"])
        .output()
        .context("gsettings is not available")?;
    if !out.status.success() {
        bail!("gsettings could not read the shortcut list");
    }
    let list = String::from_utf8_lossy(&out.stdout);
    for args in commands(exe, &accel, &list) {
        let status = Command::new("gsettings")
            .args(&args)
            .status()
            .context("gsettings is not available")?;
        if !status.success() {
            bail!("gsettings {} failed", args.join(" "));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn x11_sessions_grab_the_key_themselves() {
        assert_eq!(detect(env(&[("XDG_SESSION_TYPE", "x11")])), Session::X11);
        assert_eq!(detect(env(&[])), Session::X11);
    }

    #[test]
    fn wayland_is_seen_from_either_variable() {
        assert_eq!(
            detect(env(&[("XDG_SESSION_TYPE", "wayland")])),
            Session::Wayland { gnome: false }
        );
        assert_eq!(
            detect(env(&[("WAYLAND_DISPLAY", "wayland-0")])),
            Session::Wayland { gnome: false }
        );
    }

    #[test]
    fn gnome_is_found_in_the_desktop_list() {
        let ubuntu = [
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "ubuntu:GNOME"),
        ];
        assert_eq!(detect(env(&ubuntu)), Session::Wayland { gnome: true });
        let kde = [
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "KDE"),
        ];
        assert_eq!(detect(env(&kde)), Session::Wayland { gnome: false });
    }

    #[test]
    fn hotkeys_become_gnome_accelerators() {
        assert_eq!(
            accelerator("Ctrl+Alt+Space").as_deref(),
            Some("<Control><Alt>space")
        );
        assert_eq!(
            accelerator("Ctrl+Shift+K").as_deref(),
            Some("<Control><Shift>k")
        );
        assert_eq!(accelerator("Super+1").as_deref(), Some("<Super>1"));
        assert_eq!(accelerator("F5").as_deref(), Some("F5"));
        assert_eq!(
            accelerator("shift+alt+KeyQ").as_deref(),
            Some("<Shift><Alt>q")
        );
        assert_eq!(accelerator("Banana+X"), None);
    }

    #[test]
    fn flick_is_added_to_the_shortcut_list_once() {
        assert_eq!(
            with_flick("@as []").as_deref(),
            Some(format!("['{PATH}']").as_str())
        );
        assert_eq!(
            with_flick("['/a/']\n").as_deref(),
            Some(format!("['/a/', '{PATH}']").as_str())
        );
        assert_eq!(with_flick(&format!("['/a/', '{PATH}']")), None);
    }

    #[test]
    fn commands_name_bind_and_run_flick() {
        let cmds = commands(
            Path::new("/home/a b/flick"),
            "<Control><Alt>space",
            "@as []",
        );
        let schema = format!("{SCHEMA}.custom-keybinding:{PATH}");
        assert_eq!(cmds[0], ["set", &schema, "name", "'Flick'"]);
        assert_eq!(
            cmds[1],
            ["set", &schema, "command", "\"'/home/a b/flick' --toggle\""]
        );
        assert_eq!(
            cmds[2],
            ["set", &schema, "binding", "'<Control><Alt>space'"]
        );
        assert_eq!(
            cmds[3],
            ["set", SCHEMA, "custom-keybindings", &format!("['{PATH}']")]
        );
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn a_quote_in_the_path_stays_one_argument() {
        let cmds = commands(
            Path::new("/home/o'neil/flick"),
            "F5",
            &format!("['{PATH}']"),
        );
        assert_eq!(cmds[1][3], "\"'/home/o'\\\\''neil/flick' --toggle\"");
        assert_eq!(cmds.len(), 3);
    }
}
