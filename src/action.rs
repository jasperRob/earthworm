use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

use crate::{
    pending::{JobId, Pending},
    project::Project,
    session::Session,
    worktree::Worktree,
};

/// Active tmux session names, and refreshed worktrees per project (None if they could not be fetched)
pub type TmuxSnapshot = (Vec<String>, HashMap<Uuid, Option<Vec<Worktree>>>);

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
    // Jobs
    /// A background job's completion, handled together with ending the job
    JobFinished(JobId, JobCompletion),
    // State
    StateUpdated(HashMap<Uuid, Project>, HashMap<Uuid, Session>, Vec<Pending>),
}

/// Outcome of a background job, carrying what is needed to apply its result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobCompletion {
    ProjectCreation {
        project: Project,
        result: Result<Option<Vec<Worktree>>, String>,
    },
    ProjectUpdate {
        project: Project,
        result: Result<Option<Vec<Worktree>>, String>,
    },
    ProjectRemoval {
        project_id: Uuid,
        result: Result<(), String>,
    },
    SessionCreation {
        session: Session,
        result: Result<Option<Vec<Worktree>>, String>,
    },
    SessionUpdate {
        session: Session,
        result: Result<(), String>,
    },
    SessionRemoval {
        session: Session,
        result: Result<(), String>,
    },
    WorktreeRemoval {
        project_id: Uuid,
        result: Result<Vec<Worktree>, String>,
    },
    TmuxSync {
        known_session_ids: HashSet<Uuid>,
        result: Result<TmuxSnapshot, String>,
    },
}
