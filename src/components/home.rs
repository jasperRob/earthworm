use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use super::Component;
use crate::{
    action::Action,
    app::Mode,
    components::popups::{
        PopupOutcome, PopupState,
        help::HelpPopup,
        project::{edit_project, new_project, remove_project},
        session::{
            edit_session, new_session, new_session_from_worktree, new_session_in_project,
            remove_session,
        },
        worktree::remove_worktree,
    },
    config::key_event_to_string,
    pending::{Activity, PendingTarget, apply_placeholders},
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

const SPINNER_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_FRAME_MS: u128 = 80;

enum ListEntry {
    Project(Project),
    ProjectSession(Project, Session),
    AvailableWorktree(Project, Worktree),
    Session(Session),
}

impl ListEntry {
    fn target(&self) -> PendingTarget {
        match self {
            ListEntry::Project(p) => PendingTarget::Project(p.id),
            ListEntry::ProjectSession(_, s) | ListEntry::Session(s) => PendingTarget::Session(s.id),
            ListEntry::AvailableWorktree(p, wt) => PendingTarget::Worktree {
                project_id: p.id,
                path: wt.path.clone(),
            },
        }
    }
}

pub struct Home {
    keymaps: Vec<(String, String)>,
    list_entries: Vec<ListEntry>,
    list_state: ListState,
    popup_state: PopupState,
    search_is_capturing: bool,
    search: Option<String>,
    search_matches: Vec<usize>,
    search_cursor: usize,
    projects: HashMap<Uuid, Project>,
    session_history: VecDeque<Session>,
    session_history_index: usize,
    pending: HashMap<PendingTarget, Activity>,
    spinner_start: Instant,
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
            projects: HashMap::default(),
            session_history: VecDeque::new(),
            session_history_index: usize::default(),
            pending: HashMap::default(),
            spinner_start: Instant::now(),
        }
    }

    /// The activity running on an entry, or on the project it belongs to
    fn row_activity(&self, entry: &ListEntry) -> Option<Activity> {
        let parent = match entry {
            ListEntry::ProjectSession(p, _) | ListEntry::AvailableWorktree(p, _) => {
                self.pending.get(&PendingTarget::Project(p.id))
            }
            ListEntry::Project(_) | ListEntry::Session(_) => None,
        };
        self.pending.get(&entry.target()).or(parent).copied()
    }

    fn selected_is_pending(&self) -> bool {
        self.list_state
            .selected()
            .and_then(|i| self.list_entries.get(i))
            .is_some_and(|entry| self.row_activity(entry).is_some())
    }

    fn spinner(&self) -> char {
        let frame = self.spinner_start.elapsed().as_millis() / SPINNER_FRAME_MS;
        SPINNER_FRAMES[frame as usize % SPINNER_FRAMES.len()]
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

        let current_id = self
            .session_history
            .get(self.session_history_index)
            .map(|s| s.id);
        self.session_history
            .retain(|s| sessions.contains_key(&s.id));
        self.session_history_index = current_id
            .and_then(|id| self.session_history.iter().position(|s| s.id == id))
            .unwrap_or_else(|| self.session_history.len().saturating_sub(1));

        if self.list_entries.is_empty() {
            self.list_state.select(None);
        } else {
            let i = self.list_state.selected().unwrap_or(0);
            self.list_state
                .select(Some(i.min(self.list_entries.len() - 1)));
        }
    }

    fn push_to_session_history(&mut self, session: Session) {
        // Ensure each session appears only once, as the most-recent entry:
        // drop any prior occurrence (covers both an identical last entry and
        // older duplicates), then re-add at the back.
        self.session_history.retain(|s| s.id != session.id);
        self.session_history.push_back(session);
        self.session_history_index = self.session_history.len() - 1;
    }

    fn select_session_history(&mut self) {
        let Some(move_to_session) = self.session_history.get(self.session_history_index) else {
            return;
        };
        let list_state_index = self.list_entries.iter().position(|v| match v {
            ListEntry::ProjectSession(_, s) | ListEntry::Session(s) => s.id == move_to_session.id,
            ListEntry::Project(_) | ListEntry::AvailableWorktree(_, _) => false,
        });
        if list_state_index.is_some() {
            self.list_state.select(list_state_index);
        }
    }
}

impl Component for Home {
    fn is_capturing_input(&self) -> bool {
        !matches!(self.popup_state, PopupState::Closed) || self.search_is_capturing
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
            Action::StateUpdated(mut projects, mut sessions, pending) => {
                apply_placeholders(&mut projects, &mut sessions, &pending);
                self.pending = pending
                    .into_iter()
                    .map(|p| (p.target, p.activity))
                    .collect();
                self.projects = projects.clone();
                self.rebuild_list(projects, sessions);
            }
            Action::CmdSelectNext => self.list_state.select_next(),
            Action::CmdSelectPrev => self.list_state.select_previous(),
            Action::CmdJumpTop if !self.list_entries.is_empty() => {
                self.list_state.select(Some(0));
            }
            Action::CmdJumpBottom if !self.list_entries.is_empty() => {
                self.list_state.select(Some(self.list_entries.len() - 1));
            }
            Action::CmdStartSearch => {
                self.search = Some(String::new());
                self.search_is_capturing = true;
            }
            Action::CmdSearchNext if !self.search_matches.is_empty() => {
                self.search_cursor = (self.search_cursor + 1) % self.search_matches.len();
                self.list_state
                    .select(Some(self.search_matches[self.search_cursor]));
            }
            Action::CmdSearchPrev if !self.search_matches.is_empty() => {
                self.search_cursor = self
                    .search_cursor
                    .checked_sub(1)
                    .unwrap_or(self.search_matches.len() - 1);
                self.list_state
                    .select(Some(self.search_matches[self.search_cursor]));
            }
            Action::CmdAddProject => self.popup_state = PopupState::Open(Box::new(new_project())),
            Action::CmdAddSessionInProject if !self.selected_is_pending() => {
                if let Some(selected) = self.list_state.selected() {
                    match self.list_entries.get(selected) {
                        Some(ListEntry::Project(p)) => {
                            self.popup_state =
                                PopupState::Open(Box::new(new_session_in_project(p)));
                        }
                        Some(ListEntry::ProjectSession(p, _)) => {
                            self.popup_state =
                                PopupState::Open(Box::new(new_session_in_project(p)));
                        }
                        _ => {}
                    }
                }
            }
            Action::CmdAddSession if !self.selected_is_pending() => {
                if let Some(selected) = self.list_state.selected() {
                    match self.list_entries.get(selected) {
                        Some(ListEntry::AvailableWorktree(p, wt)) => {
                            self.popup_state =
                                PopupState::Open(Box::new(new_session_from_worktree(p, wt)))
                        }
                        _ => self.popup_state = PopupState::Open(Box::new(new_session())),
                    }
                }
            }
            Action::CmdEdit if !self.selected_is_pending() => {
                if let Some(i) = self.list_state.selected()
                    && let Some(entry) = self.list_entries.get(i)
                {
                    match entry {
                        ListEntry::Project(p) => {
                            self.popup_state = PopupState::Open(Box::new(edit_project(p.clone())))
                        }
                        ListEntry::ProjectSession(_, s) => {
                            self.popup_state = PopupState::Open(Box::new(edit_session(s.clone())))
                        }
                        ListEntry::AvailableWorktree(_, _) => {}
                        ListEntry::Session(s) => {
                            self.popup_state = PopupState::Open(Box::new(edit_session(s.clone())))
                        }
                    }
                }
            }
            Action::CmdDeleteItem if !self.selected_is_pending() => {
                if let Some(i) = self.list_state.selected()
                    && let Some(entry) = self.list_entries.get(i)
                {
                    match entry {
                        ListEntry::Project(p) => {
                            self.popup_state =
                                PopupState::Open(Box::new(remove_project(p.clone())));
                        }
                        ListEntry::ProjectSession(_, s) => {
                            self.popup_state =
                                PopupState::Open(Box::new(remove_session(s.clone())));
                        }
                        ListEntry::Session(s) => {
                            self.popup_state =
                                PopupState::Open(Box::new(remove_session(s.clone())));
                        }
                        ListEntry::AvailableWorktree(p, wt) => {
                            self.popup_state =
                                PopupState::Open(Box::new(remove_worktree(wt.clone(), p.clone())));
                        }
                    }
                }
            }
            Action::CmdAttach if !self.selected_is_pending() => {
                let mut session: Option<Session> = None;
                if let Some(i) = self.list_state.selected()
                    && let Some(entry) = self.list_entries.get(i)
                {
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
                if let Some(s) = session {
                    self.push_to_session_history(s.clone());
                    return Ok(Some(Action::AttachSession(s)));
                }
            }
            Action::CmdNextSessionHistory => {
                if self.session_history_index + 1 < self.session_history.len() {
                    self.session_history_index += 1;
                }
                self.select_session_history();
            }
            Action::CmdPrevSessionHistory => {
                if self.session_history_index > 0 {
                    self.session_history_index -= 1;
                }
                self.select_session_history();
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let spinner = self.spinner();
        let title = match self.pending.get(&PendingTarget::App) {
            Some(activity) => format!(" earthworm  {spinner} {activity}"),
            None => " earthworm".to_string(),
        };
        let title_bar = Paragraph::new(title).block(Block::default()).style(
            Style::default()
                .fg(SECONDARY)
                .add_modifier(Modifier::REVERSED),
        );

        let items: Vec<ListItem> = self
            .list_entries
            .iter()
            .map(|entry| {
                let line = match entry {
                    ListEntry::Project(p) => Line::from(Span::styled(
                        format!(" - {}", p.name.clone()),
                        Style::default().add_modifier(Modifier::BOLD).fg(SECONDARY),
                    )),
                    ListEntry::ProjectSession(_, s) => {
                        Line::from(Span::raw(format!("     {}", s.name.clone(),)))
                    }
                    ListEntry::AvailableWorktree(_, worktree) => Line::from(format!(
                        "     [worktree] ({}) {}",
                        worktree.name.clone(),
                        worktree.path.clone()
                    )),
                    ListEntry::Session(s) => Line::from(format!(" {}", s.name.clone())),
                };
                match self.row_activity(entry) {
                    Some(activity) => ListItem::new(with_activity(line, spinner, activity)),
                    None => ListItem::new(line),
                }
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

        match &mut self.popup_state {
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

/// Dims a row and marks it with the activity running on it
fn with_activity(line: Line<'_>, spinner: char, activity: Activity) -> Line<'_> {
    let mut line = line.patch_style(Modifier::DIM);
    line.push_span(Span::raw(format!("  {spinner} {activity}")));
    line
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use super::{Home, ListEntry};
    use crate::{
        action::Action,
        components::{Component, popups::PopupState},
        pending::{Activity, Pending, PendingTarget},
        project::Project,
        session::Session,
    };

    fn project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "project".to_string(),
            path: String::new(),
            sessions: Vec::new(),
            worktrees: Vec::new(),
        }
    }

    fn session(project_id: Option<Uuid>) -> Session {
        Session {
            id: Uuid::new_v4(),
            project_id,
            name: "session".to_string(),
            path: None,
            worktree: None,
        }
    }

    fn home(projects: Vec<Project>, sessions: Vec<Session>, pending: Vec<Pending>) -> Home {
        let mut home = Home::new();
        home.update(Action::StateUpdated(
            projects.into_iter().map(|p| (p.id, p)).collect(),
            sessions.into_iter().map(|s| (s.id, s)).collect(),
            pending,
        ))
        .unwrap();
        home
    }

    fn select_session(home: &mut Home, session_id: Uuid) {
        let index = home.list_entries.iter().position(|e| {
            matches!(e, ListEntry::ProjectSession(_, s) | ListEntry::Session(s) if s.id == session_id)
        });
        home.list_state.select(index);
    }

    #[test]
    fn test_placeholder_session_is_listed_under_its_project() {
        let project = project();
        let session = session(Some(project.id));
        let home = home(
            vec![project.clone()],
            Vec::new(),
            vec![Pending::creating_session(&session)],
        );
        assert!(matches!(
            home.list_entries.as_slice(),
            [ListEntry::Project(p), ListEntry::ProjectSession(_, s)]
                if p.id == project.id && s.id == session.id
        ));
        assert_eq!(
            home.row_activity(&home.list_entries[1]),
            Some(Activity::Creating)
        );
    }

    #[test]
    fn test_delete_opens_popup_on_idle_row() {
        let session = session(None);
        let mut home = home(Vec::new(), vec![session.clone()], Vec::new());
        select_session(&mut home, session.id);
        home.update(Action::CmdDeleteItem).unwrap();
        assert!(matches!(home.popup_state, PopupState::Open(_)));
    }

    #[test]
    fn test_delete_and_edit_are_blocked_on_pending_row() {
        let session = session(None);
        let mut home = home(
            Vec::new(),
            vec![session.clone()],
            vec![Pending::new(
                PendingTarget::Session(session.id),
                Activity::Removing,
            )],
        );
        select_session(&mut home, session.id);
        home.update(Action::CmdDeleteItem).unwrap();
        home.update(Action::CmdEdit).unwrap();
        assert!(matches!(home.popup_state, PopupState::Closed));
    }

    #[test]
    fn test_attach_is_blocked_on_session_being_created() {
        let session = session(None);
        let mut home = home(
            Vec::new(),
            Vec::new(),
            vec![Pending::creating_session(&session)],
        );
        select_session(&mut home, session.id);
        assert_eq!(home.update(Action::CmdAttach).unwrap(), None);
    }

    #[test]
    fn test_project_sessions_inherit_project_activity() {
        let mut project = project();
        let session = session(Some(project.id));
        project.sessions.push(session.id);
        let mut home = home(
            vec![project.clone()],
            vec![session.clone()],
            vec![Pending::new(
                PendingTarget::Project(project.id),
                Activity::Removing,
            )],
        );
        select_session(&mut home, session.id);
        assert!(home.selected_is_pending());
        home.update(Action::CmdDeleteItem).unwrap();
        assert!(matches!(home.popup_state, PopupState::Closed));
    }

    #[test]
    fn test_finished_job_unblocks_row() {
        let session = session(None);
        let mut home = home(
            Vec::new(),
            vec![session.clone()],
            vec![Pending::new(
                PendingTarget::Session(session.id),
                Activity::Updating,
            )],
        );
        home.update(Action::StateUpdated(
            HashMap::new(),
            HashMap::from([(session.id, session.clone())]),
            Vec::new(),
        ))
        .unwrap();
        select_session(&mut home, session.id);
        assert!(!home.selected_is_pending());
    }
}
