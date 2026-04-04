use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Stream, prelude::*,
};

use crate::rpc::SupervisorRpc;

#[derive(Debug, Clone)]
pub struct LocalIpcServer {
    endpoint_name: String,
    rpc: Arc<SupervisorRpc>,
}

impl LocalIpcServer {
    pub fn new(endpoint_name: String, rpc: SupervisorRpc) -> Self {
        Self {
            endpoint_name,
            rpc: Arc::new(rpc),
        }
    }

    pub fn endpoint_name(&self) -> &str {
        &self.endpoint_name
    }

    pub fn serve(&self) -> Result<()> {
        let name = local_socket_name(&self.endpoint_name)?;
        let listener = ListenerOptions::new()
            .name(name)
            .create_sync()
            .with_context(|| format!("failed to bind local IPC endpoint {}", self.endpoint_name))?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let rpc = Arc::clone(&self.rpc);
                    thread::spawn(move || {
                        if let Err(error) = handle_client(rpc, stream) {
                            eprintln!("client connection failed: {error:#}");
                        }
                    });
                }
                Err(error) => {
                    eprintln!("incoming local IPC connection failed: {error}");
                }
            }
        }

        Ok(())
    }
}

fn handle_client(rpc: Arc<SupervisorRpc>, stream: Stream) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }

        let response = rpc.handle_json(line.trim_end())?;
        let payload = serde_json::to_string(&response)?;
        reader.get_mut().write_all(payload.as_bytes())?;
        reader.get_mut().write_all(b"\n")?;
        reader.get_mut().flush()?;
    }
}

fn local_socket_name(name: &str) -> Result<interprocess::local_socket::Name<'_>> {
    if GenericNamespaced::is_supported() {
        Ok(name.to_ns_name::<GenericNamespaced>()?)
    } else {
        Ok(name.to_fs_name::<GenericFilePath>()?)
    }
}
