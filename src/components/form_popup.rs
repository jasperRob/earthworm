use crossterm::event::{KeyCode, KeyEvent};

pub struct FormPopup {
    pub labels: Vec<&'static str>,
    pub values: Vec<String>,
    pub cursor_positions: Vec<usize>,
    pub focused: usize,
}

pub enum FormEvent {
    Continue,
    Submit,
    Cancel,
}

impl FormPopup {
    pub fn new(fields: &[(&'static str, String)]) -> Self {
        let mut labels = Vec::new();
        let mut values = Vec::new();
        let mut cursor_positions = Vec::new();
        for (l, v) in fields {
            labels.push(*l);
            values.push(v.clone());
            cursor_positions.push(v.chars().count());
        }
        Self {
            labels,
            values,
            focused: 0,
            cursor_positions,
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
