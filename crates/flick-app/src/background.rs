//! The background picture: where it comes from, loading it, and fitting it
//! to the window.

use std::path::{Path, PathBuf};

use crate::builtin_backgrounds;
use anyhow::Context as _;
use eframe::egui::{ColorImage, Rect, Vec2, pos2};
use flick_core::config::BackgroundSource;

/// Pictures wider than this are scaled down when loaded; the window is
/// 640 px wide, so this keeps memory small with room for high-DPI screens.
pub const MAX_WIDTH: u32 = 1280;

/// The part of the picture (in 0..1 texture coordinates) that fills a
/// window of `target` size without stretching, cropping the overflow evenly.
pub fn cover_uv(image: [usize; 2], target: Vec2) -> Rect {
    let image_aspect = image[0] as f32 / image[1] as f32;
    let target_aspect = target.x / target.y;
    if target_aspect > image_aspect {
        let height = image_aspect / target_aspect;
        let top = (1.0 - height) / 2.0;
        Rect::from_min_max(pos2(0.0, top), pos2(1.0, top + height))
    } else {
        let width = target_aspect / image_aspect;
        let left = (1.0 - width) / 2.0;
        Rect::from_min_max(pos2(left, 0.0), pos2(left + width, 1.0))
    }
}

/// Like `cover_uv`, but the picture is fitted once to a canvas
/// `canvas_height` tall and the window shows its top part. As the window
/// grows (up to the canvas), more of the same picture is uncovered below
/// instead of the whole picture shifting and zooming.
pub fn anchored_uv(image: [usize; 2], target: Vec2, canvas_height: f32) -> Rect {
    let canvas = Vec2::new(target.x, canvas_height.max(target.y));
    let full = cover_uv(image, canvas);
    let shown = full.height() * target.y / canvas.y;
    Rect::from_min_max(full.min, pos2(full.max.x, full.min.y + shown))
}

/// Size after scaling down to at most `max_width`, keeping the shape.
pub fn scaled_size(width: u32, height: u32, max_width: u32) -> (u32, u32) {
    if width <= max_width {
        (width, height)
    } else {
        let h = (height as f64 * max_width as f64 / width as f64).round() as u32;
        (max_width, h.max(1))
    }
}

/// Decodes a JPG, PNG or WebP (the format is read from the bytes, not a
/// file extension) and scales it down to at most `MAX_WIDTH` wide.
pub fn decode(bytes: &[u8]) -> anyhow::Result<ColorImage> {
    let picture = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()?
        .decode()
        .context("not a picture Flick can read")?;
    let (w, h) = scaled_size(picture.width(), picture.height(), MAX_WIDTH);
    let picture = if (w, h) == (picture.width(), picture.height()) {
        picture
    } else {
        picture.resize_exact(w, h, image::imageops::FilterType::Triangle)
    };
    let rgba = picture.to_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    ))
}

/// Reads and decodes a picture file.
pub fn load(path: &Path) -> anyhow::Result<ColorImage> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot open {}", path.display()))?;
    decode(&bytes).with_context(|| format!("cannot read {} as a picture", path.display()))
}

/// The picture for a background setting: `None` for no picture.
pub fn load_source(source: &BackgroundSource) -> anyhow::Result<Option<ColorImage>> {
    match source {
        BackgroundSource::None => Ok(None),
        BackgroundSource::File(path) => load(path).map(Some),
        BackgroundSource::Builtin(id) => {
            let builtin = builtin_backgrounds::find(id)
                .with_context(|| format!("no built-in background called {id:?}"))?;
            decode(builtin.bytes).map(Some)
        }
    }
}

/// Opens the Windows file picker for a picture, starting in `start`.
pub fn pick_picture(start: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose a background picture")
        .add_filter("Pictures", &["jpg", "jpeg", "png", "webp"])
        .set_directory(start)
        .pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, pos2, vec2};

    fn close(a: Rect, b: Rect) -> bool {
        (a.min - b.min).length() < 1e-4 && (a.max - b.max).length() < 1e-4
    }

    #[test]
    fn same_shape_uses_whole_picture() {
        let uv = cover_uv([1920, 1200], vec2(640.0, 400.0));
        assert!(
            close(uv, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
            "{uv:?}"
        );
    }

    #[test]
    fn wide_window_crops_top_and_bottom() {
        let uv = cover_uv([1000, 1000], vec2(640.0, 320.0));
        assert!(
            close(uv, Rect::from_min_max(pos2(0.0, 0.25), pos2(1.0, 0.75))),
            "{uv:?}"
        );
    }

    #[test]
    fn tall_window_crops_the_sides() {
        let uv = cover_uv([2000, 1000], vec2(400.0, 400.0));
        assert!(
            close(uv, Rect::from_min_max(pos2(0.25, 0.0), pos2(0.75, 1.0))),
            "{uv:?}"
        );
    }

    #[test]
    fn short_window_shows_the_top_of_the_full_crop() {
        // A square picture on a 640 x 640 canvas uses all of it; a 160 px
        // tall bar shows its top quarter.
        let uv = anchored_uv([1000, 1000], vec2(640.0, 160.0), 640.0);
        assert!(
            close(uv, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 0.25))),
            "{uv:?}"
        );
    }

    #[test]
    fn growing_window_reveals_more_without_moving_the_picture() {
        let small = anchored_uv([1920, 1080], vec2(640.0, 120.0), 520.0);
        let big = anchored_uv([1920, 1080], vec2(640.0, 400.0), 520.0);
        // Same left, right and top: only the bottom edge moves down.
        assert!((small.left() - big.left()).abs() < 1e-4);
        assert!((small.right() - big.right()).abs() < 1e-4);
        assert!((small.top() - big.top()).abs() < 1e-4);
        assert!(big.bottom() > small.bottom());
        // Pixels per point stay the same, so nothing zooms.
        assert!((small.height() / 120.0 - big.height() / 400.0).abs() < 1e-6);
    }

    #[test]
    fn window_taller_than_the_canvas_just_covers() {
        let uv = anchored_uv([2000, 1000], vec2(400.0, 400.0), 200.0);
        assert!(
            close(uv, cover_uv([2000, 1000], vec2(400.0, 400.0))),
            "{uv:?}"
        );
    }

    #[test]
    fn big_pictures_are_scaled_down_small_ones_kept() {
        assert_eq!(scaled_size(3840, 2160, MAX_WIDTH), (MAX_WIDTH, 720));
        assert_eq!(scaled_size(800, 600, MAX_WIDTH), (800, 600));
    }

    #[test]
    fn loads_a_picture_without_relying_on_its_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TranscodedWallpaper"); // no extension, like Windows'
        image::RgbImage::from_pixel(4, 2, image::Rgb([200, 10, 10]))
            .save_with_format(&path, image::ImageFormat::Png)
            .unwrap();
        let picture = load(&path).unwrap();
        assert_eq!(picture.size, [4, 2]);
        assert_eq!(picture.pixels[0], egui::Color32::from_rgb(200, 10, 10));
    }

    #[test]
    fn missing_picture_is_an_error_not_a_panic() {
        assert!(load(Path::new(r"C:\definitely\not\here.jpg")).is_err());
    }

    #[test]
    fn builtin_source_loads() {
        assert!(
            load_source(&BackgroundSource::Builtin("great-wave".into()))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn unknown_builtin_is_an_error() {
        assert!(load_source(&BackgroundSource::Builtin("nope".into())).is_err());
    }

    #[test]
    fn none_loads_nothing() {
        assert!(load_source(&BackgroundSource::None).unwrap().is_none());
    }
}
