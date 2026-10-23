use async_trait::async_trait;
use tokio::sync::mpsc;
use yura_core::{Message, Result};

/// A single request to a chat model.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    /// Persona / rules. Sent as the `system` role (or Anthropic's `system`).
    pub system: Option<String>,
    /// History plus the new user turn, oldest first.
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
}

impl ChatRequest {
    /// Alternating-role view required by Anthropic: consecutive messages from
    /// the same role are merged instead of rejected by the API.
    pub fn merged_turns(&self) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::with_capacity(self.messages.len());
        for m in &self.messages {
            match out.last_mut() {
                Some(last) if last.role == m.role => {
                    last.content.push_str("\n\n");
                    last.content.push_str(&m.content);
                }
                _ => out.push(m.clone()),
            }
        }
        out
    }
}

/// One streamed piece of a reply.
///
/// Reasoning models emit their scratchpad separately from the answer; keeping
/// the two channels distinct lets the UI show "thinking" without polluting the
/// transcript or speaking it aloud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    Text(String),
    Thinking(String),
}

/// Every backend: stream tokens into `sink`, return when finished.
///
/// Deltas are pushed as `String` because that is exactly what the UI needs to
/// append; anything richer would force the frontend to re-assemble text.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Stable id, matches a [`yura_core::PRESETS`] entry.
    fn id(&self) -> &'static str;

    /// Cheap pre-flight: is the configuration plausible? No network call.
    fn verify_config(&self) -> Result<()>;

    /// Stream the reply. Returns `Ok(())` after the last delta.
    async fn stream_chat(&self, req: ChatRequest, sink: mpsc::UnboundedSender<Delta>) -> Result<()>;
}

/// Send a delta, ignoring the case where the consumer hung up (the user hit
/// stop, or the window closed mid-sentence).
pub(crate) fn emit(sink: &mpsc::UnboundedSender<Delta>, delta: Delta) -> bool {
    sink.send(delta).is_ok()
}

pub(crate) fn emit_text(sink: &mpsc::UnboundedSender<Delta>, text: impl Into<String>) -> bool {
    emit(sink, Delta::Text(text.into()))
}
