#[derive(Clone)]
enum TextRule {
    NonEmpty,
    // OneOf(Vec<String>),
}

#[derive(Clone)]
enum InputValidation {
    Text(TextRule),
    Path,
}

impl InputValidation {
    fn is_satisfied_by(&self, value: &str) -> bool {
        match self {
            InputValidation::Text(rule) => match rule {
                TextRule::NonEmpty => !value.is_empty(),
            },
            InputValidation::Path => true,
        }
    }
}

#[derive(Clone)]
pub struct FormInput {
    pub label: String,
    pub initial_value: String,
    input_validations: Vec<InputValidation>,
    pub dependant_on: Option<(usize, bool)>,
    pub readonly: bool,
}

impl FormInput {
    pub fn new() -> Self {
        Self {
            label: String::default(),
            initial_value: String::default(),
            input_validations: Vec::default(),
            dependant_on: None,
            readonly: false,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn initial_value(mut self, initial_value: String) -> Self {
        self.initial_value = initial_value;
        self
    }

    pub fn required(mut self) -> Self {
        self.input_validations
            .push(InputValidation::Text(TextRule::NonEmpty));
        self
    }

    pub fn path(mut self) -> Self {
        self.input_validations.push(InputValidation::Path);
        self
    }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    pub fn is_text(&self) -> bool {
        self.input_validations
            .iter()
            .any(|v| matches!(v, InputValidation::Text(_) | InputValidation::Path))
    }

    pub fn is_path(&self) -> bool {
        self.input_validations
            .iter()
            .any(|v| matches!(v, InputValidation::Path))
    }

    pub fn is_valid(&self, value: &str) -> bool {
        self.input_validations
            .iter()
            .all(|v| v.is_satisfied_by(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_required_input() {
        let form_input: FormInput = FormInput::new().required();
        assert!(form_input.is_valid("hello world"));
        assert!(!form_input.is_valid(""));
    }
}
