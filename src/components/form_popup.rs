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
    pub form_input: FormInput,
    pub value: String,
    pub cursor_position: usize,
    pub is_valid: bool,
    pub hidden: bool,
}

pub struct FormPopup {
    pub form_input_states: Vec<FormInputState>,
    pub focused: usize,
}

impl FormPopup {
    pub fn new(form_inputs: Vec<FormInput>) -> Self {
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
        Self {
            form_input_states,
            focused: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormEvent {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.focused = (self.focused + 1) % self.form_input_states.len();
                FormEvent::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.saturating_sub(1);
                FormEvent::Continue
            }
            KeyCode::Char(c) => {
                let focused_input_state = self.form_input_states[self.focused];
                if focused_input_state
                    .value
                    .is_char_boundary(focused_input_state.cursor_position)
                {
                    focused_input_state
                        .value
                        .insert(focused_input_state.cursor_position, c);
                    self.move_cursor_right();
                }
                FormEvent::Continue
            }
            KeyCode::Backspace => {
                let focused_input_state = self.form_input_states[self.focused];
                if focused_input_state.cursor_position > 0 {
                    self.move_cursor_left();
                    if focused_input_state.cursor_position < focused_input_state.value.len()
                        && focused_input_state
                            .value
                            .is_char_boundary(focused_input_state.cursor_position)
                    {
                        focused_input_state
                            .value
                            .remove(focused_input_state.cursor_position);
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

    pub fn validate(&self) {
        self.form_input_states.iter().for_each(|input_state| {
            input_state.is_valid = match input_state.form_input.validation {
                Some(InputValidation::Text(rules)) => rules.iter().all(|r| match r {
                    TextRule::NonEmpty => !input_state.value.is_empty(),
                    TextRule::OneOf(options) => {
                        input_state.value.is_empty()
                            || options.iter().any(|o| o == input_state.value)
                    }
                }),
                Some(InputValidation::Boolean) => {
                    input_state.value == "true" || input_state.value == "false"
                }
                None => true,
            }
        });
    }

    fn move_cursor_left(&mut self) {
        let focused_input_state = self.form_input_states[self.focused];
        let cursor_moved_left = focused_input_state.cursor_position.saturating_sub(1);
        focused_input_state.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let focused_input_state = self.form_input_states[self.focused];
        let cursor_moved_right = focused_input_state.cursor_position.saturating_add(1);
        focused_input_state.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        let focused_input_state = self.form_input_states[self.focused];
        new_cursor_pos.clamp(0, focused_input_state.value.chars().count())
    }
}
