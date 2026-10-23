//! The bridge between what she says and what she is allowed to do.
//!
//! The agent protocol is deliberately plain text — a fenced block in the reply:
//!
//! ```text
//! ```yura:run_python
//! {"code": "print(2 + 2)"}
//! ```
//! ```
//!
//! That means it works with every provider we support, including local models
//! with no tool-calling support, and the user can read exactly what she asked to
//! run. Every call lands here, and every call that changes something asks first.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;
use yura_tools::{Agenda, Entry, Workspace};

use crate::state::AppState;

/// Calls waiting for a yes or a no from the chat.
#[derive(Default)]
pub struct Approvals(Mutex<HashMap<String, oneshot::Sender<bool>>>);

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool: String,
    /// What the user is agreeing to, in one sentence.
    pub summary: String,
    /// The thing itself, shown verbatim — code, path, or notification body.
    pub detail: String,
    /// `python` | `write` | `notify` — drives the icon and the wording.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub ok: bool,
    pub output: String,
}

fn data_path(name: &str) -> PathBuf {
    yura_store::data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(name)
}

fn workspace_of(state: &AppState) -> Result<Workspace, String> {
    let settings = state.snapshot();
    let root = settings.abilities.workspace.trim().to_owned();
    if root.is_empty() {
        return Err("No workspace folder chosen. Settings → Abilities.".to_owned());
    }
    Workspace::new(root).map_err(|e| e.to_string())
}

fn agent_settings(state: &AppState) -> yura_core::Abilities {
    state.snapshot().abilities
}

/// Ask the chat for permission, and wait for the answer.
async fn ask(
    app: &AppHandle,
    approvals: &Approvals,
    tool: &str,
    summary: &str,
    detail: &str,
    kind: &str,
) -> bool {
    let id = format!("a{}", std::process::id() as u64 * 1000 + rand_id());
    let (tx, rx) = oneshot::channel();

    if let Ok(mut pending) = approvals.0.lock() {
        pending.insert(id.clone(), tx);
    }

    let request = ApprovalRequest {
        id: id.clone(),
        tool: tool.to_owned(),
        summary: summary.to_owned(),
        detail: detail.to_owned(),
        kind: kind.to_owned(),
    };
    let _ = app.emit("approval:request", request);

    // A prompt that waits forever is a hang. Two minutes, then no.
    match tokio::time::timeout(Duration::from_secs(120), rx).await {
        Ok(Ok(answer)) => answer,
        _ => {
            if let Ok(mut pending) = approvals.0.lock() {
                pending.remove(&id);
            }
            false
        }
    }
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}

/// The answer from the approve/deny card.
#[tauri::command]
pub fn respond_approval(approvals: State<'_, Approvals>, id: String, approved: bool) -> bool {
    let mut pending = match approvals.0.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match pending.remove(&id) {
        Some(sender) => sender.send(approved).is_ok(),
        None => false,
    }
}

/// Run one agent tool call.
///
/// `args` is the parsed JSON from the fenced block. Anything missing is an
/// error the model can read and correct on the next turn.
#[tauri::command]
pub async fn run_tool(
    app: AppHandle,
    state: State<'_, AppState>,
    approvals: State<'_, Approvals>,
    name: String,
    args: serde_json::Value,
) -> Result<ToolOutcome, String> {
    let abilities = agent_settings(&state);

    match name.as_str() {
        "run_python" => {
            if !abilities.python {
                return Ok(ToolOutcome {
                    ok: false,
                    output: "Python execution is turned off in Settings → Abilities.".to_owned(),
                });
            }
            let code = args
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or("run_python needs a `code` string")?
                .to_owned();
            let workspace = workspace_of(&state)?;

            if abilities.ask_before_running
                && !ask(
                    &app,
                    &approvals,
                    "run_python",
                    "Run this Python script",
                    &code,
                    "python",
                )
                .await
            {
                return Ok(ToolOutcome {
                    ok: false,
                    output: "The user denied permission to run this script.".to_owned(),
                });
            }

            let root = workspace.root().to_path_buf();
            let result = tokio::task::spawn_blocking(move || {
                yura_tools::run_python(&root, &code, Duration::from_secs(20))
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            let mut output = String::new();
            if !result.stdout.trim().is_empty() {
                output.push_str(result.stdout.trim_end());
            }
            if !result.stderr.trim().is_empty() {
                if !output.is_empty() {
                    output.push_str("\n");
                }
                output.push_str("stderr:\n");
                output.push_str(result.stderr.trim_end());
            }
            if result.timed_out {
                output.push_str("\n[timed out after 20 s and was killed]");
            }
            output.push_str(&format!(
                "\n[exit {:?} · {} ms]",
                result.code, result.duration_ms
            ));

            Ok(ToolOutcome {
                ok: !result.timed_out && result.code == Some(0),
                output,
            })
        }

        "read_file" | "list_files" => {
            if !abilities.read_files {
                return Ok(ToolOutcome {
                    ok: false,
                    output: "Reading files is turned off in Settings → Abilities.".to_owned(),
                });
            }
            let workspace = workspace_of(&state)?;
            if name == "list_files" {
                let entries: Vec<Entry> = workspace.list().map_err(|e| e.to_string())?;
                let body = entries
                    .iter()
                    .map(|e| {
                        if e.dir {
                            format!("{}/", e.name)
                        } else {
                            format!("{} ({} B)", e.name, e.bytes)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                return Ok(ToolOutcome {
                    ok: true,
                    output: if body.is_empty() {
                        "The workspace is empty.".to_owned()
                    } else {
                        body
                    },
                });
            }
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let body = workspace.read(path).map_err(|e| e.to_string())?;
            Ok(ToolOutcome {
                ok: true,
                output: body,
            })
        }

        "write_file" => {
            if !abilities.write_files {
                return Ok(ToolOutcome {
                    ok: false,
                    output: "Writing files is turned off in Settings → Abilities.".to_owned(),
                });
            }
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("write_file needs a `path`")?;
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let workspace = workspace_of(&state)?;

            if !ask(
                &app,
                &approvals,
                "write_file",
                &format!("Write {path} in your workspace"),
                content,
                "write",
            )
            .await
            {
                return Ok(ToolOutcome {
                    ok: false,
                    output: "The user denied the write.".to_owned(),
                });
            }

            let written = workspace.write(path, content).map_err(|e| e.to_string())?;
            Ok(ToolOutcome {
                ok: true,
                output: format!("Wrote {} bytes to {}.", content.len(), written.display()),
            })
        }

        "set_timer" => {
            let label = args
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("Reminder");
            let seconds = args.get("seconds").and_then(|v| v.as_u64()).unwrap_or(60);
            let path = data_path("agenda.json");
            let mut agenda = Agenda::load(&path);
            let timer = agenda.add_timer(label, seconds);
            agenda.save(&path).map_err(|e| e.to_string())?;

            crate::presence::arm(&app, &timer);
            Ok(ToolOutcome {
                ok: true,
                output: format!("Timer set for {seconds}s: {label}."),
            })
        }

        "add_task" => {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("add_task needs `text`")?;
            let path = data_path("agenda.json");
            let mut agenda = Agenda::load(&path);
            let task = agenda.add_task(text);
            agenda.save(&path).map_err(|e| e.to_string())?;
            Ok(ToolOutcome {
                ok: true,
                output: format!("Added task {}: {text}", task.id),
            })
        }

        "list_tasks" => {
            let agenda = Agenda::load(&data_path("agenda.json"));
            Ok(ToolOutcome {
                ok: true,
                output: agenda.summary(),
            })
        }

        "complete_task" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("complete_task needs `id`")?;
            let path = data_path("agenda.json");
            let mut agenda = Agenda::load(&path);
            let closed = {
                match agenda.tasks.iter_mut().find(|t| t.id == id) {
                    Some(task) => {
                        task.done = true;
                        Some(task.text.clone())
                    }
                    None => None,
                }
            };
            match closed {
                Some(text) => {
                    agenda.save(&path).map_err(|e| e.to_string())?;
                    Ok(ToolOutcome {
                        ok: true,
                        output: format!("Closed {text}."),
                    })
                }
                None => Ok(ToolOutcome {
                    ok: false,
                    output: format!("No task with id {id}."),
                }),
            }
        }

        "notify" => {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Yura");
            let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
            if !ask(
                &app,
                &approvals,
                "notify",
                "Send you a notification",
                body,
                "notify",
            )
            .await
            {
                return Ok(ToolOutcome {
                    ok: false,
                    output: "The user denied the notification.".to_owned(),
                });
            }
            crate::presence::notify(&app, title, body);
            Ok(ToolOutcome {
                ok: true,
                output: "Notification sent.".to_owned(),
            })
        }

        other => Ok(ToolOutcome {
            ok: false,
            output: format!("Unknown tool `{other}`."),
        }),
    }
}

/* ── library ──────────────────────────────────────────────────── */

#[tauri::command]
pub fn library_list(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    workspace_of(&state)?
        .list()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_read(state: State<'_, AppState>, path: String) -> Result<String, String> {
    workspace_of(&state)?
        .read(&path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_write(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> Result<(), String> {
    workspace_of(&state)?
        .write(&path, &content)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/* ── agenda ───────────────────────────────────────────────────── */

#[tauri::command]
pub fn agenda_get() -> Agenda {
    Agenda::load(&data_path("agenda.json"))
}

#[tauri::command]
pub fn agenda_add_timer(app: AppHandle, label: String, seconds: u64) -> Agenda {
    let path = data_path("agenda.json");
    let mut agenda = Agenda::load(&path);
    let timer = agenda.add_timer(label, seconds);
    let _ = agenda.save(&path);
    crate::presence::arm(&app, &timer);
    agenda
}

#[tauri::command]
pub fn agenda_complete(task_id: String) -> Agenda {
    let path = data_path("agenda.json");
    let mut agenda = Agenda::load(&path);
    if let Some(task) = agenda.tasks.iter_mut().find(|t| t.id == task_id) {
        task.done = true;
    }
    let _ = agenda.save(&path);
    agenda
}

#[tauri::command]
pub fn agenda_add_task(text: String) -> Agenda {
    let path = data_path("agenda.json");
    let mut agenda = Agenda::load(&path);
    agenda.add_task(text);
    let _ = agenda.save(&path);
    agenda
}

/* ── folder picker ────────────────────────────────────────────── */

#[tauri::command]
pub async fn pick_workspace(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    folder.and_then(|path| path.into_path().ok()).map(|p| {
        p.to_string_lossy().to_string()
    })
}

/* ── attachments ──────────────────────────────────────────────── */

#[derive(Debug, Clone, Serialize)]
pub struct Ingested {
    pub name: String,
    pub kind: String,
    pub bytes: usize,
    pub chars: usize,
    pub block: String,
    pub note: String,
}

#[tauri::command]
pub fn ingest_attachment(name: String, bytes: Vec<u8>) -> Result<Ingested, String> {
    let attachment = yura_docs::ingest_bytes(&name, &bytes).map_err(|e| e.to_string())?;
    let block = yura_docs::to_prompt_block(&attachment, 30_000);
    let chars = attachment
        .text
        .as_deref()
        .map(|t| t.chars().count())
        .unwrap_or(0);
    let note = if attachment.has_text() {
        String::new()
    } else if attachment.ocr {
        "No text layer — this looks like a scan.".to_owned()
    } else {
        "Attached as an image; the model must read it.".to_owned()
    };
    Ok(Ingested {
        name: attachment.name.clone(),
        kind: attachment.kind.as_str().to_owned(),
        bytes: attachment.bytes,
        chars,
        block,
        note,
    })
}

/// Reveal the data folder — the one honest "where is my stuff" button.
#[tauri::command]
pub fn open_data_dir(_app: AppHandle) -> Result<(), String> {
    let dir = yura_store::data_dir().map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {}
    Ok(())
}

/// The instructions that teach her the agent protocol.
///
/// Only the tools the user has actually enabled are described — a model told
/// about a tool it cannot use will keep trying to use it.
pub fn tool_protocol(abilities: &yura_core::Abilities) -> String {
    let mut out = String::from(
        "\n\n## Acting\n\nYou can act, not only talk. To use a tool, put a fenced block \
         in your reply exactly like this, and nothing else inside it:\n\n\
         ```yura:set_timer\n{\"label\": \"drink water\", \"seconds\": 2700}\n```\n\n\
         The app runs it, shows the user the result, and asks you to continue. \
         Do not describe the call in prose as well, and never invent a result — \
         wait for the real one.\n\nAvailable:\n",
    );

    if abilities.python {
        out.push_str(
            "- `run_python` — args {\"code\": string}. Runs in the user's workspace \
             folder with a 20 s limit. Print what you need to see.\n",
        );
    }
    if !abilities.workspace.trim().is_empty() {
        if abilities.read_files {
            out.push_str("- `list_files` — args {}. Lists the workspace folder.\n");
            out.push_str("- `read_file` — args {\"path\": string}. Relative to the workspace.\n");
        }
        if abilities.write_files {
            out.push_str("- `write_file` — args {\"path\": string, \"content\": string}.\n");
        }
    }
    out.push_str("- `set_timer` — args {\"label\": string, \"seconds\": number}.\n");
    out.push_str("- `add_task` — args {\"text\": string}.\n");
    out.push_str("- `list_tasks` — args {}.\n");
    out.push_str("- `complete_task` — args {\"id\": string}.\n");
    out.push_str("- `notify` — args {\"title\": string, \"body\": string}. The user is asked first.\n");
    out
}

/* ── presence settings ────────────────────────────────────────── */

/// Apply tray and start-on-login immediately, without waiting for a restart.
#[tauri::command]
pub fn apply_general(app: AppHandle, general: yura_core::General) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("yura") {
        let _ = tray.set_visible(general.tray);
    } else if general.tray {
        crate::presence::init(&app, true);
    }
    crate::presence::apply_autostart(&app, general.autostart)
}

/* ── downloadable resources ──────────────────────────────────── */

#[derive(Debug, Clone, Serialize)]
pub struct ResourceState {
    pub id: String,
    pub installed: bool,
    pub bytes: u64,
    pub path: String,
}

fn resource_dir(id: &str) -> Result<PathBuf, String> {
    let dir = yura_store::data_dir()
        .map_err(|e| e.to_string())?
        .join("resources")
        .join(id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[tauri::command]
pub fn resource_status(ids: Vec<String>) -> Vec<ResourceState> {
    ids.into_iter()
        .map(|id| {
            let path = yura_store::data_dir()
                .map(|d| d.join("resources").join(&id))
                .unwrap_or_default();
            let bytes = walk_size(&path);
            ResourceState {
                id,
                installed: bytes > 0,
                bytes,
                path: path.to_string_lossy().to_string(),
            }
        })
        .collect()
}

fn walk_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| e.metadata().map(|m| if m.is_dir() { walk_size(&e.path()) } else { m.len() }).unwrap_or(0))
        .sum()
}

/// Download one file of a resource pack, reporting progress to the UI.
///
/// Written to `<id>/<filename>.part` first and renamed on success, so an
/// interrupted download can never masquerade as a complete one.
#[tauri::command]
pub async fn download_resource(
    app: AppHandle,
    id: String,
    url: String,
    filename: String,
) -> Result<ResourceState, String> {
    let dir = resource_dir(&id)?;
    let target = dir.join(&filename);
    let part = dir.join(format!("{filename}.part"));

    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);

    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut written: u64 = 0;
    let mut stream = response;
    use std::io::Write;

    while let Some(chunk) = stream.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        written += chunk.len() as u64;
        let _ = app.emit(
            "resource:progress",
            serde_json::json!({ "id": id, "written": written, "total": total }),
        );
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    std::fs::rename(&part, &target).map_err(|e| e.to_string())?;

    Ok(ResourceState {
        id,
        installed: true,
        bytes: written,
        path: target.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn remove_resource(id: String) -> Result<(), String> {
    let dir = resource_dir(&id)?;
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())
}
