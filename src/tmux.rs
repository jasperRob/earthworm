use crate::{project::Project, session::Session};

pub fn get_tmux_session_name(session: &Session, project: Option<&Project>) -> String {
    match project {
        None => session.name.clone(),
        Some(p) => format!("{}_{}", p.name, session.name),
    }
}

pub fn get_tmux_session_path(
    session: &Session,
    project: Option<&Project>,
) -> color_eyre::Result<String> {
    let path = [
        session.worktree.as_ref().map(|w| w.path.as_str()),
        session.path.as_deref(),
        project.map(|p| p.path.as_str()),
    ]
    .into_iter()
    .flatten()
    .find(|s| !s.is_empty())
    .ok_or_else(|| color_eyre::eyre::eyre!("no path found for session"))?;
    Ok(String::from(path))
}

// tmux resolves `-t` session targets by exact name, then prefix, then glob.
// A leading `=` restricts it to an exact match, so a missing `api` can't
// resolve to `api-v2`.
fn exact_target(tmux_session_name: &str) -> String {
    format!("={tmux_session_name}")
}

// A pane target needs the trailing `:`, as `=name` alone is not a valid pane.
fn exact_pane_target(tmux_session_name: &str) -> String {
    format!("{}:", exact_target(tmux_session_name))
}

fn new_session_args(
    tmux_session_name: &str,
    tmux_session_path: &str,
    env: &[(String, String)],
) -> Vec<String> {
    let mut args = vec![
        String::from("new-session"),
        String::from("-d"),
        String::from("-s"),
        String::from(tmux_session_name),
        String::from("-c"),
        String::from(tmux_session_path),
    ];
    for (key, value) in env {
        args.push(String::from("-e"));
        args.push(format!("{key}={value}"));
    }
    args
}

// `-l` sends the text literally, so words like `Enter` aren't read as key names
fn send_keys_args(tmux_session_name: &str, text: &str) -> Vec<String> {
    vec![
        String::from("send-keys"),
        String::from("-t"),
        exact_pane_target(tmux_session_name),
        String::from("-l"),
        String::from(text),
    ]
}

fn send_enter_args(tmux_session_name: &str) -> Vec<String> {
    vec![
        String::from("send-keys"),
        String::from("-t"),
        exact_pane_target(tmux_session_name),
        String::from("Enter"),
    ]
}

fn kill_session_args(tmux_session_name: &str) -> Vec<String> {
    vec![
        String::from("kill-session"),
        String::from("-t"),
        exact_target(tmux_session_name),
    ]
}

fn rename_session_args(from_tmux_session_name: &str, to_tmux_session_name: &str) -> Vec<String> {
    vec![
        String::from("rename-session"),
        String::from("-t"),
        exact_target(from_tmux_session_name),
        String::from(to_tmux_session_name),
    ]
}

fn attach_session_args(tmux_session_name: &str, inside_tmux: bool) -> Vec<String> {
    let tmux_cmd = if inside_tmux {
        "switch-client"
    } else {
        "attach-session"
    };
    vec![
        String::from(tmux_cmd),
        String::from("-t"),
        exact_target(tmux_session_name),
    ]
}

pub fn fetch_tmux_sessions() -> color_eyre::Result<Vec<String>> {
    let output = std::process::Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()?;

    if !output.status.success() {
        // tmux returns exit code 1 when no server is running
        return Ok(Vec::new());
    }

    let sessions = String::from_utf8(output.stdout)?
        .lines()
        .map(|s| s.to_string())
        .collect();

    Ok(sessions)
}

/// Creates a detached session, with `env` set in the environment of its shells
pub fn new_tmux_session(
    tmux_session_name: String,
    tmux_session_path: String,
    env: &[(String, String)],
) -> color_eyre::Result<()> {
    let status = std::process::Command::new("tmux")
        .args(new_session_args(
            &tmux_session_name,
            &tmux_session_path,
            env,
        ))
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux new-session failed"));
    }
    Ok(())
}

/// Types `command` into the session's active pane and presses Enter
pub fn run_in_tmux_session(tmux_session_name: &str, command: &str) -> color_eyre::Result<()> {
    for args in [
        send_keys_args(tmux_session_name, command),
        send_enter_args(tmux_session_name),
    ] {
        let status = std::process::Command::new("tmux")
            .args(args)
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(color_eyre::eyre::eyre!("tmux send-keys failed"));
        }
    }
    Ok(())
}

pub fn kill_tmux_session(tmux_session_name: String) -> color_eyre::Result<()> {
    let status = std::process::Command::new("tmux")
        .args(kill_session_args(&tmux_session_name))
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux kill-session failed"));
    }
    Ok(())
}

pub fn rename_tmux_session(
    from_tmux_session_name: String,
    to_tmux_session_name: String,
) -> color_eyre::Result<()> {
    let status = std::process::Command::new("tmux")
        .args(rename_session_args(
            &from_tmux_session_name,
            &to_tmux_session_name,
        ))
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux rename-session failed"));
    }
    Ok(())
}

pub fn attach_tmux_session(tmux_session_name: String) -> color_eyre::Result<()> {
    let args = attach_session_args(&tmux_session_name, std::env::var("TMUX").is_ok());
    let status = std::process::Command::new("tmux").args(&args).status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!(format!("tmux {} failed", args[0])));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        project::{Hooks, Project},
        session::Session,
        tmux::{
            attach_session_args, exact_pane_target, exact_target, get_tmux_session_name,
            get_tmux_session_path, kill_session_args, new_session_args, rename_session_args,
            send_enter_args, send_keys_args,
        },
        worktree::Worktree,
    };

    #[test]
    fn test_get_tmux_session_name_with_project() {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::from("project-name"),
            path: String::new(),
            hooks: Hooks::default(),
            worktrees: vec![],
        };
        let session = Session {
            id: Uuid::new_v4(),
            project_id: None,
            name: String::from("session-name"),
            path: None,
            worktree: None,
        };
        assert_eq!(
            get_tmux_session_name(&session, Some(&project)),
            "project-name_session-name"
        )
    }
    #[test]
    fn test_get_tmux_session_name_without_project() {
        let session = Session {
            id: Uuid::new_v4(),
            project_id: None,
            name: String::from("session-name"),
            path: None,
            worktree: None,
        };
        assert_eq!(get_tmux_session_name(&session, None), "session-name")
    }
    #[test]
    fn test_get_tmux_session_path_priority_order() {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::new(),
            path: String::from("/project-path"),
            hooks: Hooks::default(),
            worktrees: vec![],
        };
        let mut session = Session {
            id: Uuid::new_v4(),
            project_id: None,
            name: String::new(),
            path: Some(String::from("/session-path")),
            worktree: Some(Worktree {
                name: String::new(),
                path: String::from("/worktree-path"),
            }),
        };
        assert_eq!(
            get_tmux_session_path(&session, Some(&project)).unwrap(),
            "/worktree-path"
        );
        session.worktree = None;
        assert_eq!(
            get_tmux_session_path(&session, Some(&project)).unwrap(),
            "/session-path"
        );
        session.path = None;
        assert_eq!(
            get_tmux_session_path(&session, Some(&project)).unwrap(),
            "/project-path"
        );
    }
    #[test]
    fn test_get_tmux_session_path_no_path_returns_error() {
        let session = Session {
            id: Uuid::new_v4(),
            project_id: None,
            name: String::new(),
            path: None,
            worktree: None,
        };
        assert!(get_tmux_session_path(&session, None).is_err());
    }
    #[test]
    fn test_exact_target_prefixes_equals() {
        assert_eq!(exact_target("session-name"), "=session-name");
    }
    #[test]
    fn test_exact_target_keeps_glob_characters_literal() {
        assert_eq!(exact_target("api*"), "=api*");
        assert_eq!(exact_target("api[v2]"), "=api[v2]");
    }
    #[test]
    fn test_exact_target_does_not_widen_prefix_like_names() {
        assert_eq!(exact_target("api"), "=api");
        assert_ne!(exact_target("api"), exact_target("api-v2"));
    }
    #[test]
    fn test_kill_session_args() {
        assert_eq!(kill_session_args("api"), ["kill-session", "-t", "=api"]);
    }
    #[test]
    fn test_rename_session_args() {
        assert_eq!(
            rename_session_args("api", "api-v2"),
            ["rename-session", "-t", "=api", "api-v2"]
        );
    }
    #[test]
    fn test_attach_session_args() {
        assert_eq!(
            attach_session_args("api", false),
            ["attach-session", "-t", "=api"]
        );
        assert_eq!(
            attach_session_args("api", true),
            ["switch-client", "-t", "=api"]
        );
    }
    #[test]
    fn test_exact_pane_target_ends_with_colon() {
        assert_eq!(exact_pane_target("api"), "=api:");
    }
    #[test]
    fn test_new_session_args() {
        assert_eq!(
            new_session_args("api", "/src/api", &[]),
            ["new-session", "-d", "-s", "api", "-c", "/src/api"]
        );
    }
    #[test]
    fn test_new_session_args_with_env() {
        let env = [
            (String::from("A"), String::from("1")),
            (String::from("B"), String::from("x=y z")),
        ];
        assert_eq!(
            new_session_args("api", "/src/api", &env),
            [
                "new-session",
                "-d",
                "-s",
                "api",
                "-c",
                "/src/api",
                "-e",
                "A=1",
                "-e",
                "B=x=y z"
            ]
        );
    }
    #[test]
    fn test_send_keys_args_sends_text_literally() {
        assert_eq!(
            send_keys_args("api", "make setup; echo Enter"),
            ["send-keys", "-t", "=api:", "-l", "make setup; echo Enter"]
        );
    }
    #[test]
    fn test_send_enter_args() {
        assert_eq!(
            send_enter_args("api"),
            ["send-keys", "-t", "=api:", "Enter"]
        );
    }
}
