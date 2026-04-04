use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Result;
use supervisor_core::{AppPaths, Supervisor};
use supervisor_ipc::{LocalIpcServer, SupervisorRpc};

fn main() -> Result<()> {
    let paths = AppPaths::discover()?;
    let supervisor = Supervisor::bootstrap(paths)?;
    let endpoint = endpoint_name(supervisor.paths().root.display().to_string().as_str());
    let rpc = SupervisorRpc::new(supervisor.clone(), endpoint.clone());
    let server = LocalIpcServer::new(endpoint.clone(), rpc);

    eprintln!("euri-supervisor listening on {endpoint}");
    server.serve()
}

fn endpoint_name(app_data_root: &str) -> String {
    let mut hasher = DefaultHasher::new();
    app_data_root.hash(&mut hasher);
    format!("euri-supervisor-{:016x}", hasher.finish())
}
