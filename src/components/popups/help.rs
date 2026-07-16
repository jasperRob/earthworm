use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Margin, Rect},
    style::{Modifier, Style},
    widgets::{Block, Cell, Clear, Row, Table},
};

use crate::{
    action::Action,
    components::popups::{Popup, PopupOutcome},
};

const DISPLAY_ORDER: &[Action] = &[
    Action::Quit,
    Action::Suspend,
    Action::Help,
    Action::CmdSelectNext,
    Action::CmdSelectPrev,
    Action::CmdJumpTop,
    Action::CmdJumpBottom,
    Action::CmdStartSearch,
    Action::CmdSearchNext,
    Action::CmdSearchPrev,
    Action::CmdAddProject,
    Action::CmdAddSession,
    Action::CmdDeleteItem,
    Action::CmdAttach,
    Action::CmdEdit,
    Action::CmdNextSessionHistory,
    Action::CmdPrevSessionHistory,
];

pub struct HelpPopup {
    keymaps: Vec<(String, String)>,
}

impl HelpPopup {
    pub fn new(keymaps: Vec<(String, String)>) -> Self {
        Self { keymaps }
    }
}

impl Popup for HelpPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        match key.code {
            KeyCode::Char('q') => PopupOutcome::Cancelled,
            KeyCode::Esc => PopupOutcome::Cancelled,
            _ => PopupOutcome::Pending,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let key_column_title = "Key";
        let action_column_title = "Action";

        let header = 1;
        let border = 2;
        let padding = 2;
        let column_spacing = 5;
        let line_height: u16 = 1;

        let ordered_keymaps: Vec<(String, String)> = DISPLAY_ORDER
            .iter()
            .filter_map(|action| {
                let action_str = action.to_string();
                let matches: Vec<String> = self
                    .keymaps
                    .iter()
                    .filter(|(_, act)| act == &action_str)
                    .map(|(key, _)| key.clone())
                    .collect();
                if matches.is_empty() {
                    None
                } else {
                    Some((matches.join(", "), action_str))
                }
            })
            .collect();

        let max_key_length = ordered_keymaps
            .iter()
            .map(|(key, _)| key.len())
            .max()
            .unwrap_or(key_column_title.len()) as u16;
        let max_action_length = ordered_keymaps
            .iter()
            .map(|(_, action)| action.len())
            .max()
            .unwrap_or(action_column_title.len()) as u16;

        let width = max_key_length + max_action_length + border + padding + column_spacing;
        let height = (ordered_keymaps.len() as u16 * line_height) + header + border + padding;

        let popup = area.centered(Constraint::Length(width), Constraint::Length(height));
        frame.render_widget(Clear, popup);
        frame.render_widget(Block::bordered().title("Help"), popup);
        let inner = popup.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        let rows: Vec<Row> = ordered_keymaps
            .iter()
            .map(|(key, action)| {
                Row::new([Cell::from(key.as_str()), Cell::from(action.as_str())])
                    .height(line_height)
            })
            .collect();

        let table = Table::new(
            rows,
            [Constraint::Length(max_key_length), Constraint::Fill(1)],
        )
        .column_spacing(column_spacing)
        .header(
            Row::new([key_column_title, action_column_title])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        );

        frame.render_widget(table, inner);
    }
}
