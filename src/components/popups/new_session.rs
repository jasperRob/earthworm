use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form_popup::{FormEvent, FormPopup},
        popups::{Popup, PopupOutcome, open_input_popup},
    },
    session::Session,
    worktree::Worktree,
};

enum Field {
    Name = 0,
    WorktreeName = 1,
    Path = 2,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::WorktreeName => "Worktree Name",
            Self::Path => "Path",
        }
    }
}

pub struct NewSessionPopup {
    form: FormPopup,
    project_id: Option<Uuid>,
}

impl NewSessionPopup {
    pub fn new(
        project_id: Option<Uuid>,
        name: Option<String>,
        worktree_name: Option<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            form: FormPopup::new(&[
                (Field::Name.label(), name.unwrap_or_default()),
                (
                    Field::WorktreeName.label(),
                    worktree_name.unwrap_or_default(),
                ),
                (Field::Path.label(), path.unwrap_or_default()),
            ]),
            project_id: project_id,
        }
    }
}

impl Popup for NewSessionPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                let path: String = self.form.value(Field::Path as usize).into();
                let worktree_name: String = self.form.value(Field::WorktreeName as usize).into();
                let session = Session {
                    id: Uuid::new_v4(),
                    project_id: self.project_id,
                    name: self.form.value(Field::Name as usize).into(),
                    path: Some(path.clone()).filter(|s| !s.is_empty()),
                    worktree: (!worktree_name.is_empty()).then(|| Worktree {
                        name: worktree_name.clone(),
                        path: path,
                    }),
                };
                PopupOutcome::Submitted(Action::SubmitSession(session))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        open_input_popup(frame, area, "New Session", &self.form);
    }
}
