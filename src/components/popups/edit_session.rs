use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent, FormInput, InputValidation, TextRule},
        popups::{Popup, PopupOutcome},
    },
    session::Session,
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

pub struct EditSessionPopup {
    form: Form,
    session: Session,
}

impl EditSessionPopup {
    pub fn new(session: Session) -> Self {
        Self {
            form: Form::standard().title("Edit Session").inputs(vec![
                FormInput {
                    label: Field::Name.label().to_string(),
                    initial_value: session.name.clone(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
                FormInput {
                    label: Field::Path.label().to_string(),
                    initial_value: session.path.clone().unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                    dependant_on: None,
                },
            ]),
            session,
        }
    }
}

impl Popup for EditSessionPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name: String = self.form.value(Field::Name as usize);
                let path: String = self.form.value(Field::Path as usize);
                let mut session = self.session.clone();
                session.name = name;
                session.path = Some(path);
                PopupOutcome::Submitted(Action::UpdateSession(session))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
