pub mod help;
pub mod project;
pub mod session;
pub mod worktree;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::form::{Form, FormEvent},
};

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

/// A popup around a form, which submits the action built from the form's values
pub struct FormPopup {
    form: Form,
    on_submit: Box<dyn Fn(&Form) -> Action>,
}

impl FormPopup {
    pub fn new(form: Form, on_submit: impl Fn(&Form) -> Action + 'static) -> Self {
        Self {
            form,
            on_submit: Box::new(on_submit),
        }
    }
}

impl Popup for FormPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => PopupOutcome::Submitted((self.on_submit)(&self.form)),
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{
        project::{edit_project, new_project, remove_project},
        session::{
            edit_session, new_session, new_session_from_worktree, new_session_in_project,
            remove_session,
        },
        worktree::remove_worktree,
        *,
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
        let mut popup = new_project();
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
        assert_cancels(new_project(), KeyCode::Esc);
    }

    #[test]
    fn test_edit_project_submits_updated_project() {
        let (_dir, path) = temp_dir();
        let project = project(&path);
        let mut popup = edit_project(project.clone());
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
        assert_cancels(edit_project(project("/src/repo")), KeyCode::Esc);
    }

    #[tokio::test]
    async fn test_new_session_submits_session() {
        let (_dir, path) = temp_dir();
        let mut popup = new_session();
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
        assert_cancels(new_session(), KeyCode::Esc);
    }

    #[test]
    fn test_new_session_in_project_submits_worktree_session() {
        let project = project("/src/repo");
        let mut popup = new_session_in_project(&project);
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
        assert_cancels(new_session_in_project(&project("/src/repo")), KeyCode::Esc);
    }

    #[test]
    fn test_new_session_from_worktree_submits_session_on_worktree() {
        let project = project("/src/repo");
        let worktree = worktree();
        let mut popup = new_session_from_worktree(&project, &worktree);
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
            new_session_from_worktree(&project("/src/repo"), &worktree()),
            KeyCode::Esc,
        );
    }

    #[test]
    fn test_edit_session_submits_updated_session() {
        let session = session();
        let mut popup = edit_session(session.clone());
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
        assert_cancels(edit_session(session()), KeyCode::Esc);
    }

    #[test]
    fn test_remove_project_submits_project() {
        let project = project("/src/repo");
        let mut popup = remove_project(project.clone());
        assert_eq!(
            submit(&mut popup, KeyCode::Char('y')),
            Action::RemoveProject(project)
        );
    }

    #[test]
    fn test_remove_project_cancels() {
        assert_cancels(remove_project(project("/src/repo")), KeyCode::Esc);
        assert_cancels(remove_project(project("/src/repo")), KeyCode::Char('n'));
    }

    #[test]
    fn test_remove_session_submits_session() {
        let session = session();
        let mut popup = remove_session(session.clone());
        assert_eq!(
            submit(&mut popup, KeyCode::Char('y')),
            Action::RemoveSession(session)
        );
    }

    #[test]
    fn test_remove_session_cancels() {
        assert_cancels(remove_session(session()), KeyCode::Esc);
        assert_cancels(remove_session(session()), KeyCode::Char('n'));
    }

    #[test]
    fn test_remove_worktree_submits_project_and_worktree() {
        let project = project("/src/repo");
        let worktree = worktree();
        let mut popup = remove_worktree(worktree.clone(), project.clone());
        assert_eq!(
            submit(&mut popup, KeyCode::Char('y')),
            Action::RemoveWorktree(project, worktree)
        );
    }

    #[test]
    fn test_remove_worktree_cancels() {
        assert_cancels(
            remove_worktree(worktree(), project("/src/repo")),
            KeyCode::Esc,
        );
        assert_cancels(
            remove_worktree(worktree(), project("/src/repo")),
            KeyCode::Char('n'),
        );
    }
}
