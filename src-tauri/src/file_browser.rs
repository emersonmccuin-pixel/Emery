use crate::error::{AppError, AppResult};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use ignore::WalkBuilder;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const MAX_TEXT_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_IMAGE_FILE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileReadResult {
    pub path: String,
    pub encoding: String,
    pub content: Option<String>,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub is_binary: bool,
    pub mime_type: Option<String>,
}

pub fn list_dir(
    root_path: &str,
    relative_path: &str,
    show_ignored: bool,
) -> AppResult<Vec<ProjectFileEntry>> {
    let root = canonicalize_root(root_path)?;
    let directory = resolve_existing_directory(&root, relative_path)?;

    let mut entries = if show_ignored {
        fs::read_dir(&directory)
            .map_err(|error| {
                AppError::io(format!("failed to list {}: {error}", directory.display()))
            })?
            .filter_map(|entry| entry.ok().map(|item| item.path()))
            .collect::<Vec<_>>()
    } else {
        let mut builder = WalkBuilder::new(&directory);
        builder
            .max_depth(Some(1))
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false)
            .parents(true)
            .ignore(true)
            .follow_links(false);

        builder
            .build()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.into_path())
            .filter(|path| path != &directory)
            .collect::<Vec<_>>()
    };

    entries.sort_by(|left, right| {
        let left_dir = left.is_dir();
        let right_dir = right.is_dir();
        right_dir
            .cmp(&left_dir)
            .then_with(|| left.file_name().cmp(&right.file_name()))
            .then_with(|| left.cmp(right))
    });

    entries
        .into_iter()
        .map(|path| {
            let metadata = fs::metadata(&path).map_err(|error| {
                AppError::io(format!("failed to inspect {}: {error}", path.display()))
            })?;
            Ok(ProjectFileEntry {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: relative_display_path(&root, &path)?,
                is_dir: metadata.is_dir(),
                size_bytes: metadata.len(),
                modified_at: modified_at_string(&metadata),
            })
        })
        .collect()
}

pub fn read_file(root_path: &str, relative_path: &str) -> AppResult<ProjectFileReadResult> {
    let root = canonicalize_root(root_path)?;
    let path = resolve_existing_file(&root, relative_path)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| AppError::io(format!("failed to inspect {}: {error}", path.display())))?;
    let size_bytes = metadata.len();
    let modified_at = modified_at_string(&metadata);
    let relative = relative_display_path(&root, &path)?;

    if is_image_path(&path) {
        if size_bytes > MAX_IMAGE_FILE_BYTES {
            return Err(AppError::invalid_input(format!(
                "{} is larger than the {} MB image preview limit",
                relative,
                MAX_IMAGE_FILE_BYTES / (1024 * 1024)
            )));
        }
        let bytes = fs::read(&path)
            .map_err(|error| AppError::io(format!("failed to read {}: {error}", path.display())))?;
        return Ok(ProjectFileReadResult {
            path: relative,
            encoding: "base64".to_string(),
            content: Some(BASE64_STANDARD.encode(bytes)),
            size_bytes,
            modified_at,
            is_binary: true,
            mime_type: image_mime_type(&path),
        });
    }

    if size_bytes > MAX_TEXT_FILE_BYTES {
        return Err(AppError::invalid_input(format!(
            "{} is larger than the {} MB text preview limit",
            relative,
            MAX_TEXT_FILE_BYTES / (1024 * 1024)
        )));
    }

    let bytes = fs::read(&path)
        .map_err(|error| AppError::io(format!("failed to read {}: {error}", path.display())))?;
    let text = String::from_utf8(bytes);
    match text {
        Ok(value) if !value.contains('\0') => Ok(ProjectFileReadResult {
            path: relative,
            encoding: "utf8".to_string(),
            content: Some(value),
            size_bytes,
            modified_at,
            is_binary: false,
            mime_type: None,
        }),
        _ => Ok(ProjectFileReadResult {
            path: relative,
            encoding: "none".to_string(),
            content: None,
            size_bytes,
            modified_at,
            is_binary: true,
            mime_type: None,
        }),
    }
}

pub fn write_file(root_path: &str, relative_path: &str, content: &str) -> AppResult<()> {
    let root = canonicalize_root(root_path)?;
    let path = resolve_existing_file(&root, relative_path)?;

    if is_image_path(&path) {
        return Err(AppError::invalid_input(format!(
            "{} is not editable as text",
            relative_display_path(&root, &path)?
        )));
    }

    if let Ok(existing) = fs::read(&path) {
        if String::from_utf8(existing)
            .ok()
            .filter(|value| !value.contains('\0'))
            .is_none()
        {
            return Err(AppError::invalid_input(format!(
                "{} is not editable as UTF-8 text",
                relative_display_path(&root, &path)?
            )));
        }
    }

    fs::write(&path, content)
        .map_err(|error| AppError::io(format!("failed to write {}: {error}", path.display())))?;
    Ok(())
}

pub fn reveal_in_file_explorer(root_path: &str, relative_path: &str) -> AppResult<()> {
    let root = canonicalize_root(root_path)?;
    let path = resolve_existing_path(&root, relative_path)?;

    let status = if cfg!(target_os = "windows") {
        if path.is_file() {
            Command::new("explorer.exe")
                .arg(format!("/select,{}", path.display()))
                .status()
        } else {
            Command::new("explorer.exe").arg(&path).status()
        }
    } else if cfg!(target_os = "macos") {
        if path.is_file() {
            Command::new("open").arg("-R").arg(&path).status()
        } else {
            Command::new("open").arg(&path).status()
        }
    } else {
        let target = if path.is_file() {
            path.parent().unwrap_or(&path)
        } else {
            &path
        };
        Command::new("xdg-open").arg(target).status()
    }
    .map_err(|error| AppError::io(format!("failed to reveal {}: {error}", path.display())))?;

    if !status.success() {
        return Err(AppError::io(format!(
            "file explorer returned a non-zero exit code for {}",
            path.display()
        )));
    }

    Ok(())
}

fn canonicalize_root(root_path: &str) -> AppResult<PathBuf> {
    let root = PathBuf::from(root_path);
    if !root.is_absolute() {
        return Err(AppError::invalid_input(format!(
            "project root '{root_path}' must be an absolute path"
        )));
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| AppError::io(format!("failed to resolve {root_path}: {error}")))?;
    if !canonical.is_dir() {
        return Err(AppError::invalid_input(format!(
            "project root '{root_path}' must point to a directory"
        )));
    }
    Ok(canonical)
}

fn normalize_relative_path(relative_path: &str) -> AppResult<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(AppError::invalid_input(format!(
            "path '{relative_path}' must be relative to the project root"
        )));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::invalid_input(format!(
                    "path '{relative_path}' escapes the project root"
                )))
            }
        }
    }
    Ok(normalized)
}

fn resolve_existing_directory(root: &Path, relative_path: &str) -> AppResult<PathBuf> {
    let path = if relative_path.trim().is_empty() {
        root.to_path_buf()
    } else {
        resolve_existing_path(root, relative_path)?
    };
    if !path.is_dir() {
        return Err(AppError::invalid_input(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    Ok(path)
}

fn resolve_existing_file(root: &Path, relative_path: &str) -> AppResult<PathBuf> {
    let path = resolve_existing_path(root, relative_path)?;
    if !path.is_file() {
        return Err(AppError::invalid_input(format!(
            "{} is not a file",
            path.display()
        )));
    }
    Ok(path)
}

fn resolve_existing_path(root: &Path, relative_path: &str) -> AppResult<PathBuf> {
    let relative = normalize_relative_path(relative_path)?;
    if relative.as_os_str().is_empty() {
        return Ok(root.to_path_buf());
    }
    let path = root.join(relative);
    let canonical = path
        .canonicalize()
        .map_err(|error| AppError::io(format!("failed to resolve {}: {error}", path.display())))?;
    if !canonical.starts_with(root) {
        return Err(AppError::invalid_input(format!(
            "path '{}' escapes the project root",
            relative_path
        )));
    }
    Ok(canonical)
}

fn relative_display_path(root: &Path, path: &Path) -> AppResult<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| AppError::invalid_input("path is outside the project root"))?;
    if relative.as_os_str().is_empty() {
        return Ok(String::new());
    }

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/"))
}

fn modified_at_string(metadata: &fs::Metadata) -> Option<String> {
    metadata
        .modified()
        .ok()
        .map(|value| DateTime::<Utc>::from(value).to_rfc3339())
}

fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        Some(ext)
            if matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "bmp"
            )
    )
}

fn image_mime_type(path: &Path) -> Option<String> {
    let mime = match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        _ => return None,
    };
    Some(mime.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppErrorCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("project-commander-file-browser-{name}-{nanos}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn list_dir_respects_gitignore() {
        let root = unique_temp_dir("gitignore");
        fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(root.join("visible.txt"), "visible").unwrap();
        fs::write(root.join("ignored.txt"), "ignored").unwrap();

        let hidden = list_dir(root.to_str().unwrap(), "", false).unwrap();
        assert_eq!(hidden.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), vec![
            ".gitignore",
            "visible.txt",
        ]);

        let all = list_dir(root.to_str().unwrap(), "", true).unwrap();
        assert!(all.iter().any(|entry| entry.name == "ignored.txt"));
    }

    #[test]
    fn read_and_write_text_file_round_trip() {
        let root = unique_temp_dir("round-trip");
        fs::write(root.join("notes.md"), "# Start\n").unwrap();

        let before = read_file(root.to_str().unwrap(), "notes.md").unwrap();
        assert_eq!(before.encoding, "utf8");
        assert_eq!(before.content.as_deref(), Some("# Start\n"));

        write_file(root.to_str().unwrap(), "notes.md", "# Updated\n").unwrap();
        let after = read_file(root.to_str().unwrap(), "notes.md").unwrap();
        assert_eq!(after.content.as_deref(), Some("# Updated\n"));
    }

    #[test]
    fn traversal_is_rejected() {
        let root = unique_temp_dir("traversal");
        let error = read_file(root.to_str().unwrap(), "../outside.txt").unwrap_err();
        assert_eq!(error.code, AppErrorCode::InvalidInput);
    }
}
