//! Background workers for the gallery: Wallhaven searches, thumbnails and
//! downloads, so the window never waits on the network.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use anyhow::Context as _;
use eframe::egui::ColorImage;

use crate::background;
use crate::wallhaven::{self, SearchPage, Wallpaper};

const WORKERS: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(30);

/// Search numbers, shared by every gallery and the workers. A number is
/// never reused while Flick runs, so a reply can only match the search it
/// was made for, and workers skip work for a replaced search.
#[derive(Clone, Default)]
pub struct Generations(Arc<AtomicU64>);

impl Generations {
    /// A new current search number; work for older numbers becomes stale.
    pub fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Work for the loader. `generation` ties searches and thumbnails to one
/// search; `pick` numbers the user's choices so only the latest one wins.
pub enum Job {
    Search {
        generation: u64,
        url: String,
    },
    Thumb {
        generation: u64,
        id: String,
        url: String,
    },
    /// `pick` is `None` for a favorite: saved, but not made the background.
    Download {
        wallpaper: Wallpaper,
        dir: PathBuf,
        pick: Option<u64>,
    },
}

pub enum Done {
    Search {
        generation: u64,
        result: anyhow::Result<SearchPage>,
    },
    Thumb {
        generation: u64,
        id: String,
        image: anyhow::Result<ColorImage>,
    },
    Downloaded {
        id: String,
        pick: Option<u64>,
        result: anyhow::Result<PathBuf>,
    },
}

type Fetch = dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync;

pub struct Loader {
    jobs: Mutex<mpsc::Sender<Job>>,
    done: Arc<Mutex<Vec<Done>>>,
    generations: Generations,
}

impl Loader {
    /// Starts the workers on the real network.
    pub fn start(on_update: impl Fn() + Send + Sync + 'static) -> Loader {
        let agent = crate::http::agent(TIMEOUT);
        Self::start_with(move |url| crate::http::get_bytes(&agent, url), on_update)
    }

    /// The workers with the network call passed in, so tests can run them.
    fn start_with(
        fetch: impl Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync + 'static,
        on_update: impl Fn() + Send + Sync + 'static,
    ) -> Loader {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let done = Arc::new(Mutex::new(Vec::new()));
        let fetch: Arc<Fetch> = Arc::new(fetch);
        let on_update: Arc<dyn Fn() + Send + Sync> = Arc::new(on_update);
        let generations = Generations::default();
        for _ in 0..WORKERS {
            let (rx, done, fetch, on_update, generations) = (
                rx.clone(),
                done.clone(),
                fetch.clone(),
                on_update.clone(),
                generations.clone(),
            );
            std::thread::spawn(move || {
                loop {
                    let job = match rx.lock().unwrap_or_else(|p| p.into_inner()).recv() {
                        Ok(job) => job,
                        Err(_) => break, // the gallery is gone
                    };
                    // A newer search replaced this one: don't fetch for it.
                    if let Job::Search { generation, .. } | Job::Thumb { generation, .. } = &job
                        && *generation != generations.current()
                    {
                        continue;
                    }
                    let result = run(job, fetch.as_ref());
                    done.lock().unwrap_or_else(|p| p.into_inner()).push(result);
                    on_update();
                }
            });
        }
        Loader {
            jobs: Mutex::new(tx),
            done,
            generations,
        }
    }

    pub fn send(&self, job: Job) {
        let _ = self
            .jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send(job);
    }

    /// The shared search numbers (give one to each gallery).
    pub fn generations(&self) -> Generations {
        self.generations.clone()
    }

    /// Results finished since the last call.
    pub fn take_done(&self) -> Vec<Done> {
        std::mem::take(&mut *self.done.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

fn run(job: Job, fetch: &Fetch) -> Done {
    match job {
        Job::Search { generation, url } => Done::Search {
            generation,
            result: fetch(&url)
                .and_then(|body| wallhaven::parse_search(&String::from_utf8_lossy(&body))),
        },
        Job::Thumb {
            generation,
            id,
            url,
        } => Done::Thumb {
            generation,
            id,
            image: fetch(&url).and_then(|bytes| background::decode(&bytes)),
        },
        Job::Download {
            wallpaper,
            dir,
            pick,
        } => Done::Downloaded {
            result: download(&wallpaper, &dir, fetch),
            id: wallpaper.id,
            pick,
        },
    }
}

/// Fetches the full picture, scales it to `background::MAX_WIDTH` and saves
/// it as a JPEG. Nothing is written unless the picture decodes.
fn download(
    wallpaper: &Wallpaper,
    dir: &std::path::Path,
    fetch: &Fetch,
) -> anyhow::Result<PathBuf> {
    let path = wallhaven::download_path(dir, &wallpaper.id).context("unexpected picture id")?;
    let bytes = fetch(&wallpaper.full_url)?;
    let picture = image::load_from_memory(&bytes).context("the download is not a picture")?;
    let (w, h) = background::scaled_size(picture.width(), picture.height(), background::MAX_WIDTH);
    let picture = if (w, h) == (picture.width(), picture.height()) {
        picture
    } else {
        picture.resize_exact(w, h, image::imageops::FilterType::Triangle)
    };
    std::fs::create_dir_all(dir)?;
    let file = std::fs::File::create(&path)?;
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 88);
    if let Err(e) = encoder.encode_image(&picture.to_rgb8()) {
        let _ = std::fs::remove_file(&path);
        return Err(e.into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn png_bytes() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(4, 2, image::Rgb([10, 200, 10]))
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn wallpaper(id: &str) -> Wallpaper {
        Wallpaper {
            id: id.into(),
            thumb_url: "thumb".into(),
            full_url: "full".into(),
        }
    }

    /// Starts a loader with `fetch`, sends `job`, and returns the first result.
    fn run_one(
        fetch: impl Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync + 'static,
        job: Job,
    ) -> Done {
        let (tx, rx) = mpsc::channel();
        let loader = Loader::start_with(fetch, move || {
            let _ = tx.send(());
        });
        // Search and thumbnail jobs are for the current search.
        loader.generations().next();
        loader.send(job);
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        loader.take_done().pop().unwrap()
    }

    #[test]
    fn thumbnail_jobs_come_back_decoded() {
        let bytes = png_bytes();
        let done = run_one(
            move |_| Ok(bytes.clone()),
            Job::Thumb {
                generation: 1,
                id: "a".into(),
                url: "u".into(),
            },
        );
        match done {
            Done::Thumb {
                generation,
                id,
                image,
            } => {
                assert_eq!((generation, id.as_str()), (1, "a"));
                assert_eq!(image.unwrap().size, [4, 2]);
            }
            _ => panic!("expected a thumbnail"),
        }
    }

    #[test]
    fn download_saves_a_jpeg_and_returns_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = png_bytes();
        let done = run_one(
            move |_| Ok(bytes.clone()),
            Job::Download {
                wallpaper: wallpaper("abc"),
                dir: dir.path().to_path_buf(),
                pick: Some(1),
            },
        );
        let Done::Downloaded { id, pick, result } = done else {
            panic!("expected a download")
        };
        let path = result.unwrap();
        assert_eq!((id.as_str(), pick), ("abc", Some(1)));
        assert_eq!(path, dir.path().join("wallhaven-abc.jpg"));
        assert_eq!(crate::background::load(&path).unwrap().size, [4, 2]);
    }

    #[test]
    fn a_favorite_download_is_saved_without_a_pick() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = png_bytes();
        let done = run_one(
            move |_| Ok(bytes.clone()),
            Job::Download {
                wallpaper: wallpaper("fav"),
                dir: dir.path().to_path_buf(),
                pick: None,
            },
        );
        let Done::Downloaded { pick, result, .. } = done else {
            panic!("expected a download")
        };
        assert_eq!(pick, None);
        assert!(result.unwrap().is_file());
    }

    #[test]
    fn failed_download_reports_an_error_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let done = run_one(
            |_| anyhow::bail!("offline"),
            Job::Download {
                wallpaper: wallpaper("abc"),
                dir: dir.path().join("backgrounds"),
                pick: Some(1),
            },
        );
        let Done::Downloaded { result, .. } = done else {
            panic!("expected a download")
        };
        assert!(result.is_err());
        assert!(
            !dir.path()
                .join("backgrounds")
                .join("wallhaven-abc.jpg")
                .exists()
        );
    }

    #[test]
    fn non_picture_download_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let done = run_one(
            |_| Ok(b"<html>rate limited</html>".to_vec()),
            Job::Download {
                wallpaper: wallpaper("abc"),
                dir: dir.path().to_path_buf(),
                pick: Some(1),
            },
        );
        let Done::Downloaded { result, .. } = done else {
            panic!("expected a download")
        };
        assert!(result.is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    /// Talks to wallhaven.cc: run by hand with `cargo test -- --ignored live`.
    #[test]
    #[ignore]
    fn live_wallhaven_search_and_thumbnail() {
        let (tx, rx) = mpsc::channel();
        let loader = Loader::start(move || {
            let _ = tx.send(());
        });
        let url = wallhaven::search_url("anime", wallhaven::Categories::default(), 1);
        let generation = loader.generations().next();
        loader.send(Job::Search { generation, url });
        rx.recv_timeout(Duration::from_secs(15)).unwrap();
        let Some(Done::Search { result, .. }) = loader.take_done().pop() else {
            panic!("expected a search")
        };
        let page = result.unwrap();
        println!(
            "LIVE {} results, last page {}",
            page.wallpapers.len(),
            page.last_page
        );
        let first = page.wallpapers[0].clone();
        loader.send(Job::Thumb {
            generation: 1,
            id: first.id.clone(),
            url: first.thumb_url,
        });
        rx.recv_timeout(Duration::from_secs(15)).unwrap();
        let Some(Done::Thumb { image, .. }) = loader.take_done().pop() else {
            panic!("expected a thumbnail")
        };
        println!("LIVE thumbnail {:?}", image.unwrap().size);
    }

    #[test]
    fn stale_thumbnail_jobs_are_skipped_without_fetching() {
        let fetched = Arc::new(Mutex::new(Vec::new()));
        let seen = fetched.clone();
        let bytes = png_bytes();
        let (tx, rx) = mpsc::channel();
        let loader = Loader::start_with(
            move |url| {
                seen.lock().unwrap().push(url.to_owned());
                Ok(bytes.clone())
            },
            move || {
                let _ = tx.send(());
            },
        );
        let generations = loader.generations();
        let old = generations.next();
        let new = generations.next();
        loader.send(Job::Thumb {
            generation: old,
            id: "old".into(),
            url: "old".into(),
        });
        loader.send(Job::Thumb {
            generation: new,
            id: "new".into(),
            url: "new".into(),
        });
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(*fetched.lock().unwrap(), ["new"]);
        let done = loader.take_done();
        assert!(matches!(done.as_slice(), [Done::Thumb { generation, .. }] if *generation == new));
    }

    #[test]
    fn search_jobs_come_back_parsed() {
        let reply = r#"{"data":[{"id":"x1","url":"p","purity":"sfw","resolution":"1x1","path":"f","thumbs":{"small":"t"}}],"meta":{"current_page":1,"last_page":2}}"#;
        let done = run_one(
            move |_| Ok(reply.as_bytes().to_vec()),
            Job::Search {
                generation: 1,
                url: "u".into(),
            },
        );
        let Done::Search { generation, result } = done else {
            panic!("expected a search")
        };
        let page = result.unwrap();
        assert_eq!(
            (generation, page.wallpapers.len(), page.last_page),
            (1, 1, 2)
        );
    }
}
