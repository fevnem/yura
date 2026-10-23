use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use yura_core::{Message, Role};

use crate::{io_err, Result, StoreError};

/// One line of `history.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub role: Role,
    pub content: String,
    /// Unix milliseconds — used for ordering and for the transcript's time
    /// separators; never sent to a model.
    pub ts: u64,
}

impl HistoryEntry {
    pub fn now(role: Role, content: impl Into<String>) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            role,
            content: content.into(),
            ts,
        }
    }

    pub fn into_message(self) -> Message {
        Message::new(self.role, self.content)
    }
}

/// Append-only conversation log.
///
/// Deliberately dumb: `append` is a single line write, `load` is a full read.
/// At a few thousand turns that is microseconds, and it means history can never
/// be corrupted by a half-finished transaction.
#[derive(Debug, Clone)]
pub struct History {
    path: PathBuf,
}

impl History {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn append(&self, entry: &HistoryEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io_err(&self.path))?;

        let mut line = serde_json::to_string(entry).map_err(|source| StoreError::Parse {
            path: self.path.display().to_string(),
            source,
        })?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .map_err(io_err(&self.path))?;
        Ok(())
    }

    pub fn append_turn(&self, role: Role, content: impl Into<String>) -> Result<()> {
        self.append(&HistoryEntry::now(role, content))
    }

    /// Read every entry, oldest first. Unparseable lines are skipped rather
    /// than aborting the load — one bad byte should not erase a conversation.
    pub fn load_all(&self) -> Vec<HistoryEntry> {
        let Ok(file) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        BufReader::new(file)
            .lines()
            .map_while(std::result::Result::ok)
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<HistoryEntry>(&line).ok())
            .collect()
    }

    /// The last `limit` *turns*, flattened into model messages.
    ///
    /// `limit` counts user/assistant pairs, so a `limit` of 12 keeps roughly 24
    /// messages — which is how a human thinks about "how much do you remember".
    pub fn recent_messages(&self, limit: usize) -> Vec<Message> {
        if limit == 0 {
            return Vec::new();
        }
        let max_messages = limit * 2;
        let all = self.load_all();
        let start = all.len().saturating_sub(max_messages);
        all[start..]
            .iter()
            .filter(|e| e.role != Role::System)
            .map(|e| Message::new(e.role, e.content.clone()))
            .collect()
    }

    /// Most recent entries, newest last — what the transcript panel renders.
    pub fn recent(&self, limit: usize) -> Vec<HistoryEntry> {
        let all = self.load_all();
        let start = all.len().saturating_sub(limit);
        all[start..].to_vec()
    }

    /// Truncate the log. Keeps the file (and its inode) in place.
    pub fn clear(&self) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(io_err(&self.path))?;
        drop(file);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_history(name: &str) -> History {
        let dir = std::env::temp_dir().join(format!(
            "yura-hist-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        History::new(dir.join("history.jsonl"))
    }

    #[test]
    fn appends_and_reads_back_in_order() {
        let h = temp_history("order");
        h.append_turn(Role::User, "hello").unwrap();
        h.append_turn(Role::Assistant, "hi there").unwrap();

        let all = h.load_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].content, "hello");
        assert_eq!(all[1].role, Role::Assistant);
    }

    #[test]
    fn recent_respects_turn_limit() {
        let h = temp_history("limit");
        for i in 0..10 {
            h.append_turn(Role::User, format!("u{i}")).unwrap();
            h.append_turn(Role::Assistant, format!("a{i}")).unwrap();
        }
        let msgs = h.recent_messages(3);
        assert_eq!(msgs.len(), 6);
        assert_eq!(msgs.first().unwrap().content, "u7");
        assert_eq!(msgs.last().unwrap().content, "a9");
    }

    #[test]
    fn zero_limit_returns_nothing() {
        let h = temp_history("zero");
        h.append_turn(Role::User, "x").unwrap();
        assert!(h.recent_messages(0).is_empty());
    }

    #[test]
    fn skips_corrupt_lines_and_clears() {
        let h = temp_history("corrupt");
        h.append_turn(Role::User, "good").unwrap();
        let mut f = OpenOptions::new().append(true).open(h.path()).unwrap();
        f.write_all(b"{oops\n").unwrap();
        drop(f);

        assert_eq!(h.load_all().len(), 1);
        h.clear().unwrap();
        assert!(h.load_all().is_empty());
    }

    #[test]
    fn missing_file_is_empty_not_an_error() {
        let h = temp_history("missing");
        assert!(h.load_all().is_empty());
        assert!(h.recent(5).is_empty());
    }
}
