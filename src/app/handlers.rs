use std::collections::{HashMap, HashSet};

use color_eyre::eyre::{Result, eyre};
use uuid::Uuid;

use super::App;

use crate::{
    action::{Action, TmuxSnapshot},
    git::{create_worktree, fetch_worktrees, remove_worktree},
    pending::{Activity, Pending, PendingTarget},
    project::Project,
    session::Session,
    tmux::{
        attach_tmux_session, fetch_tmux_sessions, get_tmux_session_name, get_tmux_session_path,
        kill_tmux_session, new_tmux_session, rename_tmux_session,
    },
    worktree::Worktree,
};

impl App {
    pub(super) fn on_submit_project(&mut self, project: &Project) -> Result<()> {
        let project = project.clone();
        let repo_path = project.path.clone();
        self.spawn_tracked_job(
            Pending::creating_project(&project),
            move || Ok(refresh_worktrees(&repo_path)),
            move |result| Action::ProjectCreationFinished { project, result },
        )
    }

    pub(super) fn on_project_creation_finished(
        &mut self,
        project: &Project,
        result: &Result<Option<Vec<Worktree>>, String>,
    ) -> Result<()> {
        apply_project_change(&mut self.projects, project, result)?;
        self.persist_state();
        Ok(())
    }

    pub(super) fn on_update_project(&mut self, project: &Project) -> Result<()> {
        let Some(old_project) = self.projects.get(&project.id) else {
            return Err(eyre!("Could not find existing project"));
        };
        // Update the session name of any project sessions
        let renames: Vec<(String, String)> = if project.name != old_project.name {
            self.sessions
                .values()
                .filter(|s| s.project_id == Some(project.id))
                .map(|s| {
                    (
                        get_tmux_session_name(s, Some(old_project)),
                        get_tmux_session_name(s, Some(project)),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let repo_path = project.path.clone();
        let project = project.clone();

        self.spawn_tracked_job(
            Pending::new(PendingTarget::Project(project.id), Activity::Updating),
            move || {
                rename_tmux_sessions(renames)?;
                Ok(refresh_worktrees(&repo_path))
            },
            move |result| Action::ProjectUpdateFinished { project, result },
        )
    }

    pub(super) fn on_project_update_finished(
        &mut self,
        project: &Project,
        result: &Result<Option<Vec<Worktree>>, String>,
    ) -> Result<()> {
        apply_project_update(&mut self.projects, project, result)?;
        self.persist_state();
        Ok(())
    }

    pub(super) fn on_remove_project(&mut self, project: &Project) -> Result<()> {
        let project_id = project.id;
        let tmux_session_names: Vec<String> = self
            .sessions
            .values()
            .filter(|s| s.project_id == Some(project_id))
            .map(|s| get_tmux_session_name(s, Some(project)))
            .collect();

        self.spawn_tracked_job(
            Pending::new(PendingTarget::Project(project_id), Activity::Removing),
            move || {
                let errors: Vec<String> = tmux_session_names
                    .into_iter()
                    .filter_map(|name| {
                        kill_tmux_session(name.clone())
                            .err()
                            .map(|e| format!("Failed to kill session {}: {}", name, e))
                    })
                    .collect();
                if !errors.is_empty() {
                    return Err(eyre!(
                        "Some sessions failed to terminate: {}",
                        errors.join(", ")
                    ));
                }
                Ok(())
            },
            move |result| Action::ProjectRemovalFinished { project_id, result },
        )
    }

    pub(super) fn on_project_removal_finished(
        &mut self,
        project_id: Uuid,
        result: &Result<(), String>,
    ) -> Result<()> {
        // The project is removed even if some of its sessions failed to terminate
        apply_project_removal(&mut self.projects, &mut self.sessions, project_id);
        self.persist_state();
        result.as_ref().map_err(|e| eyre!("{e}"))?;
        Ok(())
    }

    pub(super) fn on_submit_session(&mut self, session: &Session) -> Result<()> {
        let project = self.get_session_project(session)?;

        // (repo_path, worktree_name, worktree_path) of a worktree that needs creating
        let new_worktree = match (project, &session.path, &session.worktree) {
            (Some(p), Some(path), Some(worktree))
                if !p.worktrees.iter().any(|wt| wt.name == worktree.name) =>
            {
                Some((p.path.clone(), worktree.name.clone(), path.clone()))
            }
            _ => None,
        };
        // Repo whose worktrees should be refreshed once the session exists
        let refresh_repo_path = project
            .filter(|p| session.worktree.is_some() && !p.path.is_empty())
            .map(|p| p.path.clone());

        let tmux_session_name = get_tmux_session_name(session, project);
        let tmux_session_path = get_tmux_session_path(session, project)?;
        let session = session.clone();

        self.spawn_tracked_job(
            Pending::creating_session(&session),
            move || {
                if let Some((repo_path, name, path)) = new_worktree {
                    create_worktree(&repo_path, &name, &path)?;
                }
                // Failure to create the tmux session leaves the worktree intact
                new_tmux_session(tmux_session_name, tmux_session_path)?;
                Ok(refresh_repo_path.map(|p| fetch_worktrees(&p).unwrap_or_default()))
            },
            move |result| Action::SessionCreationFinished { session, result },
        )
    }

    pub(super) fn on_session_creation_finished(
        &mut self,
        session: &Session,
        result: &Result<Option<Vec<Worktree>>, String>,
    ) -> Result<()> {
        apply_session_creation(&mut self.projects, &mut self.sessions, session, result)?;
        self.persist_state();
        Ok(())
    }

    pub(super) fn on_update_session(&mut self, session: &Session) -> Result<()> {
        // we need to update the name and path of the tmux session if changed
        let Some(old_session) = self.sessions.get(&session.id) else {
            return Err(eyre!("Could not find existing session"));
        };
        let old_session_project: Option<&Project> =
            old_session.project_id.and_then(|id| self.projects.get(&id));
        // update tmux session name
        let rename = (old_session.name != session.name).then(|| {
            (
                get_tmux_session_name(old_session, old_session_project),
                get_tmux_session_name(session, old_session_project),
            )
        });
        let session = session.clone();

        self.spawn_tracked_job(
            Pending::new(PendingTarget::Session(session.id), Activity::Updating),
            move || {
                if let Some((from, to)) = rename {
                    rename_tmux_session(from, to)?;
                }
                Ok(())
            },
            move |result| Action::SessionUpdateFinished { session, result },
        )
    }

    pub(super) fn on_session_update_finished(
        &mut self,
        session: &Session,
        result: &Result<(), String>,
    ) -> Result<()> {
        apply_session_update(&mut self.sessions, session, result)?;
        self.persist_state();
        Ok(())
    }

    pub(super) fn on_attach_session(&mut self, session: &Session) -> Result<()> {
        let project = self.get_session_project(session)?;
        let tmux_session_name = get_tmux_session_name(session, project);
        attach_tmux_session(tmux_session_name)?;
        Ok(())
    }

    pub(super) fn on_remove_session(&mut self, session: &Session) -> Result<()> {
        let project = self.get_session_project(session)?;
        let tmux_session_name = get_tmux_session_name(session, project);
        let session = session.clone();
        self.spawn_tracked_job(
            Pending::new(PendingTarget::Session(session.id), Activity::Removing),
            move || kill_tmux_session(tmux_session_name),
            move |result| Action::SessionRemovalFinished { session, result },
        )
    }

    pub(super) fn on_session_removal_finished(
        &mut self,
        session: &Session,
        result: &Result<(), String>,
    ) -> Result<()> {
        apply_session_removal(&mut self.projects, &mut self.sessions, session, result)?;
        self.persist_state();
        Ok(())
    }

    pub(super) fn on_remove_worktree(
        &mut self,
        project: &Project,
        worktree: &Worktree,
    ) -> Result<()> {
        let project_id = project.id;
        let repo_path = project.path.clone();
        let wt_path = worktree.path.clone();
        self.spawn_tracked_job(
            Pending::new(
                PendingTarget::Worktree {
                    project_id,
                    path: wt_path.clone(),
                },
                Activity::Removing,
            ),
            move || {
                remove_worktree(&repo_path, &wt_path)?;
                fetch_worktrees(&repo_path)
            },
            move |result| Action::WorktreeRemovalFinished { project_id, result },
        )
    }

    pub(super) fn on_worktree_removal_finished(
        &mut self,
        project_id: Uuid,
        result: &Result<Vec<Worktree>, String>,
    ) -> Result<()> {
        apply_worktree_removal(&mut self.projects, project_id, result)?;
        Ok(())
    }

    /// Reconciles the app's in-memory session state with what tmux actually has running.
    pub(super) fn start_tmux_sync(&mut self) -> Result<()> {
        // Only sessions known now may be pruned; ones created meanwhile are newer than the fetch
        let known_session_ids: HashSet<Uuid> = self.sessions.keys().copied().collect();
        let repo_paths: Vec<(Uuid, String)> = self
            .projects
            .values()
            .filter(|p| !p.path.is_empty())
            .map(|p| (p.id, p.path.clone()))
            .collect();

        self.spawn_tracked_job(
            Pending::new(PendingTarget::App, Activity::Syncing),
            move || {
                let active_session_names = fetch_tmux_sessions()?;
                let worktrees = repo_paths
                    .into_iter()
                    .map(|(id, path)| (id, fetch_worktrees(&path).unwrap_or_default()))
                    .collect();
                Ok((active_session_names, worktrees))
            },
            move |result| Action::TmuxSyncFinished {
                known_session_ids,
                result,
            },
        )
    }

    pub(super) fn on_tmux_sync_finished(
        &mut self,
        known_session_ids: &HashSet<Uuid>,
        result: &Result<TmuxSnapshot, String>,
    ) -> Result<()> {
        apply_tmux_sync(
            &mut self.projects,
            &mut self.sessions,
            known_session_ids,
            result,
        )?;
        Ok(())
    }
}

/// Worktrees to store for a project, or None if it has no repo path to read them from
fn refresh_worktrees(repo_path: &str) -> Option<Vec<Worktree>> {
    (!repo_path.is_empty()).then(|| fetch_worktrees(repo_path).unwrap_or_default())
}

fn rename_tmux_sessions(renames: Vec<(String, String)>) -> Result<()> {
    let mut successful: Vec<(String, String)> = Vec::new();

    // TODO: find solution for updating session paths too.
    for (from, to) in renames {
        if let Err(e) = rename_tmux_session(from.clone(), to.clone()) {
            // Rollback if any fail
            for (from, to) in successful {
                let _ = rename_tmux_session(to.clone(), from.clone());
            }
            return Err(eyre!("Failed to rename {from} to {to}: {e}"));
        }
        successful.push((from, to));
    }
    Ok(())
}

fn apply_project_change(
    projects: &mut HashMap<Uuid, Project>,
    project: &Project,
    result: &Result<Option<Vec<Worktree>>, String>,
) -> Result<()> {
    let worktrees = result.as_ref().map_err(|e| eyre!("{e}"))?;
    let mut project = project.clone();
    if let Some(worktrees) = worktrees {
        project.worktrees = worktrees.clone();
    }
    projects.insert(project.id, project);
    Ok(())
}

fn apply_project_update(
    projects: &mut HashMap<Uuid, Project>,
    project: &Project,
    result: &Result<Option<Vec<Worktree>>, String>,
) -> Result<()> {
    result.as_ref().map_err(|e| eyre!("{e}"))?;
    let Some(existing) = projects.get(&project.id) else {
        return Err(eyre!("Project no longer exists"));
    };
    // TODO: For now, we will just replace in state. In future, maybe think
    // about how this can be done with specific field updates. Could be safer.
    // Sessions may have been created while the update was running, so keep the current ones.
    let mut project = project.clone();
    project.sessions = existing.sessions.clone();
    apply_project_change(projects, &project, result)
}

fn apply_project_removal(
    projects: &mut HashMap<Uuid, Project>,
    sessions: &mut HashMap<Uuid, Session>,
    project_id: Uuid,
) {
    sessions.retain(|_, s| s.project_id != Some(project_id));
    projects.remove(&project_id);
}

fn apply_session_update(
    sessions: &mut HashMap<Uuid, Session>,
    session: &Session,
    result: &Result<(), String>,
) -> Result<()> {
    result.as_ref().map_err(|e| eyre!("{e}"))?;
    if !sessions.contains_key(&session.id) {
        return Err(eyre!("Session no longer exists"));
    }
    sessions.insert(session.id, session.clone());
    Ok(())
}

fn apply_session_removal(
    projects: &mut HashMap<Uuid, Project>,
    sessions: &mut HashMap<Uuid, Session>,
    session: &Session,
    result: &Result<(), String>,
) -> Result<()> {
    result.as_ref().map_err(|e| eyre!("{e}"))?;
    sessions.remove(&session.id);
    if let Some(project_id) = session.project_id
        && let Some(project) = projects.get_mut(&project_id)
    {
        project.sessions.retain(|id| *id != session.id);
    }
    Ok(())
}

fn apply_tmux_sync(
    projects: &mut HashMap<Uuid, Project>,
    sessions: &mut HashMap<Uuid, Session>,
    known_session_ids: &HashSet<Uuid>,
    result: &Result<TmuxSnapshot, String>,
) -> Result<()> {
    let (active_session_names, worktrees) = result.as_ref().map_err(|e| eyre!("{e}"))?;
    let active_session_names: HashSet<&str> =
        active_session_names.iter().map(String::as_str).collect();

    let session_names_in_state: HashSet<String> = sessions
        .values()
        .filter_map(|s| match s.project_id {
            None => Some(get_tmux_session_name(s, None)),
            Some(project_id) => projects
                .get(&project_id)
                .map(|project| get_tmux_session_name(s, Some(project))),
        })
        .collect();

    // Prune dead sessions
    sessions.retain(|id, session| {
        if !known_session_ids.contains(id) {
            return true;
        }
        let Some(project_id) = session.project_id else {
            return true;
        };
        let Some(project) = projects.get(&project_id) else {
            return false;
        };
        active_session_names.contains(get_tmux_session_name(session, Some(project)).as_str())
    });

    // Capture unmapped tmux sessions
    for name in active_session_names {
        if !session_names_in_state.contains(name) {
            let uuid = Uuid::new_v4();
            sessions.insert(
                uuid,
                Session {
                    id: uuid,
                    project_id: None,
                    name: name.to_string(),
                    path: None,
                    worktree: None,
                },
            );
        }
    }

    for (project_id, project_worktrees) in worktrees {
        if let Some(project) = projects.get_mut(project_id) {
            project.worktrees = project_worktrees.clone();
        }
    }

    Ok(())
}

fn apply_session_creation(
    projects: &mut HashMap<Uuid, Project>,
    sessions: &mut HashMap<Uuid, Session>,
    session: &Session,
    result: &Result<Option<Vec<Worktree>>, String>,
) -> Result<()> {
    let worktrees = result.as_ref().map_err(|e| eyre!("{e}"))?;
    sessions.insert(session.id, session.clone());
    if let Some(project_id) = session.project_id
        && let Some(project) = projects.get_mut(&project_id)
    {
        project.sessions.push(session.id);
        if let Some(worktrees) = worktrees {
            project.worktrees = worktrees.clone();
        }
    }
    Ok(())
}

fn apply_worktree_removal(
    projects: &mut HashMap<Uuid, Project>,
    project_id: Uuid,
    result: &Result<Vec<Worktree>, String>,
) -> Result<()> {
    let worktrees = result.as_ref().map_err(|e| eyre!("{e}"))?;
    if let Some(project) = projects.get_mut(&project_id) {
        project.worktrees = worktrees.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use uuid::Uuid;

    use super::{
        apply_project_change, apply_project_removal, apply_project_update, apply_session_creation,
        apply_session_removal, apply_session_update, apply_tmux_sync, apply_worktree_removal,
    };
    use crate::{project::Project, session::Session, worktree::Worktree};

    fn project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: String::from("project"),
            path: String::from("/repo"),
            sessions: vec![],
            worktrees: vec![worktree("old")],
        }
    }

    fn session(project_id: Option<Uuid>) -> Session {
        Session {
            id: Uuid::new_v4(),
            project_id,
            name: String::from("session"),
            path: None,
            worktree: None,
        }
    }

    fn worktree(name: &str) -> Worktree {
        Worktree {
            name: String::from(name),
            path: format!("/repo/{name}"),
        }
    }

    #[test]
    fn test_apply_worktree_removal_ok_replaces_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        apply_worktree_removal(&mut projects, id, &Ok(vec![worktree("new")])).unwrap();
        assert_eq!(projects[&id].worktrees, vec![worktree("new")]);
    }

    #[test]
    fn test_apply_worktree_removal_err_leaves_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let err = apply_worktree_removal(&mut projects, id, &Err("boom".into())).unwrap_err();
        assert_eq!(err.to_string(), "boom");
        assert_eq!(projects[&id].worktrees, vec![worktree("old")]);
    }

    #[test]
    fn test_apply_worktree_removal_unknown_project_is_noop() {
        let mut projects = HashMap::from([(Uuid::new_v4(), project())]);
        let before = projects.clone();
        apply_worktree_removal(&mut projects, Uuid::new_v4(), &Ok(vec![])).unwrap();
        assert_eq!(projects, before);
    }

    #[test]
    fn test_apply_session_creation_ok_inserts_session_and_refreshes_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let mut sessions = HashMap::new();
        let s = session(Some(id));
        apply_session_creation(
            &mut projects,
            &mut sessions,
            &s,
            &Ok(Some(vec![worktree("new")])),
        )
        .unwrap();
        assert_eq!(sessions.get(&s.id), Some(&s));
        assert_eq!(projects[&id].sessions, vec![s.id]);
        assert_eq!(projects[&id].worktrees, vec![worktree("new")]);
    }

    #[test]
    fn test_apply_session_creation_ok_without_refresh_keeps_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let mut sessions = HashMap::new();
        let s = session(Some(id));
        apply_session_creation(&mut projects, &mut sessions, &s, &Ok(None)).unwrap();
        assert_eq!(projects[&id].worktrees, vec![worktree("old")]);
    }

    #[test]
    fn test_apply_session_creation_err_changes_nothing() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let mut sessions = HashMap::new();
        let s = session(Some(id));
        let before = projects.clone();
        let err = apply_session_creation(&mut projects, &mut sessions, &s, &Err("boom".into()))
            .unwrap_err();
        assert_eq!(err.to_string(), "boom");
        assert!(sessions.is_empty());
        assert_eq!(projects, before);
    }

    #[test]
    fn test_apply_project_change_inserts_with_refreshed_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::new();
        apply_project_change(&mut projects, &p, &Ok(Some(vec![worktree("new")]))).unwrap();
        assert_eq!(projects[&id].worktrees, vec![worktree("new")]);
    }

    #[test]
    fn test_apply_project_change_without_refresh_keeps_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::new();
        apply_project_change(&mut projects, &p, &Ok(None)).unwrap();
        assert_eq!(projects[&id].worktrees, vec![worktree("old")]);
    }

    #[test]
    fn test_apply_project_change_err_changes_nothing() {
        let mut projects = HashMap::new();
        let err = apply_project_change(&mut projects, &project(), &Err("boom".into())).unwrap_err();
        assert_eq!(err.to_string(), "boom");
        assert!(projects.is_empty());
    }

    #[test]
    fn test_apply_project_update_keeps_current_sessions() {
        let mut p = project();
        let id = p.id;
        let session_id = Uuid::new_v4();
        p.sessions = vec![session_id];
        let mut projects = HashMap::from([(id, p.clone())]);
        let mut updated = p;
        updated.name = String::from("renamed");
        updated.sessions = vec![];
        apply_project_update(&mut projects, &updated, &Ok(None)).unwrap();
        assert_eq!(projects[&id].name, "renamed");
        assert_eq!(projects[&id].sessions, vec![session_id]);
    }

    #[test]
    fn test_apply_project_update_removed_project_is_not_restored() {
        let mut projects = HashMap::new();
        let err = apply_project_update(&mut projects, &project(), &Ok(None)).unwrap_err();
        assert_eq!(err.to_string(), "Project no longer exists");
        assert!(projects.is_empty());
    }

    #[test]
    fn test_apply_project_removal_removes_project_and_its_sessions() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let in_project = session(Some(id));
        let orphan = session(None);
        let mut sessions =
            HashMap::from([(in_project.id, in_project), (orphan.id, orphan.clone())]);
        apply_project_removal(&mut projects, &mut sessions, id);
        assert!(projects.is_empty());
        assert_eq!(sessions, HashMap::from([(orphan.id, orphan)]));
    }

    #[test]
    fn test_apply_session_update_ok_replaces_session() {
        let s = session(None);
        let mut sessions = HashMap::from([(s.id, s.clone())]);
        let mut renamed = s.clone();
        renamed.name = String::from("renamed");
        apply_session_update(&mut sessions, &renamed, &Ok(())).unwrap();
        assert_eq!(sessions[&s.id].name, "renamed");
    }

    #[test]
    fn test_apply_session_update_err_changes_nothing() {
        let s = session(None);
        let mut sessions = HashMap::from([(s.id, s.clone())]);
        let mut renamed = s.clone();
        renamed.name = String::from("renamed");
        let err = apply_session_update(&mut sessions, &renamed, &Err("boom".into())).unwrap_err();
        assert_eq!(err.to_string(), "boom");
        assert_eq!(sessions[&s.id], s);
    }

    #[test]
    fn test_apply_session_update_removed_session_is_not_restored() {
        let mut sessions = HashMap::new();
        let err = apply_session_update(&mut sessions, &session(None), &Ok(())).unwrap_err();
        assert_eq!(err.to_string(), "Session no longer exists");
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_apply_session_removal_ok_removes_from_state_and_project() {
        let mut p = project();
        let id = p.id;
        let s = session(Some(id));
        p.sessions = vec![s.id];
        let mut projects = HashMap::from([(id, p)]);
        let mut sessions = HashMap::from([(s.id, s.clone())]);
        apply_session_removal(&mut projects, &mut sessions, &s, &Ok(())).unwrap();
        assert!(sessions.is_empty());
        assert!(projects[&id].sessions.is_empty());
    }

    #[test]
    fn test_apply_session_removal_err_changes_nothing() {
        let s = session(None);
        let mut projects = HashMap::new();
        let mut sessions = HashMap::from([(s.id, s.clone())]);
        let err = apply_session_removal(&mut projects, &mut sessions, &s, &Err("boom".into()))
            .unwrap_err();
        assert_eq!(err.to_string(), "boom");
        assert_eq!(sessions[&s.id], s);
    }

    #[test]
    fn test_apply_tmux_sync_prunes_dead_known_sessions_only() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let dead = session(Some(id));
        let created_during_sync = session(Some(id));
        let mut sessions = HashMap::from([
            (dead.id, dead.clone()),
            (created_during_sync.id, created_during_sync.clone()),
        ]);
        let known = HashSet::from([dead.id]);
        apply_tmux_sync(
            &mut projects,
            &mut sessions,
            &known,
            &Ok((vec![], HashMap::new())),
        )
        .unwrap();
        assert!(!sessions.contains_key(&dead.id));
        assert!(sessions.contains_key(&created_during_sync.id));
    }

    #[test]
    fn test_apply_tmux_sync_captures_unmapped_sessions_and_refreshes_worktrees() {
        let p = project();
        let id = p.id;
        let mut projects = HashMap::from([(id, p)]);
        let live = session(Some(id));
        let mut sessions = HashMap::from([(live.id, live.clone())]);
        let known = HashSet::from([live.id]);
        apply_tmux_sync(
            &mut projects,
            &mut sessions,
            &known,
            &Ok((
                vec![String::from("project_session"), String::from("stray")],
                HashMap::from([(id, vec![worktree("new")])]),
            )),
        )
        .unwrap();
        assert!(sessions.contains_key(&live.id));
        let names: HashSet<&str> = sessions.values().map(|s| s.name.as_str()).collect();
        assert_eq!(names, HashSet::from(["session", "stray"]));
        assert_eq!(projects[&id].worktrees, vec![worktree("new")]);
    }

    #[test]
    fn test_apply_tmux_sync_err_changes_nothing() {
        let mut projects = HashMap::from([(Uuid::new_v4(), project())]);
        let s = session(None);
        let mut sessions = HashMap::from([(s.id, s)]);
        let (projects_before, sessions_before) = (projects.clone(), sessions.clone());
        let err = apply_tmux_sync(
            &mut projects,
            &mut sessions,
            &HashSet::new(),
            &Err("boom".into()),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "boom");
        assert_eq!(projects, projects_before);
        assert_eq!(sessions, sessions_before);
    }
}
