use std::process::{Command, Stdio};

use color_eyre::eyre::eyre;
use tracing::warn;

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

const REMOTE: &str = "origin";

/// Creates a worktree for the branch, preferring the branch on origin, then a local branch, and
/// otherwise creating a new branch from HEAD.
pub fn create_worktree(
    repo_path: &str,
    worktree_name: &str,
    worktree_path: &str,
) -> color_eyre::Result<()> {
    let remote_ref = format!("refs/remotes/{REMOTE}/{worktree_name}");
    let on_remote = fetch_remote_branch(repo_path, worktree_name, &remote_ref)?;
    let local_exists = ref_exists(repo_path, &format!("refs/heads/{worktree_name}"))?;

    let args: Vec<&str> = match (local_exists, on_remote) {
        (true, _) => vec!["worktree", "add", worktree_path, worktree_name],
        (false, true) => vec![
            "worktree",
            "add",
            "--track",
            "-b",
            worktree_name,
            worktree_path,
            &remote_ref,
        ],
        (false, false) => vec!["worktree", "add", "-b", worktree_name, worktree_path],
    };
    run_git(repo_path, &args).map_err(|e| eyre!("Failed to add worktree: {e}"))?;

    if local_exists && on_remote {
        fast_forward(worktree_path, worktree_name, &remote_ref)?;
    }
    Ok(())
}

/// Fetches the branch from origin, returning whether origin has it. If origin is unreachable,
/// the last fetched state of the branch is used instead.
fn fetch_remote_branch(
    repo_path: &str,
    branch: &str,
    remote_ref: &str,
) -> color_eyre::Result<bool> {
    if run_git(repo_path, &["remote", "get-url", REMOTE]).is_err() {
        return Ok(false);
    }
    let output = git_command(repo_path)
        .args([
            "ls-remote",
            "--exit-code",
            "--heads",
            REMOTE,
            &format!("refs/heads/{branch}"),
        ])
        .output()?;
    match output.status.code() {
        Some(0) => {
            run_git(
                repo_path,
                &[
                    "fetch",
                    REMOTE,
                    &format!("+refs/heads/{branch}:{remote_ref}"),
                ],
            )
            .map_err(|e| eyre!("Failed to fetch {branch} from {REMOTE}: {e}"))?;
            Ok(true)
        }
        // --exit-code reports a missing ref with 2
        Some(2) => Ok(false),
        _ => {
            warn!(
                "Could not reach {REMOTE}, using last fetched state of {branch}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            ref_exists(repo_path, remote_ref)
        }
    }
}

/// Fast-forwards the branch checked out in the worktree to the remote branch, leaving it as is if
/// it has commits the remote branch doesn't.
fn fast_forward(worktree_path: &str, branch: &str, remote_ref: &str) -> color_eyre::Result<()> {
    let is_behind = git_command(worktree_path)
        .args(["merge-base", "--is-ancestor", "HEAD", remote_ref])
        .status()?
        .success();
    if !is_behind {
        warn!("Local {branch} has commits not on {REMOTE}, so it was not updated");
        return Ok(());
    }
    run_git(worktree_path, &["merge", "--ff-only", remote_ref])
        .map_err(|e| eyre!("Failed to update {branch} from {REMOTE}: {e}"))?;
    Ok(())
}

fn ref_exists(repo_path: &str, full_ref: &str) -> color_eyre::Result<bool> {
    Ok(git_command(repo_path)
        .args(["show-ref", "--verify", "--quiet", full_ref])
        .status()?
        .success())
}

/// A git command that never waits on a credential prompt, as the TUI owns the terminal
fn git_command(dir: &str) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["-C", dir])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    cmd
}

/// Runs git, returning its stderr as the error if it fails
fn run_git(dir: &str, args: &[&str]) -> color_eyre::Result<()> {
    let output = git_command(dir).args(args).output()?;
    if !output.status.success() {
        return Err(eyre!("{}", String::from_utf8_lossy(&output.stderr).trim()));
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
    use std::path::{Path, PathBuf};
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

    /// Runs git in `dir`, returning trimmed stdout
    fn git_in(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-C"])
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn commit(dir: &Path, message: &str) -> String {
        git_in(dir, &["commit", "-q", "--allow-empty", "-m", message]);
        git_in(dir, &["rev-parse", "HEAD"])
    }

    /// A bare `origin`, a `seed` clone used to push to it, and a `repo` clone under test
    fn init_with_origin() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (origin, seed, repo) = (
            root.join("origin.git"),
            root.join("seed"),
            root.join("repo"),
        );
        git_in(&root, &["init", "-q", "--bare", "-b", "main", "origin.git"]);
        git_in(&root, &["clone", "-q", origin.to_str().unwrap(), "seed"]);
        commit(&seed, "init");
        git_in(&seed, &["push", "-q", "origin", "HEAD:main"]);
        git_in(&root, &["clone", "-q", origin.to_str().unwrap(), "repo"]);
        (dir, root, seed, repo)
    }

    fn upstream(dir: &Path) -> Option<String> {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--abbrev-ref", "@{upstream}"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    #[test]
    fn test_create_worktree_tracks_branch_only_on_origin() {
        let (_dir, root, seed, repo) = init_with_origin();
        // Pushed after the clone, so it is only found by fetching
        git_in(&seed, &["checkout", "-q", "-b", "feat"]);
        let remote_head = commit(&seed, "feat work");
        git_in(&seed, &["push", "-q", "origin", "feat"]);

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["rev-parse", "HEAD"]), remote_head);
        assert_eq!(upstream(&wt_path).as_deref(), Some("origin/feat"));
    }

    #[test]
    fn test_create_worktree_fast_forwards_local_branch_behind_origin() {
        let (_dir, root, seed, repo) = init_with_origin();
        git_in(&seed, &["checkout", "-q", "-b", "feat"]);
        commit(&seed, "first");
        git_in(&seed, &["push", "-q", "origin", "feat"]);
        git_in(&repo, &["fetch", "-q", "origin"]);
        git_in(&repo, &["branch", "-q", "feat", "origin/feat"]);
        let remote_head = commit(&seed, "second");
        git_in(&seed, &["push", "-q", "origin", "feat"]);

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["rev-parse", "HEAD"]), remote_head);
    }

    #[test]
    fn test_create_worktree_keeps_local_commits_not_on_origin() {
        let (_dir, root, seed, repo) = init_with_origin();
        git_in(&seed, &["checkout", "-q", "-b", "feat"]);
        commit(&seed, "remote work");
        git_in(&seed, &["push", "-q", "origin", "feat"]);
        git_in(&repo, &["checkout", "-q", "-b", "feat"]);
        let local_head = commit(&repo, "local work");
        git_in(&repo, &["checkout", "-q", "main"]);

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["rev-parse", "HEAD"]), local_head);
    }

    #[test]
    fn test_create_worktree_uses_local_branch_not_on_origin() {
        let (_dir, root, _seed, repo) = init_with_origin();
        git_in(&repo, &["checkout", "-q", "-b", "feat"]);
        let local_head = commit(&repo, "local work");
        git_in(&repo, &["checkout", "-q", "main"]);

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["rev-parse", "HEAD"]), local_head);
        assert_eq!(upstream(&wt_path), None);
    }

    #[test]
    fn test_create_worktree_creates_missing_branch_from_head() {
        let (_dir, root, _seed, repo) = init_with_origin();
        let head = git_in(&repo, &["rev-parse", "HEAD"]);

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["rev-parse", "HEAD"]), head);
        assert_eq!(git_in(&wt_path, &["branch", "--show-current"]), "feat");
        assert_eq!(upstream(&wt_path), None);
    }

    #[test]
    fn test_create_worktree_without_origin_creates_branch() {
        let (_dir, root) = init_repo();
        let (repo, wt_path) = (root.join("repo"), root.join("wt"));

        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["branch", "--show-current"]), "feat");
    }

    #[test]
    fn test_create_worktree_uses_last_fetched_branch_when_origin_unreachable() {
        let (_dir, root, seed, repo) = init_with_origin();
        git_in(&seed, &["checkout", "-q", "-b", "feat"]);
        let remote_head = commit(&seed, "feat work");
        git_in(&seed, &["push", "-q", "origin", "feat"]);
        git_in(&repo, &["fetch", "-q", "origin"]);
        let missing = root.join("missing.git");
        git_in(
            &repo,
            &["remote", "set-url", "origin", missing.to_str().unwrap()],
        );

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        assert_eq!(git_in(&wt_path, &["rev-parse", "HEAD"]), remote_head);
        assert_eq!(upstream(&wt_path).as_deref(), Some("origin/feat"));
    }

    #[test]
    fn test_create_worktree_ignores_tag_with_branch_name() {
        let (_dir, root, _seed, repo) = init_with_origin();
        git_in(&repo, &["tag", "feat"]);

        let wt_path = root.join("wt");
        create_worktree(repo.to_str().unwrap(), "feat", wt_path.to_str().unwrap()).unwrap();

        // A tag is not a branch, so a branch is created rather than a detached worktree
        assert_eq!(git_in(&wt_path, &["branch", "--show-current"]), "feat");
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
