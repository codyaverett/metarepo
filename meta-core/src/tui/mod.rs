//! Terminal User Interface (TUI) framework for building interactive interfaces
//!
//! This module provides two TUI frameworks:
//! - Legacy vim-like modal editing (deprecated, will be removed in next major version)
//! - New menuconfig-style interface (recommended)

// Legacy vim-like interface (deprecated)
mod modes;
mod keybindings;
mod app;

// New menuconfig-style interface (recommended)
mod simple_keys;
mod menu_app;

// Widgets (shared between both interfaces)
pub mod widgets;

// Legacy exports (deprecated)
pub use modes::{Mode, EditorMode};
pub use keybindings::{KeyHandler, KeyAction};
pub use app::{TuiApp, TuiAppState, TuiConfig};

// New exports (recommended)
pub use simple_keys::{handle_key, Action};
pub use menu_app::{MenuApp, MenuAppState};

// Shared exports
pub use widgets::{TreeWidget, StatusBar, HelpPanel, TreeNode, TreeState, Breadcrumb, ContextBar};

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;

/// Initialize a TUI terminal with alternate screen
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    // Print newline to ensure clean prompt positioning after exit
    println!();
    Ok(())
}

/// Poll for keyboard events with timeout
pub fn poll_event() -> Result<Option<Event>> {
    if event::poll(std::time::Duration::from_millis(100))? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}
