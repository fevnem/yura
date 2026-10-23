//! Server-Sent Events framing.
//!
//! Chunks arrive split at arbitrary byte offsets, so a `data:` line can be cut
//! in half mid-JSON. [`SseBuffer`] reassembles full lines and drops the
//! comment/keep-alive chatter both OpenAI and Anthropic emit.

/// Reassembles `data:` payloads from a byte stream of SSE frames.
#[derive(Debug, Default)]
pub struct SseBuffer {
    buf: String,
}

impl SseBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a raw chunk. Returns every complete `data:` payload it produced.
    pub fn push(&mut self, chunk: &str) -> Vec<String> {
        self.buf.push_str(chunk);
        let mut out = Vec::new();

        while let Some(nl) = self.buf.find('\n') {
            let line = self.buf[..nl].trim_end_matches('\r').to_owned();
            self.buf.drain(..=nl);

            if line.is_empty() || line.starts_with(':') {
                continue; // keep-alive comment
            }
            let Some(payload) = line.strip_prefix("data:") else {
                continue; // `event:` / `id:` / `retry:` are not needed
            };
            let payload = payload.trim_start();
            if payload.is_empty() {
                continue;
            }
            out.push(payload.to_owned());
        }
        out
    }

    /// True when the terminal sentinel has been seen.
    pub fn is_done(payload: &str) -> bool {
        payload.trim() == "[DONE]"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassembles_split_lines() {
        let mut s = SseBuffer::new();
        assert!(s.push("data: {\"a\":").is_empty());
        assert_eq!(s.push("1}\n"), vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn ignores_comments_and_non_data_fields() {
        let mut s = SseBuffer::new();
        let out = s.push(": ping\nevent: message\ndata: hi\n\n");
        assert_eq!(out, vec!["hi".to_string()]);
    }

    #[test]
    fn handles_crlf_and_multiple_frames_in_one_chunk() {
        let mut s = SseBuffer::new();
        let out = s.push("data: a\r\ndata: b\r\n");
        assert_eq!(out, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn detects_sentinel() {
        assert!(SseBuffer::is_done("[DONE]"));
        assert!(!SseBuffer::is_done("{\"x\":1}"));
    }
}
