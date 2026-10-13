use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

use crate::{project::Project, session::Session, worktree::Worktree};

/// Active tmux session names, and refreshed worktrees per project
pub type TmuxSnapshot = (Vec<String>, HashMap<Uuid, Vec<Worktree>>);

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
    CmdAddSessionInProject,
    CmdDeleteItem,
    CmdJumpTop,
    CmdJumpBottom,
    CmdStartSearch,
    CmdSearchNext,
    CmdSearchPrev,
    CmdAttach,
    CmdEdit,
    CmdNextSessionHistory,
    CmdPrevSessionHistory,
    // Input
    CancelInput, // TODO: This should be removed in favour of FormPopup cancellation
    // Project
    SubmitProject(Project),
    UpdateProject(Project),
    RemoveProject(Project),
    ProjectCreationFinished {
        project: Project,
        result: Result<Option<Vec<Worktree>>, String>,
    },
    ProjectUpdateFinished {
        project: Project,
        result: Result<Option<Vec<Worktree>>, String>,
    },
    ProjectRemovalFinished {
        project_id: Uuid,
        result: Result<(), String>,
    },
    // Session
    SubmitSession(Session),
    UpdateSession(Session),
    AttachSession(Session),
    RemoveSession(Session),
    SessionCreationFinished {
        session: Session,
        result: Result<Option<Vec<Worktree>>, String>,
    },
    SessionUpdateFinished {
        session: Session,
        result: Result<(), String>,
    },
    SessionRemovalFinished {
        session: Session,
        result: Result<(), String>,
    },
    // Worktrees
    RemoveWorktree(Project, Worktree),
    WorktreeRemovalFinished {
        project_id: Uuid,
        result: Result<Vec<Worktree>, String>,
    },
    // State
    TmuxSyncFinished {
        known_session_ids: HashSet<Uuid>,
        result: Result<TmuxSnapshot, String>,
    },
    StateUpdated(HashMap<Uuid, Project>, HashMap<Uuid, Session>),
}
