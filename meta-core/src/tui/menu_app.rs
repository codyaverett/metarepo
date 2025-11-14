//! Simplified menuconfig-style TUI application

use super::{
    simple_keys::{handle_key, Action},
    widgets::{TreeNode, TreeState},
};
use anyhow::Result;
use crossterm::event::{Event, KeyEvent};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};

/// State for a menuconfig-style TUI application
pub struct MenuAppState {
    /// Tree navigation state
    pub tree_state: TreeState,
    /// Whether currently editing a value
    pub editing: bool,
    /// Whether there are unsaved changes
    pub modified: bool,
    /// Whether to quit the application
    pub should_quit: bool,
    /// Status message to display
    pub status_message: String,
    /// Breadcrumb path (e.g., ["Projects", "myproject", "scripts"])
    pub breadcrumb: Vec<String>,
}

impl Default for MenuAppState {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuAppState {
    pub fn new() -> Self {
        Self {
            tree_state: TreeState::new(),
            editing: false,
            modified: false,
            should_quit: false,
            status_message: String::new(),
            breadcrumb: Vec::new(),
        }
    }

    /// Set status message
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message.clear();
    }

    /// Update breadcrumb based on selected node
    pub fn update_breadcrumb(&mut self, path: Vec<String>) {
        self.breadcrumb = path;
    }
}

/// Simplified menu-driven TUI application trait
pub trait MenuApp {
    /// Get the app state
    fn state(&self) -> &MenuAppState;

    /// Get mutable app state
    fn state_mut(&mut self) -> &mut MenuAppState;

    /// Get the tree roots for rendering
    fn get_tree_roots(&self) -> &[TreeNode];

    /// Get mutable tree roots for editing
    fn get_tree_roots_mut(&mut self) -> &mut Vec<TreeNode>;

    /// Save changes to persistent storage
    fn save(&mut self) -> Result<()>;

    /// Check if the selected node is editable
    fn is_selected_editable(&self) -> bool {
        let visible: Vec<_> = self
            .get_tree_roots()
            .iter()
            .flat_map(|r| r.flatten(true))
            .collect();
        visible
            .get(self.state().tree_state.selected)
            .and_then(|node| node.value.as_ref())
            .is_some()
    }

    /// Start editing the selected node (app-specific logic)
    fn start_editing(&mut self);

    /// Save the current edit (app-specific logic)
    fn save_edit(&mut self);

    /// Cancel the current edit (app-specific logic)
    fn cancel_edit(&mut self);

    /// Handle a key event
    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        let action = handle_key(key, self.state().editing);

        match action {
            Action::None => {}

            // Navigation
            Action::NavigateUp => {
                self.state_mut().tree_state.select_previous();
                self.update_breadcrumb_for_selected();
            }
            Action::NavigateDown => {
                let visible_count = self
                    .get_tree_roots()
                    .iter()
                    .flat_map(|r| r.flatten(true))
                    .count();
                self.state_mut().tree_state.select_next(visible_count);
                self.update_breadcrumb_for_selected();
            }
            Action::NavigateTop => {
                self.state_mut().tree_state.select_first();
                self.update_breadcrumb_for_selected();
            }
            Action::NavigateBottom => {
                let visible_count = self
                    .get_tree_roots()
                    .iter()
                    .flat_map(|r| r.flatten(true))
                    .count();
                self.state_mut().tree_state.select_last(visible_count);
                self.update_breadcrumb_for_selected();
            }
            Action::NavigatePageUp => {
                for _ in 0..10 {
                    self.state_mut().tree_state.select_previous();
                }
                self.update_breadcrumb_for_selected();
            }
            Action::NavigatePageDown => {
                let visible_count = self
                    .get_tree_roots()
                    .iter()
                    .flat_map(|r| r.flatten(true))
                    .count();
                for _ in 0..10 {
                    self.state_mut().tree_state.select_next(visible_count);
                }
                self.update_breadcrumb_for_selected();
            }

            // Tree operations
            Action::ToggleExpand => {
                // If selected item is editable, start editing instead
                if self.is_selected_editable() {
                    self.start_editing();
                } else {
                    // Otherwise toggle expand/collapse
                    let selected_idx = self.state().tree_state.selected;
                    let roots = self.get_tree_roots_mut();
                    let visible: Vec<_> = roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
                    if let Some(&node_ptr) = visible.get(selected_idx) {
                        unsafe {
                            (*node_ptr).toggle();
                        }
                    }
                }
            }

            // Editing
            Action::StartEdit => {
                if self.is_selected_editable() {
                    self.start_editing();
                } else {
                    self.state_mut().set_status("Selected item is not editable");
                }
            }
            Action::ConfirmEdit => {
                self.save_edit();
            }
            Action::CancelEdit => {
                self.cancel_edit();
            }

            // File operations
            Action::Save => {
                self.save()?;
                self.state_mut().modified = false;
                self.state_mut().set_status("Saved!");
            }
            Action::Quit => {
                if self.state().modified {
                    self.state_mut()
                        .set_status("Unsaved changes! Press 'q' again to quit, 's' to save");
                    // Simple confirmation: if modified, require second quit
                    // (In real implementation, might want a proper confirmation dialog)
                } else {
                    self.state_mut().should_quit = true;
                }
            }

            // Not yet implemented
            Action::Search => {
                self.state_mut().set_status("Search not yet implemented");
            }

            // Delegated to editing mode
            Action::InsertChar(_) | Action::Backspace => {
                // These are handled by TextArea in the app implementation
            }
        }

        Ok(!self.state().should_quit)
    }

    /// Update breadcrumb for currently selected node
    fn update_breadcrumb_for_selected(&mut self) {
        let selected_idx = self.state().tree_state.selected;
        let breadcrumb = self.build_breadcrumb(selected_idx);
        self.state_mut().update_breadcrumb(breadcrumb);
    }

    /// Build breadcrumb path for a given node index
    fn build_breadcrumb(&self, selected_idx: usize) -> Vec<String> {
        let visible: Vec<_> = self
            .get_tree_roots()
            .iter()
            .flat_map(|r| r.flatten(true))
            .collect();

        if let Some(node) = visible.get(selected_idx) {
            let mut path = Vec::new();
            let mut current = *node;

            // Build path from node up to root
            loop {
                path.push(current.label.clone());

                // Find parent (this is simplified - in real implementation
                // you'd track parent pointers)
                if let Some(parent_idx) = self.find_parent_index(current, &visible) {
                    current = visible[parent_idx];
                } else {
                    break;
                }
            }

            path.reverse();
            path
        } else {
            Vec::new()
        }
    }

    /// Find parent node index (helper for breadcrumb)
    fn find_parent_index(&self, node: &TreeNode, visible: &[&TreeNode]) -> Option<usize> {
        // This is a simplified implementation
        // In practice, you'd want to track parent pointers in TreeNode
        for (i, candidate) in visible.iter().enumerate() {
            if candidate
                .children
                .iter()
                .any(|child| std::ptr::eq(child, node))
            {
                return Some(i);
            }
        }
        None
    }

    /// Render the UI
    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Breadcrumb
                Constraint::Min(1),    // Main content
                Constraint::Length(2), // Context bar (help + status)
            ])
            .split(frame.area());

        // Render breadcrumb
        self.render_breadcrumb(frame, chunks[0]);

        // Render main content
        self.render_content(frame, chunks[1]);

        // Render context bar
        self.render_context_bar(frame, chunks[2]);
    }

    /// Render breadcrumb (app can override)
    fn render_breadcrumb(&mut self, frame: &mut Frame, area: ratatui::layout::Rect);

    /// Render main content (app-specific)
    fn render_content(&mut self, frame: &mut Frame, area: ratatui::layout::Rect);

    /// Render context bar (app can override)
    fn render_context_bar(&mut self, frame: &mut Frame, area: ratatui::layout::Rect);

    /// Run the application event loop
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
