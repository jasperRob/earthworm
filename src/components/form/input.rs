#[derive(Clone)]
enum TextRule {
    NonEmpty,
    // OneOf(Vec<String>),
}

#[derive(Clone)]
enum InputValidation {
    Text(TextRule),
    // Boolean,
}

impl InputValidation {
    fn is_satisfied_by(&self, value: &str) -> bool {
        match self {
            InputValidation::Text(rule) => match rule {
                TextRule::NonEmpty => !value.is_empty(),
                // TextRule::OneOf(options) => value.is_empty() || options.iter().any(|o| o == value),
            },
            // InputValidation::Boolean => value == "true" || value == "false",
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

    // pub fn boolean(mut self) -> Self {
    //     self.input_validations.push(InputValidation::Boolean);
    //     self
    // }

    // pub fn one_of(mut self, items: Vec<String>) -> Self {
    //     self.input_validations
    //         .push(InputValidation::Text(TextRule::OneOf(items)));
    //     self
    // }

    // pub fn dependant_on(mut self, dependant_on: (usize, bool)) -> Self {
    //     self.dependant_on = Some(dependant_on);
    //     self
    // }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    pub fn is_text(&self) -> bool {
        self.input_validations
            .iter()
            .any(|v| matches!(v, InputValidation::Text(_)))
    }

    // TODO: we shouldn't be able to add more than one if Boolean is in there (update here and in
    // the boolean() method)
    // pub fn is_boolean(&self) -> bool {
    //     self.input_validations
    //         .iter()
    //         .any(|v| matches!(v, InputValidation::Boolean))
    // }

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

    // #[test]
    // fn test_validate_boolean_input() {
    //     let form_input: FormInput = FormInput::new().boolean();
    //     assert!(form_input.is_valid("true"));
    //     assert!(form_input.is_valid("false"));
    // }
}
