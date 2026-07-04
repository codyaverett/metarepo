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
    /// Collapse the current node if expanded, else jump to and collapse its parent.
    CollapseParent,

    // Editing
    StartEdit,
    CancelEdit,
    ConfirmEdit,
    InsertChar(char),
    Backspace,
    /// Add an entry in the current context.
    Add,
    /// Delete the selected entry.
    Delete,
    /// Undo the last edit.
    Undo,

    // File operations
    Save,
    Quit,

    // Enhancements
    Search,
    /// Toggle the keybinding help overlay.
    Help,

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
        (KeyCode::Left, _) => Action::CollapseParent, // Collapse node / climb to parent
        (KeyCode::Right, _) => Action::ToggleExpand,  // Expand

        // Navigation - vim keys (optional, for power users)
        (KeyCode::Char('k'), KeyModifiers::NONE) => Action::NavigateUp,
        (KeyCode::Char('j'), KeyModifiers::NONE) => Action::NavigateDown,
        (KeyCode::Char('h'), KeyModifiers::NONE) => Action::CollapseParent,
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

        // Tree entry edits
        (KeyCode::Char('a'), KeyModifiers::NONE) => Action::Add,
        (KeyCode::Char('d'), KeyModifiers::NONE) => Action::Delete,
        (KeyCode::Char('u'), KeyModifiers::NONE) => Action::Undo,

        // File operations
        (KeyCode::Char('s'), KeyModifiers::NONE) | (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            Action::Save
        }
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => Action::Quit,

        // Force quit (Ctrl+C)
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,

        // Search
        (KeyCode::Char('/'), KeyModifiers::NONE) => Action::Search,

        // Help overlay ('?' usually arrives with SHIFT, so ignore modifiers)
        (KeyCode::Char('?'), _) => Action::Help,

        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browse(c: char) -> Action {
        handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), false)
    }

    #[test]
    fn browsing_maps_tree_edit_keys() {
        assert_eq!(browse('a'), Action::Add);
        assert_eq!(browse('d'), Action::Delete);
        assert_eq!(browse('u'), Action::Undo);
        assert_eq!(browse('e'), Action::StartEdit);
        assert_eq!(browse('/'), Action::Search);
    }

    #[test]
    fn help_ignores_shift_modifier() {
        // '?' typically arrives with SHIFT held.
        assert_eq!(
            handle_key(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
                false
            ),
            Action::Help
        );
    }

    #[test]
    fn editing_keys_do_not_trigger_browse_actions() {
        // In editing mode, 'a'/'d'/'u' are text input, not tree actions.
        assert_eq!(
            handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), true),
            Action::InsertChar('a')
        );
    }
}
