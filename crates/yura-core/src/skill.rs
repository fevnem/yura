use serde::{Deserialize, Serialize};

/// A skill is a markdown file the user owns.
///
/// It is deliberately *not* a prompt snippet buried in code: it lives in
/// `%APPDATA%\Yura\skills\*.md`, it is human-editable, and it is injected into
/// the system prompt only while enabled. That makes the app programmable by the
/// person using it, without shipping a plugin runtime.
///
/// Format — optional front matter, then the instructions:
///
/// ```text
/// ---
/// name: Paper digest
/// description: Summarise an academic paper into claims, method, limits.
/// enabled: true
/// ---
/// When the user attaches a paper, answer in this order: ...
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    /// File stem, stable across renames of the display name.
    pub id: String,
    pub name: String,
    pub description: String,
    /// The markdown instructions themselves — what the model actually reads.
    pub body: String,
    pub enabled: bool,
}

impl Skill {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: id.into(),
            name: name.clone(),
            description: String::new(),
            body: String::new(),
            enabled: true,
        }
    }

    /// Parse a `.md` file. Front matter is optional; a plain markdown file
    /// becomes a skill whose name is the file stem, so dropping a note in the
    /// folder is enough.
    pub fn from_markdown(id: impl Into<String>, raw: &str) -> Self {
        let id = id.into();
        let mut skill = Skill::new(id.clone(), humanize(&id));
        let text = raw.replace("\r\n", "\n");

        if let Some(rest) = text.strip_prefix("---\n") {
            if let Some(end) = rest.find("\n---") {
                let (front, body) = rest.split_at(end);
                for line in front.lines() {
                    let Some((key, value)) = line.split_once(':') else {
                        continue;
                    };
                    let value = value.trim().trim_matches('"').trim_matches('\'');
                    match key.trim().to_ascii_lowercase().as_str() {
                        "name" => {
                            if !value.is_empty() {
                                skill.name = value.to_owned();
                            }
                        }
                        "description" | "summary" => skill.description = value.to_owned(),
                        "enabled" => skill.enabled = parse_bool(value),
                        _ => {}
                    }
                }
                skill.body = body
                    .trim_start_matches("\n---")
                    .trim_start_matches('\n')
                    .trim()
                    .to_owned();
                return skill;
            }
        }

        skill.body = text.trim().to_owned();
        skill
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::with_capacity(self.body.len() + 200);
        out.push_str("---\n");
        out.push_str(&format!("name: {}\n", self.name.trim()));
        out.push_str(&format!("description: {}\n", self.description.trim()));
        out.push_str(&format!("enabled: {}\n", self.enabled));
        out.push_str("---\n\n");
        out.push_str(self.body.trim());
        out.push('\n');
        out
    }

    /// Compact one-line summary for the skills list.
    pub fn summary(&self) -> String {
        if !self.description.trim().is_empty() {
            return self.description.trim().to_owned();
        }
        let first = self
            .body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#'))
            .unwrap_or("");
        truncate(first, 120)
    }
}

/// Build the block appended to the system prompt.
///
/// Skills are framed as *how to work*, not *what to say*, because a skill that
/// fights the persona produces a muddled voice. Empty when nothing is enabled.
pub fn compile(skills: &[Skill]) -> String {
    let active: Vec<&Skill> = skills.iter().filter(|s| s.enabled && !s.body.trim().is_empty()).collect();
    if active.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n\n## Skills\nYou have been given the following skills. Apply them when they fit the request; when a skill conflicts with your usual style, the skill wins on method and you keep your voice.\n");
    for skill in active {
        out.push_str("\n### Skill: ");
        out.push_str(skill.name.trim());
        if !skill.description.trim().is_empty() {
            out.push_str(" — ");
            out.push_str(skill.description.trim());
        }
        out.push('\n');
        out.push_str(skill.body.trim());
        out.push('\n');
    }
    out
}

/// Starter skills installed into an empty skills folder.
///
/// Three, not thirty: they demonstrate the format and are genuinely useful on
/// day one for study work. The user can delete any of them.
pub fn defaults() -> Vec<Skill> {
    let mut socratic = Skill::new("socratic-tutor", "Socratic tutor");
    socratic.description = "Teach by question, not by lecture.".into();
    socratic.body = "\nWhen the user is trying to understand something:\n1. Ask what they already believe about it, in one question.\n2. Find the smallest wrong assumption in their answer.\n3. Ask a question that makes that assumption fail, rather than correcting it outright.\n4. Only after they have moved, state the correct model in two sentences.\nNever give the full answer to the first message of a study question.".into();

    let mut digest = Skill::new("paper-digest", "Paper digest");
    digest.description = "Summarise a paper as claims, method, evidence, limits.".into();
    digest.body = "\nWhen the user shares a paper or an abstract, answer in this order and nothing else:\n1. **The claim**, in one sentence.\n2. **Method**, three bullets maximum.\n3. **What the evidence actually supports** — separate what was measured from what was inferred.\n4. **The strongest objection** a reviewer would raise.\n5. **One question** worth asking the authors.\nDo not praise the paper. Do not summarise the introduction.".into();

    let mut recall = Skill::new("recall-drill", "Recall drill");
    recall.description = "Quiz from material the user has already studied.".into();
    recall.body = "\nWhen the user asks to be quizzed:\n- Draw only on what they have already shared in this conversation; never introduce new facts.\n- Ask one question at a time and wait.\n- Mix recall (what is X?) with application (what happens if Y?).\n- After each answer, say correct or not, give the one-line reason, then move on.\n- Track a running score and end the drill when they say stop, with the two things worth revisiting.".into();

    vec![socratic, digest, recall]
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

fn humanize(id: &str) -> String {
    let spaced = id.replace(['_', '-'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Untitled skill".to_owned(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    s.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_front_matter() {
        let raw = "---\nname: Paper digest\ndescription: Claims, method, limits.\nenabled: false\n---\n\nDo the thing.\n";
        let s = Skill::from_markdown("paper-digest", raw);
        assert_eq!(s.name, "Paper digest");
        assert_eq!(s.description, "Claims, method, limits.");
        assert!(!s.enabled);
        assert_eq!(s.body, "Do the thing.");
    }

    #[test]
    fn plain_markdown_becomes_a_skill() {
        let s = Skill::from_markdown("study_notes", "# Steps\nRead, then quiz me.");
        assert_eq!(s.name, "Study notes");
        assert!(s.enabled);
        assert!(s.body.contains("quiz me"));
    }

    #[test]
    fn round_trips_through_markdown() {
        let mut s = Skill::new("x", "X skill");
        s.description = "does x".into();
        s.body = "body text".into();
        let back = Skill::from_markdown("x", &s.to_markdown());
        assert_eq!(back, s);
    }

    #[test]
    fn compile_skips_disabled_and_empty() {
        let mut a = Skill::new("a", "Active");
        a.body = "Be brief.".into();
        let mut b = Skill::new("b", "Off");
        b.body = "Never shown.".into();
        b.enabled = false;
        let empty = Skill::new("c", "Empty");

        let block = compile(&[a, b, empty]);
        assert!(block.contains("Active"));
        assert!(block.contains("Be brief."));
        assert!(!block.contains("Never shown."));
        assert!(!block.contains("Empty"));
    }

    #[test]
    fn compile_is_empty_when_nothing_enabled() {
        assert!(compile(&[]).is_empty());
        assert!(compile(&[Skill::new("x", "X")]).is_empty());
    }

    #[test]
    fn crlf_files_parse() {
        let s = Skill::from_markdown("win", "---\r\nname: Win\r\n---\r\nBody\r\n");
        assert_eq!(s.name, "Win");
        assert!(s.body.contains("Body"));
    }
}
