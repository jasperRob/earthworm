use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent, FormInput, InputValidation, TextRule},
        popups::{Popup, PopupOutcome},
    },
    project::Project,
    session::Session,
    worktree::Worktree,
};

enum Field {
    Project,
    Name,
    UseCustomWorktreeName,
    WorktreeName,
    UseCustomPath,
    Path,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Name => "Name",
            Self::UseCustomWorktreeName => "Use Custom Worktree Name?",
            Self::WorktreeName => "Worktree Name",
            Self::UseCustomPath => "Use Custom Path?",
            Self::Path => "Path",
        }
    }
}

pub struct NewSessionPopup {
    form: Form,
    all_projects: Vec<Project>,
}

impl NewSessionPopup {
    pub fn new(
        name: Option<String>,
        worktree_name: Option<String>,
        path: Option<String>,
        init_project: Option<Project>,
        all_projects: Vec<Project>,
    ) -> Self {
        Self {
            form: Form::standard().title("New Session").inputs(vec![
                FormInput {
                    label: Field::Project.label().to_string(),
                    initial_value: init_project.clone().map(|p| p.name).unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::OneOf(
                        all_projects.iter().map(|p| p.name.clone()).collect(),
                    )])),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::Name.label().to_string(),
                    initial_value: name.unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::UseCustomWorktreeName.label().to_string(),
                    initial_value: false.to_string(),
                    validation: Some(InputValidation::Boolean),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::WorktreeName.label().to_string(),
                    initial_value: worktree_name.unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: Some((Field::UseCustomWorktreeName as usize, true)),
                },
                FormInput {
                    label: Field::UseCustomPath.label().to_string(),
                    initial_value: false.to_string(),
                    validation: Some(InputValidation::Boolean),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::Path.label().to_string(),
                    initial_value: path.unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: Some((Field::UseCustomPath as usize, true)),
                },
            ]),
            all_projects,
        }
    }
}

impl Popup for NewSessionPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let project_name: String = self.form.value(Field::Project as usize);
                let project: Option<&Project> =
                    self.all_projects.iter().find(|p| p.name == project_name);
                let session_name: String = self.form.value(Field::Name as usize);
                // TODO: Implement dynamic types
                let use_custom_worktree_name: String =
                    self.form.value(Field::UseCustomWorktreeName as usize);
                let mut worktree_name: String = session_name.clone();
                if use_custom_worktree_name == "true" {
                    worktree_name = self.form.value(Field::WorktreeName as usize);
                }
                let use_custom_path: String = self.form.value(Field::UseCustomPath as usize);
                let mut path: String = String::default();
                if use_custom_path == "true" {
                    path = self.form.value(Field::Path as usize);
                } else if let Some(p) = project {
                    path = format!("{}-{}", p.path, session_name);
                }
                let mut project_id: Option<Uuid> = None;
                if let Some(p) = project {
                    project_id = Some(p.id);
                }
                let session = Session {
                    id: Uuid::new_v4(),
                    project_id,
                    name: self.form.value(Field::Name as usize),
                    path: Some(path.clone()).filter(|s| !s.is_empty()),
                    worktree: (!worktree_name.is_empty()).then(|| Worktree {
                        name: worktree_name.clone(),
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
