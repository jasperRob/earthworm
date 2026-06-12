use std::collections::{HashMap, HashSet};

use super::Component;
use crate::{
    action::Action,
    components::form_popup::{FormEvent, FormPopup},
    project::Project,
    session::Session,
    worktree::Worktree,
};
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

enum ListEntry {
    Project(Project),
    ProjectSession(Project, Session),
    AvailableWorktree(Project, Worktree),
    Session(Session),
}

enum ProjectField {
    Name = 0,
    Path = 1,
}

impl ProjectField {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Path => "Path",
        }
    }
}

enum NewSessionField {
    Name = 0,
    WorktreeName = 1,
    Path = 2,
}

impl NewSessionField {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::WorktreeName => "Worktree Name",
            Self::Path => "Path",
        }
    }
}

enum EditSessionField {
    Name = 0,
    Path = 1,
}

impl EditSessionField {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Path => "Path",
        }
    }
}

#[derive(Default)]
enum PopupState {
    #[default]
    Closed,
    NewProject {
        form: FormPopup,
    },
    EditProject {
        form: FormPopup,
        project: Project,
    },
    NewSession {
        form: FormPopup,
        project_id: Option<Uuid>,
        base_path: String,
    },
    EditSession {
        form: FormPopup,
        session: Session,
    },
    KillSession {
        session: Session,
    },
    RemoveWorktree {
        project: Project,
        worktree: Worktree,
    },
}

#[derive(Default)]
pub struct Home {
    list_entries: Vec<ListEntry>,
    list_state: ListState,
    popup_state: PopupState,
    search_is_capturing: bool,
    search: Option<String>,
    search_matches: Vec<usize>,
    search_cursor: usize,
}

impl Home {
    pub fn new() -> Self {
        Self {
            list_entries: Vec::default(),
            list_state: ListState::default(),
            popup_state: PopupState::Closed,
            search_is_capturing: false,
            search: None,
            search_matches: Vec::default(),
            search_cursor: usize::default(),
        }
    }

    fn rebuild_list(&mut self, projects: HashMap<Uuid, Project>, sessions: HashMap<Uuid, Session>) {
        let mut list_entries: Vec<ListEntry> = Vec::new();

        let mut sorted_projects: Vec<&Project> = projects.values().collect();
        sorted_projects.sort_by(|a, b| a.name.cmp(&b.name));

        for project in sorted_projects {
            list_entries.push(ListEntry::Project(project.clone()));
            for session_id in &project.sessions {
                if let Some(session) = sessions.get(session_id) {
                    list_entries.push(ListEntry::ProjectSession(project.clone(), session.clone()));
                }
            }
            // Find all paths which are currently used by sessions
            let active_paths: HashSet<&str> = sessions
                .values()
                .filter_map(|s| s.path.as_deref())
                .collect();

            // Only add worktree if path does not match that of a current session
            for worktree in &project.worktrees {
                if !active_paths.contains(worktree.path.as_str()) {
                    list_entries.push(ListEntry::AvailableWorktree(
                        project.clone(),
                        worktree.clone(),
                    ));
                }
            }
        }

        let mut orphaned: Vec<&Session> = sessions
            .values()
            .filter(|s| s.project_id.is_none())
            .collect();
        orphaned.sort_by(|a, b| a.name.cmp(&b.name));
        list_entries.extend(orphaned.into_iter().map(|s| ListEntry::Session(s.clone())));
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
        return !matches!(self.popup_state, PopupState::Closed) || self.search_is_capturing;
    }
    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        match &mut self.popup_state {
            PopupState::Closed => match key.code {
                KeyCode::Enter => {
                    if self.search_is_capturing {
                        self.search_is_capturing = false;
                    }
                    Ok(None)
                }
                KeyCode::Esc => {
                    if self.search_is_capturing {
                        self.search_is_capturing = false;
                        self.search = None;
                    }
                    Ok(None)
                }
                KeyCode::Char(c) => {
                    if self.search_is_capturing {
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
                    }
                    Ok(None)
                }
                _ => Ok(None),
            },
            PopupState::NewProject { form } => match form.handle_key(key) {
                FormEvent::Submit => {
                    let name: String = form.value(ProjectField::Name as usize).into();
                    let path: String = form.value(ProjectField::Path as usize).into();
                    let project = Project {
                        id: Uuid::new_v4(),
                        name: name,
                        path: path,
                        sessions: vec![],
                        worktrees: vec![], // TODO: this should probably init based on existing
                                           // worktrees for git repo, if any
                    };
                    self.popup_state = PopupState::Closed;
                    Ok(Some(Action::SubmitProject(project)))
                }
                FormEvent::Cancel => {
                    self.popup_state = PopupState::Closed;
                    Ok(None)
                }
                FormEvent::Continue => Ok(None),
            },
            PopupState::EditProject { form, project } => match form.handle_key(key) {
                FormEvent::Submit => {
                    let name: String = form.value(ProjectField::Name as usize).into();
                    let path: String = form.value(ProjectField::Path as usize).into();
                    let mut project = project.clone();
                    self.popup_state = PopupState::Closed;
                    project.name = name;
                    project.path = path;
                    Ok(Some(Action::UpdateProject(project.clone())))
                }
                FormEvent::Cancel => {
                    self.popup_state = PopupState::Closed;
                    Ok(None)
                }
                FormEvent::Continue => Ok(None),
            },
            PopupState::NewSession {
                form,
                project_id,
                base_path: _base_path,
            } => match form.handle_key(key) {
                FormEvent::Submit => {
                    let path: String = form.value(NewSessionField::Path as usize).into();
                    let worktree_name: String =
                        form.value(NewSessionField::WorktreeName as usize).into();
                    let session = Session {
                        id: Uuid::new_v4(),
                        project_id: *project_id,
                        name: form.value(NewSessionField::Name as usize).into(),
                        path: Some(path.clone()).filter(|s| !s.is_empty()),
                        worktree: (!worktree_name.is_empty()).then(|| Worktree {
                            name: worktree_name.clone(),
                            path: path,
                        }),
                    };
                    self.popup_state = PopupState::Closed;
                    Ok(Some(Action::SubmitSession(session)))
                }
                FormEvent::Cancel => {
                    self.popup_state = PopupState::Closed;
                    Ok(None)
                }
                FormEvent::Continue => Ok(None),
                // TODO: Find a way to integrate a feature like this
                // FormEvent::Continue => {
                //     if form.focused == NewSessionField::WorktreeName as usize {
                //         let wt_name = form
                //             .value(NewSessionField::WorktreeName as usize)
                //             .to_string();
                //         form.values[NewSessionField::Path as usize] = if wt_name.is_empty() {
                //             String::new()
                //         } else {
                //             format!("{}-{}", base_path, wt_name)
                //         }
                //     }
                //     Ok(None)
                // }
            },
            PopupState::EditSession { form, session } => match form.handle_key(key) {
                FormEvent::Submit => {
                    let name: String = form.value(EditSessionField::Name as usize).into();
                    let path: String = form.value(EditSessionField::Path as usize).into();
                    let mut session = session.clone();
                    self.popup_state = PopupState::Closed;
                    session.name = name;
                    session.path = Some(path);
                    Ok(Some(Action::SubmitSession(session.clone())))
                }
                FormEvent::Cancel => {
                    self.popup_state = PopupState::Closed;
                    Ok(None)
                }
                FormEvent::Continue => Ok(None),
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
            PopupState::RemoveWorktree { project, worktree } => match key.code {
                KeyCode::Char(c) => match c {
                    'y' => {
                        let project = project.clone();
                        let worktree = worktree.clone();
                        self.popup_state = PopupState::Closed;
                        return Ok(Some(Action::RemoveWorktree(project, worktree)));
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
            Action::CmdJumpTop => {
                if !self.list_entries.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            Action::CmdJumpBottom => {
                if !self.list_entries.is_empty() {
                    self.list_state.select(Some(self.list_entries.len() - 1));
                }
            }
            Action::CmdStartSearch => {
                self.search = Some(String::new());
                self.search_is_capturing = true;
            }
            Action::CmdSearchNext => {
                if !self.search_matches.is_empty() {
                    self.search_cursor = (self.search_cursor + 1) % self.search_matches.len();
                    self.list_state
                        .select(Some(self.search_matches[self.search_cursor]));
                }
            }
            Action::CmdSearchPrev => {
                if !self.search_matches.is_empty() {
                    self.search_cursor = self
                        .search_cursor
                        .checked_sub(1)
                        .unwrap_or(self.search_matches.len() - 1);
                    self.list_state
                        .select(Some(self.search_matches[self.search_cursor]));
                }
            }
            Action::CmdAddProject => {
                self.popup_state = PopupState::NewProject {
                    form: FormPopup::new(&[
                        (ProjectField::Name.label(), "".to_string()),
                        (ProjectField::Path.label(), "".to_string()),
                    ]),
                }
            }
            Action::CmdAddSession => {
                let mut name = String::default();
                let mut worktree_name = String::default();
                let mut path = String::default();
                let mut project_id: Option<Uuid> = None;
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
                                name = wt.name.clone();
                                worktree_name = wt.name.clone();
                                path = wt.path.clone();
                                project_id = Some(p.id.clone());
                            }
                            ListEntry::Session(_) => {}
                        }
                    }
                }
                self.popup_state = PopupState::NewSession {
                    form: FormPopup::new(&[
                        (NewSessionField::Name.label(), name.to_string()),
                        (
                            NewSessionField::WorktreeName.label(),
                            worktree_name.to_string(),
                        ),
                        (NewSessionField::Path.label(), path.to_string()),
                    ]),
                    project_id: project_id,
                    base_path: path,
                }
            }
            Action::CmdEdit => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(p) => {
                                self.popup_state = PopupState::EditProject {
                                    form: FormPopup::new(&[
                                        (ProjectField::Name.label(), p.name.to_string()),
                                        (ProjectField::Path.label(), p.path.to_string()),
                                    ]),
                                    project: p.clone(),
                                }
                            }
                            ListEntry::ProjectSession(_, s) => {
                                self.popup_state = PopupState::EditSession {
                                    form: FormPopup::new(&[
                                        (EditSessionField::Name.label(), s.name.to_string()),
                                        (
                                            EditSessionField::Path.label(),
                                            s.path.clone().unwrap_or_default(),
                                        ),
                                    ]),
                                    session: s.clone(),
                                }
                            }
                            ListEntry::AvailableWorktree(_, _) => {}
                            ListEntry::Session(s) => {
                                self.popup_state = PopupState::EditSession {
                                    form: FormPopup::new(&[
                                        (EditSessionField::Name.label(), s.name.to_string()),
                                        (
                                            EditSessionField::Path.label(),
                                            s.path.clone().unwrap_or_default(),
                                        ),
                                    ]),
                                    session: s.clone(),
                                }
                            }
                        }
                    }
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
                            ListEntry::AvailableWorktree(p, wt) => {
                                self.popup_state = PopupState::RemoveWorktree {
                                    project: p.clone(),
                                    worktree: wt.clone(),
                                };
                            }
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
                    format!(" - {}", p.name.clone()),
                    Style::default().add_modifier(Modifier::BOLD),
                ))),
                ListEntry::ProjectSession(_, s) => {
                    ListItem::new(Line::from(Span::raw(format!("     {}", s.name.clone(),))))
                }
                ListEntry::AvailableWorktree(_, worktree) => ListItem::new(Line::from(format!(
                    "     [worktree] ({})  {}",
                    worktree.name.clone(),
                    worktree.path.clone()
                ))),
                ListEntry::Session(s) => ListItem::new(Line::from(format!(" {}", s.name.clone()))),
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Sessions"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(list, area, &mut self.list_state);

        match &self.popup_state {
            PopupState::Closed => {}
            PopupState::NewProject { form } => {
                open_input_popup(frame, area, "New Project", form);
            }
            PopupState::EditProject {
                form,
                project: _project,
            } => {
                open_input_popup(frame, area, "Edit Project", form);
            }
            PopupState::NewSession {
                form,
                project_id: _project_id,
                base_path: _base_path,
            } => {
                open_input_popup(frame, area, "New Session", form);
            }
            PopupState::EditSession {
                form,
                session: _session,
            } => {
                open_input_popup(frame, area, "Edit Session", form);
            }
            PopupState::KillSession { session } => {
                let body = format!("Kill \"{}\"?", session.name);
                open_confirmation_popup(frame, area, "Kill Session", &body);
            }
            PopupState::RemoveWorktree {
                project: _project,
                worktree,
            } => {
                let body = format!("Remove \"{}\"?", worktree.name);
                open_confirmation_popup(frame, area, "Remove Worktree", &body);
            }
        }

        Ok(())
    }
}

fn open_input_popup(frame: &mut Frame, area: Rect, title: &str, form: &FormPopup) {
    let height = (2 * form.labels.len() - 1) as u16 + 4;
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
    for _ in 0..form.labels.len() {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
    }
    constraints.pop();
    constraints.push(Constraint::Fill(1));

    let areas = Layout::vertical(constraints).split(inner);

    let mut active_area = Rect::default();
    let mut active_label = String::default();
    let mut active_text = String::default();
    for (i, label) in form.labels.iter().enumerate() {
        let is_focused = form.focused == i;
        let value = form.value(i);
        let widget = labeled_input(label, value, is_focused);
        let area = areas[(i * 2) + 1];
        frame.render_widget(widget, area);
        if is_focused {
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

fn open_confirmation_popup(frame: &mut Frame, area: Rect, title: &str, body: &str) {
    let popup = area.centered(
        Constraint::Length(body.len() as u16 * 2),
        Constraint::Length(11),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(Block::bordered().title(title), popup);
    let inner = popup.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let [_, text_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(inner);
    let text = Text::from(vec![
        Line::from(body).centered(),
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from("y / n").centered(),
    ]);
    frame.render_widget(Paragraph::new(text), text_area);
}
