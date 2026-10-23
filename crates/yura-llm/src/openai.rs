use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use yura_core::{Message, Result, Role, YuraError};

use crate::http::client;
use crate::provider::{emit, ChatRequest, LlmProvider};
use crate::sse::SseBuffer;

/// Any server that speaks `POST {base}/chat/completions` with SSE deltas.
///
/// That covers OpenAI, DeepSeek, OpenRouter, Groq, Mistral, xAI, Together,
/// Ollama, LM Studio and llama.cpp's built-in server — the reason this one
/// module is worth more than a dozen vendor-specific ones.
#[derive(Debug, Clone)]
pub struct OpenAiCompatible {
    pub base_url: String,
    pub api_key: String,
    /// Some gateways (OpenRouter) like to know who is calling.
    pub referer: Option<String>,
}

impl OpenAiCompatible {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            referer: None,
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn payload(&self, req: &ChatRequest) -> Value {
        let mut messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
        if let Some(system) = req.system.as_deref().filter(|s| !s.trim().is_empty()) {
            messages.push(json!({ "role": "system", "content": system }));
        }
        messages.extend(
            req.messages
                .iter()
                .filter(|m| m.role != Role::System)
                .map(|m| json!({ "role": m.role.as_str(), "content": m.content })),
        );

        json!({
            "model": req.model,
            "messages": messages,
            "temperature": req.temperature,
            "top_p": req.top_p,
            "max_tokens": req.max_tokens,
            "stream": true,
            "stream_options": { "include_usage": false },
        })
    }
}

/// Pull the text out of one `chat.completion.chunk` frame.
pub fn extract_delta(v: &Value) -> Option<&str> {
    v.get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()
        .filter(|s| !s.is_empty())
}

#[async_trait]
impl LlmProvider for OpenAiCompatible {
    fn id(&self) -> &'static str {
        "openai_compatible"
    }

    fn verify_config(&self) -> Result<()> {
        if self.base_url.trim().is_empty() {
            return Err(YuraError::Other("base URL is empty".into()));
        }
        Ok(())
    }

    async fn stream_chat(
        &self,
        req: ChatRequest,
        sink: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        self.verify_config()?;

        let mut builder = client()
            .post(self.endpoint())
            .json(&self.payload(&req))
            .header("accept", "text/event-stream");
        if !self.api_key.trim().is_empty() {
            builder = builder.bearer_auth(self.api_key.trim());
        }
        if let Some(referer) = &self.referer {
            builder = builder
                .header("HTTP-Referer", referer)
                .header("X-Title", "Yura");
        }

        let response = builder.send().await.map_err(crate::error::net)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(YuraError::Provider {
                provider: "openai_compatible".into(),
                status: status.as_u16(),
                body: truncate(&body, 600),
            });
        }

        let mut stream = response.bytes_stream();
        let mut sse = SseBuffer::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(crate::error::net)?;
            let text = String::from_utf8_lossy(&bytes);
            for payload in sse.push(&text) {
                if SseBuffer::is_done(&payload) {
                    return Ok(());
                }
                match serde_json::from_str::<Value>(&payload) {
                    Ok(v) => {
                        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
                            let msg = err
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("provider error");
                            return Err(YuraError::Other(msg.to_owned()));
                        }
                        if let Some(delta) = extract_delta(&v) {
                            if !emit(&sink, delta) {
                                return Ok(()); // consumer gone
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(?e, frame = %payload, "skipping malformed SSE frame");
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

/// Convenience for building the alternating history a chat endpoint expects.
pub fn history_view(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .filter(|m| m.role != Role::System)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_extraction_handles_usage_only_frames() {
        let chunk = json!({"choices":[{"delta":{"content":"hi"}}]});
        assert_eq!(extract_delta(&chunk), Some("hi"));

        let usage = json!({"choices":[],"usage":{"total_tokens":9}});
        assert_eq!(extract_delta(&usage), None);

        let tool = json!({"choices":[{"delta":{"role":"assistant"}}]});
        assert_eq!(extract_delta(&tool), None);
    }

    #[test]
    fn payload_puts_system_first_and_skips_dupes() {
        let p = OpenAiCompatible::new("https://x/v1/", "");
        let req = ChatRequest {
            model: "m".into(),
            system: Some("be nice".into()),
            messages: vec![Message::system("old"), Message::user("hello")],
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 64,
        };
        let body = p.payload(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["content"], "hello");
        assert_eq!(body["stream"], true);
        assert_eq!(p.endpoint(), "https://x/v1/chat/completions");
    }
}
