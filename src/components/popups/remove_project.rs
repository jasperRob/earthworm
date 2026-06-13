use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::popups::{Popup, PopupOutcome, open_confirmation_popup},
    project::Project,
};

pub struct RemoveProjectPopup {
    project: Project,
}

impl RemoveProjectPopup {
    pub fn new(project: Project) -> Self {
        Self { project: project }
    }
}

impl Popup for RemoveProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match key.code {
            KeyCode::Char('y') => {
                PopupOutcome::Submitted(Action::RemoveProject(self.project.clone()))
            }
            KeyCode::Char('n') => PopupOutcome::Cancelled,
            _ => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let body = format!("Remove \"{}\"?", self.project.name);
        open_confirmation_popup(frame, area, "Remove Project", &body);
    }
}
