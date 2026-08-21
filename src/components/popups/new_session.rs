use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent, FormInput},
        popups::{Popup, PopupOutcome},
    },
    session::Session,
};

enum Field {
    Name,
    Path,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Path => "Path",
        }
    }
}

pub struct NewSessionPopup {
    form: Form,
}

impl NewSessionPopup {
    pub fn new() -> Self {
        Self {
            form: Form::standard().title("New Session").inputs(vec![
                FormInput::new()
                    .label(Field::Name.label().to_string())
                    .required(),
                FormInput::new()
                    .label(Field::Path.label().to_string())
                    .required(),
            ]),
        }
    }
}

impl Popup for NewSessionPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => PopupOutcome::Submitted(Action::SubmitSession(Session {
                id: Uuid::new_v4(),
                project_id: None,
                name: self.form.value(Field::Name as usize),
                path: Some(self.form.value(Field::Path as usize)),
                worktree: None,
            })),
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
