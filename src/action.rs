use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

use crate::{project::Project, session::Session, worktree::Worktree};

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
    CmdAddProject,
    CmdAddSession,
    CmdDeleteItem,
    CmdJumpTop,
    CmdJumpBottom,
    CmdStartSearch,
    CmdSearchNext,
    CmdSearchPrev,
    CmdAttach,
    CmdEdit,
    // Input
    CancelInput, // TODO: This should be removed in favour of FormPopup cancellation
    // Project
    SubmitProject(Project),
    UpdateProject(Project),
    RemoveProject(Project),
    // Session
    SubmitSession(Session),
    UpdateSession(Session),
    AttachSession(Session),
    RemoveSession(Session),
    // Worktrees
    RemoveWorktree(Project, Worktree),
    // State
    StateUpdated(HashMap<Uuid, Project>, HashMap<Uuid, Session>),
}
