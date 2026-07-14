use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form_popup::{FormEvent, FormInput, FormPopup, InputValidation, TextRule},
        popups::{Popup, PopupOutcome, open_input_popup},
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
    form: FormPopup,
    session: Session,
}

impl EditSessionPopup {
    pub fn new(session: Session) -> Self {
        Self {
            form: FormPopup::new(vec![
                FormInput {
                    label: Field::Name.label(),
                    initial_value: session.name.clone(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
                },
                FormInput {
                    label: Field::Path.label(),
                    initial_value: session.path.clone().unwrap_or_default(),
                    validation: Some(InputValidation::Text(vec![TextRule::NonEmpty])),
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
                let name: String = self.form.value(Field::Name as usize).into();
                let path: String = self.form.value(Field::Path as usize).into();
                let mut session = self.session.clone();
                session.name = name;
                session.path = Some(path);
                PopupOutcome::Submitted(Action::UpdateSession(session))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        open_input_popup(frame, area, "Edit Session", &self.form);
    }
}
