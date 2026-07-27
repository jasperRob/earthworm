use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Clear, Paragraph},
};

use crate::theme::{ERROR, ERROR_FOCUSED, SECONDARY};

pub enum FormType {
    Standard,
    Confirmation,
    Custom,
}

#[derive(Clone)]
pub enum InputValidation {
    Text(Vec<TextRule>),
    Boolean,
}

#[derive(Clone)]
pub enum TextRule {
    NonEmpty,
    OneOf(Vec<String>),
}

pub enum FormEvent {
    Continue,
    Submit,
    Cancel,
}

#[derive(Clone)]
pub struct FormInput {
    pub label: String,
    pub initial_value: String,
    pub validation: Option<InputValidation>,
    pub dependant_on: Option<(usize, bool)>,
}

#[derive(Clone)]
pub struct FormInputState {
    pub form_input: FormInput,
    pub value: String,
    pub cursor_position: usize,
    pub is_valid: bool,
    pub hidden: bool,
}

pub struct Form {
    title: String,
    body: String,
    form_type: FormType,
    form_input_states: Vec<FormInputState>,
    focused: usize,
}

impl Form {
    pub fn standard() -> Self {
        Self {
            title: String::default(),
            body: String::default(),
            form_type: FormType::Standard,
            form_input_states: vec![],
            focused: 0,
        }
    }

    pub fn confirmation() -> Self {
        Self {
            title: String::default(),
            body: String::default(),
            form_type: FormType::Confirmation,
            form_input_states: vec![],
            focused: 0,
        }
    }

    pub fn custom() -> Self {
        Self {
            title: String::default(),
            body: String::default(),
            form_type: FormType::Custom,
            form_input_states: vec![],
            focused: 0,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn body(mut self, body: String) -> Self {
        self.body = body;
        self
    }

    pub fn inputs(mut self, form_inputs: Vec<FormInput>) -> Self {
        let mut form_input_states = Vec::new();
        for form_input in form_inputs.clone() {
            form_input_states.push(FormInputState {
                form_input: form_input.clone(),
                value: form_input.initial_value.clone(),
                cursor_position: form_input.initial_value.len(),
                is_valid: false,
                hidden: false,
            })
        }
        self.form_input_states = form_input_states;
        self
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match self.form_type {
            FormType::Standard => {
                self.validate();
                self.draw_standard_form(frame, area);
            }
            FormType::Confirmation => {
                self.draw_confirmation_form(frame, area);
            }
            _ => {}
        }
    }
    pub fn handle_key(&mut self, key: KeyEvent) -> FormEvent {
        match self.form_type {
            FormType::Standard => self.match_standard_input_key(key),
            FormType::Confirmation => self.match_confirmation_input_key(key),
            _ => FormEvent::Continue,
        }
    }

    fn match_standard_input_key(&mut self, key: KeyEvent) -> FormEvent {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                let n = self.form_input_states.len();
                let mut next = self.focused + 1;
                while next < n && self.form_input_states[next].hidden {
                    next += 1;
                }
                if next < n {
                    self.focused = next;
                }
                FormEvent::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                let mut prev = self.focused - 1;
                while prev > 0 {
                    prev -= 1;
                    if !self.form_input_states[prev].hidden {
                        self.focused = prev;
                        break;
                    }
                }
                FormEvent::Continue
            }
            KeyCode::Char(c) => {
                let state = &mut self.form_input_states[self.focused];
                if let Some(InputValidation::Text(_)) = state.form_input.validation
                    && state.value.is_char_boundary(state.cursor_position)
                {
                    state.value.insert(state.cursor_position, c);
                    self.move_cursor_right();
                }
                FormEvent::Continue
            }
            KeyCode::Backspace => {
                let state = &mut self.form_input_states[self.focused];
                if let Some(InputValidation::Text(_)) = state.form_input.validation
                    && state.cursor_position > 0
                {
                    let remove_at = state.cursor_position - 1;
                    state.value.remove(remove_at);
                    self.move_cursor_left();
                }
                FormEvent::Continue
            }
            KeyCode::Left => {
                let state = &mut self.form_input_states[self.focused];
                match state.form_input.validation {
                    Some(InputValidation::Boolean) if state.value == "true" => {
                        state.value = "false".to_string();
                    }
                    Some(InputValidation::Text(_)) => self.move_cursor_left(),
                    _ => {}
                }
                FormEvent::Continue
            }
            KeyCode::Right => {
                let state = &mut self.form_input_states[self.focused];
                match state.form_input.validation {
                    Some(InputValidation::Boolean) if state.value == "false" => {
                        state.value = "true".to_string();
                    }
                    Some(InputValidation::Text(_)) => self.move_cursor_right(),
                    _ => {}
                }
                FormEvent::Continue
            }
            KeyCode::Enter => FormEvent::Submit,
            KeyCode::Esc => FormEvent::Cancel,
            _ => FormEvent::Continue,
        }
    }

    fn match_confirmation_input_key(&mut self, key: KeyEvent) -> FormEvent {
        match key.code {
            KeyCode::Char('y') => FormEvent::Submit,
            KeyCode::Char('n') | KeyCode::Esc => FormEvent::Cancel,
            _ => FormEvent::Continue,
        }
    }

    fn draw_standard_form(&self, frame: &mut Frame, area: Rect) {
        let non_hidden_input_states: Vec<&FormInputState> = self
            .form_input_states
            .iter()
            .filter(|state| !state.hidden)
            .collect();

        let height = (2 * non_hidden_input_states.len() - 1) as u16 + 4;
        let popup = area.centered(Constraint::Percentage(40), Constraint::Length(height));
        // clears out any background in the area before rendering the popup
        self.draw_form_frame(frame, popup);
        let inner = popup.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let mut constraints: Vec<Constraint> = vec![Constraint::Fill(1)];
        for _ in 0..non_hidden_input_states.len() {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(1));
        }
        constraints.pop();
        constraints.push(Constraint::Fill(1));

        let areas = Layout::vertical(constraints).split(inner);

        let mut active_area = Rect::default();
        let mut active_label = String::default();

        let visible = self
            .form_input_states
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.hidden);
        for (slot, (real_idx, state)) in visible.enumerate() {
            let is_focused = self.focused == real_idx;
            let widget = self.labeled_input(state, is_focused);
            let area = areas[(slot * 2) + 1];
            frame.render_widget(widget, area);
            if is_focused {
                active_area = area;
                active_label = state.form_input.label.to_string();
            }
        }

        frame.set_cursor_position((
            active_area.x
                + 1
                + active_label.len() as u16
                + 2
                + self.form_input_states[self.focused].cursor_position as u16,
            active_area.y,
        ));
    }

    fn labeled_input(&self, state: &FormInputState, is_focused: bool) -> Paragraph<'_> {
        let label_style = if !state.is_valid && is_focused {
            Style::default().fg(ERROR_FOCUSED)
        } else if !state.is_valid {
            Style::default().fg(ERROR)
        } else if is_focused {
            Style::default().fg(SECONDARY)
        } else {
            Style::default()
        };
        let mut value_span = Span::raw(state.value.clone());
        if let Some(validation) = &state.form_input.validation
            && let InputValidation::Boolean = validation
        {
            let span_content = if state.value == "true" { "[X]" } else { "[ ]" };
            value_span = Span::raw(span_content);
        }
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {}: ", state.form_input.label), label_style),
            value_span,
        ]))
    }

    fn draw_confirmation_form(&self, frame: &mut Frame, area: Rect) {
        let popup = area.centered(
            Constraint::Length(self.body.len() as u16 * 2),
            Constraint::Length(10),
        );
        self.draw_form_frame(frame, popup);
        let inner = popup.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        let [_, text_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(4),
            Constraint::Fill(1),
        ])
        .areas(inner);
        let text = Text::from(vec![
            Line::from(self.body.clone()).centered(),
            Line::from(""),
            Line::from(""),
            Line::from("y / n").centered(),
        ]);
        frame.render_widget(Paragraph::new(text), text_area);
    }

    pub fn draw_form_frame(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        frame.render_widget(Block::bordered().title(self.title.clone()), area);
    }

    fn validate(&mut self) {
        self.form_input_states.iter_mut().for_each(|input_state| {
            input_state.is_valid = match &input_state.form_input.validation {
                Some(InputValidation::Text(rules)) => rules.iter().all(|r| match r {
                    TextRule::NonEmpty => !input_state.value.is_empty(),
                    TextRule::OneOf(options) => {
                        input_state.value.is_empty() || options.contains(&input_state.value)
                    }
                }),
                Some(InputValidation::Boolean) => {
                    input_state.value == "true" || input_state.value == "false"
                }
                None => true,
            }
        });
        let valid: Vec<bool> = self.form_input_states.iter().map(|s| s.is_valid).collect();
        let values: Vec<String> = self
            .form_input_states
            .iter()
            .map(|s| s.value.clone())
            .collect();
        self.form_input_states.iter_mut().for_each(|input_state| {
            if let Some(dependant_on) = input_state.form_input.dependant_on
                && let Some(&is_valid) = valid.get(dependant_on.0)
                && is_valid
                && let Some(value) = values.get(dependant_on.0)
            {
                input_state.hidden =
                    value == "true" && dependant_on.1 || value == "false" && !dependant_on.1;
            }
        });
    }

    fn move_cursor_left(&mut self) {
        let moved = self.form_input_states[self.focused]
            .cursor_position
            .saturating_sub(1);
        let clamped = self.clamp_cursor(moved);
        self.form_input_states[self.focused].cursor_position = clamped;
    }

    fn move_cursor_right(&mut self) {
        let moved = self.form_input_states[self.focused]
            .cursor_position
            .saturating_add(1);
        let clamped = self.clamp_cursor(moved);
        self.form_input_states[self.focused].cursor_position = clamped;
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(
            0,
            self.form_input_states[self.focused].value.chars().count(),
        )
    }

    pub fn value(&self, index: usize) -> String {
        self.form_input_states[index].value.clone()
    }
}
