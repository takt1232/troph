mod auth;
mod error;
mod files;
mod ops;
mod paths;
mod server;
mod thumbs;
mod zip_route;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub use auth::AuthSnapshot;
pub use server::{start_server, start_server_with, start_server_with_snapshot, RunningServer};
pub use thumbs::cleanup_cache;
#[cfg(feature = "dev-thumbs")]
pub use thumbs::ImageThumbnails;

pub const APP_NAME: &str = "Troph";
pub const APP_VERSION: &str = env!("APP_VERSION");
pub const DEFAULT_PORT: u16 = 55432;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RootKind {
    #[default]
    Folder,
    File,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Root {
    pub label: String,
    pub path: String,
    #[serde(default)]
    pub kind: RootKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_kind_defaults_to_folder_and_parses_file() {
        let folder: Root = serde_json::from_str(r#"{"label":"Home","path":"/home"}"#).unwrap();
        assert_eq!(folder.kind, RootKind::Folder);

        let file: Root =
            serde_json::from_str(r#"{"label":"Photo","path":"/home/a.jpg","kind":"file"}"#)
                .unwrap();
        assert_eq!(file.kind, RootKind::File);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    pub pin: String,
    pub hostname: String,
    pub device_name: String,
    pub roots: Vec<Root>,
    pub cache_dir: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            pin: String::new(),
            hostname: String::new(),
            device_name: "Troph".to_string(),
            roots: Vec::new(),
            cache_dir: None,
        }
    }
}

impl ServerConfig {
    pub fn new(pin: impl Into<String>, roots: Vec<Root>) -> Self {
        Self {
            pin: pin.into(),
            roots,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerStatus {
    pub ok: bool,
    pub name: String,
    pub version: String,
    pub hostname: String,
    pub hostname_url: String,
    pub port: u16,
    pub url: String,
    pub urls: Vec<String>,
    pub device_name: String,
}

#[derive(Clone, Debug)]
pub struct ThumbnailData {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

pub trait ThumbnailProvider: Send + Sync + 'static {
    fn thumbnail(&self, path: &Path, size: u32) -> Result<Option<ThumbnailData>, String>;
}

pub trait MediaScanner: Send + Sync + 'static {
    fn scan(&self, path: &Path);
}

pub struct NoopThumbnails;

impl ThumbnailProvider for NoopThumbnails {
    fn thumbnail(&self, _path: &Path, _size: u32) -> Result<Option<ThumbnailData>, String> {
        Ok(None)
    }
}

pub struct NoopScanner;

impl MediaScanner for NoopScanner {
    fn scan(&self, _path: &Path) {}
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub config: Arc<ServerConfig>,
    pub paths: Arc<paths::PathSandbox>,
    pub auth: Arc<auth::Auth>,
    pub thumbs: Arc<dyn ThumbnailProvider>,
    pub scanner: Arc<dyn MediaScanner>,
    pub cache_dir: PathBuf,
    pub thumb_sem: Arc<tokio::sync::Semaphore>,
    pub thumb_writes: Arc<AtomicU64>,
}
