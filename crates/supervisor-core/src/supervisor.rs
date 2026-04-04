use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::bootstrap::AppPaths;
use crate::store::{BootstrapState, DatabaseSet, HealthSnapshot};

#[derive(Debug, Clone)]
pub struct Supervisor {
    databases: DatabaseSet,
    paths: AppPaths,
    started_at_unix_ms: u64,
}

impl Supervisor {
    pub fn bootstrap(paths: AppPaths) -> Result<Self> {
        let databases = DatabaseSet::initialize(&paths)?;
        Ok(Self {
            databases,
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

    pub fn health_snapshot(&self) -> Result<HealthSnapshot> {
        self.databases.health_snapshot()
    }

    pub fn bootstrap_state(&self) -> Result<BootstrapState> {
        self.databases.bootstrap_state()
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}
