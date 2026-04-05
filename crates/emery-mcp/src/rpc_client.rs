use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Write};

use anyhow::{Context, Result, anyhow};
use interprocess::TryClone;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, Stream, prelude::*};
use serde_json::{Value, json};

use supervisor_core::AppPaths;

pub struct RpcClient {
    writer: BufWriter<Stream>,
    reader: BufReader<Stream>,
    next_id: u64,
}

impl RpcClient {
    pub fn connect() -> Result<Self> {
        let paths = AppPaths::discover()?;
        let endpoint = endpoint_name(paths.root.display().to_string().as_str());
        let name = local_socket_name(&endpoint)?;
        let stream = Stream::connect(name)
            .with_context(|| format!("failed to connect to supervisor at {endpoint}"))?;
        let writer_stream = stream.try_clone().context("failed to clone socket stream")?;
        Ok(Self {
            writer: BufWriter::new(writer_stream),
            reader: BufReader::new(stream),
            next_id: 1,
        })
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.to_string();
        self.next_id += 1;

        let request = json!({
            "type": "request",
            "request_id": id,
            "method": method,
            "params": params,
        });

        let json_str = serde_json::to_string(&request)?;
        writeln!(self.writer, "{}", json_str)?;
        self.writer.flush()?;

        let mut line = String::new();
        self.reader.read_line(&mut line)?;

        let response: Value = serde_json::from_str(line.trim_end())
            .context("failed to parse supervisor response")?;

        if response["ok"].as_bool() == Some(true) {
            Ok(response["result"].clone())
        } else {
            let code = response["error"]["code"].as_str().unwrap_or("error");
            let message = response["error"]["message"]
                .as_str()
                .unwrap_or("unknown error");
            Err(anyhow!("{}: {}", code, message))
        }
    }
}

fn endpoint_name(app_data_root: &str) -> String {
    let mut hasher = DefaultHasher::new();
    app_data_root.hash(&mut hasher);
    format!("emery-supervisor-{:016x}", hasher.finish())
}

fn local_socket_name(name: &str) -> Result<interprocess::local_socket::Name<'_>> {
    if GenericNamespaced::is_supported() {
        Ok(name.to_ns_name::<GenericNamespaced>()?)
    } else {
        Ok(name.to_fs_name::<GenericFilePath>()?)
    }
}
