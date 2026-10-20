use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, VariantNames};
use uuid::Uuid;

use crate::worktree::Worktree;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(skip)]
    pub worktrees: Vec<Worktree>,
}

/// Scripts run when one of the project's sessions is created or removed
#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct Hooks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    #[serde(default)]
    pub setup_mode: SetupMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teardown: Option<String>,
}

/// Where a setup script runs
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Display,
    EnumString,
    VariantNames,
)]
#[strum(serialize_all = "lowercase")]
pub enum SetupMode {
    /// Typed into the new session's shell
    #[default]
    Pane,
    /// Run before the session is created, with its output hidden
    Background,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{Hooks, Project, SetupMode};

    #[test]
    fn test_deserialize_ignores_legacy_sessions_field() {
        let id = Uuid::new_v4();
        let json = format!(
            r#"{{"id":"{id}","name":"project","path":"/repo","sessions":["{}"]}}"#,
            Uuid::new_v4()
        );
        let project: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(
            project,
            Project {
                id,
                name: String::from("project"),
                path: String::from("/repo"),
                hooks: Hooks::default(),
                worktrees: vec![],
            }
        );
    }

    #[test]
    fn test_hooks_roundtrip() {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::from("project"),
            path: String::from("/repo"),
            hooks: Hooks {
                setup: Some(String::from("make setup")),
                setup_mode: SetupMode::Background,
                teardown: Some(String::from("make clean")),
            },
            worktrees: vec![],
        };
        let json = serde_json::to_string(&project).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), project);
    }

    #[test]
    fn test_setup_mode_parses_from_display() {
        for mode in [SetupMode::Pane, SetupMode::Background] {
            assert_eq!(mode.to_string().parse::<SetupMode>().unwrap(), mode);
        }
    }
}
