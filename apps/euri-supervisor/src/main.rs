use anyhow::Result;
use supervisor_core::{AppPaths, Supervisor};
use supervisor_ipc::SupervisorRpc;

fn main() -> Result<()> {
    let paths = AppPaths::discover()?;
    let supervisor = Supervisor::bootstrap(paths)?;
    let rpc = SupervisorRpc::new(supervisor);

    let response = rpc.handle_json(
        r#"{"type":"request","request_id":"bootstrap","method":"system.health","params":{}}"#,
    )?;

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}
