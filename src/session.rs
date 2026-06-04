use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub path: String,
    pub worktree: String,
}
