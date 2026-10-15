use crate::{
    action::Action,
    components::{form::Form, popups::FormPopup},
    project::Project,
    worktree::Worktree,
};

pub fn remove_worktree(worktree: Worktree, project: Project) -> FormPopup {
    FormPopup::new(
        Form::confirmation()
            .title("Remove Worktree")
            .body(format!("Remove \"{}\"?", worktree.name)),
        // TODO: is there a better way of passing the originals here?
        move |_| Action::RemoveWorktree(project.clone(), worktree.clone()),
    )
}
