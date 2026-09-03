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

pub fn new_tmux_session(
    tmux_session_name: String,
    tmux_session_path: String,
) -> color_eyre::Result<()> {
    let mut cmd = std::process::Command::new("tmux");
    cmd.args([
        "new-session",
        "-d",
        "-s",
        &tmux_session_name,
        "-c",
        &tmux_session_path,
    ]);
    let status = cmd.status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux new-sesssion failed"));
    }
    Ok(())
}

pub fn kill_tmux_session(tmux_session_name: String) -> color_eyre::Result<()> {
    let status = std::process::Command::new("tmux")
        .args(["kill-session", "-t", &tmux_session_name])
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux kill-sesssion failed"));
    }
    Ok(())
}

pub fn rename_tmux_session(
    from_tmux_session_name: String,
    to_tmux_session_name: String,
) -> color_eyre::Result<()> {
    let status = std::process::Command::new("tmux")
        .args([
            "rename-session",
            "-t",
            &from_tmux_session_name,
            &to_tmux_session_name,
        ])
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux rename-sesssion failed"));
    }
    Ok(())
}

pub fn attach_tmux_session(tmux_session_name: String) -> color_eyre::Result<()> {
    let tmux_cmd = if std::env::var("TMUX").is_ok() {
        "switch-client"
    } else {
        "attach-session"
    };
    let status = std::process::Command::new("tmux")
        .args([tmux_cmd, "-t", &tmux_session_name])
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!(format!("tmux {} failed", tmux_cmd)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        project::Project,
        session::Session,
        tmux::{get_tmux_session_name, get_tmux_session_path},
        worktree::Worktree,
    };

    #[test]
    fn test_get_tmux_session_name_with_project() {
        let project = Project {
            id: Uuid::new_v4(),
            name: String::from("project-name"),
            path: String::new(),
            sessions: vec![],
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
            sessions: vec![],
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
}
