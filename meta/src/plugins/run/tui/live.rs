//! Live per-project run view.
//!
//! Renders snapshots of a shared [`OutputManager`] on a tick while worker
//! threads (see [`runner`](super::runner)) stream child output into it. A left
//! pane lists projects with a status glyph and duration; a right pane tails the
//! selected project's combined stdout/stderr. Uses a custom poll loop (not
//! `MenuApp`, which blocks on events and has no tick) so the view keeps updating
//! while children run.

use crate::plugins::shared::{JobStatus, OutputManager};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use metarepo_core::tui::poll_event;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use std::io::Stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Interactive live run view over a shared [`OutputManager`].
pub(crate) struct LiveRunView {
    manager: Arc<OutputManager>,
    cancel: Arc<AtomicBool>,
    projects: Vec<String>,
    script: String,
    selected: usize,
    /// Vertical scroll offset into the selected project's output.
    scroll: u16,
    /// When true, keep the output pinned to the tail as it grows.
    follow: bool,
    /// Advances each tick to animate the spinner.
    tick: usize,
    /// Set once the user has requested cancellation.
    cancelling: bool,
}

impl LiveRunView {
    pub fn new(
        manager: Arc<OutputManager>,
        cancel: Arc<AtomicBool>,
        projects: Vec<String>,
        script: String,
    ) -> Self {
        Self {
            manager,
            cancel,
            projects,
            script,
            selected: 0,
            scroll: 0,
            follow: true,
            tick: 0,
            cancelling: false,
        }
    }

    /// Drive the render/poll loop until the user quits. Returns when the view is
    /// dismissed; the caller restores the terminal and prints a final summary.
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;
            self.tick = self.tick.wrapping_add(1);

            // poll_event returns None on its ~100ms timeout, which is our tick.
            let Some(ev) = poll_event()? else {
                continue;
            };
            if let Event::Key(k) = ev {
                if k.kind == KeyEventKind::Press && self.handle_key(k.code, k.modifiers) {
                    return Ok(());
                }
            }
        }
    }

    /// Handle a key; returns true when the view should exit.
    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        let quit = matches!(code, KeyCode::Char('q') | KeyCode::Esc)
            || (code == KeyCode::Char('c') && mods.contains(KeyModifiers::CONTROL));
        if quit {
            if self.manager.all_completed() || self.cancelling {
                // Already done (or asked to cancel once) — leave.
                return true;
            }
            // First quit while jobs run: request cancellation, stay to watch.
            self.cancel.store(true, Ordering::Relaxed);
            self.cancelling = true;
            return false;
        }

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected + 1 < self.projects.len() {
                    self.selected += 1;
                    self.scroll = 0;
                    self.follow = true;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.scroll = 0;
                self.follow = true;
            }
            KeyCode::Char('f') => self.follow = !self.follow,
            KeyCode::PageUp => {
                self.follow = false;
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.follow = false;
                self.scroll = self.scroll.saturating_add(10);
            }
            _ => {}
        }
        false
    }

    /// Status glyph + color for a job.
    fn status_glyph(&self, status: &JobStatus) -> Span<'static> {
        match status {
            JobStatus::Pending => Span::styled("○", Style::default().fg(Color::DarkGray)),
            JobStatus::Running => Span::styled(
                SPINNER[self.tick % SPINNER.len()].to_string(),
                Style::default().fg(Color::Yellow),
            ),
            JobStatus::Completed => Span::styled("✓", Style::default().fg(Color::Green)),
            JobStatus::Failed => Span::styled("✗", Style::default().fg(Color::Red)),
        }
    }

    fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(f.area());

        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[0]);

        self.render_project_list(f, panes[0]);
        self.render_output(f, panes[1]);
        self.render_footer(f, chunks[1]);
    }

    fn render_project_list(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .projects
            .iter()
            .map(|name| {
                let out = self.manager.get_project_output(name);
                let status = out
                    .as_ref()
                    .map(|o| o.status.clone())
                    .unwrap_or(JobStatus::Pending);
                let dur = out
                    .as_ref()
                    .and_then(|o| o.duration)
                    .map(|d| format!(" {:.1}s", d.as_secs_f32()))
                    .unwrap_or_default();
                let line = Line::from(vec![
                    self.status_glyph(&status),
                    Span::raw(" "),
                    Span::raw(name.clone()),
                    Span::styled(dur, Style::default().fg(Color::DarkGray)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected));
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Projects "))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, area, &mut list_state);
    }

    fn render_output(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let name = self
            .projects
            .get(self.selected)
            .cloned()
            .unwrap_or_default();
        let out = self.manager.get_project_output(&name);

        let text = out
            .as_ref()
            .map(|o| {
                let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
                if !o.stderr.is_empty() {
                    if !s.is_empty() && !s.ends_with('\n') {
                        s.push('\n');
                    }
                    s.push_str(&String::from_utf8_lossy(&o.stderr));
                }
                s
            })
            .unwrap_or_default();

        // When following, pin to the tail: show the last (viewport) lines.
        let inner_h = area.height.saturating_sub(2); // borders
        let total_lines = text.lines().count() as u16;
        let scroll = if self.follow {
            total_lines.saturating_sub(inner_h)
        } else {
            self.scroll.min(total_lines.saturating_sub(1))
        };

        let title = match out.as_ref().map(|o| &o.status) {
            Some(JobStatus::Failed) => format!(" {name} (failed) "),
            Some(JobStatus::Running) => format!(" {name} (running) "),
            Some(JobStatus::Completed) => format!(" {name} (done) "),
            _ => format!(" {name} "),
        };

        let follow_hint = if self.follow { " [follow]" } else { "" };
        let para = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("{title}{follow_hint}"))
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));
        f.render_widget(para, area);
    }

    fn render_footer(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let (done, running, failed) = self.manager.get_status_summary();
        let total = self.projects.len();
        let status = if self.cancelling && !self.manager.all_completed() {
            Span::styled("cancelling…", Style::default().fg(Color::Yellow))
        } else if self.manager.all_completed() {
            Span::styled(
                format!("done: {} ok, {} failed", done - failed, failed),
                Style::default().fg(if failed > 0 { Color::Red } else { Color::Green }),
            )
        } else {
            Span::styled(
                format!("{running} running, {done}/{total} done"),
                Style::default().fg(Color::DarkGray),
            )
        };

        let quit_hint = if self.manager.all_completed() {
            "q:Quit"
        } else {
            "q:Cancel"
        };
        let line = Line::from(vec![
            Span::styled(
                self.script.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            status,
            Span::raw("   "),
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::raw(":Project  "),
            Span::styled("f", Style::default().fg(Color::Cyan)),
            Span::raw(":Follow  "),
            Span::styled("PgUp/PgDn", Style::default().fg(Color::Cyan)),
            Span::raw(":Scroll  "),
            Span::styled(quit_hint, Style::default().fg(Color::Red)),
        ]);
        f.render_widget(
            Paragraph::new(line).block(Block::default().borders(Borders::TOP)),
            area,
        );
    }
}
