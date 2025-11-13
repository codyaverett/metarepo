#!/usr/bin/env bash
#
# Quick bug report creation script using gh CLI
#
# Usage:
#   Interactive mode:
#     .github/scripts/new-bug.sh
#
#   Programmatic mode (command-line args):
#     .github/scripts/new-bug.sh "Title" "Description" "Steps" "Expected" "Actual"
#
#   Programmatic mode (environment variables):
#     BUG_TITLE="..." BUG_DESC="..." BUG_STEPS="..." BUG_EXPECTED="..." BUG_ACTUAL="..." .github/scripts/new-bug.sh
#
#   Programmatic mode (JSON stdin):
#     echo '{"title":"...","description":"...","steps":"...","expected":"...","actual":"..."}' | .github/scripts/new-bug.sh --json
#
#   Options:
#     --json          Read input from JSON stdin
#     --help, -h      Show this help message
#     --silent        Suppress non-error output (useful for automation)

set -euo pipefail

SILENT=false

# Show help
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    head -n 20 "$0" | tail -n +3 | sed 's/^# //' | sed 's/^#//'
    exit 0
fi

# Check for silent flag
if [[ "${1:-}" == "--silent" ]]; then
    SILENT=true
    shift
fi

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    echo "Error: GitHub CLI (gh) is not installed." >&2
    echo "Install it from: https://cli.github.com/" >&2
    exit 1
fi

# Function to output only if not silent
log() {
    if [[ "$SILENT" == "false" ]]; then
        echo "$@"
    fi
}

# Parse JSON input
if [[ "${1:-}" == "--json" ]]; then
    if ! command -v jq &> /dev/null; then
        echo "Error: jq is required for JSON input mode." >&2
        echo "Install it from: https://stedolan.github.io/jq/" >&2
        exit 1
    fi

    JSON_INPUT=$(cat)
    title=$(echo "$JSON_INPUT" | jq -r '.title // empty')
    description=$(echo "$JSON_INPUT" | jq -r '.description // empty')
    steps=$(echo "$JSON_INPUT" | jq -r '.steps // empty')
    expected=$(echo "$JSON_INPUT" | jq -r '.expected // empty')
    actual=$(echo "$JSON_INPUT" | jq -r '.actual // empty')

    if [[ -z "$title" ]]; then
        echo "Error: 'title' is required in JSON input" >&2
        exit 1
    fi

# Command-line arguments
elif [[ $# -ge 5 ]]; then
    title="$1"
    description="$2"
    steps="$3"
    expected="$4"
    actual="$5"

# Environment variables
elif [[ -n "${BUG_TITLE:-}" ]]; then
    title="${BUG_TITLE}"
    description="${BUG_DESC:-}"
    steps="${BUG_STEPS:-}"
    expected="${BUG_EXPECTED:-}"
    actual="${BUG_ACTUAL:-}"

# Interactive mode
else
    log "=== Create a Bug Report ==="
    log

    read -p "Bug title: " title
    log
    read -p "Brief description: " description
    log
    read -p "Steps to reproduce (one line): " steps
    log
    read -p "Expected behavior: " expected
    log
    read -p "Actual behavior: " actual
    log
fi

# Validate required fields
if [[ -z "$title" ]]; then
    echo "Error: Bug title is required" >&2
    exit 1
fi

# Get metarepo version
version=$(meta --version 2>/dev/null || echo "unknown")

# Create issue body
body=$(cat <<EOF
## Bug Description
${description:-No description provided}

## Steps to Reproduce
${steps:-No steps provided}

## Expected Behavior
${expected:-No expected behavior provided}

## Actual Behavior
${actual:-No actual behavior provided}

## Environment
- metarepo version: $version
- OS: $(uname -s) $(uname -r)
- Rust version: $(rustc --version 2>/dev/null || echo "unknown")
- Shell: ${SHELL:-unknown}
EOF
)

# Create the issue
log
log "Creating bug report..."
ISSUE_URL=$(gh issue create \
    --title "[Bug]: $title" \
    --body "$body" \
    --label "bug,needs-triage" 2>&1 | tee /dev/stderr | grep -o 'https://[^ ]*' || true)

if [[ -n "$ISSUE_URL" ]]; then
    log
    log "Bug report created successfully!"
    # Output URL for programmatic usage
    echo "$ISSUE_URL"
    exit 0
else
    echo "Error: Failed to create issue" >&2
    exit 1
fi
