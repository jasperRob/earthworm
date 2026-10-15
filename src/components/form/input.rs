use std::{
    env,
    path::{Component, PathBuf},
};

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
            // A single stat; cheap enough to run on every draw
            InputValidation::Path => !value.is_empty() && normalise_path(value).is_dir(),
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

/// Expands a leading `~` or `~/` using `$HOME`. Other values are returned unchanged.
pub fn expand_home(value: &str) -> PathBuf {
    if let Some(home) = env::var_os("HOME") {
        if value == "~" {
            return PathBuf::from(home);
        }
        if let Some(rest) = value.strip_prefix("~/") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}

/// Expands `~`, makes the path absolute relative to the cwd and lexically removes `.` and
/// `..` components. Symlinks are not resolved and the path does not need to exist.
pub fn normalise_path(value: &str) -> PathBuf {
    let expanded = expand_home(value);
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        env::current_dir().unwrap_or_default().join(expanded)
    };
    let mut cleaned = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                cleaned.pop();
            }
            other => cleaned.push(other),
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn test_validate_required_input() {
        let form_input: FormInput = FormInput::new().required();
        assert!(form_input.is_valid("hello world"));
        assert!(!form_input.is_valid(""));
    }

    #[test]
    fn test_expand_home() {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/repo"), home.join("repo"));
        assert_eq!(expand_home("~other/repo"), PathBuf::from("~other/repo"));
        assert_eq!(expand_home("/a/~/b"), PathBuf::from("/a/~/b"));
    }

    #[test]
    fn test_normalise_absolute_path() {
        assert_eq!(normalise_path("/a/./b/../c/"), PathBuf::from("/a/c"));
        assert_eq!(normalise_path("/.."), PathBuf::from("/"));
    }

    #[test]
    fn test_normalise_relative_path() {
        let cwd = env::current_dir().unwrap();
        assert_eq!(normalise_path("./foo"), cwd.join("foo"));
        assert_eq!(normalise_path("foo/../bar"), cwd.join("bar"));
    }

    #[test]
    fn test_normalise_home_path() {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        assert_eq!(normalise_path("~/a/../b"), home.join("b"));
    }

    #[test]
    fn test_validate_path_input() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        fs::write(&file, "").unwrap();
        let form_input: FormInput = FormInput::new().path();
        assert!(form_input.is_valid(dir.path().to_str().unwrap()));
        assert!(form_input.is_valid(&format!("{}/./", dir.path().display())));
        assert!(!form_input.is_valid(dir.path().join("missing").to_str().unwrap()));
        assert!(!form_input.is_valid(file.to_str().unwrap()));
        assert!(!form_input.is_valid(""));
    }
}
