//! Context bar widget showing breadcrumb, help, and status

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Breadcrumb widget showing current path
pub struct Breadcrumb<'a> {
    pub path: &'a [String],
}

impl<'a> Breadcrumb<'a> {
    pub fn new(path: &'a [String]) -> Self {
        Self { path }
    }
}

impl<'a> Widget for Breadcrumb<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.path.is_empty() {
            let text = Span::styled("(root)", Style::default().fg(Color::DarkGray));
            Paragraph::new(text).render(area, buf);
            return;
        }

        let mut spans = Vec::new();

        for (i, part) in self.path.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " → ",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let style = if i == self.path.len() - 1 {
                // Last item (current selection) - bold
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                // Parent items - normal
                Style::default().fg(Color::White)
            };

            spans.push(Span::styled(part.clone(), style));
        }

        let line = Line::from(spans);
        Paragraph::new(line).render(area, buf);
    }
}

/// Context-sensitive help bar
pub struct ContextBar<'a> {
    pub editing: bool,
    pub modified: bool,
    pub status_message: &'a str,
}

impl<'a> ContextBar<'a> {
    pub fn new(editing: bool, modified: bool) -> Self {
        Self {
            editing,
            modified,
            status_message: "",
        }
    }

    pub fn status_message(mut self, msg: &'a str) -> Self {
        self.status_message = msg;
        self
    }
}

impl<'a> Widget for ContextBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Build help text based on context
        let mut help_spans = Vec::new();

        if self.editing {
            // Editing mode help
            help_spans.push(Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(":Save "));

            help_spans.push(Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(":Cancel "));
        } else {
            // Browsing mode help
            help_spans.push(Span::styled(
                "↑/↓",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(":Nav "));

            help_spans.push(Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(":Edit/Toggle "));

            help_spans.push(Span::styled(
                "S",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(":Save "));

            help_spans.push(Span::styled(
                "Q",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(":Quit "));
        }

        // Add modified indicator
        if self.modified {
            help_spans.push(Span::raw(" "));
            help_spans.push(Span::styled(
                "[+]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            help_spans.push(Span::raw(" Modified"));
        }

        let help_line = Line::from(help_spans);

        // Render help on first line
        if inner.height > 0 {
            let help_area = Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            };
            Paragraph::new(help_line).render(help_area, buf);
        }

        // Render status message on second line if present and we have enough height
        if !self.status_message.is_empty() && inner.height >= 2 {
            let status_area = Rect {
                x: inner.x,
                y: inner.y + 1,
                width: inner.width,
                height: 1,
            };

            let status_style = if self.status_message.starts_with("Error")
                || self.status_message.starts_with("Unsaved")
            {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Gray)
            };

            let status_line = Line::from(Span::styled(self.status_message, status_style));
            Paragraph::new(status_line).render(status_area, buf);
        }
    }
}
