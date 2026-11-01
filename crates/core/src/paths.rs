use crate::error::ApiError;
use crate::{Root, RootKind};
use std::path::{Component, Path, PathBuf};

const RESTRICTED_MESSAGE: &str =
    "Android/data and Android/obb are not accessible on Android 11 and newer";

#[derive(Clone, Debug)]
pub struct ResolvedRoot {
    pub label: String,
    pub path: PathBuf,
    pub kind: RootKind,
}

pub struct PathSandbox {
    roots: Vec<ResolvedRoot>,
}

impl PathSandbox {
    pub fn new(roots: &[Root]) -> Result<Self, String> {
        let mut resolved = Vec::new();
        for root in roots {
            let raw = PathBuf::from(&root.path);
            if !raw.is_absolute() {
                return Err(format!("root path is not absolute: {}", root.path));
            }
            let canon = match std::fs::canonicalize(&raw) {
                Ok(path) => path,
                Err(error) => {
                    log::warn!("skipping root {}: {error}", root.path);
                    continue;
                }
            };
            match root.kind {
                RootKind::Folder if !canon.is_dir() => {
                    log::warn!("skipping root {}: not a directory", root.path);
                    continue;
                }
                RootKind::File if !canon.is_file() => {
                    log::warn!("skipping root {}: not a file", root.path);
                    continue;
                }
                _ => {}
            }
            if is_restricted_absolute(&canon) {
                log::warn!("skipping restricted root {}", root.path);
                continue;
            }
            if resolved.iter().any(|r: &ResolvedRoot| r.path == canon) {
                continue;
            }
            resolved.push(ResolvedRoot {
                label: root.label.clone(),
                path: canon,
                kind: root.kind,
            });
        }

        let mut normalized = Vec::new();
        for (index, root) in resolved.iter().enumerate() {
            let covered = resolved.iter().enumerate().any(|(other_index, other)| {
                other_index != index
                    && other.kind == RootKind::Folder
                    && root.path.starts_with(&other.path)
                    && (root.path != other.path || other_index < index)
            });
            if !covered {
                normalized.push(root.clone());
            }
        }

        Ok(Self { roots: normalized })
    }

    pub fn roots(&self) -> Vec<Root> {
        self.roots
            .iter()
            .map(|root| {
                let path = root.path.display().to_string();
                Root {
                    label: root.label.clone(),
                    path,
                    kind: root.kind,
                }
            })
            .collect()
    }

    pub fn root_for(&self, path: &Path) -> Option<&ResolvedRoot> {
        self.roots.iter().find(|root| match root.kind {
            RootKind::Folder => path == root.path || path.starts_with(&root.path),
            RootKind::File => path == root.path,
        })
    }

    pub fn is_root(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| root.path == path)
    }

    pub fn root_kind(&self, path: &Path) -> Option<RootKind> {
        self.root_for(path).map(|root| root.kind)
    }

    pub fn resolve_existing(&self, raw: &str) -> Result<PathBuf, ApiError> {
        if raw.is_empty() || raw.contains('\0') {
            return Err(ApiError::bad_request("invalid path"));
        }
        let requested = PathBuf::from(raw);
        if !requested.is_absolute() {
            return Err(ApiError::bad_request("path must be absolute"));
        }
        let canon = std::fs::canonicalize(&requested)
            .map_err(|error| ApiError::io("cannot open path", error))?;
        if self.root_for(&canon).is_none() {
            return Err(ApiError::forbidden("path is outside the allowed roots"));
        }
        if is_restricted_absolute(&canon) || self.is_restricted(&canon) {
            return Err(ApiError::forbidden(RESTRICTED_MESSAGE));
        }
        Ok(canon)
    }

    pub fn resolve_dir(&self, raw: &str) -> Result<PathBuf, ApiError> {
        let path = self.resolve_existing(raw)?;
        let metadata =
            std::fs::metadata(&path).map_err(|error| ApiError::io("cannot stat path", error))?;
        if !metadata.is_dir() {
            return Err(ApiError::bad_request("path is not a directory"));
        }
        Ok(path)
    }

    pub fn is_restricted(&self, path: &Path) -> bool {
        is_restricted_absolute(path)
    }

    pub fn contains_restricted(&self, path: &Path) -> bool {
        is_restricted_absolute(path)
            || path.file_name().and_then(|name| name.to_str()) == Some("Android")
            || path.join("Android/data").exists()
            || path.join("Android/obb").exists()
    }

    pub fn parent_within_root(&self, path: &Path) -> Option<PathBuf> {
        if self.is_root(path) {
            return None;
        }
        let parent = path.parent()?;
        if self.root_for(parent).is_some() {
            Some(parent.to_path_buf())
        } else {
            None
        }
    }

    pub fn resolve_new(&self, base_raw: &str, relative: &str) -> Result<PathBuf, ApiError> {
        let base = self.resolve_dir(base_raw)?;
        self.resolve_new_from_base(&base, relative)
    }

    pub fn resolve_new_from_base(&self, base: &Path, relative: &str) -> Result<PathBuf, ApiError> {
        if self.root_for(base).is_none() {
            return Err(ApiError::forbidden(
                "base directory is outside the allowed roots",
            ));
        }
        let components = sanitize_relative(relative)?;
        let (file_name, dirs) = components
            .split_last()
            .ok_or_else(|| ApiError::bad_request("missing file name"))?;

        let mut prospective = base.to_path_buf();
        for component in dirs {
            prospective.push(component);
        }
        if is_restricted_absolute(&prospective) {
            return Err(ApiError::forbidden(RESTRICTED_MESSAGE));
        }

        let mut dir = base.to_path_buf();
        for component in dirs {
            dir.push(component);
            match std::fs::symlink_metadata(&dir) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(ApiError::forbidden("refusing to follow a symbolic link"));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(ApiError::conflict(format!(
                        "{} exists and is not a directory",
                        dir.display()
                    )));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(&dir)
                        .map_err(|error| ApiError::io("cannot create directory", error))?;
                }
                Err(error) => return Err(ApiError::io("cannot inspect directory", error)),
            }
        }

        let dir = std::fs::canonicalize(&dir)
            .map_err(|error| ApiError::io("cannot resolve directory", error))?;
        if self.root_for(&dir).is_none() {
            return Err(ApiError::forbidden("target is outside the allowed roots"));
        }
        if is_restricted_absolute(&dir) || self.is_restricted(&dir) {
            return Err(ApiError::forbidden(RESTRICTED_MESSAGE));
        }
        let target = dir.join(file_name);
        if is_restricted_absolute(&target) {
            return Err(ApiError::forbidden(RESTRICTED_MESSAGE));
        }
        if let Ok(metadata) = std::fs::symlink_metadata(&target) {
            if metadata.file_type().is_symlink() {
                return Err(ApiError::forbidden(
                    "refusing to write through a symbolic link",
                ));
            }
        }
        Ok(target)
    }
}

fn is_restricted_absolute(path: &Path) -> bool {
    let components: Vec<&str> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();
    components
        .windows(2)
        .any(|pair| pair[0] == "Android" && (pair[1] == "data" || pair[1] == "obb"))
}

pub fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::bad_request("name must not be empty"));
    }
    if name == "." || name == ".." {
        return Err(ApiError::bad_request("invalid name"));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(ApiError::bad_request(
            "name must not contain path separators",
        ));
    }
    if name.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(ApiError::bad_request("name contains invalid characters"));
    }
    if name.len() > 255 {
        return Err(ApiError::bad_request("name is too long"));
    }
    Ok(())
}

pub fn sanitize_relative(relative: &str) -> Result<Vec<String>, ApiError> {
    if relative.is_empty() || relative.contains('\0') {
        return Err(ApiError::bad_request("invalid name"));
    }
    if relative.contains('\\') {
        return Err(ApiError::bad_request("name must use forward slashes"));
    }
    let mut components = Vec::new();
    for part in relative.split('/') {
        if part.is_empty() {
            return Err(ApiError::bad_request(
                "name contains an empty path component",
            ));
        }
        validate_name(part)?;
        components.push(part.to_string());
    }
    if components.is_empty() {
        return Err(ApiError::bad_request("missing name"));
    }
    Ok(components)
}

pub fn unique_target(target: &Path) -> PathBuf {
    if std::fs::symlink_metadata(target).is_err() {
        return target.to_path_buf();
    }
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let stem = target
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let extension = target.extension().map(|s| s.to_string_lossy().to_string());
    for counter in 1..10_000 {
        let name = match &extension {
            Some(extension) => format!("{stem} ({counter}).{extension}"),
            None => format!("{stem} ({counter})"),
        };
        let candidate = parent.join(name);
        if std::fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-{}", uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("hello.txt").is_ok());
        assert!(validate_name("..").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("").is_err());
        assert!(sanitize_relative("folder/file.txt").is_ok());
        assert!(sanitize_relative("../file.txt").is_err());
        assert!(sanitize_relative("/file.txt").is_err());
    }

    #[test]
    fn empty_sandbox_allows_nothing() {
        let sandbox = PathSandbox::new(&[]).unwrap();
        let dir = tempfile::tempdir().unwrap();
        assert!(sandbox.roots().is_empty());
        assert!(sandbox
            .resolve_existing(dir.path().to_str().unwrap())
            .is_err());
    }

    #[test]
    fn file_root_allows_only_itself() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"a").unwrap();
        let other = dir.path().join("b.txt");
        std::fs::write(&other, b"b").unwrap();
        let canonical = std::fs::canonicalize(&file).unwrap();
        let root = Root {
            label: "a".to_string(),
            path: file.display().to_string(),
            kind: RootKind::File,
        };
        let sandbox = PathSandbox::new(&[root]).unwrap();
        assert_eq!(sandbox.roots().len(), 1);
        assert!(sandbox.resolve_existing(file.to_str().unwrap()).is_ok());
        assert!(sandbox.resolve_existing(other.to_str().unwrap()).is_err());
        assert!(sandbox.resolve_dir(file.to_str().unwrap()).is_err());
        assert_eq!(sandbox.root_kind(&canonical), Some(RootKind::File));
    }

    #[test]
    fn nested_shares_are_normalized() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"a").unwrap();
        let roots = vec![
            Root {
                label: "file".to_string(),
                path: file.display().to_string(),
                kind: RootKind::File,
            },
            Root {
                label: "folder".to_string(),
                path: dir.path().display().to_string(),
                kind: RootKind::Folder,
            },
        ];
        let sandbox = PathSandbox::new(&roots).unwrap();
        assert_eq!(sandbox.roots().len(), 1);
        assert_eq!(sandbox.roots()[0].kind, RootKind::Folder);
    }

    #[test]
    fn restricted_paths_are_detected() {
        assert!(is_restricted_absolute(Path::new(
            "/storage/emulated/0/Android/data/example"
        )));
        assert!(is_restricted_absolute(Path::new(
            "/storage/emulated/0/Android/obb"
        )));
        assert!(!is_restricted_absolute(Path::new(
            "/storage/emulated/0/Android/media"
        )));
    }

    #[test]
    fn cannot_create_inside_android_data() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = PathSandbox::new(&[Root {
            label: "root".to_string(),
            path: dir.path().display().to_string(),
            kind: RootKind::Folder,
        }])
        .unwrap();
        let base = sandbox.resolve_dir(dir.path().to_str().unwrap()).unwrap();
        assert!(sandbox
            .resolve_new_from_base(&base, "Android/data/file.txt")
            .is_err());
        assert!(!dir.path().join("Android").exists());
    }

    #[test]
    fn recursive_operations_detect_android_data() {
        let dir = tempfile::tempdir().unwrap();
        let android = dir.path().join("Android");
        std::fs::create_dir_all(android.join("data")).unwrap();
        std::fs::create_dir_all(android.join("media")).unwrap();
        std::fs::create_dir(dir.path().join("DCIM")).unwrap();
        let sandbox = PathSandbox::new(&[]).unwrap();
        assert!(sandbox.contains_restricted(dir.path()));
        assert!(sandbox.contains_restricted(&android));
        assert!(sandbox.contains_restricted(&android.join("data")));
        assert!(!sandbox.contains_restricted(&android.join("media")));
        assert!(!sandbox.contains_restricted(&dir.path().join("DCIM")));
    }
}
