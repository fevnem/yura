use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use yura_core::{Result, YuraError};

use crate::http::client;
use crate::provider::{emit, emit_text, ChatRequest, Delta, LlmProvider};
use crate::sse::SseBuffer;

/// Anthropic Messages API (`POST {base}/messages`).
///
/// Differs from the OpenAI shape in three ways that matter: `system` is a
/// top-level field rather than a message, `max_tokens` is mandatory, and the
/// stream carries typed events instead of uniform deltas.
#[derive(Debug, Clone)]
pub struct Anthropic {
    pub base_url: String,
    pub api_key: String,
}

const API_VERSION: &str = "2023-06-01";

impl Anthropic {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/messages", self.base_url)
    }

    fn payload(&self, req: &ChatRequest) -> Value {
        let turns: Vec<Value> = req
            .merged_turns()
            .into_iter()
            .filter(|m| m.role != yura_core::Role::System)
            .map(|m| {
                json!({
                    "role": if m.role == yura_core::Role::Assistant { "assistant" } else { "user" },
                    "content": m.content,
                })
            })
            .collect();

        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "messages": turns,
            "temperature": req.temperature,
            "top_p": req.top_p,
            "stream": true,
        });
        if let Some(system) = req.system.as_deref().filter(|s| !s.trim().is_empty()) {
            body["system"] = json!(system);
        }
        body
    }
}

/// Text carried by a `content_block_delta` event, if this frame is one.
pub fn extract_delta(v: &Value) -> Option<&str> {
    if v.get("type").and_then(Value::as_str) != Some("content_block_delta") {
        return None;
    }
    let delta = v.get("delta")?;
    match delta.get("type").and_then(Value::as_str) {
        Some("text_delta") => delta.get("text").and_then(Value::as_str),
        _ => None,
    }
}

/// Extended-thinking deltas arrive as `thinking_delta` inside the block event.
pub fn extract_thinking(v: &Value) -> Option<&str> {
    if v.get("type").and_then(Value::as_str) != Some("content_block_delta") {
        return None;
    }
    let delta = v.get("delta")?;
    match delta.get("type").and_then(Value::as_str) {
        Some("thinking_delta") => delta.get("thinking").and_then(Value::as_str),
        _ => None,
    }
}

#[async_trait]
impl LlmProvider for Anthropic {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn verify_config(&self) -> Result<()> {
        if self.api_key.trim().is_empty() {
            return Err(YuraError::MissingApiKey("anthropic".into()));
        }
        if self.base_url.trim().is_empty() {
            return Err(YuraError::Other("base URL is empty".into()));
        }
        Ok(())
    }

    async fn stream_chat(
        &self,
        req: ChatRequest,
        sink: mpsc::UnboundedSender<Delta>,
    ) -> Result<()> {
        self.verify_config()?;

        let response = client()
            .post(self.endpoint())
            .header("x-api-key", self.api_key.trim())
            .header("anthropic-version", API_VERSION)
            .header("accept", "text/event-stream")
            .json(&self.payload(&req))
            .send()
            .await
            .map_err(crate::error::net)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(YuraError::Provider {
                provider: "anthropic".into(),
                status: status.as_u16(),
                body: crate::openai::truncate(&body, 600),
            });
        }

        let mut stream = response.bytes_stream();
        let mut sse = SseBuffer::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(crate::error::net)?;
            let text = String::from_utf8_lossy(&bytes);
            for payload in sse.push(&text) {
                if payload.trim() == "[DONE]" {
                    return Ok(());
                }
                let Ok(v) = serde_json::from_str::<Value>(&payload) else {
                    continue;
                };
                match v.get("type").and_then(Value::as_str) {
                    Some("error") => {
                        let msg = v
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(Value::as_str)
                            .unwrap_or("anthropic error");
                        return Err(YuraError::Other(msg.to_owned()));
                    }
                    Some("message_stop") => return Ok(()),
                    _ => {}
                }
                if let Some(thinking) = extract_thinking(&v) {
                    if !emit(&sink, Delta::Thinking(thinking.to_owned())) {
                        return Ok(());
                    }
                }
                if let Some(delta) = extract_delta(&v) {
                    if !emit_text(&sink, delta) {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yura_core::Message;

    #[test]
    fn system_is_top_level_not_a_message() {
        let a = Anthropic::new("https://api.anthropic.com/v1", "sk-ant");
        let req = ChatRequest {
            model: "claude".into(),
            system: Some("rules".into()),
            messages: vec![Message::user("hi"), Message::user("again")],
            temperature: 0.5,
            top_p: 1.0,
            max_tokens: 128,
        };
        let body = a.payload(&req);
        assert_eq!(body["system"], "rules");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1); // merged
        assert_eq!(body["max_tokens"], 128);
        assert_eq!(a.endpoint(), "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn only_text_deltas_are_extracted() {
        let text = json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"yo"}});
        assert_eq!(extract_delta(&text), Some("yo"));

        let other = json!({"type":"message_start","message":{}});
        assert_eq!(extract_delta(&other), None);

        let ping = json!({"type":"ping"});
        assert_eq!(extract_delta(&ping), None);
    }

    #[test]
    fn missing_key_is_reported_before_any_network_call() {
        let a = Anthropic::new("https://api.anthropic.com/v1", " ");
        assert!(matches!(
            a.verify_config(),
            Err(YuraError::MissingApiKey(_))
        ));
    }
}
