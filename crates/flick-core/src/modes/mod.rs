//! Built-in search modes.

pub mod files;
pub mod suggest;
pub mod web;

pub use files::FilesMode;
pub use web::WebSearchMode;
