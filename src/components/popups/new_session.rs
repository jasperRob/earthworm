use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form_popup::{FormEvent, FormInput, FormPopup, InputValidation, TextRule},
        popups::{Popup, PopupOutcome, open_input_popup},
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
    form: FormPopup,
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
            form: FormPopup::new(vec![
                FormInput {
                    label: Field::Project.label(),
                    value: init_project.clone().map(|p| p.name).unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::OneOf(
                        all_projects.iter().map(|p| p.name.clone()).collect(),
                    )])),
                    dependant_on: None,
                    hidden: false,
                },
                FormInput {
                    label: Field::Name.label(),
                    value: name.unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                    hidden: false,
                },
                FormInput {
                    label: Field::UseCustomWorktreeName.label(),
                    value: false.to_string(),
                    validation: Some(InputValidation::Boolean),
                    dependant_on: None,
                    hidden: false,
                },
                FormInput {
                    label: Field::WorktreeName.label(),
                    value: worktree_name.unwrap_or_default(),
                    validation: None,
                    dependant_on: Some(Field::UseCustomWorktreeName as i32),
                    hidden: true,
                },
                FormInput {
                    label: Field::UseCustomPath.label(),
                    value: false.to_string(),
                    validation: Some(InputValidation::Boolean),
                    dependant_on: None,
                    hidden: false,
                },
                FormInput {
                    label: Field::Path.label().to_string(),
                    value: path.unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: Some(Field::UseCustomPath as i32),
                    hidden: true,
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
                let project_name: String = self.form.value(Field::Project as usize).into();
                let project_id: Option<Uuid> = self
                    .all_projects
                    .iter()
                    .find(|p| p.name == project_name)
                    .map(|p| p.id);
                let path: String = self.form.value(Field::Path as usize).into();
                let worktree_name: String = self.form.value(Field::WorktreeName as usize).into();
                let session = Session {
                    id: Uuid::new_v4(),
                    project_id,
                    name: self.form.value(Field::Name as usize).into(),
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

    fn draw(&self, frame: &mut Frame, area: Rect) {
        open_input_popup(frame, area, "New Session", &self.form);
    }
}
