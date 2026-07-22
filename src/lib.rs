pub mod config;
pub mod domain;
pub mod error;
pub mod filesystem;
pub mod importer;
pub mod render;
pub mod search;
pub mod service;
#[cfg(feature = "tui")]
pub mod tui;

pub use config::{AppConfig, ColorSetting, OutputSetting, PreviewRenderSetting, config_path};
pub use domain::{
    CatalogSnapshot, ChangeSet, Fingerprint, Fragment, FragmentManifest, LibraryManifest,
    SearchResult, Snippet, SnippetManifest, SourceMetadata, TagDefinition, TagRegistry,
};
pub use error::{ErrorKind, Result, SnipError};
pub use filesystem::Library;
pub use search::{MemoryIndex, SearchIndex};
