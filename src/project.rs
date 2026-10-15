use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::worktree::Worktree;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    #[serde(skip)]
    pub worktrees: Vec<Worktree>,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::Project;

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
                worktrees: vec![],
            }
        );
    }
}
