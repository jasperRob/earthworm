use crossterm::event::{KeyCode, KeyEvent};

pub struct FormPopup {
    pub labels: Vec<&'static str>,
    pub values: Vec<String>,
    pub focused: usize,
}

pub enum FormEvent {
    Continue,
    Submit,
    Cancel,
}

impl FormPopup {
    pub fn new(fields: &[(&'static str, String)]) -> Self {
        let (labels, fields): (Vec<&'static str>, Vec<String>) =
            fields.iter().map(|(l, v)| (*l, v.clone())).unzip();
        Self {
            labels: labels,
            values: fields,
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
                self.values[self.focused].push(c);
                FormEvent::Continue
            }
            KeyCode::Backspace => {
                self.values[self.focused].pop();
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
}
