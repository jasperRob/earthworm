use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormInput},
        popups::FormPopup,
    },
    project::Project,
    session::Session,
    worktree::Worktree,
};

enum Field {
    Name = 0,
    Path = 1,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Path => "Path",
        }
    }
}

/// Fields of the forms for a session in a project, which lead with the readonly project name
enum ProjectSessionField {
    Project = 0,
    Name = 1,
}

impl ProjectSessionField {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Name => "Name",
        }
    }
}

pub fn new_session() -> FormPopup {
    FormPopup::new(
        Form::standard().title("New Session").inputs(vec![
            FormInput::new().label(Field::Name.label()).required(),
            FormInput::new()
                .label(Field::Path.label())
                .path()
                .required(),
        ]),
        |form| {
            Action::SubmitSession(Session {
                id: Uuid::new_v4(),
                project_id: None,
                name: form.value(Field::Name as usize),
                path: Some(form.value(Field::Path as usize)),
                worktree: None,
            })
        },
    )
}

pub fn new_session_in_project(project: &Project) -> FormPopup {
    let project = project.clone();
    FormPopup::new(
        Form::standard()
            .title("New Session in Project")
            .inputs(vec![
                FormInput::new()
                    .label(ProjectSessionField::Project.label())
                    .initial_value(project.name.clone())
                    .readonly(),
                FormInput::new()
                    .label(ProjectSessionField::Name.label())
                    .required(),
            ]),
        move |form| {
            let name = form.value(ProjectSessionField::Name as usize);
            // Worktrees sit beside the project dir; `/` in the name would nest dirs
            let path = format!(
                "{}-{}",
                project.path.trim_end_matches('/'),
                name.replace('/', "-")
            );
            Action::SubmitSession(Session {
                id: Uuid::new_v4(),
                project_id: Some(project.id),
                name: name.clone(),
                path: Some(path.clone()),
                worktree: Some(Worktree { name, path }),
            })
        },
    )
}

pub fn new_session_from_worktree(project: &Project, worktree: &Worktree) -> FormPopup {
    let project_id = project.id;
    let worktree = worktree.clone();
    FormPopup::new(
        Form::standard()
            .title("New Session from Worktree")
            .inputs(vec![
                FormInput::new()
                    .label(ProjectSessionField::Project.label())
                    .initial_value(project.name.clone())
                    .readonly(),
                FormInput::new()
                    .label(ProjectSessionField::Name.label())
                    .initial_value(worktree.name.clone())
                    .required(),
            ]),
        move |form| {
            Action::SubmitSession(Session {
                id: Uuid::new_v4(),
                project_id: Some(project_id),
                name: form.value(ProjectSessionField::Name as usize),
                path: Some(worktree.path.clone()),
                worktree: Some(worktree.clone()),
            })
        },
    )
}

pub fn edit_session(session: Session) -> FormPopup {
    FormPopup::new(
        Form::standard().title("Edit Session").inputs(vec![
            FormInput::new()
                .label(Field::Name.label())
                .initial_value(session.name.clone())
                .required(),
        ]),
        move |form| {
            let mut session = session.clone();
            session.name = form.value(Field::Name as usize);
            Action::UpdateSession(session)
        },
    )
}

pub fn remove_session(session: Session) -> FormPopup {
    FormPopup::new(
        Form::confirmation()
            .title("Kill Session")
            .body(format!("Kill \"{}\"?", session.name)),
        move |_| Action::RemoveSession(session.clone()),
    )
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::components::popups::{Popup, PopupOutcome};

    fn submit_name(project_path: &str, name: &str) -> Session {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::from("repo"),
            path: String::from(project_path),
            sessions: vec![],
            worktrees: vec![],
        };
        let mut popup = new_session_in_project(&project);
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
        let mut popup = new_session_in_project(&project);
        let outcome = popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(matches!(outcome, PopupOutcome::Pending));
    }
}
