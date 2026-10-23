//! # yura-store
//!
//! Everything that survives a restart, and nothing else.
//!
//! Two artifacts live in the user's data directory (`%APPDATA%\Yura` on
//! Windows, `~/.local/share/yura` elsewhere):
//!
//! * `settings.json` — written atomically, human-editable, versioned
//! * `history.jsonl` — append-only turns, one JSON object per line
//!
//! JSONL rather than SQLite is a deliberate trade: a companion app's history is
//! small, and an append-only text file means no native dependency, no
//! migrations, and a file the user can read or delete at will. The
//! [`History`] API is the seam where a SQLite backend can slot in later without
//! touching callers.

pub mod history;
pub mod paths;
pub mod settings_store;

pub use history::{History, HistoryEntry};
pub use paths::{data_dir, ensure_data_dir, history_path, settings_path};
pub use settings_store::SettingsStore;

/// Errors raised by the persistence layer.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("no data directory available for this user")]
    NoDataDir,
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

pub(crate) fn io_err(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.display().to_string(),
        source,
    }
}
