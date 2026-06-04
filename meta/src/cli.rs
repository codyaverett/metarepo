use crate::{create_runtime_config_full, PluginRegistry};
use anyhow::Result;
use clap::{Arg, ColorChoice, Command};
use metarepo_core::NonInteractiveMode;
use std::cell::RefCell;
use std::path::PathBuf;
use std::str::FromStr;

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

        Self {
            registry: RefCell::new(registry),
        }
    }

    pub fn build_app(&self) -> Command {
        self.build_app_with_flags(false)
    }

    pub fn build_app_with_flags(&self, experimental: bool) -> Command {
        // Set up color styles for help output
        let styles = clap::builder::styling::Styles::styled()
            .header(
                clap::builder::styling::AnsiColor::BrightCyan.on_default()
                    | clap::builder::styling::Effects::BOLD,
            )
            .usage(
                clap::builder::styling::AnsiColor::BrightGreen.on_default()
                    | clap::builder::styling::Effects::BOLD,
            )
            // `literal` styles command and option names — bold bright white so
            // they stand out consistently in every help view (--help and help).
            .literal(
                clap::builder::styling::AnsiColor::BrightWhite.on_default()
                    | clap::builder::styling::Effects::BOLD,
            )
            .placeholder(clap::builder::styling::AnsiColor::BrightYellow.on_default())
            .error(
                clap::builder::styling::AnsiColor::BrightRed.on_default()
                    | clap::builder::styling::Effects::BOLD,
            )
            .valid(clap::builder::styling::AnsiColor::BrightGreen.on_default())
            .invalid(clap::builder::styling::AnsiColor::BrightRed.on_default());

        let mut app = Command::new("meta")
            .version(env!("CARGO_PKG_VERSION"))
            .about("A tool for managing multi-project systems and libraries")
            .author("Metarepo Contributors")
            .styles(styles)
            .color(ColorChoice::Always)
            // Keep clap's built-in `help` subcommand so `meta help`,
            // `meta <group> help`, and `meta help <group>` all print help like
            // `--help`. It is disabled per-command only where a command accepts
            // external subcommands (e.g. `exec`), so `meta exec help` still runs
            // a command named `help` across repos.
            .subcommand_precedence_over_arg(true)
            .disable_version_flag(true);

        // First add all subcommands from plugins
        app = self
            .registry
            .borrow()
            .build_cli_with_flags(app, experimental);

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
                    .short('x')
                    .action(clap::ArgAction::SetTrue)
                    .help("Enable experimental features")
                    .global(true)
            )
            .arg(
                Arg::new("non-interactive")
                    .long("non-interactive")
                    .value_name("MODE")
                    .value_parser(["fail", "defaults"])
                    .help("Non-interactive mode: 'fail' exits on missing input, 'defaults' uses sensible defaults")
                    .global(true)
            )
            .arg(
                Arg::new("config")
                    .long("config")
                    .short('c')
                    .value_name("PATH")
                    .help("Path to a metarepo config file (overrides auto-discovery). Format is detected from the file extension.")
                    .global(true)
            )
            .arg(
                Arg::new("allow-version-mismatch")
                    .long("allow-version-mismatch")
                    .action(clap::ArgAction::SetTrue)
                    .help("Load external plugins even if their version does not satisfy the pin in .metarepo")
                    .global(true)
            )
            .arg(
                Arg::new("workspace")
                    .long("workspace")
                    .short('w')
                    .alias("global")
                    .action(clap::ArgAction::SetTrue)
                    .help("Operate on every project in the workspace, ignoring the current directory")
                    .global(true)
            )
            .arg(
                Arg::new("root")
                    .long("root")
                    .action(clap::ArgAction::SetTrue)
                    .help("Resolve the outermost enclosing metarepo instead of the nearest one")
                    .global(true)
            );

        // Apply the standard help layout (Options before Commands) to the whole
        // command tree so every subcommand matches the top-level ordering.
        metarepo_core::with_standard_help(app)
    }

    pub fn run(&self, args: Vec<String>) -> Result<()> {
        // Initialize tracing
        self.init_logging();

        // Check if --experimental or -x is present in args
        let experimental = args
            .iter()
            .any(|arg| arg == "--experimental" || arg == "-x");

        // If experimental, create a new CLI with experimental plugins
        if experimental {
            let cli = Self::new_with_flags(true);
            return cli.run_with_experimental(args);
        }

        // Normal execution without experimental features.
        //
        // Load external plugins (declared in .metarepo plus those discovered in
        // the plugins directory) before building the app so their commands show
        // up in --help and are recognized during argument parsing.
        if let Ok(meta_config) = metarepo_core::MetaConfig::load() {
            self.registry
                .borrow_mut()
                .load_external_plugins(&meta_config);
        }

        let app = self.build_app();
        let matches = app.try_get_matches_from(args)?;

        // Parse non-interactive mode if provided
        let non_interactive = matches
            .get_one::<String>("non-interactive")
            .and_then(|s| NonInteractiveMode::from_str(s).ok());

        let config_override = resolve_config_override(matches.get_one::<String>("config"));
        let scope_workspace = matches.get_flag("workspace");
        let discover_root = matches.get_flag("root");

        // Load runtime configuration
        let mut config = create_runtime_config_full(
            false,
            non_interactive,
            config_override,
            scope_workspace,
            discover_root,
        )?;
        // Aggregate declared plugin settings so `meta config` can list them.
        config.settings_catalog = self.registry.borrow().collect_settings();

        // Route to appropriate plugin
        match matches.subcommand() {
            Some((command_name, sub_matches)) => {
                self.registry
                    .borrow()
                    .handle_command(command_name, sub_matches, &config)
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
        // Load external plugins before building the app so their commands show
        // up in --help and are recognized during argument parsing.
        if let Ok(meta_config) = metarepo_core::MetaConfig::load() {
            self.registry
                .borrow_mut()
                .load_external_plugins(&meta_config);
        }

        // Parse with experimental plugins available
        let app = self.build_app_with_flags(true);
        let matches = app.try_get_matches_from(args)?;

        // Parse non-interactive mode if provided
        let non_interactive = matches
            .get_one::<String>("non-interactive")
            .and_then(|s| NonInteractiveMode::from_str(s).ok());

        let config_override = resolve_config_override(matches.get_one::<String>("config"));
        let scope_workspace = matches.get_flag("workspace");
        let discover_root = matches.get_flag("root");

        // Load runtime configuration with experimental flag
        let mut config = create_runtime_config_full(
            true,
            non_interactive,
            config_override,
            scope_workspace,
            discover_root,
        )?;
        config.settings_catalog = self.registry.borrow().collect_settings();

        tracing::debug!("Experimental features enabled");

        // Route to appropriate plugin
        match matches.subcommand() {
            Some((command_name, sub_matches)) => {
                self.registry
                    .borrow()
                    .handle_command(command_name, sub_matches, &config)
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

        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("meta=info"));

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

/// Resolve the effective `--config` override: explicit flag wins, then the
/// `METAREPO_CONFIG` env var, otherwise None (let discovery run).
fn resolve_config_override(flag: Option<&String>) -> Option<PathBuf> {
    if let Some(path) = flag {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("METAREPO_CONFIG").map(PathBuf::from)
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
            Ok(_) => {}
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
