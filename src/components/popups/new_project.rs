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
    form: Form,
}

impl NewProjectPopup {
    pub fn new() -> Self {
        Self {
            form: Form::standard().title("New Project").inputs(vec![
                FormInput {
                    label: Field::Name.label().to_string(),
                    initial_value: String::default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::Path.label().to_string(),
                    initial_value: String::default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
            ]),
        }
    }
}

impl Popup for NewProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name: String = self.form.value(Field::Name as usize);
                let path: String = self.form.value(Field::Path as usize);
                let project = Project {
                    id: Uuid::new_v4(),
                    name,
                    path,
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

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
