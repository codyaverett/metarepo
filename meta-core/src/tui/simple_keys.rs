//! Simple, context-aware key handling for menuconfig-style interface

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions that can be triggered by key presses
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Navigation
    NavigateUp,
    NavigateDown,
    NavigatePageUp,
    NavigatePageDown,
    NavigateTop,
    NavigateBottom,

    // Tree operations
    ToggleExpand,

    // Editing
    StartEdit,
    CancelEdit,
    ConfirmEdit,
    InsertChar(char),
    Backspace,

    // File operations
    Save,
    Quit,

    // Future enhancements
    Search,

    // No-op
    None,
}

/// Handle key events based on context (editing vs browsing)
pub fn handle_key(key: KeyEvent, editing: bool) -> Action {
    if editing {
        handle_editing_keys(key)
    } else {
        handle_browsing_keys(key)
    }
}

/// Handle keys when editing a value
fn handle_editing_keys(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        // Confirm edit
        (KeyCode::Enter, KeyModifiers::NONE) => Action::ConfirmEdit,

        // Cancel edit
        (KeyCode::Esc, _) => Action::CancelEdit,

        // Text input
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => Action::InsertChar(c),

        // Backspace
        (KeyCode::Backspace, _) => Action::Backspace,

        // Force quit (Ctrl+C)
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,

        _ => Action::None,
    }
}

/// Handle keys when browsing the tree
fn handle_browsing_keys(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        // Navigation - arrow keys
        (KeyCode::Up, _) => Action::NavigateUp,
        (KeyCode::Down, _) => Action::NavigateDown,
        (KeyCode::Left, _) => Action::ToggleExpand, // Collapse
        (KeyCode::Right, _) => Action::ToggleExpand, // Expand

        // Navigation - vim keys (optional, for power users)
        (KeyCode::Char('k'), KeyModifiers::NONE) => Action::NavigateUp,
        (KeyCode::Char('j'), KeyModifiers::NONE) => Action::NavigateDown,
        (KeyCode::Char('h'), KeyModifiers::NONE) => Action::ToggleExpand,
        (KeyCode::Char('l'), KeyModifiers::NONE) => Action::ToggleExpand,

        // Navigation - page/home/end
        (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Action::NavigatePageUp
        }
        (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Action::NavigatePageDown
        }
        (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => Action::NavigateTop,
        (KeyCode::End, _) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => Action::NavigateBottom,

        // Toggle expand/collapse or start editing
        (KeyCode::Enter, _) | (KeyCode::Char(' '), KeyModifiers::NONE) => Action::ToggleExpand,

        // Start editing (only for editable items)
        (KeyCode::Char('e'), KeyModifiers::NONE) => Action::StartEdit,

        // File operations
        (KeyCode::Char('s'), KeyModifiers::NONE) | (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            Action::Save
        }
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => Action::Quit,

        // Force quit (Ctrl+C)
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,

        // Search (future)
        (KeyCode::Char('/'), KeyModifiers::NONE) => Action::Search,

        _ => Action::None,
    }
}
