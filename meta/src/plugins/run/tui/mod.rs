//! Interactive `meta run --tui`: a fuzzy script picker followed by a live
//! per-project run view with streaming output.
//!
//! - [`runner`] is the headless orchestrator: it spawns one worker thread per
//!   project, streams child stdout/stderr into a shared [`OutputManager`], and
//!   supports cooperative cancellation. It has no terminal dependency, so it is
//!   unit-tested directly.
//! - [`picker`] is the fuzzy script selector (modeled on the skill picker).
//! - [`live`] is the live run view that renders the manager's state on a tick.

mod live;
mod picker;
pub(crate) mod runner;

use crate::plugins::shared::OutputManager;
use anyhow::Result;
use metarepo_core::tui::{init_terminal, restore_terminal};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::{gather_scripts, scripts_scoped_projects};

/// Entry point for `meta run --tui`.
///
/// If `preselected` names a script it is run directly; otherwise the user picks
/// one from a fuzzy list of the workspace's cascade-merged scripts. The chosen
/// script runs across its in-scope projects in a live streaming view. Returns
/// `Ok(false)` when the user cancels at the picker (nothing ran).
pub(crate) fn run_tui(
    preselected: Option<&str>,
    base_path: &Path,
    scope: &[String],
    env_vars: &HashMap<String, String>,
) -> Result<bool> {
    let config = super::load_config_with_script_cascade(base_path)?;
    let scripts = gather_scripts(&config);
    if scripts.is_empty() {
        println!("No scripts defined in this workspace.");
        return Ok(false);
    }

    let mut terminal = init_terminal()?;

    // Resolve the script index: explicit name, or interactive pick.
    let idx = match preselected {
        Some(name) => scripts.iter().position(|s| s.name == name),
        None => match picker::pick_script(&mut terminal, scripts.clone()) {
            Ok(i) => i,
            Err(e) => {
                restore_terminal(terminal)?;
                return Err(e);
            }
        },
    };

    let Some(idx) = idx else {
        restore_terminal(terminal)?;
        if let Some(name) = preselected {
            println!("Script '{name}' not found.");
        }
        return Ok(false);
    };
    let script = scripts[idx].clone();

    // Projects: those in scope that define this script.
    let projects = scripts_scoped_projects(&config, &script.name, scope);
    if projects.is_empty() {
        restore_terminal(terminal)?;
        println!("No in-scope projects define script '{}'.", script.name);
        return Ok(false);
    }

    let manager = Arc::new(OutputManager::new(projects.clone()));
    let cancel = Arc::new(AtomicBool::new(false));
    let handles = runner::spawn_all(
        &script.name,
        projects.clone(),
        base_path.to_path_buf(),
        Arc::new(config),
        env_vars.clone(),
        Arc::clone(&manager),
        Arc::clone(&cancel),
    );

    let mut view =
        live::LiveRunView::new(Arc::clone(&manager), cancel, projects, script.name.clone());
    let loop_result = view.run(&mut terminal);

    restore_terminal(terminal)?;

    // The user may quit before children exit; ensure they are reaped (the view's
    // quit path already set the cancel flag, so this returns promptly).
    for h in handles {
        let _ = h.join();
    }
    loop_result?;

    // Print a final summary on the normal screen.
    manager.display_final_results();
    Ok(true)
}
