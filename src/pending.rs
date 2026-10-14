use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

use crate::{project::Project, session::Session};

pub type JobId = u64;

/// What a background job is working on
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PendingTarget {
    App,
    Project(Uuid),
    Session(Uuid),
    Worktree { project_id: Uuid, path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize)]
#[strum(serialize_all = "lowercase")]
pub enum Activity {
    Syncing,
    Creating,
    Updating,
    Removing,
}

/// An entity to show in the list while its creation is still running
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Placeholder {
    Project(Project),
    Session(Session),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub target: PendingTarget,
    pub activity: Activity,
    pub placeholder: Option<Placeholder>,
}

impl Pending {
    pub fn new(target: PendingTarget, activity: Activity) -> Self {
        Self {
            target,
            activity,
            placeholder: None,
        }
    }

    pub fn creating_project(project: &Project) -> Self {
        Self {
            target: PendingTarget::Project(project.id),
            activity: Activity::Creating,
            placeholder: Some(Placeholder::Project(project.clone())),
        }
    }

    pub fn creating_session(session: &Session) -> Self {
        Self {
            target: PendingTarget::Session(session.id),
            activity: Activity::Creating,
            placeholder: Some(Placeholder::Session(session.clone())),
        }
    }
}

/// Adds the placeholders of pending creations to the given state, so they are listed like real entries
pub fn apply_placeholders(
    projects: &mut HashMap<Uuid, Project>,
    sessions: &mut HashMap<Uuid, Session>,
    pending: &[Pending],
) {
    // Projects first, so a session placeholder can always find its project
    for pending in pending {
        if let Some(Placeholder::Project(project)) = &pending.placeholder {
            projects
                .entry(project.id)
                .or_insert_with(|| project.clone());
        }
    }
    for pending in pending {
        if let Some(Placeholder::Session(session)) = &pending.placeholder {
            sessions
                .entry(session.id)
                .or_insert_with(|| session.clone());
            if let Some(project_id) = session.project_id
                && let Some(project) = projects.get_mut(&project_id)
                && !project.sessions.contains(&session.id)
            {
                project.sessions.push(session.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use super::{Activity, Pending, PendingTarget, apply_placeholders};
    use crate::{project::Project, session::Session};

    fn project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "project".to_string(),
            path: String::new(),
            sessions: Vec::new(),
            worktrees: Vec::new(),
        }
    }

    fn session(project_id: Option<Uuid>) -> Session {
        Session {
            id: Uuid::new_v4(),
            project_id,
            name: "session".to_string(),
            path: None,
            worktree: None,
        }
    }

    #[test]
    fn test_apply_placeholders_adds_pending_project() {
        let project = project();
        let mut projects = HashMap::new();
        let mut sessions = HashMap::new();
        apply_placeholders(
            &mut projects,
            &mut sessions,
            &[Pending::creating_project(&project)],
        );
        assert_eq!(projects.get(&project.id), Some(&project));
    }

    #[test]
    fn test_apply_placeholders_adds_pending_session_to_its_project() {
        let project = project();
        let session = session(Some(project.id));
        let mut projects = HashMap::from([(project.id, project.clone())]);
        let mut sessions = HashMap::new();
        apply_placeholders(
            &mut projects,
            &mut sessions,
            &[Pending::creating_session(&session)],
        );
        assert_eq!(sessions.get(&session.id), Some(&session));
        assert_eq!(projects[&project.id].sessions, vec![session.id]);
    }

    #[test]
    fn test_apply_placeholders_adds_session_to_pending_project_in_any_order() {
        let project = project();
        let session = session(Some(project.id));
        let mut projects = HashMap::new();
        let mut sessions = HashMap::new();
        apply_placeholders(
            &mut projects,
            &mut sessions,
            &[
                Pending::creating_session(&session),
                Pending::creating_project(&project),
            ],
        );
        assert_eq!(projects[&project.id].sessions, vec![session.id]);
    }

    #[test]
    fn test_apply_placeholders_keeps_existing_entries() {
        let mut project = project();
        let session = session(Some(project.id));
        project.sessions.push(session.id);
        let mut placeholder = session.clone();
        placeholder.name = "placeholder".to_string();
        let mut projects = HashMap::from([(project.id, project.clone())]);
        let mut sessions = HashMap::from([(session.id, session.clone())]);
        apply_placeholders(
            &mut projects,
            &mut sessions,
            &[Pending::creating_session(&placeholder)],
        );
        assert_eq!(sessions[&session.id], session);
        assert_eq!(projects[&project.id].sessions, vec![session.id]);
    }

    #[test]
    fn test_apply_placeholders_ignores_pending_without_placeholder() {
        let project = project();
        let mut projects = HashMap::from([(project.id, project.clone())]);
        let mut sessions = HashMap::new();
        apply_placeholders(
            &mut projects,
            &mut sessions,
            &[Pending::new(
                PendingTarget::Project(project.id),
                Activity::Removing,
            )],
        );
        assert_eq!(projects.len(), 1);
        assert!(sessions.is_empty());
    }
}
