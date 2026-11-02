use crate::config::{self, DesktopConfig};
use pocketfiles_core::{
    start_server_with, start_server_with_snapshot, ImageThumbnails, NoopScanner, Root,
    RunningServer, ServerConfig, APP_NAME, APP_VERSION,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub struct ServerSlot {
    server: RunningServer,
    pin: String,
}

struct Inner {
    config: DesktopConfig,
    server: Option<ServerSlot>,
}

pub struct DesktopState {
    inner: Mutex<Inner>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSnapshot {
    pub running: bool,
    pub url: Option<String>,
    pub urls: Vec<String>,
    pub hostname_url: Option<String>,
    pub pin: Option<String>,
    pub version: String,
    pub device_name: String,
    pub port: u16,
    pub roots: Vec<Root>,
    pub config: DesktopConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub port: u16,
    pub device_name: String,
    pub hostname: String,
    pub close_to_tray: bool,
}

impl DesktopState {
    pub fn new(config: DesktopConfig) -> Self {
        Self {
            inner: Mutex::new(Inner {
                config,
                server: None,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn snapshot(&self) -> DesktopSnapshot {
        snapshot_from(&self.lock())
    }

    pub fn close_to_tray(&self) -> bool {
        self.lock().config.close_to_tray
    }
}

fn snapshot_from(inner: &Inner) -> DesktopSnapshot {
    let device_name = if inner.config.device_name.trim().is_empty() {
        APP_NAME.to_string()
    } else {
        inner.config.device_name.clone()
    };
    let mut snapshot = DesktopSnapshot {
        running: false,
        url: None,
        urls: Vec::new(),
        hostname_url: None,
        pin: None,
        version: APP_VERSION.to_string(),
        device_name,
        port: inner.config.port,
        roots: inner.config.roots.clone(),
        config: inner.config.clone(),
    };

    if let Some(slot) = inner.server.as_ref() {
        let status = slot.server.status();
        snapshot.running = true;
        snapshot.url = Some(status.url.clone());
        snapshot.urls = status.urls.clone();
        snapshot.hostname_url = Some(status.hostname_url);
        snapshot.pin = Some(slot.pin.clone());
        snapshot.device_name = status.device_name;
        snapshot.port = status.port;
        snapshot.roots = slot.server.config.roots.clone();
    }

    snapshot
}

fn emit(app: &AppHandle, snapshot: &DesktopSnapshot) {
    let _ = app.emit("state-changed", snapshot.clone());
}

fn cache_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("troph")
}

fn build_server_config(app: &AppHandle, config: &DesktopConfig, pin: &str) -> ServerConfig {
    ServerConfig {
        port: config.port,
        pin: pin.to_string(),
        roots: config.roots.clone(),
        cache_dir: Some(cache_dir(app).display().to_string()),
        device_name: config.device_name.clone(),
        hostname: config.hostname.clone(),
    }
}

fn generate_pin() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
}

pub fn start(app: &AppHandle, state: &DesktopState) -> Result<DesktopSnapshot, String> {
    let mut inner = state.lock();
    if inner.server.is_some() {
        return Ok(snapshot_from(&inner));
    }
    if inner.config.roots.is_empty() {
        return Err("Add a folder or file to share before starting the server.".to_string());
    }

    let pin = generate_pin();
    let config = build_server_config(app, &inner.config, &pin);
    let server = start_server_with(config, Arc::new(ImageThumbnails), Arc::new(NoopScanner))?;
    inner.server = Some(ServerSlot { server, pin });

    let snapshot = snapshot_from(&inner);
    drop(inner);
    emit(app, &snapshot);
    Ok(snapshot)
}

pub fn stop(app: &AppHandle, state: &DesktopState) -> Result<DesktopSnapshot, String> {
    let mut inner = state.lock();
    let old = inner.server.take();
    drop(old);

    let snapshot = snapshot_from(&inner);
    drop(inner);
    emit(app, &snapshot);
    Ok(snapshot)
}

fn apply_config_locked(
    app: &AppHandle,
    inner: &mut Inner,
    next: DesktopConfig,
    restart_running: bool,
) -> Result<DesktopSnapshot, (Box<DesktopSnapshot>, String)> {
    if let Err(error) = config::save(app, &next) {
        return Err((Box::new(snapshot_from(inner)), error));
    }

    if restart_running && inner.server.is_some() {
        let previous = inner.config.clone();
        let slot = inner.server.take().expect("server checked above");
        let pin = slot.pin.clone();
        let auth = slot.server.auth_snapshot();
        drop(slot);
        inner.config = next.clone();

        match start_server_with_snapshot(
            build_server_config(app, &next, &pin),
            Arc::new(ImageThumbnails),
            Arc::new(NoopScanner),
            Some(auth.clone()),
        ) {
            Ok(server) => {
                inner.server = Some(ServerSlot { server, pin });
            }
            Err(error) => {
                inner.config = previous.clone();
                let _ = config::save(app, &previous);
                if let Ok(server) = start_server_with_snapshot(
                    build_server_config(app, &previous, &pin),
                    Arc::new(ImageThumbnails),
                    Arc::new(NoopScanner),
                    Some(auth),
                ) {
                    inner.server = Some(ServerSlot { server, pin });
                }
                let snapshot = snapshot_from(inner);
                return Err((
                    Box::new(snapshot),
                    format!("Could not apply changes: {error}"),
                ));
            }
        }
    } else {
        inner.config = next;
    }

    Ok(snapshot_from(inner))
}

fn apply_config(
    app: &AppHandle,
    state: &DesktopState,
    next: DesktopConfig,
    restart_running: bool,
) -> Result<DesktopSnapshot, String> {
    let result = {
        let mut inner = state.lock();
        apply_config_locked(app, &mut inner, next, restart_running)
    };

    match result {
        Ok(snapshot) => {
            emit(app, &snapshot);
            Ok(snapshot)
        }
        Err((snapshot, error)) => {
            emit(app, &snapshot);
            Err(error)
        }
    }
}

pub fn add_root(
    app: &AppHandle,
    state: &DesktopState,
    path: String,
    label: Option<String>,
) -> Result<DesktopSnapshot, String> {
    let root = config::normalize_root(&path, label)?;
    let next = {
        let inner = state.lock();
        let mut next = inner.config.clone();
        config::add_root_to_list(&mut next.roots, root)?;
        next
    };
    apply_config(app, state, next, true)
}

pub fn remove_root(
    app: &AppHandle,
    state: &DesktopState,
    path: String,
) -> Result<DesktopSnapshot, String> {
    let next = {
        let inner = state.lock();
        let mut next = inner.config.clone();
        next.roots
            .retain(|root| Path::new(&root.path) != Path::new(&path));
        next
    };
    apply_config(app, state, next, true)
}

pub fn update_settings(
    app: &AppHandle,
    state: &DesktopState,
    settings: SettingsInput,
) -> Result<DesktopSnapshot, String> {
    if settings.port == 0 {
        return Err("Port must be between 1 and 65535.".to_string());
    }

    let (next, restart) = {
        let inner = state.lock();
        let mut next = inner.config.clone();
        next.port = settings.port;
        next.device_name = settings.device_name.trim().to_string();
        next.hostname = settings.hostname.trim().to_string();
        next.close_to_tray = settings.close_to_tray;
        let restart = next.port != inner.config.port
            || next.device_name != inner.config.device_name
            || next.hostname != inner.config.hostname;
        (next, restart)
    };
    apply_config(app, state, next, restart)
}

pub fn open_file_manager(app: &AppHandle, state: &DesktopState) -> Result<(), String> {
    let snapshot = state.snapshot();
    let url = snapshot
        .url
        .ok_or_else(|| "Start the server before opening the file manager.".to_string())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("cannot open the browser: {error}"))
}
