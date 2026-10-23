use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use yura_avatar::AvatarSpec;
use yura_core::Settings;
use yura_store::{data_dir, history_path, settings_path, History, SettingsStore, SkillStore};

/// Everything long-lived in the process.
///
/// Settings sit behind an `RwLock` because the UI may save them mid-stream;
/// mutation is rare and reads are cheap. The cancel flag is a plain atomic so
/// the streaming loop can check it without touching the lock.
pub struct AppState {
    pub settings: RwLock<Settings>,
    pub settings_store: SettingsStore,
    pub history: History,
    pub skills: SkillStore,
    pub cancel: Arc<AtomicBool>,
    pub avatar: AvatarSpec,
}

impl AppState {
    /// Load from disk once, healing anything broken. Never panics on bad input.
    pub fn bootstrap() -> Self {
        let settings_store = SettingsStore::new(settings_path().unwrap_or_else(|_| {
            std::path::PathBuf::from("settings.json")
        }));
        let history = History::new(history_path().unwrap_or_else(|_| {
            std::path::PathBuf::from("history.jsonl")
        }));

        let settings = settings_store.load();
        tracing::info!(
            provider = %settings.llm.provider_id,
            model = %settings.llm.model,
            "settings loaded"
        );

        let skills = SkillStore::new(data_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));

        // First run: give the user working examples instead of an empty page.
        if skills.list().is_empty() {
            for skill in yura_core::skill::defaults() {
                if let Err(e) = skills.save(&skill) {
                    tracing::warn!(error = %e, "could not install a starter skill");
                }
            }
        }

        Self {
            settings: RwLock::new(settings),
            settings_store,
            history,
            skills,
            cancel: Arc::new(AtomicBool::new(false)),
            avatar: yura_avatar::spec(),
        }
    }

    pub fn snapshot(&self) -> Settings {
        self.settings.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn begin_generation(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}
