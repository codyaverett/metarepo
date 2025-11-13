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
        }
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
}

/// State for tree widget navigation
#[derive(Debug, Clone)]
pub struct TreeState {
    /// Currently selected index in flattened view
    pub selected: usize,
    /// Scroll offset
    pub offset: usize,
}

impl Default for TreeState {
    fn default() -> Self {
        Self {
            selected: 0,
            offset: 0,
        }
    }
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

    /// Update scroll offset based on selected and viewport height
    pub fn update_offset(&mut self, height: usize) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected.saturating_sub(height - 1);
        }
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
            spans.push(Span::styled(
                "[",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                &node.node_type,
                Style::default().fg(if is_selected {
                    Color::Cyan
                } else {
                    Color::Blue
                }),
            ));
            spans.push(Span::styled(
                "] ",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                &node.label,
                Style::default().fg(Color::White),
            ));

            // Value display
            if let Some(ref val) = node.value {
                spans.push(Span::styled(
                    ": ",
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    val,
                    Style::default().fg(if is_selected {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
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

        let items: Vec<ListItem> = visible_nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                Self::render_node(node, idx == self.state.selected, self.highlight_style)
            })
            .collect();

        let mut list = List::new(items);

        if let Some(block) = block {
            list = list.block(block);
        }

        Widget::render(list, area, buf);
    }
}
