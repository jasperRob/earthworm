use std::collections::HashMap;

use color_eyre::eyre::eyre;
use crossterm::event::KeyEvent;
use keys::KeySequence;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{Component, home::Home, notification::Notification},
    config::Config,
    pending::{JobId, Pending},
    project::Project,
    session::Session,
    state::{AppState, backup_state, load_state, save_state},
    tui::{Event, Tui},
};

mod handlers;
mod jobs;
mod keys;

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    components: Vec<Box<dyn Component>>,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    key_sequence: KeySequence,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    projects: HashMap<Uuid, Project>,
    sessions: HashMap<Uuid, Session>,
    pending: HashMap<JobId, Pending>,
    next_job_id: JobId,
    persist_disabled: bool,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> color_eyre::Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![Box::new(Home::new()), Box::new(Notification::default())],
            should_quit: false,
            should_suspend: false,
            config: Config::new()?,
            mode: Mode::Home,
            key_sequence: KeySequence::default(),
            action_tx,
            action_rx,
            projects: HashMap::default(),
            sessions: HashMap::default(),
            pending: HashMap::default(),
            next_job_id: JobId::default(),
            persist_disabled: false,
        })
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = Tui::new()?
            // .mouse(true) // uncomment this line to enable mouse support
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(self.action_tx.clone())?;
        }
        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }
        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        self.load_persisted_state();
        self.broadcast_state()?;
        self.start_tmux_sync()?;

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui)?;
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => self.handle_key_event(key)?,
            _ => {}
        }
        for component in self.components.iter_mut() {
            if let Some(action) = component.handle_events(Some(event.clone()))? {
                action_tx.send(action)?;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        let capturing = self.components.iter().any(|c| c.is_capturing_input());
        if capturing {
            return Ok(());
        }
        let Some(keymap) = self.config.keybindings.0.get(&self.mode) else {
            return Ok(());
        };
        if let Some(action) = self.key_sequence.feed(key, keymap) {
            info!("Got action: {action:?}");
            self.action_tx.send(action)?;
        }
        Ok(())
    }

    fn handle_actions(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }
            let outcome = match action {
                Action::Tick => {
                    // self.key_sequence = KeySequence::default();
                    Ok(())
                }
                Action::Quit => {
                    self.should_quit = true;
                    Ok(())
                }
                Action::Suspend => {
                    self.should_suspend = true;
                    Ok(())
                }
                Action::Resume => {
                    self.should_suspend = false;
                    Ok(())
                }
                Action::ClearScreen => {
                    tui.terminal.clear()?;
                    Ok(())
                }
                Action::Resize(w, h) => {
                    self.handle_resize(tui, w, h)?;
                    Ok(())
                }
                Action::Render => {
                    self.render(tui)?;
                    Ok(())
                }
                Action::SubmitProject(ref project) => self.on_submit_project(project),
                Action::UpdateProject(ref project) => self.on_update_project(project),
                Action::RemoveProject(ref project) => self.on_remove_project(project),
                Action::SubmitSession(ref session) => self.on_submit_session(session),
                Action::UpdateSession(ref session) => self.on_update_session(session),
                Action::AttachSession(ref session) => {
                    tui.exit()?;
                    let result = self.on_attach_session(session);
                    tui.enter()?;
                    if result.is_ok() {
                        self.action_tx.send(Action::ClearScreen)?;
                        self.action_tx.send(Action::Render)?;
                    }
                    result
                }
                Action::RemoveSession(ref session) => self.on_remove_session(session),
                Action::RemoveWorktree(ref project, ref worktree) => {
                    self.on_remove_worktree(project, worktree)
                }
                Action::JobFinished(id, ref completion) => {
                    self.pending.remove(&id);
                    // Handled in place, so the job's outcome and its end are broadcast together
                    let outcome = self.handle_job_completion(completion);
                    self.broadcast_state()?;
                    outcome
                }
                _ => Ok(()),
            };
            if let Err(e) = outcome {
                self.dispatch_error(e);
            }
            for component in self.components.iter_mut() {
                if let Some(action) = component.update(action.clone())? {
                    self.action_tx.send(action)?
                };
            }
        }
        Ok(())
    }

    fn handle_job_completion(&mut self, action: &Action) -> color_eyre::Result<()> {
        match *action {
            Action::ProjectCreationFinished {
                ref project,
                ref result,
            } => self.on_project_creation_finished(project, result),
            Action::ProjectUpdateFinished {
                ref project,
                ref result,
            } => self.on_project_update_finished(project, result),
            Action::ProjectRemovalFinished {
                project_id,
                ref result,
            } => self.on_project_removal_finished(project_id, result),
            Action::SessionCreationFinished {
                ref session,
                ref result,
            } => self.on_session_creation_finished(session, result),
            Action::SessionUpdateFinished {
                ref session,
                ref result,
            } => self.on_session_update_finished(session, result),
            Action::SessionRemovalFinished {
                ref session,
                ref result,
            } => self.on_session_removal_finished(session, result),
            Action::WorktreeRemovalFinished {
                project_id,
                ref result,
            } => self.on_worktree_removal_finished(project_id, result),
            Action::TmuxSyncFinished {
                ref known_session_ids,
                ref result,
            } => self.on_tmux_sync_finished(known_session_ids, result),
            _ => Err(eyre!("Unexpected job completion: {action}")),
        }
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> color_eyre::Result<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.render(tui)?;
        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        tui.draw(|frame| {
            for component in self.components.iter_mut() {
                if let Err(err) = component.draw(frame, frame.area()) {
                    let _ = self
                        .action_tx
                        .send(Action::Error(format!("Failed to draw: {:?}", err)));
                }
            }
        })?;
        Ok(())
    }

    fn get_session_project(&self, session: &Session) -> color_eyre::Result<Option<&Project>> {
        if let Some(project_id) = session.project_id {
            self.projects
                .get(&project_id)
                .ok_or_else(|| eyre!("no project found for given project_id"))
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn broadcast_state(&self) -> color_eyre::Result<()> {
        self.action_tx.send(Action::StateUpdated(
            self.projects.clone(),
            self.sessions.clone(),
            self.pending.values().cloned().collect(),
        ))?;
        Ok(())
    }

    fn persist_state(&mut self) {
        if self.persist_disabled {
            return;
        }
        let state = AppState {
            projects: self.projects.clone(),
            sessions: self.sessions.clone(),
        };
        if let Err(e) = save_state(&state) {
            let _ = self.action_tx.send(Action::Error(e.to_string()));
        }
    }

    fn load_persisted_state(&mut self) {
        match load_state() {
            Ok(state) => {
                self.projects = state.projects;
                self.sessions = state.sessions;
            }
            Err(e) => match backup_state() {
                Ok(backup) => self.dispatch_error(format!(
                    "failed to load state ({e}); moved to {}",
                    backup.display()
                )),
                Err(backup_err) => {
                    self.persist_disabled = true;
                    self.dispatch_error(format!(
                        "failed to load state ({e}); backup failed ({backup_err}), saving disabled"
                    ));
                }
            },
        }
    }

    fn dispatch_error(&self, e: impl std::fmt::Display) {
        let _ = self.action_tx.send(Action::Error(e.to_string()));
    }
}
