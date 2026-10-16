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
const SESSION_GLYPH: char = '●';
const WORKTREE_GLYPH: char = '○';

#[derive(Clone)]
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

    /// The project a session or worktree row is listed under
    fn parent_id(&self) -> Option<Uuid> {
        match self {
            ListEntry::ProjectSession(p, _) | ListEntry::AvailableWorktree(p, _) => Some(p.id),
            ListEntry::Project(_) | ListEntry::Session(_) => None,
        }
    }

    /// Whether the entry's name (or worktree path) contains `query`, which
    /// must already be lowercase
    fn matches_search(&self, query: &str) -> bool {
        let contains = |text: &str| text.to_lowercase().contains(query);
        match self {
            ListEntry::Project(p) => contains(&p.name),
            ListEntry::ProjectSession(_, s) | ListEntry::Session(s) => contains(&s.name),
            ListEntry::AvailableWorktree(_, wt) => contains(&wt.name) || contains(&wt.path),
        }
    }
}

pub struct Home {
    keymaps: Vec<(String, String)>,
    /// Every entry, including those of collapsed projects
    tree: Vec<ListEntry>,
    /// The entries shown, which the list state indexes into
    list_entries: Vec<ListEntry>,
    collapsed: HashSet<Uuid>,
    list_state: ListState,
    popup_state: PopupState,
    search_is_capturing: bool,
    search: Option<String>,
    search_matches: Vec<PendingTarget>,
    search_cursor: usize,
    projects: HashMap<Uuid, Project>,
    sessions: HashMap<Uuid, Session>,
    session_history: VecDeque<Session>,
    session_history_index: usize,
    pending: HashMap<PendingTarget, Activity>,
    spinner_start: Instant,
}

impl Home {
    pub fn new() -> Self {
        Self {
            keymaps: Vec::default(),
            tree: Vec::default(),
            list_entries: Vec::default(),
            collapsed: HashSet::default(),
            list_state: ListState::default(),
            popup_state: PopupState::Closed,
            search_is_capturing: false,
            search: None,
            search_matches: Vec::default(),
            search_cursor: usize::default(),
            projects: HashMap::default(),
            sessions: HashMap::default(),
            session_history: VecDeque::new(),
            session_history_index: usize::default(),
            pending: HashMap::default(),
            spinner_start: Instant::now(),
        }
    }

    /// The activity running on an entry, on the project it belongs to, or, for a project, on
    /// creating one of its sessions
    fn row_activity(&self, entry: &ListEntry) -> Option<Activity> {
        let related = match entry {
            ListEntry::ProjectSession(p, _) | ListEntry::AvailableWorktree(p, _) => {
                self.pending.get(&PendingTarget::Project(p.id))
            }
            // Pending session placeholders are already listed under their project
            ListEntry::Project(p) => self.list_entries.iter().find_map(|e| match e {
                ListEntry::ProjectSession(sp, s) if sp.id == p.id => self
                    .pending
                    .get(&PendingTarget::Session(s.id))
                    .filter(|activity| **activity == Activity::Creating),
                _ => None,
            }),
            ListEntry::Session(_) => None,
        };
        self.pending.get(&entry.target()).or(related).copied()
    }

    fn selected_entry(&self) -> Option<&ListEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.list_entries.get(i))
    }

    fn position_of(&self, target: &PendingTarget) -> Option<usize> {
        self.list_entries.iter().position(|e| e.target() == *target)
    }

    fn selected_is_pending(&self) -> bool {
        self.selected_entry()
            .is_some_and(|entry| self.row_activity(entry).is_some())
    }

    fn spinner(&self) -> char {
        let frame = self.spinner_start.elapsed().as_millis() / SPINNER_FRAME_MS;
        SPINNER_FRAMES[frame as usize % SPINNER_FRAMES.len()]
    }

    fn rebuild_list(&mut self) {
        let selected = self.selected_entry().map(ListEntry::target);
        let (projects, sessions) = (&self.projects, &self.sessions);
        let mut tree: Vec<ListEntry> = Vec::new();

        let mut sorted_projects: Vec<&Project> = projects.values().collect();
        sorted_projects.sort_by(|a, b| a.name.cmp(&b.name));

        for project in sorted_projects {
            tree.push(ListEntry::Project(project.clone()));
            let mut project_sessions: Vec<&Session> = sessions
                .values()
                .filter(|s| s.project_id == Some(project.id))
                .collect();
            project_sessions.sort_by(|a, b| a.name.cmp(&b.name));
            for session in project_sessions {
                tree.push(ListEntry::ProjectSession(project.clone(), session.clone()));
            }
            // Find all paths which are currently used by sessions
            let active_paths: HashSet<&str> = sessions
                .values()
                .filter_map(|s| s.path.as_deref())
                .collect();

            // Only add worktree if path does not match that of a current session
            for worktree in &project.worktrees {
                if !active_paths.contains(worktree.path.as_str()) {
                    tree.push(ListEntry::AvailableWorktree(
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
        tree.extend(orphaned.into_iter().map(|s| ListEntry::Session(s.clone())));
        self.list_entries = tree
            .iter()
            .filter(|entry| {
                entry
                    .parent_id()
                    .is_none_or(|id| !self.collapsed.contains(&id))
            })
            .cloned()
            .collect();
        self.tree = tree;

        let current_id = self
            .session_history
            .get(self.session_history_index)
            .map(|s| s.id);
        self.session_history
            .retain(|s| self.sessions.contains_key(&s.id));
        self.session_history_index = current_id
            .and_then(|id| self.session_history.iter().position(|s| s.id == id))
            .unwrap_or_else(|| self.session_history.len().saturating_sub(1));

        if self.list_entries.is_empty() {
            self.list_state.select(None);
        } else {
            // Keep the same entry selected, as rows above it may have been added or removed
            let i = selected
                .and_then(|target| self.position_of(&target))
                .or(self.list_state.selected())
                .unwrap_or(0);
            self.list_state
                .select(Some(i.min(self.list_entries.len() - 1)));
        }

        // Entries may have been added or renamed, so refresh the matches
        if self.search.is_some() {
            self.recompute_search_matches();
        }
    }

    fn recompute_search_matches(&mut self) {
        let query = self.search.as_deref().unwrap_or_default().to_lowercase();
        self.search_matches = if query.is_empty() {
            Vec::new()
        } else {
            self.tree
                .iter()
                .filter(|entry| entry.matches_search(&query))
                .map(ListEntry::target)
                .collect()
        };
        self.search_cursor = self
            .search_cursor
            .min(self.search_matches.len().saturating_sub(1));
    }

    fn select_first_search_match(&mut self) {
        self.search_cursor = 0;
        self.select_search_match();
    }

    fn select_search_match(&mut self) {
        if let Some(target) = self.search_matches.get(self.search_cursor).cloned() {
            self.select_revealing(&target);
        }
    }

    /// Selects an entry, first expanding its project if collapsed, like opening a fold
    fn select_revealing(&mut self, target: &PendingTarget) {
        if self.position_of(target).is_none()
            && let Some(project_id) = self
                .tree
                .iter()
                .find(|entry| entry.target() == *target)
                .and_then(ListEntry::parent_id)
        {
            self.collapsed.remove(&project_id);
            self.rebuild_list();
        }
        let index = self.position_of(target);
        if index.is_some() {
            self.list_state.select(index);
        }
    }

    /// The selected project, or the project the selected row is listed under
    fn selected_project_id(&self) -> Option<Uuid> {
        match self.selected_entry()? {
            ListEntry::Project(p) => Some(p.id),
            entry => entry.parent_id(),
        }
    }

    /// Collapses the selected project, or the one the selected row is listed under, moving the
    /// selection onto it
    fn collapse_selected(&mut self) {
        let Some(project_id) = self.selected_project_id() else {
            return;
        };
        self.collapsed.insert(project_id);
        self.list_state
            .select(self.position_of(&PendingTarget::Project(project_id)));
        self.rebuild_list();
    }

    fn expand_selected(&mut self) {
        if let Some(ListEntry::Project(p)) = self.selected_entry() {
            let project_id = p.id;
            self.collapsed.remove(&project_id);
            self.rebuild_list();
        }
    }

    fn toggle_selected(&mut self) {
        let is_collapsed_project = matches!(
            self.selected_entry(),
            Some(ListEntry::Project(p)) if self.collapsed.contains(&p.id)
        );
        if is_collapsed_project {
            self.expand_selected();
        } else {
            self.collapse_selected();
        }
    }

    /// Attaches to the selected session, recording it in the session history
    fn attach_selected(&mut self) -> Option<Action> {
        if self.selected_is_pending() {
            return None;
        }
        let session = match self.selected_entry()? {
            ListEntry::ProjectSession(_, s) | ListEntry::Session(s) => s.clone(),
            ListEntry::Project(_) | ListEntry::AvailableWorktree(_, _) => return None,
        };
        self.push_to_session_history(session.clone());
        Some(Action::AttachSession(session))
    }

    fn entry_line(&self, entry: &ListEntry, next: Option<&ListEntry>) -> Line<'static> {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let branch = |project_id: Uuid| {
            // The last row under a project closes off the tree
            let glyph = if next.and_then(ListEntry::parent_id) == Some(project_id) {
                '├'
            } else {
                '└'
            };
            Span::styled(format!("   {glyph} "), dim)
        };
        match entry {
            ListEntry::Project(p) => {
                let marker = if self.collapsed.contains(&p.id) {
                    '▸'
                } else {
                    '▾'
                };
                let count = self
                    .tree
                    .iter()
                    .filter(|e| e.parent_id() == Some(p.id))
                    .count();
                Line::from(vec![
                    Span::styled(
                        format!(" {marker} {}", p.name),
                        Style::default().add_modifier(Modifier::BOLD).fg(SECONDARY),
                    ),
                    Span::styled(format!(" ({count})"), dim),
                ])
            }
            ListEntry::ProjectSession(p, s) => Line::from(vec![
                branch(p.id),
                Span::raw(format!("{SESSION_GLYPH} {}", s.name)),
            ]),
            ListEntry::AvailableWorktree(p, wt) => Line::from(vec![
                branch(p.id),
                Span::styled(format!("{WORKTREE_GLYPH} {}", wt.name), dim),
            ]),
            ListEntry::Session(s) => Line::from(format!(" {SESSION_GLYPH} {}", s.name)),
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
        if let Some(session) = self.session_history.get(self.session_history_index) {
            self.select_revealing(&PendingTarget::Session(session.id));
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
                        self.search_matches.clear();
                        self.search_cursor = 0;
                    }
                    Ok(None)
                }
                KeyCode::Char(c) => {
                    if self.search_is_capturing {
                        if let Some(search) = &mut self.search {
                            search.push(c);
                        }
                        self.recompute_search_matches();
                        self.select_first_search_match();
                    }
                    Ok(None)
                }
                KeyCode::Backspace => {
                    if self.search_is_capturing {
                        if let Some(search) = &mut self.search {
                            search.pop();
                        }
                        self.recompute_search_matches();
                        self.select_first_search_match();
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
                self.projects = projects;
                self.sessions = sessions;
                self.rebuild_list();
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
                self.select_search_match();
            }
            Action::CmdSearchPrev if !self.search_matches.is_empty() => {
                self.search_cursor = self
                    .search_cursor
                    .checked_sub(1)
                    .unwrap_or(self.search_matches.len() - 1);
                self.select_search_match();
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
                // Nothing is selected on an empty list, which still allows a plain session
                let selected = self
                    .list_state
                    .selected()
                    .and_then(|i| self.list_entries.get(i));
                match selected {
                    Some(ListEntry::AvailableWorktree(p, wt)) => {
                        self.popup_state =
                            PopupState::Open(Box::new(new_session_from_worktree(p, wt)))
                    }
                    _ => self.popup_state = PopupState::Open(Box::new(new_session())),
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
            Action::CmdAttach => return Ok(self.attach_selected()),
            Action::CmdActivate => match self.selected_entry() {
                Some(ListEntry::Project(_)) => self.toggle_selected(),
                Some(ListEntry::ProjectSession(_, _) | ListEntry::Session(_)) => {
                    return Ok(self.attach_selected());
                }
                Some(ListEntry::AvailableWorktree(_, _)) | None => {}
            },
            Action::CmdCollapse => self.collapse_selected(),
            Action::CmdExpand => self.expand_selected(),
            Action::CmdToggleCollapse => self.toggle_selected(),
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
            .enumerate()
            .map(|(i, entry)| {
                let line = self.entry_line(entry, self.list_entries.get(i + 1));
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

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    use uuid::Uuid;

    use super::{Home, ListEntry};
    use crate::{
        action::Action,
        components::{Component, popups::PopupState},
        pending::{Activity, Pending, PendingTarget},
        project::Project,
        session::Session,
        worktree::Worktree,
    };

    fn project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "project".to_string(),
            path: String::new(),
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
    fn test_project_sessions_are_listed_under_it_sorted_by_name() {
        let project = project();
        let mut b = session(Some(project.id));
        b.name = "b".to_string();
        let mut a = session(Some(project.id));
        a.name = "a".to_string();
        let home = home(
            vec![project.clone()],
            vec![b.clone(), a.clone()],
            Vec::new(),
        );
        assert!(matches!(
            home.list_entries.as_slice(),
            [
                ListEntry::Project(p),
                ListEntry::ProjectSession(_, first),
                ListEntry::ProjectSession(_, second),
            ] if p.id == project.id && first.id == a.id && second.id == b.id
        ));
    }

    #[test]
    fn test_session_of_missing_project_is_not_listed() {
        let project = project();
        let session = session(Some(Uuid::new_v4()));
        let home = home(vec![project.clone()], vec![session], Vec::new());
        assert!(matches!(
            home.list_entries.as_slice(),
            [ListEntry::Project(p)] if p.id == project.id
        ));
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
        let project = project();
        let session = session(Some(project.id));
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
    fn test_project_is_busy_while_creating_one_of_its_sessions() {
        let project = project();
        let session = session(Some(project.id));
        let mut home = home(
            vec![project.clone()],
            Vec::new(),
            vec![Pending::creating_session(&session)],
        );
        assert_eq!(
            home.row_activity(&home.list_entries[0]),
            Some(Activity::Creating)
        );
        home.list_state.select(Some(0));
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

    fn named_session(name: &str) -> Session {
        Session {
            name: name.to_string(),
            ..session(None)
        }
    }

    /// Rows of the search matches, in match order
    fn match_rows(home: &Home) -> Vec<usize> {
        home.search_matches
            .iter()
            .filter_map(|target| home.position_of(target))
            .collect()
    }

    fn press(home: &mut Home, code: KeyCode) {
        home.handle_key_event(KeyEvent::new(code, KeyModifiers::empty()))
            .unwrap();
    }

    fn search(home: &mut Home, query: &str) {
        home.update(Action::CmdStartSearch).unwrap();
        for c in query.chars() {
            press(home, KeyCode::Char(c));
        }
    }

    fn state_updated(home: &mut Home, projects: Vec<Project>, sessions: Vec<Session>) {
        home.update(Action::StateUpdated(
            projects.into_iter().map(|p| (p.id, p)).collect(),
            sessions.into_iter().map(|s| (s.id, s)).collect(),
            Vec::new(),
        ))
        .unwrap();
    }

    #[test]
    fn test_add_session_on_empty_list_opens_popup() {
        let mut home = home(Vec::new(), Vec::new(), Vec::new());
        assert_eq!(home.list_state.selected(), None);
        home.update(Action::CmdAddSession).unwrap();
        assert!(matches!(home.popup_state, PopupState::Open(_)));
    }

    #[test]
    fn test_typing_search_selects_first_match() {
        let sessions = vec![
            named_session("alpha"),
            named_session("beta"),
            named_session("gamma"),
        ];
        let mut home = home(Vec::new(), sessions, Vec::new());
        search(&mut home, "ga");
        assert_eq!(match_rows(&home), vec![2]);
        assert_eq!(home.list_state.selected(), Some(2));
    }

    #[test]
    fn test_backspace_widens_search_matches() {
        let sessions = vec![named_session("bar"), named_session("baz")];
        let mut home = home(Vec::new(), sessions, Vec::new());
        search(&mut home, "bar");
        assert_eq!(match_rows(&home), vec![0]);
        press(&mut home, KeyCode::Backspace);
        assert_eq!(home.search.as_deref(), Some("ba"));
        assert_eq!(match_rows(&home), vec![0, 1]);
    }

    #[test]
    fn test_search_is_case_insensitive() {
        let sessions = vec![named_session("alpha"), named_session("myServer")];
        let mut home = home(Vec::new(), sessions, Vec::new());
        search(&mut home, "MYs");
        assert_eq!(match_rows(&home), vec![1]);
    }

    #[test]
    fn test_search_matches_worktree_name_and_path() {
        let mut project = project();
        project.worktrees.push(Worktree {
            name: "feature".to_string(),
            path: "/tmp/checkout".to_string(),
        });
        let mut home = home(vec![project], Vec::new(), Vec::new());
        search(&mut home, "FEAT");
        assert_eq!(match_rows(&home), vec![1]);
        press(&mut home, KeyCode::Esc);
        search(&mut home, "checkout");
        assert_eq!(match_rows(&home), vec![1]);
    }

    #[test]
    fn test_esc_clears_search_matches() {
        let sessions = vec![named_session("alpha"), named_session("beta")];
        let mut home = home(Vec::new(), sessions, Vec::new());
        search(&mut home, "a");
        assert_eq!(match_rows(&home), vec![0, 1]);
        press(&mut home, KeyCode::Esc);
        assert_eq!(home.search, None);
        assert!(home.search_matches.is_empty());
        assert_eq!(home.search_cursor, 0);

        home.list_state.select(Some(0));
        home.update(Action::CmdSearchNext).unwrap();
        assert_eq!(home.list_state.selected(), Some(0));
    }

    #[test]
    fn test_search_matches_recomputed_after_state_updated() {
        let alpha = named_session("alpha");
        let beta = named_session("beta");
        let mut home = home(Vec::new(), vec![alpha.clone(), beta.clone()], Vec::new());
        search(&mut home, "beta");
        assert_eq!(match_rows(&home), vec![1]);

        // A new entry sorting first shifts every row
        state_updated(
            &mut home,
            Vec::new(),
            vec![named_session("abeta"), alpha, beta],
        );
        assert_eq!(match_rows(&home), vec![0, 2]);
    }

    #[test]
    fn test_selection_follows_entry_when_rows_shift() {
        let beta = named_session("beta");
        let mut home = home(Vec::new(), vec![beta.clone()], Vec::new());
        select_session(&mut home, beta.id);
        state_updated(&mut home, Vec::new(), vec![named_session("alpha"), beta]);
        assert_eq!(home.list_state.selected(), Some(1));
    }

    #[test]
    fn test_attach_pushes_session_history() {
        let session = session(None);
        let mut home = home(Vec::new(), vec![session.clone()], Vec::new());
        select_session(&mut home, session.id);
        assert_eq!(
            home.update(Action::CmdAttach).unwrap(),
            Some(Action::AttachSession(session.clone()))
        );
        assert_eq!(home.session_history, [session]);
        assert_eq!(home.session_history_index, 0);
    }

    #[test]
    fn test_session_history_moves_selection() {
        let alpha = named_session("alpha");
        let beta = named_session("beta");
        let mut home = home(Vec::new(), vec![alpha.clone(), beta.clone()], Vec::new());
        select_session(&mut home, alpha.id);
        home.update(Action::CmdAttach).unwrap();
        select_session(&mut home, beta.id);
        home.update(Action::CmdAttach).unwrap();

        home.update(Action::CmdPrevSessionHistory).unwrap();
        assert_eq!(home.list_state.selected(), Some(0));
        home.update(Action::CmdNextSessionHistory).unwrap();
        assert_eq!(home.list_state.selected(), Some(1));
    }

    #[test]
    fn test_removed_sessions_are_dropped_from_history() {
        let alpha = named_session("alpha");
        let beta = named_session("beta");
        let mut home = home(Vec::new(), vec![alpha.clone(), beta.clone()], Vec::new());
        select_session(&mut home, alpha.id);
        home.update(Action::CmdAttach).unwrap();
        select_session(&mut home, beta.id);
        home.update(Action::CmdAttach).unwrap();

        state_updated(&mut home, Vec::new(), vec![alpha.clone()]);
        assert_eq!(home.session_history, [alpha]);
        assert_eq!(home.session_history_index, 0);
    }

    #[test]
    fn test_projects_are_sorted_by_name() {
        let zeta = Project {
            name: "zeta".to_string(),
            ..project()
        };
        let alpha = Project {
            name: "alpha".to_string(),
            ..project()
        };
        let home = home(vec![zeta.clone(), alpha.clone()], Vec::new(), Vec::new());
        assert!(matches!(
            home.list_entries.as_slice(),
            [ListEntry::Project(a), ListEntry::Project(z)]
                if a.id == alpha.id && z.id == zeta.id
        ));
    }

    #[test]
    fn test_worktree_used_by_session_is_hidden() {
        let mut project = project();
        let used = Worktree {
            name: "used".to_string(),
            path: "/tmp/used".to_string(),
        };
        let free = Worktree {
            name: "free".to_string(),
            path: "/tmp/free".to_string(),
        };
        project.worktrees = vec![used.clone(), free.clone()];
        let session = Session {
            path: Some(used.path.clone()),
            ..session(Some(project.id))
        };
        let home = home(vec![project], vec![session.clone()], Vec::new());
        assert!(matches!(
            home.list_entries.as_slice(),
            [
                ListEntry::Project(_),
                ListEntry::ProjectSession(_, s),
                ListEntry::AvailableWorktree(_, wt),
            ] if s.id == session.id && *wt == free
        ));
    }

    /// A project with a session, and a second project after it
    fn two_projects() -> (Project, Session, Project) {
        let first = Project {
            name: "first".to_string(),
            ..project()
        };
        let second = Project {
            name: "second".to_string(),
            ..project()
        };
        let session = named_session_in("feature", first.id);
        (first, session, second)
    }

    fn named_session_in(name: &str, project_id: Uuid) -> Session {
        Session {
            name: name.to_string(),
            ..session(Some(project_id))
        }
    }

    #[test]
    fn test_collapse_hides_project_rows_and_selects_project() {
        let (first, session, second) = two_projects();
        let mut home = home(
            vec![first.clone(), second.clone()],
            vec![session.clone()],
            Vec::new(),
        );
        select_session(&mut home, session.id);
        home.update(Action::CmdCollapse).unwrap();
        assert!(matches!(
            home.list_entries.as_slice(),
            [ListEntry::Project(a), ListEntry::Project(b)] if a.id == first.id && b.id == second.id
        ));
        assert_eq!(home.list_state.selected(), Some(0));

        home.update(Action::CmdExpand).unwrap();
        assert_eq!(home.list_entries.len(), 3);
        assert_eq!(home.list_state.selected(), Some(0));
    }

    #[test]
    fn test_project_stays_collapsed_across_state_updates() {
        let (first, session, second) = two_projects();
        let mut home = home(
            vec![first.clone(), second.clone()],
            vec![session.clone()],
            Vec::new(),
        );
        home.list_state.select(Some(0));
        home.update(Action::CmdToggleCollapse).unwrap();
        state_updated(&mut home, vec![first, second], vec![session]);
        assert_eq!(home.list_entries.len(), 2);
    }

    #[test]
    fn test_toggle_on_session_collapses_its_project() {
        let (first, session, second) = two_projects();
        let mut home = home(vec![first, second], vec![session.clone()], Vec::new());
        select_session(&mut home, session.id);
        home.update(Action::CmdToggleCollapse).unwrap();
        assert_eq!(home.list_entries.len(), 2);
        home.update(Action::CmdToggleCollapse).unwrap();
        assert_eq!(home.list_entries.len(), 3);
    }

    #[test]
    fn test_collapse_ignores_session_without_project() {
        let session = session(None);
        let mut home = home(Vec::new(), vec![session.clone()], Vec::new());
        select_session(&mut home, session.id);
        home.update(Action::CmdCollapse).unwrap();
        assert!(home.collapsed.is_empty());
        assert_eq!(home.list_entries.len(), 1);
    }

    #[test]
    fn test_search_expands_collapsed_project_of_match() {
        let (first, session, second) = two_projects();
        let mut home = home(
            vec![first.clone(), second],
            vec![session.clone()],
            Vec::new(),
        );
        home.list_state.select(Some(0));
        home.update(Action::CmdCollapse).unwrap();
        search(&mut home, "feat");
        assert!(!home.collapsed.contains(&first.id));
        assert_eq!(home.list_state.selected(), Some(1));
        assert_eq!(match_rows(&home), vec![1]);
    }

    #[test]
    fn test_session_history_expands_collapsed_project() {
        let (first, session, second) = two_projects();
        let mut home = home(
            vec![first.clone(), second],
            vec![session.clone()],
            Vec::new(),
        );
        select_session(&mut home, session.id);
        home.update(Action::CmdAttach).unwrap();
        home.update(Action::CmdCollapse).unwrap();
        home.update(Action::CmdPrevSessionHistory).unwrap();
        assert!(!home.collapsed.contains(&first.id));
        assert_eq!(home.list_state.selected(), Some(1));
    }

    #[test]
    fn test_activate_toggles_project() {
        let (first, session, second) = two_projects();
        let mut home = home(vec![first, second], vec![session], Vec::new());
        home.list_state.select(Some(0));
        assert_eq!(home.update(Action::CmdActivate).unwrap(), None);
        assert_eq!(home.list_entries.len(), 2);
        home.update(Action::CmdActivate).unwrap();
        assert_eq!(home.list_entries.len(), 3);
    }

    #[test]
    fn test_activate_attaches_session() {
        let session = session(None);
        let mut home = home(Vec::new(), vec![session.clone()], Vec::new());
        select_session(&mut home, session.id);
        assert_eq!(
            home.update(Action::CmdActivate).unwrap(),
            Some(Action::AttachSession(session.clone()))
        );
        assert_eq!(home.session_history, [session]);
    }

    #[test]
    fn test_activate_does_nothing_on_worktree() {
        let mut project = project();
        project.worktrees.push(Worktree {
            name: "free".to_string(),
            path: "/tmp/free".to_string(),
        });
        let mut home = home(vec![project], Vec::new(), Vec::new());
        home.list_state.select(Some(1));
        assert_eq!(home.update(Action::CmdActivate).unwrap(), None);
        assert!(matches!(home.popup_state, PopupState::Closed));
        assert_eq!(home.list_entries.len(), 2);
    }

    /// The rendered rows of the list, without trailing spaces
    fn render(home: &mut Home, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| home.draw(frame, frame.area()).unwrap())
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn test_draw_shows_projects_as_tree() {
        let mut api = Project {
            name: "api".to_string(),
            ..project()
        };
        api.worktrees.push(Worktree {
            name: "fix-bug".to_string(),
            path: "/tmp/fix-bug".to_string(),
        });
        let web = Project {
            name: "web".to_string(),
            ..project()
        };
        let main = named_session_in("main", api.id);
        let dev = named_session_in("dev", web.id);
        let mut home = home(
            vec![api, web.clone()],
            vec![main, dev, named_session("scratch")],
            Vec::new(),
        );
        home.list_state.select(Some(3));
        home.update(Action::CmdCollapse).unwrap();
        assert_eq!(
            render(&mut home, 30, 11)[3..8],
            [
                "  ▾ api (2)",
                "    ├ ● main",
                "    └ ○ fix-bug",
                "  ▸ web (1)",
                "  ● scratch",
            ]
        );
    }

    #[test]
    fn test_orphan_sessions_are_listed_after_projects() {
        let project = Project {
            name: "zeta".to_string(),
            ..project()
        };
        let orphan = named_session("alpha");
        let home = home(vec![project.clone()], vec![orphan.clone()], Vec::new());
        assert!(matches!(
            home.list_entries.as_slice(),
            [ListEntry::Project(p), ListEntry::Session(s)]
                if p.id == project.id && s.id == orphan.id
        ));
    }
}
