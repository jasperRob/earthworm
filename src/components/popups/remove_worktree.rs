use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::popups::{Popup, PopupOutcome, open_confirmation_popup},
    project::Project,
    worktree::Worktree,
};

pub struct RemoveWorktreePopup {
    worktree: Worktree,
    project: Project,
}

impl RemoveWorktreePopup {
    pub fn new(worktree: Worktree, project: Project) -> Self {
        Self {
            worktree: worktree,
            project: project,
        }
    }
}

impl Popup for RemoveWorktreePopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match key.code {
            KeyCode::Char('y') => PopupOutcome::Submitted(Action::RemoveWorktree(
                // TODO: Shouldn't need to clone these here.
                self.project.clone(),
                self.worktree.clone(),
            )),
            KeyCode::Char('n') => PopupOutcome::Cancelled,
            _ => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let body = format!("Remove \"{}\"?", self.worktree.name);
        open_confirmation_popup(frame, area, "Remove Worktree", &body);
    }
}
