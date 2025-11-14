//! Terminal User Interface (TUI) framework for building interactive interfaces
//!
//! This module provides two TUI frameworks:
//! - Legacy vim-like modal editing (deprecated, will be removed in next major version)
//! - New menuconfig-style interface (recommended)

// Legacy vim-like interface (deprecated)
mod app;
mod keybindings;
mod modes;

// New menuconfig-style interface (recommended)
mod menu_app;
mod simple_keys;

// Widgets (shared between both interfaces)
pub mod widgets;

// Legacy exports (deprecated)
pub use app::{TuiApp, TuiAppState, TuiConfig};
pub use keybindings::{KeyAction, KeyHandler};
pub use modes::{EditorMode, Mode};

// New exports (recommended)
pub use menu_app::{MenuApp, MenuAppState};
pub use simple_keys::{handle_key, Action};

// Shared exports
pub use widgets::{Breadcrumb, ContextBar, HelpPanel, StatusBar, TreeNode, TreeState, TreeWidget};

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
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
