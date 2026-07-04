//! The interactive `meta status` dashboard screen.

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
use std::path::PathBuf;

use super::{gather_all, RepoState, RepoStatus};

/// Read-only multi-repo status dashboard.
pub struct Dashboard {
    base_path: PathBuf,
    projects: Vec<String>,
    statuses: Vec<RepoStatus>,
    state: MenuAppState,
    tree_roots: Vec<TreeNode>,
    show_help: bool,
}

impl Dashboard {
    /// Build a dashboard for `projects` (keys relative to `base_path`),
    /// gathering their status immediately.
    pub fn new(base_path: PathBuf, projects: Vec<String>) -> Self {
        let statuses = gather_all(&base_path, &projects);
        let tree_roots = Self::build_tree(&statuses);
        Self {
            base_path,
            projects,
            statuses,
            state: MenuAppState::new(),
            tree_roots,
            show_help: false,
        }
    }

    fn build_tree(statuses: &[RepoStatus]) -> Vec<TreeNode> {
        let mut root = TreeNode::new("Projects", "section");
        root.expandable = true;
        root.expanded = true;
        root.depth = 0;
        for s in statuses {
            let mut node =
                TreeNode::with_value(&s.name, s.state.summary(), format!("repo:{}", s.name));
            node.depth = 1;
            if matches!(s.state, RepoState::Ok { dirty, .. } if dirty > 0)
                || matches!(s.state, RepoState::Ok { behind, .. } if behind > 0)
            {
                // Flag repos needing attention with the shared dirty marker.
                node.dirty = true;
            }
            root.add_child(node);
        }
        vec![root]
    }

    /// Re-gather status for all projects and rebuild the tree, keeping the
    /// selection in range.
    fn refresh(&mut self) {
        self.statuses = gather_all(&self.base_path, &self.projects);
        self.tree_roots = Self::build_tree(&self.statuses);
        let count = self.tree_roots.iter().flat_map(|r| r.flatten(true)).count();
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
                    ("l / → / Enter", "Expand"),
                    ("h / ←", "Collapse"),
                    ("g / G", "Top / bottom"),
                ],
            ),
            HelpSection::new(
                "Dashboard",
                vec![
                    ("f", "Fetch the selected repo"),
                    ("p", "Pull (fast-forward) the selected repo"),
                    ("r", "Refresh status"),
                    ("?", "Toggle this help"),
                    ("q / Esc", "Quit"),
                ],
            ),
        ]
    }

    /// The status for the currently-selected tree row, if it is a repo node.
    fn selected_status(&self) -> Option<&RepoStatus> {
        let node = self
            .tree_roots
            .iter()
            .flat_map(|r| r.flatten(true))
            .nth(self.state.tree_state.selected)?;
        let name = node.node_type.strip_prefix("repo:")?;
        self.statuses.iter().find(|s| s.name == name)
    }

    /// Lines for the detail pane describing the selected repo.
    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some(status) = self.selected_status() else {
            return vec![Line::from(Span::styled(
                "Select a project",
                Style::default().fg(Color::Gray),
            ))];
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Project: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    status.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        match &status.state {
            RepoState::Ok {
                branch,
                ahead,
                behind,
                dirty,
            } => {
                let row = |label: &str, val: String| {
                    Line::from(vec![
                        Span::styled(format!("{label}: "), Style::default().fg(Color::Gray)),
                        Span::raw(val),
                    ])
                };
                lines.push(row("Branch", branch.clone()));
                lines.push(row("Ahead", ahead.to_string()));
                lines.push(row("Behind", behind.to_string()));
                lines.push(row(
                    "Working tree",
                    if *dirty == 0 {
                        "clean".to_string()
                    } else {
                        format!("{dirty} change(s)")
                    },
                ));
            }
            other => lines.push(Line::from(Span::styled(
                other.summary(),
                Style::default().fg(Color::Yellow),
            ))),
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "f: fetch   p: pull   r: refresh   ?: help   q: quit",
            Style::default().fg(Color::DarkGray),
        )));
        lines
    }

    /// Run the dashboard event loop.
    pub fn run(&mut self) -> Result<()> {
        let mut terminal = init_terminal()?;
        let result = MenuApp::run(self, &mut terminal);
        restore_terminal(terminal)?;
        result
    }
}

impl MenuApp for Dashboard {
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

    // Read-only surface: nothing to save or edit.
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
        // Dashboard-specific actions.
        match (key.code, key.modifiers) {
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.refresh();
                return Ok(true);
            }
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                self.run_repo_op("fetch", super::fetch);
                return Ok(true);
            }
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                self.run_repo_op("pull", super::pull);
                return Ok(true);
            }
            _ => {}
        }

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
            "Workspace Status",
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
            Span::styled("f", Style::default().fg(Color::Cyan)),
            Span::raw(":Fetch  "),
            Span::styled("p", Style::default().fg(Color::Cyan)),
            Span::raw(":Pull  "),
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
                KeybindingHelp::new("Status dashboard keys", Self::help_sections()),
                area,
            );
        }
    }
}

impl Dashboard {
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

    /// Run a git operation (`op`) on the selected repo, then refresh and report
    /// the outcome in the status line. Skips non-existent / non-git selections.
    /// Blocks on the network while the operation runs.
    fn run_repo_op(&mut self, verb: &str, op: fn(&std::path::Path) -> Result<(), String>) {
        let Some(status) = self.selected_status() else {
            self.state.set_status("Select a project first");
            return;
        };
        if matches!(status.state, RepoState::Missing | RepoState::NotGit) {
            self.state.set_status(format!(
                "Cannot {verb}: {} is not a cloned repo",
                status.name
            ));
            return;
        }
        let name = status.name.clone();
        let path = self.base_path.join(&name);
        match op(&path) {
            Ok(()) => {
                self.refresh();
                self.state.set_status(format!("{verb}: {name} done"));
            }
            Err(e) => self.state.set_status(format!("{verb} {name} failed: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statuses() -> Vec<RepoStatus> {
        vec![
            RepoStatus {
                name: "clean".into(),
                state: RepoState::Ok {
                    branch: "main".into(),
                    ahead: 0,
                    behind: 0,
                    dirty: 0,
                },
            },
            RepoStatus {
                name: "work".into(),
                state: RepoState::Ok {
                    branch: "dev".into(),
                    ahead: 1,
                    behind: 2,
                    dirty: 3,
                },
            },
            RepoStatus {
                name: "gone".into(),
                state: RepoState::Missing,
            },
        ]
    }

    #[test]
    fn build_tree_makes_a_row_per_repo_with_summary() {
        let roots = Dashboard::build_tree(&statuses());
        let root = &roots[0];
        assert_eq!(root.label, "Projects");
        assert_eq!(root.children.len(), 3);

        let clean = &root.children[0];
        assert_eq!(clean.node_type, "repo:clean");
        assert_eq!(clean.value.as_deref(), Some("main clean"));
        assert!(!clean.dirty);

        // A dirty/behind repo is flagged for attention.
        let work = &root.children[1];
        assert_eq!(work.value.as_deref(), Some("dev +1 -2 *3"));
        assert!(work.dirty);

        let gone = &root.children[2];
        assert_eq!(gone.value.as_deref(), Some("(missing)"));
    }

    #[test]
    fn selected_status_resolves_repo_under_cursor() {
        let mut dash = Dashboard {
            base_path: std::path::PathBuf::from("/ws"),
            projects: vec!["clean".into(), "work".into(), "gone".into()],
            statuses: statuses(),
            state: MenuAppState::new(),
            tree_roots: Dashboard::build_tree(&statuses()),
            show_help: false,
        };
        // Row 0 is the "Projects" section; row 1 is the first repo.
        dash.state.tree_state.selected = 1;
        assert_eq!(
            dash.selected_status().map(|s| s.name.as_str()),
            Some("clean")
        );
        dash.state.tree_state.selected = 2;
        assert_eq!(
            dash.selected_status().map(|s| s.name.as_str()),
            Some("work")
        );
    }
}
