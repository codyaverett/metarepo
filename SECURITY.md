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
