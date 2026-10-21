use strum::VariantNames;
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormInput},
        popups::FormPopup,
    },
    project::{Hooks, Project, SetupMode},
};

enum Field {
    Name = 0,
    Path = 1,
    Setup = 2,
    SetupMode = 3,
    Teardown = 4,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Path => "Path",
            Self::Setup => "Setup",
            Self::SetupMode => "Setup mode",
            Self::Teardown => "Teardown",
        }
    }
}

fn project_inputs(name: String, path: String, hooks: &Hooks) -> Vec<FormInput> {
    vec![
        FormInput::new()
            .label(Field::Name.label())
            .initial_value(name)
            .required(),
        FormInput::new()
            .label(Field::Path.label())
            .path()
            .initial_value(path)
            .required(),
        FormInput::new()
            .label(Field::Setup.label())
            .initial_value(hooks.setup.clone().unwrap_or_default()),
        FormInput::new()
            .label(Field::SetupMode.label())
            .choice(SetupMode::VARIANTS.iter().copied())
            .initial_value(hooks.setup_mode.to_string()),
        FormInput::new()
            .label(Field::Teardown.label())
            .initial_value(hooks.teardown.clone().unwrap_or_default()),
    ]
}

fn hooks_from(form: &Form) -> Hooks {
    let script = |field: Field| {
        let value = form.value(field as usize);
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    };
    Hooks {
        setup: script(Field::Setup),
        setup_mode: form
            .value(Field::SetupMode as usize)
            .parse()
            .unwrap_or_default(),
        teardown: script(Field::Teardown),
    }
}

pub fn new_project() -> FormPopup {
    FormPopup::new(
        Form::standard().title("New Project").inputs(project_inputs(
            String::default(),
            String::default(),
            &Hooks::default(),
        )),
        |form| {
            Action::SubmitProject(Project {
                id: Uuid::new_v4(),
                name: form.value(Field::Name as usize),
                path: form.value(Field::Path as usize),
                hooks: hooks_from(form),
                // TODO: should init based on existing worktrees for git repo
                worktrees: vec![],
            })
        },
    )
}

pub fn edit_project(project: Project) -> FormPopup {
    FormPopup::new(
        Form::standard()
            .title("Edit Project")
            .inputs(project_inputs(
                project.name.clone(),
                project.path.clone(),
                &project.hooks,
            )),
        move |form| {
            let mut project = project.clone();
            project.name = form.value(Field::Name as usize);
            project.path = form.value(Field::Path as usize);
            project.hooks = hooks_from(form);
            Action::UpdateProject(project)
        },
    )
}

pub fn remove_project(project: Project) -> FormPopup {
    FormPopup::new(
        Form::confirmation()
            .title("Remove Project")
            .body(format!("Remove \"{}\"?", project.name)),
        move |_| Action::RemoveProject(project.clone()),
    )
}
