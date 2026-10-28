//! Built-in search modes.

pub mod apps;
pub mod files;
pub mod suggest;
pub mod web;

pub use apps::AppsMode;
pub use files::FilesMode;
pub use web::WebSearchMode;
