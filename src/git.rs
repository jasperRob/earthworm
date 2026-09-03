use crate::worktree::Worktree;

pub fn fetch_worktrees(repo_path: &str) -> color_eyre::Result<Vec<Worktree>> {
    let output = std::process::Command::new("git")
        .args(["-C", repo_path, "worktree", "list", "--porcelain"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to fetch worktrees: {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut worktrees = Vec::new();
    let mut current_path: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(path.to_string());
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/")
            && let Some(path) = current_path.take()
        {
            worktrees.push(Worktree {
                name: branch.to_string(),
                path,
            })
        }
    }

    Ok(worktrees)
}

pub fn create_worktree(
    repo_path: &str,
    worktree_name: &str,
    worktree_path: &str,
) -> color_eyre::Result<()> {
    let branch_exists = std::process::Command::new("git")
        .args(["-C", repo_path, "rev-parse", "--verify", worktree_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?
        .success();

    let output = if branch_exists {
        std::process::Command::new("git")
            .args([
                "-C",
                repo_path,
                "worktree",
                "add",
                worktree_path,
                worktree_name,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()?
    } else {
        std::process::Command::new("git")
            .args([
                "-C",
                repo_path,
                "worktree",
                "add",
                "-b",
                worktree_name,
                worktree_path,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()?
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to add worktree: {}",
            stderr.trim()
        ));
    }
    Ok(())
}

pub fn remove_worktree(repo_path: &str, worktree_path: &str) -> color_eyre::Result<()> {
    let output = std::process::Command::new("git")
        .args(["-C", repo_path, "worktree", "remove", worktree_path])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to remove worktree: {}",
            stderr.trim()
        ));
    }
    Ok(())
}
