//! Fuzzy single-select picker for choosing a workspace script to run.
//!
//! Modeled on the skill picker but single-select: Enter confirms the script row
//! under the cursor. Printable keys extend a case-insensitive filter over the
//! script name and command, so this surface deliberately does NOT use the shared
//! `Action` keymap. The selection/filter logic lives in [`PickerState`]
//! (unit-tested); the render + event loop wraps it.

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use metarepo_core::tui::poll_event;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::{Frame, Terminal};
use std::io::Stdout;

use super::super::ScriptInfo;

/// What a key press resolved to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PickerOutcome {
    Continue,
    Confirm,
    Cancel,
}

/// Filter + cursor state, independent of rendering so it can be unit-tested.
pub(crate) struct PickerState {
    items: Vec<ScriptInfo>,
    /// Indices into `items` currently visible under the filter.
    visible: Vec<usize>,
    /// Cursor position as an index into `visible`.
    cursor: usize,
    /// Current filter query (case-insensitive substring on name/command).
    filter: String,
}

impl PickerState {
    pub fn new(items: Vec<ScriptInfo>) -> Self {
        let visible = (0..items.len()).collect();
        Self {
            items,
            visible,
            cursor: 0,
            filter: String::new(),
        }
    }

    #[cfg(test)]
    pub fn visible_indices(&self) -> &[usize] {
        &self.visible
    }

    /// The underlying item index under the cursor, if any.
    pub fn current_item(&self) -> Option<usize> {
        self.visible.get(self.cursor).copied()
    }

    pub fn move_down(&mut self) {
        if !self.visible.is_empty() {
            self.cursor = (self.cursor + 1).min(self.visible.len() - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Apply a filter query, recomputing visible rows and clamping the cursor.
    pub fn set_filter(&mut self, query: &str) {
        self.filter = query.to_string();
        let q = query.to_lowercase();
        self.visible = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                q.is_empty()
                    || it.name.to_lowercase().contains(&q)
                    || it.command.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
    }

    /// Handle a key press, mutating state and returning the outcome.
    pub fn handle_key(&mut self, code: KeyCode) -> PickerOutcome {
        match code {
            KeyCode::Enter => {
                if self.current_item().is_some() {
                    PickerOutcome::Confirm
                } else {
                    PickerOutcome::Continue
                }
            }
            KeyCode::Esc => {
                if self.filter.is_empty() {
                    PickerOutcome::Cancel
                } else {
                    self.set_filter("");
                    PickerOutcome::Continue
                }
            }
            KeyCode::Down => {
                self.move_down();
                PickerOutcome::Continue
            }
            KeyCode::Up => {
                self.move_up();
                PickerOutcome::Continue
            }
            KeyCode::Backspace => {
                let mut f = self.filter.clone();
                f.pop();
                self.set_filter(&f);
                PickerOutcome::Continue
            }
            KeyCode::Char(c) if !c.is_control() => {
                let mut f = self.filter.clone();
                f.push(c);
                self.set_filter(&f);
                PickerOutcome::Continue
            }
            _ => PickerOutcome::Continue,
        }
    }
}

/// Run the picker on an already-initialized `terminal` (shared with the run
/// view). Returns the chosen underlying script index, or `None` if cancelled.
pub(crate) fn pick_script(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    items: Vec<ScriptInfo>,
) -> Result<Option<usize>> {
    if items.is_empty() {
        return Ok(None);
    }
    let mut state = PickerState::new(items);
    let mut table_state = TableState::default();

    loop {
        table_state.select(Some(state.cursor));
        terminal.draw(|f| render(f, &state, &mut table_state))?;

        let Some(ev) = poll_event()? else {
            continue;
        };
        if let Event::Key(k) = ev {
            if k.kind == KeyEventKind::Press {
                match state.handle_key(k.code) {
                    PickerOutcome::Confirm => return Ok(state.current_item()),
                    PickerOutcome::Cancel => return Ok(None),
                    PickerOutcome::Continue => {}
                }
            }
        }
    }
}

fn render(f: &mut Frame, state: &PickerState, table_state: &mut TableState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(3), // filter line (bordered)
            Constraint::Min(3),    // table
            Constraint::Length(1), // hints
        ])
        .split(f.area());

    let title = Paragraph::new(Line::from(Span::styled(
        "Run a script",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    f.render_widget(title, chunks[0]);

    let filter = Paragraph::new(Line::from(vec![
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::raw(state.filter.clone()),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" filter "));
    f.render_widget(filter, chunks[1]);

    let rows: Vec<Row> = state
        .visible
        .iter()
        .map(|&i| {
            let it = &state.items[i];
            Row::new(vec![
                Cell::from(it.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(it.command.clone()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(it.projects.len().to_string()).style(Style::default().fg(Color::Cyan)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(55),
            Constraint::Percentage(15),
        ],
    )
    .header(
        Row::new(vec!["Script", "Command", "Projects"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" scripts ({}) ", state.visible.len())),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(table, chunks[2], table_state);

    let hints = Paragraph::new(Line::from(Span::styled(
        "type to filter · ↑/↓ move · enter run · esc cancel",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(hints, chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<ScriptInfo> {
        vec![
            ScriptInfo {
                name: "build".into(),
                command: "cargo build".into(),
                projects: vec!["a".into()],
            },
            ScriptInfo {
                name: "test".into(),
                command: "cargo test".into(),
                projects: vec!["a".into(), "b".into()],
            },
            ScriptInfo {
                name: "lint".into(),
                command: "cargo clippy".into(),
                projects: vec!["b".into()],
            },
        ]
    }

    #[test]
    fn filter_narrows_by_name_and_command() {
        let mut s = PickerState::new(items());
        // "cargo" matches every command.
        s.set_filter("cargo");
        assert_eq!(s.visible_indices(), &[0, 1, 2]);
        // "build" matches only the build script.
        s.set_filter("build");
        assert_eq!(s.visible_indices(), &[0]);
    }

    #[test]
    fn typing_builds_filter_then_enter_confirms_current() {
        let mut s = PickerState::new(items());
        s.handle_key(KeyCode::Char('l')); // "l" -> lint (name) + build/clippy? only lint name, clippy has no l... "clippy" has no 'l'? c-l-i-p yes 'l'
                                          // "l" matches "lint" and "clippy"
        assert!(s.visible_indices().contains(&2));
        s.set_filter("lint");
        assert_eq!(s.current_item(), Some(2));
        assert_eq!(s.handle_key(KeyCode::Enter), PickerOutcome::Confirm);
    }

    #[test]
    fn esc_clears_filter_then_cancels() {
        let mut s = PickerState::new(items());
        s.set_filter("build");
        assert_eq!(s.handle_key(KeyCode::Esc), PickerOutcome::Continue);
        assert_eq!(s.visible_indices(), &[0, 1, 2]);
        assert_eq!(s.handle_key(KeyCode::Esc), PickerOutcome::Cancel);
    }

    #[test]
    fn enter_on_empty_filter_result_does_not_confirm() {
        let mut s = PickerState::new(items());
        s.set_filter("zzz"); // matches nothing
        assert_eq!(s.visible_indices().len(), 0);
        assert_eq!(s.handle_key(KeyCode::Enter), PickerOutcome::Continue);
    }
}
