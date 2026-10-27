//! Background workers for the gallery: Wallhaven searches, thumbnails and
//! downloads, so the window never waits on the network.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use anyhow::Context as _;
use eframe::egui::ColorImage;

use crate::background;
use crate::wallhaven::{self, SearchPage, Wallpaper};

const WORKERS: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(30);

/// Work for the loader. `generation` lets the gallery ignore results of a
/// search it has already replaced.
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
    Download {
        wallpaper: Wallpaper,
        dir: PathBuf,
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
        result: anyhow::Result<PathBuf>,
    },
}

type Fetch = dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync;

pub struct Loader {
    jobs: Mutex<mpsc::Sender<Job>>,
    done: Arc<Mutex<Vec<Done>>>,
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
        for _ in 0..WORKERS {
            let (rx, done, fetch, on_update) =
                (rx.clone(), done.clone(), fetch.clone(), on_update.clone());
            std::thread::spawn(move || {
                loop {
                    let job = match rx.lock().unwrap_or_else(|p| p.into_inner()).recv() {
                        Ok(job) => job,
                        Err(_) => break, // the gallery is gone
                    };
                    let result = run(job, fetch.as_ref());
                    done.lock().unwrap_or_else(|p| p.into_inner()).push(result);
                    on_update();
                }
            });
        }
        Loader {
            jobs: Mutex::new(tx),
            done,
        }
    }

    pub fn send(&self, job: Job) {
        let _ = self
            .jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send(job);
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
        Job::Download { wallpaper, dir } => Done::Downloaded {
            result: download(&wallpaper, &dir, fetch),
            id: wallpaper.id,
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
            page_url: "page".into(),
            resolution: "4x2".into(),
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
                generation: 3,
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
                assert_eq!((generation, id.as_str()), (3, "a"));
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
            },
        );
        let Done::Downloaded { id, result } = done else {
            panic!("expected a download")
        };
        let path = result.unwrap();
        assert_eq!(id, "abc");
        assert_eq!(path, dir.path().join("wallhaven-abc.jpg"));
        assert_eq!(crate::background::load(&path).unwrap().size, [4, 2]);
    }

    #[test]
    fn failed_download_reports_an_error_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let done = run_one(
            |_| anyhow::bail!("offline"),
            Job::Download {
                wallpaper: wallpaper("abc"),
                dir: dir.path().join("backgrounds"),
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
        loader.send(Job::Search { generation: 1, url });
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
    fn search_jobs_come_back_parsed() {
        let reply = r#"{"data":[{"id":"x1","url":"p","purity":"sfw","resolution":"1x1","path":"f","thumbs":{"small":"t"}}],"meta":{"current_page":1,"last_page":2}}"#;
        let done = run_one(
            move |_| Ok(reply.as_bytes().to_vec()),
            Job::Search {
                generation: 7,
                url: "u".into(),
            },
        );
        let Done::Search { generation, result } = done else {
            panic!("expected a search")
        };
        let page = result.unwrap();
        assert_eq!(
            (generation, page.wallpapers.len(), page.last_page),
            (7, 1, 2)
        );
    }
}
