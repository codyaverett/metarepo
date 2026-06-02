//! Full-screen ratatui table picker for choosing which skills to steal.
//!
//! Shows a static repo descriptor on top, a filterable / scrollable
//! `Name | Description` table where selected rows are highlighted, and a hint
//! line. Keyboard and mouse driven. The selection/filter logic lives in
//! [`PickerState`] (unit-tested); the render + event loop wraps it.

use anyhow::{anyhow, Result};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use crossterm::execute;
use metarepo_core::tui::{init_terminal, poll_event, restore_terminal};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

/// One row in the picker.
pub struct PickerItem {
    pub name: String,
    pub description: String,
    /// Whether the skill has a HIGH-severity audit finding (flagged in red).
    pub high: bool,
}

/// What a key press resolved to.
#[derive(Debug, PartialEq, Eq)]
pub enum PickerOutcome {
    Continue,
    Confirm,
    Cancel,
}

/// Selection + filter state, independent of rendering so it can be unit-tested.
pub struct PickerState {
    items: Vec<PickerItem>,
    /// Parallel to `items`: whether each underlying item is selected.
    selected: Vec<bool>,
    /// Indices into `items` currently visible under the filter.
    visible: Vec<usize>,
    /// Cursor position as an index into `visible`.
    cursor: usize,
    /// Current filter query (case-insensitive substring on name/description).
    filter: String,
}

impl PickerState {
    pub fn new(items: Vec<PickerItem>) -> Self {
        let n = items.len();
        let visible = (0..n).collect();
        Self {
            items,
            selected: vec![false; n],
            visible,
            cursor: 0,
            filter: String::new(),
        }
    }

    /// Indices of currently visible items (under the active filter).
    pub fn visible_indices(&self) -> &[usize] {
        &self.visible
    }

    /// The underlying item index under the cursor, if any.
    fn current_item(&self) -> Option<usize> {
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

    /// Toggle the item under the cursor.
    pub fn toggle(&mut self) {
        if let Some(i) = self.current_item() {
            self.selected[i] = !self.selected[i];
        }
    }

    /// Select all if any visible item is unselected, else clear all visible.
    pub fn toggle_all(&mut self) {
        let any_off = self.visible.iter().any(|&i| !self.selected[i]);
        for &i in &self.visible {
            self.selected[i] = any_off;
        }
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
                    || it.description.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
    }

    /// The selected underlying indices, in original order.
    pub fn selected_indices(&self) -> Vec<usize> {
        (0..self.items.len())
            .filter(|&i| self.selected[i])
            .collect()
    }

    /// Handle a key press, mutating state and returning the outcome.
    pub fn handle_key(&mut self, code: KeyCode) -> PickerOutcome {
        match code {
            KeyCode::Enter => PickerOutcome::Confirm,
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
            KeyCode::Char(' ') => {
                self.toggle();
                PickerOutcome::Continue
            }
            KeyCode::Backspace => {
                let mut f = self.filter.clone();
                f.pop();
                self.set_filter(&f);
                PickerOutcome::Continue
            }
            // `a` toggles all only when not actively filtering (so it can be typed
            // into a query); otherwise printable chars extend the filter.
            KeyCode::Char('a') if self.filter.is_empty() => {
                self.toggle_all();
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

/// Run the picker UI. Returns the chosen underlying item indices, or an error if
/// the user cancelled.
pub fn pick(header_lines: &[String], items: Vec<PickerItem>) -> Result<Vec<usize>> {
    if items.is_empty() {
        return Ok(vec![]);
    }
    let mut state = PickerState::new(items);

    let mut terminal = init_terminal()?;
    execute!(terminal.backend_mut(), EnableMouseCapture).ok();

    let result = run_loop(&mut terminal, &mut state, header_lines);

    execute!(terminal.backend_mut(), DisableMouseCapture).ok();
    restore_terminal(terminal)?;

    match result? {
        PickerOutcome::Confirm => Ok(state.selected_indices()),
        _ => Err(anyhow!("selection cancelled")),
    }
}

fn run_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    state: &mut PickerState,
    header_lines: &[String],
) -> Result<PickerOutcome> {
    let mut table_state = TableState::default();
    loop {
        table_state.select(Some(state.cursor));
        let table_area = draw(terminal, state, header_lines, &mut table_state)?;

        let Some(ev) = poll_event()? else {
            continue;
        };
        match ev {
            Event::Key(k) if k.kind == KeyEventKind::Press => match state.handle_key(k.code) {
                PickerOutcome::Confirm => return Ok(PickerOutcome::Confirm),
                PickerOutcome::Cancel => return Ok(PickerOutcome::Cancel),
                PickerOutcome::Continue => {}
            },
            Event::Mouse(m) => match m.kind {
                MouseEventKind::ScrollDown => state.move_down(),
                MouseEventKind::ScrollUp => state.move_up(),
                MouseEventKind::Down(MouseButton::Left) => {
                    // Map the click row (inside the table area, minus header) to a
                    // visible index, move the cursor there, and toggle it.
                    if let Some(area) = table_area {
                        if m.row > area.y && m.row < area.y + area.height {
                            let row = (m.row - area.y - 1) as usize; // -1 for header
                            if row < state.visible_indices().len() {
                                state.cursor = row;
                                state.toggle();
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Render one frame; returns the table's screen `Rect` for mouse hit-testing.
fn draw(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    state: &PickerState,
    header_lines: &[String],
    table_state: &mut TableState,
) -> Result<Option<Rect>> {
    let mut table_area: Option<Rect> = None;
    terminal.draw(|f| {
        table_area = Some(render(f, state, header_lines, table_state));
    })?;
    Ok(table_area)
}

fn render(
    f: &mut Frame,
    state: &PickerState,
    header_lines: &[String],
    table_state: &mut TableState,
) -> Rect {
    let header_h = (header_lines.len() as u16) + 2; // borders
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Length(3), // filter line (bordered)
            Constraint::Min(3),    // table
            Constraint::Length(1), // hints
        ])
        .split(f.area());

    // Header descriptor.
    let header = Paragraph::new(
        header_lines
            .iter()
            .map(|l| Line::from(l.clone()))
            .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::ALL).title(" steal "));
    f.render_widget(header, chunks[0]);

    // Filter line.
    let filter = Paragraph::new(Line::from(vec![
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::raw(state.filter.clone()),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" filter "));
    f.render_widget(filter, chunks[1]);

    // Table of skills.
    let selected_count = state.selected.iter().filter(|&&b| b).count();
    let rows: Vec<Row> = state
        .visible
        .iter()
        .map(|&i| {
            let it = &state.items[i];
            let on = state.selected[i];
            let mark = if on { "✓" } else { " " };
            let name_style = if on {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if it.high {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };
            let name = format!("{mark} {}{}", it.name, if it.high { " ⚠" } else { "" });
            Row::new(vec![
                Cell::from(name).style(name_style),
                Cell::from(it.description.clone()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Percentage(40), Constraint::Percentage(60)],
    )
    .header(
        Row::new(vec!["Skill", "Description"]).style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" skills ({selected_count} selected) ")),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(table, chunks[2], table_state);

    // Hints.
    let hints = Paragraph::new(Line::from(Span::styled(
        "type to filter · ↑/↓ move · space toggle · a all · enter confirm · esc cancel",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(hints, chunks[3]);

    chunks[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<PickerItem> {
        vec![
            PickerItem {
                name: "alpha".into(),
                description: "first thing".into(),
                high: false,
            },
            PickerItem {
                name: "bravo".into(),
                description: "second thing".into(),
                high: true,
            },
            PickerItem {
                name: "charlie".into(),
                description: "third".into(),
                high: false,
            },
        ]
    }

    #[test]
    fn toggle_and_select_indices() {
        let mut s = PickerState::new(items());
        s.toggle(); // alpha
        s.move_down();
        s.move_down();
        s.toggle(); // charlie
        assert_eq!(s.selected_indices(), vec![0, 2]);
    }

    #[test]
    fn toggle_all_then_clear() {
        let mut s = PickerState::new(items());
        s.toggle_all();
        assert_eq!(s.selected_indices(), vec![0, 1, 2]);
        s.toggle_all();
        assert!(s.selected_indices().is_empty());
    }

    #[test]
    fn filter_narrows_and_keeps_selection() {
        let mut s = PickerState::new(items());
        s.toggle(); // select alpha (index 0)
        s.set_filter("thing"); // matches alpha + bravo by description
        assert_eq!(s.visible_indices(), &[0, 1]);
        // Selection of alpha survives the filter.
        assert_eq!(s.selected_indices(), vec![0]);
        s.set_filter(""); // clear
        assert_eq!(s.visible_indices(), &[0, 1, 2]);
    }

    #[test]
    fn typing_builds_filter_and_a_is_literal_while_filtering() {
        let mut s = PickerState::new(items());
        // Empty filter: 'a' toggles all.
        assert_eq!(s.handle_key(KeyCode::Char('a')), PickerOutcome::Continue);
        assert_eq!(s.selected_indices(), vec![0, 1, 2]);
        s.toggle_all(); // clear
                        // Start a filter, then 'a' is a literal character.
        s.handle_key(KeyCode::Char('c'));
        s.handle_key(KeyCode::Char('h'));
        assert_eq!(s.visible_indices(), &[2]); // charlie
        s.handle_key(KeyCode::Char('a')); // "cha"
        assert_eq!(s.visible_indices(), &[2]);
    }

    #[test]
    fn esc_clears_filter_then_cancels() {
        let mut s = PickerState::new(items());
        s.set_filter("alp");
        assert_eq!(s.handle_key(KeyCode::Esc), PickerOutcome::Continue); // clears
        assert_eq!(s.visible_indices(), &[0, 1, 2]);
        assert_eq!(s.handle_key(KeyCode::Esc), PickerOutcome::Cancel); // then cancels
    }

    #[test]
    fn enter_confirms() {
        let mut s = PickerState::new(items());
        assert_eq!(s.handle_key(KeyCode::Enter), PickerOutcome::Confirm);
    }
}
