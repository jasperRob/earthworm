use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent},
        popups::{Popup, PopupOutcome},
    },
    project::Project,
};

pub struct RemoveProjectPopup {
    form: Form,
    project: Project,
}

impl RemoveProjectPopup {
    pub fn new(project: Project) -> Self {
        Self {
            form: Form::confirmation()
                .title("Remove Project")
                .body(format!("Remove \"{}\"?", project.name)),
            project,
        }
    }
}

impl Popup for RemoveProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                PopupOutcome::Submitted(Action::RemoveProject(self.project.clone()))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
