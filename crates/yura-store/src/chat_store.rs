use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use yura_core::Role;

use crate::{io_err, Result, StoreError};

/// One conversation.
///
/// A chat is a single JSON file under `<data>/chats/`. That choice is the same
/// one made for settings: no database, no migrations, and a file the user can
/// read, back up or delete by hand. Renaming and pinning are just field writes;
/// deleting is one `remove_file`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
    /// Unix milliseconds.
    pub created: u64,
    pub updated: u64,
    #[serde(default)]
    pub messages: Vec<Turn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: Role,
    pub content: String,
    pub ts: u64,
}

impl Turn {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            ts: now_ms(),
        }
    }

    pub fn to_message(&self) -> yura_core::Message {
        yura_core::Message::new(self.role, self.content.clone())
    }
}

/// Lightweight row for the sidebar — no message bodies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub updated: u64,
    pub turns: usize,
    /// First user line, for the second row of the sidebar item.
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct ChatStore {
    dir: PathBuf,
}

impl ChatStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            dir: data_dir.as_ref().join("chats"),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(io_err(&self.dir))
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", sanitise(id)))
    }

    /// Newest first, pinned first. Unreadable files are skipped rather than
    /// breaking the list.
    pub fn list(&self) -> Vec<ChatSummary> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };

        let mut out: Vec<ChatSummary> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|entry| {
                let raw = std::fs::read_to_string(entry.path()).ok()?;
                let chat: Chat = serde_json::from_str(&raw).ok()?;
                let preview = chat
                    .messages
                    .iter()
                    .find(|t| t.role == Role::User)
                    .map(|t| t.content.chars().take(90).collect::<String>())
                    .unwrap_or_default();

                Some(ChatSummary {
                    id: chat.id,
                    title: chat.title,
                    pinned: chat.pinned,
                    updated: chat.updated,
                    turns: chat.messages.len(),
                    preview,
                })
            })
            .collect();

        out.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated.cmp(&a.updated))
        });
        out
    }

    pub fn load(&self, id: &str) -> Option<Chat> {
        let raw = std::fs::read_to_string(self.path_for(id)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn create(&self, title: impl Into<String>) -> Result<Chat> {
        self.ensure()?;
        let stamp = now_ms();
        let chat = Chat {
            id: format!("c{stamp:x}"),
            title: title.into(),
            pinned: false,
            created: stamp,
            updated: stamp,
            messages: Vec::new(),
        };
        self.save(&chat)?;
        Ok(chat)
    }

    pub fn save(&self, chat: &Chat) -> Result<()> {
        self.ensure()?;
        let path = self.path_for(&chat.id);
        let body = serde_json::to_vec_pretty(chat).map_err(|source| StoreError::Parse {
            path: path.display().to_string(),
            source,
        })?;

        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &body).map_err(io_err(&tmp))?;
        std::fs::rename(&tmp, &path).map_err(io_err(&path))?;
        Ok(())
    }

    /// Append a turn and touch `updated`. Creates the chat if it vanished
    /// (deleted in another window, say) so a send never fails on a stale id.
    pub fn append(&self, id: &str, role: Role, content: &str) -> Result<Chat> {
        let mut chat = self.load(id).unwrap_or(Chat {
            id: id.to_owned(),
            title: "New conversation".into(),
            pinned: false,
            created: now_ms(),
            updated: now_ms(),
            messages: Vec::new(),
        });

        chat.messages.push(Turn::new(role, content));
        chat.updated = now_ms();

        // First user message becomes the title, like every chat app ever.
        if role == Role::User && chat.messages.iter().filter(|t| t.role == Role::User).count() == 1 {
            chat.title = title_from(content);
        }

        self.save(&chat)?;
        Ok(chat)
    }

    pub fn rename(&self, id: &str, title: &str) -> Result<()> {
        if let Some(mut chat) = self.load(id) {
            chat.title = title.trim().chars().take(80).collect();
            self.save(&chat)?;
        }
        Ok(())
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<()> {
        if let Some(mut chat) = self.load(id) {
            chat.pinned = pinned;
            self.save(&chat)?;
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.path_for(id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io {
                path: path.display().to_string(),
                source: e,
            }),
        }
    }

    /// Most recent chats as model messages, newest last.
    pub fn context(&self, id: &str, turns: usize) -> Vec<yura_core::Message> {
        let Some(chat) = self.load(id) else {
            return Vec::new();
        };
        let max = turns.saturating_mul(2);
        let start = chat.messages.len().saturating_sub(max);
        chat.messages[start..]
            .iter()
            .filter(|t| t.role != Role::System)
            .map(Turn::to_message)
            .collect()
    }
}

fn title_from(text: &str) -> String {
    let clean = text.trim().replace('\n', " ");
    let short: String = clean.chars().take(48).collect();
    if short.trim().is_empty() {
        "New conversation".into()
    } else if clean.chars().count() > 48 {
        format!("{}…", short.trim_end())
    } else {
        short
    }
}

fn sanitise(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> ChatStore {
        let dir = std::env::temp_dir().join(format!("yura-chats-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        ChatStore::new(dir)
    }

    #[test]
    fn creates_appends_and_titles_from_first_message() {
        let s = store("title");
        let chat = s.create("New conversation").unwrap();
        s.append(&chat.id, Role::User, "explain entropy please").unwrap();

        let loaded = s.load(&chat.id).unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.title, "explain entropy please");
    }

    #[test]
    fn list_puts_pinned_first_then_newest() {
        let s = store("order");
        let a = s.create("A").unwrap();
        let b = s.create("B").unwrap();
        let c = s.create("C").unwrap();

        s.append(&b.id, Role::User, "second").unwrap();
        s.set_pinned(&c.id, true).unwrap();

        let list = s.list();
        assert_eq!(list[0].id, c.id, "pinned first");
        assert_eq!(list[1].id, b.id, "then most recently updated");
    }

    #[test]
    fn rename_and_delete() {
        let s = store("rename");
        let chat = s.create("x").unwrap();
        s.rename(&chat.id, "Kinetics").unwrap();
        assert_eq!(s.load(&chat.id).unwrap().title, "Kinetics");

        s.delete(&chat.id).unwrap();
        s.delete(&chat.id).unwrap(); // idempotent
        assert!(s.load(&chat.id).is_none());
        assert!(s.list().is_empty());
    }

    #[test]
    fn context_respects_turn_limit_and_order() {
        let s = store("ctx");
        let chat = s.create("c").unwrap();
        for i in 0..10 {
            s.append(&chat.id, Role::User, &format!("u{i}")).unwrap();
            s.append(&chat.id, Role::Assistant, &format!("a{i}")).unwrap();
        }
        let ctx = s.context(&chat.id, 2);
        assert_eq!(ctx.len(), 4);
        assert_eq!(ctx.last().unwrap().content, "a9");
    }

    #[test]
    fn append_to_missing_chat_recreates_it() {
        let s = store("ghost");
        let chat = s.append("deadbeef", Role::User, "still here").unwrap();
        assert_eq!(chat.messages.len(), 1);
        assert!(s.load("deadbeef").is_some());
    }

    #[test]
    fn long_titles_are_truncated() {
        let s = store("long");
        let chat = s.create("t").unwrap();
        let long = "a".repeat(200);
        s.append(&chat.id, Role::User, &long).unwrap();
        let title = s.load(&chat.id).unwrap().title;
        assert!(title.chars().count() <= 49, "title: {title}");
        assert!(title.ends_with('…'));
    }
}
