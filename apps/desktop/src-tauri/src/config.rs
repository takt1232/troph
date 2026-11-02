use pocketfiles_core::{Root, RootKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesktopConfig {
    pub port: u16,
    pub device_name: String,
    pub hostname: String,
    pub roots: Vec<Root>,
    pub close_to_tray: bool,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        let name = machine_name();
        Self {
            port: pocketfiles_core::DEFAULT_PORT,
            device_name: name.clone(),
            hostname: format!("troph-{}", name.to_lowercase()),
            roots: Vec::new(),
            close_to_tray: true,
        }
    }
}

fn machine_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Desktop".to_string())
}

pub fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("cannot resolve config directory: {error}"))?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create config directory: {error}"))?;
    Ok(directory.join("config.json"))
}

pub fn load(app: &AppHandle) -> DesktopConfig {
    let result = (|| -> Result<Option<DesktopConfig>, String> {
        let path = config_path(app)?;
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read config: {error}"))?;
        let config =
            serde_json::from_str(&text).map_err(|error| format!("cannot parse config: {error}"))?;
        Ok(Some(config))
    })();

    match result {
        Ok(Some(config)) => config,
        Ok(None) => DesktopConfig::default(),
        Err(error) => {
            log::warn!("{error}");
            DesktopConfig::default()
        }
    }
}

pub fn save(app: &AppHandle, config: &DesktopConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let temporary = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(config)
        .map_err(|error| format!("cannot serialize config: {error}"))?;
    std::fs::write(&temporary, text).map_err(|error| format!("cannot write config: {error}"))?;
    std::fs::rename(&temporary, &path).map_err(|error| format!("cannot save config: {error}"))?;
    Ok(())
}

pub fn normalize_root(path: &str, label: Option<String>) -> Result<Root, String> {
    let raw = PathBuf::from(path);
    if !raw.is_absolute() {
        return Err("Choose an absolute folder or file path.".to_string());
    }
    let canonical = std::fs::canonicalize(&raw)
        .map_err(|error| format!("cannot open {}: {error}", raw.display()))?;
    let kind = if canonical.is_dir() {
        RootKind::Folder
    } else if canonical.is_file() {
        RootKind::File
    } else {
        return Err("The selected path is not a folder or file.".to_string());
    };
    let label = label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            canonical
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "Shared item".to_string());
    Ok(Root {
        label,
        path: canonical.display().to_string(),
        kind,
    })
}

pub fn add_root_to_list(roots: &mut Vec<Root>, root: Root) -> Result<(), String> {
    let candidate = Path::new(&root.path);
    for existing in roots.iter() {
        let existing_path = Path::new(&existing.path);
        let covered = match existing.kind {
            RootKind::Folder => candidate == existing_path || candidate.starts_with(existing_path),
            RootKind::File => candidate == existing_path,
        };
        if covered {
            return Err(format!("{} is already shared.", root.path));
        }
    }

    if root.kind == RootKind::Folder {
        roots.retain(|existing| {
            let existing_path = Path::new(&existing.path);
            !(existing_path == candidate || existing_path.starts_with(candidate))
        });
    } else {
        roots.retain(|existing| Path::new(&existing.path) != candidate);
    }
    roots.push(root);
    Ok(())
}
