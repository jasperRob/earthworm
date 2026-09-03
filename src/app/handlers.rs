use color_eyre::eyre::{Result, eyre};
use uuid::Uuid;

use super::App;

use crate::{
    git::{create_worktree, fetch_worktrees, remove_worktree},
    project::Project,
    session::Session,
    tmux::{
        attach_tmux_session, get_tmux_session_name, get_tmux_session_path, kill_tmux_session,
        new_tmux_session, rename_tmux_session,
    },
    worktree::Worktree,
};

impl App {
    pub(super) fn on_submit_project(&mut self, project: &Project) -> Result<()> {
        let mut project = project.clone();
        if !project.path.is_empty() {
            project.worktrees = fetch_worktrees(&project.path).unwrap_or_default();
        }
        self.projects.insert(project.id, project);
        self.resync()
    }

    pub(super) fn on_update_project(&mut self, project: &Project) -> Result<()> {
        let Some(old_project) = self.projects.get(&project.id) else {
            return Err(eyre!("Could not find existing project"));
        };
        // Update the session name of any project sessions
        if project.name != old_project.name {
            let renames: Vec<(String, String)> = self
                .sessions
                .values()
                .filter(|s| s.project_id == Some(project.id))
                .map(|s| {
                    (
                        get_tmux_session_name(s, Some(old_project)),
                        get_tmux_session_name(s, Some(project)),
                    )
                })
                .collect();

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
        }
        // TODO: For now, we will just replace in state. In future, maybe think
        // about how this can be done with specific field updates. Could be safer.
        self.projects.insert(project.id, project.clone());
        self.resync()
    }

    pub(super) fn on_remove_project(&mut self, project: &Project) -> Result<()> {
        let session_ids: Vec<Uuid> = self
            .sessions
            .values()
            .filter(|s| s.project_id == Some(project.id))
            .map(|s| s.id)
            .collect();
        for id in session_ids {
            if let Some(session) = self.sessions.get(&id) {
                let tmux_session_name = get_tmux_session_name(session, Some(project));
                kill_tmux_session(tmux_session_name)?;
            }
            self.sessions.remove(&id);
        }
        self.projects.remove(&project.id);
        self.resync()
    }

    pub(super) fn on_submit_session(&mut self, session: &Session) -> Result<()> {
        let project = self.get_session_project(session)?;

        if let Some(p) = project
            && let Some(path) = session.path.clone()
            && let Some(worktree) = session.worktree.clone()
            && !p.worktrees.iter().any(|wt| wt.name == worktree.name)
            && let Err(e) = create_worktree(&p.path, &worktree.name, &path)
        {
            return Err(e);
        }

        let tmux_session_name = get_tmux_session_name(session, project);
        let tmux_session_path = get_tmux_session_path(session, project)?;

        // Failure to create the tmux session leaves the worktree intact
        new_tmux_session(tmux_session_name, tmux_session_path)?;

        self.sessions.insert(session.id, session.clone());
        if let Some(project_id) = session.project_id
            && let Some(project) = self.projects.get_mut(&project_id)
        {
            project.sessions.push(session.id);
        }
        self.resync()
    }

    pub(super) fn on_update_session(&mut self, session: &Session) -> Result<()> {
        // we need to update the name and path of the tmux session if changed
        let Some(old_session) = self.sessions.get(&session.id) else {
            return Err(eyre!("Could not find existing session"));
        };
        let mut old_session_project: Option<&Project> = None;
        if let Some(old_session_project_id) = old_session.project_id {
            old_session_project = self.projects.get(&old_session_project_id)
        }
        // update tmux session name
        if old_session.name != session.name {
            let from = get_tmux_session_name(old_session, old_session_project);
            let to = get_tmux_session_name(session, old_session_project);
            rename_tmux_session(from, to)?;
            self.sessions.insert(session.id, session.clone());
        }
        self.resync()
    }

    pub(super) fn on_attach_session(&mut self, session: &Session) -> Result<()> {
        let project = self.get_session_project(session)?;
        let tmux_session_name = get_tmux_session_name(session, project);
        if let Err(e) = attach_tmux_session(tmux_session_name) {
            return Err(eyre!(e));
        }
        self.resync()
    }

    pub(super) fn on_remove_session(&mut self, session: &Session) -> Result<()> {
        let project = self.get_session_project(session)?;
        let tmux_session_name = get_tmux_session_name(session, project);
        kill_tmux_session(tmux_session_name)?;
        self.sessions.remove(&session.id);
        if let Some(project_id) = session.project_id
            && let Some(project) = self.projects.get_mut(&project_id)
        {
            project.sessions.retain(|id| *id != session.id);
        }
        self.resync()
    }

    pub(super) fn on_remove_worktree(
        &mut self,
        project: &Project,
        worktree: &Worktree,
    ) -> Result<()> {
        if let Err(e) = remove_worktree(&project.path, &worktree.path) {
            return Err(eyre!(e));
        }
        self.resync()
    }
}
