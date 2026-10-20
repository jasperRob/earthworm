use std::{
    fs,
    path::Path,
    sync::mpsc::{Receiver, Sender, channel},
};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, Paragraph},
};

use crate::{
    components::form::{
        FormInput,
        input::{expand_home, normalise_path},
    },
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
    pub suggested_path: Option<String>,
    /// Id of the latest suggestion requested for this input; older results are discarded
    pub suggestion_request: u64,
}

/// A path suggestion computed off the UI thread
struct PathSuggestion {
    input_index: usize,
    request: u64,
    suggestion: String,
}

pub struct Form {
    title: String,
    body: String,
    form_type: FormType,
    form_input_states: Vec<FormInputState>,
    focused: usize,
    suggestion_tx: Sender<PathSuggestion>,
    suggestion_rx: Receiver<PathSuggestion>,
}

impl Form {
    fn new(form_type: FormType) -> Self {
        let (suggestion_tx, suggestion_rx) = channel();
        Self {
            title: String::default(),
            body: String::default(),
            form_type,
            form_input_states: vec![],
            focused: 0,
            suggestion_tx,
            suggestion_rx,
        }
    }

    pub fn standard() -> Self {
        Self::new(FormType::Standard)
    }

    pub fn confirmation() -> Self {
        Self::new(FormType::Confirmation)
    }

    pub fn custom() -> Self {
        Self::new(FormType::Custom)
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
                suggested_path: None,
                suggestion_request: 0,
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
        self.apply_suggestions();
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
        self.apply_suggestions();
        match self.form_type {
            FormType::Standard => self.match_standard_input_key(key),
            FormType::Confirmation => self.match_confirmation_input_key(key),
            _ => FormEvent::Continue,
        }
    }

    fn match_standard_input_key(&mut self, key: KeyEvent) -> FormEvent {
        match key.code {
            KeyCode::Char(' ' | 'l') | KeyCode::Right if self.focused_is_choice() => {
                self.cycle_choice(true);
                FormEvent::Continue
            }
            KeyCode::Char('h') | KeyCode::Left if self.focused_is_choice() => {
                self.cycle_choice(false);
                FormEvent::Continue
            }
            KeyCode::Tab | KeyCode::Down => {
                let n = self.form_input_states.len();
                let mut next = self.focused + 1;
                while next < n && self.form_input_states[next].form_input.readonly {
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
                    if !self.form_input_states[prev].form_input.readonly {
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
                    let is_path = state.form_input.is_path();
                    self.move_cursor_right();
                    if is_path {
                        self.request_suggestion();
                    }
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
                    let is_path = state.form_input.is_path();
                    self.move_cursor_left();
                    if is_path {
                        self.request_suggestion();
                    }
                }
                FormEvent::Continue
            }
            KeyCode::Left => {
                self.move_cursor_left();
                FormEvent::Continue
            }
            KeyCode::Right => {
                let state = &mut self.form_input_states[self.focused];
                if state.form_input.is_path()
                    && state.cursor_position == state.value.chars().count()
                    && let Some(suggestion) = &state.suggested_path
                {
                    state.value.push_str(suggestion);
                    self.move_cursor_end();
                    self.request_suggestion();
                } else {
                    self.move_cursor_right();
                }
                FormEvent::Continue
            }
            KeyCode::Enter => {
                self.validate();
                match self.first_invalid_input() {
                    None => FormEvent::Submit,
                    Some(index) => {
                        if !self.form_input_states[index].form_input.readonly {
                            self.focused = index;
                        }
                        FormEvent::Continue
                    }
                }
            }
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
        let height = (2 * self.form_input_states.len() - 1) as u16 + 4;
        let popup = area.centered(Constraint::Percentage(40), Constraint::Length(height));
        // clears out any background in the area before rendering the popup
        self.draw_form_frame(frame, popup);
        let inner = popup.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let mut constraints: Vec<Constraint> = vec![Constraint::Fill(1)];
        for _ in 0..self.form_input_states.len() {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(1));
        }
        constraints.pop();
        constraints.push(Constraint::Fill(1));

        let areas = Layout::vertical(constraints).split(inner);

        let mut active_area = Rect::default();
        let mut active_label = String::default();

        for (index, state) in self.form_input_states.iter().enumerate() {
            let is_focused = self.focused == index;
            let widget = self.labeled_input(state, is_focused);
            let area = areas[(index * 2) + 1];
            frame.render_widget(widget, area);
            if is_focused {
                active_area = area;
                active_label = state.form_input.label.to_string();
            }
        }

        // A choice input has no cursor, and the cursor is hidden for frames that don't set it
        let focused = &self.form_input_states[self.focused];
        if !focused.form_input.is_choice() {
            frame.set_cursor_position((
                active_area.x + 1 + active_label.len() as u16 + 2 + focused.cursor_position as u16,
                active_area.y,
            ));
        }
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
        let mut spans = vec![Span::styled(
            format!(" {}: ", state.form_input.label),
            label_style,
        )];
        if state.form_input.is_choice() {
            // Every choice is listed, so it's clear what the input can switch to
            for choice in &state.form_input.choices {
                let style = if *choice == state.value {
                    Style::default()
                        .fg(SECONDARY)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                spans.push(Span::styled(choice.clone(), style));
                spans.push(Span::raw("  "));
            }
        } else {
            spans.push(Span::raw(state.value.clone()));
            spans.push(Span::styled(
                state.suggested_path.clone().unwrap_or_default(),
                Style::default().fg(Color::DarkGray),
            ));
        }
        Paragraph::new(Line::from(spans))
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
    }

    fn first_invalid_input(&self) -> Option<usize> {
        self.form_input_states.iter().position(|s| !s.is_valid)
    }

    fn focused_is_choice(&self) -> bool {
        let input = &self.form_input_states[self.focused].form_input;
        input.is_choice() && !input.readonly
    }

    /// Moves the focused choice input to its next or previous choice, wrapping around. A value
    /// that isn't one of the choices moves to the first.
    fn cycle_choice(&mut self, forward: bool) {
        let state = &mut self.form_input_states[self.focused];
        let choices = &state.form_input.choices;
        let n = choices.len();
        let next = match choices.iter().position(|c| *c == state.value) {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None => 0,
        };
        state.value = choices[next].clone();
    }

    // TODO: These should take a form input state as a param. The use of self should be separated
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

    fn move_cursor_end(&mut self) {
        let pos = self.form_input_states[self.focused].value.chars().count();
        self.form_input_states[self.focused].cursor_position = pos;
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(
            0,
            self.form_input_states[self.focused].value.chars().count(),
        )
    }

    /// Computes a suggestion for the focused input off the UI thread, as reading the
    /// directory can be slow. The stale suggestion is hidden until the new one arrives.
    fn request_suggestion(&mut self) {
        let input_index = self.focused;
        let state = &mut self.form_input_states[input_index];
        state.suggested_path = None;
        state.suggestion_request += 1;
        let request = state.suggestion_request;
        let input = state.value.clone();
        let tx = self.suggestion_tx.clone();
        tokio::task::spawn_blocking(move || {
            let _ = tx.send(PathSuggestion {
                input_index,
                request,
                suggestion: Self::return_suggested(input),
            });
        });
    }

    fn apply_suggestions(&mut self) {
        while let Ok(result) = self.suggestion_rx.try_recv() {
            if let Some(state) = self.form_input_states.get_mut(result.input_index)
                && state.suggestion_request == result.request
            {
                state.suggested_path = Some(result.suggestion);
            }
        }
    }

    fn return_suggested(input: String) -> String {
        if input.ends_with("/") {
            return String::new();
        }
        let expanded = expand_home(&input);
        let path = expanded.as_path();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));

        let file_name = match path.file_name() {
            Some(name) => name,
            None => return String::new(),
        };

        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(_) => return String::new(),
        };

        entries
            .filter_map(|e| e.ok())
            .find(|e| {
                e.file_name()
                    .as_encoded_bytes()
                    .starts_with(file_name.as_encoded_bytes())
            })
            .and_then(|e| {
                let full_name = e.file_name().to_string_lossy().to_string();
                let prefix = file_name.to_string_lossy();

                // Strip the prefix that was already typed
                full_name
                    .strip_prefix(prefix.as_ref())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default()
    }

    /// Returns the value of the input at `index`. Path inputs are returned normalised.
    pub fn value(&self, index: usize) -> String {
        let state = &self.form_input_states[index];
        if state.form_input.is_path() && !state.value.is_empty() {
            normalise_path(&state.value).to_string_lossy().into_owned()
        } else {
            state.value.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf, time::Duration};

    use crossterm::event::KeyModifiers;
    use tempfile::TempDir;

    use super::*;

    /// Waits for the pending path suggestion to be computed, then applies it
    fn settle_suggestion(form: &mut Form) {
        let result = form
            .suggestion_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        form.suggestion_tx.send(result).unwrap();
        form.apply_suggestions();
    }

    #[test]
    fn test_type_char_at_string_end() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new()]);
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

    // TODO: Should recompute the suggestion on any character or backspace, and should only
    // autocomplete if the right key is pressed from the end of the string
    //
    // TODO:
    // This should also work fine with unicode chars

    /// Creates a temp dir containing only a `src` dir. Returns it with its path, which
    /// ends in a `/`.
    fn suggestion_dir() -> (TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        let root = format!("{}/", dir.path().display());
        (dir, root)
    }

    #[tokio::test]
    async fn test_right_arrow_at_string_end_autocompletes_path() {
        let (_dir, root) = suggestion_dir();
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().path()]);
        form.form_input_states[0].value = root.clone();
        form.form_input_states[0].cursor_position = root.chars().count();
        assert_eq!(form.form_input_states[0].suggested_path.as_deref(), None);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()));
        settle_suggestion(&mut form);
        assert_eq!(
            form.form_input_states[0].suggested_path.as_deref(),
            Some("rc")
        );
        form.match_standard_input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, format!("{root}src"));
    }

    #[tokio::test]
    async fn test_right_arrow_mid_string_does_not_autocompletes() {
        let (_dir, root) = suggestion_dir();
        let parent = root.trim_end_matches('/');
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().path()]);
        form.form_input_states[0].value = format!("{parent}s");
        form.form_input_states[0].cursor_position = parent.chars().count();
        assert_eq!(form.form_input_states[0].suggested_path.as_deref(), None);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        settle_suggestion(&mut form);
        assert_eq!(
            form.form_input_states[0].suggested_path.as_deref(),
            Some("rc")
        );
        form.match_standard_input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].value, format!("{root}s"));
    }

    #[tokio::test]
    async fn test_backspace_at_string_end_recomputes_suggestion() {
        let (_dir, root) = suggestion_dir();
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().path()]);
        form.form_input_states[0].value = format!("{root}sx");
        form.form_input_states[0].cursor_position = root.chars().count() + 2;
        assert_eq!(form.form_input_states[0].suggested_path.as_deref(), None);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        settle_suggestion(&mut form);
        assert_eq!(
            form.form_input_states[0].suggested_path.as_deref(),
            Some("rc")
        );
    }

    #[tokio::test]
    async fn test_backspace_mid_string_recomputes_suggestion() {
        let (_dir, root) = suggestion_dir();
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().path()]);
        form.form_input_states[0].value = format!("{root}xs");
        form.form_input_states[0].cursor_position = root.chars().count() + 1;
        assert_eq!(form.form_input_states[0].suggested_path.as_deref(), None);
        form.match_standard_input_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        settle_suggestion(&mut form);
        assert_eq!(
            form.form_input_states[0].suggested_path.as_deref(),
            Some("rc")
        );
    }

    #[tokio::test]
    async fn test_typing_hides_suggestion_until_recomputed() {
        let (_dir, root) = suggestion_dir();
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().path()]);
        form.form_input_states[0].value = format!("{root}s");
        form.form_input_states[0].cursor_position = root.chars().count() + 1;
        form.form_input_states[0].suggested_path = Some(String::from("rc"));
        form.match_standard_input_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
        assert_eq!(form.form_input_states[0].suggested_path.as_deref(), None);
        settle_suggestion(&mut form);
        assert_eq!(
            form.form_input_states[0].suggested_path.as_deref(),
            Some("")
        );
    }

    #[test]
    fn test_stale_suggestion_is_discarded() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().path()]);
        form.form_input_states[0].suggestion_request = 2;
        form.suggestion_tx
            .send(PathSuggestion {
                input_index: 0,
                request: 1,
                suggestion: String::from("stale"),
            })
            .unwrap();
        form.apply_suggestions();
        assert_eq!(form.form_input_states[0].suggested_path.as_deref(), None);
    }

    fn press_enter(form: &mut Form) -> FormEvent {
        form.match_standard_input_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()))
    }

    #[test]
    fn test_enter_with_empty_required_input_does_not_submit() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new().required()]);
        assert!(matches!(press_enter(&mut form), FormEvent::Continue));
    }

    #[test]
    fn test_enter_with_valid_inputs_submits() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .initial_value(String::from("hello"))
                .required(),
        ]);
        assert!(matches!(press_enter(&mut form), FormEvent::Submit));
    }

    #[test]
    fn test_enter_focuses_first_invalid_input() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .initial_value(String::from("hello"))
                .required(),
            FormInput::new().required(),
            FormInput::new().required(),
        ]);
        assert_eq!(form.focused, 0);
        assert!(matches!(press_enter(&mut form), FormEvent::Continue));
        assert_eq!(form.focused, 1);
    }

    #[test]
    fn test_enter_with_missing_path_does_not_submit() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .path()
                .initial_value(missing.to_string_lossy().into_owned())
                .required(),
        ]);
        assert!(matches!(press_enter(&mut form), FormEvent::Continue));
        fs::create_dir(&missing).unwrap();
        assert!(matches!(press_enter(&mut form), FormEvent::Submit));
    }

    #[test]
    fn test_value_normalises_path_input() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("child")).unwrap();
        let form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .path()
                .initial_value(format!("{}/./child/../child/", dir.path().display())),
            FormInput::new().path().initial_value(String::from("~")),
        ]);
        assert_eq!(PathBuf::from(form.value(0)), dir.path().join("child"));
        assert_eq!(
            PathBuf::from(form.value(1)),
            PathBuf::from(env::var_os("HOME").unwrap())
        );
    }

    #[test]
    fn test_value_does_not_normalise_text_input() {
        let form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .initial_value(String::from("./foo"))
                .required(),
        ]);
        assert_eq!(form.value(0), "./foo");
    }

    fn choice_form(initial: &str) -> Form {
        Form::standard().inputs(vec![
            FormInput::new()
                .choice(["a", "b", "c"])
                .initial_value(String::from(initial)),
        ])
    }

    fn press(form: &mut Form, code: KeyCode) {
        form.match_standard_input_key(KeyEvent::new(code, KeyModifiers::empty()));
    }

    #[test]
    fn test_choice_moves_forward_and_wraps() {
        let mut form = choice_form("b");
        press(&mut form, KeyCode::Char(' '));
        assert_eq!(form.value(0), "c");
        press(&mut form, KeyCode::Right);
        assert_eq!(form.value(0), "a");
        press(&mut form, KeyCode::Char('l'));
        assert_eq!(form.value(0), "b");
    }

    #[test]
    fn test_choice_moves_back_and_wraps() {
        let mut form = choice_form("b");
        press(&mut form, KeyCode::Left);
        assert_eq!(form.value(0), "a");
        press(&mut form, KeyCode::Char('h'));
        assert_eq!(form.value(0), "c");
    }

    #[test]
    fn test_choice_is_not_typed_into() {
        let mut form = choice_form("a");
        press(&mut form, KeyCode::Char('x'));
        press(&mut form, KeyCode::Backspace);
        assert_eq!(form.value(0), "a");
    }

    #[test]
    fn test_readonly_choice_does_not_change() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new()
                .choice(["a", "b"])
                .initial_value(String::from("a"))
                .readonly(),
        ]);
        press(&mut form, KeyCode::Char(' '));
        press(&mut form, KeyCode::Left);
        assert_eq!(form.value(0), "a");
    }

    #[test]
    fn test_choice_keys_still_edit_text_inputs() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new()]);
        press(&mut form, KeyCode::Char('h'));
        press(&mut form, KeyCode::Char(' '));
        press(&mut form, KeyCode::Char('l'));
        assert_eq!(form.value(0), "h l");
    }

    #[test]
    fn test_enter_with_invalid_choice_focuses_it_until_changed() {
        let mut form: Form = Form::standard().inputs(vec![
            FormInput::new().initial_value(String::from("text")),
            FormInput::new()
                .choice(["a", "b"])
                .initial_value(String::from("x")),
        ]);
        assert!(matches!(press_enter(&mut form), FormEvent::Continue));
        assert_eq!(form.focused, 1);
        press(&mut form, KeyCode::Char(' '));
        assert_eq!(form.value(1), "a");
        assert!(matches!(press_enter(&mut form), FormEvent::Submit));
    }

    #[test]
    fn test_enter_with_empty_optional_input_submits() {
        let mut form: Form = Form::standard().inputs(vec![FormInput::new()]);
        assert!(matches!(press_enter(&mut form), FormEvent::Submit));
    }
}
