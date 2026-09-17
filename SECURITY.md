# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.10.x  | :white_check_mark: |
| < 0.10  | :x:                |

## Reporting a Vulnerability

We take the security of metarepo seriously. If you discover a security vulnerability, please follow these steps:

### Private Disclosure Process

**DO NOT** create a public GitHub issue for security vulnerabilities.

Instead, please use one of these methods:

#### 1. GitHub Security Advisory (Recommended)

1. Go to the [Security tab](https://github.com/caavere/metarepo/security)
2. Click "Report a vulnerability"
3. Fill out the security advisory form with:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

#### 2. Email Report

Send details to: **[Your security email address]**

Include:
- Description of the vulnerability
- Steps to reproduce the issue
- Potential impact and attack scenarios
- Any suggested fixes or mitigations
- Your contact information for follow-up

### What to Expect

- **Acknowledgment**: We will acknowledge receipt of your report within 48 hours
- **Initial Assessment**: We will provide an initial assessment within 5 business days
- **Status Updates**: We will keep you informed of our progress
- **Credit**: We will credit you in the security advisory (unless you prefer to remain anonymous)
- **Fix Timeline**: We aim to release security fixes within 30 days for critical issues

### Security Vulnerability Scope

We are interested in vulnerabilities including but not limited to:

- **Command Injection**: Unsafe execution of shell commands
- **Path Traversal**: Unauthorized file system access
- **Code Execution**: Remote or local code execution vulnerabilities
- **Privilege Escalation**: Unauthorized elevation of privileges
- **Information Disclosure**: Unintended exposure of sensitive information
- **Dependency Vulnerabilities**: Critical security issues in dependencies

### Out of Scope

The following are generally **not** considered security vulnerabilities:

- Denial of service through local resource exhaustion (expected for build tools)
- Issues requiring physical access to a developer's machine
- Social engineering attacks
- Vulnerabilities in development/test dependencies
- Issues that require the user to run malicious code directly

## Reporting a Supply-Chain Compromise

A supply-chain report is different from a vulnerability report: it is not about a bug in metarepo's own code, but about a dependency, a maintainer account, or a published artifact being tampered with or malicious. Use this section instead of the vulnerability scope above when that is what you suspect.

### Suspected Compromise of a Dependency

If you believe a crate metarepo depends on has been compromised (malicious code injected, a maintainer account hijacked, a malicious version published, a typosquat, etc.):

1. **Report upstream first, if the compromise is in the crate's own code.** The crate's maintainers are the ones who can yank, patch, or rotate credentials for their own package; metarepo only consumes it. Report through the crate's own security policy or contact, and consider filing with [RustSec](https://rustsec.org/) so the advisory database picks it up.
2. **Also tell us**, using the same private disclosure process above (Security Advisory or email), so we can mitigate on our side while upstream responds. Include:
   - The crate name and affected version(s)
   - Evidence of the compromise
   - Whether you have already contacted the upstream maintainers
3. We will assess whether to pin to a known-good version, patch around it, or drop the dependency until it is resolved.

### Suspected Compromise of a Published Artifact

This covers our own published outputs: the crates published to crates.io, or the binaries attached to GitHub Releases.

Report through the private disclosure process above (Security Advisory or email), including:

- Which artifact and version (crate name and version, or release tag and target platform for a binary)
- How you obtained it and what looked wrong (unexpected checksum, unexpected file contents, a release that does not match the tagged source, etc.)

Please do not run or further distribute the suspect artifact beyond what is needed to capture evidence.

### What We Do Today

The automated checks in `.github/workflows/security.yml` (see below) are our current supply-chain controls:

- `cargo audit` fails the build on any advisory not explicitly listed in `.cargo/audit.toml`, and each listed exception is annotated with why it is there so it can be revisited
- `cargo deny` enforces `deny.toml`: an allowed-license list, a deny list for specific crates/versions, `wildcards = "deny"` to block wildcard version requirements, `yanked = "deny"` to fail if a yanked crate version is in the lockfile, and unknown package registries denied (unknown git sources currently only produce a warning)
- `cargo geiger` reports unsafe code usage in the dependency tree (informational; it does not currently block CI)
- GitHub Actions across our workflows are pinned to a specific commit SHA rather than a floating tag or branch, so a workflow step cannot be silently repointed to different code
- Our [threat model](docs/security/threat-model.md) documents what we trust and what mitigations are already in place across the dependency, CI, and release pipeline

A few further hardening steps are planned but not yet implemented, so do not assume they are in place: publishing an SBOM with releases ([#44](https://github.com/codyaverett/metarepo/issues/44)), crates.io trusted publishing via OIDC ([#38](https://github.com/codyaverett/metarepo/issues/38)), vendoring dependencies for release builds ([#35](https://github.com/codyaverett/metarepo/issues/35)), and provenance attestation for release artifacts ([#96](https://github.com/codyaverett/metarepo/issues/96)).

### What to Expect

Supply-chain reports go through the same response process as other vulnerability reports:

- **Acknowledgment**: within 48 hours
- **Initial Assessment**: within 5 business days
- **Status Updates**: as we work with you and, where applicable, upstream maintainers
- **Fix Timeline**: critical supply-chain issues follow the same 30-day target as other critical fixes

## Security Best Practices

When using metarepo:

1. **Keep Updated**: Always use the latest version
2. **Review Scripts**: Inspect any third-party plugins or scripts before use
3. **Secure Credentials**: Never commit credentials or secrets to repositories
4. **Limit Permissions**: Run metarepo with minimal necessary permissions
5. **Audit Dependencies**: Regularly review and update dependencies

## Automated Security Scanning

This project includes automated security scanning:

- **cargo-audit**: Dependency vulnerability scanning
- **cargo-deny**: License and dependency policy enforcement
- **cargo-geiger**: Unsafe code detection
- **Security-focused clippy**: Additional security lints
- **GitHub Dependabot**: Automated dependency updates

See `.github/workflows/security.yml` for details.

## Security Updates

Security advisories are published in:
- [GitHub Security Advisories](https://github.com/caavere/metarepo/security/advisories)
- Release notes for security patches
- Email notifications (for critical issues)

## Contact

For security-related questions that are **not** vulnerabilities:
- Open a [GitHub Discussion](https://github.com/caavere/metarepo/discussions)
- Create a non-sensitive issue using the Security template

For **actual vulnerabilities**, please follow the private disclosure process above.

---

**Thank you for helping keep metarepo and its users secure!**
