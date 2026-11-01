use crate::error::ApiError;
use crate::paths::PathSandbox;
use crate::{thumbs, AppState};
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

pub const ROOTS_PATH: &str = "@roots";

#[derive(Deserialize)]
pub struct ListQuery {
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct EntryInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub mime: String,
    pub kind: String,
    pub has_thumb: bool,
    pub restricted: bool,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub path: String,
    pub name: String,
    pub parent: Option<String>,
    pub root_label: String,
    pub entries: Vec<EntryInfo>,
    pub is_file_root: bool,
    pub is_virtual_root: bool,
}

#[derive(Deserialize)]
pub struct RawQuery {
    pub path: Option<String>,
    #[serde(default)]
    pub dl: bool,
}

#[derive(Deserialize)]
pub struct ThumbQuery {
    pub path: Option<String>,
    #[serde(default = "default_thumb_size")]
    pub size: u32,
}

fn default_thumb_size() -> u32 {
    320
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ListResponse>, ApiError> {
    let raw = query
        .path
        .ok_or_else(|| ApiError::bad_request("missing path"))?;
    let path = state.paths.resolve_existing(&raw)?;
    let paths = state.paths.clone();
    let response = tokio::task::spawn_blocking(move || {
        let metadata =
            std::fs::metadata(&path).map_err(|error| ApiError::io("cannot stat path", error))?;
        if metadata.is_dir() {
            build_listing(&paths, &path)
        } else if metadata.is_file() && paths.root_kind(&path) == Some(crate::RootKind::File) {
            build_file_root_listing(&paths, &path)
        } else {
            Err(ApiError::bad_request("path is not a directory"))
        }
    })
    .await
    .map_err(|error| ApiError::internal(format!("listing task failed: {error}")))??;
    Ok(Json(response))
}

pub async fn list_roots(State(state): State<AppState>) -> Result<Json<ListResponse>, ApiError> {
    let paths = state.paths.clone();
    let response = tokio::task::spawn_blocking(move || build_roots_listing(&paths))
        .await
        .map_err(|error| ApiError::internal(format!("roots listing task failed: {error}")))??;
    Ok(Json(response))
}

fn build_roots_listing(paths: &PathSandbox) -> Result<ListResponse, ApiError> {
    let mut entries = Vec::new();
    for root in paths.roots() {
        let path = PathBuf::from(&root.path);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                log::warn!("cannot stat shared root {}: {error}", root.path);
                continue;
            }
        };
        let name = if root.label.trim().is_empty() {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| root.path.clone())
        } else {
            root.label.clone()
        };
        let (is_dir, kind, mime, size, has_thumb) = if root.kind == crate::RootKind::Folder {
            (
                true,
                "folder".to_string(),
                "inode/directory".to_string(),
                0,
                false,
            )
        } else {
            let kind = kind_for_path(&path).to_string();
            let has_thumb = kind == "image" || kind == "video";
            (false, kind, mime_for(&path), metadata.len(), has_thumb)
        };
        entries.push(EntryInfo {
            name,
            path: root.path.clone(),
            is_dir,
            size,
            modified: modified_secs(&metadata),
            mime,
            kind,
            has_thumb,
            restricted: false,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(ListResponse {
        path: ROOTS_PATH.to_string(),
        name: "Shared items".to_string(),
        parent: None,
        root_label: "Shared items".to_string(),
        entries,
        is_file_root: false,
        is_virtual_root: true,
    })
}

fn build_file_root_listing(paths: &PathSandbox, path: &Path) -> Result<ListResponse, ApiError> {
    let root = paths
        .root_for(path)
        .ok_or_else(|| ApiError::forbidden("path is outside the allowed roots"))?;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| ApiError::io("cannot stat file", error))?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let kind = kind_for_path(path).to_string();
    let entry = EntryInfo {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| root.label.clone()),
        path: path.to_string_lossy().to_string(),
        is_dir: false,
        size: metadata.len(),
        modified: modified_secs(&metadata),
        mime: mime_for(path),
        has_thumb: kind == "image" || kind == "video",
        restricted: false,
        kind,
    };
    Ok(ListResponse {
        path: path.to_string_lossy().to_string(),
        name: root.label.clone(),
        parent: None,
        root_label: root.label.clone(),
        entries: vec![entry],
        is_file_root: true,
        is_virtual_root: false,
    })
}

fn build_listing(paths: &PathSandbox, path: &Path) -> Result<ListResponse, ApiError> {
    let root = paths
        .root_for(path)
        .ok_or_else(|| ApiError::forbidden("path is outside the allowed roots"))?;
    let name = if paths.is_root(path) {
        root.label.clone()
    } else {
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| root.label.clone())
    };
    let parent = paths
        .parent_within_root(path)
        .map(|parent| parent.to_string_lossy().to_string());

    let read_dir =
        std::fs::read_dir(path).map_err(|error| ApiError::io("cannot read directory", error))?;
    let mut entries = Vec::new();
    for item in read_dir {
        let item = match item {
            Ok(item) => item,
            Err(error) => {
                log::warn!("cannot read directory entry in {}: {error}", path.display());
                continue;
            }
        };
        let child = item.path();
        let metadata = match std::fs::symlink_metadata(&child) {
            Ok(metadata) => metadata,
            Err(error) => {
                log::debug!("cannot stat {}: {error}", child.display());
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let is_dir = metadata.is_dir();
        if !is_dir && !metadata.is_file() {
            continue;
        }
        let modified = modified_secs(&metadata);
        let kind = if is_dir {
            "folder".to_string()
        } else {
            kind_for_path(&child).to_string()
        };
        entries.push(EntryInfo {
            name: item.file_name().to_string_lossy().to_string(),
            path: child.to_string_lossy().to_string(),
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            modified,
            mime: if is_dir {
                "inode/directory".to_string()
            } else {
                mime_for(&child)
            },
            has_thumb: kind == "image" || kind == "video",
            restricted: paths.is_restricted(&child),
            kind,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(ListResponse {
        path: path.to_string_lossy().to_string(),
        name,
        parent,
        root_label: root.label.clone(),
        entries,
        is_file_root: false,
        is_virtual_root: false,
    })
}

pub async fn raw(
    State(state): State<AppState>,
    Query(query): Query<RawQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let raw = query
        .path
        .ok_or_else(|| ApiError::bad_request("missing path"))?;
    let path = state.paths.resolve_existing(&raw)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| ApiError::io("cannot stat file", error))?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let length = metadata.len();
    let modified = metadata.modified().ok();
    let modified_seconds = modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let etag = format!("\"{length:x}-{modified_seconds:x}\"");
    if let Some(value) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    {
        if value.split(',').any(|candidate| candidate.trim() == etag) {
            let mut response = StatusCode::NOT_MODIFIED.into_response();
            response.headers_mut().insert(
                header::ETAG,
                HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("\"\"")),
            );
            return Ok(response);
        }
    }

    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_range(value, length));
    let (start, end, partial) = match range {
        Some((start, end)) => (start, end, true),
        None => (0, length.saturating_sub(1), false),
    };

    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|error| ApiError::io("cannot open file", error))?;
    if partial {
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|error| ApiError::io("cannot seek file", error))?;
    }
    let bytes_to_send = if length == 0 {
        0
    } else {
        end.saturating_sub(start) + 1
    };
    let stream = ReaderStream::new(file.take(bytes_to_send));
    let mut response = Response::new(Body::from_stream(stream));
    if partial {
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    }
    let headers = response.headers_mut();
    let mime = mime_for(&path);
    let inline = !query.dl && is_inline_type(&mime);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&content_disposition(&path, inline))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("\"\"")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    if let Some(modified) = modified {
        if let Ok(value) = HeaderValue::from_str(&httpdate::fmt_http_date(modified)) {
            headers.insert(header::LAST_MODIFIED, value);
        }
    }
    if partial {
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{length}"))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
        );
    }
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(bytes_to_send));
    Ok(response)
}

pub async fn thumb(
    State(state): State<AppState>,
    Query(query): Query<ThumbQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let raw = query
        .path
        .ok_or_else(|| ApiError::bad_request("missing path"))?;
    let path = state.paths.resolve_existing(&raw)?;
    let Some(cached) = thumbs::get_thumbnail(&state, &path, query.size).await? else {
        return Err(ApiError::not_found("no thumbnail available"));
    };
    if let Some(value) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    {
        if value
            .split(',')
            .any(|candidate| candidate.trim() == cached.etag)
        {
            return Ok(StatusCode::NOT_MODIFIED.into_response());
        }
    }
    let mut response = Response::new(Body::from(cached.bytes));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(cached.content_type),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&cached.etag).unwrap_or_else(|_| HeaderValue::from_static("\"\"")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    Ok(response)
}

pub fn parse_range(value: &str, length: u64) -> Option<(u64, u64)> {
    let value = value.strip_prefix("bytes=")?;
    if value.contains(',') || length == 0 {
        return None;
    }
    let (start, end) = value.split_once('-')?;
    if start.is_empty() {
        let suffix: u64 = end.trim().parse().ok()?;
        if suffix == 0 {
            return None;
        }
        let start = length.saturating_sub(suffix);
        return Some((start, length - 1));
    }
    let start: u64 = start.trim().parse().ok()?;
    if start >= length {
        return None;
    }
    let end = if end.trim().is_empty() {
        length - 1
    } else {
        end.trim().parse::<u64>().ok()?.min(length - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

fn content_disposition(path: &Path, inline: bool) -> String {
    let disposition = if inline { "inline" } else { "attachment" };
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded = percent_encoding::utf8_percent_encode(&name, percent_encoding::NON_ALPHANUMERIC)
        .to_string();
    format!("{disposition}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

fn is_inline_type(mime: &str) -> bool {
    mime.starts_with("image/")
        || mime.starts_with("video/")
        || mime.starts_with("audio/")
        || mime == "application/pdf"
        || mime.starts_with("text/")
}

pub fn kind_for_path(path: &Path) -> &'static str {
    let extension = extension_of(path);
    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "heif" | "avif" | "jxl" => {
            "image"
        }
        "mp4" | "m4v" | "mkv" | "webm" | "mov" | "avi" | "3gp" | "ts" | "m2ts" => "video",
        "mp3" | "flac" | "wav" | "ogg" | "opus" | "m4a" | "aac" | "wma" => "audio",
        "pdf" => "pdf",
        "txt" | "md" | "log" | "csv" | "json" | "xml" | "html" | "htm" | "css" | "js" | "yml"
        | "yaml" | "toml" | "ini" | "sh" => "text",
        "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "zst" => "archive",
        "apk" | "apks" | "xapk" | "aab" => "app",
        _ => "other",
    }
}

pub fn mime_for(path: &Path) -> String {
    let extension = extension_of(path);
    let known = match extension.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "heic" | "heif" => Some("image/heic"),
        "avif" => Some("image/avif"),
        "svg" => Some("image/svg+xml"),
        "mp4" | "m4v" => Some("video/mp4"),
        "mkv" => Some("video/x-matroska"),
        "webm" => Some("video/webm"),
        "mov" => Some("video/quicktime"),
        "avi" => Some("video/x-msvideo"),
        "3gp" => Some("video/3gpp"),
        "ts" | "m2ts" => Some("video/mp2t"),
        "mp3" => Some("audio/mpeg"),
        "flac" => Some("audio/flac"),
        "wav" => Some("audio/wav"),
        "ogg" | "opus" => Some("audio/ogg"),
        "m4a" | "aac" => Some("audio/mp4"),
        "pdf" => Some("application/pdf"),
        "txt" | "md" | "log" => Some("text/plain"),
        "csv" => Some("text/csv"),
        "json" => Some("application/json"),
        "xml" => Some("application/xml"),
        "html" | "htm" => Some("text/html"),
        "css" => Some("text/css"),
        "js" => Some("text/javascript"),
        "zip" => Some("application/zip"),
        "apk" => Some("application/vnd.android.package-archive"),
        _ => None,
    };
    known.map(|value| value.to_string()).unwrap_or_else(|| {
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string()
    })
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn modified_secs(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-9", 100), Some((0, 9)));
        assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=0-999", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=5-2", 100), None);
        assert_eq!(parse_range("bytes=0-1,4-5", 100), None);
        assert_eq!(parse_range("bytes=-0", 100), None);
        assert_eq!(parse_range("bytes=0-0", 0), None);
    }

    #[test]
    fn file_root_listing_has_single_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("photo.jpg");
        std::fs::write(&file, b"image").unwrap();
        let sandbox = PathSandbox::new(&[crate::Root {
            label: "Photo".to_string(),
            path: file.display().to_string(),
            kind: crate::RootKind::File,
        }])
        .unwrap();
        let path = sandbox.resolve_existing(file.to_str().unwrap()).unwrap();
        let listing = build_file_root_listing(&sandbox, &path).unwrap();
        assert!(listing.is_file_root);
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].name, "photo.jpg");
        assert!(!listing.entries[0].is_dir);
        assert_eq!(listing.parent, None);
    }

    #[test]
    fn roots_listing_contains_every_shared_root() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Pictures");
        std::fs::create_dir(&folder).unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, b"notes").unwrap();
        let sandbox = PathSandbox::new(&[
            crate::Root {
                label: "Pictures".to_string(),
                path: folder.display().to_string(),
                kind: crate::RootKind::Folder,
            },
            crate::Root {
                label: "Notes".to_string(),
                path: file.display().to_string(),
                kind: crate::RootKind::File,
            },
        ])
        .unwrap();
        let listing = build_roots_listing(&sandbox).unwrap();
        assert!(listing.is_virtual_root);
        assert_eq!(listing.path, ROOTS_PATH);
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.entries[0].name, "Pictures");
        assert!(listing.entries[0].is_dir);
        assert_eq!(listing.entries[1].name, "Notes");
        assert!(!listing.entries[1].is_dir);
        assert_eq!(listing.entries[1].size, 5);
    }
}
