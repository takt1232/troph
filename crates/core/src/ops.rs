use crate::error::ApiError;
use crate::paths::{unique_target, validate_name, PathSandbox};
use crate::AppState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct MkdirRequest {
    pub path: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameRequest {
    pub path: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub paths: Vec<String>,
}

#[derive(Deserialize)]
pub struct TransferRequest {
    pub paths: Vec<String>,
    pub dest: String,
}

#[derive(Serialize)]
pub struct OpResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

pub async fn mkdir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MkdirRequest>,
) -> Result<Json<OpResult>, ApiError> {
    require_csrf(&state, &headers)?;
    validate_name(&request.name)?;
    let paths = state.paths.clone();
    let target = tokio::task::spawn_blocking(move || {
        let target = paths.resolve_new(&request.path, &request.name)?;
        std::fs::create_dir(&target)
            .map_err(|error| ApiError::io("cannot create directory", error))?;
        Ok::<PathBuf, ApiError>(target)
    })
    .await
    .map_err(|error| ApiError::internal(format!("mkdir task failed: {error}")))??;
    Ok(Json(OpResult {
        ok: true,
        paths: vec![target.to_string_lossy().to_string()],
        errors: Vec::new(),
    }))
}

pub async fn rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RenameRequest>,
) -> Result<Json<OpResult>, ApiError> {
    require_csrf(&state, &headers)?;
    validate_name(&request.name)?;
    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || {
        let source = paths.resolve_existing(&request.path)?;
        if paths.is_root(&source) {
            return Err(ApiError::forbidden("cannot rename a storage root"));
        }
        if std::fs::metadata(&source)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
            && paths.contains_restricted(&source)
        {
            return Err(ApiError::forbidden(
                "cannot rename a folder that contains Android system data",
            ));
        }
        let parent = source
            .parent()
            .ok_or_else(|| ApiError::bad_request("path has no parent"))?;
        let target = parent.join(&request.name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(ApiError::conflict("an item with that name already exists"));
        }
        std::fs::rename(&source, &target).map_err(|error| ApiError::io("cannot rename", error))?;
        Ok::<PathBuf, ApiError>(target)
    })
    .await
    .map_err(|error| ApiError::internal(format!("rename task failed: {error}")))??;
    state.scanner.scan(&result);
    Ok(Json(OpResult {
        ok: true,
        paths: vec![result.to_string_lossy().to_string()],
        errors: Vec::new(),
    }))
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteRequest>,
) -> Result<Json<OpResult>, ApiError> {
    require_csrf(&state, &headers)?;
    if request.paths.is_empty() {
        return Err(ApiError::bad_request("no paths selected"));
    }
    let paths = state.paths.clone();
    let errors = tokio::task::spawn_blocking(move || {
        let mut targets = Vec::new();
        let mut errors = Vec::new();
        for raw in &request.paths {
            match paths.resolve_existing(raw) {
                Ok(path) => {
                    let is_dir = std::fs::metadata(&path)
                        .map(|metadata| metadata.is_dir())
                        .unwrap_or(false);
                    if paths.is_root(&path) {
                        errors.push(format!("cannot delete storage root {}", path.display()));
                    } else if is_dir && paths.contains_restricted(&path) {
                        errors.push(format!(
                            "cannot delete a folder that contains Android system data: {}",
                            path.display()
                        ));
                    } else {
                        targets.push(path);
                    }
                }
                Err(error) => errors.push(error.message),
            }
        }
        targets.sort();
        targets.dedup();
        let mut roots_removed: Vec<PathBuf> = Vec::new();
        for target in targets {
            if roots_removed
                .iter()
                .any(|removed| target.starts_with(removed))
            {
                continue;
            }
            let result = std::fs::symlink_metadata(&target).and_then(|metadata| {
                if metadata.is_dir() {
                    std::fs::remove_dir_all(&target)
                } else {
                    std::fs::remove_file(&target)
                }
            });
            match result {
                Ok(()) => {
                    roots_removed.push(target);
                }
                Err(error) => errors.push(format!("{}: {error}", target.display())),
            }
        }
        errors
    })
    .await
    .map_err(|error| ApiError::internal(format!("delete task failed: {error}")))?;

    Ok(Json(OpResult {
        ok: errors.is_empty(),
        paths: Vec::new(),
        errors,
    }))
}

pub async fn move_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TransferRequest>,
) -> Result<Json<OpResult>, ApiError> {
    require_csrf(&state, &headers)?;
    if request.paths.is_empty() {
        return Err(ApiError::bad_request("no paths selected"));
    }
    let paths = state.paths.clone();
    let (moved, errors) = tokio::task::spawn_blocking(move || {
        let dest = match paths.resolve_dir(&request.dest) {
            Ok(dest) => dest,
            Err(error) => return (Vec::new(), vec![error.message]),
        };
        let mut moved = Vec::new();
        let mut errors = Vec::new();
        for raw in &request.paths {
            match move_one(&paths, raw, &dest) {
                Ok(path) => moved.push(path.to_string_lossy().to_string()),
                Err(error) => errors.push(error.message),
            }
        }
        (moved, errors)
    })
    .await
    .map_err(|error| ApiError::internal(format!("move task failed: {error}")))?;

    for path in &moved {
        state.scanner.scan(Path::new(path));
    }
    Ok(Json(OpResult {
        ok: errors.is_empty(),
        paths: moved,
        errors,
    }))
}

pub async fn copy_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TransferRequest>,
) -> Result<Json<OpResult>, ApiError> {
    require_csrf(&state, &headers)?;
    if request.paths.is_empty() {
        return Err(ApiError::bad_request("no paths selected"));
    }
    let paths = state.paths.clone();
    let (copied, errors) = tokio::task::spawn_blocking(move || {
        let dest = match paths.resolve_dir(&request.dest) {
            Ok(dest) => dest,
            Err(error) => return (Vec::new(), vec![error.message]),
        };
        let mut copied = Vec::new();
        let mut errors = Vec::new();
        for raw in &request.paths {
            match copy_one(&paths, raw, &dest) {
                Ok(path) => copied.push(path.to_string_lossy().to_string()),
                Err(error) => errors.push(error.message),
            }
        }
        (copied, errors)
    })
    .await
    .map_err(|error| ApiError::internal(format!("copy task failed: {error}")))?;

    for path in &copied {
        state.scanner.scan(Path::new(path));
    }
    Ok(Json(OpResult {
        ok: errors.is_empty(),
        paths: copied,
        errors,
    }))
}

fn move_one(paths: &PathSandbox, raw: &str, dest: &Path) -> Result<PathBuf, ApiError> {
    let source = paths.resolve_existing(raw)?;
    if paths.is_root(&source) {
        return Err(ApiError::forbidden("cannot move a storage root"));
    }
    if std::fs::metadata(&source)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
        && paths.contains_restricted(&source)
    {
        return Err(ApiError::forbidden(
            "cannot move a folder that contains Android system data",
        ));
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| ApiError::bad_request("path has no file name"))?;
    let target = dest.join(file_name);
    if target == source {
        return Ok(target);
    }
    if dest.starts_with(&source) {
        return Err(ApiError::bad_request("cannot move a folder into itself"));
    }
    let target = unique_target(&target);
    match std::fs::rename(&source, &target) {
        Ok(()) => Ok(target),
        Err(error) if error.raw_os_error() == Some(18) => {
            copy_recursive(&source, &target)?;
            remove_recursive(&source)?;
            Ok(target)
        }
        Err(error) => Err(ApiError::io("cannot move", error)),
    }
}

fn copy_one(paths: &PathSandbox, raw: &str, dest: &Path) -> Result<PathBuf, ApiError> {
    let source = paths.resolve_existing(raw)?;
    if std::fs::metadata(&source)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
        && paths.contains_restricted(&source)
    {
        return Err(ApiError::forbidden(
            "cannot copy a folder that contains Android system data",
        ));
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| ApiError::bad_request("path has no file name"))?;
    let target = dest.join(file_name);
    if dest.starts_with(&source) {
        return Err(ApiError::bad_request("cannot copy a folder into itself"));
    }
    let target = unique_target(&target);
    copy_recursive(&source, &target)?;
    Ok(target)
}

fn copy_recursive(source: &Path, target: &Path) -> Result<(), ApiError> {
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|error| ApiError::io("cannot inspect source", error))?;
    if metadata.file_type().is_symlink() {
        return Err(ApiError::forbidden("refusing to copy a symbolic link"));
    }
    if metadata.is_dir() {
        std::fs::create_dir_all(target)
            .map_err(|error| ApiError::io("cannot create folder", error))?;
        let entries =
            std::fs::read_dir(source).map_err(|error| ApiError::io("cannot read folder", error))?;
        for entry in entries {
            let entry = entry.map_err(|error| ApiError::io("cannot read folder entry", error))?;
            let child_target = target.join(entry.file_name());
            copy_recursive(&entry.path(), &child_target)?;
        }
        Ok(())
    } else if metadata.is_file() {
        std::fs::copy(source, target).map_err(|error| ApiError::io("cannot copy file", error))?;
        Ok(())
    } else {
        Err(ApiError::bad_request("unsupported file type"))
    }
}

fn remove_recursive(path: &Path) -> Result<(), ApiError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ApiError::io("cannot inspect source", error))?;
    if metadata.is_dir() {
        std::fs::remove_dir_all(path).map_err(|error| ApiError::io("cannot remove folder", error))
    } else {
        std::fs::remove_file(path).map_err(|error| ApiError::io("cannot remove file", error))
    }
}

fn require_csrf(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.auth.check_csrf_header(headers) {
        Ok(())
    } else {
        Err(ApiError::forbidden("invalid CSRF token"))
    }
}
