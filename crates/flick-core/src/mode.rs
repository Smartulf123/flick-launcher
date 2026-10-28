//! The contract every search mode implements (Flow Launcher's plugin model, in Rust).

use std::path::PathBuf;

/// What happens when the user presses Enter on a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open a web address in the default browser.
    OpenUrl(String),
    /// Open a file or folder with its default program.
    OpenPath(PathBuf),
    /// Start an installed program; what the text means depends on the
    /// system (see `apps::AppEntry::launch`).
    Launch(String),
    /// Informational row; Enter does nothing.
    None,
}

/// What a result is, so the window can pick an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKind {
    Web,
    File,
    Folder,
    /// An installed program.
    App,
    /// A message row, such as "Indexing files...".
    Info,
}

/// One row in the result list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultItem {
    pub title: String,
    pub subtitle: String,
    /// Higher is better. Only compared within one mode.
    pub score: u32,
    pub action: Action,
    pub kind: ResultKind,
}

/// A search mode such as Google, YouTube or Files.
pub trait SearchMode: Send + Sync {
    /// Stable identifier used in the config file, e.g. `"google"`.
    fn id(&self) -> &str;
    /// Name shown in the search bar, e.g. `"Google"`.
    fn name(&self) -> &str;
    /// Results for `text`, best first, at most `max_results`.
    fn query(&self, text: &str, max_results: usize) -> Vec<ResultItem>;
}
