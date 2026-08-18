use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Clear, Paragraph},
};

use crate::{
    components::form::FormInput,
    theme::{ERROR, ERROR_FOCUSED, SECONDARY},
};

enum FormType {
    Standard,
    Confirmation,
    Custom,
}

pub enum FormEvent {
    Continue,
    Submit,
    Cancel,
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
                cursor_position: form_input.initial_value.chars().count(), // no non-ascii support
                is_valid: false,
                hidden: false,
            })
        }
        self.form_input_states = form_input_states;
        self.focused = self
            .form_input_states
            .iter()
            .position(|s| !s.form_input.readonly)
            .unwrap_or(0);
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
                while next < n
                    && (self.form_input_states[next].hidden
                        || self.form_input_states[next].form_input.readonly)
                {
                    next += 1;
                }
                if next < n {
                    self.focused = next;
                }
                FormEvent::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                let mut prev = self.focused;
                while prev > 0 {
                    prev -= 1;
                    if !(self.form_input_states[prev].hidden
                        || self.form_input_states[prev].form_input.readonly)
                    {
                        self.focused = prev;
                        break;
                    }
                }
                FormEvent::Continue
            }
            KeyCode::Char(c) => {
                let state = &mut self.form_input_states[self.focused];
                if state.form_input.readonly {
                    return FormEvent::Continue;
                }
                if state.form_input.is_text() {
                    let byte_idx = state
                        .value
                        .char_indices()
                        .nth(state.cursor_position)
                        .map(|(i, _)| i)
                        .unwrap_or(state.value.len());
                    state.value.insert(byte_idx, c);
                    self.move_cursor_right();
                }
                FormEvent::Continue
            }
            KeyCode::Backspace => {
                let state = &mut self.form_input_states[self.focused];
                if state.form_input.readonly {
                    return FormEvent::Continue;
                }
                if state.form_input.is_text()
                    && state.cursor_position > 0
                    && let Some((byte_idx, _)) =
                        state.value.char_indices().nth(state.cursor_position - 1)
                {
                    state.value.remove(byte_idx);
                    self.move_cursor_left();
                }
                FormEvent::Continue
            }
            KeyCode::Left => {
                // let state = &mut self.form_input_states[self.focused];
                // if state.form_input.is_boolean() && state.value == "true" {
                //     state.value = "false".to_string();
                // } else if state.form_input.is_text() {
                //     self.move_cursor_left();
                // }
                self.move_cursor_left();
                FormEvent::Continue
            }
            KeyCode::Right => {
                // let state = &mut self.form_input_states[self.focused];
                // if state.form_input.is_boolean() && state.value == "false" {
                //     state.value = "true".to_string();
                // } else if state.form_input.is_text() {
                //     self.move_cursor_right();
                // }
                self.move_cursor_right();
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
        let value_span = Span::raw(state.value.clone());
        // if state.form_input.is_boolean() {
        //     let span_content = if state.value == "true" { "[X]" } else { "[ ]" };
        //     value_span = Span::raw(span_content);
        // }
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
            input_state.is_valid = input_state.form_input.is_valid(&input_state.value)
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
                    !(value == "true" && dependant_on.1 || value == "false" && !dependant_on.1);
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

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    #[test]
    fn test_type_char_at_string_end() {
        // TODO: .required() is necessary to set a text input validation. Fix that later.
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "a");
        assert_eq!(form.form_input_states[0].cursor_position, 1);
    }

    #[test]
    fn test_type_char_mid_string() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 3;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "helglo");
        assert_eq!(form.form_input_states[0].cursor_position, 4);
    }

    #[test]
    fn test_type_multi_byte_char() {
        // TODO: Fix this with multi byte char
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('é'), KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "é");
        assert_eq!(form.form_input_states[0].cursor_position, 1);
    }

    #[test]
    fn test_backspace_at_zero_index() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 0;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 0);
    }

    #[test]
    fn test_backspace_mid_string() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 2;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hllo");
        assert_eq!(form.form_input_states[0].cursor_position, 1);
    }

    #[test]
    fn test_backspace_at_string_end() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 5;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hell");
        assert_eq!(form.form_input_states[0].cursor_position, 4);
    }

    #[test]
    fn test_backspace_multi_byte_char() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("é");
        form.form_input_states[0].cursor_position = 1;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "");
        assert_eq!(form.form_input_states[0].cursor_position, 0);
    }

    #[test]
    fn test_cursor_left_at_zero_index() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 0;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 0);
    }

    #[test]
    fn test_cursor_left_mid_string() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 3;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 2);
    }

    #[test]
    fn test_cursor_left_at_string_end() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 5;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 4);
    }

    #[test]
    fn test_cursor_right_at_zero_index() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 0;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 1);
    }

    #[test]
    fn test_cursor_right_mid_string() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 3;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 4);
    }

    #[test]
    fn test_cursor_right_at_string_end() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        form.form_input_states[0].value = String::from("hello");
        form.form_input_states[0].cursor_position = 5;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        assert_eq!(form.form_input_states[0].cursor_position, 5);
    }

    #[test]
    fn test_tab_moves_focus_down() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        form.focused = 0;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(form.focused, 1);
    }

    #[test]
    fn test_down_arrow_moves_focus_down() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        form.focused = 0;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
        assert_eq!(form.focused, 1);
    }

    #[test]
    fn test_focus_down_has_no_effect_on_last_input() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        form.focused = 1;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(form.focused, 1);
    }

    #[test]
    fn test_backtab_moves_focus_up() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        form.focused = 1;
        form.match_standard_input_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()));
        assert_eq!(form.focused, 0);
    }

    #[test]
    fn test_up_arrow_moves_focus_up() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        form.focused = 1;
        form.match_standard_input_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
        assert_eq!(form.focused, 0);
    }

    #[test]
    fn test_focus_up_has_no_effect_on_first_input() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        assert_eq!(form.focused, 0);
        form.match_standard_input_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()));
        assert_eq!(form.focused, 0);
    }

    // #[test]
    // fn test_right_arrow_toggles_boolean_input_true() {
    //     let mut form: Form = Form::standard().inputs(vec![
    //         FormInput::new()
    //             .boolean()
    //             .initial_value(String::from("false")),
    //     ]);
    //     form.match_standard_input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
    //     assert_eq!(form.form_input_states[0].value, "true");
    // }

    // #[test]
    // fn test_left_arrow_toggles_boolean_input_false() {
    //     let mut form: Form = Form::standard().inputs(vec![
    //         FormInput::new()
    //             .boolean()
    //             .initial_value(String::from("true")),
    //     ]);
    //     form.match_standard_input_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    //     assert_eq!(form.form_input_states[0].value, "false");
    // }

    #[test]
    fn test_readonly_input_is_not_focusable() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .initial_value(String::from("hello"))
                .readonly(),
            FormInput::new().required(),
        ]);
        assert_eq!(form.focused, 1);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
        assert_eq!(form.focused, 1);
    }

    #[test]
    fn test_readonly_input_is_not_editable() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .initial_value(String::from("hello"))
                .readonly(),
        ]);
        form.focused = 0;
        assert_eq!(form.form_input_states[0].value, "hello");
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, "hello");
    }
}
