use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form_popup::{FormEvent, FormPopup},
        popups::{Popup, PopupOutcome, open_input_popup},
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
    form: FormPopup,
    project: Project,
}

impl EditProjectPopup {
    pub fn new(project: Project) -> Self {
        Self {
            form: FormPopup::new(&[
                (Field::Name.label(), String::default()),
                (Field::Path.label(), String::default()),
            ]),
            project: project,
        }
    }
}

impl Popup for EditProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name: String = self.form.value(Field::Name as usize).into();
                let path: String = self.form.value(Field::Path as usize).into();
                let mut project = self.project.clone();
                project.name = name;
                project.path = path;
                PopupOutcome::Submitted(Action::UpdateProject(project))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        open_input_popup(frame, area, "Edit Project", &self.form);
    }
}
