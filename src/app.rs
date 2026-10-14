use std::collections::HashMap;

use color_eyre::eyre::eyre;
use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    action::{Action, JobCompletion},
    components::{Component, fps::FpsCounter, home::Home, notification::Notification},
    config::Config,
    pending::{JobId, Pending},
    project::Project,
    session::Session,
    state::{AppState, load_state, save_state},
    tui::{Event, Tui},
};

mod handlers;
mod jobs;

/// How often tmux is re-synced, to pick up sessions created or killed outside the app
const TMUX_SYNC_INTERVAL_SECS: f64 = 5.0;

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    components: Vec<Box<dyn Component>>,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    projects: HashMap<Uuid, Project>,
    sessions: HashMap<Uuid, Session>,
    pending: HashMap<JobId, Pending>,
    next_job_id: JobId,
    ticks_since_sync: u64,
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
            components: vec![
                Box::new(Home::new()),
                Box::new(FpsCounter::default()),
                Box::new(Notification::default()),
            ],
            should_quit: false,
            should_suspend: false,
            config: Config::new()?,
            mode: Mode::Home,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            projects: HashMap::default(),
            sessions: HashMap::default(),
            pending: HashMap::default(),
            next_job_id: JobId::default(),
            ticks_since_sync: 0,
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
        let action_tx = self.action_tx.clone();
        let Some(keymap) = self.config.keybindings.0.get(&self.mode) else {
            return Ok(());
        };
        match keymap.get(&vec![key]) {
            Some(action) => {
                info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
                self.last_tick_key_events.drain(..);
            }
            _ => {
                // If the key was not handled as a single key action,
                // then consider it for multi-key combinations.
                self.last_tick_key_events.push(key);

                // Check for multi-key combinations
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                    self.last_tick_key_events.drain(..);
                } else {
                    let is_prefix = keymap.keys().any(|k| {
                        k.starts_with(&self.last_tick_key_events)
                            && k.len() > self.last_tick_key_events.len()
                    });
                    if !is_prefix {
                        self.last_tick_key_events.drain(..);
                    }
                }
            }
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
                    // self.last_tick_key_events.drain(..);
                    self.ticks_since_sync += 1;
                    if self.ticks_since_sync >= self.sync_interval_ticks() {
                        self.start_tmux_sync()
                    } else {
                        Ok(())
                    }
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
                    // Sessions may have been created or killed while attached
                    result.and_then(|()| self.start_tmux_sync())
                }
                Action::RemoveSession(ref session) => self.on_remove_session(session),
                Action::RemoveWorktree(ref project, ref worktree) => {
                    self.on_remove_worktree(project, worktree)
                }
                Action::JobFinished(id, ref completion) => {
                    self.pending.remove(&id);
                    // Handled in place, so the job's outcome and its end are broadcast together
                    let outcome = self.handle_job_completion(id, completion);
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

    fn handle_job_completion(
        &mut self,
        id: JobId,
        completion: &JobCompletion,
    ) -> color_eyre::Result<()> {
        match *completion {
            JobCompletion::ProjectCreation {
                ref project,
                ref result,
            } => self.on_project_creation_finished(project, result),
            JobCompletion::ProjectUpdate {
                ref project,
                ref result,
            } => self.on_project_update_finished(project, result),
            JobCompletion::ProjectRemoval {
                project_id,
                ref result,
            } => self.on_project_removal_finished(project_id, result),
            JobCompletion::SessionCreation {
                ref session,
                ref result,
            } => self.on_session_creation_finished(session, result),
            JobCompletion::SessionUpdate {
                ref session,
                ref result,
            } => self.on_session_update_finished(session, result),
            JobCompletion::SessionRemoval {
                ref session,
                ref result,
            } => self.on_session_removal_finished(session, result),
            JobCompletion::WorktreeRemoval {
                project_id,
                ref result,
            } => self.on_worktree_removal_finished(project_id, result),
            JobCompletion::TmuxSync {
                ref known_session_ids,
                ref result,
            } => self.on_tmux_sync_finished(id, known_session_ids, result),
        }
    }

    /// Number of ticks between periodic tmux syncs
    fn sync_interval_ticks(&self) -> u64 {
        (TMUX_SYNC_INTERVAL_SECS * self.tick_rate).round().max(1.0) as u64
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
            Err(e) => {
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
    }

    fn dispatch_error(&self, e: impl std::fmt::Display) {
        let _ = self.action_tx.send(Action::Error(e.to_string()));
    }
}
