//! MCP servers: connect, list, call — and never a call without consent.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use yura_mcp::{Connection, McpConfig, McpTool, ServerConfig};

#[derive(Default)]
pub struct McpState(Mutex<HashMap<String, Connection>>);

#[derive(Debug, Clone, Serialize)]
pub struct ServerView {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub connected: bool,
    pub tools: usize,
}

fn path() -> std::path::PathBuf {
    yura_store::data_dir()
        .map(|d| yura_mcp::config_path(&d))
        .unwrap_or_else(|_| std::path::PathBuf::from("mcp.json"))
}

fn load() -> McpConfig {
    McpConfig::load(&path())
}

#[tauri::command]
pub fn mcp_servers(state: State<'_, McpState>) -> Vec<ServerView> {
    let config = load();
    let live = state.0.lock().map(|c| c.len()).unwrap_or(0);
    let _ = live;
    let mut guard = match state.0.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    config
        .servers
        .into_iter()
        .map(|server| {
            let connected = guard.contains_key(&server.id);
            let tools = guard
                .get_mut(&server.id)
                .and_then(|connection| connection.list_tools().ok())
                .map(|t| t.len())
                .unwrap_or(0);
            ServerView {
                id: server.id,
                command: server.command,
                args: server.args,
                enabled: server.enabled,
                connected,
                tools,
            }
        })
        .collect()
}

#[tauri::command]
pub fn mcp_add_server(id: String, command: String, args: Vec<String>) -> Result<(), String> {
    let id = id.trim().to_owned();
    if id.is_empty() || command.trim().is_empty() {
        return Err("A server needs a name and a command.".to_owned());
    }
    let path = path();
    let mut config = McpConfig::load(&path);
    config.servers.retain(|s| s.id != id);
    config.servers.push(ServerConfig {
        id,
        command: command.trim().to_owned(),
        args,
        enabled: true,
    });
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mcp_remove_server(state: State<'_, McpState>, id: String) -> Result<(), String> {
    let path = path();
    let mut config = McpConfig::load(&path);
    config.servers.retain(|s| s.id != id);
    config.save(&path).map_err(|e| e.to_string())?;

    if let Ok(mut guard) = state.0.lock() {
        if let Some(mut connection) = guard.remove(&id) {
            connection.stop();
        }
    }
    Ok(())
}

/// Start a server and complete the handshake. Errors are returned verbatim —
/// "connection closed" is far more useful than "failed".
#[tauri::command]
pub fn mcp_connect(state: State<'_, McpState>, id: String) -> Result<Vec<McpTool>, String> {
    let config = load();
    let server = config
        .servers
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("No server named {id}."))?;

    let mut connection = Connection::start(&server).map_err(|e| e.to_string())?;
    connection.initialize().map_err(|e| e.to_string())?;
    let tools = connection.list_tools().map_err(|e| e.to_string())?;

    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut old) = guard.insert(id, connection) {
        old.stop();
    }
    Ok(tools)
}

#[tauri::command]
pub fn mcp_disconnect(state: State<'_, McpState>, id: String) -> bool {
    match state.0.lock() {
        Ok(mut guard) => {
            if let Some(mut connection) = guard.remove(&id) {
                connection.stop();
                true
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Every tool from every connected server.
#[tauri::command]
pub fn mcp_tools(state: State<'_, McpState>) -> Vec<McpTool> {
    let mut guard = match state.0.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    yura_mcp::namespace(&mut guard).unwrap_or_default()
}

#[tauri::command]
pub async fn mcp_call(
    state: State<'_, McpState>,
    server: String,
    tool: String,
    arguments: serde_json::Value,
) -> Result<String, String> {
    // The registry is a Mutex, and a tool call blocks on the child's stdout, so
    // it is done on a blocking thread with the lock held for as short a time as
    // the protocol allows.
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let connection = guard
        .get_mut(&server)
        .ok_or_else(|| format!("{server} is not connected."))?;
    connection
        .call_tool(&tool, arguments)
        .map_err(|e| e.to_string())
}

/// Called from `run_tool`, so every MCP call goes through the same approval gate.
pub fn call_from_agent(
    state: &McpState,
    server: &str,
    tool: &str,
    arguments: serde_json::Value,
) -> Result<String, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let connection = guard
        .get_mut(server)
        .ok_or_else(|| format!("{server} is not connected. Settings → MCP."))?;
    connection
        .call_tool(tool, arguments)
        .map_err(|e| e.to_string())
}

/// Used by the prompt builder to decide whether to mention MCP at all.
pub fn has_servers() -> bool {
    !load().servers.is_empty()
}

/// A hint line the settings page can show after connecting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedSummary {
    pub servers: usize,
    pub tools: usize,
}

#[tauri::command]
pub fn mcp_summary(state: State<'_, McpState>, _app: AppHandle) -> ConnectedSummary {
    let servers = {
        let guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.len()
    };
    ConnectedSummary {
        servers,
        tools: mcp_tools(state).len(),
    }
}
