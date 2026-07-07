//! Headless orchestrator for the live run view.
//!
//! Spawns one worker thread per project. Each worker builds the script command,
//! spawns the child with piped stdout/stderr, drains both pipes on dedicated
//! reader threads (so a full pipe never blocks completion detection), and polls
//! for exit while honoring a shared cancel flag. All output flows into a shared
//! [`OutputManager`] via its streaming append methods, which the UI thread reads
//! by snapshot each tick — no channel is needed since the manager is already
//! `Arc<Mutex<..>>` internally.

use crate::plugins::shared::OutputManager;
use metarepo_core::MetaConfig;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::super::build_script_command;

/// How often a worker polls its child for exit / checks the cancel flag.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Spawn a worker thread per project to run `script`, streaming output into
/// `manager`. Returns the worker join handles; the caller drives the UI while
/// they run and joins them when [`OutputManager::all_completed`] is true (or the
/// user quits). Setting `cancel` asks in-flight children to be killed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_all(
    script: &str,
    projects: Vec<String>,
    base_path: PathBuf,
    config: Arc<MetaConfig>,
    env_vars: HashMap<String, String>,
    manager: Arc<OutputManager>,
    cancel: Arc<AtomicBool>,
) -> Vec<JoinHandle<()>> {
    projects
        .into_iter()
        .map(|project| {
            let script = script.to_string();
            let base_path = base_path.clone();
            let config = Arc::clone(&config);
            let env_vars = env_vars.clone();
            let manager = Arc::clone(&manager);
            let cancel = Arc::clone(&cancel);
            thread::spawn(move || {
                run_one(
                    &script, &project, &base_path, &config, &env_vars, &manager, &cancel,
                );
            })
        })
        .collect()
}

/// Run a single project's script to completion (or cancellation), streaming its
/// output into `manager`. Records exit code -1 with an error note on any setup
/// or spawn failure so the batch still completes.
#[allow(clippy::too_many_arguments)]
fn run_one(
    script: &str,
    project: &str,
    base_path: &Path,
    config: &MetaConfig,
    env_vars: &HashMap<String, String>,
    manager: &Arc<OutputManager>,
    cancel: &Arc<AtomicBool>,
) {
    manager.start_project(project);

    let (mut cmd, display) =
        match build_script_command(config, script, project, base_path, env_vars) {
            Ok(pair) => pair,
            Err(e) => {
                manager.append_stderr(project, format!("{e}\n").as_bytes());
                manager.finish_project(project, -1);
                return;
            }
        };
    manager.set_project_command(project, display);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            manager.append_stderr(project, format!("failed to spawn: {e}\n").as_bytes());
            manager.finish_project(project, -1);
            return;
        }
    };

    // Drain each pipe on its own thread so a chatty child cannot deadlock by
    // filling one pipe's buffer while we wait on the other.
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_reader(
            stdout,
            project.to_string(),
            Arc::clone(manager),
            true,
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_reader(
            stderr,
            project.to_string(),
            Arc::clone(manager),
            false,
        ));
    }

    let mut killed = false;
    let exit_code = loop {
        if cancel.load(Ordering::Relaxed) && !killed {
            let _ = child.kill();
            killed = true;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(_) => break -1,
        }
    };

    // Ensure all buffered output is drained before reporting completion.
    for r in readers {
        let _ = r.join();
    }
    if killed {
        manager.append_stderr(project, b"\n[cancelled]\n");
    }
    manager.finish_project(project, exit_code);
}

/// Spawn a thread that reads `pipe` in chunks and appends each into `manager`
/// for `project`, routing to stdout or stderr per `is_stdout`.
fn spawn_reader<R: Read + Send + 'static>(
    mut pipe: R,
    project: String,
    manager: Arc<OutputManager>,
    is_stdout: bool,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if is_stdout {
                        manager.append_stdout(&project, &buf[..n]);
                    } else {
                        manager.append_stderr(&project, &buf[..n]);
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use metarepo_core::MetaConfig;
    use std::time::Instant;
    use tempfile::tempdir;

    /// Build a workspace with a `.meta` defining `scripts` and the given project
    /// directories, returning (base_path, loaded config).
    fn workspace(scripts: &[(&str, &str)], projects: &[&str]) -> (PathBuf, MetaConfig) {
        let tmp = tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        // Leak the tempdir so it outlives the test body (children run in it).
        std::mem::forget(tmp);

        let mut script_json = String::new();
        for (i, (name, cmd)) in scripts.iter().enumerate() {
            if i > 0 {
                script_json.push(',');
            }
            script_json.push_str(&format!("\"{name}\": \"{cmd}\""));
        }
        let mut proj_json = String::new();
        for (i, p) in projects.iter().enumerate() {
            if i > 0 {
                proj_json.push(',');
            }
            proj_json.push_str(&format!("\"{p}\": \"./{p}\""));
            std::fs::create_dir_all(base.join(p)).unwrap();
        }
        let meta = format!("{{\"projects\": {{{proj_json}}}, \"scripts\": {{{script_json}}}}}");
        std::fs::write(base.join(".meta"), meta).unwrap();

        let config = MetaConfig::load_from_file(base.join(".meta")).unwrap();
        (base, config)
    }

    #[test]
    fn streams_output_and_exit_codes() {
        let (base, config) = workspace(
            &[("hi", "echo hello"), ("boom", "sh -c 'exit 3'")],
            &["a", "b"],
        );
        let projects = vec!["a".to_string(), "b".to_string()];
        let manager = Arc::new(OutputManager::new(projects.clone()));
        let cancel = Arc::new(AtomicBool::new(false));

        // Run the "hi" script in both projects.
        let handles = spawn_all(
            "hi",
            projects,
            base.clone(),
            Arc::new(config.clone()),
            HashMap::new(),
            Arc::clone(&manager),
            cancel,
        );
        for h in handles {
            h.join().unwrap();
        }

        assert!(manager.all_completed());
        for p in ["a", "b"] {
            let out = manager.get_project_output(p).unwrap();
            assert_eq!(out.exit_code, Some(0));
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
        }

        // A failing script reports its non-zero exit code.
        let manager2 = Arc::new(OutputManager::new(vec!["a".to_string()]));
        let handles = spawn_all(
            "boom",
            vec!["a".to_string()],
            base,
            Arc::new(config),
            HashMap::new(),
            Arc::clone(&manager2),
            Arc::new(AtomicBool::new(false)),
        );
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(manager2.get_project_output("a").unwrap().exit_code, Some(3));
    }

    #[test]
    fn cancel_kills_in_flight_children_promptly() {
        let (base, config) = workspace(&[("slow", "sleep 5")], &["a"]);
        let manager = Arc::new(OutputManager::new(vec!["a".to_string()]));
        let cancel = Arc::new(AtomicBool::new(false));

        let start = Instant::now();
        let handles = spawn_all(
            "slow",
            vec!["a".to_string()],
            base,
            Arc::new(config),
            HashMap::new(),
            Arc::clone(&manager),
            Arc::clone(&cancel),
        );
        // Let the child get going, then cancel.
        thread::sleep(Duration::from_millis(300));
        cancel.store(true, Ordering::Relaxed);
        for h in handles {
            h.join().unwrap();
        }

        // Well under the script's 5s sleep.
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(manager.all_completed());
    }
}
