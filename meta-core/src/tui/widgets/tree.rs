//! Tree widget for hierarchical navigation

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Widget},
};

/// A node in the tree structure
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Display label for the node
    pub label: String,
    /// Whether this node can have children
    pub expandable: bool,
    /// Whether this node is currently expanded
    pub expanded: bool,
    /// Child nodes
    pub children: Vec<TreeNode>,
    /// Depth level in the tree (0 = root)
    pub depth: usize,
    /// Optional value associated with this node
    pub value: Option<String>,
    /// Node type identifier (for rendering and behavior)
    pub node_type: String,
    /// Optional dim annotation shown after the value (e.g. the cascade source
    /// of an inherited setting). Purely informational — never edited or saved.
    pub annotation: Option<String>,
}

impl TreeNode {
    /// Create a new tree node
    pub fn new(label: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            expandable: false,
            expanded: false,
            children: Vec::new(),
            depth: 0,
            value: None,
            node_type: node_type.into(),
            annotation: None,
        }
    }

    /// Create a node with children
    pub fn with_children(
        label: impl Into<String>,
        node_type: impl Into<String>,
        children: Vec<TreeNode>,
    ) -> Self {
        let has_children = !children.is_empty();
        Self {
            label: label.into(),
            expandable: has_children,
            expanded: false,
            children,
            depth: 0,
            value: None,
            node_type: node_type.into(),
            annotation: None,
        }
    }

    /// Create a node with a value
    pub fn with_value(
        label: impl Into<String>,
        value: impl Into<String>,
        node_type: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            expandable: false,
            expanded: false,
            children: Vec::new(),
            depth: 0,
            value: Some(value.into()),
            node_type: node_type.into(),
            annotation: None,
        }
    }

    /// Attach a dim informational annotation shown after the value.
    pub fn with_annotation(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }

    /// Toggle expansion state
    pub fn toggle(&mut self) {
        if self.expandable {
            self.expanded = !self.expanded;
        }
    }

    /// Expand this node
    pub fn expand(&mut self) {
        if self.expandable {
            self.expanded = true;
        }
    }

    /// Collapse this node
    pub fn collapse(&mut self) {
        if self.expandable {
            self.expanded = false;
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, child: TreeNode) {
        self.children.push(child);
        self.expandable = true;
    }

    /// Get flattened list of visible nodes (for rendering)
    pub fn flatten(&self, include_self: bool) -> Vec<&TreeNode> {
        let mut result = Vec::new();
        if include_self {
            result.push(self);
        }
        if self.expanded {
            for child in &self.children {
                result.extend(child.flatten(true));
            }
        }
        result
    }

    /// Get mutable flattened list of visible nodes
    /// Note: This method is complex due to borrow checker limitations with recursive structures
    pub fn flatten_mut(&mut self) -> Vec<*mut TreeNode> {
        let mut result = Vec::new();
        result.push(self as *mut TreeNode);

        if self.expanded {
            for child in &mut self.children {
                result.extend(child.flatten_mut());
            }
        }
        result
    }

    /// Every node in the subtree (self + all descendants) regardless of
    /// expansion. Used when persisting edits, which must not depend on whether
    /// a node's parent happens to be expanded in the view.
    pub fn flatten_all(&self) -> Vec<&TreeNode> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.flatten_all());
        }
        result
    }

    /// Mutable counterpart of [`flatten_all`](Self::flatten_all).
    pub fn flatten_all_mut(&mut self) -> Vec<*mut TreeNode> {
        let mut result = vec![self as *mut TreeNode];
        for child in &mut self.children {
            result.extend(child.flatten_all_mut());
        }
        result
    }
}

/// State for tree widget navigation
#[derive(Debug, Clone, Default)]
pub struct TreeState {
    /// Currently selected index in flattened view
    pub selected: usize,
    /// Scroll offset (index of the first row drawn at the top of the viewport)
    pub offset: usize,
    /// Inner height of the tree viewport, cached from the last render so key
    /// handlers can keep the selection on screen without re-deriving the layout.
    pub viewport_height: usize,
}

impl TreeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Select next item
    pub fn select_next(&mut self, max: usize) {
        if self.selected < max.saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Select previous item
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    /// Jump to top
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }

    /// Jump to bottom
    pub fn select_last(&mut self, max: usize) {
        self.selected = max.saturating_sub(1);
    }

    /// Scroll the viewport the minimum amount needed to keep the selected row
    /// visible. No-op when the row is already on screen.
    pub fn update_offset(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
    }

    /// After expanding the selected node, scroll down so that as much of its
    /// freshly revealed subtree as possible is visible.
    ///
    /// `subtree_last` is the flattened index of the deepest/last descendant now
    /// visible under the selected node. The selected (parent) row is kept on
    /// screen: if the whole subtree already fits, nothing moves; otherwise the
    /// viewport scrolls down just far enough to show the most children without
    /// pushing the parent off the top. Works at any tree depth.
    pub fn reveal_subtree(&mut self, subtree_last: usize, height: usize) {
        if height == 0 {
            return;
        }
        // Make sure the parent row itself is on screen first.
        self.update_offset(height);
        // If the last descendant already fits in the viewport, leave it alone.
        if subtree_last < self.offset + height {
            return;
        }
        // Bring the last descendant to the bottom edge, but never scroll past
        // the parent — keep it visible as the anchor for the expansion.
        let bottom_anchored = subtree_last + 1 - height;
        self.offset = bottom_anchored.min(self.selected);
    }
}

/// Tree widget for rendering hierarchical data
pub struct TreeWidget<'a> {
    /// Root nodes
    pub roots: &'a [TreeNode],
    /// Current state
    pub state: &'a TreeState,
    /// Block for border and title
    pub block: Option<Block<'a>>,
    /// Highlight style for selected item
    pub highlight_style: Style,
}

impl<'a> TreeWidget<'a> {
    pub fn new(roots: &'a [TreeNode], state: &'a TreeState) -> Self {
        Self {
            roots,
            state,
            block: None,
            highlight_style: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    /// Get flattened visible nodes from all roots
    fn get_visible_nodes(&self) -> Vec<&TreeNode> {
        let mut nodes = Vec::new();
        for root in self.roots {
            nodes.extend(root.flatten(true));
        }
        nodes
    }

    /// Render a single tree node as a ListItem
    fn render_node(node: &TreeNode, is_selected: bool, _highlight_style: Style) -> ListItem<'_> {
        let indent = "  ".repeat(node.depth);

        // Expansion icon for containers
        let icon = if node.expandable {
            if node.expanded {
                "▼ "
            } else {
                "▶ "
            }
        } else if node.value.is_some() {
            // Editable item indicator
            "→ "
        } else {
            "  "
        };

        // Build the line with styled components for better visual feedback
        let mut spans = Vec::new();

        // Indent
        if !indent.is_empty() {
            spans.push(Span::raw(indent));
        }

        // Icon
        spans.push(Span::styled(
            icon,
            Style::default().fg(if is_selected {
                Color::Yellow
            } else {
                Color::DarkGray
            }),
        ));

        // Label with type indicator for editable items
        if node.value.is_some() {
            // Editable item - show with brackets
            spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                &node.node_type,
                Style::default().fg(if is_selected {
                    Color::Cyan
                } else {
                    Color::Blue
                }),
            ));
            spans.push(Span::styled("] ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(&node.label, Style::default().fg(Color::White)));

            // Value display
            if let Some(ref val) = node.value {
                spans.push(Span::styled(": ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    val,
                    Style::default().fg(if is_selected {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ));
            }

            // Dim informational annotation (e.g. cascade source) after value.
            if let Some(ref note) = node.annotation {
                spans.push(Span::styled(
                    format!("  {}", note),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ));
            }
        } else {
            // Container item - just show label
            spans.push(Span::styled(
                &node.label,
                Style::default().fg(if is_selected {
                    Color::White
                } else {
                    Color::Gray
                }),
            ));
        }

        // Apply highlight background to selected item
        if is_selected {
            for span in &mut spans {
                span.style = span.style.bg(Color::DarkGray);
            }
        }

        ListItem::new(Line::from(spans))
    }
}

impl<'a> Widget for TreeWidget<'a> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let block = self.block.take();
        let visible_nodes = self.get_visible_nodes();

        // The List widget itself does not scroll, so apply the scroll offset by
        // slicing the visible rows to the window starting at `offset`. Without
        // this, rows past the bottom edge (e.g. children of a just-expanded node
        // near the end of the list) are simply never drawn.
        let inner = block.as_ref().map(|b| b.inner(area)).unwrap_or(area);
        let height = inner.height as usize;
        let offset = self.state.offset.min(visible_nodes.len().saturating_sub(1));
        let end = visible_nodes.len().min(offset + height.max(1));

        let items: Vec<ListItem> = visible_nodes[offset..end]
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                let abs = offset + idx;
                Self::render_node(node, abs == self.state.selected, self.highlight_style)
            })
            .collect();

        let mut list = List::new(items);

        if let Some(block) = block {
            list = list.block(block);
        }

        Widget::render(list, area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(selected: usize, offset: usize) -> TreeState {
        TreeState {
            selected,
            offset,
            viewport_height: 0,
        }
    }

    #[test]
    fn update_offset_scrolls_down_to_show_selected() {
        // Viewport height 5, selection moved below the window.
        let mut s = state(7, 0);
        s.update_offset(5);
        // Selected row must be the last visible: offset = 7 - 5 + 1 = 3.
        assert_eq!(s.offset, 3);
        assert!(s.selected >= s.offset && s.selected < s.offset + 5);
    }

    #[test]
    fn update_offset_scrolls_up_to_show_selected() {
        let mut s = state(2, 6);
        s.update_offset(5);
        assert_eq!(s.offset, 2);
    }

    #[test]
    fn update_offset_noop_when_visible() {
        let mut s = state(4, 2);
        s.update_offset(5); // window covers rows 2..=6, selected 4 is inside
        assert_eq!(s.offset, 2);
    }

    #[test]
    fn update_offset_guards_zero_height() {
        let mut s = state(9, 3);
        s.update_offset(0);
        assert_eq!(s.offset, 3); // unchanged, no panic
    }

    #[test]
    fn reveal_subtree_noop_when_whole_subtree_fits() {
        // Expanded parent at row 1, last child at row 4, height 10 — all visible.
        let mut s = state(1, 0);
        s.reveal_subtree(4, 10);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn reveal_subtree_scrolls_to_show_children_keeping_parent() {
        // Parent at row 8 near the bottom, subtree ends at row 12, height 5.
        // Children overflow, so scroll down — but not past the parent (row 8).
        let mut s = state(8, 0);
        s.reveal_subtree(12, 5);
        // bottom-anchored would be 12 - 5 + 1 = 8; min(8, parent=8) = 8.
        assert_eq!(s.offset, 8);
        assert!(s.selected >= s.offset); // parent stays visible
    }

    #[test]
    fn reveal_subtree_caps_offset_at_parent_for_huge_subtree() {
        // Subtree far taller than the viewport: keep the parent pinned at the top
        // so the maximum number of children show beneath it.
        let mut s = state(3, 0);
        s.reveal_subtree(40, 6);
        assert_eq!(s.offset, 3); // parent at top edge
    }

    #[test]
    fn reveal_subtree_scrolls_down_just_enough_for_small_subtree() {
        // Parent at row 6, small subtree ending row 9, height 5, starting at top.
        // Whole subtree (rows 6..=9) doesn't fit window 0..=4, so scroll down to
        // bottom-anchor row 9: offset = 9 - 5 + 1 = 5 (parent row 6 still shown).
        let mut s = state(6, 0);
        s.reveal_subtree(9, 5);
        assert_eq!(s.offset, 5);
        assert!(s.selected >= s.offset && s.selected < s.offset + 5);
    }
}
