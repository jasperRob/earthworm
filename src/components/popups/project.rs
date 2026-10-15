use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        form::{Form, FormInput},
        popups::FormPopup,
    },
    project::Project,
};

enum Field {
    Name = 0,
    Path = 1,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Path => "Path",
        }
    }
}

pub fn new_project() -> FormPopup {
    FormPopup::new(
        Form::standard().title("New Project").inputs(vec![
            FormInput::new()
                .label(Field::Name.label())
                .initial_value(String::default())
                .required(),
            FormInput::new()
                .label(Field::Path.label())
                .path()
                .initial_value(String::default())
                .required(),
        ]),
        |form| {
            Action::SubmitProject(Project {
                id: Uuid::new_v4(),
                name: form.value(Field::Name as usize),
                path: form.value(Field::Path as usize),
                sessions: vec![],
                // TODO: should init based on existing worktrees for git repo
                worktrees: vec![],
            })
        },
    )
}

pub fn edit_project(project: Project) -> FormPopup {
    FormPopup::new(
        Form::standard().title("Edit Project").inputs(vec![
            FormInput::new()
                .label(Field::Name.label())
                .initial_value(project.name.clone())
                .required(),
            FormInput::new()
                .label(Field::Path.label())
                .path()
                .initial_value(project.path.clone())
                .required(),
        ]),
        move |form| {
            let mut project = project.clone();
            project.name = form.value(Field::Name as usize);
            project.path = form.value(Field::Path as usize);
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
