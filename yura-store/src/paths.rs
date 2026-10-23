use std::path::PathBuf;

use crate::{Result, StoreError};

const APP_DIR: &str = "Yura";
const SETTINGS_FILE: &str = "settings.json";
const HISTORY_FILE: &str = "history.jsonl";

/// Per-user data directory. Created on first use.
pub fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or(StoreError::NoDataDir)?;
    Ok(base.join(APP_DIR))
}

/// Same as [`data_dir`] but guarantees the directory exists.
pub fn ensure_data_dir() -> Result<PathBuf> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| StoreError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    Ok(dir)
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(ensure_data_dir()?.join(SETTINGS_FILE))
}

pub fn history_path() -> Result<PathBuf> {
    Ok(ensure_data_dir()?.join(HISTORY_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_live_under_one_directory() {
        let dir = data_dir().unwrap();
        assert!(dir.ends_with("Yura"));
        assert!(settings_path().unwrap().starts_with(&dir));
        assert!(history_path().unwrap().ends_with("history.jsonl"));
    }
}
