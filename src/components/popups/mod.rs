pub mod edit_project;
pub mod edit_session;
pub mod help;
pub mod new_project;
pub mod new_session;
pub mod new_session_from_worktree;
pub mod new_session_in_project;
pub mod remove_project;
pub mod remove_session;
pub mod remove_worktree;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::action::Action;

#[derive(Default)]
pub enum PopupState {
    #[default]
    Closed,
    Open(Box<dyn Popup>),
}

pub enum PopupOutcome {
    Pending,
    Submitted(Action),
    Cancelled,
}

pub trait Popup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome;
    fn draw(&mut self, frame: &mut Frame, area: Rect);
}
