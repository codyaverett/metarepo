#!/usr/bin/env bash
#
# Ultra-fast idea capture script using gh CLI
#
# Usage:
#   Interactive mode:
#     .github/scripts/new-idea.sh
#
#   Programmatic mode (command-line args):
#     .github/scripts/new-idea.sh "Idea title"
#     .github/scripts/new-idea.sh "Idea title" "Optional notes"
#
#   Programmatic mode (environment variables):
#     IDEA_TITLE="..." IDEA_NOTES="..." .github/scripts/new-idea.sh
#
#   Programmatic mode (JSON stdin):
#     echo '{"title":"...","notes":"..."}' | .github/scripts/new-idea.sh --json
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
    notes=$(echo "$JSON_INPUT" | jq -r '.notes // empty')

    if [[ -z "$title" ]]; then
        echo "Error: 'title' is required in JSON input" >&2
        exit 1
    fi

# Command-line arguments
elif [[ $# -ge 1 ]]; then
    title="$1"
    notes="${2:-}"

# Environment variables
elif [[ -n "${IDEA_TITLE:-}" ]]; then
    title="${IDEA_TITLE}"
    notes="${IDEA_NOTES:-}"

# Interactive mode
else
    read -p "Idea: " title
    read -p "Notes (optional): " notes
fi

# Validate required fields
if [[ -z "$title" ]]; then
    echo "Error: Idea title is required" >&2
    exit 1
fi

# Create issue body
if [[ -n "$notes" ]]; then
    body="$notes"
else
    body="Quick idea capture - expand details later."
fi

# Create the issue
log "Creating idea..."
ISSUE_URL=$(gh issue create \
    --title "$title" \
    --body "$body" \
    --label "idea" 2>&1 | tee /dev/stderr | grep -o 'https://[^ ]*' || true)

if [[ -n "$ISSUE_URL" ]]; then
    log "Idea captured successfully!"
    # Output URL for programmatic usage
    echo "$ISSUE_URL"
    exit 0
else
    echo "Error: Failed to create issue" >&2
    exit 1
fi
