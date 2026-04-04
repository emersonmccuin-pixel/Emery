use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, anyhow};
use interprocess::TryClone;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Stream, prelude::*,
};

use crate::protocol::ResponseEnvelope;
use crate::rpc::SupervisorRpc;

#[derive(Debug, Clone)]
pub struct LocalIpcServer {
    endpoint_name: String,
    rpc: Arc<SupervisorRpc>,
}

#[derive(Debug)]
enum OutboundMessage {
    Response(ResponseEnvelope),
    Event(crate::protocol::EventEnvelope),
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
                    rpc.record_connection_event("connection.accepted", serde_json::json!({}));
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
    let writer_stream = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let (outbound_tx, outbound_rx) = mpsc::channel::<OutboundMessage>();
    let (event_tx, event_rx) = mpsc::channel();
    let connection = rpc.new_connection_state(event_tx);

    let writer_handle = thread::spawn(move || -> Result<()> {
        let mut stream = writer_stream;
        for message in outbound_rx {
            let payload = match message {
                OutboundMessage::Response(response) => serde_json::to_string(&response)?,
                OutboundMessage::Event(event) => serde_json::to_string(&event)?,
            };
            stream.write_all(payload.as_bytes())?;
            stream.write_all(b"\n")?;
            stream.flush()?;
        }
        Ok(())
    });

    let forward_tx = outbound_tx.clone();
    let event_handle = thread::spawn(move || {
        while let Ok(event) = event_rx.recv() {
            if forward_tx.send(OutboundMessage::Event(event)).is_err() {
                break;
            }
        }
    });

    let result = handle_client_requests(
        rpc.clone(),
        Arc::clone(&connection),
        &mut reader,
        outbound_tx,
    );
    rpc.close_connection(connection);
    rpc.record_connection_event("connection.closed", serde_json::json!({}));

    let _ = event_handle.join();
    match writer_handle.join() {
        Ok(writer_result) => writer_result?,
        Err(_) => return Err(anyhow!("client writer thread panicked")),
    }

    result
}

fn handle_client_requests(
    rpc: Arc<SupervisorRpc>,
    connection: Arc<crate::rpc::ConnectionState>,
    reader: &mut BufReader<Stream>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
) -> Result<()> {
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }

        let response = rpc.handle_json(line.trim_end(), Arc::clone(&connection))?;
        outbound_tx
            .send(OutboundMessage::Response(response))
            .map_err(|_| anyhow!("client outbound channel closed"))?;
    }
}

fn local_socket_name(name: &str) -> Result<interprocess::local_socket::Name<'_>> {
    if GenericNamespaced::is_supported() {
        Ok(name.to_ns_name::<GenericNamespaced>()?)
    } else {
        Ok(name.to_fs_name::<GenericFilePath>()?)
    }
}
