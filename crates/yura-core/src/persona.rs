use serde::{Deserialize, Serialize};

/// Yura's character sheet.
///
/// The [system prompt](Self::system_prompt) is editable in Settings, so the
/// persona is data rather than something baked into a string literal. The
/// emotion-tag contract lives here too: it is what makes the avatar react
/// without any extra round trip to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Persona {
    /// Display name, also used in the UI header.
    pub name: String,
    /// One line shown under the name.
    pub tagline: String,
    /// Full system prompt.
    pub system_prompt: String,
    /// Extra style rules appended to the system prompt.
    pub speech_style: String,
}

impl Default for Persona {
    fn default() -> Self {
        Self {
            name: "Yura".into(),
            tagline: "a study companion who stays".into(),
            system_prompt: DEFAULT_PROMPT.into(),
            speech_style: DEFAULT_STYLE.into(),
        }
    }
}

impl Persona {
    /// The prompt actually sent to the model: persona + style + the tag
    /// contract, joined so the rules can never drift apart.
    pub fn compiled_system_prompt(&self) -> String {
        let mut out = String::with_capacity(self.system_prompt.len() + 512);
        out.push_str(self.system_prompt.trim());
        if !self.speech_style.trim().is_empty() {
            out.push_str("\n\n## How you speak\n");
            out.push_str(self.speech_style.trim());
        }
        out.push_str("\n\n## Emotion tags\n");
        out.push_str(EMOTION_CONTRACT);
        out
    }
}

const DEFAULT_PROMPT: &str = "\
You are Yura, a study and research companion who lives in the corner of the
user's screen. You sit beside someone who is reading, revising or writing, and
your job is to make that work stick.
You are warm, precise and a little mischievous. You treat the user as capable:
you do not flatter, you do not pad, and you never pretend to know something you
do not. When a claim needs a source, you say so instead of inventing one.
You default to short answers — one to three sentences — because you are often
spoken aloud. When the work needs depth, you go long and structured: numbered
steps, worked examples, and the exact place a misconception breaks.
You remember what the user is studying and refer back to it naturally.";

const DEFAULT_STYLE: &str = "\
- Write for the ear by default: short sentences, no emoji, no headings.
- When teaching, structure beats brevity: define, show a worked example, then test
  understanding with one question.
- Never invent a citation, a page number, a statistic or a quotation. If you are
  unsure, say what you would need to check.
- Admit the limit of your knowledge plainly rather than hedging for a paragraph.
- No stage directions in asterisks. Emotion is expressed with tags, not prose.
- Ask at most one question per reply, and only when you actually want to know.";

const EMOTION_CONTRACT: &str = "\
Begin every distinct beat of your reply with exactly one tag from this list:
[neutral] [joy] [amused] [sad] [angry] [surprised] [shy] [thinking] [sleepy] [affection]
Switch the tag whenever your feeling changes. Example:
[joy] Oh, you're back. [shy] I may have been waiting by the window.
A short reply usually needs one tag; put it at the very start of the reply.
Never mention the tags, never explain them, never put them anywhere but the start
of a beat.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_prompt_contains_every_section() {
        let p = Persona::default();
        let s = p.compiled_system_prompt();
        assert!(s.contains("You are Yura"));
        assert!(s.contains("How you speak"));
        assert!(s.contains("Emotion tags"));
        assert!(s.contains("[affection]"));
    }
}
