use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{project::Project, session::Session};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppState {
    pub projects: HashMap<Uuid, Project>,
    pub sessions: HashMap<Uuid, Session>,
}

pub fn save_state(state: &AppState) -> color_eyre::Result<()> {
    let path = crate::config::get_data_dir().join("state.json");
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_state() -> color_eyre::Result<AppState> {
    let path = crate::config::get_data_dir().join("state.json");
    if !path.exists() {
        return Ok(AppState {
            projects: HashMap::new(),
            sessions: HashMap::new(),
        });
    }
    let json = std::fs::read_to_string(path)?;
    let state = serde_json::from_str(&json)?;
    Ok(state)
}
