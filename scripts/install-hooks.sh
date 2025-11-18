#!/usr/bin/env bash
#
# Install git hooks for the metarepo project
#
# Usage:
#   ./scripts/install-hooks.sh
#   make install-hooks
#
# What this installs:
#   - Pre-commit hook that runs before each commit
#   - Auto-fixes: formatting, trailing whitespace, end-of-file newlines
#   - Validates: clippy, merge conflicts, file sizes, YAML/TOML syntax, security
#
# After installation:
#   - Hook runs automatically on: git commit
#   - Skip when needed with: git commit --no-verify
#   - See detailed docs in README.md
#
# This script can be run multiple times safely (idempotent)
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}Installing git hooks for metarepo...${NC}\n"

# Determine script and project root directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

# Verify we're in a git repository
if [ ! -d "$PROJECT_ROOT/.git" ]; then
    echo -e "${RED}✗ Error: Not in a git repository${NC}"
    echo -e "  Current directory: $PROJECT_ROOT"
    exit 1
fi

echo -e "${GREEN}✓${NC} Found git repository at: ${CYAN}$PROJECT_ROOT${NC}\n"

# Check for required tools
echo -e "${CYAN}Checking required tools...${NC}"

check_tool() {
    local tool=$1
    local install_cmd=$2
    local required=${3:-true}

    if command -v "$tool" &> /dev/null; then
        echo -e "${GREEN}✓${NC} $tool installed"
        return 0
    else
        if [ "$required" = "true" ]; then
            echo -e "${RED}✗${NC} $tool not found"
            echo -e "  Install with: ${YELLOW}$install_cmd${NC}"
            return 1
        else
            echo -e "${YELLOW}⚠${NC} $tool not installed (optional)"
            echo -e "  Install with: ${YELLOW}$install_cmd${NC}"
            return 0
        fi
    fi
}

MISSING_REQUIRED=0

# Required tools
check_tool "cargo" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" || MISSING_REQUIRED=1

# Optional but recommended tools
check_tool "cargo-audit" "cargo install cargo-audit" "false"
check_tool "python3" "Install Python 3 from python.org" "false"

echo ""

if [ $MISSING_REQUIRED -gt 0 ]; then
    echo -e "${RED}✗ Missing required tools. Please install them and try again.${NC}"
    exit 1
fi

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

# Define the pre-commit hook content
PRE_COMMIT_HOOK="$HOOKS_DIR/pre-commit"

# Check if pre-commit hook already exists
if [ -f "$PRE_COMMIT_HOOK" ]; then
    # Check if it's our hook or a different one
    if grep -q "Pre-commit hook for metarepo" "$PRE_COMMIT_HOOK"; then
        echo -e "${YELLOW}⚠${NC} Pre-commit hook already installed"
        echo -e "  Location: ${CYAN}$PRE_COMMIT_HOOK${NC}"
        echo ""
        read -p "Do you want to reinstall/update it? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${GREEN}✓${NC} Keeping existing hook"
            exit 0
        fi
    else
        echo -e "${YELLOW}⚠${NC} Found existing pre-commit hook (not created by this script)"
        echo -e "  Location: ${CYAN}$PRE_COMMIT_HOOK${NC}"
        echo ""
        read -p "Do you want to backup and replace it? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${YELLOW}Aborted. Your existing hook was not modified.${NC}"
            exit 0
        fi

        # Backup existing hook
        BACKUP="$PRE_COMMIT_HOOK.backup.$(date +%Y%m%d_%H%M%S)"
        mv "$PRE_COMMIT_HOOK" "$BACKUP"
        echo -e "${GREEN}✓${NC} Backed up existing hook to: ${CYAN}$BACKUP${NC}"
    fi
fi

# Create the pre-commit hook
cat > "$PRE_COMMIT_HOOK" << 'EOFHOOK'
#!/usr/bin/env bash
#
# Pre-commit hook for metarepo
# This hook runs quality checks before allowing a commit
#
# To bypass: git commit --no-verify
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Symbols
CHECK="${GREEN}✓${NC}"
CROSS="${RED}✗${NC}"
WARN="${YELLOW}⚠${NC}"
INFO="${BLUE}ℹ${NC}"

echo -e "${CYAN}Running pre-commit checks...${NC}\n"

# Track if any check failed
FAILED=0
FIXED=0

# Function to run a check
run_check() {
    local name="$1"
    local command="$2"
    local auto_fix="${3:-false}"

    echo -ne "${BLUE}▶${NC} ${name}... "

    if eval "$command" > /dev/null 2>&1; then
        echo -e "${CHECK}"
        return 0
    else
        if [ "$auto_fix" = "true" ]; then
            echo -e "${YELLOW}fixed${NC}"
            FIXED=$((FIXED + 1))
            return 0
        else
            echo -e "${CROSS}"
            FAILED=$((FAILED + 1))
            return 1
        fi
    fi
}

# Function to run a check with output on failure
run_check_verbose() {
    local name="$1"
    local command="$2"

    echo -ne "${BLUE}▶${NC} ${name}... "

    local output
    if output=$(eval "$command" 2>&1); then
        echo -e "${CHECK}"
        return 0
    else
        echo -e "${CROSS}"
        echo -e "${RED}Error output:${NC}"
        echo "$output" | head -20
        FAILED=$((FAILED + 1))
        return 1
    fi
}

# ==============================================================================
# Auto-fix Checks
# ==============================================================================

echo -e "${CYAN}Auto-fix checks:${NC}"

# Cargo fmt (auto-fix)
if ! run_check "Formatting Rust code" "cargo fmt --all" "false"; then
    echo -e "  ${INFO} Running: ${YELLOW}cargo fmt --all${NC}"
    cargo fmt --all 2>&1 | grep -v "^Diff in" || true
    git add -u
    FIXED=$((FIXED + 1))
    echo -e "  ${CHECK} Code formatted and staged"
fi

# Trailing whitespace (auto-fix)
echo -ne "${BLUE}▶${NC} Removing trailing whitespace... "
if git diff --cached --name-only --diff-filter=ACM | xargs -I {} find {} -type f 2>/dev/null | while read -r file; do
    if [[ -f "$file" && ! "$file" =~ \.(png|jpg|jpeg|gif|ico|pdf|zip|tar|gz|lock)$ ]]; then
        sed -i '' 's/[[:space:]]*$//' "$file" 2>/dev/null || sed -i 's/[[:space:]]*$//' "$file" 2>/dev/null || true
    fi
done; then
    echo -e "${CHECK}"
else
    echo -e "${YELLOW}checked${NC}"
fi

# End of file fixer (auto-fix)
echo -ne "${BLUE}▶${NC} Ensuring files end with newline... "
if git diff --cached --name-only --diff-filter=ACM | xargs -I {} find {} -type f 2>/dev/null | while read -r file; do
    if [[ -f "$file" && ! "$file" =~ \.(png|jpg|jpeg|gif|ico|pdf|zip|tar|gz|lock)$ ]]; then
        if [ -n "$(tail -c 1 "$file")" ]; then
            echo "" >> "$file"
        fi
    fi
done; then
    echo -e "${CHECK}"
else
    echo -e "${YELLOW}checked${NC}"
fi

# Stage any auto-fixed files
git add -u 2>/dev/null || true

echo ""

# ==============================================================================
# Validation Checks
# ==============================================================================

echo -e "${CYAN}Validation checks:${NC}"

# Cargo clippy
run_check_verbose "Running clippy" "cargo clippy --all-targets --all-features -- -D warnings"

# Check for merge conflict markers
echo -ne "${BLUE}▶${NC} Checking for merge conflicts... "
if git diff --cached | grep -E '^(<<<<<<<|=======|>>>>>>>)' > /dev/null; then
    echo -e "${CROSS}"
    echo -e "  ${RED}Found merge conflict markers in staged files${NC}"
    FAILED=$((FAILED + 1))
else
    echo -e "${CHECK}"
fi

# Check for large files
echo -ne "${BLUE}▶${NC} Checking for large files... "
large_files=$(git diff --cached --name-only --diff-filter=ACM | while read -r file; do
    if [ -f "$file" ]; then
        size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null || echo 0)
        if [ "$size" -gt 1048576 ]; then # 1MB
            echo "$file ($((size / 1024))KB)"
        fi
    fi
done)

if [ -n "$large_files" ]; then
    echo -e "${WARN}"
    echo -e "  ${YELLOW}Warning: Large files detected:${NC}"
    echo "$large_files" | sed 's/^/    /'
    echo -e "  ${INFO} Consider using Git LFS or excluding from repo"
else
    echo -e "${CHECK}"
fi

# Validate YAML files
yaml_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(yml|yaml)$' || true)
if [ -n "$yaml_files" ]; then
    echo -ne "${BLUE}▶${NC} Validating YAML syntax... "
    yaml_valid=true
    for file in $yaml_files; do
        if ! python3 -c "import yaml, sys; yaml.safe_load(open('$file'))" 2>/dev/null; then
            yaml_valid=false
            echo -e "${CROSS}"
            echo -e "  ${RED}Invalid YAML in: $file${NC}"
            FAILED=$((FAILED + 1))
            break
        fi
    done
    if $yaml_valid; then
        echo -e "${CHECK}"
    fi
fi

# Validate TOML files
toml_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.toml$' || true)
if [ -n "$toml_files" ]; then
    echo -ne "${BLUE}▶${NC} Validating TOML syntax... "
    toml_valid=true
    for file in $toml_files; do
        if ! cargo read-manifest --manifest-path "$file" > /dev/null 2>&1 && [ "$(basename "$file")" = "Cargo.toml" ]; then
            toml_valid=false
            echo -e "${CROSS}"
            echo -e "  ${RED}Invalid TOML in: $file${NC}"
            FAILED=$((FAILED + 1))
            break
        fi
    done
    if $toml_valid; then
        echo -e "${CHECK}"
    fi
fi

# Security audit (if cargo-audit is installed)
if command -v cargo-audit &> /dev/null; then
    echo -ne "${BLUE}▶${NC} Running security audit... "
    if cargo audit --deny warnings > /dev/null 2>&1; then
        echo -e "${CHECK}"
    else
        # Check if it's just unmaintained warnings (not critical)
        if cargo audit 2>&1 | grep -q "warning:.*allowed warning"; then
            echo -e "${YELLOW}warnings${NC}"
            echo -e "  ${INFO} Non-critical warnings found (run 'cargo audit' for details)"
        else
            echo -e "${CROSS}"
            echo -e "  ${RED}Security vulnerabilities found!${NC}"
            echo -e "  ${INFO} Run 'cargo audit' for details"
            FAILED=$((FAILED + 1))
        fi
    fi
else
    echo -e "${BLUE}▶${NC} Security audit... ${YELLOW}skipped${NC} (cargo-audit not installed)"
    echo -e "  ${INFO} Install with: ${CYAN}cargo install cargo-audit${NC}"
fi

echo ""

# ==============================================================================
# Summary
# ==============================================================================

if [ $FAILED -gt 0 ]; then
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${RED}✗ Pre-commit checks failed ($FAILED errors)${NC}"
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "${YELLOW}To fix:${NC}"
    echo -e "  1. Review the errors above"
    echo -e "  2. Fix the issues"
    echo -e "  3. Re-stage your changes: ${CYAN}git add .${NC}"
    echo -e "  4. Commit again"
    echo ""
    echo -e "${YELLOW}To bypass (not recommended):${NC}"
    echo -e "  ${CYAN}git commit --no-verify${NC}"
    echo ""
    exit 1
elif [ $FIXED -gt 0 ]; then
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}✓ Pre-commit checks passed${NC} ${YELLOW}($FIXED auto-fixes applied)${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "${INFO} Auto-fixed files have been staged"
    echo ""
else
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}✓ All pre-commit checks passed${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
fi

exit 0
EOFHOOK

# Make the hook executable
chmod +x "$PRE_COMMIT_HOOK"

echo -e "${GREEN}✓${NC} Pre-commit hook installed successfully!"
echo -e "  Location: ${CYAN}$PRE_COMMIT_HOOK${NC}"
echo ""
echo -e "${CYAN}What the hook does:${NC}"
echo -e "  ${GREEN}✓${NC} Auto-fixes: formatting, trailing whitespace, end-of-file"
echo -e "  ${GREEN}✓${NC} Validates: clippy, YAML, TOML, merge conflicts"
echo -e "  ${GREEN}✓${NC} Security: cargo audit (if installed)"
echo ""
echo -e "${CYAN}To bypass the hook:${NC}"
echo -e "  ${YELLOW}git commit --no-verify${NC}"
echo ""
echo -e "${GREEN}✓ Installation complete!${NC}"
