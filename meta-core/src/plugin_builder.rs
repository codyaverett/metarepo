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
            .about(about);
        
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