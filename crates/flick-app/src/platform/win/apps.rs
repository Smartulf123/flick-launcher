//! Windows: the programs in Start's "All apps" list, which the shell
//! exposes as the `AppsFolder` known folder (desktop and Store apps alike).

use eframe::egui::ColorImage;
use flick_core::apps::{self, AppEntry};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItemImageFactory,
    KF_FLAG_DEFAULT, SHCreateItemFromParsingName, SHGetKnownFolderItem, SIGDN, SIGDN_NORMALDISPLAY,
    SIGDN_PARENTRELATIVEPARSING, SIIGBF_ICONONLY,
};
use windows::core::{HSTRING, PWSTR};

use super::shell_icons;

/// What the shell opens to start the app with this `AppsFolder` parsing name.
pub fn shell_target(parsing_name: &str) -> String {
    format!("shell:AppsFolder\\{parsing_name}")
}

/// Starts the app, desktop or Store, without waiting for it. Explorer is
/// the shell's own way in, and quicker than `open`'s PowerShell route.
pub fn launch(parsing_name: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("explorer.exe")
        .arg(shell_target(parsing_name))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

/// Every app in Start, filtered by `flick_core::apps::windows_apps`. Runs
/// on its own thread, so it sets COM up for itself. Empty if the shell
/// cannot be asked.
pub fn scan() -> Vec<AppEntry> {
    // SAFETY: COM setup for this thread, undone below once.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let items = list_apps_folder().unwrap_or_else(|e| {
        log::error!("listing apps: {e}");
        Vec::new()
    });
    if com.is_ok() {
        // SAFETY: pairs with the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    apps::windows_apps(items)
}

/// (display name, parsing name) of each item in `AppsFolder`.
fn list_apps_folder() -> windows::core::Result<Vec<(String, String)>> {
    // SAFETY: plain shell calls; every out-value is owned by a COM wrapper.
    unsafe {
        let folder: IShellItem = SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)?;
        let items: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems)?;
        let mut out = Vec::new();
        loop {
            let mut batch = [None];
            let mut fetched = 0;
            items.Next(&mut batch, Some(&mut fetched))?;
            let Some(item) = batch[0].take().filter(|_| fetched == 1) else {
                break;
            };
            if let (Some(name), Some(id)) = (
                display_name(&item, SIGDN_NORMALDISPLAY),
                display_name(&item, SIGDN_PARENTRELATIVEPARSING),
            ) {
                out.push((name, id));
            }
        }
        Ok(out)
    }
}

fn display_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: the shell allocates the string; we copy it and free it once.
    unsafe {
        let text: PWSTR = item.GetDisplayName(kind).ok()?;
        let name = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as *const _));
        name.filter(|n| !n.is_empty())
    }
}

/// The app's 32x32 icon, as Start shows it.
pub fn icon(parsing_name: &str) -> Option<ColorImage> {
    shell_icons::com_ready();
    // SAFETY: plain shell calls; the bitmap is ours to delete once.
    unsafe {
        let item: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(shell_target(parsing_name)), None).ok()?;
        let bitmap = item
            .GetImage(SIZE { cx: 32, cy: 32 }, SIIGBF_ICONONLY)
            .ok()?;
        let image = shell_icons::bitmap_to_image(bitmap, true);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_start_through_the_apps_folder() {
        assert_eq!(
            shell_target("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
            r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        );
    }

    #[test]
    fn start_lists_apps_and_their_icons() {
        let apps = scan();
        assert!(apps.len() > 5, "{} apps", apps.len());
        assert!(
            apps.iter()
                .all(|a| !a.name.is_empty() && !a.launch.is_empty())
        );
        let image = apps.iter().find_map(|a| icon(&a.launch)).expect("an icon");
        assert_eq!(image.size, [32, 32]);
    }

    #[test]
    #[ignore = "needs a Windows desktop with Store apps (CI runs Windows Server)"]
    fn start_lists_calculator_as_a_store_app_with_an_icon() {
        let apps = scan();
        let calculator = apps
            .iter()
            .find(|a| a.name == "Calculator")
            .expect("Calculator is in Start");
        assert!(calculator.launch.contains("!App"), "{}", calculator.launch);
        let image = icon(&calculator.launch).expect("an icon");
        assert_eq!(image.size, [32, 32]);
        assert!(image.pixels.iter().any(|p| p.a() > 0));
    }
}
