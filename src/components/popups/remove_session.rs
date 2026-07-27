use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent},
        popups::{Popup, PopupOutcome},
    },
    session::Session,
};

pub struct RemoveSessionPopup {
    form: Form,
    session: Session,
}

impl RemoveSessionPopup {
    pub fn new(session: Session) -> Self {
        Self {
            form: Form::confirmation()
                .title("Kill Session")
                .body(format!("Kill \"{}\"?", session.name)),
            session,
        }
    }
}

impl Popup for RemoveSessionPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                PopupOutcome::Submitted(Action::RemoveSession(self.session.clone()))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
