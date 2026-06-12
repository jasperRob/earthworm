use crate::worktree::Worktree;

pub fn fetch_worktrees(repo_path: &str) -> color_eyre::Result<Vec<Worktree>> {
    let output = std::process::Command::new("git")
        .args(["-C", repo_path, "worktree", "list", "--porcelain"])
        .stderr(std::process::Stdio::null())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let mut worktrees = Vec::new();
    let mut current_path: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(path.to_string());
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            if let Some(path) = current_path.take() {
                worktrees.push(Worktree {
                    name: branch.to_string(),
                    path,
                })
            }
        }
    }

    Ok(worktrees)
}

pub fn create_worktree(
    repo_path: &str,
    worktree_name: &str,
    worktree_path: &str,
) -> color_eyre::Result<()> {
    // let worktree_path = format!("{}-{}", repo_path.trim_end_matches('/'), worktree_name);

    let branch_exists = std::process::Command::new("git")
        .args(["-C", repo_path, "rev-parse", "--verify", worktree_name])
        .status()?
        .success();

    let status = if branch_exists {
        std::process::Command::new("git")
            .args([
                "-C",
                repo_path,
                "worktree",
                "add",
                &worktree_path,
                worktree_name,
            ])
            .status()?
    } else {
        std::process::Command::new("git")
            .args([
                "-C",
                repo_path,
                "worktree",
                "add",
                "-b",
                worktree_name,
                &worktree_path,
            ])
            .status()?
    };
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("git worktree add failed"));
    }
    Ok(())
}

pub fn remove_worktree(repo_path: &str, worktree_path: &str) -> color_eyre::Result<()> {
    let status = std::process::Command::new("git")
        .args(["-C", repo_path, "worktree", "remove", &worktree_path])
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("git worktree remove failed"));
    }
    Ok(())
}
