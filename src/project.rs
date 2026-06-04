use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::git::Worktree;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub sessions: Vec<Uuid>,
    #[serde(skip)]
    pub worktrees: Vec<Worktree>,
}
