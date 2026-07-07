use crate::{create_runtime_config_full, PluginRegistry};
use anyhow::Result;
use clap::{Arg, ColorChoice, Command};
use metarepo_core::NonInteractiveMode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

/// Normalize the illogical `meta <cmd> help --help` (and `-h`) into
/// `meta <cmd> --help` by dropping the `help` token that sits directly before the
/// help flag. Commands that forward opaque external subcommands (`exec` and its
/// aliases) are left untouched so `meta exec help --help` still runs across repos.
fn strip_help_before_help_flag(args: Vec<String>) -> Vec<String> {
    const EXTERNAL: [&str; 3] = ["exec", "e", "x"];
    if let Some(first) = args.iter().skip(1).find(|a| !a.starts_with('-')) {
        if EXTERNAL.contains(&first.as_str()) {
            return args;
        }
    }
    if let Some(i) = args.iter().position(|a| a == "help") {
        if matches!(
            args.get(i + 1).map(String::as_str),
            Some("--help") | Some("-h")
        ) {
            let mut out = args;
            out.remove(i);
            return out;
        }
    }
    args
}

/// Apply per-command `helpDescription` overrides from `.meta` onto the clap
/// command tree. Each key is a dotted command path (e.g. "project.add"); a match
/// replaces that command's man-page `Description:` section, winning over whatever
/// the plugin/module declared.
fn apply_help_overrides(cmd: Command, prefix: &str, map: &HashMap<String, String>) -> Command {
    cmd.mut_subcommands(|sub| {
        let path = if prefix.is_empty() {
            sub.get_name().to_string()
        } else {
            format!("{}.{}", prefix, sub.get_name())
        };
        let sub = apply_help_overrides(sub, &path, map);
        match map.get(&path) {
            Some(body) => {
                let rendered: &'static str =
                    Box::leak(metarepo_core::format_help_description(body).into_boxed_str());
                sub.after_long_help(rendered)
            }
            None => sub,
        }
    })
}

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
                Arg::new("allow-any-path")
                    .long("allow-any-path")
                    .action(clap::ArgAction::SetTrue)
                    .help("Load external plugins from any directory, bypassing the plugin-path allowlist")
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

        // `meta <cmd> help --help` is illogical but should just show <cmd>'s help.
        let args = strip_help_before_help_flag(args);

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
        let meta_config = metarepo_core::MetaConfig::load().ok();
        if let Some(ref mc) = meta_config {
            self.registry.borrow_mut().load_external_plugins(mc);
        }

        let mut app = self.build_app();
        if let Some(map) = meta_config
            .as_ref()
            .and_then(|mc| mc.help_descriptions.as_ref())
        {
            if !map.is_empty() {
                app = apply_help_overrides(app, "", map);
            }
        }
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
        let meta_config = metarepo_core::MetaConfig::load().ok();
        if let Some(ref mc) = meta_config {
            self.registry.borrow_mut().load_external_plugins(mc);
        }

        // Parse with experimental plugins available
        let mut app = self.build_app_with_flags(true);
        if let Some(map) = meta_config
            .as_ref()
            .and_then(|mc| mc.help_descriptions.as_ref())
        {
            if !map.is_empty() {
                app = apply_help_overrides(app, "", map);
            }
        }
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

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn strip_help_before_help_flag_rewrites() {
        assert_eq!(
            strip_help_before_help_flag(v(&["meta", "project", "help", "--help"])),
            v(&["meta", "project", "--help"])
        );
        assert_eq!(
            strip_help_before_help_flag(v(&["meta", "help", "--help"])),
            v(&["meta", "--help"])
        );
        assert_eq!(
            strip_help_before_help_flag(v(&["meta", "project", "help", "-h"])),
            v(&["meta", "project", "-h"])
        );
    }

    #[test]
    fn strip_help_before_help_flag_leaves_other_cases() {
        // Plain `help` (no trailing help flag) is untouched.
        assert_eq!(
            strip_help_before_help_flag(v(&["meta", "project", "help"])),
            v(&["meta", "project", "help"])
        );
        // `exec` forwards opaque args — never rewritten.
        assert_eq!(
            strip_help_before_help_flag(v(&["meta", "exec", "help", "--help"])),
            v(&["meta", "exec", "help", "--help"])
        );
        // A non-adjacent help flag is not collapsed.
        assert_eq!(
            strip_help_before_help_flag(v(&["meta", "project", "help", "add", "--help"])),
            v(&["meta", "project", "help", "add", "--help"])
        );
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
