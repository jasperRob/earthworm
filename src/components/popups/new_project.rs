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

pub struct NewProjectPopup {
    form: FormPopup,
}

impl NewProjectPopup {
    pub fn new() -> Self {
        Self {
            form: FormPopup::new(vec![
                FormInput {
                    label: Field::Name.label(),
                    initial_value: String::default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                },
                FormInput {
                    label: Field::Path.label(),
                    initial_value: String::default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                },
            ]),
        }
    }
}

impl Popup for NewProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name: String = self.form.value(Field::Name as usize).into();
                let path: String = self.form.value(Field::Path as usize).into();
                let project = Project {
                    id: Uuid::new_v4(),
                    name: name,
                    path: path,
                    sessions: vec![],
                    // TODO: should init based on existing worktrees for git repo
                    worktrees: vec![],
                };
                PopupOutcome::Submitted(Action::SubmitProject(project))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        open_input_popup(frame, area, "New Project", &self.form);
    }
}
