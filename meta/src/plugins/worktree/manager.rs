//! The interactive `meta worktree tui` manager screen.
//!
//! Lists every project's extra worktrees in a navigable tree and supports
//! removing and pruning them, plus revealing a worktree's path on exit (so the
//! caller can `cd` into it). Built on the shared tree-shell primitives
//! ([`metarepo_core::tui`]), mirroring the read-only status dashboard but adding
//! write actions. Creating worktrees stays on the CLI (`meta worktree add`) for
//! now: the shared TUI has no text-input widget yet, and `add` needs interactive
//! starting-point and hook-consent prompts that cannot run under raw mode.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use metarepo_core::tui::{
    centered_rect, init_terminal, render_tree_pane, restore_terminal, Action, Breadcrumb,
    HelpSection, KeybindingHelp, MenuApp, MenuAppState, TreeNode,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use std::path::{Path, PathBuf};

use super::{
    gather_project_worktrees, prune_project_quiet, remove_worktree_quiet, short_branch_name,
    worktree_is_dirty, ProjectWorktrees, WorktreeInfo,
};

/// A remove that is armed and awaiting a confirming second `d` keypress.
struct PendingDelete {
    project_path: PathBuf,
    wt_path: PathBuf,
    /// The worktree is dirty, so removing it discards uncommitted work (force).
    dirty: bool,
    label: String,
    project_name: String,
}

/// Interactive multi-project worktree manager.
pub struct WorktreeManager {
    base_path: PathBuf,
    projects: Vec<String>,
    data: Vec<ProjectWorktrees>,
    state: MenuAppState,
    tree_roots: Vec<TreeNode>,
    show_help: bool,
    pending_delete: Option<PendingDelete>,
    /// Path printed on exit so the caller can `cd` into it.
    exit_path: Option<PathBuf>,
}

impl WorktreeManager {
    /// Build a manager for `projects` (keys relative to `base_path`), gathering
    /// their worktrees immediately.
    pub fn new(base_path: PathBuf, projects: Vec<String>) -> Self {
        let data = gather_project_worktrees(&base_path, &projects);
        let tree_roots = Self::build_tree(&base_path, &data);
        Self {
            base_path,
            projects,
            data,
            state: MenuAppState::new(),
            tree_roots,
            show_help: false,
            pending_delete: None,
            exit_path: None,
        }
    }

    /// Display label for a worktree row: its short branch name, or the directory
    /// name when detached / branchless.
    fn worktree_label(wt: &WorktreeInfo) -> String {
        if !wt.branch.is_empty() {
            short_branch_name(&wt.branch).to_string()
        } else if let Some(name) = wt.path.file_name() {
            name.to_string_lossy().to_string()
        } else {
            "(unknown)".to_string()
        }
    }

    /// Dim annotation for a worktree row: path relative to the workspace root,
    /// plus locked/detached markers.
    fn worktree_annotation(base_path: &Path, wt: &WorktreeInfo) -> String {
        let rel = wt.path.strip_prefix(base_path).unwrap_or(&wt.path);
        let mut s = rel.display().to_string();
        if wt.is_detached {
            s.push_str(" (detached)");
        }
        if wt.is_locked {
            s.push_str(" (locked)");
        }
        s
    }

    fn build_tree(base_path: &Path, data: &[ProjectWorktrees]) -> Vec<TreeNode> {
        let mut root = TreeNode::new("Worktrees", "section");
        root.expandable = true;
        root.expanded = true;
        root.depth = 0;

        for (pi, pw) in data.iter().enumerate() {
            let count = pw.worktrees.len();
            let summary = if count == 0 {
                "no worktrees".to_string()
            } else if count == 1 {
                "1 worktree".to_string()
            } else {
                format!("{count} worktrees")
            };
            let mut proj = TreeNode::with_value(
                &pw.project_name,
                summary,
                format!("proj:{}", pw.project_name),
            );
            proj.depth = 1;
            proj.expandable = count > 0;
            proj.expanded = true;

            for (wi, wt) in pw.worktrees.iter().enumerate() {
                let mut node = TreeNode::new(Self::worktree_label(wt), format!("wt:{pi}:{wi}"))
                    .with_annotation(Self::worktree_annotation(base_path, wt));
                node.depth = 2;
                node.dirty = worktree_is_dirty(&wt.path);
                proj.add_child(node);
            }
            root.add_child(proj);
        }
        vec![root]
    }

    /// Re-gather worktrees and rebuild the tree, keeping the selection in range.
    /// Clears any armed delete, since node indices may have shifted.
    fn refresh(&mut self) {
        self.pending_delete = None;
        self.data = gather_project_worktrees(&self.base_path, &self.projects);
        self.tree_roots = Self::build_tree(&self.base_path, &self.data);
        let count = self.visible_count();
        if self.state.tree_state.selected >= count {
            self.state.tree_state.selected = count.saturating_sub(1);
        }
        self.state.set_status("Refreshed");
    }

    fn help_sections() -> Vec<HelpSection> {
        vec![
            HelpSection::new(
                "Navigation",
                vec![
                    ("j / ↓", "Move down"),
                    ("k / ↑", "Move up"),
                    ("l / →", "Expand"),
                    ("h / ←", "Collapse"),
                    ("g / G", "Top / bottom"),
                ],
            ),
            HelpSection::new(
                "Worktrees",
                vec![
                    ("Enter", "Print worktree path on exit (cd into it)"),
                    ("d", "Remove selected worktree (press twice)"),
                    ("x", "Prune stale refs for the project"),
                    ("r", "Refresh"),
                    ("?", "Toggle this help"),
                    ("q / Esc", "Quit"),
                ],
            ),
        ]
    }

    /// The `(project_index, worktree_index)` for the selected row, if it is a
    /// worktree node.
    fn selected_worktree_idx(&self) -> Option<(usize, usize)> {
        let node = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)?;
        let rest = node.node_type.strip_prefix("wt:")?;
        let (pi, wi) = rest.split_once(':')?;
        Some((pi.parse().ok()?, wi.parse().ok()?))
    }

    /// The project index for the selected row, whether a project node or a
    /// worktree beneath it. `None` when the section row (or nothing) is selected.
    fn selected_project_idx(&self) -> Option<usize> {
        let node = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)?;
        if let Some(name) = node.node_type.strip_prefix("proj:") {
            return self.data.iter().position(|p| p.project_name == name);
        }
        if let Some(rest) = node.node_type.strip_prefix("wt:") {
            let pi = rest.split_once(':')?.0;
            return pi.parse().ok();
        }
        None
    }

    fn selected_worktree(&self) -> Option<(&ProjectWorktrees, &WorktreeInfo)> {
        let (pi, wi) = self.selected_worktree_idx()?;
        let pw = self.data.get(pi)?;
        let wt = pw.worktrees.get(wi)?;
        Some((pw, wt))
    }

    /// Arm or confirm a remove of the selected worktree.
    fn handle_delete(&mut self) {
        let Some((pw, wt)) = self.selected_worktree() else {
            self.state.set_status("Select a worktree to remove");
            return;
        };
        let project_path = pw.project_path.clone();
        let wt_path = wt.path.clone();
        let dirty = worktree_is_dirty(&wt.path);
        let label = Self::worktree_label(wt);
        let project_name = pw.project_name.clone();

        // Confirm when the armed delete still matches the current selection.
        let confirmed = self
            .pending_delete
            .as_ref()
            .is_some_and(|p| p.wt_path == wt_path);

        if confirmed {
            let pending = self.pending_delete.take().unwrap();
            match remove_worktree_quiet(&pending.project_path, &pending.wt_path, pending.dirty) {
                Ok(()) => {
                    self.refresh();
                    self.state.set_status(format!(
                        "Removed {} ({})",
                        pending.label, pending.project_name
                    ));
                }
                Err(e) => self
                    .state
                    .set_status(format!("Remove {} failed: {e}", pending.label)),
            }
            return;
        }

        let msg = if dirty {
            format!("{label} is DIRTY - press d again to remove and DISCARD changes")
        } else {
            format!("Press d again to remove {label} from {project_name}")
        };
        self.state.set_status(msg);
        self.pending_delete = Some(PendingDelete {
            project_path,
            wt_path,
            dirty,
            label,
            project_name,
        });
    }

    /// Prune stale worktree references for the selected project, or every
    /// project when the top-level section row is selected.
    fn handle_prune(&mut self) {
        let targets: Vec<usize> = match self.selected_project_idx() {
            Some(pi) => vec![pi],
            None => (0..self.data.len()).collect(),
        };
        if targets.is_empty() {
            self.state.set_status("Nothing to prune");
            return;
        }
        let mut pruned = 0usize;
        let mut errors = 0usize;
        for pi in &targets {
            match prune_project_quiet(&self.data[*pi].project_path) {
                Ok(n) => pruned += n,
                Err(_) => errors += 1,
            }
        }
        self.refresh();
        if errors > 0 {
            self.state.set_status(format!(
                "Pruned {pruned} entries ({errors} project(s) failed)"
            ));
        } else {
            self.state
                .set_status(format!("Pruned {pruned} stale entries"));
        }
    }

    /// Select the current worktree for `cd`-on-exit: record its path and quit.
    fn handle_reveal(&mut self) {
        if let Some((_, wt)) = self.selected_worktree() {
            self.exit_path = Some(wt.path.clone());
            self.state.should_quit = true;
        }
    }

    /// Lines for the detail pane describing the selected worktree.
    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some((pw, wt)) = self.selected_worktree() else {
            return vec![Line::from(Span::styled(
                "Select a worktree",
                Style::default().fg(Color::Gray),
            ))];
        };

        let row = |label: &str, val: String| {
            Line::from(vec![
                Span::styled(format!("{label}: "), Style::default().fg(Color::Gray)),
                Span::raw(val),
            ])
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Project: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    pw.project_name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            row("Branch", Self::worktree_label(wt)),
            row("Path", wt.path.display().to_string()),
        ];
        if let Some(head) = &wt.head {
            lines.push(row("HEAD", head.chars().take(10).collect()));
        }
        let mut flags = Vec::new();
        if worktree_is_dirty(&wt.path) {
            flags.push("dirty");
        }
        if wt.is_locked {
            flags.push("locked");
        }
        if wt.is_detached {
            flags.push("detached");
        }
        lines.push(row(
            "State",
            if flags.is_empty() {
                "clean".to_string()
            } else {
                flags.join(", ")
            },
        ));

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter: cd path   d: remove   x: prune   r: refresh   q: quit",
            Style::default().fg(Color::DarkGray),
        )));
        lines
    }

    /// Run the manager event loop, returning the path to `cd` into (if any).
    pub fn run(&mut self) -> Result<Option<PathBuf>> {
        let mut terminal = init_terminal()?;
        let result = MenuApp::run(self, &mut terminal);
        restore_terminal(terminal)?;
        result?;
        Ok(self.exit_path.take())
    }

    fn visible_count(&self) -> usize {
        self.tree_roots.iter().flat_map(|r| r.flatten(true)).count()
    }

    fn sync_scroll(&mut self) {
        let h = self.state.tree_state.viewport_height;
        self.state.tree_state.update_offset(h);
    }

    fn toggle_selected(&mut self) {
        let idx = self.state.tree_state.selected;
        let roots = self.get_tree_roots_mut();
        let visible: Vec<_> = roots.iter_mut().flat_map(|r| r.flatten_mut()).collect();
        if let Some(&ptr) = visible.get(idx) {
            // SAFETY: pointer comes from this tree and is not aliased here.
            unsafe {
                (*ptr).toggle();
            }
        }
    }

    /// True when the selected row is a worktree node (so Enter reveals its path
    /// rather than expanding/collapsing).
    fn selection_is_worktree(&self) -> bool {
        self.selected_worktree_idx().is_some()
    }
}

impl MenuApp for WorktreeManager {
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

    // Worktrees are managed by explicit keys, not the generic edit/save flow.
    fn save(&mut self) -> Result<()> {
        Ok(())
    }
    fn is_selected_editable(&self) -> bool {
        false
    }
    fn start_editing(&mut self) {}
    fn save_edit(&mut self) {}
    fn cancel_edit(&mut self) {}

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Help overlay consumes the next key to dismiss.
        if self.show_help {
            self.show_help = false;
            return Ok(true);
        }

        // Manager-specific actions, intercepted before the shared keymap.
        match (key.code, key.modifiers) {
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.refresh();
                return Ok(true);
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                self.handle_delete();
                return Ok(true);
            }
            (KeyCode::Char('x'), KeyModifiers::NONE) => {
                self.handle_prune();
                return Ok(true);
            }
            // Enter reveals a worktree's path; on non-worktree rows it falls
            // through to expand/collapse.
            (KeyCode::Enter, KeyModifiers::NONE) if self.selection_is_worktree() => {
                self.handle_reveal();
                return Ok(!self.state.should_quit);
            }
            _ => {}
        }

        // Any other key cancels an armed delete.
        self.pending_delete = None;

        match metarepo_core::tui::handle_key(key, false) {
            Action::NavigateUp => {
                self.state.tree_state.select_previous();
                self.update_breadcrumb_for_selected();
                self.sync_scroll();
            }
            Action::NavigateDown => {
                let count = self.visible_count();
                self.state.tree_state.select_next(count);
                self.update_breadcrumb_for_selected();
                self.sync_scroll();
            }
            Action::NavigateTop => {
                self.state.tree_state.select_first();
                self.update_breadcrumb_for_selected();
                self.sync_scroll();
            }
            Action::NavigateBottom => {
                let count = self.visible_count();
                self.state.tree_state.select_last(count);
                self.update_breadcrumb_for_selected();
                self.sync_scroll();
            }
            Action::NavigatePageUp => {
                for _ in 0..10 {
                    self.state.tree_state.select_previous();
                }
                self.sync_scroll();
            }
            Action::NavigatePageDown => {
                let count = self.visible_count();
                for _ in 0..10 {
                    self.state.tree_state.select_next(count);
                }
                self.sync_scroll();
            }
            Action::ToggleExpand => self.toggle_selected(),
            Action::CollapseParent => self.toggle_selected(),
            Action::Help => self.show_help = true,
            Action::Quit => self.state.should_quit = true,
            _ => {}
        }

        Ok(!self.state.should_quit)
    }

    fn render_breadcrumb(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Breadcrumb::new(&self.state.breadcrumb), area);
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        let detail_area = render_tree_pane(
            frame,
            area,
            &self.tree_roots,
            &mut self.state.tree_state,
            "Worktrees",
        );
        let panel = Paragraph::new(self.detail_lines())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Details ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(panel, detail_area);
    }

    fn render_context_bar(&mut self, frame: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::raw(":Nav  "),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::raw(":Path  "),
            Span::styled("d", Style::default().fg(Color::Cyan)),
            Span::raw(":Remove  "),
            Span::styled("x", Style::default().fg(Color::Cyan)),
            Span::raw(":Prune  "),
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::raw(":Refresh  "),
            Span::styled("?", Style::default().fg(Color::Cyan)),
            Span::raw(":Help  "),
            Span::styled("q", Style::default().fg(Color::Red)),
            Span::raw(":Quit   "),
            Span::styled(
                self.state.status_message.clone(),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line).block(Block::default().borders(Borders::TOP)),
            area,
        );
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .split(frame.area());
        self.render_breadcrumb(frame, chunks[0]);
        self.render_content(frame, chunks[1]);
        self.render_context_bar(frame, chunks[2]);

        if self.show_help {
            let area = centered_rect(60, 70, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(
                KeybindingHelp::new("Worktree manager keys", Self::help_sections()),
                area,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<ProjectWorktrees> {
        vec![
            ProjectWorktrees {
                project_name: "api".into(),
                project_path: PathBuf::from("/ws/api"),
                worktrees: vec![WorktreeInfo {
                    branch: "refs/heads/feature".into(),
                    path: PathBuf::from("/ws/api/.worktrees/feature"),
                    is_bare: false,
                    is_detached: false,
                    is_locked: false,
                    head: Some("abcdef1234567890".into()),
                }],
            },
            ProjectWorktrees {
                project_name: "web".into(),
                project_path: PathBuf::from("/ws/web"),
                worktrees: vec![],
            },
        ]
    }

    fn manager_with(data: Vec<ProjectWorktrees>) -> WorktreeManager {
        let base = PathBuf::from("/ws");
        let tree_roots = WorktreeManager::build_tree(&base, &data);
        WorktreeManager {
            base_path: base,
            projects: vec!["api".into(), "web".into()],
            data,
            state: MenuAppState::new(),
            tree_roots,
            show_help: false,
            pending_delete: None,
            exit_path: None,
        }
    }

    #[test]
    fn build_tree_lays_out_projects_and_worktrees() {
        let roots = WorktreeManager::build_tree(&PathBuf::from("/ws"), &sample());
        let root = &roots[0];
        assert_eq!(root.label, "Worktrees");
        assert_eq!(root.children.len(), 2);

        let api = &root.children[0];
        assert_eq!(api.node_type, "proj:api");
        assert_eq!(api.value.as_deref(), Some("1 worktree"));
        assert!(api.expandable);
        assert_eq!(api.children.len(), 1);

        let wt = &api.children[0];
        assert_eq!(wt.node_type, "wt:0:0");
        assert_eq!(wt.label, "feature");
        assert_eq!(wt.annotation.as_deref(), Some("api/.worktrees/feature"));

        // A project with no worktrees is shown but not expandable.
        let web = &root.children[1];
        assert_eq!(web.node_type, "proj:web");
        assert_eq!(web.value.as_deref(), Some("no worktrees"));
        assert!(!web.expandable);
        assert!(web.children.is_empty());
    }

    #[test]
    fn selection_resolves_worktree_and_project() {
        let mut m = manager_with(sample());
        // Row 0 = section, row 1 = api project, row 2 = its worktree, row 3 = web.
        m.state.tree_state.selected = 2;
        assert_eq!(m.selected_worktree_idx(), Some((0, 0)));
        assert_eq!(m.selected_project_idx(), Some(0));
        assert!(m.selection_is_worktree());

        // On the project row, no worktree but the project resolves.
        m.state.tree_state.selected = 1;
        assert_eq!(m.selected_worktree_idx(), None);
        assert_eq!(m.selected_project_idx(), Some(0));
        assert!(!m.selection_is_worktree());

        // On the section row, neither resolves (prune would target all).
        m.state.tree_state.selected = 0;
        assert_eq!(m.selected_project_idx(), None);
    }

    #[test]
    fn delete_arms_then_confirms_against_same_selection() {
        let mut m = manager_with(sample());
        m.state.tree_state.selected = 2; // the api worktree

        // First press arms a pending delete without acting.
        m.handle_delete();
        assert!(m.pending_delete.is_some());
        assert_eq!(m.data[0].worktrees.len(), 1);
    }
}
