use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::popups::{Popup, PopupOutcome, open_confirmation_popup},
    session::Session,
};

pub struct RemoveSessionPopup {
    session: Session,
}

impl RemoveSessionPopup {
    pub fn new(session: Session) -> Self {
        Self { session: session }
    }
}

impl Popup for RemoveSessionPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match key.code {
            KeyCode::Char('y') => {
                PopupOutcome::Submitted(Action::RemoveSession(self.session.clone()))
            }
            KeyCode::Char('n') => PopupOutcome::Cancelled,
            _ => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let body = format!("Kill \"{}\"?", self.session.name);
        open_confirmation_popup(frame, area, "Kill Session", &body);
    }
}
