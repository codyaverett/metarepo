use crate::{
    MetaPlugin, RuntimeConfig, OutputFormat, OutputFormatter,
    TableOutput, ListOutput, TreeOutput, 
    format_success, format_error, format_warning, format_info,
    format_header, format_section
};
use anyhow::Result;
use clap::{ArgMatches, Command, Arg};
use serde::Serialize;
use serde_json;
use std::collections::HashSet;

/// Trait for plugins that support structured output formatting
/// This trait enforces consistent output handling across all plugins
pub trait FormattedPlugin: MetaPlugin {
    /// Returns the list of command names that support output formatting
    fn formatted_commands(&self) -> Vec<&str>;
    
    /// Handle a command with structured output context
    fn handle_formatted_command(
        &self,
        command: &str,
        matches: &ArgMatches,
        config: &RuntimeConfig,
        output: &mut dyn OutputContext,
    ) -> Result<()>;
    
    /// Default implementation of handle_command that sets up output context
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Get the subcommand name if it exists
        let (command_name, sub_matches) = match matches.subcommand() {
            Some((name, sub)) => (name, sub),
            None => ("", matches),
        };
        
        // Check if this command supports formatting
        let formatted_cmds: HashSet<&str> = self.formatted_commands().into_iter().collect();
        
        if formatted_cmds.contains(command_name) || (command_name.is_empty() && !formatted_cmds.is_empty()) {
            let output_format = self.get_output_format(sub_matches);
            let mut context = OutputContextImpl::new(output_format);
            self.handle_formatted_command(command_name, sub_matches, config, &mut context)?;
            context.flush();
        } else {
            // Fall back to non-formatted handling for commands that don't support it
            self.handle_unformatted_command(command_name, sub_matches, config)?;
        }
        
        Ok(())
    }
    
    /// Override to handle commands that don't support output formatting
    fn handle_unformatted_command(
        &self,
        _command: &str,
        _matches: &ArgMatches,
        _config: &RuntimeConfig,
    ) -> Result<()> {
        Ok(())
    }
    
    /// Automatically returns true since FormattedPlugin always supports output formats
    fn supports_output_format(&self) -> bool {
        true
    }
    
    /// Enhanced command registration that automatically adds output-format arg
    fn register_formatted_commands(&self, app: Command) -> Command {
        let base_app = self.register_commands(app);
        
        // This would need to be implemented more carefully to add the arg
        // only to the specific subcommands that support formatting
        base_app
    }
}

/// Status types for consistent status reporting
#[derive(Debug, Clone, Copy)]
pub enum Status {
    Success,
    Error,
    Warning,
    Info,
}

/// Context for structured output that abstracts format details
pub trait OutputContext {
    /// Get the current output format
    fn format(&self) -> OutputFormat;
    
    /// Print a header
    fn print_header(&mut self, title: &str);
    
    /// Print a section title
    fn print_section(&mut self, title: &str);
    
    /// Print a table
    fn print_table(&mut self, table: TableOutput);
    
    /// Print a list
    fn print_list(&mut self, list: ListOutput);
    
    /// Print a tree
    fn print_tree(&mut self, tree: TreeOutput);
    
    /// Print a status message
    fn print_status(&mut self, status: Status, message: &str);
    
    /// Print raw text (format-aware)
    fn print_text(&mut self, text: &str);
    
    /// Print a JSON value (only works in JSON mode)
    fn print_json(&mut self, value: serde_json::Value);
    
    /// Collect structured data for JSON output
    fn add_data(&mut self, key: &str, value: serde_json::Value);
}

/// Concrete implementation of OutputContext
pub struct OutputContextImpl {
    format: OutputFormat,
    json_data: serde_json::Map<String, serde_json::Value>,
    buffer: String,
}

impl OutputContextImpl {
    pub fn new(format: OutputFormat) -> Self {
        Self {
            format,
            json_data: serde_json::Map::new(),
            buffer: String::new(),
        }
    }
    
    /// Flush any buffered output
    pub fn flush(&mut self) {
        match self.format {
            OutputFormat::Json => {
                if !self.json_data.is_empty() {
                    let json = serde_json::Value::Object(self.json_data.clone());
                    println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
                }
            }
            _ => {
                if !self.buffer.is_empty() {
                    print!("{}", self.buffer);
                    self.buffer.clear();
                }
            }
        }
    }
}

impl OutputContext for OutputContextImpl {
    fn format(&self) -> OutputFormat {
        self.format
    }
    
    fn print_header(&mut self, title: &str) {
        let output = format_header(title, self.format);
        match self.format {
            OutputFormat::Json => {
                self.json_data.insert("title".to_string(), serde_json::Value::String(title.to_string()));
            }
            _ => println!("{}", output),
        }
    }
    
    fn print_section(&mut self, title: &str) {
        let output = format_section(title, self.format);
        match self.format {
            OutputFormat::Json => {
                // Sections in JSON are handled differently
            }
            _ => println!("{}", output),
        }
    }
    
    fn print_table(&mut self, table: TableOutput) {
        let output = table.format(self.format);
        match self.format {
            OutputFormat::Json => {
                if let Ok(value) = serde_json::from_str(&output) {
                    self.json_data.insert("table".to_string(), value);
                }
            }
            _ => println!("{}", output),
        }
    }
    
    fn print_list(&mut self, list: ListOutput) {
        let output = list.format(self.format);
        match self.format {
            OutputFormat::Json => {
                if let Ok(value) = serde_json::from_str(&output) {
                    self.json_data.insert("list".to_string(), value);
                }
            }
            _ => println!("{}", output),
        }
    }
    
    fn print_tree(&mut self, tree: TreeOutput) {
        let output = tree.format(self.format);
        match self.format {
            OutputFormat::Json => {
                if let Ok(value) = serde_json::from_str(&output) {
                    self.json_data.insert("tree".to_string(), value);
                }
            }
            _ => println!("{}", output),
        }
    }
    
    fn print_status(&mut self, status: Status, message: &str) {
        let output = match status {
            Status::Success => format_success(message, self.format),
            Status::Error => format_error(message, self.format),
            Status::Warning => format_warning(message, self.format),
            Status::Info => format_info(message, self.format),
        };
        
        match self.format {
            OutputFormat::Json => {
                let status_str = match status {
                    Status::Success => "success",
                    Status::Error => "error",
                    Status::Warning => "warning",
                    Status::Info => "info",
                };
                self.json_data.insert("status".to_string(), serde_json::Value::String(status_str.to_string()));
                self.json_data.insert("message".to_string(), serde_json::Value::String(message.to_string()));
            }
            _ => println!("{}", output),
        }
    }
    
    fn print_text(&mut self, text: &str) {
        match self.format {
            OutputFormat::Json => {
                // In JSON mode, collect text in a messages array
                let messages = self.json_data.entry("messages".to_string())
                    .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                
                if let serde_json::Value::Array(arr) = messages {
                    arr.push(serde_json::Value::String(text.to_string()));
                }
            }
            _ => println!("{}", text),
        }
    }
    
    fn print_json(&mut self, value: serde_json::Value) {
        match self.format {
            OutputFormat::Json => {
                // Merge with existing data
                if let serde_json::Value::Object(map) = value {
                    for (k, v) in map {
                        self.json_data.insert(k, v);
                    }
                }
            }
            _ => {
                // In non-JSON modes, pretty print the JSON as text
                if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                    println!("{}", pretty);
                }
            }
        }
    }
    
    fn add_data(&mut self, key: &str, value: serde_json::Value) {
        if self.format == OutputFormat::Json {
            self.json_data.insert(key.to_string(), value);
        }
    }
}

/// Builder for type-safe output construction
pub struct OutputBuilder<T: Serialize> {
    data: T,
    header: Option<String>,
    sections: Vec<(String, String)>,
}

impl<T: Serialize> OutputBuilder<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            header: None,
            sections: Vec::new(),
        }
    }
    
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }
    
    pub fn with_section(mut self, title: impl Into<String>, content: impl Into<String>) -> Self {
        self.sections.push((title.into(), content.into()));
        self
    }
    
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(&self.data).unwrap_or_default()
            }
            OutputFormat::Ai => {
                let mut output = String::new();
                
                if let Some(header) = &self.header {
                    output.push_str(&format!("# {}\n\n", header));
                }
                
                for (title, content) in &self.sections {
                    output.push_str(&format!("## {}\n\n{}\n\n", title, content));
                }
                
                // Add structured data as code block
                if let Ok(json) = serde_json::to_string_pretty(&self.data) {
                    output.push_str("```json\n");
                    output.push_str(&json);
                    output.push_str("\n```\n");
                }
                
                output
            }
            OutputFormat::Human => {
                let mut output = String::new();
                
                if let Some(header) = &self.header {
                    output.push_str(&format_header(header, format));
                    output.push('\n');
                }
                
                for (title, content) in &self.sections {
                    output.push_str(&format_section(title, format));
                    output.push('\n');
                    output.push_str(content);
                    output.push('\n');
                }
                
                output
            }
        }
    }
    
    pub fn print(&self, context: &mut dyn OutputContext) {
        let output = self.render(context.format());
        context.print_text(&output);
    }
}

/// Helper to automatically add output-format arg to specific commands
pub fn add_output_format_to_commands(app: Command, _commands: &[&str]) -> Command {
    let _output_arg = Arg::new("output-format")
        .long("output-format")
        .value_name("FORMAT")
        .help("Output format (human, ai, json)")
        .default_value("human")
        .value_parser(["human", "ai", "json"]);
    
    // This is a simplified version - in practice we'd need to recursively
    // traverse and modify only the specified subcommands
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_context(format: OutputFormat) -> OutputContextImpl {
        OutputContextImpl::new(format)
    }
    
    #[test]
    fn test_output_context_formats() {
        let formats = vec![OutputFormat::Human, OutputFormat::Ai, OutputFormat::Json];
        
        for format in formats {
            let mut ctx = create_test_context(format);
            assert_eq!(ctx.format(), format);
            
            ctx.print_header("Test Header");
            ctx.print_status(Status::Success, "Test successful");
            ctx.flush();
        }
    }
    
    #[test]
    fn test_output_builder() {
        #[derive(Serialize)]
        struct TestData {
            name: String,
            value: i32,
        }
        
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        
        let builder = OutputBuilder::new(data)
            .with_header("Test Output")
            .with_section("Details", "Some details here");
        
        // Test each format
        let _ = builder.render(OutputFormat::Human);
        let _ = builder.render(OutputFormat::Ai);
        let json_output = builder.render(OutputFormat::Json);
        
        assert!(json_output.contains("\"name\": \"test\""));
        assert!(json_output.contains("\"value\": 42"));
    }
}