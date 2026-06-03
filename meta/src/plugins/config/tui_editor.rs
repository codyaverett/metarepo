//! TUI editor for .meta configuration files (menuconfig-style)

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use metarepo_core::{
    tui::{
        init_terminal, restore_terminal, Action, Breadcrumb, ContextBar, MenuApp, MenuAppState,
        TreeNode, TreeWidget,
    },
    ConfigSetting, ConfigValueType, MetaConfig,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::collections::HashSet;
use std::path::PathBuf;
use tui_textarea::{Input, TextArea};

/// Encode a declared setting's dotted key and value type into a `TreeNode`
/// `node_type` so `save` can route the edit back through the typed config API.
/// Form: `setting:<type-label>:<dotted-key>`.
fn setting_node_type(key: &str, vt: ConfigValueType) -> String {
    format!("setting:{}:{}", vt.label(), key)
}

/// Decode `(value_type, dotted_key)` from a setting node's `node_type`, or
/// `None` if the node is not a declared setting.
fn parse_setting_node_type(node_type: &str) -> Option<(ConfigValueType, &str)> {
    let rest = node_type.strip_prefix("setting:")?;
    let (label, key) = rest.split_once(':')?;
    Some((ConfigValueType::from_label(label)?, key))
}

/// Which script map a script node belongs to.
enum ScriptRef {
    /// A workspace-global script.
    Global(String),
    /// A script under project `proj`.
    Project { proj: String, name: String },
}

/// `node_type` for a global script entry.
fn global_script_node_type(name: &str) -> String {
    format!("script:global:{name}")
}

/// `node_type` for a per-project script entry.
fn project_script_node_type(proj: &str, name: &str) -> String {
    format!("script:project:{proj}:{name}")
}

/// Decode a script node's `node_type` into a [`ScriptRef`], or `None`.
fn parse_script_node_type(node_type: &str) -> Option<ScriptRef> {
    if let Some(name) = node_type.strip_prefix("script:global:") {
        return Some(ScriptRef::Global(name.to_string()));
    }
    let rest = node_type.strip_prefix("script:project:")?;
    // Project keys may contain '/', not ':'; the script name is the last segment.
    let (proj, name) = rest.rsplit_once(':')?;
    Some(ScriptRef::Project {
        proj: proj.to_string(),
        name: name.to_string(),
    })
}

/// Where a newly added script should live.
#[derive(Debug, Clone)]
enum AddContext {
    /// A workspace-global script.
    GlobalScript,
    /// A script under the named project.
    ProjectScript(String),
}

/// Config editor using menuconfig-style TUI
pub struct ConfigEditor {
    /// Path to the .meta file
    meta_file: PathBuf,
    /// Loaded config
    config: MetaConfig,
    /// Declared settings catalog (core + plugins + modules), kept for tree
    /// rebuilds after structural edits (add/remove).
    settings: Vec<ConfigSetting>,
    /// Dotted keys whose setting value was edited this session (and so should be
    /// written back on save).
    edited_settings: HashSet<String>,
    /// `node_type`s of script nodes whose command was edited this session.
    edited_scripts: HashSet<String>,
    /// When `Some`, a name-input prompt is open to add a new script in the given
    /// context; the `textarea` holds the name being typed.
    adding: Option<AddContext>,
    /// App state
    state: MenuAppState,
    /// Tree representation of the config
    tree_roots: Vec<TreeNode>,
    /// Text area for editing values
    textarea: Option<TextArea<'static>>,
}

impl ConfigEditor {
    /// Create a new config editor. `settings` is the aggregated catalog from
    /// [`metarepo_core::RuntimeConfig::settings_catalog`].
    pub fn new(meta_file: PathBuf, settings: Vec<ConfigSetting>) -> Result<Self> {
        let config = MetaConfig::load_from_file(&meta_file)?;
        let tree_roots = Self::build_tree(&config, &settings);

        Ok(Self {
            meta_file,
            config,
            settings,
            edited_settings: HashSet::new(),
            edited_scripts: HashSet::new(),
            adding: None,
            state: MenuAppState::new(),
            tree_roots,
            textarea: None,
        })
    }

    /// Build tree representation from config
    fn build_tree(config: &MetaConfig, settings: &[ConfigSetting]) -> Vec<TreeNode> {
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
                        let mut script_node = TreeNode::with_value(
                            script_name,
                            script_cmd,
                            project_script_node_type(name, script_name),
                        );
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
                    let mut script_node =
                        TreeNode::with_value(name, cmd, global_script_node_type(name));
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

        // Settings section — driven by the declared settings catalog so every
        // setting from core, plugins, and dynamically loaded modules is
        // editable. Grouped by namespace (the segment before the first dot).
        let mut settings_node = TreeNode::new("Settings", "section");
        settings_node.expandable = true;
        settings_node.expanded = true;
        settings_node.depth = 0;

        // Stable namespace order: first appearance in the (already sorted) catalog.
        let mut namespaces: Vec<&str> = Vec::new();
        for s in settings {
            if !namespaces.contains(&s.namespace()) {
                namespaces.push(s.namespace());
            }
        }

        for ns in namespaces {
            let mut ns_node = TreeNode::new(ns, "section");
            ns_node.depth = 1;
            ns_node.expandable = true;
            ns_node.expanded = true;

            for setting in settings.iter().filter(|s| s.namespace() == ns) {
                // Effective value: current config value, else declared default,
                // else empty. Display the short key (after the namespace).
                let current = config.get_dotted(&setting.key).map(|v| match v {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                });
                let display = current
                    .or_else(|| setting.default.clone())
                    .unwrap_or_default();
                let short = setting
                    .key
                    .strip_prefix(ns)
                    .and_then(|s| s.strip_prefix('.'))
                    .unwrap_or(&setting.key);

                let mut node = TreeNode::with_value(
                    short,
                    display,
                    setting_node_type(&setting.key, setting.value_type),
                );
                node.depth = 2;
                ns_node.add_child(node);
            }

            settings_node.add_child(ns_node);
        }

        // Core fields not owned by any plugin remain editable under "core".
        let mut core_node = TreeNode::new("core", "section");
        core_node.depth = 1;
        core_node.expandable = true;
        core_node.expanded = true;
        {
            let bare = config
                .default_bare
                .map(|b| b.to_string())
                .unwrap_or_default();
            let mut bare_node = TreeNode::with_value(
                "default_bare",
                bare,
                setting_node_type("default_bare", ConfigValueType::Bool),
            );
            bare_node.depth = 2;
            core_node.add_child(bare_node);

            let wt = config.worktree_init.clone().unwrap_or_default();
            let mut wt_node = TreeNode::with_value(
                "worktree_init",
                wt,
                setting_node_type("worktree_init", ConfigValueType::String),
            );
            wt_node.depth = 2;
            core_node.add_child(wt_node);
        }
        settings_node.add_child(core_node);

        roots.push(settings_node);

        roots
    }

    /// Apply edited settings and script commands into `self.config` (in memory,
    /// without writing the file). Shared by `save` and by structural edits that
    /// rebuild the tree, so pending value edits are not lost on rebuild.
    fn apply_pending_edits(&mut self) -> Result<()> {
        if !self.edited_settings.is_empty() {
            let updates: Vec<(ConfigValueType, String, String)> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .filter_map(|node| {
                    let (vt, key) = parse_setting_node_type(&node.node_type)?;
                    if !self.edited_settings.contains(key) {
                        return None;
                    }
                    Some((vt, key.to_string(), node.value.clone()?))
                })
                .collect();

            for (vt, key, raw) in updates {
                if raw.trim().is_empty() {
                    continue;
                }
                let parsed = vt
                    .parse(&raw)
                    .map_err(|e| anyhow::anyhow!("{}: {}", key, e))?;
                self.config = self.config.with_dotted_set(&key, parsed)?;
            }
            self.edited_settings.clear();
        }

        if !self.edited_scripts.is_empty() {
            let updates: Vec<(ScriptRef, String)> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .filter_map(|node| {
                    if !self.edited_scripts.contains(&node.node_type) {
                        return None;
                    }
                    Some((
                        parse_script_node_type(&node.node_type)?,
                        node.value.clone()?,
                    ))
                })
                .collect();

            for (target, cmd) in updates {
                self.set_script(&target, cmd);
            }
            self.edited_scripts.clear();
        }

        Ok(())
    }

    /// Insert or update a script command in the appropriate map.
    fn set_script(&mut self, target: &ScriptRef, cmd: String) {
        match target {
            ScriptRef::Global(name) => {
                self.config
                    .scripts
                    .get_or_insert_with(Default::default)
                    .insert(name.clone(), cmd);
            }
            ScriptRef::Project { proj, name } => {
                if let Some(metarepo_core::ProjectEntry::Metadata(meta)) =
                    self.config.projects.get_mut(proj)
                {
                    meta.scripts.insert(name.clone(), cmd);
                }
            }
        }
    }

    /// Rebuild the tree from the current config after a structural change,
    /// keeping the selection in range.
    fn rebuild_tree(&mut self) {
        self.tree_roots = Self::build_tree(&self.config, &self.settings);
        let visible = self.tree_roots.iter().flat_map(|r| r.flatten(true)).count();
        if self.state.tree_state.selected >= visible && visible > 0 {
            self.state.tree_state.selected = visible - 1;
        }
    }

    /// Determine where an "add" started from the current selection would place a
    /// new script. Defaults to a global script.
    fn add_context_for_selected(&self) -> AddContext {
        let node = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected);
        if let Some(node) = node {
            if let Some(rest) = node.node_type.strip_prefix("script:project:") {
                if let Some((proj, _)) = rest.rsplit_once(':') {
                    return AddContext::ProjectScript(proj.to_string());
                }
            }
            if node.node_type == "project" {
                return AddContext::ProjectScript(node.label.clone());
            }
        }
        AddContext::GlobalScript
    }

    /// Open the name-input prompt for adding a script.
    fn start_add(&mut self) {
        let ctx = self.add_context_for_selected();
        let label = match &ctx {
            AddContext::GlobalScript => "global script".to_string(),
            AddContext::ProjectScript(p) => format!("script in {p}"),
        };
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().bg(Color::Cyan));
        self.textarea = Some(textarea);
        self.adding = Some(ctx);
        self.state.editing = true;
        self.state
            .set_status(format!("New {label} name — Enter to create, Esc to cancel"));
    }

    /// Create the script entry from the typed name and select it for editing.
    fn confirm_add(&mut self) {
        let name = self
            .textarea
            .as_ref()
            .map(|t| t.lines().join("").trim().to_string())
            .unwrap_or_default();
        let ctx = self.adding.take();
        self.textarea = None;
        self.state.editing = false;

        let (ctx, name) = match (ctx, name) {
            (Some(c), n) if !n.is_empty() => (c, n),
            _ => {
                self.state.set_status("Add cancelled (empty name)");
                return;
            }
        };

        let target = match &ctx {
            AddContext::GlobalScript => ScriptRef::Global(name.clone()),
            AddContext::ProjectScript(proj) => ScriptRef::Project {
                proj: proj.clone(),
                name: name.clone(),
            },
        };

        // Reject duplicates / projects that can't hold scripts.
        let exists = match &target {
            ScriptRef::Global(n) => self
                .config
                .scripts
                .as_ref()
                .map(|m| m.contains_key(n))
                .unwrap_or(false),
            ScriptRef::Project { proj, name } => match self.config.projects.get(proj) {
                Some(metarepo_core::ProjectEntry::Metadata(m)) => m.scripts.contains_key(name),
                Some(metarepo_core::ProjectEntry::Url(_)) => {
                    self.state.set_status(format!(
                        "Project {proj} has no metadata block; cannot add scripts yet"
                    ));
                    return;
                }
                None => false,
            },
        };
        if exists {
            self.state
                .set_status(format!("Script {name} already exists"));
            return;
        }

        self.set_script(&target, String::new());
        self.state.modified = true;
        self.rebuild_tree();
        self.state
            .set_status(format!("Added {name} — edit its command, then 's' to save"));
    }

    /// Delete the selected script entry (in memory; persisted on save).
    fn delete_selected(&mut self) {
        let node_type = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)
            .map(|n| n.node_type.clone());

        let Some(target) = node_type.as_deref().and_then(parse_script_node_type) else {
            self.state
                .set_status("Delete only supports script entries for now");
            return;
        };

        let nt = node_type.unwrap();
        match &target {
            ScriptRef::Global(name) => {
                if let Some(m) = self.config.scripts.as_mut() {
                    m.remove(name);
                }
            }
            ScriptRef::Project { proj, name } => {
                if let Some(metarepo_core::ProjectEntry::Metadata(meta)) =
                    self.config.projects.get_mut(proj)
                {
                    meta.scripts.remove(name);
                }
            }
        }
        // Drop any pending edit for the removed node.
        self.edited_scripts.remove(&nt);
        self.state.modified = true;
        self.rebuild_tree();
        self.state
            .set_status("Deleted (unsaved — 's' to write, 'q' to discard)");
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
        let new_value = match &self.textarea {
            Some(textarea) => textarea.lines().join("\n"),
            None => return,
        };

        // Inspect the selected node's type immutably first.
        let node_type = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)
            .map(|n| n.node_type.clone());

        // For declared settings, validate against the declared type before
        // committing; reject invalid input and keep the editor open.
        if let Some(nt) = &node_type {
            if let Some((vt, key)) = parse_setting_node_type(nt) {
                if !new_value.trim().is_empty() {
                    if let Err(e) = vt.parse(&new_value) {
                        self.state
                            .set_status(format!("Invalid {}: {}", vt.label(), e));
                        return; // stay in edit mode
                    }
                }
                self.edited_settings.insert(key.to_string());
            } else if parse_script_node_type(nt).is_some() {
                self.edited_scripts.insert(nt.clone());
            }
        }

        // Commit the new value to the node.
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

        self.textarea = None;
        self.state.editing = false;
    }

    fn cancel_edit(&mut self) {
        self.textarea = None;
        self.state.editing = false;
        self.state.set_status("Edit cancelled");
    }

    fn save(&mut self) -> Result<()> {
        self.apply_pending_edits()?;
        self.config.save_to_file(&self.meta_file)?;
        Ok(())
    }

    /// Override handle_key to intercept keys for textarea
    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // If textarea is active, handle keys directly
        if let Some(textarea) = &mut self.textarea {
            match (key.code, key.modifiers) {
                // Enter confirms (add prompt or value edit)
                (KeyCode::Enter, KeyModifiers::NONE) => {
                    if self.adding.is_some() {
                        self.confirm_add();
                    } else {
                        self.save_edit();
                    }
                }
                // Esc cancels
                (KeyCode::Esc, _) => {
                    if self.adding.is_some() {
                        self.adding = None;
                        self.textarea = None;
                        self.state.editing = false;
                        self.state.set_status("Add cancelled");
                    } else {
                        self.cancel_edit();
                    }
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
        } else if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('a'), KeyModifiers::NONE)
        ) {
            // Add a new script in the selected context.
            self.start_add();
            Ok(true)
        } else if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('d'), KeyModifiers::NONE)
        ) {
            // Delete the selected script entry.
            self.delete_selected();
            Ok(true)
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

#[cfg(test)]
impl ConfigEditor {
    /// Test hook: set the value of the setting node with dotted `key` and mark
    /// it edited, mirroring what an interactive edit does.
    fn edit_setting_for_test(&mut self, key: &str, value: &str) {
        for ptr in self.tree_roots.iter_mut().flat_map(|r| r.flatten_all_mut()) {
            // SAFETY: pointers come from this tree and are not aliased here.
            let node = unsafe { &mut *ptr };
            if let Some((_, k)) = parse_setting_node_type(&node.node_type) {
                if k == key {
                    node.value = Some(value.to_string());
                    self.edited_settings.insert(key.to_string());
                    return;
                }
            }
        }
        panic!("no setting node for key {key}");
    }

    /// Test hook: set a script node's command (by `node_type`) and mark edited.
    fn edit_script_for_test(&mut self, node_type: &str, value: &str) {
        for ptr in self.tree_roots.iter_mut().flat_map(|r| r.flatten_all_mut()) {
            // SAFETY: pointers come from this tree and are not aliased here.
            let node = unsafe { &mut *ptr };
            if node.node_type == node_type {
                node.value = Some(value.to_string());
                self.edited_scripts.insert(node_type.to_string());
                return;
            }
        }
        panic!("no script node for {node_type}");
    }

    /// Test hook: drive the add-script flow with a typed name.
    fn add_script_for_test(&mut self, ctx: AddContext, name: &str) {
        self.adding = Some(ctx);
        self.textarea = Some(TextArea::new(vec![name.to_string()]));
        self.state.editing = true;
        self.confirm_add();
    }

    /// Test hook: expand the whole tree and select the node with `node_type`.
    fn select_for_test(&mut self, node_type: &str) {
        fn expand(node: &mut TreeNode) {
            node.expanded = true;
            for c in &mut node.children {
                expand(c);
            }
        }
        for r in &mut self.tree_roots {
            expand(r);
        }
        let idx = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .position(|n| n.node_type == node_type)
            .unwrap_or_else(|| panic!("no node {node_type}"));
        self.state.tree_state.selected = idx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn catalog() -> Vec<ConfigSetting> {
        vec![
            ConfigSetting::new("skill.dest", "d", ConfigValueType::String),
            ConfigSetting::new("skill.search-limit", "l", ConfigValueType::Integer)
                .with_default("25"),
        ]
    }

    #[test]
    fn node_type_roundtrips() {
        let nt = setting_node_type("skill.search-limit", ConfigValueType::Integer);
        let (vt, key) = parse_setting_node_type(&nt).unwrap();
        assert_eq!(vt, ConfigValueType::Integer);
        assert_eq!(key, "skill.search-limit");
        assert!(parse_setting_node_type("project:foo").is_none());
    }

    #[test]
    fn build_tree_renders_catalog_and_core() {
        let cfg = MetaConfig::default();
        let roots = ConfigEditor::build_tree(&cfg, &catalog());
        let settings = roots
            .iter()
            .find(|n| n.label == "Settings")
            .expect("Settings section");

        // Namespace group "skill" plus the "core" group.
        let groups: HashSet<&str> = settings.children.iter().map(|c| c.label.as_str()).collect();
        assert!(groups.contains("skill"));
        assert!(groups.contains("core"));

        // The search-limit node shows its default and carries the typed key.
        let skill = settings
            .children
            .iter()
            .find(|c| c.label == "skill")
            .unwrap();
        let limit = skill
            .children
            .iter()
            .find(|c| c.label == "search-limit")
            .unwrap();
        assert_eq!(limit.value.as_deref(), Some("25"));
        assert_eq!(
            parse_setting_node_type(&limit.node_type),
            Some((ConfigValueType::Integer, "skill.search-limit"))
        );
    }

    #[test]
    fn save_persists_edited_setting_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), catalog()).unwrap();
        editor.edit_setting_for_test("skill.search-limit", "50");
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        assert_eq!(
            reloaded.get_dotted("skill.search-limit"),
            Some(serde_json::json!(50))
        );
    }

    #[test]
    fn save_persists_edited_scripts_global_and_project() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(
            &path,
            r#"{
              "projects": { "app": { "url": "x", "scripts": { "build": "old" } } },
              "scripts": { "test": "old-test" }
            }"#,
        )
        .unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.edit_script_for_test(&global_script_node_type("test"), "new-test");
        editor.edit_script_for_test(&project_script_node_type("app", "build"), "new-build");
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        assert_eq!(
            reloaded.scripts.as_ref().unwrap().get("test").unwrap(),
            "new-test"
        );
        match reloaded.projects.get("app").unwrap() {
            metarepo_core::ProjectEntry::Metadata(m) => {
                assert_eq!(m.scripts.get("build").unwrap(), "new-build");
            }
            _ => panic!("expected project metadata"),
        }
    }

    #[test]
    fn add_then_edit_global_script_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.add_script_for_test(AddContext::GlobalScript, "deploy");
        editor.edit_script_for_test(&global_script_node_type("deploy"), "echo deploying");
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        assert_eq!(
            reloaded.scripts.as_ref().unwrap().get("deploy").unwrap(),
            "echo deploying"
        );
    }

    #[test]
    fn delete_global_script_removes_it_on_save() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{},"scripts":{"a":"1","b":"2"}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.select_for_test(&global_script_node_type("a"));
        editor.delete_selected();
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        let scripts = reloaded.scripts.unwrap();
        assert!(!scripts.contains_key("a"));
        assert!(scripts.contains_key("b"));
    }
}
