//! Vim-like keybindings and key handling

use super::modes::Mode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions that can be triggered by key presses
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    // Mode transitions
    EnterNormalMode,
    EnterInsertMode,
    EnterVisualMode,
    EnterCommandMode,

    // Navigation (vim-style)
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveToTop,
    MoveToBottom,
    MovePageUp,
    MovePageDown,

    // Tree navigation
    ExpandNode,
    CollapseNode,
    ToggleNode,

    // Editing
    DeleteChar,
    DeleteLine,
    DeleteNode,
    InsertChar(char),
    NewLine,
    Backspace,

    // Commands
    Save,
    Quit,
    SaveAndQuit,
    ForceQuit,
    ExecuteCommand(String),

    // Visual mode
    SelectUp,
    SelectDown,
    SelectAll,

    // Misc
    ShowHelp,
    Refresh,
    Undo,
    Redo,
    Repeat,
    None,
}

/// Key handler that maps key events to actions based on current mode
pub struct KeyHandler;

impl KeyHandler {
    /// Handle a key event in the given mode and return the appropriate action
    pub fn handle(key: KeyEvent, mode: Mode) -> KeyAction {
        match mode {
            Mode::Normal => Self::handle_normal_mode(key),
            Mode::Insert => Self::handle_insert_mode(key),
            Mode::Visual => Self::handle_visual_mode(key),
            Mode::Command => Self::handle_command_mode(key),
        }
    }

    /// Handle keys in Normal mode
    fn handle_normal_mode(key: KeyEvent) -> KeyAction {
        match (key.code, key.modifiers) {
            // Mode transitions
            (KeyCode::Char('i'), KeyModifiers::NONE) => KeyAction::EnterInsertMode,
            (KeyCode::Char('v'), KeyModifiers::NONE) => KeyAction::EnterVisualMode,
            (KeyCode::Char(':'), KeyModifiers::NONE) => KeyAction::EnterCommandMode,

            // Navigation - vim style (hjkl)
            (KeyCode::Char('h'), KeyModifiers::NONE) => KeyAction::MoveLeft,
            (KeyCode::Char('j'), KeyModifiers::NONE) => KeyAction::MoveDown,
            (KeyCode::Char('k'), KeyModifiers::NONE) => KeyAction::MoveUp,
            (KeyCode::Char('l'), KeyModifiers::NONE) => KeyAction::MoveRight,

            // Navigation - arrow keys
            (KeyCode::Left, _) => KeyAction::MoveLeft,
            (KeyCode::Down, _) => KeyAction::MoveDown,
            (KeyCode::Up, _) => KeyAction::MoveUp,
            (KeyCode::Right, _) => KeyAction::MoveRight,

            // Navigation - page/home/end
            (KeyCode::Char('g'), KeyModifiers::NONE) => KeyAction::MoveToTop,
            (KeyCode::Char('G'), KeyModifiers::SHIFT) => KeyAction::MoveToBottom,
            (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                KeyAction::MovePageUp
            }
            (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                KeyAction::MovePageDown
            }
            (KeyCode::Home, _) => KeyAction::MoveToTop,
            (KeyCode::End, _) => KeyAction::MoveToBottom,

            // Tree operations
            (KeyCode::Enter, _) | (KeyCode::Char('o'), KeyModifiers::NONE) => KeyAction::ToggleNode,
            (KeyCode::Char('O'), KeyModifiers::SHIFT) => KeyAction::ExpandNode,
            (KeyCode::Char('C'), KeyModifiers::SHIFT) => KeyAction::CollapseNode,

            // Editing operations
            (KeyCode::Char('d'), KeyModifiers::NONE) => KeyAction::DeleteNode,
            (KeyCode::Char('x'), KeyModifiers::NONE) => KeyAction::DeleteChar,
            (KeyCode::Delete, _) => KeyAction::DeleteChar,

            // Quick commands
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => KeyAction::Save,
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => KeyAction::Quit,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => KeyAction::ForceQuit,
            (KeyCode::Esc, KeyModifiers::NONE) => KeyAction::Quit,

            // Undo/Redo
            (KeyCode::Char('u'), KeyModifiers::NONE) => KeyAction::Undo,
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => KeyAction::Redo,
            (KeyCode::Char('.'), KeyModifiers::NONE) => KeyAction::Repeat,

            // Help
            (KeyCode::Char('?'), KeyModifiers::NONE) => KeyAction::ShowHelp,

            // Refresh
            (KeyCode::Char('r'), KeyModifiers::NONE) => KeyAction::Refresh,

            _ => KeyAction::None,
        }
    }

    /// Handle keys in Insert mode
    fn handle_insert_mode(key: KeyEvent) -> KeyAction {
        match (key.code, key.modifiers) {
            // Exit insert mode
            (KeyCode::Esc, _) => KeyAction::EnterNormalMode,

            // Text editing
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                KeyAction::InsertChar(c)
            }
            (KeyCode::Backspace, _) => KeyAction::Backspace,
            (KeyCode::Delete, _) => KeyAction::DeleteChar,
            (KeyCode::Enter, _) => KeyAction::NewLine,

            // Navigation in insert mode
            (KeyCode::Left, _) => KeyAction::MoveLeft,
            (KeyCode::Right, _) => KeyAction::MoveRight,
            (KeyCode::Up, _) => KeyAction::MoveUp,
            (KeyCode::Down, _) => KeyAction::MoveDown,

            _ => KeyAction::None,
        }
    }

    /// Handle keys in Visual mode
    fn handle_visual_mode(key: KeyEvent) -> KeyAction {
        match (key.code, key.modifiers) {
            // Exit visual mode
            (KeyCode::Esc, _) | (KeyCode::Char('v'), KeyModifiers::NONE) => {
                KeyAction::EnterNormalMode
            }

            // Selection navigation - vim style
            (KeyCode::Char('h'), KeyModifiers::NONE) => KeyAction::MoveLeft,
            (KeyCode::Char('j'), KeyModifiers::NONE) => KeyAction::SelectDown,
            (KeyCode::Char('k'), KeyModifiers::NONE) => KeyAction::SelectUp,
            (KeyCode::Char('l'), KeyModifiers::NONE) => KeyAction::MoveRight,

            // Selection navigation - arrow keys
            (KeyCode::Up, _) => KeyAction::SelectUp,
            (KeyCode::Down, _) => KeyAction::SelectDown,

            // Select all
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => KeyAction::SelectAll,

            // Delete selection
            (KeyCode::Char('d'), KeyModifiers::NONE) => KeyAction::DeleteNode,
            (KeyCode::Delete, _) | (KeyCode::Char('x'), KeyModifiers::NONE) => {
                KeyAction::DeleteNode
            }

            _ => KeyAction::None,
        }
    }

    /// Handle keys in Command mode
    fn handle_command_mode(key: KeyEvent) -> KeyAction {
        match (key.code, key.modifiers) {
            // Exit command mode
            (KeyCode::Esc, _) => KeyAction::EnterNormalMode,

            // Execute command
            (KeyCode::Enter, _) => KeyAction::ExecuteCommand(String::new()), // Command will be filled by app

            // Edit command
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                KeyAction::InsertChar(c)
            }
            (KeyCode::Backspace, _) => KeyAction::Backspace,

            _ => KeyAction::None,
        }
    }

    /// Parse a vim command string (e.g., "w", "q", "wq", "q!")
    pub fn parse_command(cmd: &str) -> KeyAction {
        let cmd = cmd.trim();
        match cmd {
            "w" | "write" => KeyAction::Save,
            "q" | "quit" => KeyAction::Quit,
            "wq" | "x" => KeyAction::SaveAndQuit,
            "q!" | "quit!" => KeyAction::ForceQuit,
            _ => KeyAction::None,
        }
    }
}
