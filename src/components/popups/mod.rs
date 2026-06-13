pub mod edit_project;
pub mod edit_session;
pub mod new_project;
pub mod new_session;
pub mod remove_project;
pub mod remove_session;
pub mod remove_worktree;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, Paragraph},
};

use crate::{action::Action, components::form_popup::FormPopup};

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
    fn draw(&self, frame: &mut Frame, area: Rect);
}

pub(super) fn open_input_popup(frame: &mut Frame, area: Rect, title: &str, form: &FormPopup) {
    let height = (2 * form.labels.len() - 1) as u16 + 4;
    let popup = area.centered(
        Constraint::Percentage(40),
        Constraint::Length(height as u16),
    );
    // clears out any background in the area before rendering the popup
    frame.render_widget(Clear, popup);
    frame.render_widget(Block::bordered().title(title), popup);
    let inner = popup.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let mut constraints: Vec<Constraint> = vec![Constraint::Fill(1)];
    for _ in 0..form.labels.len() {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
    }
    constraints.pop();
    constraints.push(Constraint::Fill(1));

    let areas = Layout::vertical(constraints).split(inner);

    let mut active_area = Rect::default();
    let mut active_label = String::default();
    let mut active_text = String::default();
    for (i, label) in form.labels.iter().enumerate() {
        let is_focused = form.focused == i;
        let value = form.value(i);
        let widget = labeled_input(label, value, is_focused);
        let area = areas[(i * 2) + 1];
        frame.render_widget(widget, area);
        if is_focused {
            active_area = area;
            active_label = label.to_string();
            active_text = value.to_string();
        }
    }

    frame.set_cursor_position((
        active_area.x + 1 + active_label.len() as u16 + 2 + active_text.len() as u16,
        active_area.y,
    ));
}

pub(super) fn labeled_input<'a>(label: &'a str, value: &'a str, is_focused: bool) -> Paragraph<'a> {
    let label_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    Paragraph::new(Line::from(vec![
        Span::styled(format!(" {}: ", label), label_style),
        Span::raw(value),
    ]))
}

pub(super) fn open_confirmation_popup(frame: &mut Frame, area: Rect, title: &str, body: &str) {
    let popup = area.centered(
        Constraint::Length(body.len() as u16 * 2),
        Constraint::Length(11),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(Block::bordered().title(title), popup);
    let inner = popup.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let [_, text_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(inner);
    let text = Text::from(vec![
        Line::from(body).centered(),
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from("y / n").centered(),
    ]);
    frame.render_widget(Paragraph::new(text), text_area);
}
