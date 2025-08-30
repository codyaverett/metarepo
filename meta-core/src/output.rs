use colored::*;
use serde::{Serialize, Deserialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Human,
    Ai,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Human
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Human => write!(f, "human"),
            OutputFormat::Ai => write!(f, "ai"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" => Ok(OutputFormat::Human),
            "ai" => Ok(OutputFormat::Ai),
            "json" => Ok(OutputFormat::Json),
            _ => Err(format!("Invalid output format: {}. Valid options are: human, ai, json", s)),
        }
    }
}

pub trait OutputFormatter {
    fn format(&self, format: OutputFormat) -> String;
}

pub struct TableOutput {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl TableOutput {
    pub fn new(headers: Vec<String>) -> Self {
        Self {
            headers,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }
}

impl OutputFormatter for TableOutput {
    fn format(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Human => self.format_human(),
            OutputFormat::Ai => self.format_ai(),
            OutputFormat::Json => self.format_json(),
        }
    }
}

impl TableOutput {
    fn format_human(&self) -> String {
        let mut output = String::new();
        
        // Calculate column widths
        let mut widths = self.headers.iter().map(|h| h.len()).collect::<Vec<_>>();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }
        
        // Print headers
        for (i, header) in self.headers.iter().enumerate() {
            if i > 0 {
                output.push_str(" │ ");
            }
            output.push_str(&format!("{:width$}", header.cyan().bold(), width = widths[i]));
        }
        output.push('\n');
        
        // Print separator
        for (i, width) in widths.iter().enumerate() {
            if i > 0 {
                output.push_str("─┼─");
            }
            output.push_str(&"─".repeat(*width));
        }
        output.push('\n');
        
        // Print rows
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i > 0 {
                    output.push_str(" │ ");
                }
                if i < widths.len() {
                    output.push_str(&format!("{:width$}", cell, width = widths[i]));
                } else {
                    output.push_str(cell);
                }
            }
            output.push('\n');
        }
        
        output
    }
    
    fn format_ai(&self) -> String {
        let mut output = String::new();
        output.push_str("| ");
        output.push_str(&self.headers.join(" | "));
        output.push_str(" |\n|");
        output.push_str(&self.headers.iter().map(|_| "---").collect::<Vec<_>>().join("|"));
        output.push_str("|\n");
        
        for row in &self.rows {
            output.push_str("| ");
            output.push_str(&row.join(" | "));
            output.push_str(" |\n");
        }
        
        output
    }
    
    fn format_json(&self) -> String {
        let mut objects = Vec::new();
        for row in &self.rows {
            let mut obj = serde_json::Map::new();
            for (i, cell) in row.iter().enumerate() {
                if i < self.headers.len() {
                    obj.insert(self.headers[i].clone(), serde_json::Value::String(cell.clone()));
                }
            }
            objects.push(serde_json::Value::Object(obj));
        }
        serde_json::to_string_pretty(&objects).unwrap_or_default()
    }
}

pub struct ListOutput {
    items: Vec<ListItem>,
}

pub struct ListItem {
    pub label: String,
    pub value: String,
    pub status: Option<ListItemStatus>,
}

#[derive(Debug, Clone)]
pub enum ListItemStatus {
    Success,
    Warning,
    Error,
    Info,
}

impl ListOutput {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, label: String, value: String, status: Option<ListItemStatus>) {
        self.items.push(ListItem { label, value, status });
    }
}

impl OutputFormatter for ListOutput {
    fn format(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Human => self.format_human(),
            OutputFormat::Ai => self.format_ai(),
            OutputFormat::Json => self.format_json(),
        }
    }
}

impl ListOutput {
    fn format_human(&self) -> String {
        let mut output = String::new();
        for item in &self.items {
            let status_symbol = match &item.status {
                Some(ListItemStatus::Success) => "✓".green(),
                Some(ListItemStatus::Warning) => "⚠".yellow(),
                Some(ListItemStatus::Error) => "✗".red(),
                Some(ListItemStatus::Info) => "ℹ".blue(),
                None => "•".normal(),
            };
            output.push_str(&format!("{} {} {}\n", status_symbol, item.label.bold(), item.value));
        }
        output
    }
    
    fn format_ai(&self) -> String {
        let mut output = String::new();
        for item in &self.items {
            let status_prefix = match &item.status {
                Some(ListItemStatus::Success) => "✓",
                Some(ListItemStatus::Warning) => "⚠",
                Some(ListItemStatus::Error) => "✗",
                Some(ListItemStatus::Info) => "ℹ",
                None => "-",
            };
            output.push_str(&format!("{} **{}**: {}\n", status_prefix, item.label, item.value));
        }
        output
    }
    
    fn format_json(&self) -> String {
        let objects: Vec<serde_json::Value> = self.items.iter().map(|item| {
            let mut obj = serde_json::Map::new();
            obj.insert("label".to_string(), serde_json::Value::String(item.label.clone()));
            obj.insert("value".to_string(), serde_json::Value::String(item.value.clone()));
            if let Some(status) = &item.status {
                let status_str = match status {
                    ListItemStatus::Success => "success",
                    ListItemStatus::Warning => "warning",
                    ListItemStatus::Error => "error",
                    ListItemStatus::Info => "info",
                };
                obj.insert("status".to_string(), serde_json::Value::String(status_str.to_string()));
            }
            serde_json::Value::Object(obj)
        }).collect();
        serde_json::to_string_pretty(&objects).unwrap_or_default()
    }
}

pub struct TreeOutput {
    root: TreeNode,
}

pub struct TreeNode {
    pub label: String,
    pub children: Vec<TreeNode>,
    pub node_type: TreeNodeType,
}

#[derive(Debug, Clone)]
pub enum TreeNodeType {
    Directory,
    File,
    Meta,
    Project,
}

impl TreeOutput {
    pub fn new(root_label: String, node_type: TreeNodeType) -> Self {
        Self {
            root: TreeNode {
                label: root_label,
                children: Vec::new(),
                node_type,
            },
        }
    }

    pub fn root_mut(&mut self) -> &mut TreeNode {
        &mut self.root
    }
}

impl TreeNode {
    pub fn add_child(&mut self, label: String, node_type: TreeNodeType) -> &mut TreeNode {
        self.children.push(TreeNode {
            label,
            children: Vec::new(),
            node_type,
        });
        self.children.last_mut().unwrap()
    }
}

impl OutputFormatter for TreeOutput {
    fn format(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Human => self.format_human(),
            OutputFormat::Ai => self.format_ai(),
            OutputFormat::Json => self.format_json(),
        }
    }
}

impl TreeOutput {
    fn format_human(&self) -> String {
        let mut output = String::new();
        self.format_node_human(&self.root, &mut output, "", true);
        output
    }
    
    fn format_node_human(&self, node: &TreeNode, output: &mut String, prefix: &str, is_last: bool) {
        let node_symbol = match node.node_type {
            TreeNodeType::Directory => "📁",
            TreeNodeType::File => "📄",
            TreeNodeType::Meta => "📦",
            TreeNodeType::Project => "🔧",
        };
        
        if prefix.is_empty() {
            output.push_str(&format!("{} {}\n", node_symbol, node.label.bold()));
        } else {
            let connector = if is_last { "└── " } else { "├── " };
            output.push_str(&format!("{}{}{} {}\n", prefix, connector, node_symbol, node.label));
        }
        
        let child_prefix = if prefix.is_empty() {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };
        
        for (i, child) in node.children.iter().enumerate() {
            let is_last_child = i == node.children.len() - 1;
            self.format_node_human(child, output, &child_prefix, is_last_child);
        }
    }
    
    fn format_ai(&self) -> String {
        let mut output = String::new();
        output.push_str("```\n");
        self.format_node_ai(&self.root, &mut output, 0);
        output.push_str("```\n");
        output
    }
    
    fn format_node_ai(&self, node: &TreeNode, output: &mut String, depth: usize) {
        let indent = "  ".repeat(depth);
        let type_marker = match node.node_type {
            TreeNodeType::Directory => "[DIR]",
            TreeNodeType::File => "[FILE]",
            TreeNodeType::Meta => "[META]",
            TreeNodeType::Project => "[PROJECT]",
        };
        
        output.push_str(&format!("{}{} {}\n", indent, type_marker, node.label));
        
        for child in &node.children {
            self.format_node_ai(child, output, depth + 1);
        }
    }
    
    fn format_json(&self) -> String {
        let json_node = self.node_to_json(&self.root);
        serde_json::to_string_pretty(&json_node).unwrap_or_default()
    }
    
    fn node_to_json(&self, node: &TreeNode) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("label".to_string(), serde_json::Value::String(node.label.clone()));
        
        let type_str = match node.node_type {
            TreeNodeType::Directory => "directory",
            TreeNodeType::File => "file",
            TreeNodeType::Meta => "meta",
            TreeNodeType::Project => "project",
        };
        obj.insert("type".to_string(), serde_json::Value::String(type_str.to_string()));
        
        if !node.children.is_empty() {
            let children: Vec<serde_json::Value> = node.children.iter()
                .map(|child| self.node_to_json(child))
                .collect();
            obj.insert("children".to_string(), serde_json::Value::Array(children));
        }
        
        serde_json::Value::Object(obj)
    }
}

pub fn format_success(message: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => format!("{} {}", "✓".green(), message),
        OutputFormat::Ai => format!("✓ **Success**: {}", message),
        OutputFormat::Json => {
            serde_json::json!({
                "status": "success",
                "message": message
            }).to_string()
        }
    }
}

pub fn format_error(message: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => format!("{} {}", "✗".red(), message.red()),
        OutputFormat::Ai => format!("✗ **Error**: {}", message),
        OutputFormat::Json => {
            serde_json::json!({
                "status": "error",
                "message": message
            }).to_string()
        }
    }
}

pub fn format_warning(message: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => format!("{} {}", "⚠".yellow(), message.yellow()),
        OutputFormat::Ai => format!("⚠ **Warning**: {}", message),
        OutputFormat::Json => {
            serde_json::json!({
                "status": "warning",
                "message": message
            }).to_string()
        }
    }
}

pub fn format_info(message: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => format!("{} {}", "ℹ".blue(), message),
        OutputFormat::Ai => format!("ℹ **Info**: {}", message),
        OutputFormat::Json => {
            serde_json::json!({
                "status": "info",
                "message": message
            }).to_string()
        }
    }
}

pub fn format_header(title: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => {
            let line = "═".repeat(title.len() + 4);
            format!("{}\n  {}  \n{}", line.blue(), title.cyan().bold(), line.blue())
        },
        OutputFormat::Ai => format!("# {}\n", title),
        OutputFormat::Json => String::new(),
    }
}

pub fn format_section(title: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => format!("\n{}\n{}", title.yellow().bold(), "─".repeat(title.len()).blue()),
        OutputFormat::Ai => format!("\n## {}\n", title),
        OutputFormat::Json => String::new(),
    }
}