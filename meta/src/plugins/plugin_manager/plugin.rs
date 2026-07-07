use anyhow::{anyhow, Context, Result};
use clap::ArgMatches;
use colored::Colorize;
use metarepo_core::{
    arg, command, plugin, BasePlugin, ConfigSetting, ConfigValueType, MetaConfig, MetaPlugin,
    PluginManifest, RuntimeConfig,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::install::{
    install_from_spec, integrity_status, locked_version, record_lock_entry, remove_lock_entry,
    resolved_binary_path, IntegrityStatus,
};
use super::spec::PluginSpec;
use super::verify::version_satisfies;
use crate::plugins::plugin_loader::ExternalPlugin;

/// Manages external metarepo plugins: install / list / remove / update.
pub struct PluginManagerPlugin;

impl PluginManagerPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn create_plugin() -> impl MetaPlugin {
        plugin("plugin")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Manage metarepo plugins")
            .author("Metarepo Contributors")
            .help_description(
                "Install, list, update, remove, and verify external metarepo plugins.\n\
                 \n\
                 External plugins extend the meta CLI with new top-level commands. Each\n\
                 one is recorded under plugins.<name> in the active .metarepo so it loads\n\
                 on the next run, and its checksum is tracked in .metarepo.lock so the\n\
                 installed binary can be verified later. Plugins install from crates.io, a\n\
                 local file, or a git repository, and may be either protocol binaries or\n\
                 manifest-based plugins.\n\
                 \n\
                 Examples:\n\
                 \n\
                   meta plugin install hello   add an external plugin\n\
                   meta plugin list            show registered plugins and status\n\
                   meta plugin verify          check binaries against the lockfile",
            )
            .command(
                command("install")
                    .about("Install an external plugin and register it in .metarepo")
                    .help_description(
                        "Install an external plugin and record it under plugins.<name> in the\n\
                         active .metarepo so it loads on the next run.\n\
                         \n\
                         The plugin name becomes a top-level command (install hello makes\n\
                         meta hello available). After fetching the binary, the plugin is\n\
                         registered in the workspace config and its SHA-256 checksum is\n\
                         recorded in .metarepo.lock so its integrity can be verified later.\n\
                         \n\
                         Source (--from):\n\
                         \n\
                           crates:<crate>   install from crates.io (default: metarepo-plugin-<name>)\n\
                           file:<path>      copy a local executable\n\
                           git+<url>        clone the repo and cargo build --release\n\
                         \n\
                         --version pins a crates.io version (a bare X.Y.Z is treated as the\n\
                         caret requirement ^X.Y.Z, matching Cargo).\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta plugin install hello\n\
                           meta plugin install hello --version 0.2.0\n\
                           meta plugin install hello --from file:./target/release/metarepo-plugin-hello\n\
                           meta plugin install hello --from git+https://github.com/me/metarepo-plugin-hello.git",
                    )
                    .arg(
                        arg("name")
                            .help("Plugin command name (e.g. hello for 'meta hello')")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("from")
                            .long("from")
                            .help("Source spec: crates:<crate>, file:<path>, or git+<url>")
                            .takes_value(true),
                    )
                    .arg(
                        arg("version")
                            .long("version")
                            .help("Version to install (crates.io sources only)")
                            .takes_value(true),
                    ),
            )
            .command(
                command("list")
                    .about("List registered plugins and their install status")
                    .help_description(
                        "List the plugins registered in the active .metarepo and their status.\n\
                         \n\
                         For each plugin it shows the source label and whether the binary is\n\
                         installed, missing, or reports a version that does not satisfy the\n\
                         declared pin. Manifest plugins show the version from their manifest.\n\
                         A checksum that does not match .metarepo.lock is always flagged; the\n\
                         other integrity states are only shown when the workspace requires\n\
                         integrity.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta plugin list\n\
                           meta plugin ls",
                    )
                    .alias("ls"),
            )
            .command(
                command("remove")
                    .about("Remove a plugin from .metarepo (and optionally its binary)")
                    .help_description(
                        "Unregister a plugin from the active .metarepo so it no longer loads.\n\
                         \n\
                         Removes the plugins.<name> entry and its .metarepo.lock checksum. The\n\
                         installed binary is left in place by default; pass --purge to also\n\
                         delete it (for manifest plugins this removes their install directory).\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta plugin remove hello\n\
                           meta plugin rm hello --purge",
                    )
                    .alias("rm")
                    .arg(
                        arg("name")
                            .help("Plugin command name to remove")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("purge")
                            .long("purge")
                            .help("Also delete the installed binary"),
                    ),
            )
            .command(
                command("update")
                    .about("Reinstall a plugin (or all) from its recorded spec")
                    .help_description(
                        "Reinstall plugins from their recorded source to pick up new builds.\n\
                         \n\
                         Re-runs the install for each plugin's stored spec, refreshes its\n\
                         .metarepo.lock checksum, and reports any version change. Pass a name\n\
                         to update one plugin; omit it to update every registered plugin.\n\
                         file: plugins have no upstream to pull from and are skipped (reinstall\n\
                         from the original source instead).\n\
                         \n\
                         --version re-pins a single crates.io plugin to a new version and\n\
                         persists the new spec to .metarepo before reinstalling.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta plugin update\n\
                           meta plugin update hello\n\
                           meta plugin update hello --version 0.3.0",
                    )
                    .arg(
                        arg("name")
                            .help("Plugin to update; omit to update all")
                            .required(false)
                            .takes_value(true),
                    )
                    .arg(
                        arg("version")
                            .long("version")
                            .help("Re-pin to this version before updating (crates.io plugins, single plugin only)")
                            .takes_value(true),
                    ),
            )
            .command(
                command("verify")
                    .about("Verify installed plugin binaries against .metarepo.lock checksums")
                    .help_description(
                        "Check that installed plugin binaries match their recorded checksums.\n\
                         \n\
                         Recomputes the SHA-256 of each installed plugin binary and compares it\n\
                         to the digest recorded in .metarepo.lock. Exits non-zero if any\n\
                         plugin's checksum does not match, so it is suitable for CI. Plugins\n\
                         without a recorded checksum are reported as unverified (reinstall to\n\
                         record one) and do not fail the run.\n\
                         \n\
                         Pass a name to verify a single plugin; omit it to verify all.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta plugin verify\n\
                           meta plugin verify hello",
                    )
                    .arg(
                        arg("name")
                            .help("Plugin to verify; omit to verify all")
                            .required(false)
                            .takes_value(true),
                    ),
            )
            .handler("install", handle_install)
            .handler("list", handle_list)
            .handler("remove", handle_remove)
            .handler("update", handle_update)
            .handler("verify", handle_verify)
            .build()
    }
}

/// Resolve the active config file path, preferring the one the host already
/// loaded (which honors --config), falling back to discovery.
fn active_meta_file(config: &RuntimeConfig) -> Option<PathBuf> {
    config
        .meta_file_path
        .clone()
        .or_else(MetaConfig::find_meta_file)
}

fn require_meta_file(config: &RuntimeConfig) -> Result<PathBuf> {
    active_meta_file(config).ok_or_else(|| {
        anyhow!("No metarepo config found. Run 'meta init' first, or pass --config <path>.")
    })
}

fn load_plugins(meta_file: &PathBuf) -> Result<(MetaConfig, HashMap<String, String>)> {
    let cfg = MetaConfig::load_from_file(meta_file)?;
    let plugins = cfg.plugins.clone().unwrap_or_default();
    Ok((cfg, plugins))
}

fn handle_install(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow!("Plugin name is required"))?;
    let from = matches.get_one::<String>("from").map(String::as_str);
    let version = matches.get_one::<String>("version").map(String::as_str);

    // Require a config up front so we don't install something we can't register.
    let meta_file = require_meta_file(config)?;

    let spec = PluginSpec::from_args(name, from, version)?;
    println!(
        "\n  {} {} ({})",
        "Installing".cyan().bold(),
        name,
        spec.source_label()
    );

    let stored = install_from_spec(name, &spec)?;

    let mut cfg = MetaConfig::load_from_file(&meta_file)?;
    cfg.plugins
        .get_or_insert_with(HashMap::new)
        .insert(name.clone(), stored.to_spec_string());
    cfg.save_to_file(&meta_file)
        .with_context(|| format!("Failed to update {}", meta_file.display()))?;

    // Record the checksum so integrity can be enforced (now or later).
    if let Err(e) = record_lock_entry(&meta_file, name, &stored) {
        eprintln!("  {} Could not record checksum: {}", "⚠".yellow(), e);
    }

    println!(
        "  {} Registered '{}' in {} — available on next run",
        "✓".green(),
        name,
        meta_file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| meta_file.display().to_string())
    );
    Ok(())
}

fn handle_list(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let meta_file = match active_meta_file(config) {
        Some(p) => p,
        None => {
            println!("  {} No metarepo config found.", "·".bright_black());
            return Ok(());
        }
    };

    let (cfg, plugins) = load_plugins(&meta_file)?;
    if plugins.is_empty() {
        println!(
            "  {} No plugins registered in {}",
            "·".bright_black(),
            meta_file.display()
        );
        return Ok(());
    }

    let integrity_required = cfg.integrity_required();
    println!("\n  {}", "Registered plugins".bold());
    if integrity_required {
        println!("  {}", "integrity: required".bright_black());
    }
    let mut names: Vec<&String> = plugins.keys().collect();
    names.sort();
    for name in names {
        let spec_str = &plugins[name];
        match PluginSpec::parse(name, spec_str) {
            Ok(spec) => {
                print_plugin_status(name, &spec);
                print_integrity_line(&meta_file, name, &spec, integrity_required);
            }
            Err(e) => println!("  {} {}  ({})", "✗".red(), name, e),
        }
    }
    Ok(())
}

/// Print an indented integrity annotation under a plugin's `list` entry. A
/// checksum mismatch is always surfaced (tampering matters regardless of mode);
/// the other states are only shown when the workspace enforces integrity, to
/// keep output quiet for everyone else.
fn print_integrity_line(meta_file: &Path, name: &str, spec: &PluginSpec, required: bool) {
    match integrity_status(meta_file, name, spec) {
        IntegrityStatus::Mismatch => println!(
            "      {} checksum does not match .metarepo.lock",
            "integrity: MISMATCH".red().bold()
        ),
        IntegrityStatus::Ok if required => {
            println!("      {}", "integrity: ok".green())
        }
        IntegrityStatus::NotRecorded if required => println!(
            "      {} (reinstall to record a checksum)",
            "integrity: not recorded".yellow()
        ),
        IntegrityStatus::Unreadable(e) if required => {
            println!("      {} ({})", "integrity: unverifiable".yellow(), e)
        }
        _ => {}
    }
}

fn print_plugin_status(name: &str, spec: &PluginSpec) {
    let path = match resolved_binary_path(name, spec) {
        Ok(p) => p,
        Err(e) => {
            println!("  {} {}  (cannot resolve: {})", "✗".red(), name, e);
            return;
        }
    };

    if !path.exists() {
        println!(
            "  {} {}  [{}]  missing — run 'meta plugin install {}'",
            "✗".red(),
            name.bold(),
            spec.source_label(),
            name
        );
        return;
    }

    // Manifest plugins: read the version from the manifest instead of probing
    // (the binary may be a script that doesn't speak the protocol).
    if PluginManifest::is_manifest_path(&path) {
        let version = PluginManifest::from_file_auto(&path)
            .map(|m| m.plugin.version)
            .unwrap_or_else(|_| "?".to_string());
        println!(
            "  {} {}  [manifest]  installed (v{})",
            "✓".green(),
            name.bold(),
            version
        );
        return;
    }

    // Installed: try to probe the reported version for a mismatch check. This is
    // a display path over a binary already in the plugin dir; resolve the
    // allowlist bypass from flag/env only (no workspace config in scope).
    let declared = spec.declared_version();
    let allow_any_path = crate::plugins::plugin_loader::plugin_allow_any_path(None);
    match ExternalPlugin::probe(&path, allow_any_path) {
        Ok((_, installed)) => {
            if let Some(declared) = declared {
                // Match the loader's semver enforcement, not exact equality, so
                // an installed 1.4.2 satisfying a `1.0.0` (i.e. ^1.0.0) pin is
                // not falsely reported as a mismatch.
                if !version_satisfies(declared, &installed) {
                    println!(
                        "  {} {}  [{}]  version mismatch (declared {}, installed {})",
                        "⚠".yellow(),
                        name.bold(),
                        spec.source_label(),
                        declared,
                        installed
                    );
                    return;
                }
            }
            println!(
                "  {} {}  [{}]  installed (v{})",
                "✓".green(),
                name.bold(),
                spec.source_label(),
                installed
            );
        }
        Err(_) => {
            // Binary present but not probeable (e.g. outside the allowed path
            // policy or not protocol-speaking). Report presence only.
            println!(
                "  {} {}  [{}]  installed at {}",
                "✓".green(),
                name.bold(),
                spec.source_label(),
                path.display()
            );
        }
    }
}

fn handle_remove(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow!("Plugin name is required"))?;
    let purge = matches.get_flag("purge");

    let meta_file = require_meta_file(config)?;
    let mut cfg = MetaConfig::load_from_file(&meta_file)?;

    let removed_spec = cfg
        .plugins
        .as_mut()
        .and_then(|plugins| plugins.remove(name));

    let Some(spec_str) = removed_spec else {
        return Err(anyhow!(
            "Plugin '{}' is not registered in {}",
            name,
            meta_file.display()
        ));
    };

    cfg.save_to_file(&meta_file)
        .with_context(|| format!("Failed to update {}", meta_file.display()))?;

    if let Err(e) = remove_lock_entry(&meta_file, name) {
        eprintln!("  {} Could not update lockfile: {}", "⚠".yellow(), e);
    }

    println!(
        "  {} Removed '{}' from {}",
        "✓".green(),
        name,
        meta_file.display()
    );

    if purge {
        if let Ok(spec) = PluginSpec::parse(name, &spec_str) {
            if let Ok(path) = resolved_binary_path(name, &spec) {
                if PluginManifest::is_manifest_path(&path) {
                    // Manifest plugins live in a per-plugin directory; remove it.
                    if let Some(dir) = path.parent() {
                        if dir.exists() {
                            std::fs::remove_dir_all(dir)
                                .with_context(|| format!("Failed to delete {}", dir.display()))?;
                            println!("  {} Deleted {}", "✓".yellow(), dir.display());
                        }
                    }
                } else if path.exists() {
                    std::fs::remove_file(&path)
                        .with_context(|| format!("Failed to delete {}", path.display()))?;
                    println!("  {} Deleted binary {}", "✓".yellow(), path.display());
                }
            }
        }
    }
    Ok(())
}

/// Render a concise version-change suffix for update output: `old → new` when
/// it changed, `(vX)` when known and unchanged, empty otherwise.
fn version_change(before: &Option<String>, after: &Option<String>) -> String {
    match (before, after) {
        (Some(o), Some(n)) if o != n => format!("  {} → {}", o, n),
        (_, Some(n)) => format!("  (v{n})"),
        _ => String::new(),
    }
}

fn handle_update(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let meta_file = require_meta_file(config)?;
    let (_, plugins) = load_plugins(&meta_file)?;
    let repin = matches.get_one::<String>("version").map(String::as_str);

    if let Some(name) = matches.get_one::<String>("name") {
        let spec_str = plugins
            .get(name)
            .ok_or_else(|| anyhow!("Plugin '{}' is not registered", name))?;
        let mut spec = PluginSpec::parse(name, spec_str)?;

        // `--version` re-pins (crates.io only) and persists the new spec before
        // reinstalling, so the change sticks in .metarepo.
        if let Some(version) = repin {
            match &mut spec {
                PluginSpec::Crates { version: v, .. } => *v = Some(version.to_string()),
                _ => return Err(anyhow!("--version can only re-pin crates.io plugins")),
            }
            let mut cfg = MetaConfig::load_from_file(&meta_file)?;
            cfg.plugins
                .get_or_insert_with(HashMap::new)
                .insert(name.clone(), spec.to_spec_string());
            cfg.save_to_file(&meta_file)
                .with_context(|| format!("Failed to update {}", meta_file.display()))?;
        }

        // file: sources record the install destination, so reinstalling would
        // copy the file onto itself and truncate it. They have no upstream to
        // pull from — reinstall from the original source instead.
        if matches!(spec, PluginSpec::File { .. }) {
            println!(
                "\n  {} '{}' was installed from a file source — nothing to update.",
                "·".bright_black(),
                name
            );
            println!(
                "    To refresh it, reinstall from the original source: meta plugin install {name} --from file:<path>"
            );
            return Ok(());
        }
        println!("\n  {} {}", "Updating".cyan().bold(), name);
        let before = locked_version(&meta_file, name);
        let stored = install_from_spec(name, &spec)?;
        if let Err(e) = record_lock_entry(&meta_file, name, &stored) {
            eprintln!("  {} Could not record checksum: {}", "⚠".yellow(), e);
        }
        let after = locked_version(&meta_file, name);
        println!(
            "  {} Updated '{}'{}",
            "✓".green(),
            name,
            version_change(&before, &after)
        );
        return Ok(());
    }

    if plugins.is_empty() {
        println!("  {} No plugins to update.", "·".bright_black());
        return Ok(());
    }

    let mut names: Vec<&String> = plugins.keys().collect();
    names.sort();
    for name in names {
        let spec = match PluginSpec::parse(name, &plugins[name]) {
            Ok(s) => s,
            Err(e) => {
                println!("  {} {} skipped ({})", "✗".red(), name, e);
                continue;
            }
        };
        // file: sources have no upstream to pull from; skip in bulk update.
        if matches!(spec, PluginSpec::File { .. }) {
            println!("  {} {} skipped (file source)", "·".bright_black(), name);
            continue;
        }
        println!("  {} {}", "Updating".cyan(), name);
        let before = locked_version(&meta_file, name);
        match install_from_spec(name, &spec) {
            Ok(stored) => {
                if let Err(e) = record_lock_entry(&meta_file, name, &stored) {
                    eprintln!("    {} could not record checksum: {}", "⚠".yellow(), e);
                }
                let after = locked_version(&meta_file, name);
                println!(
                    "  {} {}{}",
                    "✓".green(),
                    name,
                    version_change(&before, &after)
                );
            }
            Err(e) => println!("  {} {} failed: {}", "✗".red(), name, e),
        }
    }
    Ok(())
}

fn handle_verify(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let meta_file = require_meta_file(config)?;
    let (_, plugins) = load_plugins(&meta_file)?;

    // Verify a single named plugin or every registered one.
    let targets: Vec<String> = match matches.get_one::<String>("name") {
        Some(name) => {
            if !plugins.contains_key(name) {
                return Err(anyhow!("Plugin '{}' is not registered", name));
            }
            vec![name.clone()]
        }
        None => {
            if plugins.is_empty() {
                println!("  {} No plugins registered.", "·".bright_black());
                return Ok(());
            }
            let mut names: Vec<String> = plugins.keys().cloned().collect();
            names.sort();
            names
        }
    };

    println!("\n  {}", "Verifying plugin integrity".bold());
    let mut mismatches = 0;
    let mut unverified = 0;
    for name in &targets {
        let spec = match PluginSpec::parse(name, &plugins[name]) {
            Ok(s) => s,
            Err(e) => {
                println!("  {} {}  (bad spec: {})", "✗".red(), name.bold(), e);
                unverified += 1;
                continue;
            }
        };
        match integrity_status(&meta_file, name, &spec) {
            IntegrityStatus::Ok => println!("  {} {}  matches", "✓".green(), name.bold()),
            IntegrityStatus::Mismatch => {
                println!(
                    "  {} {}  {} — does not match .metarepo.lock",
                    "✗".red(),
                    name.bold(),
                    "MISMATCH".red().bold()
                );
                mismatches += 1;
            }
            IntegrityStatus::NotRecorded => {
                println!(
                    "  {} {}  no checksum recorded (reinstall to record one)",
                    "·".yellow(),
                    name.bold()
                );
                unverified += 1;
            }
            IntegrityStatus::Unreadable(e) => {
                println!("  {} {}  unverifiable ({})", "·".yellow(), name.bold(), e);
                unverified += 1;
            }
        }
    }

    println!();
    if mismatches > 0 {
        return Err(anyhow!(
            "{} plugin(s) failed checksum verification",
            mismatches
        ));
    }
    if unverified > 0 {
        println!(
            "  {} {} verified, {} without a recorded checksum",
            "✓".green(),
            targets.len() - unverified,
            unverified
        );
    } else {
        println!("  {} all {} plugin(s) verified", "✓".green(), targets.len());
    }
    Ok(())
}

impl MetaPlugin for PluginManagerPlugin {
    fn name(&self) -> &str {
        "plugin"
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        Self::create_plugin().register_commands(app)
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        Self::create_plugin().handle_command(matches, config)
    }

    fn settings(&self) -> Vec<ConfigSetting> {
        vec![
            ConfigSetting::new(
                "allow-version-mismatch",
                "Load external plugins even when their version does not satisfy the pin in .meta. Relaxes a security default; precedence: flag --allow-version-mismatch > env METAREPO_ALLOW_VERSION_MISMATCH > this config > false.",
                ConfigValueType::Bool,
            )
            .with_default("false")
            .with_env("METAREPO_ALLOW_VERSION_MISMATCH"),
            ConfigSetting::new(
                "plugin-allow-any-path",
                "Load external plugins from any directory, bypassing the plugin-path allowlist. Relaxes a security default; precedence: flag --allow-any-path > env METAREPO_PLUGIN_ALLOW_ANY_PATH > this config > false.",
                ConfigValueType::Bool,
            )
            .with_default("false")
            .with_env("METAREPO_PLUGIN_ALLOW_ANY_PATH"),
        ]
    }
}

impl BasePlugin for PluginManagerPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Manage metarepo plugins")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for PluginManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}
