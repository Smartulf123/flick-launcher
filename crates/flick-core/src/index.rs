//! An in-memory list of files and folders under a few root folders.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;

use walkdir::WalkDir;

/// One file or folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    /// The file name only, which is what the user searches.
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Default)]
pub struct FileIndex {
    pub entries: Vec<FileEntry>,
}

/// The index shared between the UI thread and the indexing thread.
/// `None` until the first build finishes.
pub type SharedIndex = Arc<RwLock<Option<FileIndex>>>;

impl FileIndex {
    /// Walks every root, skipping folders named in `exclude_dir_names` and
    /// anything that cannot be read (missing roots, permission errors).
    pub fn build(roots: &[PathBuf], exclude_dir_names: &[String]) -> FileIndex {
        let mut entries = Vec::new();
        for root in roots {
            let walker = WalkDir::new(root).min_depth(1).into_iter();
            let walker = walker.filter_entry(|e| {
                !(e.file_type().is_dir()
                    && exclude_dir_names
                        .iter()
                        .any(|x| e.file_name().to_string_lossy() == x.as_str()))
            });
            entries.extend(walker.filter_map(Result::ok).map(|e| FileEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: e.file_type().is_dir(),
                path: e.into_path(),
            }));
        }
        FileIndex { entries }
    }
}

/// Builds a fresh index on a background thread, then swaps it in. The old
/// index stays searchable until the swap. `on_done` runs after the swap.
pub fn spawn_rebuild(
    shared: SharedIndex,
    roots: Vec<PathBuf>,
    exclude_dir_names: Vec<String>,
    on_done: impl FnOnce() + Send + 'static,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let index = FileIndex::build(&roots, &exclude_dir_names);
        // A poisoned lock only means another thread panicked mid-write; the
        // value is replaced wholesale, so recovering it is safe.
        *shared.write().unwrap_or_else(|p| p.into_inner()) = Some(index);
        on_done();
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        for file in [
            "docs/budget_2026.xlsx",
            "docs/notes.txt",
            "node_modules/pkg/index.js",
            ".git/HEAD",
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"").unwrap();
        }
        Fixture { _dir: dir, root }
    }

    fn excludes() -> Vec<String> {
        vec![".git".into(), "node_modules".into()]
    }

    #[test]
    fn indexes_files_and_folders_by_name() {
        let f = fixture();
        let idx = FileIndex::build(std::slice::from_ref(&f.root), &excludes());
        let names: Vec<_> = idx
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.is_dir))
            .collect();
        assert!(names.contains(&("budget_2026.xlsx", false)));
        assert!(names.contains(&("docs", true)));
    }

    #[test]
    fn excluded_folders_are_skipped_with_contents() {
        let f = fixture();
        let idx = FileIndex::build(std::slice::from_ref(&f.root), &excludes());
        assert!(
            idx.entries
                .iter()
                .all(|e| e.name != "index.js" && e.name != "node_modules" && e.name != "HEAD")
        );
    }

    #[test]
    fn roots_themselves_are_not_entries() {
        let f = fixture();
        let idx = FileIndex::build(std::slice::from_ref(&f.root), &excludes());
        assert!(idx.entries.iter().all(|e| e.path != f.root));
    }

    #[test]
    fn missing_root_is_ignored() {
        let f = fixture();
        let idx = FileIndex::build(
            &[f.root.join("does-not-exist"), f.root.clone()],
            &excludes(),
        );
        assert!(idx.entries.iter().any(|e| e.name == "notes.txt"));
    }

    #[test]
    fn rebuild_fills_shared_index_and_calls_back() {
        let f = fixture();
        let shared: SharedIndex = Default::default();
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_rebuild(
            shared.clone(),
            vec![f.root.clone()],
            excludes(),
            move || tx.send(()).unwrap(),
        )
        .join()
        .unwrap();
        rx.try_recv().unwrap();
        assert!(shared.read().unwrap().as_ref().unwrap().entries.len() >= 3);
    }
}
