use crate::error::ApiError;
use crate::{files, AppState, ThumbnailData};
use axum::body::Bytes;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;
use tokio::sync::Semaphore;

const CACHE_LIMIT_BYTES: u64 = 256 * 1024 * 1024;

pub struct CachedThumb {
    pub bytes: Bytes,
    pub content_type: &'static str,
    pub etag: String,
}

pub async fn get_thumbnail(
    state: &AppState,
    path: &Path,
    requested_size: u32,
) -> Result<Option<CachedThumb>, ApiError> {
    let size = requested_size.clamp(32, 1024);
    let kind = files::kind_for_path(path);
    if kind != "image" && kind != "video" {
        return Ok(None);
    }

    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| ApiError::io("cannot stat file", error))?;
    if !metadata.is_file() {
        return Ok(None);
    }

    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            hasher.update(duration.as_nanos().to_le_bytes());
        }
    }
    hasher.update(size.to_le_bytes());
    let hash = hex::encode(&hasher.finalize()[..20]);
    let etag = format!("\"{hash}\"");
    let cache_dir = state.cache_dir.join("thumbnails");
    let cache_path = find_cached(&cache_dir, &hash).await;
    if let Some((cache_path, content_type)) = cache_path {
        match tokio::fs::read(&cache_path).await {
            Ok(bytes) => {
                return Ok(Some(CachedThumb {
                    bytes: Bytes::from(bytes),
                    content_type,
                    etag,
                }));
            }
            Err(error) => log::warn!(
                "cannot read thumbnail cache {}: {error}",
                cache_path.display()
            ),
        }
    }

    let permit = state
        .thumb_sem
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiError::internal("thumbnail queue is closed"))?;
    let provider = state.thumbs.clone();
    let path_owned = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || provider.thumbnail(&path_owned, size))
        .await
        .map_err(|error| ApiError::internal(format!("thumbnail task failed: {error}")))?;
    drop(permit);

    let Some(data) =
        result.map_err(|error| ApiError::internal(format!("thumbnail error: {error}")))?
    else {
        return Ok(None);
    };

    let extension = extension_for(&data.content_type);
    let final_path = cache_dir.join(format!("{hash}.{extension}"));
    if let Err(error) = write_cache(&cache_dir, &final_path, &data).await {
        log::warn!("cannot write thumbnail cache: {error}");
    } else {
        let writes = state.thumb_writes.fetch_add(1, Ordering::Relaxed) + 1;
        if writes.is_multiple_of(64) {
            let dir = cache_dir.clone();
            tokio::task::spawn_blocking(move || cleanup_cache(&dir, CACHE_LIMIT_BYTES));
        }
    }

    Ok(Some(CachedThumb {
        bytes: Bytes::from(data.bytes),
        content_type: if extension == "webp" {
            "image/webp"
        } else if extension == "png" {
            "image/png"
        } else {
            "image/jpeg"
        },
        etag,
    }))
}

async fn find_cached(dir: &Path, hash: &str) -> Option<(PathBuf, &'static str)> {
    for (extension, content_type) in [
        ("webp", "image/webp"),
        ("jpg", "image/jpeg"),
        ("png", "image/png"),
    ] {
        let path = dir.join(format!("{hash}.{extension}"));
        if tokio::fs::metadata(&path).await.is_ok() {
            return Some((path, content_type));
        }
    }
    None
}

fn extension_for(content_type: &str) -> &'static str {
    if content_type.contains("webp") {
        "webp"
    } else if content_type.contains("png") {
        "png"
    } else {
        "jpg"
    }
}

async fn write_cache(dir: &Path, target: &Path, data: &ThumbnailData) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let temp = dir.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
    tokio::fs::write(&temp, &data.bytes).await?;
    tokio::fs::rename(&temp, target).await?;
    Ok(())
}

pub fn cleanup_cache(dir: &Path, limit: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        total += metadata.len();
        files.push((path, metadata.len(), modified));
    }
    if total <= limit {
        return;
    }
    files.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in files {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

pub fn new_semaphore() -> std::sync::Arc<Semaphore> {
    std::sync::Arc::new(Semaphore::new(2))
}

#[cfg(any(feature = "dev-thumbs", feature = "image-thumbs"))]
pub struct ImageThumbnails;

#[cfg(any(feature = "dev-thumbs", feature = "image-thumbs"))]
impl crate::ThumbnailProvider for ImageThumbnails {
    fn thumbnail(&self, path: &Path, size: u32) -> Result<Option<ThumbnailData>, String> {
        let kind = files::kind_for_path(path);
        if kind != "image" {
            return Ok(None);
        }
        let image = match image::open(path) {
            Ok(image) => image,
            Err(error) => {
                log::debug!("cannot decode {}: {error}", path.display());
                return Ok(None);
            }
        };
        let thumb = image.thumbnail(size, size).to_rgb8();
        let mut output = std::io::Cursor::new(Vec::new());
        thumb
            .write_to(&mut output, image::ImageFormat::Jpeg)
            .map_err(|error| error.to_string())?;
        Ok(Some(ThumbnailData {
            bytes: output.into_inner(),
            content_type: "image/jpeg".to_string(),
        }))
    }
}
