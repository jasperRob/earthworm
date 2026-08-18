use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent, FormInput},
        popups::{Popup, PopupOutcome},
    },
    session::Session,
};

enum Field {
    Name = 0,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
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
                FormInput::new()
                    .label(Field::Name.label())
                    .initial_value(session.name.clone())
                    .required(),
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
                let mut session = self.session.clone();
                session.name = name;
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
