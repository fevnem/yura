//! The IPC surface.
//!
//! Every function here is a thin coordinator: validate, read state, delegate to
//! a domain crate, emit events. Business logic lives in `yura-core`,
//! `yura-llm`, `yura-avatar` and `yura-store` — this module should never grow a
//! rule of its own.
//!
//! Streaming protocol (all payloads are JSON):
//! * `chat:start`     `{ text }`                      — the user turn was accepted
//! * `chat:delta`     `{ delta, visible }`            — raw token + cleaned-so-far
//! * `chat:utterance` `{ emotion, text }`             — a completed beat, ready to speak
//! * `chat:done`      `{ text, reply }`               — final, with parsed beats
//! * `chat:cancelled` `{ text }`                      — stopped by the user
//! * `chat:error`     `{ message }`                   — provider or network failure
//! * `settings:changed` `{ settings }`                — state the UI should re-read

use std::sync::atomic::Ordering;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;
use yura_core::{Message, ReplyParser, Role, Settings, PRESETS};
use yura_llm::ChatRequest;
use yura_store::HistoryEntry;

use crate::state::AppState;

/// Static provider catalogue, shaped for the settings dropdown.
#[derive(Serialize)]
pub struct PresetDto {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub requires_key: bool,
}

/// Everything the UI needs on first paint, in one round trip.
#[derive(Serialize)]
pub struct Bootstrap {
    pub settings: Settings,
    pub presets: Vec<PresetDto>,
    pub avatar: yura_avatar::AvatarSpec,
    pub history: Vec<HistoryEntry>,
    pub version: String,
    pub data_dir: String,
    /// False until a model endpoint is usable — drives the setup card.
    pub configured: bool,
}

fn preset_list() -> Vec<PresetDto> {
    PRESETS
        .iter()
        .map(|p| PresetDto {
            id: p.id,
            label: p.label,
            kind: p.kind.as_str(),
            base_url: p.base_url,
            default_model: p.default_model,
            requires_key: p.requires_key,
        })
        .collect()
}

fn is_configured(settings: &Settings) -> bool {
    let key_ok = match yura_core::preset(&settings.llm.provider_id) {
        Some(p) => !p.requires_key || !settings.llm.api_key.trim().is_empty(),
        None => !settings.llm.base_url.trim().is_empty(),
    };
    key_ok && !settings.llm.model.trim().is_empty() && !settings.llm.base_url.trim().is_empty()
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let settings = state.snapshot();
    Bootstrap {
        configured: is_configured(&settings),
        presets: preset_list(),
        avatar: state.avatar.clone(),
        history: state.history.recent(200),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        data_dir: state
            .settings_store
            .path()
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        settings,
    }
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
) -> Result<Settings, String> {
    settings.normalize();

    if let Err(e) = state.settings_store.save(&settings) {
        tracing::error!(error = %e, "could not persist settings");
        return Err(format!("could not save settings: {e}"));
    }
    if let Ok(mut guard) = state.settings.write() {
        *guard = settings.clone();
    }
    if let Some(window) = app.get_webview_window("main") {
        crate::window::restore(&window, &settings);
    }
    app.emit("settings:changed", json!({ "settings": settings }))
        .ok();

    Ok(settings)
}

#[tauri::command]
pub fn apply_provider_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Settings, String> {
    let mut settings = state.snapshot();
    let previous = settings.llm.provider_id.clone();
    settings.llm.apply_preset(&id);

    // Switching to a vendor that already has a stored key should not wipe it.
    if previous != id {
        settings.llm.api_key = String::new();
    }
    settings.normalize();
    let _ = state.settings_store.save(&settings);
    if let Ok(mut guard) = state.settings.write() {
        *guard = settings.clone();
    }
    app.emit("settings:changed", json!({ "settings": settings }))
        .ok();
    Ok(settings)
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err("nothing to say".into());
    }

    let settings = state.snapshot();
    let provider = yura_llm::build(&settings.llm).map_err(|e| e.to_string())?;

    let mut messages = state.history.recent_messages(settings.llm.context_turns);
    messages.push(Message::user(&text));

    let request = ChatRequest {
        model: settings.llm.model.clone(),
        system: Some(settings.persona.compiled_system_prompt()),
        messages,
        temperature: settings.llm.temperature,
        top_p: settings.llm.top_p,
        max_tokens: settings.llm.max_tokens,
    };

    if let Err(e) = state.history.append_turn(Role::User, &text) {
        tracing::warn!(error = %e, "could not record the user turn");
    }
    state.begin_generation();
    app.emit("chat:start", json!({ "text": text })).ok();

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let handle = tokio::spawn(async move { provider.stream_chat(request, tx).await });

    let cancel = state.cancel.clone();
    let mut parser = ReplyParser::new();
    let mut aborted = false;

    while let Some(delta) = rx.recv().await {
        if cancel.load(Ordering::SeqCst) {
            aborted = true;
            handle.abort();
            break;
        }
        for utterance in parser.feed(&delta) {
            app.emit("chat:utterance", &utterance).ok();
        }
        let visible = parser.visible_text();
        app.emit("chat:delta", json!({ "delta": delta, "visible": visible }))
            .ok();
    }

    if aborted {
        let (_, reply) = parser.finish();
        let plain = reply.plain_text();
        if !plain.is_empty() {
            let _ = state.history.append_turn(Role::Assistant, &plain);
        }
        app.emit(
            "chat:cancelled",
            json!({ "text": plain, "reply": reply }),
        )
        .ok();
        return Ok(());
    }

    match handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let message = e.to_string();
            tracing::warn!(error = %message, "provider failed");
            app.emit("chat:error", json!({ "message": message })).ok();
            return Err(message);
        }
        Err(e) => {
            let message = format!("generation task failed: {e}");
            app.emit("chat:error", json!({ "message": message })).ok();
            return Err(message);
        }
    }

    let (_, reply) = parser.finish();
    let plain = reply.plain_text();
    if !plain.is_empty() {
        if let Err(e) = state.history.append_turn(Role::Assistant, &plain) {
            tracing::warn!(error = %e, "could not record the assistant turn");
        }
    }
    app.emit("chat:done", json!({ "text": plain, "reply": reply }))
        .ok();
    Ok(())
}

#[tauri::command]
pub fn cancel_generation(state: State<'_, AppState>) {
    state.request_cancel();
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history.clear().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recent_history(state: State<'_, AppState>, limit: usize) -> Vec<HistoryEntry> {
    state.history.recent(limit.clamp(1, 500))
}

#[tauri::command]
pub fn set_always_on_top(app: AppHandle, on: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is gone".to_string())?;
    window.set_always_on_top(on).map_err(|e| e.to_string())
}
