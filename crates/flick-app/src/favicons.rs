//! Site logos for web engines and web results, fetched once from Google's
//! favicon service (only the site's name is sent, never what is typed) and
//! kept on disk. Until a logo is there, callers draw Flick's own icon.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use flick_core::catalog::icon_domain;
use flick_core::config::Config;

use crate::icons::Icon;

/// Gets a logo's bytes for a site name, e.g. `"claude.ai"`.
type Fetch = Arc<dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync>;

/// Fetched logos waiting to become textures: (site, image or failure).
type Arrived = Arc<Mutex<Vec<(String, Option<ColorImage>)>>>;

pub struct Favicons {
    dir: PathBuf,
    fetch: Fetch,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// `None` once a site has no logo this session.
    textures: HashMap<String, Option<TextureHandle>>,
    asked: HashSet<String>,
    arrived: Arrived,
}

/// The logo address for a site, 64 pixels square where the site has one.
pub fn service_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={domain}&sz=64")
}

/// Where a site's logo is kept: only names made of letters, digits, dots
/// and dashes are used, so a name can never point outside `dir`.
pub fn cache_path(dir: &Path, domain: &str) -> Option<PathBuf> {
    let safe = !domain.is_empty()
        && !domain.starts_with('.')
        && !domain.contains("..")
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    safe.then(|| dir.join(format!("{domain}.png")))
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

impl Favicons {
    /// Logos from the web, kept in `dir`; `wake` repaints when one arrives.
    pub fn start(dir: PathBuf, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let agent = crate::http::agent(Duration::from_secs(10));
        Self::with_fetch(
            dir,
            move |domain| crate::http::get_bytes(&agent, &service_url(domain)),
            wake,
        )
    }

    pub fn with_fetch(
        dir: PathBuf,
        fetch: impl Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync + 'static,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            dir,
            fetch: Arc::new(fetch),
            wake: Arc::new(wake),
            textures: HashMap::new(),
            asked: HashSet::new(),
            arrived: Arrived::default(),
        }
    }

    /// The logo for `domain` if it is ready; otherwise starts getting it
    /// (from disk, or the web in the background) and returns `None`.
    pub fn get(&mut self, ctx: &egui::Context, domain: &str) -> Option<TextureHandle> {
        let arrived = std::mem::take(&mut *self.arrived.lock().unwrap_or_else(|e| e.into_inner()));
        for (site, image) in arrived {
            self.store(ctx, site, image);
        }
        if let Some(known) = self.textures.get(domain) {
            return known.clone();
        }
        let path = cache_path(&self.dir, domain)?;
        if let Some(image) = std::fs::read(&path).ok().and_then(|b| decode(&b)) {
            return self.store(ctx, domain.to_owned(), Some(image));
        }
        if self.asked.insert(domain.to_owned()) {
            let (fetch, wake, arrived) =
                (self.fetch.clone(), self.wake.clone(), self.arrived.clone());
            let domain = domain.to_owned();
            std::thread::spawn(move || {
                let image = fetch(&domain)
                    .inspect_err(|e| log::warn!("logo for {domain}: {e:#}"))
                    .ok()
                    .and_then(|bytes| {
                        let image = decode(&bytes)?;
                        let saved = std::fs::create_dir_all(path.parent()?)
                            .and_then(|()| std::fs::write(&path, &bytes));
                        if let Err(e) = saved {
                            log::warn!("saving logo for {domain}: {e}");
                        }
                        Some(image)
                    });
                arrived
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((domain, image));
                wake();
            });
        }
        None
    }

    fn store(
        &mut self,
        ctx: &egui::Context,
        domain: String,
        image: Option<ColorImage>,
    ) -> Option<TextureHandle> {
        let texture = image
            .map(|image| ctx.load_texture(format!("logo:{domain}"), image, TextureOptions::LINEAR));
        self.textures.insert(domain, texture.clone());
        texture
    }
}

/// What stands before an engine's name: its site's logo, or Flick's own
/// drawn icon (Files, Apps, or a site whose logo is not here yet).
pub enum Mark {
    Logo(TextureHandle),
    Drawn(Icon),
}

impl Mark {
    /// The mark for engine `id` (named `name`) on this bar.
    pub fn for_engine(
        logos: &mut Favicons,
        ctx: &egui::Context,
        config: &Config,
        id: &str,
        name: &str,
    ) -> Self {
        config
            .engine_url(id)
            .and_then(|url| icon_domain(&url))
            .and_then(|domain| logos.get(ctx, &domain))
            .map_or(Mark::Drawn(Icon::for_mode(name)), Mark::Logo)
    }

    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
        match self {
            Mark::Logo(texture) => {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), rect, uv, egui::Color32::WHITE);
            }
            Mark::Drawn(icon) => icon.paint(painter, rect, color),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([200, 80, 40, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    /// Asks until the logo is there (the fetch runs on another thread).
    fn wait_for(icons: &mut Favicons, ctx: &egui::Context, domain: &str) -> Option<TextureHandle> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) {
            let texture = icons.get(ctx, domain);
            if texture.is_some() || icons.textures.contains_key(domain) {
                return texture;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }

    #[test]
    fn a_logo_is_fetched_once_kept_on_disk_and_loaded_from_there_next_time() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let fetches = Arc::new(AtomicUsize::new(0));
        let counter = fetches.clone();
        let mut icons = Favicons::with_fetch(
            dir.path().to_path_buf(),
            move |domain| {
                assert_eq!(domain, "claude.ai");
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(png())
            },
            || {},
        );
        assert!(icons.get(&ctx, "claude.ai").is_none());
        let texture = wait_for(&mut icons, &ctx, "claude.ai").expect("a logo");
        assert_eq!(texture.size(), [16, 16]);
        assert!(icons.get(&ctx, "claude.ai").is_some());
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert!(dir.path().join("claude.ai.png").exists());

        // Next start: straight from disk, no fetch.
        let mut again =
            Favicons::with_fetch(dir.path().to_path_buf(), |_| panic!("no fetch"), || {});
        assert!(again.get(&ctx, "claude.ai").is_some());
    }

    #[test]
    fn a_failed_fetch_leaves_flicks_own_icon_and_is_not_retried_this_session() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let fetches = Arc::new(AtomicUsize::new(0));
        let counter = fetches.clone();
        let mut icons = Favicons::with_fetch(
            dir.path().to_path_buf(),
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                anyhow::bail!("offline")
            },
            || {},
        );
        assert!(wait_for(&mut icons, &ctx, "x.com").is_none());
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert!(!dir.path().join("x.com.png").exists());
    }

    #[test]
    fn odd_site_names_never_become_paths() {
        let dir = Path::new("/cache");
        assert_eq!(
            cache_path(dir, "maps.google.com"),
            Some(dir.join("maps.google.com.png"))
        );
        assert_eq!(cache_path(dir, "../evil"), None);
        assert_eq!(cache_path(dir, "a/b"), None);
        assert_eq!(cache_path(dir, ""), None);
    }
}
