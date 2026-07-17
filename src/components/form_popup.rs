use crossterm::event::{KeyCode, KeyEvent};

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
    pub dependant_on: Option<i32>,
}

#[derive(Clone)]
pub struct FormInputState {
    pub form_input: String,
    pub value: String,
    pub cursor_position: usize,
    pub is_valid: bool,
    pub hidden: bool,
}

pub struct FormPopup {
    pub form_inputs: Vec<FormInput>,
    pub focused: usize,
}

impl FormPopup {
    pub fn new(form_inputs: Vec<FormInput>) -> Self {
        let mut cursor_positions = Vec::new();
        for form_input in form_inputs.clone() {
            cursor_positions.push(form_input.value.chars().count());
        }
        Self {
            form_inputs,
            cursor_positions,
            focused: 0,
        }
    }

    pub fn non_hidden_inputs(&self) -> Vec<&FormInput> {
        self.form_inputs.iter().filter(|i| !i.hidden).collect()
    }

    fn focused_index(&self) -> usize {
        self.form_inputs
            .iter()
            .enumerate()
            .filter(|(_, i)| !i.hidden)
            .map(|(idx, _)| idx)
            .nth(self.focused)
            .expect("focused is a valid non-hidden index")
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormEvent {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.focused = (self.focused + 1) % self.non_hidden_inputs().len();
                FormEvent::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.saturating_sub(1);
                FormEvent::Continue
            }
            KeyCode::Char(c) => {
                if self.non_hidden_inputs()[self.focused]
                    .value
                    .is_char_boundary(self.cursor_positions[self.focused])
                {
                    let idx = self.focused_index();
                    self.form_inputs[idx]
                        .value
                        .insert(self.cursor_positions[self.focused], c);
                    self.move_cursor_right();
                }
                FormEvent::Continue
            }
            KeyCode::Backspace => {
                if self.cursor_positions[self.focused] > 0 {
                    self.move_cursor_left();
                    let idx = self.focused_index();
                    let cursor_idx = self.cursor_positions[idx];
                    let s = &self.non_hidden_inputs()[self.focused].value;
                    if cursor_idx < s.len() && s.is_char_boundary(cursor_idx) {
                        self.non_hidden_inputs()[self.focused]
                            .value
                            .remove(cursor_idx);
                    }
                }
                FormEvent::Continue
            }
            KeyCode::Left => {
                self.move_cursor_left();
                FormEvent::Continue
            }
            KeyCode::Right => {
                self.move_cursor_right();
                FormEvent::Continue
            }
            KeyCode::Enter => FormEvent::Submit,
            KeyCode::Esc => FormEvent::Cancel,
            _ => FormEvent::Continue,
        }
    }

    pub fn value(&self, i: usize) -> &str {
        &self.form_inputs[i].value
    }

    pub fn validate(&self, i: usize) -> bool {
        let value = self.value(i);
        match &self.form_inputs[i].validation {
            Some(InputValidation::Text(rules)) => rules.iter().all(|r| match r {
                TextRule::NonEmpty => !value.is_empty(),
                TextRule::OneOf(options) => value.is_empty() || options.iter().any(|o| o == value),
            }),
            Some(InputValidation::Boolean) => value == "true" || value == "false",
            None => true,
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.cursor_positions[self.focused].saturating_sub(1);
        self.cursor_positions[self.focused] = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.cursor_positions[self.focused].saturating_add(1);
        self.cursor_positions[self.focused] = self.clamp_cursor(cursor_moved_right);
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.values[self.focused].chars().count())
    }
}
