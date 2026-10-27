use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{
    action::Action,
    components::{
        form::Form,
        popups::{Popup, PopupOutcome},
    },
    config::compact_key_sequence,
    theme::SECONDARY,
};

struct SectionSpec {
    title: &'static str,
    actions: &'static [Action],
    /// Whether its actions only apply to some rows, so those for the selected row are marked
    contextual: bool,
}

const SECTIONS: &[SectionSpec] = &[
    SectionSpec {
        title: "General",
        actions: &[Action::Quit, Action::Suspend, Action::Help],
        contextual: false,
    },
    SectionSpec {
        title: "Navigation",
        actions: &[
            Action::CmdSelectNext,
            Action::CmdSelectPrev,
            Action::CmdJumpTop,
            Action::CmdJumpBottom,
            Action::CmdCollapse,
            Action::CmdExpand,
            Action::CmdToggleCollapse,
        ],
        contextual: false,
    },
    SectionSpec {
        title: "Search",
        actions: &[
            Action::CmdStartSearch,
            Action::CmdSearchNext,
            Action::CmdSearchPrev,
        ],
        contextual: false,
    },
    SectionSpec {
        title: "Items",
        actions: &[
            Action::CmdActivate,
            Action::CmdAttach,
            Action::CmdAddProject,
            Action::CmdAddSession,
            Action::CmdAddSessionInProject,
            Action::CmdEdit,
            Action::CmdDeleteItem,
        ],
        contextual: true,
    },
    SectionSpec {
        title: "History",
        actions: &[Action::CmdNextSessionHistory, Action::CmdPrevSessionHistory],
        contextual: false,
    },
];

/// Keys handled by the forms and confirmations themselves, which aren't configurable
const DIALOG_KEYS: &[(&str, &str)] = &[
    ("tab, down", "Next field"),
    ("shift-tab, up", "Previous field"),
    ("␣, l, right", "Next choice"),
    ("h, left", "Previous choice"),
    ("⏎", "Submit"),
    ("y", "Confirm"),
    ("n, esc", "Cancel"),
];

/// Keys that terminals send as another key, left out when that key is bound to the same action
const KEY_ALIASES: &[(&str, &str)] = &[("ctrl-i", "tab")];

const MARKER: &str = "▸ ";
const COLUMN_SPACING: usize = 3;
const NAVIGATION_HINT: &str = " / filter  j/k scroll  ? close ";
const FILTER_HINT: &str = " ⏎ keep  esc clear ";
const MARKER_HINT: &str = " ▸ applies to selection ";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Relevance {
    /// Applies wherever the selection is
    Always,
    /// Applies to the selected row
    Selected,
    /// Doesn't apply to the selected row
    Unselected,
}

struct Entry {
    keys: String,
    description: String,
    relevance: Relevance,
}

impl Entry {
    /// Whether its keys or description contain `query`, which must already be lowercase
    fn matches(&self, query: &str) -> bool {
        self.keys.to_lowercase().contains(query) || self.description.to_lowercase().contains(query)
    }
}

struct Section {
    title: &'static str,
    entries: Vec<Entry>,
}

pub struct HelpPopup {
    form: Form,
    sections: Vec<Section>,
    filter: Option<String>,
    filter_is_capturing: bool,
    /// Lines scrolled past, kept within the content when drawn
    scroll: usize,
}

impl HelpPopup {
    /// `applicable` are the actions that apply to the selected row
    pub fn new(keymaps: Vec<(String, String)>, applicable: Vec<Action>) -> Self {
        let mut sections: Vec<Section> = SECTIONS
            .iter()
            .map(|spec| Section {
                title: spec.title,
                entries: spec
                    .actions
                    .iter()
                    .filter_map(|action| {
                        let keys = bound_keys(&keymaps, &action.to_string())?;
                        let relevance = if !spec.contextual {
                            Relevance::Always
                        } else if applicable.contains(action) {
                            Relevance::Selected
                        } else {
                            Relevance::Unselected
                        };
                        Some(Entry {
                            keys,
                            description: action.description().unwrap_or_default().to_string(),
                            relevance,
                        })
                    })
                    .collect(),
            })
            .collect();

        // Bindings to actions without a section still show, named as configured
        let mut other_actions: Vec<&str> = keymaps
            .iter()
            .map(|(_, action)| action.as_str())
            .filter(|action| {
                !SECTIONS
                    .iter()
                    .flat_map(|spec| spec.actions)
                    .any(|known| known.to_string() == *action)
            })
            .collect();
        other_actions.sort();
        other_actions.dedup();
        sections.push(Section {
            title: "Other",
            entries: other_actions
                .into_iter()
                .filter_map(|action| {
                    Some(Entry {
                        keys: bound_keys(&keymaps, action)?,
                        description: action.to_string(),
                        relevance: Relevance::Always,
                    })
                })
                .collect(),
        });

        sections.push(Section {
            title: "In dialogs",
            entries: DIALOG_KEYS
                .iter()
                .map(|(keys, description)| Entry {
                    keys: keys.to_string(),
                    description: description.to_string(),
                    relevance: Relevance::Always,
                })
                .collect(),
        });

        sections.retain(|section| !section.entries.is_empty());

        Self {
            form: Form::custom().title("Help"),
            sections,
            filter: None,
            filter_is_capturing: false,
            scroll: 0,
        }
    }

    fn has_marked_entries(&self) -> bool {
        self.sections
            .iter()
            .flat_map(|section| &section.entries)
            .any(|entry| entry.relevance == Relevance::Selected)
    }

    fn key_width(&self) -> usize {
        self.sections
            .iter()
            .flat_map(|section| &section.entries)
            .map(|entry| entry.keys.chars().count())
            .max()
            .unwrap_or_default()
    }

    /// The width of the widest line, filtered or not, so filtering doesn't resize the popup
    fn content_width(&self) -> usize {
        let key_width = self.key_width();
        self.sections
            .iter()
            .flat_map(|section| {
                std::iter::once(section.title.chars().count()).chain(section.entries.iter().map(
                    |entry| {
                        MARKER.chars().count()
                            + key_width
                            + COLUMN_SPACING
                            + entry.description.chars().count()
                    },
                ))
            })
            .max()
            .unwrap_or_default()
    }

    /// Section titles and their entries, with a blank line between sections, leaving out entries
    /// that don't match the filter and sections left empty
    fn lines(&self, filter: Option<&str>) -> Vec<Line<'static>> {
        let query = filter.map(str::to_lowercase).unwrap_or_default();
        let key_width = self.key_width();
        let mut lines = Vec::new();
        for section in &self.sections {
            let entries: Vec<&Entry> = section
                .entries
                .iter()
                .filter(|entry| entry.matches(&query))
                .collect();
            if entries.is_empty() {
                continue;
            }
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.push(Line::styled(
                section.title,
                Style::default().add_modifier(Modifier::BOLD),
            ));
            for entry in entries {
                let (marker, key_style, description_style) = match entry.relevance {
                    Relevance::Always => ("  ", Style::default().fg(SECONDARY), Style::default()),
                    Relevance::Selected => (
                        MARKER,
                        Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Relevance::Unselected => (
                        "  ",
                        Style::default().fg(SECONDARY).add_modifier(Modifier::DIM),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                };
                lines.push(Line::from(vec![
                    Span::raw(marker),
                    Span::styled(format!("{:<key_width$}", entry.keys), key_style),
                    Span::raw(" ".repeat(COLUMN_SPACING)),
                    Span::styled(entry.description.clone(), description_style),
                ]));
            }
        }
        lines
    }
}

/// The keys bound to `action`, written compactly and shortest first, or None if there are none
fn bound_keys(keymaps: &[(String, String)], action: &str) -> Option<String> {
    let bound: Vec<&str> = keymaps
        .iter()
        .filter(|(_, bound)| bound == action)
        .map(|(keys, _)| keys.as_str())
        .collect();
    let mut keys: Vec<String> = bound
        .iter()
        .filter(|keys| {
            !KEY_ALIASES
                .iter()
                .any(|(alias, key)| *keys == alias && bound.contains(key))
        })
        .map(|keys| compact_key_sequence(keys))
        .collect();
    if keys.is_empty() {
        return None;
    }
    keys.sort_by_key(|keys| (keys.chars().count(), keys.clone()));
    Some(keys.join(", "))
}

impl Popup for HelpPopup {
    fn handle_key(&mut self, key: KeyEvent) -> PopupOutcome {
        if self.filter_is_capturing {
            match key.code {
                KeyCode::Enter => self.filter_is_capturing = false,
                KeyCode::Esc => {
                    self.filter_is_capturing = false;
                    self.filter = None;
                }
                KeyCode::Char(c) => {
                    if let Some(filter) = &mut self.filter {
                        filter.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(filter) = &mut self.filter {
                        filter.pop();
                    }
                }
                _ => {}
            }
            self.scroll = 0;
            return PopupOutcome::Pending;
        }
        match key.code {
            KeyCode::Char('q' | '?') => return PopupOutcome::Cancelled,
            // A kept filter is cleared before the popup closes
            KeyCode::Esc if self.filter.is_some() => {
                self.filter = None;
                self.scroll = 0;
            }
            KeyCode::Esc => return PopupOutcome::Cancelled,
            KeyCode::Char('/') => {
                self.filter = Some(String::new());
                self.filter_is_capturing = true;
                self.scroll = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = usize::MAX,
            _ => {}
        }
        PopupOutcome::Pending
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let border = 2;
        let padding = 2;
        let filter_height = usize::from(self.filter.is_some());

        let hint = if self.filter_is_capturing {
            FILTER_HINT
        } else {
            NAVIGATION_HINT
        };
        let marker_hint = if self.has_marked_entries() {
            MARKER_HINT
        } else {
            ""
        };
        let hints_width = NAVIGATION_HINT.chars().count() + marker_hint.chars().count() + border;

        let width = (self.content_width() + border + padding).max(hints_width);
        // Sized for every line, so filtering doesn't resize the popup
        let height = self.lines(None).len() + filter_height + border + padding;
        let popup = area.centered(
            Constraint::Length(width.min(usize::from(area.width)) as u16),
            Constraint::Length(height.min(usize::from(area.height)) as u16),
        );

        self.form.draw_form_frame(frame, popup);
        // Kept off the corners of the frame's border
        frame.render_widget(
            Block::new()
                .title_bottom(Line::from(marker_hint).left_aligned())
                .title_bottom(Line::from(hint).right_aligned()),
            popup.inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );

        // Inside the border, with a space of padding on each side
        let inner = popup.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });
        let viewport_height = inner.height.saturating_sub(filter_height as u16);
        let content = Rect {
            height: viewport_height,
            ..inner
        };

        let lines = self.lines(self.filter.as_deref());
        let max_scroll = lines.len().saturating_sub(usize::from(viewport_height));
        self.scroll = self.scroll.min(max_scroll);
        let line_count = lines.len();
        frame.render_widget(
            Paragraph::new(lines).scroll((self.scroll as u16, 0)),
            content,
        );

        if max_scroll > 0 {
            let mut scrollbar_state = ScrollbarState::new(max_scroll).position(self.scroll);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None),
                popup.inner(Margin {
                    horizontal: 0,
                    vertical: 1,
                }),
                &mut scrollbar_state,
            );
        }

        if let Some(filter) = &self.filter {
            let filter_area = Rect {
                y: inner.y + viewport_height,
                height: 1,
                ..inner
            };
            let text = if line_count == 0 {
                format!("/{filter}  no matches")
            } else {
                format!("/{filter}")
            };
            frame.render_widget(Paragraph::new(text), filter_area);
            if self.filter_is_capturing {
                frame.set_cursor_position((
                    filter_area.x + 1 + filter.chars().count() as u16,
                    filter_area.y,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        app::Mode,
        config::{Config, key_event_to_string},
    };

    /// The default config's bindings, as the home component keeps them
    fn default_keymaps() -> Vec<(String, String)> {
        let config: Config =
            json5::from_str(include_str!("../../../.config/config.json5")).unwrap();
        config.keybindings.0[&Mode::Home]
            .iter()
            .map(|(keys, action)| {
                let keys = keys
                    .iter()
                    .map(key_event_to_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                (keys, action.to_string())
            })
            .collect()
    }

    fn keymap(keys: &str, action: &str) -> (String, String) {
        (keys.to_string(), action.to_string())
    }

    /// The rendered rows, without trailing spaces
    fn render(popup: &mut HelpPopup, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| popup.draw(frame, frame.area()))
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

    fn press(popup: &mut HelpPopup, code: KeyCode) -> PopupOutcome {
        popup.handle_key(KeyEvent::from(code))
    }

    fn type_text(popup: &mut HelpPopup, text: &str) {
        for c in text.chars() {
            press(popup, KeyCode::Char(c));
        }
    }

    fn entry<'a>(popup: &'a HelpPopup, description: &str) -> &'a Entry {
        popup
            .sections
            .iter()
            .flat_map(|section| &section.entries)
            .find(|entry| entry.description == description)
            .unwrap()
    }

    #[test]
    fn test_every_default_binding_is_in_a_section() {
        let in_sections: Vec<String> = SECTIONS
            .iter()
            .flat_map(|spec| spec.actions)
            .map(|action| action.to_string())
            .collect();
        for (keys, action) in default_keymaps() {
            assert!(in_sections.contains(&action), "{keys} → {action}");
        }
    }

    #[test]
    fn test_every_sectioned_action_has_a_description() {
        for action in SECTIONS.iter().flat_map(|spec| spec.actions) {
            assert!(action.description().is_some(), "{action}");
        }
    }

    #[test]
    fn test_keys_are_compact_and_shortest_first() {
        let popup = HelpPopup::new(default_keymaps(), Vec::new());
        assert_eq!(entry(&popup, "Quit").keys, "q, ctrl-c");
        assert_eq!(entry(&popup, "New project").keys, "␣p");
        assert_eq!(entry(&popup, "Jump to top").keys, "gg");
        assert_eq!(entry(&popup, "Jump to bottom").keys, "G");
    }

    #[test]
    fn test_alias_is_left_out_when_its_key_is_bound_too() {
        let popup = HelpPopup::new(default_keymaps(), Vec::new());
        assert_eq!(entry(&popup, "Next session in history").keys, "tab");

        let popup = HelpPopup::new(vec![keymap("ctrl-i", "CmdNextSessionHistory")], Vec::new());
        assert_eq!(entry(&popup, "Next session in history").keys, "ctrl-i");
    }

    #[test]
    fn test_unsectioned_binding_shows_under_other() {
        let popup = HelpPopup::new(vec![keymap("ctrl-l", "ClearScreen")], Vec::new());
        let other = popup.sections.iter().find(|s| s.title == "Other").unwrap();
        assert_eq!(other.entries[0].keys, "ctrl-l");
        assert_eq!(other.entries[0].description, "ClearScreen");
    }

    #[test]
    fn test_marks_actions_for_selected_row_only_in_contextual_section() {
        let popup = HelpPopup::new(default_keymaps(), vec![Action::CmdEdit, Action::Help]);
        assert!(entry(&popup, "Edit").relevance == Relevance::Selected);
        assert!(entry(&popup, "Delete").relevance == Relevance::Unselected);
        assert!(entry(&popup, "Show this help").relevance == Relevance::Always);
    }

    #[test]
    fn test_draw_groups_entries_into_sections() {
        let mut popup = HelpPopup::new(default_keymaps(), vec![Action::CmdEdit]);
        let rows = render(&mut popup, 80, 60);
        let general = rows.iter().position(|row| row.contains("General")).unwrap();
        assert!(rows[general + 1].contains("q, ctrl-c"));
        assert!(rows[general + 1].contains("Quit"));
        assert!(
            rows.iter()
                .any(|row| row.contains("▸ ␣e") && row.contains("Edit"))
        );
        assert!(rows.iter().any(|row| row.contains("In dialogs")));
        assert!(!rows.iter().any(|row| row.contains("Cmd")));
    }

    #[test]
    fn test_draw_fits_small_terminal_and_scrolls() {
        let mut popup = HelpPopup::new(default_keymaps(), Vec::new());
        let rows = render(&mut popup, 60, 12);
        assert_eq!(rows.len(), 12);
        assert!(rows[2].contains("General"));

        press(&mut popup, KeyCode::Char('j'));
        let rows = render(&mut popup, 60, 12);
        assert!(!rows[2].contains("General"));

        // Scrolling past the end stops at the last line
        press(&mut popup, KeyCode::Char('G'));
        let rows = render(&mut popup, 60, 12);
        assert!(rows[9].contains("Cancel"));
        press(&mut popup, KeyCode::Char('k'));
        let rows = render(&mut popup, 60, 12);
        assert!(rows[9].contains("Confirm"));
    }

    #[test]
    fn test_filter_narrows_entries() {
        let mut popup = HelpPopup::new(default_keymaps(), Vec::new());
        press(&mut popup, KeyCode::Char('/'));
        type_text(&mut popup, "session");
        let rows = render(&mut popup, 80, 60);
        assert!(
            rows.iter()
                .any(|row| row.contains("New session in project"))
        );
        assert!(!rows.iter().any(|row| row.contains("Quit")));
        assert!(rows.iter().any(|row| row.contains("/session")));
    }

    #[test]
    fn test_filter_without_matches_says_so() {
        let mut popup = HelpPopup::new(default_keymaps(), Vec::new());
        press(&mut popup, KeyCode::Char('/'));
        type_text(&mut popup, "zzz");
        let rows = render(&mut popup, 80, 60);
        assert!(rows.iter().any(|row| row.contains("/zzz  no matches")));
    }

    #[test]
    fn test_filter_keys_do_not_close_or_scroll() {
        let mut popup = HelpPopup::new(default_keymaps(), Vec::new());
        press(&mut popup, KeyCode::Char('/'));
        assert!(matches!(
            press(&mut popup, KeyCode::Char('q')),
            PopupOutcome::Pending
        ));
        assert!(matches!(
            press(&mut popup, KeyCode::Char('?')),
            PopupOutcome::Pending
        ));
        assert_eq!(popup.filter.as_deref(), Some("q?"));
    }

    #[test]
    fn test_esc_clears_kept_filter_before_closing() {
        let mut popup = HelpPopup::new(default_keymaps(), Vec::new());
        press(&mut popup, KeyCode::Char('/'));
        type_text(&mut popup, "quit");
        press(&mut popup, KeyCode::Enter);
        assert_eq!(popup.filter.as_deref(), Some("quit"));
        assert!(matches!(
            press(&mut popup, KeyCode::Esc),
            PopupOutcome::Pending
        ));
        assert_eq!(popup.filter, None);
        assert!(matches!(
            press(&mut popup, KeyCode::Esc),
            PopupOutcome::Cancelled
        ));
    }

    #[test]
    fn test_question_mark_and_q_close() {
        let mut popup = HelpPopup::new(default_keymaps(), Vec::new());
        assert!(matches!(
            press(&mut popup, KeyCode::Char('?')),
            PopupOutcome::Cancelled
        ));
        assert!(matches!(
            press(&mut popup, KeyCode::Char('q')),
            PopupOutcome::Cancelled
        ));
    }
}
