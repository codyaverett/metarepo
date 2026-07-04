//! Reusable building blocks for tree-based TUI surfaces.
//!
//! These are the surface-agnostic pieces of the config editor's shell, factored
//! out so other tree screens (a status dashboard, a worktree manager, ...) can
//! reuse them instead of re-deriving layout, search, and popup geometry:
//!
//! - [`centered_rect`] — geometry for popup overlays (help, confirms).
//! - [`render_tree_pane`] — the left tree pane of a two-pane layout; returns the
//!   remaining area for the caller to fill with a surface-specific detail panel.
//! - [`search_and_reveal`] — case-insensitive search over a tree that expands
//!   ancestors of the first match and moves the selection to it.
//!
//! The detail panel, edit logic, and key handling stay with each surface; this
//! module deliberately covers only what is identical across them.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};

use super::widgets::{TreeNode, TreeState, TreeWidget};

/// A `Rect` centered within `area`, sized to `percent_x` × `percent_y` of it.
/// Used to place popup overlays (help panels, confirmations).
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

/// Render the left tree pane of a two-pane (tree | detail) layout and return the
/// right-hand area for the caller to fill with a surface-specific detail panel.
///
/// Splits `area` 50/50, draws `roots` in a bordered pane titled `title`, and
/// caches the viewport's inner height on `state` so key handlers can keep the
/// selection on screen. The returned `Rect` is the detail area.
pub fn render_tree_pane(
    frame: &mut Frame,
    area: Rect,
    roots: &[TreeNode],
    state: &mut TreeState,
    title: &str,
) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Inner height = pane height minus the top/bottom border rows.
    state.viewport_height = chunks[0].height.saturating_sub(2) as usize;

    let tree = TreeWidget::new(roots, state).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(tree, chunks[0]);

    chunks[1]
}

/// Whether `node`'s label or value contains the (already-lowercased) query.
fn node_matches(node: &TreeNode, q: &str) -> bool {
    node.label.to_lowercase().contains(q)
        || node
            .value
            .as_deref()
            .map(|v| v.to_lowercase().contains(q))
            .unwrap_or(false)
}

/// Case-insensitive search over the tree. Expands the ancestors of the first
/// matching node so it becomes visible, moves `state.selected` to it, and
/// returns whether a match was found. A blank query is a no-op returning false.
pub fn search_and_reveal(roots: &mut [TreeNode], state: &mut TreeState, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }

    fn expand_to(node: &mut TreeNode, q: &str) -> bool {
        if node_matches(node, q) {
            return true;
        }
        for c in &mut node.children {
            if expand_to(c, q) {
                node.expanded = true;
                return true;
            }
        }
        false
    }

    let mut found = false;
    for r in roots.iter_mut() {
        if expand_to(r, &q) {
            found = true;
            break;
        }
    }
    if !found {
        return false;
    }

    if let Some(idx) = roots
        .iter()
        .flat_map(|r| r.flatten(true))
        .position(|n| node_matches(n, &q))
    {
        state.selected = idx;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_is_centered_and_sized() {
        let area = Rect::new(0, 0, 100, 100);
        let r = centered_rect(60, 40, area);
        assert_eq!(r.width, 60);
        assert_eq!(r.height, 40);
        assert_eq!(r.x, 20);
        assert_eq!(r.y, 30);
    }

    fn tree() -> Vec<TreeNode> {
        let mut root = TreeNode::new("settings", "section");
        let mut group = TreeNode::new("skill", "section");
        group.add_child(TreeNode::with_value(
            "dest",
            "~/skills",
            "setting:string:skill.dest",
        ));
        root.add_child(group);
        vec![root]
    }

    #[test]
    fn search_reveals_and_selects_match() {
        let mut roots = tree();
        let mut state = TreeState::new();
        // Collapsed initially; searching a leaf expands ancestors and selects it.
        assert!(search_and_reveal(&mut roots, &mut state, "dest"));
        // The matched leaf is now the selected, visible row.
        let visible: Vec<_> = roots.iter().flat_map(|r| r.flatten(true)).collect();
        assert_eq!(visible[state.selected].label, "dest");
    }

    #[test]
    fn search_matches_on_value_too() {
        let mut roots = tree();
        let mut state = TreeState::new();
        assert!(search_and_reveal(&mut roots, &mut state, "skills"));
    }

    #[test]
    fn blank_or_missing_query_returns_false() {
        let mut roots = tree();
        let mut state = TreeState::new();
        assert!(!search_and_reveal(&mut roots, &mut state, "   "));
        assert!(!search_and_reveal(&mut roots, &mut state, "nonexistent"));
    }
}
