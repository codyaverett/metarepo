//! TUI editor for .meta configuration files (menuconfig-style)

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use metarepo_core::{
    tui::{
        centered_rect, init_terminal, render_tree_pane, restore_terminal, search_and_reveal,
        Action, Breadcrumb, ContextBar, HelpSection, KeybindingHelp, MenuApp, MenuAppState,
        TreeNode,
    },
    ConfigSetting, ConfigValueType, MetaConfig,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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

/// `node_type` for a project's URL field.
fn project_url_node_type(proj: &str) -> String {
    format!("url:project:{proj}")
}

/// The project a URL node belongs to, or `None`.
fn parse_project_url_node_type(node_type: &str) -> Option<&str> {
    node_type.strip_prefix("url:project:")
}

/// Replace `old` with `new` in a string vec, or remove `old` when `new` is
/// blank. No-op if `old` is absent.
fn replace_in_vec(vec: &mut Vec<String>, old: &str, new: &str) {
    if let Some(pos) = vec.iter().position(|x| x == old) {
        if new.trim().is_empty() {
            vec.remove(pos);
        } else {
            vec[pos] = new.to_string();
        }
    }
}

/// `node_type` for a global ignore pattern (value-keyed list item).
fn ignore_item_node_type(val: &str) -> String {
    format!("ignoreitem:{val}")
}

/// The pattern an ignore-item node carries, or `None`.
fn parse_ignore_item(node_type: &str) -> Option<&str> {
    node_type.strip_prefix("ignoreitem:")
}

/// `node_type` for a per-project alias (value-keyed list item).
fn project_alias_item_node_type(proj: &str, val: &str) -> String {
    format!("paliasitem:{proj}:{val}")
}

/// Decode a project alias-item node into `(proj, value)`, or `None`. Project
/// keys never contain ':', so the first ':' splits project from value.
fn parse_project_alias_item(node_type: &str) -> Option<(&str, &str)> {
    node_type.strip_prefix("paliasitem:")?.split_once(':')
}

/// `node_type` for a global alias (alias name → project path).
fn global_alias_node_type(name: &str) -> String {
    format!("alias:global:{name}")
}

/// The alias name a global-alias node carries, or `None`.
fn parse_global_alias_node_type(node_type: &str) -> Option<&str> {
    node_type.strip_prefix("alias:global:")
}

/// `node_type` for a per-project environment variable.
fn project_env_node_type(proj: &str, key: &str) -> String {
    format!("env:project:{proj}:{key}")
}

/// Decode a project env var node's `node_type` into `(proj, key)`, or `None`.
fn parse_project_env_node_type(node_type: &str) -> Option<(&str, &str)> {
    let rest = node_type.strip_prefix("env:project:")?;
    rest.rsplit_once(':')
}

/// Where a newly added entry should live. For context-filtered adds the project
/// is already known.
#[derive(Debug, Clone, PartialEq)]
enum AddContext {
    /// A workspace-global script.
    GlobalScript,
    /// A script under the named project.
    ProjectScript(String),
    /// An environment variable under the named project.
    ProjectEnv(String),
    /// A workspace-global alias (alias name → project path).
    GlobalAlias,
    /// A workspace ignore pattern (list item).
    IgnorePattern,
    /// An alias under the named project (list item).
    ProjectAlias(String),
    /// A new project in the workspace.
    NewProject,
}

impl AddContext {
    /// Short label for the add-type selector.
    fn menu_label(&self) -> String {
        match self {
            AddContext::GlobalScript => "Global script".to_string(),
            AddContext::ProjectScript(p) => format!("Script in {p}"),
            AddContext::ProjectEnv(p) => format!("Env var in {p}"),
            AddContext::GlobalAlias => "Global alias".to_string(),
            AddContext::IgnorePattern => "Ignore pattern".to_string(),
            AddContext::ProjectAlias(p) => format!("Alias in {p}"),
            AddContext::NewProject => "New project".to_string(),
        }
    }

    /// What the name prompt is collecting.
    fn name_prompt(&self) -> String {
        match self {
            AddContext::GlobalScript => "global script name".to_string(),
            AddContext::ProjectScript(p) => format!("script name in {p}"),
            AddContext::ProjectEnv(p) => format!("env var name in {p}"),
            AddContext::GlobalAlias => "global alias name".to_string(),
            AddContext::IgnorePattern => "ignore pattern".to_string(),
            AddContext::ProjectAlias(p) => format!("alias in {p}"),
            AddContext::NewProject => "project name".to_string(),
        }
    }
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
    /// Project names whose URL was edited this session.
    edited_urls: HashSet<String>,
    /// `node_type`s of env var nodes whose value was edited this session.
    edited_env: HashSet<String>,
    /// `node_type`s of global-alias nodes whose target was edited this session.
    edited_aliases: HashSet<String>,
    /// `node_type`s of value-keyed list items (ignore patterns, project
    /// aliases) edited this session; the node_type encodes the *old* value.
    edited_list: HashSet<String>,
    /// When `Some`, the add-type selector is open: the candidate contexts and
    /// the highlighted index.
    add_menu: Option<(Vec<AddContext>, usize)>,
    /// True while a "discard unsaved changes?" quit confirmation is showing.
    confirm_quit: bool,
    /// True while the search prompt is open (the `textarea` holds the query).
    searching: bool,
    /// When `Some`, a name-input prompt is open to add an entry in the given
    /// context; the `textarea` holds the name being typed.
    adding: Option<AddContext>,
    /// App state
    state: MenuAppState,
    /// Tree representation of the config
    tree_roots: Vec<TreeNode>,
    /// Text area for editing values
    textarea: Option<TextArea<'static>>,
    /// Enclosing `.meta` configs (outermost → nearest-1) whose settings this
    /// workspace inherits. Empty in a flat (non-nested) workspace. Used to
    /// annotate inherited-vs-local values in the tree.
    ancestors: Vec<(PathBuf, MetaConfig)>,
    /// Whether the `?` keybinding help overlay is showing.
    show_help: bool,
    /// `node_type`s of nodes with an unsaved edit this session, so the tree can
    /// mark them. Persists across rebuilds; cleared on save.
    dirty: HashSet<String>,
    /// Single-level undo: a snapshot of the mutable editor state captured before
    /// the last value edit or override. `None` when there is nothing to undo.
    undo_snapshot: Option<EditorSnapshot>,
}

/// A snapshot of the editor's mutable state for single-level undo.
#[derive(Clone)]
struct EditorSnapshot {
    config: MetaConfig,
    tree_roots: Vec<TreeNode>,
    edited_settings: HashSet<String>,
    edited_scripts: HashSet<String>,
    edited_urls: HashSet<String>,
    edited_env: HashSet<String>,
    edited_aliases: HashSet<String>,
    edited_list: HashSet<String>,
    dirty: HashSet<String>,
    modified: bool,
    selected: usize,
}

impl ConfigEditor {
    /// Scroll the tree so the selected row stays on screen after navigation.
    fn sync_tree_scroll(&mut self) {
        let height = self.state.tree_state.viewport_height;
        self.state.tree_state.update_offset(height);
    }

    /// Collapse the node at the given flattened index (no selection change).
    fn collapse_at(&mut self, idx: usize) {
        let roots = self.get_tree_roots_mut();
        let visible: Vec<_> = roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
        if let Some(&node_ptr) = visible.get(idx) {
            unsafe {
                (*node_ptr).collapse();
            }
        }
    }

    /// Create a new config editor. `settings` is the aggregated catalog from
    /// [`metarepo_core::RuntimeConfig::settings_catalog`].
    pub fn new(meta_file: PathBuf, settings: Vec<ConfigSetting>) -> Result<Self> {
        let config = MetaConfig::load_from_file(&meta_file)?;
        let ancestors = Self::load_ancestors(&meta_file);
        let tree_roots = Self::build_tree(&config, &settings, &ancestors);

        Ok(Self {
            meta_file,
            config,
            settings,
            edited_settings: HashSet::new(),
            edited_scripts: HashSet::new(),
            edited_urls: HashSet::new(),
            edited_env: HashSet::new(),
            edited_aliases: HashSet::new(),
            edited_list: HashSet::new(),
            add_menu: None,
            confirm_quit: false,
            searching: false,
            adding: None,
            state: MenuAppState::new(),
            tree_roots,
            textarea: None,
            ancestors,
            show_help: false,
            dirty: HashSet::new(),
            undo_snapshot: None,
        })
    }

    /// Capture the current mutable state for single-level undo, overwriting any
    /// prior snapshot. Called before a value edit or override commits.
    fn snapshot_for_undo(&mut self) {
        self.undo_snapshot = Some(EditorSnapshot {
            config: self.config.clone(),
            tree_roots: self.tree_roots.clone(),
            edited_settings: self.edited_settings.clone(),
            edited_scripts: self.edited_scripts.clone(),
            edited_urls: self.edited_urls.clone(),
            edited_env: self.edited_env.clone(),
            edited_aliases: self.edited_aliases.clone(),
            edited_list: self.edited_list.clone(),
            dirty: self.dirty.clone(),
            modified: self.state.modified,
            selected: self.state.tree_state.selected,
        });
    }

    /// Restore the last undo snapshot, if any. Single-level: the slot is cleared
    /// afterward so a second undo is a no-op until the next edit.
    fn undo(&mut self) {
        match self.undo_snapshot.take() {
            Some(s) => {
                self.config = s.config;
                self.tree_roots = s.tree_roots;
                self.edited_settings = s.edited_settings;
                self.edited_scripts = s.edited_scripts;
                self.edited_urls = s.edited_urls;
                self.edited_env = s.edited_env;
                self.edited_aliases = s.edited_aliases;
                self.edited_list = s.edited_list;
                self.dirty = s.dirty;
                self.state.modified = s.modified;
                self.state.tree_state.selected = s.selected;
                self.sync_tree_scroll();
                self.state.set_status("Undid last edit");
            }
            None => self.state.set_status("Nothing to undo"),
        }
    }

    /// Set each editable node's `dirty` flag from the tracked `dirty` set, so the
    /// tree shows an unsaved-edit marker. Called after (re)building the tree.
    fn mark_dirty_nodes(&mut self) {
        fn walk(node: &mut TreeNode, dirty: &HashSet<String>) {
            node.dirty = dirty.contains(&node.node_type);
            for c in &mut node.children {
                walk(c, dirty);
            }
        }
        let dirty = self.dirty.clone();
        for r in &mut self.tree_roots {
            walk(r, &dirty);
        }
    }

    /// Clear all unsaved-edit markers (after a successful save).
    fn clear_dirty(&mut self) {
        self.dirty.clear();
        fn walk(node: &mut TreeNode) {
            node.dirty = false;
            for c in &mut node.children {
                walk(c);
            }
        }
        for r in &mut self.tree_roots {
            walk(r);
        }
    }

    /// The keybinding sections shown in the `?` help overlay. Kept in sync with
    /// the key handling in [`handle_key`](Self::handle_key) and
    /// [`metarepo_core::tui::handle_key`].
    fn help_sections() -> Vec<HelpSection> {
        vec![
            HelpSection::new(
                "Navigation",
                vec![
                    ("j / ↓", "Move down"),
                    ("k / ↑", "Move up"),
                    ("g / G", "Jump to top / bottom"),
                    ("Ctrl+u / Ctrl+d", "Page up / down"),
                ],
            ),
            HelpSection::new(
                "Tree",
                vec![
                    ("l / → / Space", "Expand or edit leaf"),
                    ("h / ←", "Collapse node or climb to parent"),
                    ("Enter", "Toggle expand, or edit a value"),
                ],
            ),
            HelpSection::new(
                "Edit",
                vec![
                    ("e / Enter", "Edit the value (bools toggle in place)"),
                    ("a", "Add an entry in this context"),
                    ("d", "Delete the selected entry"),
                    ("u", "Undo the last edit"),
                    ("Esc", "Cancel an in-progress edit"),
                ],
            ),
            HelpSection::new(
                "Cascade",
                vec![
                    ("o", "Override an inherited value locally"),
                    ("O", "Reveal the source of an inherited value"),
                ],
            ),
            HelpSection::new(
                "Workspace",
                vec![
                    ("/", "Search and jump to a node"),
                    ("s / Ctrl+w", "Save to disk"),
                    ("q / Esc", "Quit (guards unsaved changes)"),
                    ("?", "Toggle this help"),
                ],
            ),
        ]
    }

    /// The dotted setting key of the currently-selected node, if it is a declared
    /// setting node (skill.*, default_bare, worktree_init, ...).
    fn selected_setting_key(&self) -> Option<String> {
        let nt = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)
            .map(|n| n.node_type.clone())?;
        parse_setting_node_type(&nt).map(|(_, key)| key.to_string())
    }

    /// Override an inherited setting in the nearest config: copy its effective
    /// (inherited) value into `self.config` so it becomes a local override. A
    /// no-op with a status hint when the selection is not an inherited setting.
    fn override_inherited_here(&mut self) {
        let key = match self.selected_setting_key() {
            Some(k) => k,
            None => {
                self.state
                    .set_status("Select a setting to override it locally");
                return;
            }
        };

        // Only meaningful when the value is inherited (unset locally).
        if Self::inherited_annotation(&self.config, &self.ancestors, &key).is_none() {
            self.state
                .set_status(format!("'{}' is not inherited — nothing to override", key));
            return;
        }

        let effective = match Self::effective_value(&self.config, &self.ancestors, &key) {
            Some(v) => v,
            None => {
                self.state
                    .set_status(format!("'{}' has no value to copy", key));
                return;
            }
        };

        // Route through the typed config API so the override matches the
        // setting's declared type (falling back to a string), mirroring an edit.
        let parsed = self
            .settings
            .iter()
            .find(|s| s.key == key)
            .and_then(|s| s.value_type.parse(&effective).ok())
            .unwrap_or_else(|| serde_json::Value::String(effective.clone()));

        // Snapshot before mutating so this override is undoable, and mark the
        // selected node dirty so the marker survives the rebuild below.
        self.snapshot_for_undo();
        if let Some(nt) = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)
            .map(|n| n.node_type.clone())
        {
            self.dirty.insert(nt);
        }

        match self.config.clone().with_dotted_set(&key, parsed) {
            Ok(updated) => {
                self.config = updated;
                self.state.modified = true;
                self.rebuild_tree();
                self.state
                    .set_status(format!("Overrode '{}' locally = {}", key, effective));
            }
            Err(e) => {
                self.state
                    .set_status(format!("Could not override '{}': {}", key, e));
            }
        }
    }

    /// If the selected node is a bool setting, flip it in place (true ⇄ false)
    /// and commit, instead of opening a text buffer. Returns true when it
    /// handled the node so the caller skips the normal text editor. Empty/unset
    /// bools toggle to true first.
    fn toggle_bool_setting(&mut self) -> bool {
        let selected = self.state.tree_state.selected;
        let (node_type, current) = match self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(selected)
            .map(|n| (n.node_type.clone(), n.value.clone().unwrap_or_default()))
        {
            Some(v) => v,
            None => return false,
        };

        let key = match parse_setting_node_type(&node_type) {
            Some((ConfigValueType::Bool, key)) => key.to_string(),
            _ => return false,
        };

        let new_value = if current.trim() == "true" {
            "false"
        } else {
            "true"
        };

        // Snapshot for undo, mark dirty, and commit the flipped value the same
        // way a text edit would (edited_settings drives the write on save).
        self.snapshot_for_undo();
        self.edited_settings.insert(key.clone());
        self.dirty.insert(node_type);

        let visible_ptrs: Vec<_> = self
            .tree_roots
            .iter_mut()
            .flat_map(|r| r.flatten_mut())
            .collect();
        if let Some(&node_ptr) = visible_ptrs.get(selected) {
            unsafe {
                (*node_ptr).value = Some(new_value.to_string());
                (*node_ptr).dirty = true;
            }
        }
        self.state.modified = true;
        self.state
            .set_status(format!("Toggled '{}' = {}", key, new_value));
        true
    }

    /// If the selected node is a choice-constrained setting, advance it to the
    /// next allowed value in place (wrapping) and commit, instead of opening a
    /// text buffer. Returns true when it handled the node. An unset/empty or
    /// unrecognized current value starts at the first choice.
    fn cycle_choice_setting(&mut self) -> bool {
        let selected = self.state.tree_state.selected;
        let (node_type, current) = match self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(selected)
            .map(|n| (n.node_type.clone(), n.value.clone().unwrap_or_default()))
        {
            Some(v) => v,
            None => return false,
        };

        // Only declared settings with a choices list are cyclable.
        let key = match parse_setting_node_type(&node_type) {
            Some((_, key)) => key.to_string(),
            None => return false,
        };
        let choices = match self
            .settings
            .iter()
            .find(|s| s.key == key)
            .and_then(|s| s.choices.as_ref())
        {
            Some(c) if !c.is_empty() => c.clone(),
            _ => return false,
        };

        // Advance to the next choice, wrapping; unknown current starts at [0].
        let cur = current.trim();
        let next_idx = choices
            .iter()
            .position(|c| c == cur)
            .map(|i| (i + 1) % choices.len())
            .unwrap_or(0);
        let new_value = choices[next_idx].clone();

        self.snapshot_for_undo();
        self.edited_settings.insert(key.clone());
        self.dirty.insert(node_type);

        let visible_ptrs: Vec<_> = self
            .tree_roots
            .iter_mut()
            .flat_map(|r| r.flatten_mut())
            .collect();
        if let Some(&node_ptr) = visible_ptrs.get(selected) {
            unsafe {
                (*node_ptr).value = Some(new_value.clone());
                (*node_ptr).dirty = true;
            }
        }
        self.state.modified = true;
        self.state
            .set_status(format!("Set '{}' = {}", key, new_value));
        true
    }

    /// Show where an inherited setting's value comes from (the ancestor `.meta`
    /// path) in the status bar. A no-op hint when the selection is local/unset.
    fn reveal_inherited_source(&mut self) {
        let key = match self.selected_setting_key() {
            Some(k) => k,
            None => {
                self.state.set_status("Select a setting to see its source");
                return;
            }
        };
        match Self::inherited_annotation(&self.config, &self.ancestors, &key) {
            Some(note) => self.state.set_status(format!("{} {}", key, note)),
            None => self
                .state
                .set_status(format!("'{}' is set locally (or unset)", key)),
        }
    }

    /// Load the enclosing `.meta` chain above `meta_file` (outermost → nearest),
    /// dropping the nearest entry (the file being edited) so only inherited
    /// ancestors remain. Best-effort: discovery or parse failures yield an empty
    /// chain, which simply disables inheritance annotations.
    fn load_ancestors(meta_file: &std::path::Path) -> Vec<(PathBuf, MetaConfig)> {
        let start = match meta_file.parent() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let discovered = match MetaConfig::discover_chain_from(start) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        discovered
            .into_iter()
            .filter(|d| d.path != meta_file)
            .filter_map(|d| {
                MetaConfig::load_from_file(&d.path)
                    .ok()
                    .map(|c| (d.path, c))
            })
            .collect()
    }

    /// Cascade annotation for a dotted `key`: `None` when the key is set in the
    /// nearest (live) config or unset everywhere; `Some("(inherited from …)")`
    /// when only an ancestor sets it. Recomputed on rebuild so a fresh local
    /// edit correctly drops the inherited note.
    fn inherited_annotation(
        config: &MetaConfig,
        ancestors: &[(PathBuf, MetaConfig)],
        key: &str,
    ) -> Option<String> {
        if config.get_dotted(key).is_some() {
            return None; // local override wins
        }
        for (path, cfg) in ancestors.iter().rev() {
            if cfg.get_dotted(key).is_some() {
                return Some(format!("(inherited from {})", path.display()));
            }
        }
        None
    }

    /// Effective display value for a dotted `key`: the nearest config that sets
    /// it wins, then ancestors nearest-first, mirroring the cascade used by
    /// `meta config get`/`list`. Returns `None` when unset across the chain so
    /// the caller can fall back to the declared default.
    fn effective_value(
        config: &MetaConfig,
        ancestors: &[(PathBuf, MetaConfig)],
        key: &str,
    ) -> Option<String> {
        let fmt = |v: serde_json::Value| match v {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        };
        if let Some(v) = config.get_dotted(key) {
            return Some(fmt(v));
        }
        for (_, cfg) in ancestors.iter().rev() {
            if let Some(v) = cfg.get_dotted(key) {
                return Some(fmt(v));
            }
        }
        None
    }

    /// Build tree representation from config
    fn build_tree(
        config: &MetaConfig,
        settings: &[ConfigSetting],
        ancestors: &[(PathBuf, MetaConfig)],
    ) -> Vec<TreeNode> {
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
                let mut url_node =
                    TreeNode::with_value("url", &meta.url, project_url_node_type(name));
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

                // Environment variables
                if !meta.env.is_empty() {
                    let mut env_node = TreeNode::new("env", "section");
                    env_node.depth = 2;
                    env_node.expandable = true;

                    for (key, val) in &meta.env {
                        let mut kv =
                            TreeNode::with_value(key, val, project_env_node_type(name, key));
                        kv.depth = 3;
                        env_node.add_child(kv);
                    }

                    children.push(env_node);
                }

                // Aliases
                if !meta.aliases.is_empty() {
                    let mut aliases_node = TreeNode::new("aliases", "section");
                    aliases_node.depth = 2;
                    aliases_node.expandable = true;

                    for alias in &meta.aliases {
                        let mut alias_node = TreeNode::with_value(
                            alias,
                            alias,
                            project_alias_item_node_type(name, alias),
                        );
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
                    let mut alias_node =
                        TreeNode::with_value(name, value, global_alias_node_type(name));
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
                let mut pattern_node =
                    TreeNode::with_value(pattern, pattern, ignore_item_node_type(pattern));
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
                // Effective value across the cascade (nearest wins, then
                // ancestors), else the declared default, else empty. Display the
                // short key (after the namespace).
                let display = Self::effective_value(config, ancestors, &setting.key)
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
                if let Some(note) = Self::inherited_annotation(config, ancestors, &setting.key) {
                    node.annotation = Some(note);
                }
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
            let bare = Self::effective_value(config, ancestors, "default_bare").unwrap_or_default();
            let mut bare_node = TreeNode::with_value(
                "default_bare",
                bare,
                setting_node_type("default_bare", ConfigValueType::Bool),
            );
            bare_node.depth = 2;
            bare_node.annotation = Self::inherited_annotation(config, ancestors, "default_bare");
            core_node.add_child(bare_node);

            let wt = Self::effective_value(config, ancestors, "worktree_init").unwrap_or_default();
            let mut wt_node = TreeNode::with_value(
                "worktree_init",
                wt,
                setting_node_type("worktree_init", ConfigValueType::String),
            );
            wt_node.depth = 2;
            wt_node.annotation = Self::inherited_annotation(config, ancestors, "worktree_init");
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

        if !self.edited_urls.is_empty() {
            let updates: Vec<(String, String)> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .filter_map(|node| {
                    let proj = parse_project_url_node_type(&node.node_type)?;
                    if !self.edited_urls.contains(proj) {
                        return None;
                    }
                    Some((proj.to_string(), node.value.clone()?))
                })
                .collect();

            for (proj, url) in updates {
                match self.config.projects.get_mut(&proj) {
                    Some(metarepo_core::ProjectEntry::Metadata(meta)) => meta.url = url,
                    Some(entry @ metarepo_core::ProjectEntry::Url(_)) => {
                        *entry = metarepo_core::ProjectEntry::Url(url);
                    }
                    None => {}
                }
            }
            self.edited_urls.clear();
        }

        if !self.edited_env.is_empty() {
            let updates: Vec<(String, String, String)> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .filter_map(|node| {
                    let (proj, key) = parse_project_env_node_type(&node.node_type)?;
                    if !self.edited_env.contains(&node.node_type) {
                        return None;
                    }
                    Some((proj.to_string(), key.to_string(), node.value.clone()?))
                })
                .collect();

            for (proj, key, val) in updates {
                self.set_env(&proj, &key, val);
            }
            self.edited_env.clear();
        }

        if !self.edited_aliases.is_empty() {
            let updates: Vec<(String, String)> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .filter_map(|node| {
                    let name = parse_global_alias_node_type(&node.node_type)?;
                    if !self.edited_aliases.contains(&node.node_type) {
                        return None;
                    }
                    Some((name.to_string(), node.value.clone()?))
                })
                .collect();

            for (name, target) in updates {
                self.config
                    .aliases
                    .get_or_insert_with(Default::default)
                    .insert(name, target);
            }
            self.edited_aliases.clear();
        }

        if !self.edited_list.is_empty() {
            // Each node_type encodes the OLD value; node.value holds the new one.
            let updates: Vec<(String, String)> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .filter_map(|node| {
                    if !self.edited_list.contains(&node.node_type) {
                        return None;
                    }
                    Some((node.node_type.clone(), node.value.clone()?))
                })
                .collect();

            for (nt, new) in updates {
                if let Some(old) = parse_ignore_item(&nt) {
                    replace_in_vec(&mut self.config.ignore, old, &new);
                } else if let Some((proj, old)) = parse_project_alias_item(&nt) {
                    if let Some(metarepo_core::ProjectEntry::Metadata(meta)) =
                        self.config.projects.get_mut(proj)
                    {
                        replace_in_vec(&mut meta.aliases, old, &new);
                    }
                }
            }
            self.edited_list.clear();
        }

        Ok(())
    }

    /// Insert or update a per-project env var.
    fn set_env(&mut self, proj: &str, key: &str, val: String) {
        if let Some(metarepo_core::ProjectEntry::Metadata(meta)) =
            self.config.projects.get_mut(proj)
        {
            meta.env.insert(key.to_string(), val);
        }
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
    /// preserving which sections are expanded and keeping the selection in range.
    fn rebuild_tree(&mut self) {
        // Flush pending value edits into the in-memory config first; otherwise
        // rebuilding from config would discard edits not yet applied (e.g. an
        // edit followed by an add/delete elsewhere).
        let _ = self.apply_pending_edits();

        // Capture expansion state by label-path so add/delete don't collapse the
        // section the user is working in.
        let mut expanded: HashSet<String> = HashSet::new();
        fn capture(node: &TreeNode, prefix: &str, out: &mut HashSet<String>) {
            let path = format!("{prefix}/{}", node.label);
            if node.expanded {
                out.insert(path.clone());
            }
            for c in &node.children {
                capture(c, &path, out);
            }
        }
        for r in &self.tree_roots {
            capture(r, "", &mut expanded);
        }

        self.tree_roots = Self::build_tree(&self.config, &self.settings, &self.ancestors);
        self.mark_dirty_nodes();

        fn apply(node: &mut TreeNode, prefix: &str, set: &HashSet<String>) {
            let path = format!("{prefix}/{}", node.label);
            if set.contains(&path) {
                node.expanded = true;
            }
            for c in &mut node.children {
                apply(c, &path, set);
            }
        }
        for r in &mut self.tree_roots {
            apply(r, "", &expanded);
        }

        let visible = self.tree_roots.iter().flat_map(|r| r.flatten(true)).count();
        if self.state.tree_state.selected >= visible && visible > 0 {
            self.state.tree_state.selected = visible - 1;
        }
    }

    /// Expand the ancestors of the node with `node_type` and select it.
    fn expand_and_select(&mut self, node_type: &str) {
        fn expand_path(node: &mut TreeNode, target: &str) -> bool {
            if node.node_type == target {
                return true;
            }
            for c in &mut node.children {
                if expand_path(c, target) {
                    node.expanded = true;
                    return true;
                }
            }
            false
        }
        for r in &mut self.tree_roots {
            expand_path(r, node_type);
        }
        if let Some(idx) = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .position(|n| n.node_type == node_type)
        {
            self.state.tree_state.selected = idx;
        }
    }

    /// Open the search prompt.
    fn start_search(&mut self) {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().bg(Color::Cyan));
        self.textarea = Some(textarea);
        self.searching = true;
        self.state.editing = true;
        self.state
            .set_status("Search: type, Enter to jump, Esc to cancel");
    }

    /// Expand to and select the first node whose label or value contains
    /// `query` (case-insensitive). Returns false if nothing matched.
    fn jump_to_match(&mut self, query: &str) -> bool {
        // Delegate to the shared tree-shell search (expand-to-match + select).
        search_and_reveal(&mut self.tree_roots, &mut self.state.tree_state, query)
    }

    /// The project that owns the currently-selected node, if the selection is
    /// anywhere within a project's subtree.
    fn project_context(&self) -> Option<String> {
        fn dfs(
            node: &TreeNode,
            idx: &mut usize,
            target: usize,
            cur: Option<&str>,
        ) -> Option<Option<String>> {
            let this = if node.node_type == "project" {
                Some(node.label.as_str())
            } else {
                cur
            };
            if *idx == target {
                return Some(this.map(String::from));
            }
            *idx += 1;
            if node.expanded {
                for c in &node.children {
                    if let Some(found) = dfs(c, idx, target, this) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let target = self.state.tree_state.selected;
        let mut idx = 0;
        for r in &self.tree_roots {
            if let Some(found) = dfs(r, &mut idx, target, None) {
                return found;
            }
        }
        None
    }

    /// Context-filtered list of things addable from the current selection.
    fn add_options_for_selected(&self) -> Vec<AddContext> {
        let node = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected);
        if let Some(node) = node {
            if node.node_type == "section" && node.label == "Projects" {
                return vec![AddContext::NewProject];
            }
        }
        if let Some(proj) = self.project_context() {
            return vec![
                AddContext::ProjectScript(proj.clone()),
                AddContext::ProjectEnv(proj.clone()),
                AddContext::ProjectAlias(proj),
            ];
        }
        vec![
            AddContext::GlobalScript,
            AddContext::GlobalAlias,
            AddContext::IgnorePattern,
            AddContext::NewProject,
        ]
    }

    /// Handle 'a': open the add-type selector, or skip straight to the name
    /// prompt when there is a single choice.
    fn start_add(&mut self) {
        let opts = self.add_options_for_selected();
        match opts.len() {
            0 => self.state.set_status("Nothing to add here"),
            1 => self.begin_name_prompt(opts.into_iter().next().unwrap()),
            _ => {
                self.add_menu = Some((opts, 0));
                self.state
                    .set_status("Add what? — ↑/↓ to choose, Enter to confirm, Esc to cancel");
            }
        }
    }

    /// Open the name-input prompt for the chosen add context.
    fn begin_name_prompt(&mut self, ctx: AddContext) {
        let prompt = ctx.name_prompt();
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().bg(Color::Cyan));
        self.textarea = Some(textarea);
        self.adding = Some(ctx);
        self.state.editing = true;
        self.state
            .set_status(format!("New {prompt} — Enter to create, Esc to cancel"));
    }

    /// Create the new entry from the typed name and select it for editing.
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

        // Adding a project is a different shape than a script entry.
        if let AddContext::NewProject = ctx {
            if self.config.projects.contains_key(&name) {
                self.state
                    .set_status(format!("Project {name} already exists"));
                return;
            }
            self.config.projects.insert(
                name.clone(),
                metarepo_core::ProjectEntry::Metadata(metarepo_core::ProjectMetadata {
                    url: String::new(),
                    aliases: Vec::new(),
                    scripts: std::collections::HashMap::new(),
                    env: std::collections::HashMap::new(),
                    worktree_init: None,
                    bare: None,
                    enabled: None,
                    depth: None,
                }),
            );
            self.state.modified = true;
            self.rebuild_tree();
            // Chain into editing the new project's URL.
            let url_nt = project_url_node_type(&name);
            self.expand_and_select(&url_nt);
            self.start_editing();
            self.state.set_status(format!(
                "Enter URL for {name} — Enter to save, Esc to cancel"
            ));
            return;
        }

        // Adding a project env var: key then value (like a script command).
        if let AddContext::ProjectEnv(proj) = &ctx {
            let proj = proj.clone();
            match self.config.projects.get(&proj) {
                Some(metarepo_core::ProjectEntry::Metadata(m)) => {
                    if m.env.contains_key(&name) {
                        self.state
                            .set_status(format!("Env var {name} already exists"));
                        return;
                    }
                }
                Some(metarepo_core::ProjectEntry::Url(_)) => {
                    self.state.set_status(format!(
                        "Project {proj} has no metadata block; cannot add env vars yet"
                    ));
                    return;
                }
                None => {}
            }
            self.set_env(&proj, &name, String::new());
            self.state.modified = true;
            self.rebuild_tree();
            let nt = project_env_node_type(&proj, &name);
            self.expand_and_select(&nt);
            self.start_editing();
            self.state.set_status(format!(
                "Enter value for {name} — Enter to save, Esc to cancel"
            ));
            return;
        }

        // Adding a global alias: name then target path.
        if let AddContext::GlobalAlias = ctx {
            let exists = self
                .config
                .aliases
                .as_ref()
                .map(|m| m.contains_key(&name))
                .unwrap_or(false);
            if exists {
                self.state
                    .set_status(format!("Alias {name} already exists"));
                return;
            }
            self.config
                .aliases
                .get_or_insert_with(Default::default)
                .insert(name.clone(), String::new());
            self.state.modified = true;
            self.rebuild_tree();
            let nt = global_alias_node_type(&name);
            self.expand_and_select(&nt);
            self.start_editing();
            self.state.set_status(format!(
                "Enter target path for {name} — Enter to save, Esc to cancel"
            ));
            return;
        }

        // List items (ignore pattern, project alias): the typed name IS the
        // value; append and finish (no value step).
        if let AddContext::IgnorePattern = ctx {
            if self.config.ignore.iter().any(|p| p == &name) {
                self.state
                    .set_status(format!("Ignore pattern {name} already exists"));
                return;
            }
            self.config.ignore.push(name.clone());
            self.state.modified = true;
            self.rebuild_tree();
            self.state
                .set_status(format!("Added ignore pattern {name}"));
            return;
        }
        if let AddContext::ProjectAlias(proj) = &ctx {
            let proj = proj.clone();
            match self.config.projects.get_mut(&proj) {
                Some(metarepo_core::ProjectEntry::Metadata(meta)) => {
                    if meta.aliases.contains(&name) {
                        self.state
                            .set_status(format!("Alias {name} already exists"));
                        return;
                    }
                    meta.aliases.push(name.clone());
                }
                _ => {
                    self.state.set_status(format!(
                        "Project {proj} has no metadata block; cannot add aliases yet"
                    ));
                    return;
                }
            }
            self.state.modified = true;
            self.rebuild_tree();
            self.state
                .set_status(format!("Added alias {name} to {proj}"));
            return;
        }

        let target = match &ctx {
            AddContext::GlobalScript => ScriptRef::Global(name.clone()),
            AddContext::ProjectScript(proj) => ScriptRef::Project {
                proj: proj.clone(),
                name: name.clone(),
            },
            AddContext::NewProject
            | AddContext::ProjectEnv(_)
            | AddContext::GlobalAlias
            | AddContext::IgnorePattern
            | AddContext::ProjectAlias(_) => {
                unreachable!("handled above")
            }
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

        // Chain straight into editing the new entry's command so the user enters
        // the value in the same flow, without re-finding the node.
        let new_nt = match &target {
            ScriptRef::Global(n) => global_script_node_type(n),
            ScriptRef::Project { proj, name } => project_script_node_type(proj, name),
        };
        self.expand_and_select(&new_nt);
        self.start_editing();
        self.state.set_status(format!(
            "Enter command for {name} — Enter to save, Esc to cancel"
        ));
    }

    /// Delete the selected script or project entry (in memory; persisted on
    /// save).
    fn delete_selected(&mut self) {
        let selected = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected);
        let (node_type, label) = match selected {
            Some(n) => (n.node_type.clone(), n.label.clone()),
            None => return,
        };

        // A whole project (the "project" node, labelled with the project key).
        if node_type == "project" {
            self.config.projects.remove(&label);
            self.edited_urls.remove(&label);
            self.state.modified = true;
            self.rebuild_tree();
            self.state.set_status(format!(
                "Deleted project {label} (unsaved — 's' to write, 'q' to discard)"
            ));
            return;
        }

        // A project env var.
        if let Some((proj, key)) = parse_project_env_node_type(&node_type) {
            let (proj, key) = (proj.to_string(), key.to_string());
            if let Some(metarepo_core::ProjectEntry::Metadata(meta)) =
                self.config.projects.get_mut(&proj)
            {
                meta.env.remove(&key);
            }
            self.edited_env.remove(&node_type);
            self.state.modified = true;
            self.rebuild_tree();
            self.state
                .set_status("Deleted (unsaved — 's' to write, 'q' to discard)");
            return;
        }

        // A global alias.
        if let Some(name) = parse_global_alias_node_type(&node_type) {
            let name = name.to_string();
            if let Some(m) = self.config.aliases.as_mut() {
                m.remove(&name);
            }
            self.edited_aliases.remove(&node_type);
            self.state.modified = true;
            self.rebuild_tree();
            self.state
                .set_status("Deleted (unsaved — 's' to write, 'q' to discard)");
            return;
        }

        // A value-keyed list item (ignore pattern / project alias).
        if let Some(val) = parse_ignore_item(&node_type) {
            let val = val.to_string();
            self.config.ignore.retain(|p| p != &val);
            self.edited_list.remove(&node_type);
            self.state.modified = true;
            self.rebuild_tree();
            self.state
                .set_status("Deleted (unsaved — 's' to write, 'q' to discard)");
            return;
        }
        if let Some((proj, val)) = parse_project_alias_item(&node_type) {
            let (proj, val) = (proj.to_string(), val.to_string());
            if let Some(metarepo_core::ProjectEntry::Metadata(meta)) =
                self.config.projects.get_mut(&proj)
            {
                meta.aliases.retain(|a| a != &val);
            }
            self.edited_list.remove(&node_type);
            self.state.modified = true;
            self.rebuild_tree();
            self.state
                .set_status("Deleted (unsaved — 's' to write, 'q' to discard)");
            return;
        }

        let Some(target) = parse_script_node_type(&node_type) else {
            self.state
                .set_status("Delete supports scripts, env vars, and projects for now");
            return;
        };

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
        self.edited_scripts.remove(&node_type);
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

    /// Draw the standard three-pane layout, then overlay the help popup when
    /// `?` is active.
    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Breadcrumb
                Constraint::Min(1),    // Main content
                Constraint::Length(2), // Context bar
            ])
            .split(frame.area());
        self.render_breadcrumb(frame, chunks[0]);
        self.render_content(frame, chunks[1]);
        self.render_context_bar(frame, chunks[2]);

        if self.show_help {
            let area = centered_rect(64, 80, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(
                KeybindingHelp::new("Config editor keys", Self::help_sections()),
                area,
            );
        }
    }

    fn get_tree_roots(&self) -> &[TreeNode] {
        &self.tree_roots
    }

    fn get_tree_roots_mut(&mut self) -> &mut Vec<TreeNode> {
        &mut self.tree_roots
    }

    fn start_editing(&mut self) {
        // Bool settings toggle in place instead of opening a text buffer.
        if self.toggle_bool_setting() {
            return;
        }
        // Choice-constrained settings cycle to the next allowed value in place.
        if self.cycle_choice_setting() {
            return;
        }

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
            } else if parse_project_env_node_type(nt).is_some() {
                self.edited_env.insert(nt.clone());
            } else if parse_global_alias_node_type(nt).is_some() {
                self.edited_aliases.insert(nt.clone());
            } else if parse_ignore_item(nt).is_some() || parse_project_alias_item(nt).is_some() {
                self.edited_list.insert(nt.clone());
            } else if let Some(proj) = parse_project_url_node_type(nt) {
                self.edited_urls.insert(proj.to_string());
            }
        }

        // Value is valid and about to commit: snapshot for single-level undo and
        // record the node as dirty for the unsaved-edit marker.
        self.snapshot_for_undo();
        if let Some(nt) = &node_type {
            self.dirty.insert(nt.clone());
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
                (*node_ptr).dirty = true;
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
        // Help overlay: while open, any key dismisses it and is consumed.
        // (Opening is routed through Action::Help in the shared keymap below.)
        if self.show_help {
            self.show_help = false;
            return Ok(true);
        }

        // Quit confirmation is showing: resolve it first.
        if self.confirm_quit {
            match (key.code, key.modifiers) {
                (KeyCode::Char('s'), KeyModifiers::NONE) => {
                    self.save()?;
                    self.state.modified = false;
                    self.confirm_quit = false;
                    self.state.should_quit = true;
                }
                (KeyCode::Char('q'), KeyModifiers::NONE)
                | (KeyCode::Char('y'), KeyModifiers::NONE) => {
                    self.confirm_quit = false;
                    self.state.should_quit = true;
                }
                (KeyCode::Esc, _) | (KeyCode::Char('n'), KeyModifiers::NONE) => {
                    self.confirm_quit = false;
                    self.state.set_status("Continue editing");
                }
                _ => {}
            }
            return Ok(!self.state.should_quit);
        }

        // Add-type selector is open: navigate and pick a type.
        if let Some((opts, idx)) = self.add_menu.as_mut() {
            match (key.code, key.modifiers) {
                (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                    *idx = idx.saturating_sub(1);
                }
                (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    *idx = (*idx + 1).min(opts.len() - 1);
                }
                (KeyCode::Enter, _) => {
                    let ctx = opts[*idx].clone();
                    self.add_menu = None;
                    self.begin_name_prompt(ctx);
                }
                (KeyCode::Esc, _) | (KeyCode::Char('q'), KeyModifiers::NONE) => {
                    self.add_menu = None;
                    self.state.set_status("Add cancelled");
                }
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    self.state.should_quit = true;
                    return Ok(false);
                }
                _ => {}
            }
            return Ok(!self.state.should_quit);
        }

        // If textarea is active, handle keys directly
        if let Some(textarea) = &mut self.textarea {
            match (key.code, key.modifiers) {
                // Enter confirms (search jump, add prompt, or value edit)
                (KeyCode::Enter, KeyModifiers::NONE) => {
                    if self.searching {
                        let query = self
                            .textarea
                            .as_ref()
                            .map(|t| t.lines().join(""))
                            .unwrap_or_default();
                        self.searching = false;
                        self.textarea = None;
                        self.state.editing = false;
                        if self.jump_to_match(&query) {
                            self.state
                                .set_status(format!("Jumped to '{}'", query.trim()));
                        } else {
                            self.state
                                .set_status(format!("No match for '{}'", query.trim()));
                        }
                    } else if self.adding.is_some() {
                        self.confirm_add();
                    } else {
                        self.save_edit();
                    }
                }
                // Esc cancels
                (KeyCode::Esc, _) => {
                    if self.searching {
                        self.searching = false;
                        self.textarea = None;
                        self.state.editing = false;
                        self.state.set_status("Search cancelled");
                    } else if self.adding.is_some() {
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
            (KeyCode::Char('o'), KeyModifiers::NONE)
        ) {
            // Override an inherited setting locally (cascade-specific, kept local).
            self.override_inherited_here();
            Ok(true)
        } else if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('O'), KeyModifiers::SHIFT)
        ) {
            // Reveal the source of an inherited setting (cascade-specific).
            self.reveal_inherited_source();
            Ok(true)
        } else {
            // Not editing - use the shared keymap for the common actions.
            let action = metarepo_core::tui::handle_key(key, self.state.editing);

            match action {
                Action::None => {}

                // Navigation
                Action::NavigateUp => {
                    self.state.tree_state.select_previous();
                    self.update_breadcrumb_for_selected();
                    self.sync_tree_scroll();
                }
                Action::NavigateDown => {
                    let visible_count = self
                        .get_tree_roots()
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .count();
                    self.state.tree_state.select_next(visible_count);
                    self.update_breadcrumb_for_selected();
                    self.sync_tree_scroll();
                }
                Action::NavigateTop => {
                    self.state.tree_state.select_first();
                    self.update_breadcrumb_for_selected();
                    self.sync_tree_scroll();
                }
                Action::NavigateBottom => {
                    let visible_count = self
                        .get_tree_roots()
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .count();
                    self.state.tree_state.select_last(visible_count);
                    self.update_breadcrumb_for_selected();
                    self.sync_tree_scroll();
                }
                Action::NavigatePageUp => {
                    for _ in 0..10 {
                        self.state.tree_state.select_previous();
                    }
                    self.update_breadcrumb_for_selected();
                    self.sync_tree_scroll();
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
                    self.sync_tree_scroll();
                }

                // Tree operations
                Action::ToggleExpand => {
                    let selected_idx = self.state.tree_state.selected;
                    // Read state up front so the immutable borrow is released
                    // before we take the mutable borrow to toggle.
                    let (expandable, was_expanded) = self
                        .tree_roots
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .nth(selected_idx)
                        .map(|n| (n.expandable, n.expanded))
                        .unwrap_or((false, false));

                    // Prioritize expand/collapse over editing.
                    if expandable {
                        {
                            let roots = self.get_tree_roots_mut();
                            let visible_mut: Vec<_> =
                                roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
                            if let Some(&node_ptr) = visible_mut.get(selected_idx) {
                                unsafe {
                                    (*node_ptr).toggle();
                                }
                            }
                        }

                        let height = self.state.tree_state.viewport_height;
                        if was_expanded {
                            // Just collapsed — keep the selected row visible.
                            self.state.tree_state.update_offset(height);
                        } else {
                            // Just expanded — scroll down to reveal as much of the
                            // new subtree as possible. The subtree spans from the
                            // selected row to selected + (visible descendants).
                            let span = self
                                .tree_roots
                                .iter()
                                .flat_map(|r| r.flatten(true))
                                .nth(selected_idx)
                                .map(|n| n.flatten(true).len())
                                .unwrap_or(1);
                            let subtree_last = selected_idx + span.saturating_sub(1);
                            self.state.tree_state.reveal_subtree(subtree_last, height);
                        }
                    } else if self.is_selected_editable() {
                        // Not expandable but editable — start editing.
                        self.start_editing();
                    }
                }

                // Collapse current node, or climb to and collapse its parent.
                Action::CollapseParent => {
                    let selected_idx = self.state.tree_state.selected;
                    let rows: Vec<(usize, bool, bool)> = self
                        .tree_roots
                        .iter()
                        .flat_map(|r| r.flatten(true))
                        .map(|n| (n.depth, n.expandable, n.expanded))
                        .collect();
                    if let Some(&(depth, expandable, expanded)) = rows.get(selected_idx) {
                        let target = if expandable && expanded {
                            // Collapse the node we're sitting on.
                            Some(selected_idx)
                        } else if depth > 0 {
                            // Climb to the nearest shallower row (the parent).
                            (0..selected_idx).rev().find(|&i| rows[i].0 < depth)
                        } else {
                            None
                        };
                        if let Some(idx) = target {
                            self.collapse_at(idx);
                            self.state.tree_state.selected = idx;
                            self.update_breadcrumb_for_selected();
                            self.sync_tree_scroll();
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
                    self.clear_dirty();
                    self.undo_snapshot = None;
                    self.state.set_status("Saved!");
                }
                Action::Quit => {
                    // Guard against losing unsaved edits: ask first.
                    if self.state.modified {
                        self.confirm_quit = true;
                        self.state.set_status(
                            "Unsaved changes — s: save & quit, q: discard & quit, Esc: cancel",
                        );
                    } else {
                        self.state.should_quit = true;
                    }
                }

                Action::Search => {
                    self.start_search();
                }

                Action::Add => {
                    self.start_add();
                }
                Action::Delete => {
                    self.delete_selected();
                }
                Action::Undo => {
                    self.undo();
                }
                Action::Help => {
                    self.show_help = true;
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
        // Left tree pane via the shared shell; the returned area is the detail
        // pane, which the config editor fills with its own panels below.
        let detail_area = render_tree_pane(
            frame,
            area,
            &self.tree_roots,
            &mut self.state.tree_state,
            "Config Tree",
        );

        // Render detail/edit panel
        if let Some((opts, idx)) = &self.add_menu {
            // Render the add-type selector.
            let mut lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    "Add what?",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            for (i, opt) in opts.iter().enumerate() {
                let selected = i == *idx;
                let marker = if selected { "› " } else { "  " };
                let style = if selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                lines.push(Line::from(Span::styled(
                    format!("{marker}{}", opt.menu_label()),
                    style,
                )));
            }
            let panel = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Add ")
                    .border_style(Style::default().fg(Color::Green)),
            );
            frame.render_widget(panel, detail_area);
        } else if let Some(textarea) = &mut self.textarea {
            // Render text editor (or search prompt)
            let title = if self.searching {
                " Search "
            } else if self.adding.is_some() {
                " New name "
            } else {
                " Edit Value "
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Green));

            textarea.set_block(block);
            frame.render_widget(&*textarea, detail_area);
        } else {
            // Show selected node details
            let visible: Vec<_> = self
                .tree_roots
                .iter()
                .flat_map(|r| r.flatten(true))
                .collect();
            let detail_content = if let Some(node) = visible.get(self.state.tree_state.selected) {
                let mut lines = vec![
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
                ];
                // Cascade source, when the value is inherited from an outer .meta.
                if let Some(note) = &node.annotation {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("Source: ", Style::default().fg(Color::Gray)),
                        Span::styled(note, Style::default().fg(Color::DarkGray)),
                    ]));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press Enter to edit",
                    Style::default().fg(Color::DarkGray),
                )));
                lines
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

            frame.render_widget(detail_panel, detail_area);
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

    /// Test hook: drive the add flow with a typed name (script or project).
    fn add_script_for_test(&mut self, ctx: AddContext, name: &str) {
        self.adding = Some(ctx);
        self.textarea = Some(TextArea::new(vec![name.to_string()]));
        self.state.editing = true;
        self.confirm_add();
    }

    /// Test hook: set a node's value by `node_type` and route it to the right
    /// edited set, mirroring `save_edit`'s tracking.
    fn edit_value_for_test(&mut self, node_type: &str, value: &str) {
        for ptr in self.tree_roots.iter_mut().flat_map(|r| r.flatten_all_mut()) {
            // SAFETY: pointers come from this tree and are not aliased here.
            let node = unsafe { &mut *ptr };
            if node.node_type == node_type {
                node.value = Some(value.to_string());
                if let Some((_, k)) = parse_setting_node_type(node_type) {
                    self.edited_settings.insert(k.to_string());
                } else if parse_script_node_type(node_type).is_some() {
                    self.edited_scripts.insert(node_type.to_string());
                } else if parse_project_env_node_type(node_type).is_some() {
                    self.edited_env.insert(node_type.to_string());
                } else if parse_global_alias_node_type(node_type).is_some() {
                    self.edited_aliases.insert(node_type.to_string());
                } else if parse_ignore_item(node_type).is_some()
                    || parse_project_alias_item(node_type).is_some()
                {
                    self.edited_list.insert(node_type.to_string());
                } else if let Some(p) = parse_project_url_node_type(node_type) {
                    self.edited_urls.insert(p.to_string());
                }
                return;
            }
        }
        panic!("no node for {node_type}");
    }

    /// Test hook: expand the whole tree and select the node with `node_type`.
    fn select_for_test(&mut self, node_type: &str) {
        self.select_by_test(|n| n.node_type == node_type);
    }

    /// Test hook: expand the whole tree and select the first node matching
    /// `pred`.
    fn select_by_test(&mut self, pred: impl Fn(&TreeNode) -> bool) {
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
            .position(pred)
            .expect("no matching node");
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
    fn inherited_annotation_reflects_cascade_source() {
        use std::path::PathBuf;
        let outer: MetaConfig =
            serde_json::from_str(r#"{"projects":{},"skill":{"dest":"~/outer"}}"#).unwrap();
        let ancestors = vec![(PathBuf::from("/ws/.meta"), outer)];

        // Set locally -> no annotation.
        let local: MetaConfig =
            serde_json::from_str(r#"{"projects":{},"skill":{"dest":"~/inner"}}"#).unwrap();
        assert!(ConfigEditor::inherited_annotation(&local, &ancestors, "skill.dest").is_none());

        // Unset locally but set in an ancestor -> inherited annotation naming the file.
        let empty: MetaConfig = serde_json::from_str(r#"{"projects":{}}"#).unwrap();
        let note = ConfigEditor::inherited_annotation(&empty, &ancestors, "skill.dest").unwrap();
        assert!(note.contains("inherited from"));
        assert!(note.contains("/ws/.meta"));

        // Unset everywhere -> no annotation.
        assert!(ConfigEditor::inherited_annotation(&empty, &ancestors, "skill.dest").is_some());
        assert!(
            ConfigEditor::inherited_annotation(&empty, &ancestors, "skill.search-limit").is_none()
        );
    }

    #[test]
    fn build_tree_annotates_inherited_settings() {
        use std::path::PathBuf;
        let outer: MetaConfig =
            serde_json::from_str(r#"{"projects":{},"skill":{"dest":"~/outer"}}"#).unwrap();
        let ancestors = vec![(PathBuf::from("/ws/.meta"), outer)];
        // Nearest config does not set skill.dest, so it is inherited.
        let cfg: MetaConfig = serde_json::from_str(r#"{"projects":{}}"#).unwrap();

        let roots = ConfigEditor::build_tree(&cfg, &catalog(), &ancestors);
        let skill = roots
            .iter()
            .find(|n| n.label == "Settings")
            .unwrap()
            .children
            .iter()
            .find(|c| c.label == "skill")
            .unwrap();
        let dest = skill.children.iter().find(|c| c.label == "dest").unwrap();
        assert_eq!(dest.value.as_deref(), Some("~/outer"));
        assert!(dest
            .annotation
            .as_deref()
            .is_some_and(|a| a.contains("inherited from")));
    }

    #[test]
    fn build_tree_renders_catalog_and_core() {
        let cfg = MetaConfig::default();
        let roots = ConfigEditor::build_tree(&cfg, &catalog(), &[]);
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
    fn add_chains_into_value_editor_on_new_node() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor.add_script_for_test(AddContext::GlobalScript, "deploy");

        // The editor is now editing the new entry's command directly.
        assert!(editor.state.editing);
        assert!(editor.textarea.is_some());
        assert!(editor.adding.is_none());
        let selected = editor
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(editor.state.tree_state.selected)
            .unwrap();
        assert_eq!(selected.node_type, global_script_node_type("deploy"));
    }

    #[test]
    fn delete_keeps_section_expanded() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{},"scripts":{"a":"1","b":"2"}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor.select_for_test(&global_script_node_type("a")); // expands the tree
        editor.delete_selected();

        let gs = editor
            .tree_roots
            .iter()
            .find(|n| n.label == "Global Scripts")
            .expect("Global Scripts section");
        assert!(gs.expanded, "section should stay expanded after delete");
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

    #[test]
    fn edit_project_url_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{"app":{"url":"old"}}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.edit_value_for_test(&project_url_node_type("app"), "git@new");
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        match reloaded.projects.get("app").unwrap() {
            metarepo_core::ProjectEntry::Metadata(m) => assert_eq!(m.url, "git@new"),
            _ => panic!("expected metadata"),
        }
    }

    #[test]
    fn add_project_chains_to_url_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.add_script_for_test(AddContext::NewProject, "newproj");
        // The flow is now editing the new project's URL.
        assert!(editor.state.editing);
        editor.edit_value_for_test(&project_url_node_type("newproj"), "git@x");
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        match reloaded.projects.get("newproj").unwrap() {
            metarepo_core::ProjectEntry::Metadata(m) => assert_eq!(m.url, "git@x"),
            _ => panic!("expected metadata"),
        }
    }

    #[test]
    fn delete_project_removes_it_on_save() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{"a":{"url":"x"},"b":{"url":"y"}}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.select_by_test(|n| n.node_type == "project" && n.label == "a");
        editor.delete_selected();
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        assert!(!reloaded.projects.contains_key("a"));
        assert!(reloaded.projects.contains_key("b"));
    }

    fn project_env<'a>(
        cfg: &'a MetaConfig,
        proj: &str,
    ) -> &'a std::collections::HashMap<String, String> {
        match cfg.projects.get(proj).unwrap() {
            metarepo_core::ProjectEntry::Metadata(m) => &m.env,
            _ => panic!("expected metadata"),
        }
    }

    #[test]
    fn add_env_var_chains_to_value_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{"app":{"url":"x"}}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.add_script_for_test(AddContext::ProjectEnv("app".into()), "TOKEN");
        assert!(editor.state.editing);
        editor.edit_value_for_test(&project_env_node_type("app", "TOKEN"), "secret");
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        assert_eq!(
            project_env(&reloaded, "app").get("TOKEN").unwrap(),
            "secret"
        );
    }

    #[test]
    fn delete_env_var_removes_on_save() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(
            &path,
            r#"{"projects":{"app":{"url":"x","env":{"A":"1","B":"2"}}}}"#,
        )
        .unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.select_by_test(|n| n.node_type == project_env_node_type("app", "A"));
        editor.delete_selected();
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        let env = project_env(&reloaded, "app");
        assert!(!env.contains_key("A"));
        assert!(env.contains_key("B"));
    }

    #[test]
    fn add_selector_on_project_offers_script_and_env() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{"app":{"url":"x"}}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor.select_by_test(|n| n.node_type == "project" && n.label == "app");
        editor.start_add();

        let (opts, _) = editor.add_menu.as_ref().expect("selector open");
        assert_eq!(opts.len(), 3);
        assert!(opts.contains(&AddContext::ProjectScript("app".into())));
        assert!(opts.contains(&AddContext::ProjectEnv("app".into())));
        assert!(opts.contains(&AddContext::ProjectAlias("app".into())));
    }

    #[test]
    fn add_edit_delete_ignore_pattern_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{},"ignore":["target","keep"]}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        // Add a pattern.
        editor.add_script_for_test(AddContext::IgnorePattern, "node_modules");
        // Edit an existing one by old value.
        editor.edit_value_for_test(&ignore_item_node_type("target"), "dist");
        // Delete one.
        editor.select_by_test(|n| n.node_type == ignore_item_node_type("keep"));
        editor.delete_selected();
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        assert!(reloaded.ignore.contains(&"node_modules".to_string()));
        assert!(reloaded.ignore.contains(&"dist".to_string()));
        assert!(!reloaded.ignore.contains(&"target".to_string()));
        assert!(!reloaded.ignore.contains(&"keep".to_string()));
    }

    #[test]
    fn add_and_delete_project_alias_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(
            &path,
            r#"{"projects":{"app":{"url":"x","aliases":["old"]}}}"#,
        )
        .unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        editor.add_script_for_test(AddContext::ProjectAlias("app".into()), "a1");
        editor.select_by_test(|n| n.node_type == project_alias_item_node_type("app", "old"));
        editor.delete_selected();
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        match reloaded.projects.get("app").unwrap() {
            metarepo_core::ProjectEntry::Metadata(m) => {
                assert!(m.aliases.contains(&"a1".to_string()));
                assert!(!m.aliases.contains(&"old".to_string()));
            }
            _ => panic!("expected metadata"),
        }
    }

    #[test]
    fn add_and_delete_global_alias_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{},"aliases":{"old":"x"}}"#).unwrap();

        let mut editor = ConfigEditor::new(path.clone(), vec![]).unwrap();
        // Add a new alias (chains to target-path edit).
        editor.add_script_for_test(AddContext::GlobalAlias, "ci");
        editor.edit_value_for_test(&global_alias_node_type("ci"), "tools/ci");
        // Delete the pre-existing one.
        editor.select_by_test(|n| n.node_type == global_alias_node_type("old"));
        editor.delete_selected();
        editor.save().unwrap();

        let reloaded = MetaConfig::load_from_file(&path).unwrap();
        let aliases = reloaded.aliases.unwrap();
        assert_eq!(aliases.get("ci").unwrap(), "tools/ci");
        assert!(!aliases.contains_key("old"));
    }

    #[test]
    fn quit_with_unsaved_prompts_then_discards() {
        use crossterm::event::KeyEvent;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor.state.modified = true;

        editor
            .handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .unwrap();
        assert!(editor.confirm_quit);
        assert!(!editor.state.should_quit);

        editor
            .handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .unwrap();
        assert!(editor.state.should_quit);
    }

    #[test]
    fn search_jumps_to_and_expands_match() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(
            &path,
            r#"{"projects":{"app":{"url":"x","scripts":{"deploy-prod":"echo"}}}}"#,
        )
        .unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        assert!(editor.jump_to_match("deploy"));
        // The selected node is the matching script, and its ancestors expanded.
        let selected = editor
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(editor.state.tree_state.selected)
            .unwrap();
        assert_eq!(
            selected.node_type,
            project_script_node_type("app", "deploy-prod")
        );

        assert!(!editor.jump_to_match("nonexistent-xyz"));
    }

    #[test]
    fn quit_clean_exits_without_prompt() {
        use crossterm::event::KeyEvent;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor
            .handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .unwrap();
        assert!(!editor.confirm_quit);
        assert!(editor.state.should_quit);
    }

    #[test]
    fn help_overlay_toggles_and_dismisses() {
        use crossterm::event::KeyEvent;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        assert!(!editor.show_help);

        // '?' opens the overlay.
        editor
            .handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE))
            .unwrap();
        assert!(editor.show_help);

        // While open, any key (here 'j') just dismisses it and is consumed —
        // the selection must not move.
        let before = editor.state.tree_state.selected;
        editor
            .handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))
            .unwrap();
        assert!(!editor.show_help);
        assert_eq!(editor.state.tree_state.selected, before);
    }

    #[test]
    fn help_sections_are_non_empty() {
        let sections = ConfigEditor::help_sections();
        assert!(!sections.is_empty());
        assert!(sections.iter().all(|s| !s.entries.is_empty()));
    }

    #[test]
    fn edit_marks_node_dirty_and_undo_reverts_it() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, catalog()).unwrap();
        editor.select_by_test(|n| {
            parse_setting_node_type(&n.node_type)
                .map(|(_, k)| k == "skill.search-limit")
                .unwrap_or(false)
        });

        // Drive the real value-edit path: open the editor buffer, replace it, save.
        editor.textarea = Some(TextArea::new(vec!["50".to_string()]));
        editor.state.editing = true;
        editor.save_edit();

        assert!(editor.state.modified);
        assert!(editor.dirty.contains("setting:int:skill.search-limit"));
        assert!(editor.undo_snapshot.is_some());
        // The node carries the dirty marker.
        let dirty_node = editor
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten_all())
            .find(|n| {
                parse_setting_node_type(&n.node_type)
                    .map(|(_, k)| k == "skill.search-limit")
                    .unwrap_or(false)
            })
            .unwrap();
        assert!(dirty_node.dirty);

        // Undo reverts the dirty set and the modified flag, and empties the slot.
        editor.undo();
        assert!(!editor.dirty.contains("setting:int:skill.search-limit"));
        assert!(!editor.state.modified);
        assert!(editor.undo_snapshot.is_none());
        // Second undo is a no-op (does not panic).
        editor.undo();
    }

    #[test]
    fn bool_setting_toggles_in_place_without_text_editor() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        // default_bare is a core bool setting rendered under Settings > core.
        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor.select_by_test(|n| {
            parse_setting_node_type(&n.node_type)
                .map(|(vt, k)| vt == ConfigValueType::Bool && k == "default_bare")
                .unwrap_or(false)
        });

        // Editing a bool toggles it and does NOT open a text buffer.
        editor.start_editing();
        assert!(editor.textarea.is_none());
        assert!(!editor.state.editing);
        assert!(editor.state.modified);
        assert!(editor.dirty.contains("setting:bool:default_bare"));

        let val = |e: &ConfigEditor| {
            e.tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .find(|n| n.node_type == "setting:bool:default_bare")
                .and_then(|n| n.value.clone())
                .unwrap_or_default()
        };
        // Unset -> true on first toggle, then flips back to false.
        assert_eq!(val(&editor), "true");
        editor.start_editing();
        assert_eq!(val(&editor), "false");
    }

    #[test]
    fn choice_setting_cycles_through_allowed_values() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        // A choice-constrained String setting declared in the catalog.
        let catalog =
            vec![
                metarepo_core::ConfigSetting::new("mode", "pick one", ConfigValueType::String)
                    .with_choices(["off", "required"]),
            ];
        let mut editor = ConfigEditor::new(path, catalog).unwrap();
        editor.select_by_test(|n| {
            parse_setting_node_type(&n.node_type)
                .map(|(vt, k)| vt == ConfigValueType::String && k == "mode")
                .unwrap_or(false)
        });

        let val = |e: &ConfigEditor| {
            e.tree_roots
                .iter()
                .flat_map(|r| r.flatten_all())
                .find(|n| n.node_type == "setting:string:mode")
                .and_then(|n| n.value.clone())
                .unwrap_or_default()
        };

        // Editing cycles rather than opening a text buffer: unset -> first
        // choice -> next -> wraps back to first.
        editor.start_editing();
        assert!(editor.textarea.is_none());
        assert!(!editor.state.editing);
        assert!(editor.state.modified);
        assert!(editor.dirty.contains("setting:string:mode"));
        assert_eq!(val(&editor), "off");
        editor.start_editing();
        assert_eq!(val(&editor), "required");
        editor.start_editing();
        assert_eq!(val(&editor), "off");
    }

    #[test]
    fn override_inherited_here_copies_ancestor_value_locally() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("inner")).unwrap();
        // Outer ancestor sets skill.dest; inner (edited) does not -> inherited.
        std::fs::write(
            root.join(".metarepo"),
            r#"{"projects":{},"skill":{"dest":"~/outer"}}"#,
        )
        .unwrap();
        let inner = root.join("inner").join(".meta");
        std::fs::write(&inner, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(inner, catalog()).unwrap();
        // Precondition: skill.dest is inherited, not set locally.
        assert!(editor.config.get_dotted("skill.dest").is_none());

        editor.select_by_test(|n| {
            parse_setting_node_type(&n.node_type)
                .map(|(_, k)| k == "skill.dest")
                .unwrap_or(false)
        });
        editor.override_inherited_here();

        // Now set locally to the previously-inherited value, and marked dirty.
        assert_eq!(
            editor.config.get_dotted("skill.dest"),
            Some(serde_json::json!("~/outer"))
        );
        assert!(editor.state.modified);
        // The annotation should now report local (no inherited note).
        assert!(ConfigEditor::inherited_annotation(
            &editor.config,
            &editor.ancestors,
            "skill.dest"
        )
        .is_none());
    }

    #[test]
    fn add_on_projects_section_skips_menu_for_single_option() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".meta");
        std::fs::write(&path, r#"{"projects":{}}"#).unwrap();

        let mut editor = ConfigEditor::new(path, vec![]).unwrap();
        editor.select_by_test(|n| n.node_type == "section" && n.label == "Projects");
        editor.start_add();

        // Single option → no menu, straight to the name prompt.
        assert!(editor.add_menu.is_none());
        assert_eq!(editor.adding, Some(AddContext::NewProject));
    }
}
