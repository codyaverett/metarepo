use crate::{PluginRegistry, create_runtime_config};
use anyhow::Result;
use clap::{Arg, Command};

pub struct GestaltCli {
    registry: PluginRegistry,
}

impl GestaltCli {
    pub fn new() -> Self {
        let mut registry = PluginRegistry::new();
        registry.register_all_workspace_plugins();
        
        Self { registry }
    }
    
    pub fn build_app(&self) -> Command {
        let base_app = Command::new("gest")
            .version(env!("CARGO_PKG_VERSION"))
            .about("A tool for managing multi-project systems and libraries")
            .author("Gestalt Contributors")
            .arg(
                Arg::new("verbose")
                    .long("verbose")
                    .short('v')
                    .action(clap::ArgAction::SetTrue)
                    .help("Enable verbose output")
                    .global(true)
            )
            .arg(
                Arg::new("quiet")
                    .long("quiet")
                    .short('q')
                    .action(clap::ArgAction::SetTrue)
                    .help("Suppress output")
                    .global(true)
                    .conflicts_with("verbose")
            );
            
        self.registry.build_cli(base_app)
    }
    
    pub fn run(&self, args: Vec<String>) -> Result<()> {
        // Initialize tracing
        self.init_logging();
        
        // Parse command line arguments
        let app = self.build_app();
        let matches = app.try_get_matches_from(args)?;
        
        // Load runtime configuration
        let config = create_runtime_config()?;
        
        // Handle global flags
        if matches.get_flag("verbose") {
            tracing::debug!("Verbose mode enabled");
        }
        
        // Route to appropriate plugin
        match matches.subcommand() {
            Some((command_name, sub_matches)) => {
                self.registry.handle_command(command_name, sub_matches, &config)
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
    
    fn init_logging(&self) {
        use tracing_subscriber::{fmt, EnvFilter};
        
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("gest=info"));
            
        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .without_time()
            .init();
    }
}

impl Default for GestaltCli {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cli_creation() {
        let cli = GestaltCli::new();
        let app = cli.build_app();
        
        // Verify basic app structure
        assert_eq!(app.get_name(), "gest");
        assert!(app.get_version().is_some());
    }
    
    #[test]
    fn test_help_command() {
        let cli = GestaltCli::new();
        let result = cli.run(vec!["gest".to_string(), "--help".to_string()]);
        
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