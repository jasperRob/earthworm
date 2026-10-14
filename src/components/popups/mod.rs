pub mod edit_project;
pub mod edit_session;
pub mod help;
pub mod new_project;
pub mod new_session;
pub mod new_session_from_worktree;
pub mod new_session_in_project;
pub mod remove_project;
pub mod remove_session;
pub mod remove_worktree;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::action::Action;

#[derive(Default)]
pub enum PopupState {
    #[default]
    Closed,
    Open(Box<dyn Popup>),
}

pub enum PopupOutcome {
    Pending,
    Submitted(Action),
    Cancelled,
}

pub trait Popup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome;
    fn draw(&mut self, frame: &mut Frame, area: Rect);
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{
        edit_project::EditProjectPopup, edit_session::EditSessionPopup,
        new_project::NewProjectPopup, new_session::NewSessionPopup,
        new_session_from_worktree::NewSessionFromWorktreePopup,
        new_session_in_project::NewSessionInProjectPopup, remove_project::RemoveProjectPopup,
        remove_session::RemoveSessionPopup, remove_worktree::RemoveWorktreePopup, *,
    };
    use crate::{project::Project, session::Session, worktree::Worktree};

    fn press(popup: &mut dyn Popup, code: KeyCode) -> PopupOutcome {
        popup.handle_key(KeyEvent::new(code, KeyModifiers::empty()))
    }

    fn type_text(popup: &mut dyn Popup, text: &str) {
        for c in text.chars() {
            press(popup, KeyCode::Char(c));
        }
    }

    fn submit(popup: &mut dyn Popup, code: KeyCode) -> Action {
        match press(popup, code) {
            PopupOutcome::Submitted(action) => action,
            _ => panic!("expected the popup to submit"),
        }
    }

    fn assert_cancels(mut popup: impl Popup, code: KeyCode) {
        assert!(matches!(press(&mut popup, code), PopupOutcome::Cancelled));
    }

    fn temp_dir() -> (TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        (dir, path)
    }

    fn project(path: &str) -> Project {
        Project {
            id: Uuid::new_v4(),
            name: String::from("repo"),
            path: String::from(path),
            sessions: vec![],
            worktrees: vec![],
        }
    }

    fn session() -> Session {
        Session {
            id: Uuid::new_v4(),
            project_id: None,
            name: String::from("old"),
            path: Some(String::from("/src/old")),
            worktree: None,
        }
    }

    fn worktree() -> Worktree {
        Worktree {
            name: String::from("wt"),
            path: String::from("/src/repo-wt"),
        }
    }

    // Typing into a path input computes suggestions on the blocking pool, so needs a runtime
    #[tokio::test]
    async fn test_new_project_submits_project() {
        let (_dir, path) = temp_dir();
        let mut popup = NewProjectPopup::new();
        type_text(&mut popup, "repo");
        press(&mut popup, KeyCode::Tab);
        type_text(&mut popup, &path);
        let Action::SubmitProject(project) = submit(&mut popup, KeyCode::Enter) else {
            panic!("expected SubmitProject");
        };
        assert_eq!(
            project,
            Project {
                id: project.id,
                name: String::from("repo"),
                path,
                sessions: vec![],
                worktrees: vec![],
            }
        );
    }

    #[test]
    fn test_new_project_cancels() {
        assert_cancels(NewProjectPopup::new(), KeyCode::Esc);
    }

    #[test]
    fn test_edit_project_submits_updated_project() {
        let (_dir, path) = temp_dir();
        let project = project(&path);
        let mut popup = EditProjectPopup::new(project.clone());
        type_text(&mut popup, "2");
        assert_eq!(
            submit(&mut popup, KeyCode::Enter),
            Action::UpdateProject(Project {
                name: String::from("repo2"),
                ..project
            })
        );
    }

    #[test]
    fn test_edit_project_cancels() {
        assert_cancels(EditProjectPopup::new(project("/src/repo")), KeyCode::Esc);
    }

    #[tokio::test]
    async fn test_new_session_submits_session() {
        let (_dir, path) = temp_dir();
        let mut popup = NewSessionPopup::new();
        type_text(&mut popup, "fix");
        press(&mut popup, KeyCode::Tab);
        type_text(&mut popup, &path);
        let Action::SubmitSession(session) = submit(&mut popup, KeyCode::Enter) else {
            panic!("expected SubmitSession");
        };
        assert_eq!(
            session,
            Session {
                id: session.id,
                project_id: None,
                name: String::from("fix"),
                path: Some(path),
                worktree: None,
            }
        );
    }

    #[test]
    fn test_new_session_cancels() {
        assert_cancels(NewSessionPopup::new(), KeyCode::Esc);
    }

    #[test]
    fn test_new_session_in_project_submits_worktree_session() {
        let project = project("/src/repo");
        let mut popup = NewSessionInProjectPopup::new(&project);
        type_text(&mut popup, "fix");
        let Action::SubmitSession(session) = submit(&mut popup, KeyCode::Enter) else {
            panic!("expected SubmitSession");
        };
        assert_eq!(
            session,
            Session {
                id: session.id,
                project_id: Some(project.id),
                name: String::from("fix"),
                path: Some(String::from("/src/repo-fix")),
                worktree: Some(Worktree {
                    name: String::from("fix"),
                    path: String::from("/src/repo-fix"),
                }),
            }
        );
    }

    #[test]
    fn test_new_session_in_project_cancels() {
        assert_cancels(
            NewSessionInProjectPopup::new(&project("/src/repo")),
            KeyCode::Esc,
        );
    }

    #[test]
    fn test_new_session_from_worktree_submits_session_on_worktree() {
        let project = project("/src/repo");
        let worktree = worktree();
        let mut popup = NewSessionFromWorktreePopup::new(&project, &worktree);
        type_text(&mut popup, "2");
        let Action::SubmitSession(session) = submit(&mut popup, KeyCode::Enter) else {
            panic!("expected SubmitSession");
        };
        assert_eq!(
            session,
            Session {
                id: session.id,
                project_id: Some(project.id),
                name: String::from("wt2"),
                path: Some(worktree.path.clone()),
                worktree: Some(worktree),
            }
        );
    }

    #[test]
    fn test_new_session_from_worktree_cancels() {
        assert_cancels(
            NewSessionFromWorktreePopup::new(&project("/src/repo"), &worktree()),
            KeyCode::Esc,
        );
    }

    #[test]
    fn test_edit_session_submits_updated_session() {
        let session = session();
        let mut popup = EditSessionPopup::new(session.clone());
        type_text(&mut popup, "2");
        assert_eq!(
            submit(&mut popup, KeyCode::Enter),
            Action::UpdateSession(Session {
                name: String::from("old2"),
                ..session
            })
        );
    }

    #[test]
    fn test_edit_session_cancels() {
        assert_cancels(EditSessionPopup::new(session()), KeyCode::Esc);
    }

    #[test]
    fn test_remove_project_submits_project() {
        let project = project("/src/repo");
        let mut popup = RemoveProjectPopup::new(project.clone());
        assert_eq!(
            submit(&mut popup, KeyCode::Char('y')),
            Action::RemoveProject(project)
        );
    }

    #[test]
    fn test_remove_project_cancels() {
        assert_cancels(RemoveProjectPopup::new(project("/src/repo")), KeyCode::Esc);
        assert_cancels(
            RemoveProjectPopup::new(project("/src/repo")),
            KeyCode::Char('n'),
        );
    }

    #[test]
    fn test_remove_session_submits_session() {
        let session = session();
        let mut popup = RemoveSessionPopup::new(session.clone());
        assert_eq!(
            submit(&mut popup, KeyCode::Char('y')),
            Action::RemoveSession(session)
        );
    }

    #[test]
    fn test_remove_session_cancels() {
        assert_cancels(RemoveSessionPopup::new(session()), KeyCode::Esc);
        assert_cancels(RemoveSessionPopup::new(session()), KeyCode::Char('n'));
    }

    #[test]
    fn test_remove_worktree_submits_project_and_worktree() {
        let project = project("/src/repo");
        let worktree = worktree();
        let mut popup = RemoveWorktreePopup::new(worktree.clone(), project.clone());
        assert_eq!(
            submit(&mut popup, KeyCode::Char('y')),
            Action::RemoveWorktree(project, worktree)
        );
    }

    #[test]
    fn test_remove_worktree_cancels() {
        assert_cancels(
            RemoveWorktreePopup::new(worktree(), project("/src/repo")),
            KeyCode::Esc,
        );
        assert_cancels(
            RemoveWorktreePopup::new(worktree(), project("/src/repo")),
            KeyCode::Char('n'),
        );
    }
}
