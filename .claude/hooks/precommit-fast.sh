#!/usr/bin/env bash
# Fast pre-commit checks for the metarepo workspace.
#
# Wired as a project-scoped `Stop` hook in .claude/settings.json: it runs when an
# agent finishes a turn, but only if Rust sources changed in the working tree.
# Mirrors the fast half of ci.yml (fmt + clippy + build; the slow test suite is
# left out by design). On failure it returns a `decision: block` so the agent has
# to fix the issues before the turn can end.
set -uo pipefail

cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0

# Only act when Rust-relevant files changed (tracked edits or untracked files).
if ! git status --porcelain 2>/dev/null | grep -qE '\.rs$|Cargo\.(toml|lock)$'; then
    exit 0
fi

fails=""
run() {
    local name="$1"
    shift
    local out
    if ! out=$("$@" 2>&1); then
        fails+="### \`${name}\` failed:"$'\n'"${out}"$'\n\n'
    fi
}

run "cargo fmt --all -- --check" cargo fmt --all -- --check
run "cargo clippy --all-targets --all-features -- -D warnings" \
    cargo clippy --all-targets --all-features -- -D warnings
run "cargo build --all" cargo build --all

if [ -n "$fails" ]; then
    reason="Pre-commit checks failed — fix these before finishing:"$'\n\n'"${fails}"
    if command -v jq >/dev/null 2>&1; then
        jq -n --arg r "$reason" '{decision: "block", reason: $r}'
    else
        # Fallback when jq is unavailable: exit code 2 feeds stderr back to Claude.
        printf '%s\n' "$reason" >&2
        exit 2
    fi
fi

exit 0
