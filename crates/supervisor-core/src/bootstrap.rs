use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const CURRENT_APP_DATA_DIR_ENV: &str = "EMERY_APP_DATA_DIR";
const LEGACY_APP_DATA_DIR_ENV: &str = "EURI_APP_DATA_DIR";
const KNOWLEDGE_DB_ENV: &str = "EMERY_KNOWLEDGE_DB";
const CURRENT_APP_DIR_NAME: &str = "Emery";
const LEGACY_APP_DIR_NAME: &str = "EURI";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub app_db: PathBuf,
    pub knowledge_db: PathBuf,
    pub sessions_dir: PathBuf,
    pub worktrees_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        if let Ok(root) = env::var(CURRENT_APP_DATA_DIR_ENV) {
            return Self::from_root(PathBuf::from(root));
        }

        if let Ok(root) = env::var(LEGACY_APP_DATA_DIR_ENV) {
            return Self::from_root(PathBuf::from(root));
        }

        let base = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
            .context("LOCALAPPDATA or APPDATA is required to discover the Emery app data root")?;

        Self::from_root(discover_app_root(base)?)
    }

    pub fn from_root(root: PathBuf) -> Result<Self> {
        let knowledge_db = env::var(KNOWLEDGE_DB_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("knowledge.db"));

        let paths = Self {
            app_db: root.join("app.db"),
            knowledge_db,
            sessions_dir: root.join("sessions"),
            worktrees_dir: root.join("worktrees"),
            logs_dir: root.join("logs"),
            backups_dir: root.join("backups"),
            cache_dir: root.join("cache"),
            root,
        };
        paths.ensure_layout()?;
        Ok(paths)
    }

    fn ensure_layout(&self) -> Result<()> {
        for dir in [
            &self.root,
            &self.sessions_dir,
            &self.worktrees_dir,
            &self.logs_dir,
            &self.backups_dir,
            &self.cache_dir,
        ] {
            create_dir_all(dir)?;
        }
        Ok(())
    }
}

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create app directory {}", path.display()))
}

fn discover_app_root(base: PathBuf) -> Result<PathBuf> {
    let current_root = base.join(CURRENT_APP_DIR_NAME);
    if current_root.exists() {
        return Ok(current_root);
    }

    let legacy_root = base.join(LEGACY_APP_DIR_NAME);
    if legacy_root.exists() {
        migrate_legacy_root(&legacy_root, &current_root)?;
    }

    Ok(current_root)
}

fn migrate_legacy_root(legacy_root: &Path, current_root: &Path) -> Result<()> {
    if current_root.exists() {
        return Ok(());
    }

    match fs::rename(legacy_root, current_root) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            copy_dir_all(legacy_root, current_root).with_context(|| {
                format!(
                    "failed to migrate legacy app data from {} to {} after rename failed: {}",
                    legacy_root.display(),
                    current_root.display(),
                    rename_error
                )
            })
        }
    }
}

fn copy_dir_all(source: &Path, destination: &Path) -> Result<()> {
    create_dir_all(destination)?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read app directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().with_context(|| {
            format!("failed to read file type for {}", source_path.display())
        })?;

        if file_type.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}
