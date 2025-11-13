//! Status bar widget for displaying mode and hints

use crate::tui::modes::Mode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Status bar widget
pub struct StatusBar<'a> {
    /// Current mode
    pub mode: Mode,
    /// Status message
    pub message: &'a str,
    /// Command buffer (for command mode)
    pub command: &'a str,
    /// Whether there are unsaved changes
    pub modified: bool,
}

impl<'a> StatusBar<'a> {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            message: "",
            command: "",
            modified: false,
        }
    }

    pub fn message(mut self, msg: &'a str) -> Self {
        self.message = msg;
        self
    }

    pub fn command(mut self, cmd: &'a str) -> Self {
        self.command = cmd;
        self
    }

    pub fn modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Mode indicator on the left
        let mode_style = Style::default()
            .fg(Color::Black)
            .bg(self.mode.color())
            .add_modifier(Modifier::BOLD);

        let mode_text = format!(" {} ", self.mode.name());
        let mode_width = mode_text.len() as u16;

        // Modified indicator
        let modified_text = if self.modified { " [+]" } else { "" };

        // Message in the middle
        let message_text = if self.mode == Mode::Command {
            format!(":{}", self.command)
        } else {
            self.message.to_string()
        };

        // Compose the status line
        let mut spans = vec![Span::styled(mode_text, mode_style)];

        if !modified_text.is_empty() {
            spans.push(Span::styled(
                modified_text,
                Style::default().fg(Color::Yellow),
            ));
        }

        if !message_text.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::raw(message_text));
        }

        // Render the line
        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);

        // Fill the rest with the mode background
        let bg_style = Style::default().bg(Color::DarkGray);
        for x in area.x + mode_width..area.right() {
            buf[(x, area.y)].set_style(bg_style);
        }
    }
}
