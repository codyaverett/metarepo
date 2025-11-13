#!/usr/bin/env bash
#
# Quick feature request creation script using gh CLI
#
# Usage:
#   Interactive mode:
#     .github/scripts/new-feature.sh
#
#   Programmatic mode (command-line args):
#     .github/scripts/new-feature.sh "Title" "Summary" "Problem" "Solution" "Priority"
#
#   Programmatic mode (environment variables):
#     FEATURE_TITLE="..." FEATURE_SUMMARY="..." FEATURE_PROBLEM="..." FEATURE_SOLUTION="..." FEATURE_PRIORITY="..." .github/scripts/new-feature.sh
#
#   Programmatic mode (JSON stdin):
#     echo '{"title":"...","summary":"...","problem":"...","solution":"...","priority":"..."}' | .github/scripts/new-feature.sh --json
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
    summary=$(echo "$JSON_INPUT" | jq -r '.summary // empty')
    problem=$(echo "$JSON_INPUT" | jq -r '.problem // empty')
    solution=$(echo "$JSON_INPUT" | jq -r '.solution // empty')
    priority=$(echo "$JSON_INPUT" | jq -r '.priority // "medium"')

    if [[ -z "$title" ]]; then
        echo "Error: 'title' is required in JSON input" >&2
        exit 1
    fi

# Command-line arguments
elif [[ $# -ge 4 ]]; then
    title="$1"
    summary="$2"
    problem="$3"
    solution="$4"
    priority="${5:-medium}"

# Environment variables
elif [[ -n "${FEATURE_TITLE:-}" ]]; then
    title="${FEATURE_TITLE}"
    summary="${FEATURE_SUMMARY:-}"
    problem="${FEATURE_PROBLEM:-}"
    solution="${FEATURE_SOLUTION:-}"
    priority="${FEATURE_PRIORITY:-medium}"

# Interactive mode
else
    log "=== Create a Feature Request ==="
    log

    read -p "Feature title: " title
    log
    read -p "Brief summary: " summary
    log
    read -p "What problem does this solve? " problem
    log
    read -p "Proposed solution (brief): " solution
    log
    read -p "Priority (low/medium/high/critical): " priority
    log
fi

# Validate required fields
if [[ -z "$title" ]]; then
    echo "Error: Feature title is required" >&2
    exit 1
fi

# Default priority if not set
priority="${priority:-medium}"

# Create issue body
body=$(cat <<EOF
## Feature Summary
${summary:-No summary provided}

## Problem Statement
${problem:-No problem statement provided}

## Proposed Solution
${solution:-No solution provided}

## Priority
$priority

---
_Note: For detailed feature proposals, please use the full template at github.com/issues/new/choose_
EOF
)

# Create the issue
log
log "Creating feature request..."
ISSUE_URL=$(gh issue create \
    --title "[Feature]: $title" \
    --body "$body" \
    --label "enhancement,needs-triage" 2>&1 | tee /dev/stderr | grep -o 'https://[^ ]*' || true)

if [[ -n "$ISSUE_URL" ]]; then
    log
    log "Feature request created successfully!"
    # Output URL for programmatic usage
    echo "$ISSUE_URL"
    exit 0
else
    echo "Error: Failed to create issue" >&2
    exit 1
fi
