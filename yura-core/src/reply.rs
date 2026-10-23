use crate::{Emotion, YuraError};
use serde::{Deserialize, Serialize};

/// One spoken beat: a chunk of text plus the feeling behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Utterance {
    pub emotion: Emotion,
    pub text: String,
}

impl Utterance {
    pub fn new(emotion: Emotion, text: impl Into<String>) -> Self {
        Self {
            emotion,
            text: text.into(),
        }
    }
}

/// A complete reply, already split into beats.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub utterances: Vec<Utterance>,
}

impl Reply {
    /// Full text with tags removed — what belongs in conversation history and
    /// in the chat transcript.
    pub fn plain_text(&self) -> String {
        self.utterances
            .iter()
            .map(|u| u.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn is_empty(&self) -> bool {
        self.utterances.iter().all(|u| u.text.trim().is_empty())
    }
}

/// Incremental parser for the `[emotion]` tag protocol.
///
/// Fed one streamed delta at a time, it emits a [`Utterance`] as soon as the
/// *next* tag arrives — so the app can start speaking the first sentence while
/// the model is still writing the second, and the avatar's face can change
/// mid-reply. Tags that are cut in half by a chunk boundary are held in a
/// pending buffer rather than leaking into the transcript.
#[derive(Debug)]
pub struct ReplyParser {
    emotion: Emotion,
    current: String,
    pending: String,
    emitted: Vec<Utterance>,
}

impl Default for ReplyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplyParser {
    pub fn new() -> Self {
        Self {
            emotion: Emotion::Neutral,
            current: String::new(),
            pending: String::new(),
            emitted: Vec::new(),
        }
    }

    /// Feed a delta. Returns any beats completed by this chunk.
    pub fn feed(&mut self, delta: &str) -> Vec<Utterance> {
        let mut out = Vec::new();
        self.pending.push_str(delta);

        loop {
            let Some(open) = self.pending.find('[') else {
                self.current.push_str(&self.pending);
                self.pending.clear();
                break;
            };

            // Text before the bracket is speech.
            if open > 0 {
                self.current.push_str(&self.pending[..open]);
                self.pending.drain(..open);
            }

            // pending now starts with '['.
            let Some(close) = self.pending.find(']') else {
                // Tag is split across chunks — wait for the rest.
                break;
            };

            let tag: String = self.pending[1..close].to_owned();
            self.pending.drain(..=close);

            match tag.parse::<Emotion>() {
                Ok(emotion) => {
                    if let Some(done) = self.take_current() {
                        out.push(done);
                    }
                    self.emotion = emotion;
                }
                // `[laughs]`, `[1]` and friends are not emotions: keep them.
                Err(_) => {
                    self.current.push('[');
                    self.current.push_str(&tag);
                    self.current.push(']');
                }
            }
        }

        self.emitted.extend(out.iter().cloned());
        out
    }

    /// Call once the stream ends. Emits the final beat, if any.
    pub fn finish(mut self) -> (Vec<Utterance>, Reply) {
        // An unterminated tag at EOF is literal text.
        if !self.pending.is_empty() {
            self.current.push_str(&self.pending);
            self.pending.clear();
        }
        let tail = self.take_current();
        if let Some(t) = tail {
            self.emitted.push(t);
        }
        let reply = Reply {
            utterances: self.emitted.clone(),
        };
        (self.emitted, reply)
    }

    /// Text visible so far, with tags stripped — used to paint the streaming
    /// bubble before the reply is complete.
    pub fn visible_text(&self) -> String {
        let mut s = String::with_capacity(self.emitted.len() * 32 + self.current.len());
        for u in &self.emitted {
            s.push_str(u.text.trim());
            s.push(' ');
        }
        s.push_str(self.current.trim());
        s.trim().to_owned()
    }

    fn take_current(&mut self) -> Option<Utterance> {
        let text = std::mem::take(&mut self.current);
        let text = text.trim().to_owned();
        if text.is_empty() {
            None
        } else {
            Some(Utterance {
                emotion: self.emotion,
                text,
            })
        }
    }
}

/// Convenience: parse a complete string in one shot.
pub fn parse(raw: &str) -> Result<Reply, YuraError> {
    let mut p = ReplyParser::new();
    p.feed(raw);
    let (_, reply) = p.finish();
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_tags() {
        let r = parse("[joy] You're back! [shy] I was waiting.").unwrap();
        assert_eq!(r.utterances.len(), 2);
        assert_eq!(r.utterances[0].emotion, Emotion::Joy);
        assert_eq!(r.utterances[0].text, "You're back!");
        assert_eq!(r.utterances[1].emotion, Emotion::Shy);
        assert_eq!(r.plain_text(), "You're back! I was waiting.");
    }

    #[test]
    fn holds_partial_tags_across_chunks() {
        let mut p = ReplyParser::new();
        assert!(p.feed("[jo").is_empty());
        assert_eq!(p.visible_text(), "");
        // Second half completes nothing yet, third starts the speech.
        assert!(p.feed("y] Hmm").is_empty());
        assert_eq!(p.visible_text(), "Hmm");
        let out = p.feed("... [sad] it's fine.");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].emotion, Emotion::Joy);
        assert_eq!(out[0].text, "Hmm...");
        let (all, reply) = p.finish();
        assert_eq!(all.len(), 2);
        assert_eq!(reply.utterances[1].emotion, Emotion::Sad);
    }

    #[test]
    fn keeps_non_emotion_brackets() {
        let r = parse("[neutral] A [laughs] quiet sound.").unwrap();
        assert_eq!(r.utterances[0].text, "A [laughs] quiet sound.");
    }

    #[test]
    fn untagged_reply_is_neutral() {
        let r = parse("Just talking.").unwrap();
        assert_eq!(r.utterances[0].emotion, Emotion::Neutral);
    }

    #[test]
    fn empty_and_whitespace_only() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse("[joy]   ").unwrap().is_empty());
    }

    #[test]
    fn streaming_matches_batch() {
        let raw = "[joy] one. [thinking] two. [affection] three.";
        let batch = parse(raw).unwrap();
        let mut p = ReplyParser::new();
        for ch in raw.split_inclusive(|_| true) {
            p.feed(ch);
        }
        let (_, streamed) = p.finish();
        assert_eq!(batch.utterances, streamed.utterances);
    }
}
