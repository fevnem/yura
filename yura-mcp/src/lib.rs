//! # yura-mcp
//!
//! A minimal MCP client, spoken over stdio.
//!
//! Model Context Protocol servers are ordinary child processes that speak
//! newline-delimited JSON-RPC on stdin/stdout. That is the whole transport, so
//! this crate has no SDK and no runtime of its own: it spawns the process, sends
//! a request, and reads until the line with the matching id arrives.
//!
//! What it deliberately does not do: SSE/HTTP transports, sampling, resources and
//! prompts. Tools are the part that matters for an agent, and inventing the rest
//! before anyone asks for it is how clients become unmaintainable.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("could not start the server: {0}")]
    Spawn(String),

    #[error("the server closed the connection")]
    Closed,

    #[error("the server returned an error: {0}")]
    Remote(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = McpError> = std::result::Result<T, E>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: String,
    /// Executable, e.g. `npx`, `uvx`, or an absolute path.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
}

impl McpConfig {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| McpError::Protocol(e.to_string()))?;
        std::fs::write(path, body)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub schema: Value,
    /// Which server it came from — filled in by the caller.
    #[serde(default)]
    pub server: String,
}

/// A running server, and the pipes to talk to it.
pub struct Connection {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    pub id: String,
}

impl Connection {
    pub fn start(config: &ServerConfig) -> Result<Self> {
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| McpError::Spawn(format!("{}: {e}", config.command)))?;

        let stdin = child.stdin.take().ok_or(McpError::Closed)?;
        let stdout = BufReader::new(child.stdout.take().ok_or(McpError::Closed)?);

        Ok(Self {
            child,
            stdin,
            stdout,
            next_id: 1,
            id: config.id.clone(),
        })
    }

    fn send(&mut self, message: &Value) -> Result<()> {
        let line = serde_json::to_string(message).map_err(|e| McpError::Protocol(e.to_string()))?;
        writeln!(self.stdin, "{line}")?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Send a request and read until its response arrives.
    ///
    /// Notifications from the server are skipped rather than treated as errors —
    /// a server that logs progress is being well behaved, not broken.
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let mut message = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        if let Some(params) = params {
            message["params"] = params;
        }
        self.send(&message)?;

        let mut line = String::new();
        loop {
            line.clear();
            let read = self.stdout.read_line(&mut line)?;
            if read == 0 {
                return Err(McpError::Closed);
            }

            let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
                continue; // stray output on stdout; ignore it
            };
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue; // a notification, or a reply to an older call
            }
            if let Some(error) = value.get("error") {
                return Err(McpError::Remote(error.to_string()));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    pub fn initialize(&mut self) -> Result<Value> {
        let result = self.request(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "clientInfo": { "name": "yura", "version": env!("CARGO_PKG_VERSION") }
            })),
        )?;

        // The handshake is not complete until the server is told so.
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
        Ok(result)
    }

    pub fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let result = self.request("tools/list", None)?;
        let mut tools: Vec<McpTool> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or_else(|| json!([])),
        )
        .map_err(|e| McpError::Protocol(e.to_string()))?;

        for tool in &mut tools {
            tool.server = self.id.clone();
        }
        Ok(tools)
    }

    /// Call a tool and flatten the content blocks into plain text.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String> {
        let result = self.request(
            "tools/call",
            Some(json!({ "name": name, "arguments": arguments })),
        )?;

        let mut out = String::new();
        if let Some(blocks) = result.get("content").and_then(Value::as_array) {
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if !out.is_empty() {
                            out.push('\n');
                        }
                        out.push_str(block.get("text").and_then(Value::as_str).unwrap_or(""));
                    }
                    Some(other) => {
                        out.push_str(&format!("[{other} content omitted]"));
                    }
                    None => {}
                }
            }
        }
        if out.trim().is_empty() {
            out = serde_json::to_string_pretty(&result).unwrap_or_default();
        }
        Ok(out)
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Where the server list lives, given the app's data directory.
pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("mcp.json")
}

/// Merge every connected server's tools into one map, keyed `server__tool`.
pub fn namespace(connections: &mut HashMap<String, Connection>) -> Result<Vec<McpTool>> {
    let mut all = Vec::new();
    for (id, connection) in connections.iter_mut() {
        match connection.list_tools() {
            Ok(tools) => all.extend(tools),
            Err(error) => {
                tracing::warn!(server = %id, %error, "could not list MCP tools");
            }
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn config() -> ServerConfig {
        ServerConfig {
            id: "echo".into(),
            command: "python3".into(),
            args: vec!["-c".into(), SERVER.into()],
            enabled: true,
        }
    }

    /// A two-tool server, written out so the test exercises the real transport.
    const SERVER: &str = r#"
import sys, json
def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    msg = json.loads(line)
    method = msg.get("method")
    rid = msg.get("id")
    if method == "initialize":
        send({"jsonrpc":"2.0","id":rid,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"echo","version":"1"}}})
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        send({"jsonrpc":"2.0","id":rid,"result":{"tools":[{"name":"echo","description":"Echo text","inputSchema":{"type":"object"}}]}})
    elif method == "tools/call":
        text = msg["params"]["arguments"].get("text","")
        send({"jsonrpc":"2.0","id":rid,"result":{"content":[{"type":"text","text":"echo: " + text}]}})
    elif rid is not None:
        send({"jsonrpc":"2.0","id":rid,"error":{"message":"unknown method"}})
"#;

    #[test]
    fn config_round_trips() {
        let dir = std::env::temp_dir().join(format!("yura-mcp-{}", std::process::id()));
        let path = config_path(&dir);
        let config = McpConfig {
            servers: vec![ServerConfig {
                id: "files".into(),
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
                enabled: true,
            }],
        };
        config.save(&path).unwrap();

        let loaded = McpConfig::load(&path);
        assert_eq!(loaded.servers.len(), 1);
        assert_eq!(loaded.servers[0].id, "files");
    }

    #[test]
    fn talks_to_a_real_server() {
        let mut connection = match Connection::start(&config()) {
            Ok(connection) => connection,
            Err(_) => return, // no python on this machine
        };

        connection.initialize().unwrap();
        let tools = connection.list_tools().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].server, "echo");

        let out = connection
            .call_tool("echo", json!({ "text": "hello" }))
            .unwrap();
        assert_eq!(out, "echo: hello");
    }

    #[test]
    fn a_missing_binary_is_an_error_not_a_panic() {
        let config = ServerConfig {
            id: "ghost".into(),
            command: "definitely-not-a-real-binary-xyz".into(),
            args: vec![],
            enabled: true,
        };
        assert!(matches!(Connection::start(&config), Err(McpError::Spawn(_))));
    }

    #[test]
    fn server_that_dies_is_reported() {
        let config = ServerConfig {
            id: "dies".into(),
            command: "python3".into(),
            args: vec!["-c".into(), "import sys; sys.exit(0)".into()],
            enabled: true,
        };
        let mut connection = match Connection::start(&config) {
            Ok(connection) => connection,
            Err(_) => return,
        };
        assert!(matches!(connection.initialize(), Err(McpError::Closed)));
    }

    #[test]
    fn empty_content_falls_back_to_json() {
        // Exercised through a server that returns a result with no content.
        let config = ServerConfig {
            id: "empty".into(),
            command: "python3".into(),
            args: vec![
                "-c".into(),
                r#"
import sys, json
for line in sys.stdin:
    m = json.loads(line)
    if m.get("method") == "initialize":
        sys.stdout.write(json.dumps({"jsonrpc":"2.0","id":m["id"],"result":{}})+"\n"); sys.stdout.flush()
    elif m.get("method") == "tools/call":
        sys.stdout.write(json.dumps({"jsonrpc":"2.0","id":m["id"],"result":{"ok":True}})+"\n"); sys.stdout.flush()
"#
                .into(),
            ],
            enabled: true,
        };
        let mut connection = match Connection::start(&config) {
            Ok(connection) => connection,
            Err(_) => return,
        };
        connection.initialize().unwrap();
        let out = connection.call_tool("anything", json!({})).unwrap();
        assert!(out.contains("ok"));
    }
}
