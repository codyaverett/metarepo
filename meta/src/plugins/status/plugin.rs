//! Plugin wiring for the `meta status` dashboard.

use anyhow::Result;
use clap::{ArgMatches, Command};
use metarepo_core::{BasePlugin, MetaPlugin, RuntimeConfig};

use super::dashboard::Dashboard;

/// Registers the top-level `meta status` command.
pub struct StatusPlugin;

impl StatusPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatusPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl MetaPlugin for StatusPlugin {
    fn name(&self) -> &str {
        "status"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("status")
                .about("Interactive multi-repo status dashboard")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Open an interactive dashboard of the workspace's git status.\n\
                     \n\
                     Shows each in-scope project with its branch, ahead/behind counts versus\n\
                     the upstream, and working-tree change count, in a navigable tree. Select a\n\
                     project to see its details on the right. The view is read-only: navigate\n\
                     with the arrow keys or j/k, press r to refresh, ? for help, and q to quit.\n\
                     \n\
                     Examples:\n  \
                       meta status                 Dashboard for the whole workspace\n  \
                       cd team/api && meta status  Dashboard scoped to the current directory\n",
                )),
        )
    }

    fn handle_command(&self, _matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let base_path = config
            .meta_root()
            .unwrap_or_else(|| config.working_dir.clone());
        let projects = config.scoped_project_keys();
        if projects.is_empty() {
            println!("No projects in this workspace. Run 'meta project add' to track one.");
            return Ok(());
        }
        Dashboard::new(base_path, projects).run()
    }
}

impl BasePlugin for StatusPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }

    fn description(&self) -> Option<&str> {
        Some("Interactive multi-repo status dashboard")
    }
}
