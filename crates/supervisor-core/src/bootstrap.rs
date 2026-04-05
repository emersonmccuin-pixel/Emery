use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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
        if let Ok(root) = env::var("EURI_APP_DATA_DIR") {
            return Self::from_root(PathBuf::from(root));
        }

        let base = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
            .context("LOCALAPPDATA or APPDATA is required to discover the EURI app data root")?;

        Self::from_root(base.join("EURI"))
    }

    pub fn from_root(root: PathBuf) -> Result<Self> {
        let paths = Self {
            app_db: root.join("app.db"),
            knowledge_db: root.join("knowledge.db"),
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
