use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::bootstrap::AppPaths;

#[derive(Debug, Clone)]
pub struct Supervisor {
    paths: AppPaths,
    started_at_unix_ms: u64,
}

impl Supervisor {
    pub fn bootstrap(paths: AppPaths) -> Result<Self> {
        Ok(Self {
            paths,
            started_at_unix_ms: unix_time_ms(),
        })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn started_at_unix_ms(&self) -> u64 {
        self.started_at_unix_ms
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}
