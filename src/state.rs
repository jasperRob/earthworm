use std::{
    collections::HashMap,
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{project::Project, session::Session};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppState {
    pub projects: HashMap<Uuid, Project>,
    pub sessions: HashMap<Uuid, Session>,
}

fn state_path() -> PathBuf {
    crate::config::get_data_dir().join("state.json")
}

pub fn save_state(state: &AppState) -> color_eyre::Result<()> {
    save_state_to(&state_path(), state)
}

pub fn load_state() -> color_eyre::Result<AppState> {
    load_state_from(&state_path())
}

/// Moves an unreadable state file aside so it isn't overwritten by the next save.
pub fn backup_state() -> color_eyre::Result<PathBuf> {
    backup_state_file(&state_path())
}

fn with_suffix(path: &Path, suffix: &str) -> color_eyre::Result<PathBuf> {
    let mut name = OsString::from(
        path.file_name()
            .ok_or_else(|| eyre!("invalid state path: {}", path.display()))?,
    );
    name.push(suffix);
    Ok(path.with_file_name(name))
}

fn save_state_to(path: &Path, state: &AppState) -> color_eyre::Result<()> {
    let json = serde_json::to_string_pretty(state)?;
    let tmp = with_suffix(path, ".tmp")?;
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn load_state_from(path: &Path) -> color_eyre::Result<AppState> {
    if !path.exists() {
        return Ok(AppState::default());
    }
    let json = std::fs::read_to_string(path)?;
    let state = serde_json::from_str(&json)?;
    Ok(state)
}

fn backup_state_file(path: &Path) -> color_eyre::Result<PathBuf> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let backup = with_suffix(path, &format!(".corrupt-{ts}"))?;
    std::fs::rename(path, &backup)?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use super::{AppState, backup_state_file, load_state_from, save_state_to};
    use crate::session::Session;

    fn state() -> AppState {
        let id = Uuid::new_v4();
        AppState {
            projects: HashMap::new(),
            sessions: HashMap::from([(
                id,
                Session {
                    id,
                    project_id: None,
                    name: String::from("session"),
                    path: None,
                    worktree: None,
                },
            )]),
        }
    }

    #[test]
    fn test_save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let original = state();

        save_state_to(&path, &original).unwrap();
        let loaded = load_state_from(&path).unwrap();

        assert_eq!(
            serde_json::to_value(&loaded).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_state_from(&dir.path().join("state.json")).unwrap();

        assert!(loaded.projects.is_empty());
        assert!(loaded.sessions.is_empty());
    }

    #[test]
    fn test_load_corrupt_file_errors_and_backup_preserves_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "{ not json").unwrap();

        assert!(load_state_from(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");

        let backup = backup_state_file(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(backup.parent(), path.parent());
        assert!(
            backup
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("state.json.corrupt-")
        );
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ not json");

        save_state_to(&path, &AppState::default()).unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ not json");
    }

    #[test]
    fn test_save_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "old").unwrap();

        save_state_to(&path, &state()).unwrap();

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("state.json")]);
        assert!(load_state_from(&path).is_ok());
    }
}
