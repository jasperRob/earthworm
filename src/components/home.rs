use std::collections::{HashMap, HashSet, VecDeque};

use super::Component;
use crate::{
    action::Action,
    app::Mode,
    components::popups::{
        PopupOutcome, PopupState, edit_project::EditProjectPopup, edit_session::EditSessionPopup,
        help::HelpPopup, new_project::NewProjectPopup, new_session::NewSessionPopup,
        remove_project::RemoveProjectPopup, remove_session::RemoveSessionPopup,
        remove_worktree::RemoveWorktreePopup,
    },
    config::key_event_to_string,
    project::Project,
    session::Session,
    theme::SECONDARY,
    worktree::Worktree,
};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use uuid::Uuid;

enum ListEntry {
    Project(Project),
    ProjectSession(Project, Session),
    AvailableWorktree(Project, Worktree),
    Session(Session),
}

#[derive(Default)]
pub struct Home {
    keymaps: Vec<(String, String)>,
    list_entries: Vec<ListEntry>,
    list_state: ListState,
    popup_state: PopupState,
    search_is_capturing: bool,
    search: Option<String>,
    search_matches: Vec<usize>,
    search_cursor: usize,
    session_history: VecDeque<Session>,
}

impl Home {
    pub fn new() -> Self {
        Self {
            keymaps: Vec::default(),
            list_entries: Vec::default(),
            list_state: ListState::default(),
            popup_state: PopupState::Closed,
            search_is_capturing: false,
            search: None,
            search_matches: Vec::default(),
            search_cursor: usize::default(),
            session_history: VecDeque::new(),
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

    fn push_to_session_history(&mut self, session: Session) {
        self.session_history.push_back(session);
        if self.session_history.len() > 2 {
            self.session_history.pop_front();
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
            PopupState::Open(p) => match p.handle_key(key) {
                PopupOutcome::Submitted(action) => {
                    self.popup_state = PopupState::Closed;
                    Ok(Some(action))
                }
                PopupOutcome::Cancelled => {
                    self.popup_state = PopupState::Closed;
                    Ok(None)
                }
                PopupOutcome::Pending => Ok(None),
            },
        }
    }
    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::Help => {
                self.popup_state = PopupState::Open(Box::new(HelpPopup::new(self.keymaps.clone())))
            }
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
                self.popup_state = PopupState::Open(Box::new(NewProjectPopup::new()))
            }
            Action::CmdAddSession => {
                let (project_id, path, name, worktree_name) = self
                    .list_state
                    .selected()
                    .and_then(|i| self.list_entries.get(i))
                    .map(|entry| match entry {
                        ListEntry::Project(p) => {
                            (Some(p.id.clone()), Some(p.path.clone()), None, None)
                        }
                        ListEntry::ProjectSession(p, _) => {
                            (Some(p.id.clone()), Some(p.path.clone()), None, None)
                        }
                        ListEntry::AvailableWorktree(p, wt) => (
                            Some(p.id.clone()),
                            Some(wt.path.clone()),
                            Some(wt.name.clone()),
                            Some(wt.name.clone()),
                        ),
                        ListEntry::Session(_) => (None, None, None, None),
                    })
                    .unwrap_or((None, None, None, None));
                self.popup_state = PopupState::Open(Box::new(NewSessionPopup::new(
                    project_id,
                    name,
                    worktree_name,
                    path,
                )))
            }
            Action::CmdEdit => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(p) => {
                                self.popup_state =
                                    PopupState::Open(Box::new(EditProjectPopup::new(p.clone())))
                            }
                            ListEntry::ProjectSession(_, s) => {
                                self.popup_state =
                                    PopupState::Open(Box::new(EditSessionPopup::new(s.clone())))
                            }
                            ListEntry::AvailableWorktree(_, _) => {}
                            ListEntry::Session(s) => {
                                self.popup_state =
                                    PopupState::Open(Box::new(EditSessionPopup::new(s.clone())))
                            }
                        }
                    }
                }
            }
            Action::CmdDeleteItem => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(p) => {
                                self.popup_state =
                                    PopupState::Open(Box::new(RemoveProjectPopup::new(p.clone())));
                            }
                            ListEntry::ProjectSession(_, s) => {
                                self.popup_state =
                                    PopupState::Open(Box::new(RemoveSessionPopup::new(s.clone())));
                            }
                            ListEntry::Session(s) => {
                                self.popup_state =
                                    PopupState::Open(Box::new(RemoveSessionPopup::new(s.clone())));
                            }
                            ListEntry::AvailableWorktree(p, wt) => {
                                self.popup_state = PopupState::Open(Box::new(
                                    RemoveWorktreePopup::new(wt.clone(), p.clone()),
                                ));
                            }
                        }
                    }
                }
            }
            Action::CancelInput => {
                self.popup_state = PopupState::Closed;
            }
            Action::CmdAttach => {
                let mut session: Option<Session> = None;
                if let Some(i) = self.list_state.selected() {
                    if let Some(entry) = self.list_entries.get(i) {
                        match entry {
                            ListEntry::Project(_) => {}
                            ListEntry::ProjectSession(_, s) => {
                                session = Some(s.clone());
                            }
                            ListEntry::Session(s) => {
                                session = Some(s.clone());
                            }
                            ListEntry::AvailableWorktree(_, _) => {}
                        }
                    }
                }
                if let Some(s) = session {
                    self.push_to_session_history(s.clone());
                    return Ok(Some(Action::AttachSession(s)));
                }
            }
            Action::CmdSelectPrevSession => {
                if self.session_history.len() >= 2 {
                    let prev_prev = &self.session_history[self.session_history.len() - 2];
                    let index = self
                        .list_entries
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| match v {
                            ListEntry::Project(_) => None,
                            ListEntry::ProjectSession(_, s) => {
                                if s.id == prev_prev.id {
                                    Some(i)
                                } else {
                                    None
                                }
                            }
                            ListEntry::Session(s) => {
                                if s.id == prev_prev.id {
                                    Some(i)
                                } else {
                                    None
                                }
                            }
                            ListEntry::AvailableWorktree(_, _) => None,
                        })
                        .next();
                    self.list_state.select(index);
                }
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let title_bar = Paragraph::new(" earthworm").block(Block::default()).style(
            Style::default()
                .fg(SECONDARY)
                .add_modifier(Modifier::REVERSED),
        );

        let items: Vec<ListItem> = self
            .list_entries
            .iter()
            .map(|entry| match entry {
                ListEntry::Project(p) => ListItem::new(Line::from(Span::styled(
                    format!(" - {}", p.name.clone()),
                    Style::default().add_modifier(Modifier::BOLD).fg(SECONDARY),
                ))),
                ListEntry::ProjectSession(_, s) => {
                    ListItem::new(Line::from(Span::raw(format!("     {}", s.name.clone(),))))
                }
                ListEntry::AvailableWorktree(_, worktree) => ListItem::new(Line::from(format!(
                    "     [worktree] ({}) {}",
                    worktree.name.clone(),
                    worktree.path.clone()
                ))),
                ListEntry::Session(s) => ListItem::new(Line::from(format!(" {}", s.name.clone()))),
            })
            .collect();

        let list = List::new(items)
            .block(Block::default())
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let info_bar = Paragraph::new(" ?: help")
            .block(Block::default())
            .style(Style::default().add_modifier(Modifier::REVERSED));

        let search_bar = Paragraph::new(format!(
            "{}{}",
            if self.search.is_some() { "/" } else { "" },
            self.search.as_deref().unwrap_or("")
        ))
        .block(Block::default());

        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let [title_row, _, list_body, info_row, search_row] = Layout::vertical(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        frame.render_widget(title_bar, title_row);
        frame.render_stateful_widget(list, list_body, &mut self.list_state);
        frame.render_widget(info_bar, info_row);
        frame.render_widget(search_bar, search_row);

        match &self.popup_state {
            PopupState::Closed => {}
            PopupState::Open(p) => p.draw(frame, area),
        }

        Ok(())
    }

    fn register_config_handler(&mut self, config: crate::config::Config) -> color_eyre::Result<()> {
        if let Some(bindings) = config.keybindings.0.get(&Mode::Home) {
            self.keymaps = bindings
                .iter()
                .map(|(keys, action)| {
                    let key_str = keys
                        .iter()
                        .map(key_event_to_string)
                        .collect::<Vec<_>>()
                        .join(" ");
                    (key_str, action.to_string())
                })
                .collect();
        }
        Ok(())
    }
}
