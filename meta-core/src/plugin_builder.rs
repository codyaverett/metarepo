use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use std::collections::HashMap;

use crate::{MetaPlugin, RuntimeConfig, BasePlugin};

/// Builder for creating plugins declaratively
pub struct PluginBuilder {
    name: String,
    version: String,
    description: String,
    author: String,
    experimental: bool,
    commands: Vec<CommandBuilder>,
    handlers: HashMap<String, Box<dyn Fn(&ArgMatches, &RuntimeConfig) -> Result<()> + Send + Sync>>,
}

impl PluginBuilder {
    /// Create a new plugin builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".to_string(),
            description: String::new(),
            author: String::new(),
            experimental: false,
            commands: Vec::new(),
            handlers: HashMap::new(),
        }
    }
    
    /// Set plugin version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }
    
    /// Set plugin description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
    
    /// Set plugin author
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }
    
    /// Mark plugin as experimental
    pub fn experimental(mut self, experimental: bool) -> Self {
        self.experimental = experimental;
        self
    }
    
    /// Add a command to the plugin
    pub fn command(mut self, builder: CommandBuilder) -> Self {
        self.commands.push(builder);
        self
    }
    
    /// Add a handler for a specific command
    pub fn handler<F>(mut self, command: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&ArgMatches, &RuntimeConfig) -> Result<()> + Send + Sync + 'static,
    {
        self.handlers.insert(command.into(), Box::new(handler));
        self
    }
    
    /// Build the plugin
    pub fn build(self) -> BuiltPlugin {
        BuiltPlugin {
            name: self.name,
            version: self.version,
            description: self.description,
            author: self.author,
            experimental: self.experimental,
            commands: self.commands,
            handlers: self.handlers,
        }
    }
}

/// A plugin built from the builder
pub struct BuiltPlugin {
    name: String,
    version: String,
    description: String,
    author: String,
    experimental: bool,
    commands: Vec<CommandBuilder>,
    handlers: HashMap<String, Box<dyn Fn(&ArgMatches, &RuntimeConfig) -> Result<()> + Send + Sync>>,
}

impl MetaPlugin for BuiltPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn register_commands(&self, app: Command) -> Command {
        if self.commands.is_empty() {
            return app;
        }
        
        let name: &'static str = Box::leak(self.name.clone().into_boxed_str());
        let desc: &'static str = Box::leak(self.description.clone().into_boxed_str());
        let vers: &'static str = Box::leak(self.version.clone().into_boxed_str());
        
        let mut plugin_cmd = Command::new(name)
            .about(desc)
            .version(vers);
        
        for cmd_builder in &self.commands {
            plugin_cmd = plugin_cmd.subcommand(cmd_builder.build());
        }
        
        app.subcommand(plugin_cmd)
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Find which subcommand was called
        if let Some((cmd_name, sub_matches)) = matches.subcommand() {
            // Look for a handler
            if let Some(handler) = self.handlers.get(cmd_name) {
                return handler(sub_matches, config);
            }
            
            // If no handler, show help
            println!("No handler registered for command: {}", cmd_name);
        }
        
        // Show help if no subcommand
        let mut help_cmd = self.build_help_command();
        help_cmd.print_help()?;
        Ok(())
    }
    
    fn is_experimental(&self) -> bool {
        self.experimental
    }
}

impl BuiltPlugin {
    fn build_help_command(&self) -> Command {
        let name: &'static str = Box::leak(self.name.clone().into_boxed_str());
        let desc: &'static str = Box::leak(self.description.clone().into_boxed_str());
        let vers: &'static str = Box::leak(self.version.clone().into_boxed_str());
        
        let mut plugin_cmd = Command::new(name)
            .about(desc)
            .version(vers);
        
        for cmd_builder in &self.commands {
            plugin_cmd = plugin_cmd.subcommand(cmd_builder.build());
        }
        
        plugin_cmd
    }
}

impl BasePlugin for BuiltPlugin {
    fn version(&self) -> Option<&str> {
        Some(&self.version)
    }
    
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    
    fn author(&self) -> Option<&str> {
        Some(&self.author)
    }
}

/// Builder for individual commands
pub struct CommandBuilder {
    name: String,
    about: String,
    long_about: Option<String>,
    aliases: Vec<String>,
    args: Vec<ArgBuilder>,
    subcommands: Vec<CommandBuilder>,
    allow_external_subcommands: bool,
}

impl CommandBuilder {
    /// Create a new command builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            about: String::new(),
            long_about: None,
            aliases: Vec::new(),
            args: Vec::new(),
            subcommands: Vec::new(),
            allow_external_subcommands: false,
        }
    }
    
    /// Set command description
    pub fn about(mut self, about: impl Into<String>) -> Self {
        self.about = about.into();
        self
    }
    
    /// Set long command description
    pub fn long_about(mut self, long_about: impl Into<String>) -> Self {
        self.long_about = Some(long_about.into());
        self
    }
    
    /// Add command alias
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }
    
    /// Add command aliases
    pub fn aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases.extend(aliases);
        self
    }
    
    /// Add an argument
    pub fn arg(mut self, arg: ArgBuilder) -> Self {
        self.args.push(arg);
        self
    }
    
    /// Add a subcommand
    pub fn subcommand(mut self, cmd: CommandBuilder) -> Self {
        self.subcommands.push(cmd);
        self
    }
    
    /// Allow external subcommands (for commands like exec that need to pass through arbitrary commands)
    pub fn allow_external_subcommands(mut self, allow: bool) -> Self {
        self.allow_external_subcommands = allow;
        self
    }
    
    /// Build the clap Command
    fn build(&self) -> Command {
        let name: &'static str = Box::leak(self.name.clone().into_boxed_str());
        let about: &'static str = Box::leak(self.about.clone().into_boxed_str());
        
        let mut cmd = Command::new(name)
            .about(about)
            .version(env!("CARGO_PKG_VERSION"));
        
        if let Some(ref long_about) = self.long_about {
            let long_about_str: &'static str = Box::leak(long_about.clone().into_boxed_str());
            cmd = cmd.long_about(long_about_str);
        }
        
        if !self.aliases.is_empty() {
            let aliases: Vec<&'static str> = self.aliases.iter()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
                .collect();
            cmd = cmd.visible_aliases(aliases);
        }
        
        for arg_builder in &self.args {
            cmd = cmd.arg(arg_builder.build());
        }
        
        for subcmd_builder in &self.subcommands {
            cmd = cmd.subcommand(subcmd_builder.build());
        }
        
        if self.allow_external_subcommands {
            cmd = cmd.allow_external_subcommands(true);
        }
        
        cmd
    }
}

/// Builder for arguments
pub struct ArgBuilder {
    name: String,
    short: Option<char>,
    long: Option<String>,
    help: Option<String>,
    required: bool,
    takes_value: bool,
    default_value: Option<String>,
    possible_values: Vec<String>,
}

impl ArgBuilder {
    /// Create a new argument builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short: None,
            long: None,
            help: None,
            required: false,
            takes_value: false,
            default_value: None,
            possible_values: Vec::new(),
        }
    }
    
    /// Set short flag
    pub fn short(mut self, short: char) -> Self {
        self.short = Some(short);
        self
    }
    
    /// Set long flag
    pub fn long(mut self, long: impl Into<String>) -> Self {
        self.long = Some(long.into());
        self
    }
    
    /// Set help text
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
    
    /// Mark as required
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
    
    /// Set whether argument takes a value
    pub fn takes_value(mut self, takes: bool) -> Self {
        self.takes_value = takes;
        self
    }
    
    /// Set default value
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }
    
    /// Add possible value
    pub fn possible_value(mut self, value: impl Into<String>) -> Self {
        self.possible_values.push(value.into());
        self
    }
    
    /// Build the clap Arg
    fn build(&self) -> Arg {
        let name: &'static str = Box::leak(self.name.clone().into_boxed_str());
        let mut arg = Arg::new(name);
        
        if let Some(short) = self.short {
            arg = arg.short(short);
        }
        
        if let Some(ref long) = self.long {
            let long_str: &'static str = Box::leak(long.clone().into_boxed_str());
            arg = arg.long(long_str);
        }
        
        if let Some(ref help) = self.help {
            let help_str: &'static str = Box::leak(help.clone().into_boxed_str());
            arg = arg.help(help_str);
        }
        
        if self.required {
            arg = arg.required(true);
        }
        
        if self.takes_value {
            arg = arg.action(clap::ArgAction::Set);
        } else {
            arg = arg.action(clap::ArgAction::SetTrue);
        }
        
        if let Some(ref default) = self.default_value {
            let default_str: &'static str = Box::leak(default.clone().into_boxed_str());
            arg = arg.default_value(default_str);
        }
        
        if !self.possible_values.is_empty() {
            let values: Vec<&'static str> = self.possible_values.iter()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
                .collect();
            arg = arg.value_parser(values);
        }
        
        arg
    }
}

/// Convenience function to create a new plugin builder
pub fn plugin(name: impl Into<String>) -> PluginBuilder {
    PluginBuilder::new(name)
}

/// Convenience function to create a new command builder
pub fn command(name: impl Into<String>) -> CommandBuilder {
    CommandBuilder::new(name)
}

/// Convenience function to create a new argument builder
pub fn arg(name: impl Into<String>) -> ArgBuilder {
    ArgBuilder::new(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_builder_basic() {
        let plugin = plugin("test-plugin")
            .version("1.0.0")
            .description("Test plugin")
            .author("Test Author")
            .experimental(true)
            .build();
        
        assert_eq!(plugin.name(), "test-plugin");
        assert_eq!(plugin.version(), Some("1.0.0"));
        assert_eq!(plugin.description(), Some("Test plugin"));
        assert_eq!(plugin.author(), Some("Test Author"));
        assert!(plugin.is_experimental());
    }
    
    #[test]
    fn test_plugin_builder_with_commands() {
        let test_handler = |_matches: &ArgMatches, _config: &RuntimeConfig| -> Result<()> {
            Ok(())
        };
        
        let plugin = plugin("test-plugin")
            .command(
                command("test-cmd")
                    .about("Test command")
                    .arg(
                        arg("input")
                            .short('i')
                            .long("input")
                            .help("Input file")
                            .takes_value(true)
                    )
            )
            .handler("test-cmd", test_handler)
            .build();
        
        let app = Command::new("test");
        let app_with_plugin = plugin.register_commands(app);
        
        // Check that the plugin command was added
        let plugin_cmd = app_with_plugin.find_subcommand("test-plugin");
        assert!(plugin_cmd.is_some());
        
        let plugin_cmd = plugin_cmd.unwrap();
        let test_cmd = plugin_cmd.find_subcommand("test-cmd");
        assert!(test_cmd.is_some());
    }
    
    #[test]
    fn test_command_builder() {
        let cmd = command("test")
            .about("Test command")
            .long_about("This is a longer description")
            .aliases(vec!["t".to_string(), "tst".to_string()])
            .arg(
                arg("verbose")
                    .short('v')
                    .long("verbose")
                    .help("Enable verbose output")
            )
            .build();
        
        assert_eq!(cmd.get_name(), "test");
        assert_eq!(cmd.get_about().map(|s| s.to_string()), Some("Test command".to_string()));
        assert_eq!(cmd.get_long_about().map(|s| s.to_string()), Some("This is a longer description".to_string()));
        
        // Check aliases
        let aliases: Vec<&str> = cmd.get_visible_aliases().map(|a| a).collect();
        assert!(aliases.contains(&"t"));
        assert!(aliases.contains(&"tst"));
        
        // Check arguments
        let verbose_arg = cmd.get_arguments().find(|a| a.get_id() == "verbose");
        assert!(verbose_arg.is_some());
    }
    
    #[test]
    fn test_arg_builder_flag() {
        let arg = arg("verbose")
            .short('v')
            .long("verbose")
            .help("Enable verbose output")
            .build();
        
        assert_eq!(arg.get_id().to_string(), "verbose");
        assert_eq!(arg.get_short(), Some('v'));
        assert_eq!(arg.get_long(), Some("verbose"));
        assert_eq!(arg.get_help().map(|s| s.to_string()), Some("Enable verbose output".to_string()));
    }
    
    #[test]
    fn test_arg_builder_with_value() {
        let arg = arg("input")
            .long("input")
            .help("Input file")
            .required(true)
            .takes_value(true)
            .default_value("default.txt")
            .build();
        
        assert_eq!(arg.get_id().to_string(), "input");
        assert_eq!(arg.get_long(), Some("input"));
        assert!(arg.is_required_set());
        assert_eq!(arg.get_default_values(), &["default.txt"]);
    }
    
    #[test]
    fn test_arg_builder_with_possible_values() {
        let arg = arg("format")
            .long("format")
            .help("Output format")
            .takes_value(true)
            .possible_value("json")
            .possible_value("yaml")
            .possible_value("text")
            .build();
        
        assert_eq!(arg.get_id().to_string(), "format");
        // Note: Testing possible values would require checking the value parser
        // which is not directly accessible from the Arg struct
    }
    
    #[test]
    fn test_command_builder_with_subcommands() {
        let cmd = command("parent")
            .about("Parent command")
            .subcommand(
                command("child1")
                    .about("First child")
            )
            .subcommand(
                command("child2")
                    .about("Second child")
                    .arg(
                        arg("option")
                            .long("option")
                            .takes_value(true)
                    )
            )
            .build();
        
        assert_eq!(cmd.get_name(), "parent");
        
        let child1 = cmd.find_subcommand("child1");
        assert!(child1.is_some());
        assert_eq!(child1.unwrap().get_about().map(|s| s.to_string()), Some("First child".to_string()));
        
        let child2 = cmd.find_subcommand("child2");
        assert!(child2.is_some());
        let child2 = child2.unwrap();
        assert_eq!(child2.get_about().map(|s| s.to_string()), Some("Second child".to_string()));
        
        let option_arg = child2.get_arguments().find(|a| a.get_id() == "option");
        assert!(option_arg.is_some());
    }
    
    #[test]
    fn test_command_builder_external_subcommands() {
        let cmd = command("exec")
            .about("Execute commands")
            .allow_external_subcommands(true)
            .build();
        
        assert_eq!(cmd.get_name(), "exec");
        assert!(cmd.is_allow_external_subcommands_set());
    }
    
    #[test]
    fn test_plugin_handler_execution() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();
        
        let test_handler = move |_matches: &ArgMatches, _config: &RuntimeConfig| -> Result<()> {
            executed_clone.store(true, Ordering::SeqCst);
            Ok(())
        };
        
        let plugin = plugin("test-plugin")
            .command(
                command("test-cmd")
                    .about("Test command")
            )
            .handler("test-cmd", test_handler)
            .build();
        
        // Create mock matches for the plugin and its test-cmd subcommand
        // The plugin expects matches to have the subcommand structure
        let app = Command::new("test-plugin")
            .subcommand(Command::new("test-cmd"));
        let matches = app.clone().get_matches_from(vec!["test-plugin", "test-cmd"]);
        
        // Create a dummy runtime config
        let config = RuntimeConfig {
            meta_config: crate::MetaConfig::default(),
            working_dir: std::path::PathBuf::from("."),
            meta_file_path: None,
            experimental: false,
        };
        
        // Handle the command - the plugin will look for subcommands in the matches
        plugin.handle_command(&matches, &config).unwrap();
        
        // Check that the handler was executed
        assert!(executed.load(Ordering::SeqCst));
    }
}