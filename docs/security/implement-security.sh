#!/bin/bash

# Metarepo Security Implementation Script
# This script helps implement immediate security fixes and testing

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "=========================================="
echo "Metarepo Security Implementation"
echo "=========================================="
echo ""

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[*]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

# Check if running as part of CI
if [ "${CI:-false}" = "true" ]; then
    print_status "Running in CI environment"
fi

# Step 1: Install security dependencies
print_status "Installing security dependencies..."

# Add security dependencies to Cargo.toml if not present
if ! grep -q "shlex" meta/Cargo.toml; then
    print_warning "Adding shlex dependency to meta/Cargo.toml"
    sed -i.bak '/\[dependencies\]/a\
shlex = "1.3"' meta/Cargo.toml
fi

if ! grep -q "tempfile" meta/Cargo.toml; then
    print_warning "Adding tempfile to dev-dependencies"
    if ! grep -q "\[dev-dependencies\]" meta/Cargo.toml; then
        echo "" >> meta/Cargo.toml
        echo "[dev-dependencies]" >> meta/Cargo.toml
    fi
    sed -i.bak '/\[dev-dependencies\]/a\
tempfile = "3.8"' meta/Cargo.toml
fi

print_success "Dependencies configured"

# Step 2: Install security tools
print_status "Installing security tools..."

# Check if cargo-audit is installed
if ! command -v cargo-audit &> /dev/null; then
    print_warning "Installing cargo-audit..."
    cargo install cargo-audit --features=fix
else
    print_success "cargo-audit already installed"
fi

# Check if cargo-deny is installed
if ! command -v cargo-deny &> /dev/null; then
    print_warning "Installing cargo-deny..."
    cargo install cargo-deny
else
    print_success "cargo-deny already installed"
fi

# Check if cargo-outdated is installed
if ! command -v cargo-outdated &> /dev/null; then
    print_warning "Installing cargo-outdated..."
    cargo install cargo-outdated
else
    print_success "cargo-outdated already installed"
fi

print_success "Security tools installed"

# Step 3: Run initial security audit
print_status "Running security audit..."

echo ""
echo "Checking for known vulnerabilities..."
if cargo audit; then
    print_success "No known vulnerabilities found"
else
    print_warning "Vulnerabilities detected - review output above"
fi

# Step 4: Check dependencies with cargo-deny
print_status "Checking dependencies..."

if [ -f "deny.toml" ]; then
    echo ""
    echo "Running cargo deny check..."
    if cargo deny check 2>/dev/null; then
        print_success "Dependency check passed"
    else
        print_warning "Dependency issues found - review output above"
    fi
else
    print_warning "deny.toml not found - skipping cargo-deny check"
fi

# Step 5: Run security tests if they exist
print_status "Running security tests..."

if [ -f "tests/security_tests.rs" ]; then
    echo ""
    if cargo test --test security_tests 2>/dev/null; then
        print_success "Security tests passed"
    else
        print_error "Security tests failed - critical issues detected!"
        print_warning "Apply SECURITY_HOTFIX.patch immediately"
    fi
else
    print_warning "Security tests not found at tests/security_tests.rs"
fi

# Step 6: Check for the security hotfix
print_status "Checking for security hotfix..."

if [ -f "SECURITY_HOTFIX.patch" ]; then
    print_warning "Critical security patch available!"
    echo ""
    echo "To apply the security hotfix, run:"
    echo "  git apply SECURITY_HOTFIX.patch"
    echo ""
    read -p "Apply security hotfix now? (y/N) " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if git apply --check SECURITY_HOTFIX.patch 2>/dev/null; then
            git apply SECURITY_HOTFIX.patch
            print_success "Security hotfix applied successfully"
        else
            print_error "Failed to apply patch - may already be applied or conflicts exist"
        fi
    fi
fi

# Step 7: Setup GitHub Actions workflow
print_status "Checking GitHub Actions security workflow..."

if [ ! -f ".github/workflows/security.yml" ]; then
    print_warning "Security workflow not found"
    if [ -d ".github/workflows" ] || mkdir -p .github/workflows; then
        print_status "GitHub Actions workflow directory ready"
    fi
else
    print_success "Security workflow exists"
fi

# Step 8: Initialize fuzzing
print_status "Checking fuzzing setup..."

if [ ! -d "fuzz" ]; then
    print_warning "Fuzzing not initialized"
    echo "To initialize fuzzing:"
    echo "  cargo install cargo-fuzz"
    echo "  cargo fuzz init"
else
    print_success "Fuzzing directory exists"
fi

# Step 9: Generate security report
print_status "Generating security report..."

REPORT_FILE="SECURITY_REPORT_$(date +%Y%m%d_%H%M%S).md"

cat > "$REPORT_FILE" << EOF
# Security Implementation Report
Generated: $(date)

## Environment
- Rust Version: $(rustc --version)
- Cargo Version: $(cargo --version)
- Platform: $(uname -s)

## Security Tools Status
- cargo-audit: $(if command -v cargo-audit &> /dev/null; then echo "✓ Installed"; else echo "✗ Not installed"; fi)
- cargo-deny: $(if command -v cargo-deny &> /dev/null; then echo "✓ Installed"; else echo "✗ Not installed"; fi)
- cargo-outdated: $(if command -v cargo-outdated &> /dev/null; then echo "✓ Installed"; else echo "✗ Not installed"; fi)

## Security Files Status
- SECURITY_TESTING_STRATEGY.md: $(if [ -f "SECURITY_TESTING_STRATEGY.md" ]; then echo "✓ Present"; else echo "✗ Missing"; fi)
- tests/security_tests.rs: $(if [ -f "tests/security_tests.rs" ]; then echo "✓ Present"; else echo "✗ Missing"; fi)
- deny.toml: $(if [ -f "deny.toml" ]; then echo "✓ Present"; else echo "✗ Missing"; fi)
- .github/workflows/security.yml: $(if [ -f ".github/workflows/security.yml" ]; then echo "✓ Present"; else echo "✗ Missing"; fi)
- fuzz/: $(if [ -d "fuzz" ]; then echo "✓ Present"; else echo "✗ Missing"; fi)

## Vulnerability Scan Results
\`\`\`
$(cargo audit 2>&1 || echo "cargo-audit not available")
\`\`\`

## Outdated Dependencies
\`\`\`
$(cargo outdated 2>&1 || echo "cargo-outdated not available")
\`\`\`

## Next Steps
1. Review and apply SECURITY_HOTFIX.patch if not already applied
2. Run full test suite: \`cargo test\`
3. Enable security workflow in GitHub Actions
4. Set up fuzzing campaigns
5. Schedule regular security audits

## Critical Actions Required
⚠️  **IMMEDIATE**: Apply security hotfix for command injection vulnerabilities
⚠️  **HIGH**: Implement input validation for all user inputs
⚠️  **HIGH**: Replace shell execution with direct command execution
⚠️  **MEDIUM**: Set up continuous security monitoring
EOF

print_success "Security report generated: $REPORT_FILE"

# Step 10: Summary
echo ""
echo "=========================================="
echo "Security Implementation Summary"
echo "=========================================="
echo ""

# Count completed vs pending items
COMPLETED=0
TOTAL=10

[ -f "SECURITY_TESTING_STRATEGY.md" ] && ((COMPLETED++))
[ -f "tests/security_tests.rs" ] && ((COMPLETED++))
[ -f "deny.toml" ] && ((COMPLETED++))
[ -f ".github/workflows/security.yml" ] && ((COMPLETED++))
[ -d "fuzz" ] && ((COMPLETED++))
command -v cargo-audit &> /dev/null && ((COMPLETED++))
command -v cargo-deny &> /dev/null && ((COMPLETED++))
grep -q "shlex" meta/Cargo.toml 2>/dev/null && ((COMPLETED++))

echo "Progress: $COMPLETED/$TOTAL tasks completed"
echo ""

if [ $COMPLETED -eq $TOTAL ]; then
    print_success "All security measures implemented!"
else
    print_warning "Security implementation incomplete - $(($TOTAL - $COMPLETED)) tasks remaining"
fi

echo ""
echo "Critical Security Issues:"
echo "------------------------"
print_error "Command injection in worktree_init (CRITICAL)"
print_error "Command injection in script execution (CRITICAL)"
print_warning "Path traversal in project paths (HIGH)"
print_warning "Missing input validation (HIGH)"

echo ""
echo "Recommended Actions:"
echo "-------------------"
echo "1. Apply security hotfix immediately:"
echo "   git apply SECURITY_HOTFIX.patch"
echo ""
echo "2. Run security tests:"
echo "   cargo test --test security_tests"
echo ""
echo "3. Audit dependencies:"
echo "   cargo audit fix"
echo ""
echo "4. Update all dependencies:"
echo "   cargo update"
echo ""
echo "5. Enable GitHub Actions security workflow:"
echo "   git add .github/workflows/security.yml"
echo "   git commit -m 'feat: add automated security testing workflow'"
echo ""

print_success "Security implementation script completed"

# Exit with error if critical issues remain
if [ $COMPLETED -lt $TOTAL ]; then
    exit 1
fi