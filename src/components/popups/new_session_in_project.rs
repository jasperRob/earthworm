use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent, FormInput},
        popups::{Popup, PopupOutcome},
    },
    project::Project,
    session::Session,
    worktree::Worktree,
};

enum Field {
    Project,
    Name,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Name => "Name",
        }
    }
}

pub struct NewSessionInProjectPopup {
    form: Form,
    project: Project,
}

impl NewSessionInProjectPopup {
    pub fn new(project: &Project) -> Self {
        Self {
            form: Form::standard()
                .title("New Session in Project")
                .inputs(vec![
                    FormInput::new()
                        .label(Field::Project.label().to_string())
                        .initial_value(project.name.clone())
                        .readonly(),
                    FormInput::new()
                        .label(Field::Name.label().to_string())
                        .required(),
                ]),
            project: project.clone(),
        }
    }
}

impl Popup for NewSessionInProjectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let name = self.form.value(Field::Name as usize);
                let path = format!("{}-{}", self.project.path, name);
                let session = Session {
                    id: Uuid::new_v4(),
                    project_id: Some(self.project.id),
                    name: name.clone(),
                    path: Some(path.clone()),
                    worktree: Some(Worktree {
                        name: name.clone(),
                        path: path,
                    }),
                };
                PopupOutcome::Submitted(Action::SubmitSession(session))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
