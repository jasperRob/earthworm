use std::collections::{HashMap, HashSet};

use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{Component, fps::FpsCounter, home::Home, notification::Notification},
    config::Config,
    git::fetch_worktrees,
    project::Project,
    session::Session,
    state::{AppState, load_state, save_state},
    tmux::{
        attach_tmux_session, create_worktree, fetch_tmux_sessions, kill_tmux_session,
        new_tmux_session, tmux_session_name,
    },
    tui::{Event, Tui},
};

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
        self.fetch_and_map_tmux_sessions()?;
        self.broadcast_state()?;

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
            }
            _ => {
                // If the key was not handled as a single key action,
                // then consider it for multi-key combinations.
                self.last_tick_key_events.push(key);

                // Check for multi-key combinations
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
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
            match action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                Action::SubmitProject(ref project) => {
                    let mut project = project.clone();
                    if !project.path.is_empty() {
                        project.worktrees = fetch_worktrees(&project.path).unwrap_or_default();
                    }
                    self.projects.insert(project.id, project.clone());
                    self.persist_state();
                    self.broadcast_state()?;
                }
                Action::SubmitSession(ref session) => {
                    let path: Option<String> =
                        if !session.worktree.is_empty() && !session.path.is_empty() {
                            match create_worktree(&session.path, &session.worktree) {
                                Ok(p) => Some(p),
                                Err(e) => {
                                    self.action_tx.send(Action::Error(e.to_string()))?;
                                    return Ok(());
                                }
                            }
                        } else if !session.path.is_empty() {
                            Some(session.path.clone())
                        } else {
                            None
                        };
                    if let Err(e) = new_tmux_session(session, &self.projects, path.as_deref()) {
                        self.action_tx.send(Action::Error(e.to_string()))?;
                    } else {
                        self.sessions.insert(session.id, session.clone());
                        if let Some(project_id) = session.project_id {
                            if let Some(project) = self.projects.get_mut(&project_id) {
                                project.sessions.push(session.id);
                            }
                        }
                        self.fetch_and_map_tmux_sessions()?;
                        self.persist_state();
                        self.broadcast_state()?;
                        self.action_tx.send(Action::ClearScreen)?;
                    }
                }
                Action::AttachSession(ref session) => {
                    tui.exit()?;
                    if let Err(e) = attach_tmux_session(session, &self.projects) {
                        self.action_tx.send(Action::Error(e.to_string()))?;
                    } else {
                        self.fetch_and_map_tmux_sessions()?;
                        self.persist_state();
                        self.broadcast_state()?;
                        self.action_tx.send(Action::ClearScreen)?;
                    }
                    tui.enter()?;
                }
                Action::RemoveSession(ref session) => {
                    if let Err(e) = kill_tmux_session(session, &self.projects) {
                        self.action_tx.send(Action::Error(e.to_string()))?;
                    }
                    self.sessions.remove(&session.id);
                    if let Some(project_id) = session.project_id {
                        if let Some(project) = self.projects.get_mut(&project_id) {
                            project.sessions.retain(|id| *id != session.id);
                        }
                    }
                    self.fetch_and_map_tmux_sessions()?;
                    self.persist_state();
                    self.broadcast_state()?;
                    self.action_tx.send(Action::ClearScreen)?;
                }
                _ => {}
            }
            for component in self.components.iter_mut() {
                if let Some(action) = component.update(action.clone())? {
                    self.action_tx.send(action)?
                };
            }
        }
        Ok(())
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

    fn fetch_and_map_tmux_sessions(&mut self) -> color_eyre::Result<()> {
        let active_session_names: HashSet<String> = fetch_tmux_sessions()?.into_iter().collect();

        let session_names_in_state: HashSet<String> = self
            .sessions
            .values()
            .filter_map(|s| tmux_session_name(s, &self.projects).ok())
            .collect();

        let projects = &self.projects;
        self.sessions.retain(|_id, session| {
            if session.project_id.is_none() {
                return true;
            }
            tmux_session_name(session, projects)
                .map(|name| active_session_names.contains(&name))
                .unwrap_or(false)
        });

        for name in &active_session_names {
            if !session_names_in_state.contains(name) {
                let uuid = Uuid::new_v4();
                self.sessions.insert(
                    uuid,
                    Session {
                        id: uuid,
                        project_id: None,
                        name: name.clone(),
                        path: String::new(),
                        worktree: String::new(),
                    },
                );
            }
        }

        for project in self.projects.values_mut() {
            if !project.path.is_empty() {
                project.worktrees = fetch_worktrees(&project.path).unwrap_or_default();
            }
        }
        Ok(())
    }

    fn broadcast_state(&self) -> color_eyre::Result<()> {
        self.action_tx.send(Action::StateUpdated(
            self.projects.clone(),
            self.sessions.clone(),
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
}
