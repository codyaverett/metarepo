use anyhow::Result;
use clap::Command;
use serde::{Serialize, Deserialize};
use std::fmt;

use crate::MetaPlugin;

/// Base trait for plugins with default implementations
pub trait BasePlugin: MetaPlugin {
    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().unwrap_or("0.1.0").to_string(),
            description: self.description().unwrap_or("").to_string(),
            author: self.author().unwrap_or("").to_string(),
            experimental: self.is_experimental(),
        }
    }
    
    /// Get plugin version
    fn version(&self) -> Option<&str> {
        None
    }
    
    /// Get plugin description
    fn description(&self) -> Option<&str> {
        None
    }
    
    /// Get plugin author
    fn author(&self) -> Option<&str> {
        None
    }
    
    /// Default help implementation that generates from command structure
    fn show_help(&self, format: HelpFormat) -> Result<()> {
        let app = self.build_help_command();
        let formatter = format.formatter();
        formatter.format_help(&app)
    }
    
    /// Build a command for help generation
    fn build_help_command(&self) -> Command {
        let name: &'static str = Box::leak(format!("meta {}", self.name()).into_boxed_str());
        let app = Command::new(name);
        self.register_commands(app)
    }
    
    /// Show AI-friendly help output
    fn show_ai_help(&self) -> Result<()> {
        self.show_help(HelpFormat::Json)
    }
}

/// Plugin metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub experimental: bool,
}

/// Help output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpFormat {
    Terminal,
    Json,
    Yaml,
    Markdown,
}

impl HelpFormat {
    pub fn formatter(&self) -> Box<dyn HelpFormatter> {
        match self {
            HelpFormat::Terminal => Box::new(TerminalHelpFormatter),
            HelpFormat::Json => Box::new(JsonHelpFormatter),
            HelpFormat::Yaml => Box::new(YamlHelpFormatter),
            HelpFormat::Markdown => Box::new(MarkdownHelpFormatter),
        }
    }
    
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "terminal" | "term" => Some(HelpFormat::Terminal),
            "json" => Some(HelpFormat::Json),
            "yaml" | "yml" => Some(HelpFormat::Yaml),
            "markdown" | "md" => Some(HelpFormat::Markdown),
            _ => None,
        }
    }
}

impl fmt::Display for HelpFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelpFormat::Terminal => write!(f, "terminal"),
            HelpFormat::Json => write!(f, "json"),
            HelpFormat::Yaml => write!(f, "yaml"),
            HelpFormat::Markdown => write!(f, "markdown"),
        }
    }
}

/// Trait for formatting help output
pub trait HelpFormatter {
    fn format_help(&self, app: &Command) -> Result<()>;
}

/// Terminal help formatter (default colorful output)
pub struct TerminalHelpFormatter;

impl HelpFormatter for TerminalHelpFormatter {
    fn format_help(&self, app: &Command) -> Result<()> {
        let mut app = app.clone();
        app.print_help()?;
        println!();
        Ok(())
    }
}

/// JSON help formatter for structured output
pub struct JsonHelpFormatter;

impl HelpFormatter for JsonHelpFormatter {
    fn format_help(&self, app: &Command) -> Result<()> {
        let help_data = extract_command_info(app);
        let json = serde_json::to_string_pretty(&help_data)?;
        println!("{}", json);
        Ok(())
    }
}

/// YAML help formatter for structured output
pub struct YamlHelpFormatter;

impl HelpFormatter for YamlHelpFormatter {
    fn format_help(&self, app: &Command) -> Result<()> {
        let help_data = extract_command_info(app);
        let yaml = serde_yaml::to_string(&help_data)?;
        println!("{}", yaml);
        Ok(())
    }
}

/// Markdown help formatter for documentation
pub struct MarkdownHelpFormatter;

impl HelpFormatter for MarkdownHelpFormatter {
    fn format_help(&self, app: &Command) -> Result<()> {
        let mut output = String::new();
        
        // Command name and description
        output.push_str(&format!("# {}\n\n", app.get_name()));
        if let Some(about) = app.get_about() {
            output.push_str(&format!("{}\n\n", about));
        }
        
        // Usage
        output.push_str("## Usage\n\n```\n");
        output.push_str(&format!("{} [OPTIONS]", app.get_name()));
        if app.get_subcommands().count() > 0 {
            output.push_str(" <COMMAND>");
        }
        output.push_str("\n```\n\n");
        
        // Options
        let args: Vec<_> = app.get_arguments().collect();
        if !args.is_empty() {
            output.push_str("## Options\n\n");
            for arg in args {
                if let Some(help) = arg.get_help() {
                    let short = arg.get_short().map(|s| format!("-{}", s)).unwrap_or_default();
                    let long = arg.get_long().map(|l| format!("--{}", l)).unwrap_or_default();
                    let flags = match (&short[..], &long[..]) {
                        ("", l) => l.to_string(),
                        (s, "") => s.to_string(),
                        (s, l) => format!("{}, {}", s, l),
                    };
                    output.push_str(&format!("- `{}`: {}\n", flags, help));
                }
            }
            output.push('\n');
        }
        
        // Subcommands
        let subcommands: Vec<_> = app.get_subcommands().collect();
        if !subcommands.is_empty() {
            output.push_str("## Commands\n\n");
            for subcmd in subcommands {
                output.push_str(&format!("### {}\n\n", subcmd.get_name()));
                if let Some(about) = subcmd.get_about() {
                    output.push_str(&format!("{}\n\n", about));
                }
            }
        }
        
        println!("{}", output);
        Ok(())
    }
}

/// Extract command information for structured output
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub subcommands: Vec<CommandInfo>,
    pub arguments: Vec<ArgumentInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgumentInfo {
    pub name: String,
    pub short: Option<char>,
    pub long: Option<String>,
    pub help: Option<String>,
    pub required: bool,
    pub takes_value: bool,
}

fn extract_command_info(app: &Command) -> CommandInfo {
    CommandInfo {
        name: app.get_name().to_string(),
        description: app.get_about().map(|s| s.to_string()),
        version: app.get_version().map(|s| s.to_string()),
        subcommands: app.get_subcommands()
            .map(|cmd| extract_command_info(cmd))
            .collect(),
        arguments: app.get_arguments()
            .map(|arg| ArgumentInfo {
                name: arg.get_id().to_string(),
                short: arg.get_short(),
                long: arg.get_long().map(|s| s.to_string()),
                help: arg.get_help().map(|s| s.to_string()),
                required: arg.is_required_set(),
                takes_value: arg.get_num_args().map(|n| n.takes_values()).unwrap_or(false),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_help_format_from_str() {
        assert_eq!(HelpFormat::parse("terminal"), Some(HelpFormat::Terminal));
        assert_eq!(HelpFormat::parse("term"), Some(HelpFormat::Terminal));
        assert_eq!(HelpFormat::parse("json"), Some(HelpFormat::Json));
        assert_eq!(HelpFormat::parse("yaml"), Some(HelpFormat::Yaml));
        assert_eq!(HelpFormat::parse("yml"), Some(HelpFormat::Yaml));
        assert_eq!(HelpFormat::parse("markdown"), Some(HelpFormat::Markdown));
        assert_eq!(HelpFormat::parse("md"), Some(HelpFormat::Markdown));
        assert_eq!(HelpFormat::parse("unknown"), None);
        
        // Test case insensitive
        assert_eq!(HelpFormat::parse("JSON"), Some(HelpFormat::Json));
        assert_eq!(HelpFormat::parse("Terminal"), Some(HelpFormat::Terminal));
    }
    
    #[test]
    fn test_help_format_display() {
        assert_eq!(format!("{}", HelpFormat::Terminal), "terminal");
        assert_eq!(format!("{}", HelpFormat::Json), "json");
        assert_eq!(format!("{}", HelpFormat::Yaml), "yaml");
        assert_eq!(format!("{}", HelpFormat::Markdown), "markdown");
    }
    
    #[test]
    fn test_extract_command_info() {
        let app = Command::new("test-app")
            .version("1.0.0")
            .about("Test application")
            .arg(
                clap::Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("Enable verbose output")
            )
            .arg(
                clap::Arg::new("input")
                    .long("input")
                    .help("Input file")
                    .required(true)
                    .value_name("FILE")
            )
            .subcommand(
                Command::new("sub")
                    .about("Subcommand")
                    .arg(
                        clap::Arg::new("flag")
                            .short('f')
                            .help("A flag")
                    )
            );
        
        let info = extract_command_info(&app);
        
        assert_eq!(info.name, "test-app");
        assert_eq!(info.description, Some("Test application".to_string()));
        assert_eq!(info.version, Some("1.0.0".to_string()));
        assert_eq!(info.subcommands.len(), 1);
        assert_eq!(info.subcommands[0].name, "sub");
        
        // Check arguments (note: clap includes help and version by default)
        let verbose_arg = info.arguments.iter().find(|a| a.name == "verbose");
        assert!(verbose_arg.is_some());
        let verbose = verbose_arg.unwrap();
        assert_eq!(verbose.short, Some('v'));
        assert_eq!(verbose.long, Some("verbose".to_string()));
        assert_eq!(verbose.help, Some("Enable verbose output".to_string()));
        
        let input_arg = info.arguments.iter().find(|a| a.name == "input");
        assert!(input_arg.is_some());
        let input = input_arg.unwrap();
        assert_eq!(input.long, Some("input".to_string()));
        assert!(input.required);
    }
    
    #[test]
    fn test_plugin_metadata() {
        #[derive(Debug)]
        struct TestPlugin;
        
        impl MetaPlugin for TestPlugin {
            fn name(&self) -> &str {
                "test"
            }
            
            fn register_commands(&self, app: Command) -> Command {
                app
            }
            
            fn handle_command(&self, _matches: &clap::ArgMatches, _config: &crate::RuntimeConfig) -> Result<()> {
                Ok(())
            }
            
            fn is_experimental(&self) -> bool {
                true
            }
        }
        
        impl BasePlugin for TestPlugin {
            fn version(&self) -> Option<&str> {
                Some("1.2.3")
            }
            
            fn description(&self) -> Option<&str> {
                Some("Test plugin")
            }
            
            fn author(&self) -> Option<&str> {
                Some("Test Author")
            }
        }
        
        let plugin = TestPlugin;
        let metadata = plugin.metadata();
        
        assert_eq!(metadata.name, "test");
        assert_eq!(metadata.version, "1.2.3");
        assert_eq!(metadata.description, "Test plugin");
        assert_eq!(metadata.author, "Test Author");
        assert!(metadata.experimental);
    }
    
    // Note: Testing formatter types is not directly possible due to trait object limitations
    // The test above for HelpFormat::from_str and display is sufficient
}