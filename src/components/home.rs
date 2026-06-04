use std::collections::{HashMap, HashSet};

use super::Component;
use crate::{action::Action, git::Worktree, project::Project, session::Session};
use color_eyre::eyre::Ok;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use uuid::Uuid;

#[derive(Default)]
pub struct Home {
    list_entries: Vec<ListEntry>,
    list_state: ListState,
    popup_state: PopupState,
}

enum ListEntry {
    Project(Project),
    ProjectSession(Project, Session),
    AvailableWorktree(Project, Worktree),
    Session(Session),
}

#[derive(Default)]
enum PopupState {
    #[default]
    Closed,
    NewProject {
        project: Project,
        focused: usize,
    },
    NewSession {
        session: Session,
        focused: usize,
    },
}

impl Home {
    pub fn new() -> Self {
        Self {
            list_entries: Vec::default(),
            list_state: ListState::default(),
            popup_state: PopupState::Closed,
        }
    }

    fn rebuild_list(&mut self, projects: HashMap<Uuid, Project>, sessions: HashMap<Uuid, Session>) {
        let mut list_entries: Vec<ListEntry> = Vec::new();

        for project in projects.values() {
            list_entries.push(ListEntry::Project(project.clone()));
            for session_id in &project.sessions {
                if let Some(session) = sessions.get(session_id) {
                    list_entries.push(ListEntry::ProjectSession(project.clone(), session.clone()));
                }
            }
            let active_paths: HashSet<&str> = sessions.values().map(|s| s.path.as_str()).collect();

            for worktree in &project.worktrees {
                if !active_paths.contains(worktree.path.as_str()) {
                    list_entries.push(ListEntry::AvailableWorktree(
                        project.clone(),
                        worktree.clone(),
                    ));
                }
            }
        }

        list_entries.extend(
            sessions
                .values()
                .filter(|s| s.project_id.is_none())
                .map(|s| ListEntry::Session(s.clone())),
        );
        self.list_entries = list_entries;

        if self.list_entries.is_empty() {
            self.list_state.select(None);
        } else {
            let i = self.list_state.selected().unwrap_or(0);
            self.list_state
                .select(Some(i.min(self.list_entries.len() - 1)));
        }
    }
}

impl Component for Home {
    fn is_capturing_input(&self) -> bool {
        return !matches!(self.popup_state, PopupState::Closed);
    }
    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        match &mut self.popup_state {
            PopupState::Closed => Ok(None), // config keybindings handle this
            PopupState::NewProject { project, focused } => match key.code {
                KeyCode::Tab => {
                    *focused = (*focused + 1) % 2;
                    return Ok(None);
                }
                KeyCode::BackTab => {
                    if *focused > 0 {
                        *focused -= 1;
                    }
                    return Ok(None);
                }
                KeyCode::Char(c) => {
                    match *focused {
                        0 => project.name.push(c),
                        1 => project.path.push(c),
                        _ => {}
                    }
                    Ok(None)
                }
                KeyCode::Backspace => {
                    match *focused {
                        0 => {
                            project.name.pop();
                        }
                        1 => {
                            project.path.pop();
                        }
                        _ => {}
                    }
                    Ok(None)
                }
                KeyCode::Enter => {
                    let project = project.clone();
                    self.popup_state = PopupState::Closed;
                    return Ok(Some(Action::SubmitProject(project)));
                }
                KeyCode::Esc => {
                    self.popup_state = PopupState::Closed;
                    return Ok(None);
                }
                _ => Ok(None),
            },
            PopupState::NewSession { session, focused } => match key.code {
                KeyCode::Tab => {
                    *focused = (*focused + 1) % 3;
                    return Ok(None);
                }
                KeyCode::BackTab => {
                    if *focused > 0 {
                        *focused -= 1;
                    }
                    return Ok(None);
                }
                KeyCode::Char(c) => {
                    match *focused {
                        0 => session.name.push(c),
                        1 => session.path.push(c),
                        2 => session.worktree.push(c),
                        _ => {}
                    }
                    Ok(None)
                }
                KeyCode::Backspace => {
                    match *focused {
                        0 => {
                            session.name.pop();
                        }
                        1 => {
                            session.path.pop();
                        }
                        2 => {
                            session.worktree.pop();
                        }
                        _ => {}
                    }
                    Ok(None)
                }
                KeyCode::Enter => {
                    let session = session.clone();
                    self.popup_state = PopupState::Closed;
                    return Ok(Some(Action::SubmitSession(session)));
                }
                KeyCode::Esc => Ok(Some(Action::CancelInput)),
                _ => Ok(None),
            },
        }
    }
    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::StateUpdated(projects, sessions) => {
                self.rebuild_list(projects, sessions);
            }
            Action::CmdSelectNext => self.list_state.select_next(),
            Action::CmdSelectPrev => self.list_state.select_previous(),
            Action::CmdManageProjects => {
                let project_id = Uuid::new_v4();
                self.popup_state = PopupState::NewProject {
                    project: Project {
                        id: project_id,
                        name: String::default(),
                        path: String::default(),
                        sessions: vec![],
                        worktrees: vec![],
                    },
                    focused: 0,
                }
            }
            Action::CmdAddSession => {
                let mut path = String::default();
                let mut project_id: Option<Uuid> = None;
                let mut name = String::default();
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(p) => {
                                path = p.path.clone();
                                project_id = Some(p.id.clone());
                            } // Do nothing
                            ListEntry::ProjectSession(p, _) => {
                                path = p.path.clone();
                                project_id = Some(p.id.clone());
                            }
                            ListEntry::AvailableWorktree(p, wt) => {
                                name = wt.name.clone(); // use worktree name as default session name
                                path = wt.path.clone();
                                project_id = Some(p.id.clone());
                            }
                            ListEntry::Session(_) => {}
                        }
                    }
                }
                self.popup_state = PopupState::NewSession {
                    session: Session {
                        id: Uuid::new_v4(),
                        project_id: project_id,
                        name: name,
                        path: path,
                        worktree: String::default(),
                    },
                    focused: 0,
                }
            }
            Action::CmdDeleteItem => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(_) => {} // Do nothing
                            ListEntry::ProjectSession(_, s) => {
                                let session = s.clone();
                                return Ok(Some(Action::RemoveSession(session)));
                            }
                            ListEntry::Session(s) => {
                                let session = s.clone();
                                return Ok(Some(Action::RemoveSession(session)));
                            }
                            ListEntry::AvailableWorktree(_, _) => {}
                        }
                    }
                }
            }
            Action::CancelInput => {
                self.popup_state = PopupState::Closed;
            }
            Action::CmdAttach => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(_) => {}
                            ListEntry::ProjectSession(_, s) => {
                                return Ok(Some(Action::AttachSession(s.clone())));
                            }
                            ListEntry::Session(s) => {
                                return Ok(Some(Action::AttachSession(s.clone())));
                            }
                            ListEntry::AvailableWorktree(_, _) => {}
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let items: Vec<ListItem> = self
            .list_entries
            .iter()
            .map(|entry| match entry {
                ListEntry::Project(p) => ListItem::new(Line::from(Span::styled(
                    format!("- {}", p.name.clone()),
                    Style::default().add_modifier(Modifier::BOLD),
                ))),
                ListEntry::ProjectSession(_, s) => ListItem::new(Line::from(vec![
                    Span::raw("    "),
                    Span::raw(s.name.clone()),
                ])),
                ListEntry::AvailableWorktree(_, worktree) => ListItem::new(Line::from(vec![
                    Span::raw("    "),
                    Span::raw(worktree.path.clone()),
                ])),
                ListEntry::Session(s) => ListItem::new(s.name.clone()),
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Sessions"))
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(list, area, &mut self.list_state);

        match &self.popup_state {
            PopupState::Closed => {}
            PopupState::NewProject { project, focused } => {
                let centered_area =
                    area.centered(Constraint::Percentage(60), Constraint::Percentage(20));
                // clears out any background in the area before rendering the popup
                frame.render_widget(Clear, centered_area);
                frame.render_widget(Block::bordered().title("New Project"), centered_area);

                let [name_area, path_area] =
                    Layout::vertical([Constraint::Length(3), Constraint::Length(3)]).areas(
                        centered_area.inner(Margin {
                            horizontal: 1,
                            vertical: 1,
                        }),
                    );
                let field_style = |i: usize| {
                    if *focused == i {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }
                };

                frame.render_widget(
                    Paragraph::new(project.name.as_str())
                        .block(Block::bordered().title("Name").border_style(field_style(0))),
                    name_area,
                );
                frame.render_widget(
                    Paragraph::new(project.path.as_str())
                        .block(Block::bordered().title("Path").border_style(field_style(1))),
                    path_area,
                );

                let mut active_area = name_area;
                let mut active_text = project.name.as_str();
                match focused {
                    0 => {}
                    1 => {
                        active_area = path_area;
                        active_text = project.path.as_str();
                    }
                    _ => {}
                }
                frame.set_cursor_position((
                    active_area.x + 1 + active_text.len() as u16,
                    active_area.y + 1,
                ));
            }
            PopupState::NewSession { session, focused } => {
                let centered_area =
                    area.centered(Constraint::Percentage(60), Constraint::Percentage(20));
                // clears out any background in the area before rendering the popup
                frame.render_widget(Clear, centered_area);
                frame.render_widget(Block::bordered().title("New Session"), centered_area);

                let [name_area, path_area, worktree_area] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .areas(centered_area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }));
                let field_style = |i: usize| {
                    if *focused == i {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }
                };

                frame.render_widget(
                    Paragraph::new(session.name.as_str())
                        .block(Block::bordered().title("Name").border_style(field_style(0))),
                    name_area,
                );
                frame.render_widget(
                    Paragraph::new(session.path.as_str())
                        .block(Block::bordered().title("Path").border_style(field_style(1))),
                    path_area,
                );
                frame.render_widget(
                    Paragraph::new(session.worktree.as_str()).block(
                        Block::bordered()
                            .title("Worktree")
                            .border_style(field_style(2)),
                    ),
                    worktree_area,
                );

                let mut active_area = name_area;
                let mut active_text = session.name.as_str();
                match focused {
                    0 => {}
                    1 => {
                        active_area = path_area;
                        active_text = session.path.as_str();
                    }
                    2 => {
                        active_area = worktree_area;
                        active_text = session.worktree.as_str();
                    }
                    _ => {}
                }
                frame.set_cursor_position((
                    active_area.x + 1 + active_text.len() as u16,
                    active_area.y + 1,
                ));
            }
        }

        Ok(())
    }
}
