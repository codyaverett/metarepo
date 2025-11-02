# Security Documentation

This directory contains security-related documentation, testing strategies, and implementation guides for the Metarepo project.

## 📁 Directory Structure

```
docs/security/
├── README.md                          # This file
├── SECURITY_TESTING_STRATEGY.md       # Comprehensive security testing strategy
├── SECURITY_HOTFIX.patch              # Critical vulnerability patches
└── implement-security.sh              # Automated security implementation script
```

## 📋 Related Security Files

Security infrastructure is distributed across the project:

- **`/deny.toml`** - cargo-deny configuration for dependency security scanning
- **`/fuzz/`** - Fuzzing targets for vulnerability discovery
  - `fuzz_targets/command_injection.rs` - Command injection fuzzer
  - `fuzz_targets/path_traversal.rs` - Path traversal fuzzer
- **`/.github/workflows/security.yml`** - Automated security testing workflow
- **`/tests/security_tests.rs`** - Security test suite

## 🚨 Critical Vulnerabilities

The project has been assessed and the following critical vulnerabilities were identified:

1. **Command Injection** (CVSS 9.8) - in worktree_init and script execution
2. **Path Traversal** - in project path handling
3. **Environment Variable Injection** - in subprocess execution

## 🔧 Quick Start

### 1. Apply Security Hotfix (IMMEDIATE)

```bash
# From project root
git apply docs/security/SECURITY_HOTFIX.patch
```

### 2. Run Implementation Script

```bash
# From project root
chmod +x docs/security/implement-security.sh
./docs/security/implement-security.sh
```

### 3. Install Security Tools

```bash
cargo install cargo-audit cargo-deny cargo-fuzz
```

### 4. Run Security Checks

```bash
# Audit dependencies for known vulnerabilities
cargo audit

# Check dependencies against security policies
cargo deny check

# Run security test suite
cargo test --test security_tests

# Run fuzzing campaigns (requires nightly Rust)
cargo +nightly fuzz run command_injection
cargo +nightly fuzz run path_traversal
```

## 📖 Documentation

### SECURITY_TESTING_STRATEGY.md

Complete security testing strategy including:

- **Security Testing Framework** - SAST, DAST, fuzzing, penetration testing methodologies
- **Priority Test Areas** - Critical vulnerabilities ranked by severity
- **Specific Test Cases** - Concrete test scenarios for each vulnerability type
- **Tools and Automation** - Recommended tools and CI/CD integration
- **Security Hardening** - Code examples and best practices
- **Continuous Testing Plan** - Ongoing security monitoring strategy
- **Implementation Roadmap** - Week-by-week implementation schedule

### SECURITY_HOTFIX.patch

Critical security patches addressing:

- Command injection prevention in worktree_init
- Command injection prevention in script execution
- Input validation and sanitization
- Path canonicalization

### implement-security.sh

Automated script that:

- Checks for required dependencies
- Installs security tools (cargo-audit, cargo-deny)
- Runs security audits
- Executes security test suite
- Generates security report

## 🛡️ Security Testing Workflow

### Daily (Automated via CI/CD)

- Dependency vulnerability scanning (`cargo audit`)
- Security linting (`cargo deny check`)
- Security test suite execution

### Weekly

- Review dependency updates
- Run fuzzing campaigns
- Review security test coverage

### Monthly

- Comprehensive security review
- Update security testing strategy
- Penetration testing exercises

### Before Each Release

- Full security audit
- Manual code review of security-sensitive areas
- Update security documentation

## 🔐 Security Best Practices

### Input Validation

All user input must be validated and sanitized:

```rust
use shlex;

// GOOD: Parse shell commands safely
if let Some(args) = shlex::split(&user_command) {
    Command::new(&args[0])
        .args(&args[1..])
        .spawn()
}

// BAD: Direct shell execution
Command::new("sh")
    .arg("-c")
    .arg(&user_command)  // Vulnerable to injection!
    .spawn()
```

### Path Handling

Canonicalize and validate all paths:

```rust
use std::path::PathBuf;

// GOOD: Canonicalize and validate
let canonical_path = path.canonicalize()?;
if !canonical_path.starts_with(&base_dir) {
    return Err("Path traversal attempt detected");
}

// BAD: Use paths directly
let path = PathBuf::from(user_input);
```

### Command Execution

Use direct execution instead of shell:

```rust
// GOOD: Direct execution
Command::new("git")
    .args(&["clone", url, path])
    .spawn()

// BAD: Shell execution
Command::new("sh")
    .arg("-c")
    .arg(format!("git clone {} {}", url, path))
    .spawn()
```

## 📊 Security Metrics

Track these key performance indicators:

- **Vulnerability Discovery Rate** - New vulnerabilities found per week
- **Mean Time to Remediation (MTTR)** - Average time to fix vulnerabilities
- **Security Test Coverage** - Percentage of security-sensitive code covered
- **Dependency Currency** - Percentage of dependencies on latest secure versions
- **False Positive Rate** - Percentage of false positives in automated scans

## 🚀 Implementation Timeline

### Week 1 (IMMEDIATE - Critical)

- [ ] Apply SECURITY_HOTFIX.patch
- [ ] Fix command injection vulnerabilities
- [ ] Implement input validation
- [ ] Add cargo-audit to CI/CD pipeline

### Week 2 (High Priority)

- [ ] Implement path traversal prevention
- [ ] Configure cargo-deny policies
- [ ] Set up security test suite
- [ ] Add security.yml workflow

### Week 3-4 (Medium Priority)

- [ ] Implement fuzzing infrastructure
- [ ] Add resource limits and rate limiting
- [ ] Enhance plugin security validation
- [ ] Complete security documentation

### Ongoing

- [ ] Weekly security audits
- [ ] Monthly penetration testing
- [ ] Continuous dependency monitoring
- [ ] Regular security training

## 📞 Reporting Security Vulnerabilities

If you discover a security vulnerability:

1. **DO NOT** create a public GitHub issue
2. Email security reports to: [security contact email]
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if available)

## 📚 Additional Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [cargo-audit Documentation](https://docs.rs/cargo-audit/)
- [cargo-deny Documentation](https://embarkstudios.github.io/cargo-deny/)
- [cargo-fuzz Documentation](https://rust-fuzz.github.io/book/cargo-fuzz.html)

## 🔄 Updating This Documentation

This documentation should be reviewed and updated:

- After each security incident
- When new vulnerabilities are discovered
- When security tools or processes change
- At least quarterly as part of regular maintenance

---

Last Updated: 2025-11-01
Version: 0.8.3
