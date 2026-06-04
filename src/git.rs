#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: String,
    pub name: String,
}

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
                    path,
                    name: branch.to_string(),
                })
            }
        }
    }

    Ok(worktrees)
}
