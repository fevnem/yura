use std::io::Write;
use std::path::{Path, PathBuf};

use yura_core::Settings;

use crate::{io_err, Result, StoreError};

/// Reads and writes `settings.json`.
///
/// Writes go to a sibling temp file and are then renamed over the original, so
/// a crash mid-save can never leave a truncated config behind — the failure
/// mode users actually hit is "app won't start, settings.json is 0 bytes".
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load settings, healing anything unusable.
    ///
    /// A missing file yields defaults. A corrupt file is moved aside to
    /// `settings.json.corrupt` so the user can inspect it, and defaults are
    /// returned — never a hard failure to launch.
    pub fn load(&self) -> Settings {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            Err(e) => {
                tracing::warn!(error = %e, path = %self.path.display(), "settings unreadable, using defaults");
                return Settings::default();
            }
        };

        match serde_json::from_str::<Settings>(&raw) {
            Ok(mut settings) => {
                settings.normalize();
                settings
            }
            Err(e) => {
                tracing::warn!(error = %e, "settings.json is invalid, moving it aside");
                let _ = std::fs::rename(&self.path, self.path.with_extension("json.corrupt"));
                Settings::default()
            }
        }
    }

    /// Persist atomically. Creates the parent directory if needed.
    pub fn save(&self, settings: &Settings) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }

        let body = serde_json::to_vec_pretty(settings).map_err(|source| StoreError::Parse {
            path: self.path.display().to_string(),
            source,
        })?;

        let tmp = self.path.with_extension("json.tmp");
        {
            let mut file = std::fs::File::create(&tmp).map_err(io_err(&tmp))?;
            file.write_all(&body).map_err(io_err(&tmp))?;
            file.sync_all().map_err(io_err(&tmp))?;
        }
        std::fs::rename(&tmp, &self.path).map_err(io_err(&self.path))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yura-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = temp_dir("missing");
        let store = SettingsStore::new(dir.join("settings.json"));
        assert_eq!(store.load().persona.name, "Yura");
    }

    #[test]
    fn round_trips() {
        let dir = temp_dir("roundtrip");
        let store = SettingsStore::new(dir.join("settings.json"));
        let mut s = Settings::default();
        s.llm.api_key = "sk-test".into();
        s.persona.name = "Yura".into();
        store.save(&s).unwrap();

        let loaded = store.load();
        assert_eq!(loaded.llm.api_key, "sk-test");
        assert!(!dir.join("settings.json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_quarantined_not_fatal() {
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{ this is not json").unwrap();
        drop(f);

        let store = SettingsStore::new(&path);
        assert_eq!(store.load().version, yura_core::SETTINGS_VERSION);
        assert!(dir.join("settings.json.corrupt").exists());
    }

    #[test]
    fn partial_config_is_filled_in() {
        let dir = temp_dir("partial");
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{"llm":{"model":"custom-model"}}"#).unwrap();
        let s = SettingsStore::new(&path).load();
        assert_eq!(s.llm.model, "custom-model");
        assert_eq!(s.persona.name, "Yura"); // default survived
        assert_eq!(s.voice.rate, 1.04);
    }
}
