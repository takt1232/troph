use crate::error::ApiError;
use crate::paths::PathSandbox;
use crate::AppState;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use chrono::{Datelike, TimeZone, Timelike};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use zip::write::{SimpleFileOptions, StreamWriter, ZipWriter};
use zip::CompressionMethod;

const MAX_FORM_BYTES: usize = 4 * 1024 * 1024;
const WRITE_CHUNK: usize = 256 * 1024;

#[derive(Clone, Debug)]
struct Selection {
    path: PathBuf,
    archive_name: String,
}

pub async fn download(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, ApiError> {
    let body = to_bytes(request.into_body(), MAX_FORM_BYTES)
        .await
        .map_err(|error| ApiError::bad_request(format!("cannot read form: {error}")))?;
    let mut base = None;
    let mut csrf = None;
    let mut selected = Vec::new();
    for (key, value) in form_urlencoded::parse(&body) {
        match key.as_ref() {
            "base" => base = Some(value.into_owned()),
            "csrf" => csrf = Some(value.into_owned()),
            "path" => selected.push(value.into_owned()),
            _ => {}
        }
    }
    let Some(csrf) = csrf else {
        return Err(ApiError::forbidden("invalid CSRF token"));
    };
    if !state.auth.check_csrf_value(&csrf) {
        return Err(ApiError::forbidden("invalid CSRF token"));
    }
    let Some(base) = base else {
        return Err(ApiError::bad_request("missing base folder"));
    };
    if selected.is_empty() {
        return Err(ApiError::bad_request("no files selected"));
    }

    let paths = state.paths.clone();
    let base_for_task = base.clone();
    let selections =
        tokio::task::spawn_blocking(move || prepare_selections(&paths, &base_for_task, &selected))
            .await
            .map_err(|error| ApiError::internal(format!("zip preparation failed: {error}")))??;

    let archive_name = archive_file_name(&base, &state.paths);
    let (sender, receiver) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);
    let error_sender = sender.clone();
    let paths = state.paths.clone();
    tokio::task::spawn_blocking(move || {
        let writer = ChannelWriter { sender };
        let zip = ZipWriter::new_stream(writer).set_auto_large_file();
        let result = build_archive(zip, &paths, &selections);
        if let Err(error) = result {
            log::warn!("zip stream failed: {error}");
            let _ = error_sender.try_send(Err(std::io::Error::other(error)));
        }
    });

    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|item| (item, receiver))
    });
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{archive_name}\""))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn prepare_selections(
    paths: &PathSandbox,
    base_raw: &str,
    requested: &[String],
) -> Result<Vec<Selection>, ApiError> {
    let base = paths.resolve_dir(base_raw)?;
    let mut resolved = Vec::new();
    for raw in requested {
        let path = paths.resolve_existing(raw)?;
        if path != base && !path.starts_with(&base) {
            return Err(ApiError::forbidden(
                "selection is outside the current folder",
            ));
        }
        resolved.push(path);
    }
    resolved.sort();
    resolved.dedup();

    let mut retained: Vec<PathBuf> = Vec::new();
    for path in resolved {
        if retained.iter().any(|existing| path.starts_with(existing)) {
            continue;
        }
        retained.push(path);
    }

    let mut selections = Vec::new();
    for path in retained {
        let name = if path == base {
            base_name(&base, paths)
        } else {
            let relative = path
                .strip_prefix(&base)
                .map_err(|_| ApiError::forbidden("selection is outside the current folder"))?;
            relative
                .components()
                .map(|component| sanitize_segment(&component.as_os_str().to_string_lossy()))
                .collect::<Vec<_>>()
                .join("/")
        };
        if name.is_empty() {
            continue;
        }
        selections.push(Selection {
            path,
            archive_name: name,
        });
    }
    if selections.is_empty() {
        return Err(ApiError::bad_request("no files selected"));
    }
    Ok(selections)
}

fn base_name(base: &Path, paths: &PathSandbox) -> String {
    if paths.is_root(base) {
        paths
            .root_for(base)
            .map(|root| sanitize_segment(&root.label))
            .unwrap_or_else(|| "Troph".to_string())
    } else {
        base.file_name()
            .map(|name| sanitize_segment(&name.to_string_lossy()))
            .unwrap_or_else(|| "Troph".to_string())
    }
}

fn archive_file_name(base_raw: &str, paths: &PathSandbox) -> String {
    let base = PathBuf::from(base_raw);
    let name = base_name(&base, paths);
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = if name.is_empty() {
        "Troph".to_string()
    } else {
        name
    };
    format!(
        "Troph-{}-{}.zip",
        name,
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )
}

fn sanitize_segment(input: &str) -> String {
    let value: String = input
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == '\0' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if value.is_empty() {
        "_".to_string()
    } else {
        value
    }
}

struct ChannelWriter {
    sender: mpsc::Sender<Result<Bytes, std::io::Error>>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let chunk = Bytes::copy_from_slice(buffer);
        self.sender.blocking_send(Ok(chunk)).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "client disconnected")
        })?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn build_archive(
    mut zip: ZipWriter<StreamWriter<ChannelWriter>>,
    paths: &PathSandbox,
    selections: &[Selection],
) -> Result<(), String> {
    let mut names = HashSet::new();
    for selection in selections {
        add_path(
            &mut zip,
            paths,
            &selection.path,
            &selection.archive_name,
            &mut names,
        )?;
    }
    zip.finish()
        .map_err(|error| format!("cannot finish zip: {error}"))?;
    Ok(())
}

fn add_path(
    zip: &mut ZipWriter<StreamWriter<ChannelWriter>>,
    paths: &PathSandbox,
    path: &Path,
    archive_name: &str,
    names: &mut HashSet<String>,
) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        log::debug!("skipping symlink {}", path.display());
        return Ok(());
    }
    if paths.is_restricted(path) {
        log::debug!("skipping restricted path {}", path.display());
        return Ok(());
    }
    let modified = zip_datetime(&metadata);
    if metadata.is_dir() {
        let directory_name = unique_name(format!("{archive_name}/"), names);
        let directory_path = directory_name.trim_end_matches('/').to_string();
        zip.add_directory(
            &directory_name,
            SimpleFileOptions::default()
                .last_modified_time(modified)
                .unix_permissions(0o755),
        )
        .map_err(|error| format!("cannot add folder {directory_name}: {error}"))?;
        let mut children = Vec::new();
        let entries = std::fs::read_dir(path)
            .map_err(|error| format!("cannot read folder {}: {error}", path.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot read folder entry: {error}"))?;
            children.push(entry.path());
        }
        children.sort();
        for child in children {
            let Some(name) = child.file_name() else {
                continue;
            };
            let child_name = format!(
                "{directory_path}/{}",
                sanitize_segment(&name.to_string_lossy())
            );
            add_path(zip, paths, &child, &child_name, names)?;
        }
        Ok(())
    } else if metadata.is_file() {
        let file_name = unique_name(archive_name.to_string(), names);
        let stored = should_store(&file_name);
        let method = if stored {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        zip.start_file(
            &file_name,
            SimpleFileOptions::default()
                .compression_method(method)
                .last_modified_time(modified)
                .unix_permissions(0o644),
        )
        .map_err(|error| format!("cannot add file {file_name}: {error}"))?;
        let mut source = std::fs::File::open(path)
            .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        let mut buffer = vec![0u8; WRITE_CHUNK];
        loop {
            let read = std::io::Read::read(&mut source, &mut buffer)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            if read == 0 {
                break;
            }
            zip.write_all(&buffer[..read])
                .map_err(|error| format!("cannot write zip entry: {error}"))?;
        }
        Ok(())
    } else {
        Ok(())
    }
}

fn unique_name(name: String, names: &mut HashSet<String>) -> String {
    if names.insert(name.clone()) {
        return name;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            (stem.to_string(), Some(extension.to_string()))
        }
        _ => (name.trim_end_matches('/').to_string(), None),
    };
    let is_dir = name.ends_with('/');
    for counter in 1..10_000 {
        let candidate = match &extension {
            Some(extension) if !is_dir => format!("{stem} ({counter}).{extension}"),
            _ => format!("{stem} ({counter})"),
        };
        let candidate = if is_dir {
            format!("{candidate}/")
        } else {
            candidate
        };
        if names.insert(candidate.clone()) {
            return candidate;
        }
    }
    format!(
        "Troph-{}{}",
        uuid::Uuid::new_v4(),
        if is_dir { "/" } else { "" }
    )
}

fn should_store(name: &str) -> bool {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        extension.as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "webp"
            | "heic"
            | "heif"
            | "avif"
            | "mp4"
            | "m4v"
            | "mkv"
            | "webm"
            | "mov"
            | "avi"
            | "mp3"
            | "flac"
            | "m4a"
            | "aac"
            | "ogg"
            | "opus"
            | "zip"
            | "apk"
            | "7z"
            | "rar"
            | "gz"
            | "xz"
            | "zst"
            | "pdf"
    )
}

fn zip_datetime(metadata: &std::fs::Metadata) -> zip::DateTime {
    let Some(modified) = metadata.modified().ok() else {
        return zip::DateTime::default();
    };
    let Ok(age) = modified.duration_since(std::time::UNIX_EPOCH) else {
        return zip::DateTime::default();
    };
    let Some(local) = chrono::Local
        .timestamp_opt(age.as_secs() as i64, age.subsec_nanos())
        .single()
    else {
        return zip::DateTime::default();
    };
    zip::DateTime::from_date_and_time(
        local.year() as u16,
        local.month() as u8,
        local.day() as u8,
        local.hour() as u8,
        local.minute() as u8,
        local.second() as u8,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names = HashSet::new();
        assert_eq!(unique_name("a.txt".to_string(), &mut names), "a.txt");
        assert_eq!(unique_name("a.txt".to_string(), &mut names), "a (1).txt");
        assert_eq!(unique_name("dir/".to_string(), &mut names), "dir/");
        assert_eq!(unique_name("dir/".to_string(), &mut names), "dir (1)/");
    }

    #[test]
    fn sanitizes_segments() {
        assert_eq!(sanitize_segment("a/b"), "a_b");
        assert_eq!(sanitize_segment(".."), "..");
        assert_eq!(sanitize_segment(""), "_");
    }
}
