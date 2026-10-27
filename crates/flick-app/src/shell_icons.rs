//! Real file and folder icons from the Windows shell, plus Windows' name for
//! each file type ("Microsoft Word Document"), cached per type.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

static SHELL_LOCK: Mutex<()> = Mutex::new(());

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HGDIOBJ, ReleaseDC,
};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_TYPENAME, SHGFI_USEFILEATTRIBUTES,
    SHGetFileInfoW,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::HSTRING;

/// Files whose icon is their own (apps, shortcuts) rather than their type's.
const OWN_ICON: [&str; 4] = ["exe", "lnk", "ico", "url"];

/// The cache key for a path's icon.
pub fn icon_key(path: &Path, is_dir: bool) -> String {
    if is_dir {
        return "<folder>".into();
    }
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()) {
        None => "<file>".into(),
        Some(ext) if OWN_ICON.contains(&ext.as_str()) => path.to_string_lossy().to_lowercase(),
        Some(ext) => format!(".{ext}"),
    }
}

/// Windows bitmaps are BGRA; egui wants RGBA. Old icons carry no alpha at
/// all, which would make them invisible, so those become opaque.
pub fn bgra_to_rgba(pixels: &mut [u8]) {
    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    for p in pixels.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        }
    }
}

pub struct ShellIcon {
    pub image: ColorImage,
    pub type_name: String,
}

thread_local! {
    /// SHGetFileInfo needs COM on the calling thread. The UI thread already
    /// has it (winit sets it up); this covers any other thread, once.
    static COM_READY: () = {
        // SAFETY: plain COM setup; "already initialized" results are fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    };
}

/// Asks the shell for the 32x32 icon and type name. Per-type icons are
/// looked up by extension only, so OneDrive files are never downloaded.
pub fn lookup(path: &Path, is_dir: bool) -> Option<ShellIcon> {
    COM_READY.with(|_| ());
    // The shell's icon list misbehaves when two threads use it at once
    // (success with no icon). One lookup at a time, process-wide.
    let _one_at_a_time = SHELL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let own_icon = !is_dir && icon_key(path, false).contains('\\');
    let (attributes, mut flags) = if is_dir {
        (FILE_ATTRIBUTE_DIRECTORY, SHGFI_USEFILEATTRIBUTES)
    } else if own_icon {
        (FILE_ATTRIBUTE_NORMAL, Default::default())
    } else {
        (FILE_ATTRIBUTE_NORMAL, SHGFI_USEFILEATTRIBUTES)
    };
    flags |= SHGFI_ICON | SHGFI_LARGEICON | SHGFI_TYPENAME;

    let wide_path = HSTRING::from(path.as_os_str());
    let mut info = SHFILEINFOW::default();
    // The shell can report success yet hand back no icon while another
    // thread is using its icon list; one retry covers that.
    for _ in 0..2 {
        info = SHFILEINFOW::default();
        // SAFETY: `info` is a valid, correctly sized out-parameter.
        let ok = unsafe {
            SHGetFileInfoW(
                &wide_path,
                attributes,
                Some(&mut info),
                size_of::<SHFILEINFOW>() as u32,
                flags,
            )
        };
        if ok != 0 && !info.hIcon.is_invalid() {
            break;
        }
    }
    if info.hIcon.is_invalid() {
        return None;
    }
    let image = icon_to_image(info.hIcon);
    // SAFETY: the shell handed us this icon; we own it and free it once.
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    let name_len = info.szTypeName.iter().position(|&c| c == 0).unwrap_or(0);
    Some(ShellIcon {
        image: image?,
        type_name: String::from_utf16_lossy(&info.szTypeName[..name_len]),
    })
}

/// Copies an icon's color bitmap into an egui image.
fn icon_to_image(icon: HICON) -> Option<ColorImage> {
    let mut info = ICONINFO::default();
    // SAFETY: GetIconInfo fills `info` and gives us two bitmaps to delete.
    unsafe { GetIconInfo(icon, &mut info).ok()? };
    let free_bitmaps = || unsafe {
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
    };
    if info.hbmColor.is_invalid() {
        free_bitmaps();
        return None;
    }

    let mut bitmap = BITMAP::default();
    // SAFETY: `bitmap` is the right size for a BITMAP description.
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(info.hbmColor.0),
            size_of::<BITMAP>() as i32,
            Some(std::ptr::from_mut(&mut bitmap).cast()),
        )
    };
    let (w, h) = (bitmap.bmWidth, bitmap.bmHeight);
    if got == 0 || w <= 0 || h <= 0 {
        free_bitmaps();
        return None;
    }

    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // negative: rows top to bottom
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    // SAFETY: `pixels` holds exactly w*h 32-bit pixels as described by `header`.
    let lines = unsafe {
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            info.hbmColor,
            0,
            h as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        lines
    };
    free_bitmaps();
    if lines == 0 {
        return None;
    }
    bgra_to_rgba(&mut pixels);
    Some(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        &pixels,
    ))
}

/// Icon textures and type names, looked up once per key.
#[derive(Default)]
pub struct IconCache {
    entries: HashMap<String, Option<(TextureHandle, String)>>,
}

impl IconCache {
    /// The texture and type name for `path`, or `None` if Windows has no icon.
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        is_dir: bool,
    ) -> Option<&(TextureHandle, String)> {
        let key = icon_key(path, is_dir);
        self.entries
            .entry(key.clone())
            .or_insert_with(|| {
                let icon = lookup(path, is_dir)?;
                let texture = ctx.load_texture(key, icon.image, TextureOptions::LINEAR);
                Some((texture, icon.type_name))
            })
            .as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_shared_per_type_except_apps_and_shortcuts() {
        assert_eq!(icon_key(Path::new(r"C:\a\Report.DOCX"), false), ".docx");
        assert_eq!(icon_key(Path::new(r"C:\a\notes"), false), "<file>");
        assert_eq!(icon_key(Path::new(r"C:\a\Budgets"), true), "<folder>");
        assert_eq!(
            icon_key(Path::new(r"C:\Apps\Tool.EXE"), false),
            r"c:\apps\tool.exe"
        );
        assert_eq!(
            icon_key(Path::new(r"C:\Desk\Game.lnk"), false),
            r"c:\desk\game.lnk"
        );
    }

    #[test]
    fn bgra_becomes_rgba() {
        let mut px = vec![1, 2, 3, 255, 10, 20, 30, 128];
        bgra_to_rgba(&mut px);
        assert_eq!(px, vec![3, 2, 1, 255, 30, 20, 10, 128]);
    }

    #[test]
    fn icons_without_alpha_become_opaque() {
        let mut px = vec![1, 2, 3, 0, 4, 5, 6, 0];
        bgra_to_rgba(&mut px);
        assert_eq!(px, vec![3, 2, 1, 255, 6, 5, 4, 255]);
    }

    #[test]
    fn shell_gives_an_icon_and_type_name_for_a_text_file() {
        let icon = lookup(Path::new(r"C:\nowhere\notes.txt"), false).unwrap();
        assert_eq!(icon.image.size, [32, 32]);
        assert!(!icon.type_name.is_empty());
        assert!(icon.image.pixels.iter().any(|p| p.a() > 0));
    }

    #[test]
    fn shell_gives_a_folder_icon() {
        let icon = lookup(Path::new(r"C:\Windows"), true).unwrap();
        assert_eq!(icon.image.size, [32, 32]);
        assert!(!icon.type_name.is_empty());
    }
}
