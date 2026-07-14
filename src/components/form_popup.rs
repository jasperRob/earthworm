use crossterm::event::{KeyCode, KeyEvent};

#[derive(Clone)]
pub enum InputValidation {
    Text(Vec<TextRule>),
    // Number(Vec<NumberRule>),
}

#[derive(Clone)]
pub enum TextRule {
    NonEmpty,
    OneOf(Vec<String>),
}

// #[derive(Clone)]
// pub enum NumberRule {
//     NonEmpty,
//     NonZero,
//     Min(f64),
//     Max(f64),
// }

pub enum FormEvent {
    Continue,
    Submit,
    Cancel,
}

pub struct FormInput {
    pub label: &'static str,
    pub initial_value: String,
    pub validation: Option<InputValidation>,
}

pub struct FormPopup {
    pub labels: Vec<&'static str>,
    pub values: Vec<String>,
    pub cursor_positions: Vec<usize>,
    pub validations: Vec<Option<InputValidation>>,
    pub focused: usize,
}

impl FormPopup {
    pub fn new(form_inputs: Vec<FormInput>) -> Self {
        let mut labels = Vec::new();
        let mut values = Vec::new();
        let mut cursor_positions = Vec::new();
        let mut validations = Vec::new();
        for form_input in form_inputs {
            labels.push(form_input.label);
            values.push(form_input.initial_value.clone());
            cursor_positions.push(form_input.initial_value.chars().count());
            validations.push(form_input.validation);
        }
        Self {
            labels,
            values,
            cursor_positions: cursor_positions,
            validations: validations,
            focused: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormEvent {
        match key.code {
            KeyCode::Tab => {
                self.focused = (self.focused + 1) % self.values.len();
                FormEvent::Continue
            }
            KeyCode::BackTab => {
                self.focused = self.focused.saturating_sub(1);
                FormEvent::Continue
            }
            KeyCode::Char(c) => {
                if self.values[self.focused].is_char_boundary(self.cursor_positions[self.focused]) {
                    self.values[self.focused].insert(self.cursor_positions[self.focused], c);
                    self.move_cursor_right();
                }
                FormEvent::Continue
            }
            KeyCode::Backspace => {
                if self.cursor_positions[self.focused] > 0 {
                    self.move_cursor_left();
                    let idx = self.cursor_positions[self.focused];
                    let s = &self.values[self.focused];
                    if idx < s.len() && s.is_char_boundary(idx) {
                        self.values[self.focused].remove(idx);
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
        &self.values[i]
    }

    pub fn validate(&self, i: usize) -> bool {
        let value = self.value(i);
        match &self.validations[i] {
            Some(InputValidation::Text(rules)) => rules.iter().all(|r| match r {
                TextRule::NonEmpty => !value.is_empty(),
                TextRule::OneOf(options) => value.is_empty() || options.iter().any(|o| o == value),
            }),
            // Some(InputValidation::Number(rules)) => match value.parse::<f64>() {
            //     Err(_) => false,
            //     Ok(n) => rules.iter().all(|r| match r {
            //         NumberRule::NonEmpty => !value.is_empty(),
            //         NumberRule::NonZero => n != 0.0,
            //         NumberRule::Min(min) => value.is_empty() || n >= *min,
            //         NumberRule::Max(max) => value.is_empty() || n <= *max,
            //     }),
            // },
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
