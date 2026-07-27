use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

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

pub struct EditProjectPopup {
    form: Form,
    project: Project,
}

impl EditProjectPopup {
    pub fn new(project: Project) -> Self {
        Self {
            form: Form::standard().title("Edit Project").inputs(vec![
                FormInput {
                    label: Field::Name.label().to_string(),
                    initial_value: project.name.clone(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::Path.label().to_string(),
                    initial_value: project.path.clone(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
            ]),
            project,
        }
    }
}

impl Popup for EditProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name: String = self.form.value(Field::Name as usize);
                let path: String = self.form.value(Field::Path as usize);
                let mut project = self.project.clone();
                project.name = name;
                project.path = path;
                PopupOutcome::Submitted(Action::UpdateProject(project))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
