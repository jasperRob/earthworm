use std::{
    io::Read,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use color_eyre::eyre::eyre;
use tracing::warn;

use crate::{
    project::Project,
    session::Session,
    tmux::{get_tmux_session_name, get_tmux_session_path},
};

/// How long a script run in the background may take before it is killed
pub const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How long to wait for a failed script's output, which processes it left running may hold open
const OUTPUT_WAIT: Duration = Duration::from_secs(1);

/// Lines of a failed script's output kept in its error
const OUTPUT_TAIL_LINES: usize = 3;

/// A project's setup or teardown script, ready to run for one of its sessions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hook {
    pub script: String,
    /// Directory the script runs in, which is the session's
    pub dir: String,
    pub env: Vec<(String, String)>,
    /// The session's tmux name, to say which session a failure was for
    pub tmux_session_name: String,
}

impl Hook {
    /// `script` set up to run for the session, or None if there is no script
    pub fn for_session(script: Option<&str>, session: &Session, project: &Project) -> Option<Self> {
        let script = script.map(str::trim).filter(|s| !s.is_empty())?;
        let dir = get_tmux_session_path(session, Some(project)).ok()?;
        let tmux_session_name = get_tmux_session_name(session, Some(project));
        Some(Self {
            script: script.to_string(),
            env: hook_env(session, project, &dir, &tmux_session_name),
            dir,
            tmux_session_name,
        })
    }

    /// Runs the script with `sh`, killing it and everything it started if it outlasts `timeout`.
    /// Its output is kept off the terminal, which the TUI owns.
    pub fn run(&self, timeout: Duration) -> color_eyre::Result<()> {
        let (mut reader, writer) = std::io::pipe()?;
        let mut child = {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", &self.script])
                .current_dir(&self.dir)
                .envs(self.env.iter().map(|(k, v)| (k, v)))
                .stdin(Stdio::null())
                .stdout(writer.try_clone()?)
                .stderr(writer)
                // Its own process group, so a timeout can kill everything the script started
                .process_group(0);
            cmd.spawn()
                .map_err(|e| eyre!("could not run in {}: {e}", self.dir))?
            // Dropping `cmd` closes this process's ends of the pipe, so the reader sees EOF
        };

        let (output_tx, output_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut output = Vec::new();
            let _ = reader.read_to_end(&mut output);
            let _ = output_tx.send(output);
        });

        let deadline = Instant::now() + timeout;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                kill_process_group(child.id());
                let _ = child.wait();
                return Err(eyre!("timed out after {}s", timeout.as_secs()));
            }
            thread::sleep(POLL_INTERVAL);
        };
        if status.success() {
            return Ok(());
        }

        let output = output_rx.recv_timeout(OUTPUT_WAIT).unwrap_or_default();
        let output = String::from_utf8_lossy(&output);
        warn!("`{}` failed with {status}:\n{output}", self.script);
        match output_tail(&output) {
            Some(tail) => Err(eyre!("{status}: {tail}")),
            None => Err(eyre!("{status}")),
        }
    }
}

/// Variables describing the session, set for its setup and teardown scripts
fn hook_env(
    session: &Session,
    project: &Project,
    dir: &str,
    tmux_session_name: &str,
) -> Vec<(String, String)> {
    let mut env = vec![
        ("EARTHWORM_PROJECT_NAME", project.name.clone()),
        ("EARTHWORM_PROJECT_PATH", project.path.clone()),
        ("EARTHWORM_SESSION_NAME", session.name.clone()),
        ("EARTHWORM_SESSION_PATH", dir.to_string()),
        ("EARTHWORM_TMUX_SESSION", tmux_session_name.to_string()),
    ];
    if let Some(worktree) = &session.worktree {
        env.push(("EARTHWORM_BRANCH", worktree.name.clone()));
    }
    env.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// The last few non-empty lines of a script's output, joined onto one line
fn output_tail(output: &str) -> Option<String> {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let tail = &lines[lines.len().saturating_sub(OUTPUT_TAIL_LINES)..];
    (!tail.is_empty()).then(|| tail.join(" / "))
}

fn kill_process_group(pid: u32) {
    // SAFETY: kill has no memory safety requirements. A negative pid targets the process group
    // the child leads, as it was spawned with process_group(0).
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    use uuid::Uuid;

    use super::{Hook, output_tail};
    use crate::{
        project::{Hooks, Project},
        session::Session,
        worktree::Worktree,
    };

    fn hook(script: &str, dir: &Path) -> Hook {
        Hook {
            script: String::from(script),
            dir: dir.to_string_lossy().into_owned(),
            env: vec![(String::from("EARTHWORM_TEST"), String::from("value"))],
            tmux_session_name: String::from("project_session"),
        }
    }

    fn project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: String::from("api"),
            path: String::from("/src/api"),
            hooks: Hooks::default(),
            worktrees: vec![],
        }
    }

    fn session(project: &Project) -> Session {
        Session {
            id: Uuid::new_v4(),
            project_id: Some(project.id),
            name: String::from("fix"),
            path: Some(String::from("/src/api-fix")),
            worktree: Some(Worktree {
                name: String::from("feat/fix"),
                path: String::from("/src/api-fix"),
            }),
        }
    }

    #[test]
    fn test_for_session_sets_dir_and_env() {
        let project = project();
        let session = session(&project);
        let hook = Hook::for_session(Some(" make setup "), &session, &project).unwrap();
        assert_eq!(hook.script, "make setup");
        assert_eq!(hook.dir, "/src/api-fix");
        assert_eq!(hook.tmux_session_name, "api_fix");
        let env: Vec<(&str, &str)> = hook
            .env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            env,
            [
                ("EARTHWORM_PROJECT_NAME", "api"),
                ("EARTHWORM_PROJECT_PATH", "/src/api"),
                ("EARTHWORM_SESSION_NAME", "fix"),
                ("EARTHWORM_SESSION_PATH", "/src/api-fix"),
                ("EARTHWORM_TMUX_SESSION", "api_fix"),
                ("EARTHWORM_BRANCH", "feat/fix"),
            ]
        );
    }

    #[test]
    fn test_for_session_without_worktree_has_no_branch() {
        let project = project();
        let mut session = session(&project);
        session.worktree = None;
        let hook = Hook::for_session(Some("true"), &session, &project).unwrap();
        assert!(!hook.env.iter().any(|(k, _)| k == "EARTHWORM_BRANCH"));
    }

    #[test]
    fn test_for_session_without_script_is_none() {
        let project = project();
        let session = session(&project);
        assert_eq!(Hook::for_session(None, &session, &project), None);
        assert_eq!(Hook::for_session(Some("  "), &session, &project), None);
    }

    #[test]
    fn test_run_uses_dir_and_env() {
        let dir = tempfile::tempdir().unwrap();
        hook(r#"printf '%s' "$EARTHWORM_TEST" > out"#, dir.path())
            .run(Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out")).unwrap(),
            "value"
        );
    }

    #[test]
    fn test_run_failure_reports_status_and_output_tail() {
        let dir = tempfile::tempdir().unwrap();
        let err = hook(
            "echo one; echo two; echo three >&2; echo four; exit 3",
            dir.path(),
        )
        .run(Duration::from_secs(5))
        .unwrap_err();
        assert_eq!(err.to_string(), "exit status: 3: two / three / four");
    }

    #[test]
    fn test_run_failure_without_output_reports_status() {
        let dir = tempfile::tempdir().unwrap();
        let err = hook("exit 1", dir.path())
            .run(Duration::from_secs(5))
            .unwrap_err();
        assert_eq!(err.to_string(), "exit status: 1");
    }

    #[test]
    fn test_run_in_missing_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let err = hook("true", &dir.path().join("missing"))
            .run(Duration::from_secs(5))
            .unwrap_err();
        assert!(err.to_string().starts_with("could not run in"));
    }

    #[test]
    fn test_run_success_does_not_wait_on_background_processes() {
        let dir = tempfile::tempdir().unwrap();
        let start = Instant::now();
        hook("sleep 5 &", dir.path())
            .run(Duration::from_secs(10))
            .unwrap();
        assert!(start.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn test_run_timeout_kills_script_and_its_children() {
        let dir = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let err = hook("(sleep 1; touch late) & sleep 5", dir.path())
            .run(Duration::from_millis(200))
            .unwrap_err();
        assert!(err.to_string().starts_with("timed out"));
        assert!(start.elapsed() < Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(1500));
        assert!(!dir.path().join("late").exists());
    }

    #[test]
    fn test_output_tail() {
        assert_eq!(output_tail(""), None);
        assert_eq!(output_tail("\n  \n"), None);
        assert_eq!(output_tail("a\n\nb\n"), Some(String::from("a / b")));
        assert_eq!(output_tail("a\nb\nc\nd"), Some(String::from("b / c / d")));
    }
}
