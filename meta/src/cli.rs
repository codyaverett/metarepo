use crate::{PluginRegistry, create_runtime_config};
use anyhow::Result;
use clap::{Arg, Command, ColorChoice};
use std::cell::RefCell;

pub struct MetarepoCli {
    registry: RefCell<PluginRegistry>,
}

impl MetarepoCli {
    pub fn new() -> Self {
        Self::new_with_flags(false)
    }
    
    pub fn new_with_flags(experimental: bool) -> Self {
        let mut registry = PluginRegistry::new();
        registry.register_all_workspace_plugins_with_flags(experimental);
        
        // Don't load external plugins here - do it lazily when needed
        // This prevents plugin output during --version or --help
        
        Self { registry: RefCell::new(registry) }
    }
    
    pub fn build_app(&self) -> Command {
        self.build_app_with_flags(false)
    }
    
    pub fn build_app_with_flags(&self, experimental: bool) -> Command {
        // Set up color styles for help output
        let styles = clap::builder::styling::Styles::styled()
            .header(clap::builder::styling::AnsiColor::BrightCyan.on_default() | clap::builder::styling::Effects::BOLD)
            .usage(clap::builder::styling::AnsiColor::BrightGreen.on_default() | clap::builder::styling::Effects::BOLD)
            .literal(clap::builder::styling::AnsiColor::BrightWhite.on_default())
            .placeholder(clap::builder::styling::AnsiColor::BrightYellow.on_default())
            .error(clap::builder::styling::AnsiColor::BrightRed.on_default() | clap::builder::styling::Effects::BOLD)
            .valid(clap::builder::styling::AnsiColor::BrightGreen.on_default())
            .invalid(clap::builder::styling::AnsiColor::BrightRed.on_default());
            
        let mut app = Command::new("meta")
            .version(env!("CARGO_PKG_VERSION"))
            .about("A tool for managing multi-project systems and libraries")
            .author("Metarepo Contributors")
            .styles(styles)
            .color(ColorChoice::Always)
            .disable_help_subcommand(true)
            .subcommand_precedence_over_arg(true)
            .disable_version_flag(true);
            
        // First add all subcommands from plugins
        app = self.registry.borrow().build_cli_with_flags(app, experimental);
        
        // Then add global options after subcommands
        // Only version and experimental are truly global
        app = app
            .arg(
                Arg::new("version")
                    .long("version")
                    .short('v')
                    .action(clap::ArgAction::Version)
                    .help("Print version information")
                    .global(true)
            )
            .arg(
                Arg::new("experimental")
                    .long("experimental")
                    .action(clap::ArgAction::SetTrue)
                    .help("Enable experimental features")
                    .global(true)
            );
            
        app
    }
    
    pub fn run(&self, args: Vec<String>) -> Result<()> {
        // Initialize tracing
        self.init_logging();
        
        // Check if --experimental is present in args
        let experimental = args.iter().any(|arg| arg == "--experimental");
        
        // If experimental, create a new CLI with experimental plugins
        if experimental {
            let cli = Self::new_with_flags(true);
            return cli.run_with_experimental(args);
        }
        
        // Normal execution without experimental features
        let app = self.build_app();
        let matches = app.try_get_matches_from(args)?;
        
        // Load runtime configuration
        let config = create_runtime_config(false)?;
        
        // Route to appropriate plugin
        match matches.subcommand() {
            Some((command_name, sub_matches)) => {
                // Load external plugins only when we have an actual command to run
                // This prevents plugin output during --version or --help
                if let Ok(meta_config) = metarepo_core::MetaConfig::load() {
                    self.registry.borrow_mut().load_external_plugins(&meta_config);
                }
                
                self.registry.borrow().handle_command(command_name, sub_matches, &config)
            }
            None => {
                // No subcommand provided, show help
                let mut app = self.build_app();
                app.print_help()?;
                println!();
                Ok(())
            }
        }
    }
    
    fn run_with_experimental(&self, args: Vec<String>) -> Result<()> {
        // Parse with experimental plugins available
        let app = self.build_app_with_flags(true);
        let matches = app.try_get_matches_from(args)?;
        
        // Load runtime configuration with experimental flag
        let config = create_runtime_config(true)?;
        
        tracing::debug!("Experimental features enabled");
        
        // Route to appropriate plugin
        match matches.subcommand() {
            Some((command_name, sub_matches)) => {
                // Load external plugins only when we have an actual command to run
                if let Ok(meta_config) = metarepo_core::MetaConfig::load() {
                    self.registry.borrow_mut().load_external_plugins(&meta_config);
                }
                
                self.registry.borrow().handle_command(command_name, sub_matches, &config)
            }
            None => {
                // No subcommand provided, show help
                let mut app = self.build_app_with_flags(true);
                app.print_help()?;
                println!();
                Ok(())
            }
        }
    }
    
    fn init_logging(&self) {
        use tracing_subscriber::{fmt, EnvFilter};
        
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("meta=info"));
            
        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .without_time()
            .init();
    }
}

impl Default for MetarepoCli {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cli_creation() {
        let cli = MetarepoCli::new();
        let app = cli.build_app();
        
        // Verify basic app structure
        assert_eq!(app.get_name(), "meta");
        assert!(app.get_version().is_some());
    }
    
    #[test]
    fn test_help_command() {
        let cli = MetarepoCli::new();
        let result = cli.run(vec!["meta".to_string(), "--help".to_string()]);
        
        // Help should succeed but not return an error
        match result {
            Ok(_) => {},
            Err(e) => {
                // clap help exits with a special error type that's actually success
                if let Some(clap_err) = e.downcast_ref::<clap::Error>() {
                    assert_eq!(clap_err.kind(), clap::error::ErrorKind::DisplayHelp);
                } else {
                    panic!("Unexpected error type: {}", e);
                }
            }
        }
    }
}