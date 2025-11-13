//! Help panel widget

use crate::tui::modes::Mode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Help panel showing keybindings
pub struct HelpPanel {
    /// Current mode (affects which help to show)
    pub mode: Mode,
}

impl HelpPanel {
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }

    /// Get help text for the current mode
    fn get_help_lines(&self) -> Vec<Line> {
        let title = Line::from(vec![
            Span::styled("Keybindings", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" - Press "),
            Span::styled("?", Style::default().fg(Color::Cyan)),
            Span::raw(" to close"),
        ]);

        let mut lines = vec![title, Line::from("")];

        match self.mode {
            Mode::Normal => {
                lines.extend(vec![
                    Line::from(vec![
                        Span::styled("Navigation:", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from("  h/j/k/l or ←/↓/↑/→  Move cursor"),
                    Line::from("  g / G               Jump to top / bottom"),
                    Line::from("  Ctrl+u / Ctrl+d     Page up / down"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Tree:", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from("  Enter or o          Toggle expand/collapse"),
                    Line::from("  O                   Expand node"),
                    Line::from("  C                   Collapse node"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Editing:", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from("  i                   Enter insert mode"),
                    Line::from("  v                   Enter visual mode"),
                    Line::from("  d                   Delete node"),
                    Line::from("  x                   Delete character"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Commands:", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from("  :                   Enter command mode"),
                    Line::from("  Ctrl+w              Save"),
                    Line::from("  Ctrl+q              Quit"),
                    Line::from("  ?                   Show/hide help"),
                ]);
            }
            Mode::Insert => {
                lines.extend(vec![
                    Line::from(vec![
                        Span::styled("Insert Mode:", Style::default().fg(Color::Green)),
                    ]),
                    Line::from("  Esc                 Return to normal mode"),
                    Line::from("  Typing              Insert text"),
                    Line::from("  Backspace           Delete previous char"),
                    Line::from("  Enter               New line / confirm"),
                    Line::from("  ←/→/↑/↓             Navigate"),
                ]);
            }
            Mode::Visual => {
                lines.extend(vec![
                    Line::from(vec![
                        Span::styled("Visual Mode:", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from("  Esc or v            Return to normal mode"),
                    Line::from("  j/k or ↓/↑          Extend selection"),
                    Line::from("  Ctrl+a              Select all"),
                    Line::from("  d or x              Delete selection"),
                ]);
            }
            Mode::Command => {
                lines.extend(vec![
                    Line::from(vec![
                        Span::styled("Command Mode:", Style::default().fg(Color::Magenta)),
                    ]),
                    Line::from("  Esc                 Cancel command"),
                    Line::from("  Enter               Execute command"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Commands:", Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from("  :w or :write        Save changes"),
                    Line::from("  :q or :quit         Quit (fails if modified)"),
                    Line::from("  :q! or :quit!       Force quit (discard changes)"),
                    Line::from("  :wq or :x           Save and quit"),
                ]);
            }
        }

        lines
    }
}

impl Widget for HelpPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.get_help_lines();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Help ");

        let paragraph = Paragraph::new(lines).block(block);

        Widget::render(paragraph, area, buf);
    }
}
