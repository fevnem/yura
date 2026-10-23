use std::path::{Path, PathBuf};

use yura_core::skill::Skill;

use crate::{io_err, Result, StoreError};

const SKILLS_DIR: &str = "skills";

/// Skills live as plain markdown in `<data dir>/skills/`.
///
/// One file per skill, named by its id. The user can open the folder, write a
/// `.md` in any editor, and it appears in Settings. Importing a file is just
/// copying it in — there is no database to migrate and no schema to break.
#[derive(Debug, Clone)]
pub struct SkillStore {
    dir: PathBuf,
}

impl SkillStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            dir: data_dir.as_ref().join(SKILLS_DIR),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(io_err(&self.dir))
    }

    /// Every skill on disk, alphabetical, unreadable files skipped.
    pub fn list(&self) -> Vec<Skill> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };

        let mut skills: Vec<Skill> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .filter_map(|entry| {
                let path = entry.path();
                let id = path.file_stem()?.to_str()?.to_owned();
                let raw = std::fs::read_to_string(&path).ok()?;
                Some(Skill::from_markdown(id, &raw))
            })
            .collect();

        skills.sort_by_key(|s| s.name.to_lowercase());
        skills
    }

    /// Write one skill. The id becomes the file name, sanitised.
    pub fn save(&self, skill: &Skill) -> Result<Skill> {
        self.ensure()?;
        let mut stored = skill.clone();
        stored.id = slug(&skill.id, &skill.name);
        let path = self.path_for(&stored.id);

        std::fs::write(&path, stored.to_markdown()).map_err(io_err(&path))?;
        Ok(stored)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.path_for(&slug(id, id));
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io {
                path: path.display().to_string(),
                source: e,
            }),
        }
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.md"))
    }
}

/// Filesystem-safe id: lowercase ascii, digits, dashes.
pub fn slug(id: &str, fallback: &str) -> String {
    let source = if id.trim().is_empty() { fallback } else { id };
    let mut out = String::with_capacity(source.len());
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | ' ' | '.') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "skill".to_owned()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each test gets its own directory: sharing one by pid makes the suite
    /// race when tests run in parallel.
    fn temp(name: &str) -> SkillStore {
        let dir = std::env::temp_dir().join(format!("yura-skills-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        SkillStore::new(dir)
    }

    #[test]
    fn saves_lists_and_reloads() {
        let store = temp("save");
        let mut skill = Skill::new("", "Paper digest");
        skill.description = "claims, method, limits".into();
        skill.body = "Read the abstract first.".into();

        let saved = store.save(&skill).unwrap();
        assert_eq!(saved.id, "paper-digest");

        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Paper digest");
        assert!(listed[0].body.contains("abstract"));
    }

    #[test]
    fn delete_is_idempotent() {
        let store = temp("delete");
        store.save(&Skill::new("gone", "Gone")).unwrap();
        store.delete("gone").unwrap();
        store.delete("gone").unwrap();
        assert!(store.list().is_empty());
    }

    #[test]
    fn slug_is_filesystem_safe() {
        assert_eq!(slug("Paper Digest v2!", "x"), "paper-digest-v2");
        assert_eq!(slug("", "My Skill"), "my-skill");
        assert_eq!(slug("///", "x"), "skill");
    }

    #[test]
    fn ignores_non_markdown() {
        let store = temp("nonmd");
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(store.dir().join("notes.txt"), "nope").unwrap();
        assert!(store.list().is_empty());
    }
}
