//! TUI application state machine

use super::{
    keybindings::{KeyAction, KeyHandler},
    modes::{EditorMode, Mode},
    widgets::{HelpPanel, StatusBar, TreeNode, TreeState},
};
use anyhow::Result;
use crossterm::event::{Event, KeyEvent};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame, Terminal,
};

/// Configuration for TUI app
#[derive(Debug, Clone)]
pub struct TuiConfig {
    /// Window title
    pub title: String,
    /// Show help panel on startup
    pub show_help: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            title: "Meta TUI".to_string(),
            show_help: false,
        }
    }
}

/// State for the TUI application
pub struct TuiAppState {
    /// Editor mode state
    pub mode: EditorMode,
    /// Tree navigation state
    pub tree_state: TreeState,
    /// Whether to show help panel
    pub show_help: bool,
    /// Status message
    pub status_message: String,
    /// Whether there are unsaved changes
    pub modified: bool,
    /// Whether to quit the application
    pub should_quit: bool,
    /// Selected node path (for editing)
    pub selected_path: Vec<usize>,
}

impl Default for TuiAppState {
    fn default() -> Self {
        Self {
            mode: EditorMode::new(),
            tree_state: TreeState::new(),
            show_help: false,
            status_message: String::new(),
            modified: false,
            should_quit: false,
            selected_path: Vec::new(),
        }
    }
}

impl TuiAppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set status message
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message.clear();
    }
}

/// Base TUI application trait
pub trait TuiApp {
    /// Get the app state
    fn state(&self) -> &TuiAppState;

    /// Get mutable app state
    fn state_mut(&mut self) -> &mut TuiAppState;

    /// Get the tree roots for rendering
    fn get_tree_roots(&self) -> &[TreeNode];

    /// Get mutable tree roots for editing
    fn get_tree_roots_mut(&mut self) -> &mut Vec<TreeNode>;

    /// Handle a key action (app-specific logic)
    fn handle_action(&mut self, action: KeyAction) -> Result<()>;

    /// Save changes
    fn save(&mut self) -> Result<()>;

    /// Validate before quit
    fn can_quit(&self) -> bool {
        !self.state().modified
    }

    /// Handle key event and return whether to continue
    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Get the action based on current mode
        let action = {
            let state = self.state();
            KeyHandler::handle(key, state.mode.mode)
        };

        match action {
            KeyAction::None => {}

            // Mode transitions
            KeyAction::EnterNormalMode => {
                let state = self.state_mut();
                state.mode.switch_to(Mode::Normal);
                state.clear_status();
            }
            KeyAction::EnterInsertMode => {
                let state = self.state_mut();
                state.mode.switch_to(Mode::Insert);
                state.set_status("-- INSERT --");
            }
            KeyAction::EnterVisualMode => {
                let state = self.state_mut();
                state.mode.switch_to(Mode::Visual);
                state.mode.visual_start = Some((state.tree_state.selected, 0));
                state.set_status("-- VISUAL --");
            }
            KeyAction::EnterCommandMode => {
                let state = self.state_mut();
                state.mode.switch_to(Mode::Command);
                state.clear_status();
            }

            // Command mode
            KeyAction::InsertChar(c) if self.state().mode.is_command() => {
                self.state_mut().mode.push_command_char(c);
            }
            KeyAction::Backspace if self.state().mode.is_command() => {
                self.state_mut().mode.pop_command_char();
            }
            KeyAction::ExecuteCommand(_) => {
                let cmd_action = {
                    let state = self.state_mut();
                    let cmd = state.mode.command().to_string();
                    let cmd_action = KeyHandler::parse_command(&cmd);
                    state.mode.switch_to(Mode::Normal);
                    state.mode.clear_command();
                    cmd_action
                };
                return self.handle_key_action(cmd_action);
            }

            // Navigation
            KeyAction::MoveUp => {
                self.state_mut().tree_state.select_previous();
            }
            KeyAction::MoveDown => {
                let visible_count = {
                    let roots = self.get_tree_roots();
                    roots.iter().flat_map(|r| r.flatten(true)).count()
                };
                self.state_mut().tree_state.select_next(visible_count);
            }
            KeyAction::MoveToTop => {
                self.state_mut().tree_state.select_first();
            }
            KeyAction::MoveToBottom => {
                let visible_count = {
                    let roots = self.get_tree_roots();
                    roots.iter().flat_map(|r| r.flatten(true)).count()
                };
                self.state_mut().tree_state.select_last(visible_count);
            }
            KeyAction::MovePageUp => {
                let state = self.state_mut();
                for _ in 0..10 {
                    state.tree_state.select_previous();
                }
            }
            KeyAction::MovePageDown => {
                let visible_count = {
                    let roots = self.get_tree_roots();
                    roots.iter().flat_map(|r| r.flatten(true)).count()
                };
                let state = self.state_mut();
                for _ in 0..10 {
                    state.tree_state.select_next(visible_count);
                }
            }

            // Tree operations
            KeyAction::ToggleNode => {
                let selected_idx = self.state().tree_state.selected;
                let roots = self.get_tree_roots_mut();
                let visible: Vec<_> = roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
                if let Some(&node_ptr) = visible.get(selected_idx) {
                    unsafe { (*node_ptr).toggle(); }
                }
            }
            KeyAction::ExpandNode => {
                let selected_idx = self.state().tree_state.selected;
                let roots = self.get_tree_roots_mut();
                let visible: Vec<_> = roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
                if let Some(&node_ptr) = visible.get(selected_idx) {
                    unsafe { (*node_ptr).expand(); }
                }
            }
            KeyAction::CollapseNode => {
                let selected_idx = self.state().tree_state.selected;
                let roots = self.get_tree_roots_mut();
                let visible: Vec<_> = roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
                if let Some(&node_ptr) = visible.get(selected_idx) {
                    unsafe { (*node_ptr).collapse(); }
                }
            }

            // Commands
            KeyAction::Save => {
                self.save()?;
                let state = self.state_mut();
                state.modified = false;
                state.set_status("Saved!");
            }
            KeyAction::Quit => {
                if !self.can_quit() {
                    self.state_mut().set_status("Error: Unsaved changes. Use :q! to force quit or :wq to save and quit");
                } else {
                    self.state_mut().should_quit = true;
                }
            }
            KeyAction::ForceQuit => {
                self.state_mut().should_quit = true;
            }
            KeyAction::SaveAndQuit => {
                self.save()?;
                let state = self.state_mut();
                state.modified = false;
                state.should_quit = true;
            }

            // Help
            KeyAction::ShowHelp => {
                let show_help = self.state().show_help;
                self.state_mut().show_help = !show_help;
            }

            // Delegate other actions to app-specific handler
            _ => {
                self.handle_action(action)?;
            }
        }

        Ok(!self.state().should_quit)
    }

    /// Helper to handle key actions
    fn handle_key_action(&mut self, action: KeyAction) -> Result<bool> {
        // Create a fake KeyEvent for the action
        // This is a bit hacky but allows us to reuse the handle_key logic
        match action {
            KeyAction::Save => {
                self.save()?;
                self.state_mut().modified = false;
                self.state_mut().set_status("Saved!");
            }
            KeyAction::Quit => {
                if !self.can_quit() {
                    self.state_mut().set_status("Error: Unsaved changes. Use :q! to force quit or :wq to save and quit");
                } else {
                    self.state_mut().should_quit = true;
                }
            }
            KeyAction::ForceQuit => {
                self.state_mut().should_quit = true;
            }
            KeyAction::SaveAndQuit => {
                self.save()?;
                self.state_mut().modified = false;
                self.state_mut().should_quit = true;
            }
            _ => {}
        }
        Ok(!self.state().should_quit)
    }

    /// Render the UI
    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),      // Main content
                Constraint::Length(1),    // Status bar
            ])
            .split(frame.area());

        // Render main content (tree or help)
        if self.state().show_help {
            let help = HelpPanel::new(self.state().mode.mode);
            frame.render_widget(help, chunks[0]);
        } else {
            self.render_content(frame, chunks[0]);
        }

        // Render status bar
        let status = StatusBar::new(self.state().mode.mode)
            .message(&self.state().status_message)
            .command(self.state().mode.command())
            .modified(self.state().modified);
        frame.render_widget(status, chunks[1]);
    }

    /// Render app-specific content
    fn render_content(&mut self, frame: &mut Frame, area: Rect);

    /// Run the TUI application
    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Some(Event::Key(key)) = super::poll_event()? {
                let should_continue = self.handle_key(key)?;
                if !should_continue {
                    break;
                }
            }
        }

        Ok(())
    }
}
