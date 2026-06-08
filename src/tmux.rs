use std::collections::HashMap;

use uuid::Uuid;

use crate::{project::Project, session::Session};

pub fn tmux_session_name(
    session: &Session,
    projects: &HashMap<Uuid, Project>,
) -> color_eyre::Result<String> {
    match session.project_id {
        None => Ok(session.name.clone()),
        Some(project_id) => match projects.get(&project_id) {
            None => Err(color_eyre::eyre::eyre!(
                "No project found for id: {}",
                project_id
            )),
            Some(project) => Ok(format!("{}_{}", project.name, session.name)),
        },
    }
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

pub fn attach_tmux_session(
    session: &Session,
    projects: &HashMap<Uuid, Project>,
) -> color_eyre::Result<()> {
    let session_name = tmux_session_name(session, projects)?;
    let tmux_cmd = if std::env::var("TMUX").is_ok() {
        "switch-client"
    } else {
        "attach-session"
    };
    let status = std::process::Command::new("tmux")
        .args([tmux_cmd, "-t", &session_name])
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!(format!("tmux {} failed", tmux_cmd)));
    }
    Ok(())
}

pub fn new_tmux_session(
    session: &Session,
    projects: &HashMap<Uuid, Project>,
    path: Option<&str>,
) -> color_eyre::Result<()> {
    let session_name = tmux_session_name(session, projects)?;
    let mut cmd = std::process::Command::new("tmux");
    cmd.args(["new-session", "-d", "-s", &session_name]);
    if let Some(p) = path {
        cmd.args(["-c", p]);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux new-sesssion failed"));
    }
    Ok(())
}

pub fn kill_tmux_session(
    session: &Session,
    projects: &HashMap<Uuid, Project>,
) -> color_eyre::Result<()> {
    let session_name = tmux_session_name(session, projects)?;
    let status = std::process::Command::new("tmux")
        .args(["kill-session", "-t", &session_name])
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(color_eyre::eyre::eyre!("tmux kill-sesssion failed"));
    }
    Ok(())
}
