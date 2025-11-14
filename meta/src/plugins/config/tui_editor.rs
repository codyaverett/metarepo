//! TUI editor for .meta configuration files (menuconfig-style)

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use metarepo_core::{
    tui::{
        init_terminal, restore_terminal, Action, Breadcrumb, ContextBar, MenuApp, MenuAppState,
        TreeNode, TreeWidget,
    },
    MetaConfig,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::path::PathBuf;
use tui_textarea::{Input, TextArea};

/// Config editor using menuconfig-style TUI
pub struct ConfigEditor {
    /// Path to the .meta file
    meta_file: PathBuf,
    /// Loaded config
    config: MetaConfig,
    /// App state
    state: MenuAppState,
    /// Tree representation of the config
    tree_roots: Vec<TreeNode>,
    /// Text area for editing values
    textarea: Option<TextArea<'static>>,
}

impl ConfigEditor {
    /// Create a new config editor
    pub fn new(meta_file: PathBuf) -> Result<Self> {
        let config = MetaConfig::load_from_file(&meta_file)?;
        let tree_roots = Self::build_tree(&config);

        Ok(Self {
            meta_file,
            config,
            state: MenuAppState::new(),
            tree_roots,
            textarea: None,
        })
    }

    /// Build tree representation from config
    fn build_tree(config: &MetaConfig) -> Vec<TreeNode> {
        let mut roots = Vec::new();

        // Projects section
        let mut projects_node = TreeNode::new("Projects", "section");
        projects_node.expandable = true;
        projects_node.expanded = true;

        for (name, entry) in &config.projects {
            let url = match entry {
                metarepo_core::ProjectEntry::Url(url) => url.clone(),
                metarepo_core::ProjectEntry::Metadata(meta) => meta.url.clone(),
            };

            let mut project_node = TreeNode::with_value(name.as_str(), &url, "project");
            project_node.depth = 1;
            project_node.expandable = true;

            // Add metadata if it exists
            if let metarepo_core::ProjectEntry::Metadata(meta) = entry {
                let mut children = Vec::new();

                // URL
                let mut url_node = TreeNode::with_value("url", &meta.url, "url");
                url_node.depth = 2;
                children.push(url_node);

                // Scripts
                if !meta.scripts.is_empty() {
                    let mut scripts_node = TreeNode::new("scripts", "section");
                    scripts_node.depth = 2;
                    scripts_node.expandable = true;

                    for (script_name, script_cmd) in &meta.scripts {
                        let mut script_node =
                            TreeNode::with_value(script_name, script_cmd, "script");
                        script_node.depth = 3;
                        scripts_node.add_child(script_node);
                    }

                    children.push(scripts_node);
                }

                // Aliases
                if !meta.aliases.is_empty() {
                    let mut aliases_node = TreeNode::new("aliases", "section");
                    aliases_node.depth = 2;
                    aliases_node.expandable = true;

                    for alias in &meta.aliases {
                        let mut alias_node = TreeNode::with_value("alias", alias, "alias");
                        alias_node.depth = 3;
                        aliases_node.add_child(alias_node);
                    }

                    children.push(aliases_node);
                }

                project_node.children = children;
            }

            projects_node.add_child(project_node);
        }

        roots.push(projects_node);

        // Global scripts section
        if let Some(scripts) = &config.scripts {
            if !scripts.is_empty() {
                let mut scripts_node = TreeNode::new("Global Scripts", "section");
                scripts_node.expandable = true;

                for (name, cmd) in scripts {
                    let mut script_node = TreeNode::with_value(name, cmd, "script");
                    script_node.depth = 1;
                    scripts_node.add_child(script_node);
                }

                roots.push(scripts_node);
            }
        }

        // Global aliases section
        if let Some(aliases) = &config.aliases {
            if !aliases.is_empty() {
                let mut aliases_node = TreeNode::new("Global Aliases", "section");
                aliases_node.expandable = true;

                for (name, value) in aliases {
                    let mut alias_node = TreeNode::with_value(name, value, "alias");
                    alias_node.depth = 1;
                    aliases_node.add_child(alias_node);
                }

                roots.push(aliases_node);
            }
        }

        // Ignore patterns section
        if !config.ignore.is_empty() {
            let mut ignore_node = TreeNode::new("Ignore Patterns", "section");
            ignore_node.expandable = true;

            for pattern in &config.ignore {
                let mut pattern_node = TreeNode::with_value("pattern", pattern, "ignore");
                pattern_node.depth = 1;
                ignore_node.add_child(pattern_node);
            }

            roots.push(ignore_node);
        }

        // Settings section
        let mut settings_node = TreeNode::new("Settings", "section");
        settings_node.expandable = true;
        settings_node.depth = 0;

        if let Some(default_bare) = config.default_bare {
            let mut bare_node =
                TreeNode::with_value("default_bare", &default_bare.to_string(), "boolean");
            bare_node.depth = 1;
            settings_node.add_child(bare_node);
        }

        if let Some(worktree_init) = &config.worktree_init {
            let mut wt_node = TreeNode::with_value("worktree_init", worktree_init, "string");
            wt_node.depth = 1;
            settings_node.add_child(wt_node);
        }

        if !settings_node.children.is_empty() {
            roots.push(settings_node);
        }

        roots
    }

    /// Run the editor
    pub fn run(&mut self) -> Result<()> {
        let mut terminal = init_terminal()?;
        let result = MenuApp::run(self, &mut terminal);
        restore_terminal(terminal)?;
        result
    }
}

impl MenuApp for ConfigEditor {
    fn state(&self) -> &MenuAppState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MenuAppState {
        &mut self.state
    }

    fn get_tree_roots(&self) -> &[TreeNode] {
        &self.tree_roots
    }

    fn get_tree_roots_mut(&mut self) -> &mut Vec<TreeNode> {
        &mut self.tree_roots
    }

    fn start_editing(&mut self) {
        let visible: Vec<_> = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .collect();
        if let Some(node) = visible.get(self.state.tree_state.selected) {
            if let Some(value) = &node.value {
                let mut textarea = TextArea::new(vec![value.clone()]);
                textarea.set_cursor_line_style(Style::default());
                textarea.set_cursor_style(Style::default().bg(Color::Cyan));

                // Move cursor to the end of the line
                textarea.move_cursor(tui_textarea::CursorMove::End);

                self.textarea = Some(textarea);
                self.state.editing = true;
                self.state
                    .set_status("Editing value - Press Enter to save, Esc to cancel");
            } else {
                self.state.set_status("Cannot edit: no value for this node");
            }
        }
    }

    fn save_edit(&mut self) {
        if let Some(textarea) = &self.textarea {
            let new_value = textarea.lines().join("\n");

            // Get the selected node and update its value
            let visible_ptrs: Vec<_> = self
                .tree_roots
                .iter_mut()
                .flat_map(|r| r.flatten_mut())
                .collect();
            if let Some(&node_ptr) = visible_ptrs.get(self.state.tree_state.selected) {
                unsafe {
                    (*node_ptr).value = Some(new_value);
                }
                self.state.modified = true;
                self.state
                    .set_status("Edit saved (press 's' to write to file)");
            }
        }

        self.textarea = None;
        self.state.editing = false;
    }

    fn cancel_edit(&mut self) {
        self.textarea = None;
        self.state.editing = false;
        self.state.set_status("Edit cancelled");
    }

    fn save(&mut self) -> Result<()> {
        // Rebuild config from tree
        // TODO: Implement proper config rebuilding from tree
        // For now, just save the existing config
        self.config.save_to_file(&self.meta_file)?;
        Ok(())
    }

    /// Override handle_key to intercept keys for textarea
    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // If textarea is active, handle keys directly
        if let Some(textarea) = &mut self.textarea {
            match (key.code, key.modifiers) {
                // Enter saves and exits editing
                (KeyCode::Enter, KeyModifiers::NONE) => {
                    self.save_edit();
                }
                // Esc cancels editing
                (KeyCode::Esc, _) => {
                    self.cancel_edit();
                }
                // Ctrl+C force quits
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    self.state.should_quit = true;
                    return Ok(false);
                }
                // All other keys go to the textarea
                _ => {
                    textarea.input(Input::from(key));
                    self.state.modified = true;
                }
            }
            Ok(!self.state.should_quit)
        } else {
            // Not editing - use simple key handling
            let action = metarepo_core::tui::handle_key(key, self.state.editing);

            match action {
                Action::None => {}

                // Navigation
                Action::NavigateUp => {
                    self.state.tree_state.select_previous();
                    self.update_breadcrumb_for_selected();
                }
                Action::NavigateDown => {
                    let visible_count = self
                        .get_tree_roots()
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .count();
                    self.state.tree_state.select_next(visible_count);
                    self.update_breadcrumb_for_selected();
                }
                Action::NavigateTop => {
                    self.state.tree_state.select_first();
                    self.update_breadcrumb_for_selected();
                }
                Action::NavigateBottom => {
                    let visible_count = self
                        .get_tree_roots()
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .count();
                    self.state.tree_state.select_last(visible_count);
                    self.update_breadcrumb_for_selected();
                }
                Action::NavigatePageUp => {
                    for _ in 0..10 {
                        self.state.tree_state.select_previous();
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
                        self.state.tree_state.select_next(visible_count);
                    }
                    self.update_breadcrumb_for_selected();
                }

                // Tree operations
                Action::ToggleExpand => {
                    let selected_idx = self.state.tree_state.selected;
                    let visible: Vec<_> = self
                        .tree_roots
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .collect();

                    // Check if node is expandable first (prioritize expanding over editing)
                    if let Some(node) = visible.get(selected_idx) {
                        if node.expandable {
                            // Toggle expand/collapse for expandable nodes
                            let roots = self.get_tree_roots_mut();
                            let visible_mut: Vec<_> =
                                roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
                            if let Some(&node_ptr) = visible_mut.get(selected_idx) {
                                unsafe {
                                    (*node_ptr).toggle();
                                }
                            }
                        } else if self.is_selected_editable() {
                            // Only start editing if node is NOT expandable but IS editable
                            self.start_editing();
                        }
                    }
                }

                // Editing
                Action::StartEdit => {
                    if self.is_selected_editable() {
                        self.start_editing();
                    } else {
                        self.state.set_status("Selected item is not editable");
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
                    self.state.modified = false;
                    self.state.set_status("Saved!");
                }
                Action::Quit => {
                    // Always exit immediately - user can save with 's' before quitting
                    // This matches menuconfig-style behavior where Escape means "exit now"
                    self.state.should_quit = true;
                }

                // Not yet implemented
                Action::Search => {
                    self.state.set_status("Search not yet implemented");
                }

                // These are handled by TextArea
                Action::InsertChar(_) | Action::Backspace => {}
            }

            Ok(!self.state.should_quit)
        }
    }

    fn render_breadcrumb(&mut self, frame: &mut Frame, area: Rect) {
        let breadcrumb = Breadcrumb::new(&self.state.breadcrumb);
        frame.render_widget(breadcrumb, area);
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Tree
                Constraint::Percentage(50), // Detail/Edit panel
            ])
            .split(area);

        // Render tree
        let tree = TreeWidget::new(&self.tree_roots, &self.state.tree_state).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Config Tree ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(tree, chunks[0]);

        // Render detail/edit panel
        if let Some(textarea) = &mut self.textarea {
            // Render text editor
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Edit Value ")
                .border_style(Style::default().fg(Color::Green));

            textarea.set_block(block);
            frame.render_widget(&*textarea, chunks[1]);
        } else {
            // Show selected node details
            let visible: Vec<_> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten(true))
                .collect();
            let detail_content = if let Some(node) = visible.get(self.state.tree_state.selected) {
                vec![
                    Line::from(vec![
                        Span::styled("Selected: ", Style::default().fg(Color::Gray)),
                        Span::styled(&node.label, Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Type: ", Style::default().fg(Color::Gray)),
                        Span::raw(&node.node_type),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("Value:", Style::default().fg(Color::Gray))),
                    Line::from(node.value.as_deref().unwrap_or("(no value)")),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Enter to edit",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]
            } else {
                vec![Line::from("No item selected")]
            };

            let detail_panel = Paragraph::new(detail_content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Details ")
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: false });

            frame.render_widget(detail_panel, chunks[1]);
        }
    }

    fn render_context_bar(&mut self, frame: &mut Frame, area: Rect) {
        let context_bar = ContextBar::new(self.state.editing, self.state.modified)
            .status_message(&self.state.status_message);
        frame.render_widget(context_bar, area);
    }
}
