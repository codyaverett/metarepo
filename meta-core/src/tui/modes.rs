//! Vim-like modal editing system

use std::fmt;

/// Editor modes similar to vim
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal mode: navigation and commands
    Normal,
    /// Insert/Edit mode: text editing
    Insert,
    /// Visual mode: selection
    Visual,
    /// Command mode: vim-style commands (:w, :q, etc.)
    Command,
}

impl Mode {
    /// Get the display name for the mode
    pub fn name(&self) -> &str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::Command => "COMMAND",
        }
    }

    /// Get the display color for the mode
    pub fn color(&self) -> ratatui::style::Color {
        match self {
            Mode::Normal => ratatui::style::Color::Cyan,
            Mode::Insert => ratatui::style::Color::Green,
            Mode::Visual => ratatui::style::Color::Yellow,
            Mode::Command => ratatui::style::Color::Magenta,
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Extended editor mode with additional state
#[derive(Debug, Clone)]
pub struct EditorMode {
    /// Current mode
    pub mode: Mode,
    /// Command buffer for Command mode
    pub command_buffer: String,
    /// Visual mode selection start (row, col)
    pub visual_start: Option<(usize, usize)>,
    /// Last action for repeat (.)
    pub last_action: Option<String>,
}

impl Default for EditorMode {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            command_buffer: String::new(),
            visual_start: None,
            last_action: None,
        }
    }
}

impl EditorMode {
    /// Create a new editor mode in Normal mode
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch to a different mode
    pub fn switch_to(&mut self, mode: Mode) {
        self.mode = mode;

        // Reset mode-specific state
        match mode {
            Mode::Normal => {
                self.visual_start = None;
            }
            Mode::Command => {
                self.command_buffer.clear();
            }
            _ => {}
        }
    }

    /// Check if in normal mode
    pub fn is_normal(&self) -> bool {
        self.mode == Mode::Normal
    }

    /// Check if in insert mode
    pub fn is_insert(&self) -> bool {
        self.mode == Mode::Insert
    }

    /// Check if in visual mode
    pub fn is_visual(&self) -> bool {
        self.mode == Mode::Visual
    }

    /// Check if in command mode
    pub fn is_command(&self) -> bool {
        self.mode == Mode::Command
    }

    /// Add character to command buffer
    pub fn push_command_char(&mut self, c: char) {
        if self.is_command() {
            self.command_buffer.push(c);
        }
    }

    /// Remove last character from command buffer
    pub fn pop_command_char(&mut self) {
        if self.is_command() {
            self.command_buffer.pop();
        }
    }

    /// Get the current command buffer
    pub fn command(&self) -> &str {
        &self.command_buffer
    }

    /// Clear the command buffer
    pub fn clear_command(&mut self) {
        self.command_buffer.clear();
    }
}
