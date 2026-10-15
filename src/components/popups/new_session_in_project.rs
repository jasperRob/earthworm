use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent, FormInput},
        popups::{Popup, PopupOutcome},
    },
    project::Project,
    session::Session,
    worktree::Worktree,
};

enum Field {
    Project,
    Name,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Name => "Name",
        }
    }
}

pub struct NewSessionInProjectPopup {
    form: Form,
    project: Project,
}

impl NewSessionInProjectPopup {
    pub fn new(project: &Project) -> Self {
        Self {
            form: Form::standard()
                .title("New Session in Project")
                .inputs(vec![
                    FormInput::new()
                        .label(Field::Project.label().to_string())
                        .initial_value(project.name.clone())
                        .readonly(),
                    FormInput::new()
                        .label(Field::Name.label().to_string())
                        .required(),
                ]),
            project: project.clone(),
        }
    }
}

impl Popup for NewSessionInProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name = self.form.value(Field::Name as usize);
                // Worktrees sit beside the project dir; `/` in the name would nest dirs
                let path = format!(
                    "{}-{}",
                    self.project.path.trim_end_matches('/'),
                    name.replace('/', "-")
                );
                let session = Session {
                    id: Uuid::new_v4(),
                    project_id: Some(self.project.id),
                    name: name.clone(),
                    path: Some(path.clone()),
                    worktree: Some(Worktree {
                        name: name.clone(),
                        path,
                    }),
                };
                PopupOutcome::Submitted(Action::SubmitSession(session))
            }
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

    use super::*;

    fn submit_name(project_path: &str, name: &str) -> Session {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::from("repo"),
            path: String::from(project_path),
            sessions: vec![],
            worktrees: vec![],
        };
        let mut popup = NewSessionInProjectPopup::new(&project);
        for c in name.chars() {
            popup.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
        }
        match popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())) {
            PopupOutcome::Submitted(Action::SubmitSession(session)) => {
                assert_eq!(session.project_id, Some(project.id));
                session
            }
            _ => panic!("expected SubmitSession"),
        }
    }

    #[test]
    fn test_submit_builds_worktree_beside_project() {
        let session = submit_name("/src/repo", "fix");
        assert_eq!(session.name, "fix");
        assert_eq!(session.path.as_deref(), Some("/src/repo-fix"));
        assert_eq!(
            session.worktree,
            Some(Worktree {
                name: String::from("fix"),
                path: String::from("/src/repo-fix"),
            })
        );
    }

    #[test]
    fn test_submit_trims_trailing_slash_from_project_path() {
        let session = submit_name("/src/repo/", "fix");
        assert_eq!(session.path.as_deref(), Some("/src/repo-fix"));
    }

    #[test]
    fn test_submit_flattens_slashes_in_worktree_dir() {
        let session = submit_name("/src/repo", "feat/x");
        assert_eq!(session.name, "feat/x");
        assert_eq!(session.path.as_deref(), Some("/src/repo-feat-x"));
        assert_eq!(
            session.worktree,
            Some(Worktree {
                name: String::from("feat/x"),
                path: String::from("/src/repo-feat-x"),
            })
        );
    }

    #[test]
    fn test_submit_with_empty_name_is_pending() {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::from("repo"),
            path: String::from("/src/repo"),
            sessions: vec![],
            worktrees: vec![],
        };
        let mut popup = NewSessionInProjectPopup::new(&project);
        let outcome = popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(matches!(outcome, PopupOutcome::Pending));
    }
}
