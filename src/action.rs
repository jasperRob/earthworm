use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

use crate::{git::Worktree, project::Project, session::Session};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
    // Commands
    CmdSelectNext,
    CmdSelectPrev,
    CmdManageProjects,
    CmdAddSession,
    CmdDeleteItem,
    CmdJumpTop,
    CmdJumpBottom,
    CmdStartSearch,
    CmdSearchNext,
    CmdSearchPrev,
    // Internal
    SubmitInput(String),
    CancelInput,
    CmdAttach,
    SubmitProject(Project),
    SubmitSession(Session),
    AttachSession(Session),
    RemoveSession(Session),
    RemoveWorktree(Project, Worktree),
    StateUpdated(HashMap<Uuid, Project>, HashMap<Uuid, Session>),
}
