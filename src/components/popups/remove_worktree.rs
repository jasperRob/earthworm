use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::{
        form::{Form, FormEvent},
        popups::{Popup, PopupOutcome},
    },
    project::Project,
    worktree::Worktree,
};

pub struct RemoveWorktreePopup {
    form: Form,
    worktree: Worktree,
    project: Project,
}

impl RemoveWorktreePopup {
    pub fn new(worktree: Worktree, project: Project) -> Self {
        Self {
            form: Form::confirmation()
                .title("Remove Worktree")
                .body(format!("Remove \"{}\"?", worktree.name)),
            worktree,
            project,
        }
    }
}

impl Popup for RemoveWorktreePopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match self.form.handle_key(key) {
            FormEvent::Submit => {
                PopupOutcome::Submitted(Action::RemoveWorktree(
                    // TODO: is there a better way of passing the originals here?
                    self.project.clone(),
                    self.worktree.clone(),
                ))
            }
            FormEvent::Cancel => PopupOutcome::Cancelled,
            FormEvent::Continue => PopupOutcome::Pending,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.form.draw(frame, area);
    }
}
