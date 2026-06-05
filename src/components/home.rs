use std::collections::{HashMap, HashSet};

use super::Component;
use crate::{action::Action, git::Worktree, project::Project, session::Session};
use color_eyre::eyre::Ok;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use uuid::Uuid;

#[derive(Default)]
pub struct Home {
    list_entries: Vec<ListEntry>,
    list_state: ListState,
    popup_state: PopupState,
    search: Option<String>,
    search_matches: Vec<usize>,
    search_cursor: usize,
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
    KillSession {
        session: Session,
    },
}

impl Home {
    pub fn new() -> Self {
        Self {
            list_entries: Vec::default(),
            list_state: ListState::default(),
            popup_state: PopupState::Closed,
            search: None,
            search_matches: Vec::default(),
            search_cursor: usize::default(),
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
        return !matches!(self.popup_state, PopupState::Closed) || self.search.is_some();
    }
    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        match &mut self.popup_state {
            PopupState::Closed => match key.code {
                KeyCode::Esc => {
                    if self.search.is_some() {
                        self.search = None;
                    }
                    Ok(None)
                }
                KeyCode::Char(c) => {
                    if self.search.is_some() {
                        if let Some(search) = &mut self.search {
                            search.push(c);
                        }
                        self.search_matches = self
                            .list_entries
                            .iter()
                            .enumerate()
                            .filter_map(|(index, item)| {
                                let text = match item {
                                    ListEntry::Project(p) => p.name.as_str(),
                                    ListEntry::ProjectSession(_, s) => s.name.as_str(),
                                    ListEntry::AvailableWorktree(_, w) => w.path.as_str(),
                                    ListEntry::Session(s) => s.name.as_str(),
                                };
                                let query = self.search.clone().unwrap_or_default();
                                if text.contains(&query) {
                                    Some(index)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        self.search_cursor = 0;
                        if let Some(&first_match) = self.search_matches.first() {
                            self.list_state.select(Some(first_match))
                        }
                    } else {
                        match c {
                            '/' => {
                                self.search = Some(String::new());
                            }
                            'n' => {
                                if !self.search_matches.is_empty() {
                                    self.search_cursor =
                                        (self.search_cursor + 1) % self.search_matches.len();
                                    self.list_state
                                        .select(Some(self.search_matches[self.search_cursor]));
                                }
                            }
                            'N' => {
                                if !self.search_matches.is_empty() {
                                    self.search_cursor = self
                                        .search_cursor
                                        .checked_sub(1)
                                        .unwrap_or(self.search_matches.len() - 1);
                                    self.list_state
                                        .select(Some(self.search_matches[self.search_cursor]));
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(None)
                }
                _ => Ok(None),
            },
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
            PopupState::KillSession { session } => match key.code {
                KeyCode::Char(c) => match c {
                    'y' => {
                        let session = session.clone();
                        self.popup_state = PopupState::Closed;
                        return Ok(Some(Action::RemoveSession(session)));
                    }
                    'n' => {
                        self.popup_state = PopupState::Closed;
                        return Ok(None);
                    }
                    _ => Ok(None),
                },
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
                                self.popup_state = PopupState::KillSession { session: s.clone() };
                            }
                            ListEntry::Session(s) => {
                                self.popup_state = PopupState::KillSession { session: s.clone() };
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
                let fields = &[
                    ("Name", project.name.as_str(), *focused == 0),
                    ("Path", project.path.as_str(), *focused == 1),
                ];

                open_popup(frame, area, "New Project", fields);
            }
            PopupState::NewSession { session, focused } => {
                let fields = &[
                    ("Name", session.name.as_str(), *focused == 0),
                    ("Path", session.path.as_str(), *focused == 1),
                    ("Worktree", session.worktree.as_str(), *focused == 2),
                ];

                open_popup(frame, area, "New Session", fields);
            }
            PopupState::KillSession { session } => {
                let text_body = format!("Kill \"{}\"?", session.name);
                let popup = area.centered(
                    Constraint::Length(text_body.len() as u16 * 2),
                    Constraint::Length(11),
                );
                popup_ouline(frame, popup, "Kill Session?");
                let [_, text_area, _] = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(5),
                    Constraint::Fill(1),
                ])
                .areas(popup);
                let text = Text::from(vec![
                    Line::from(format!("Kill \"{}\"?", session.name)).centered(),
                    Line::from(""),
                    Line::from(""),
                    Line::from(""),
                    Line::from("y / n").centered(),
                ]);
                frame.render_widget(Paragraph::new(text), text_area);
            }
        }

        Ok(())
    }
}

fn open_popup(frame: &mut Frame, area: Rect, title: &str, fields: &[(&str, &str, bool)]) {
    let height = (2 * fields.len() - 1) as u16 + 4;
    let popup = area.centered(
        Constraint::Percentage(40),
        Constraint::Length(height as u16),
    );
    // clears out any background in the area before rendering the popup
    frame.render_widget(Clear, popup);
    frame.render_widget(Block::bordered().title(title), popup);
    let inner = popup.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let mut constraints: Vec<Constraint> = vec![Constraint::Fill(1)];
    for _ in fields {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
    }
    constraints.pop();
    constraints.push(Constraint::Fill(1));

    let areas = Layout::vertical(constraints).split(inner);

    let mut active_area = Rect::default();
    let mut active_label = String::default();
    let mut active_text = String::default();
    for (i, (label, value, is_focused)) in fields.iter().enumerate() {
        let widget = labeled_input(label, value, *is_focused);
        let area = areas[(i * 2) + 1];
        frame.render_widget(widget, area);
        if *is_focused {
            active_area = area;
            active_label = label.to_string();
            active_text = value.to_string();
        }
    }

    frame.set_cursor_position((
        active_area.x + 1 + active_label.len() as u16 + 2 + active_text.len() as u16,
        active_area.y,
    ));
}

fn labeled_input<'a>(label: &'a str, value: &'a str, is_focused: bool) -> Paragraph<'a> {
    let label_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    Paragraph::new(Line::from(vec![
        Span::styled(format!(" {}: ", label), label_style),
        Span::raw(value),
    ]))
}

fn popup_ouline(frame: &mut Frame, popup: Rect, title: &str) -> Rect {
    frame.render_widget(Clear, popup);
    frame.render_widget(Block::bordered().title(title), popup);
    popup.inner(Margin {
        horizontal: 1,
        vertical: 1,
    })
}
