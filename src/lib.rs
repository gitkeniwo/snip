pub mod config;
pub mod domain;
pub mod error;
pub mod filesystem;
pub mod importer;
pub mod render;
pub mod search;
pub mod service;
pub mod sort;
#[cfg(feature = "tui")]
pub mod tui;

pub use config::{
    AppConfig, ColorSetting, OutputSetting, PreviewRenderSetting, TuiConfig, TuiIconSetting,
    TuiThemeSetting, config_path,
};
pub use domain::{
    CatalogSnapshot, ChangeSet, Fingerprint, Fragment, FragmentManifest, LibraryManifest,
    SearchResult, Snippet, SnippetManifest, SourceMetadata, TagDefinition, TagRegistry,
    UNCATEGORIZED, folder_label,
};
pub use error::{ErrorKind, Result, SnipError};
pub use filesystem::Library;
pub use search::{MemoryIndex, SearchIndex};
pub use sort::SortMode;
