//! Entry point for the example plugin.
//!
//! In subprocess mode (how metarepo invokes plugins) the SDK's `serve` runs the
//! request loop over stdin/stdout. Run directly from a shell, it prints a short
//! usage note instead.

use metarepo_plugin_example::ExamplePlugin;

fn main() -> anyhow::Result<()> {
    if std::env::var_os("METAREPO_PLUGIN_MODE").is_some() {
        metarepo_plugin_sdk::serve(ExamplePlugin::new())
    } else {
        println!("metarepo example plugin");
        println!();
        println!("This is an external plugin for the metarepo CLI. metarepo runs it");
        println!("automatically in subprocess mode; it is not meant to be run directly.");
        println!();
        println!("Commands once registered:");
        println!("  meta example hello <name>   Print a greeting");
        println!("  meta example info           Show repository information");
        println!("  meta example count          Count projects in the repository");
        Ok(())
    }
}
