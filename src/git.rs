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
    Ok(parse_worktree_porcelain(&stdout))
}

/// Parses `git worktree list --porcelain` output. Worktrees without a
/// `branch` line (detached HEAD, bare) are skipped.
fn parse_worktree_porcelain(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<String> = None;

    for line in output.lines() {
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

    worktrees
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;

    use crate::git::{create_worktree, fetch_worktrees, parse_worktree_porcelain, remove_worktree};
    use crate::worktree::Worktree;

    fn init_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let repo = root.join("repo");
        let git = |args: &[&str]| {
            assert!(Command::new("git").args(args).status().unwrap().success());
        };
        git(&["init", "-q", repo.to_str().unwrap()]);
        git(&[
            "-C",
            repo.to_str().unwrap(),
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ]);
        (dir, root)
    }

    fn wt(name: &str, path: &str) -> Worktree {
        Worktree {
            name: name.to_string(),
            path: path.to_string(),
        }
    }

    #[test]
    fn test_parse_porcelain_main_and_linked() {
        let output = "\
worktree /repo
HEAD 1111111111111111111111111111111111111111
branch refs/heads/main

worktree /wt/feat
HEAD 2222222222222222222222222222222222222222
branch refs/heads/feat

";
        assert_eq!(
            parse_worktree_porcelain(output),
            vec![wt("main", "/repo"), wt("feat", "/wt/feat")]
        );
    }

    #[test]
    fn test_parse_porcelain_branch_with_slash() {
        let output = "\
worktree /wt/x
HEAD 1111111111111111111111111111111111111111
branch refs/heads/feat/x

";
        assert_eq!(
            parse_worktree_porcelain(output),
            vec![wt("feat/x", "/wt/x")]
        );
    }

    #[test]
    fn test_parse_porcelain_skips_detached_without_stealing_path() {
        let output = "\
worktree /wt/detached
HEAD 1111111111111111111111111111111111111111
detached

worktree /wt/feat
HEAD 2222222222222222222222222222222222222222
branch refs/heads/feat

";
        assert_eq!(
            parse_worktree_porcelain(output),
            vec![wt("feat", "/wt/feat")]
        );
    }

    #[test]
    fn test_parse_porcelain_skips_bare() {
        let output = "\
worktree /repo.git
bare

worktree /wt/feat
HEAD 2222222222222222222222222222222222222222
branch refs/heads/feat
locked reason why is locked

";
        assert_eq!(
            parse_worktree_porcelain(output),
            vec![wt("feat", "/wt/feat")]
        );
    }

    #[test]
    fn test_parse_porcelain_empty() {
        assert!(parse_worktree_porcelain("").is_empty());
    }

    #[test]
    fn test_parse_porcelain_path_with_spaces() {
        let output = "\
worktree /path/my worktree
HEAD 1111111111111111111111111111111111111111
branch refs/heads/feat

";
        assert_eq!(
            parse_worktree_porcelain(output),
            vec![wt("feat", "/path/my worktree")]
        );
    }

    #[test]
    fn test_create_worktree_for_existing_branch() {
        let (_dir, root) = init_repo();
        let repo = root.join("repo");
        let wt_path = root.join("existing");
        let (repo, wt_path) = (repo.to_str().unwrap(), wt_path.to_str().unwrap());

        assert!(
            Command::new("git")
                .args(["-C", repo, "branch", "existing"])
                .status()
                .unwrap()
                .success()
        );

        create_worktree(repo, "existing", wt_path).unwrap();
        assert!(
            fetch_worktrees(repo)
                .unwrap()
                .contains(&wt("existing", wt_path))
        );
    }

    #[test]
    fn test_remove_worktree_removes_it_from_list() {
        let (_dir, root) = init_repo();
        let repo = root.join("repo");
        let wt = root.join("feat");
        let (repo, wt) = (repo.to_str().unwrap(), wt.to_str().unwrap());

        create_worktree(repo, "feat", wt).unwrap();
        assert!(fetch_worktrees(repo).unwrap().iter().any(|w| w.path == wt));

        remove_worktree(repo, wt).unwrap();
        assert!(!fetch_worktrees(repo).unwrap().iter().any(|w| w.path == wt));
    }

    #[test]
    fn test_remove_worktree_nonexistent_returns_error() {
        let (_dir, root) = init_repo();
        let repo = root.join("repo");
        assert!(remove_worktree(repo.to_str().unwrap(), "/does/not/exist").is_err());
    }

    #[test]
    fn test_remove_worktree_with_untracked_files_returns_error() {
        let (_dir, root) = init_repo();
        let repo = root.join("repo");
        let wt = root.join("feat");

        create_worktree(repo.to_str().unwrap(), "feat", wt.to_str().unwrap()).unwrap();
        std::fs::write(wt.join("dirty.txt"), "x").unwrap();
        assert!(remove_worktree(repo.to_str().unwrap(), wt.to_str().unwrap()).is_err());
    }
}
