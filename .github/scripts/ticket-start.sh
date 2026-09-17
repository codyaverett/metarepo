#!/usr/bin/env bash
#
# Start work on a GitHub issue: derive the branch, create the worktree, and
# move the issue to in-progress.
#
# Usage:
#   Start ticket 141:
#     .github/scripts/ticket-start.sh 141
#
#   Preview every step without touching anything:
#     .github/scripts/ticket-start.sh 141 --dry-run
#
#   Pass extra arguments through to meta worktree add:
#     .github/scripts/ticket-start.sh 141 -- --project meta --from origin/main
#
#   The worktree path is printed on stdout, so you can cd straight into it:
#     cd "$(.github/scripts/ticket-start.sh 141 --silent)"
#
#   Options:
#     --dry-run       Print the steps; create nothing, label nothing
#     --silent        Suppress progress output on stderr
#     --self-check    Validate branch-name derivation offline and exit
#     --help, -h      Show this help message
#
# Requires: gh, jq, and meta on PATH. Run it from inside the project the ticket
# touches: meta creates the worktree in the current .meta project.

set -euo pipefail

IN_PROGRESS_LABEL="in-progress"
IN_PROGRESS_COLOR="0e8a16"
IN_PROGRESS_DESC="Someone is actively working this issue"
MAX_SLUG=40

DRY_RUN=false
SILENT=false
SELF_CHECK=false
ISSUE=""
META_ARGS=()

if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    head -n 26 "$0" | tail -n +3 | sed 's/^# //' | sed 's/^#//'
    exit 0
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --silent) SILENT=true; shift ;;
        --self-check) SELF_CHECK=true; shift ;;
        --)
            shift
            META_ARGS=("$@")
            break
            ;;
        -*) echo "Error: unknown option $1" >&2; exit 1 ;;
        *)
            if [[ -n "$ISSUE" ]]; then
                echo "Error: only one issue number may be given" >&2
                exit 1
            fi
            ISSUE="$1"; shift
            ;;
    esac
done

log() {
    if [[ "$SILENT" == "false" ]]; then
        echo "$@" >&2
    fi
}

# slugify <title>
# Lowercase, collapse every run of non-alphanumerics to one hyphen, trim to
# MAX_SLUG on a word boundary, and never end on a hyphen.
slugify() {
    local slug="$1"
    slug=$(printf '%s' "$slug" | LC_ALL=C tr '[:upper:]' '[:lower:]' | LC_ALL=C tr -cs 'a-z0-9' '-')
    slug="${slug#-}"
    slug="${slug%-}"
    if [[ ${#slug} -gt $MAX_SLUG ]]; then
        local cut="${slug:0:$MAX_SLUG}"
        # Drop the word the cut landed inside, unless it ended cleanly.
        [[ "${slug:$MAX_SLUG:1}" == "-" ]] || cut="${cut%-*}"
        slug="$cut"
    fi
    slug="${slug%-}"
    printf '%s' "${slug:-issue}"
}

# branch_for <issue-number> <issue-title>
# "[Bug]: ..." titles become fix/, everything else feature/. The bracketed
# template prefix is dropped before slugging so it never lands in the branch.
branch_for() {
    local number="$1" title="$2" prefix="feature"
    if [[ "$title" =~ ^\[[Bb][Uu][Gg]\] ]]; then
        prefix="fix"
    fi
    title=$(printf '%s' "$title" | sed 's/^\[[^]]*\]:*[[:space:]]*//')
    printf '%s/%s-%s' "$prefix" "$number" "$(slugify "$title")"
}

if [[ "$SELF_CHECK" == "true" ]]; then
    failed=0
    expect() {
        local got want
        got=$(branch_for "$1" "$2")
        want="$3"
        if [[ "$got" == "$want" ]]; then
            echo "ok    $want"
        else
            echo "FAIL  want $want, got $got" >&2
            failed=1
        fi
    }
    expect 141 "[Feature]: Add ticket-start workflow command: issue to worktree in one step" \
        "feature/141-add-ticket-start-workflow-command-issue"
    expect 42 "[Bug]: meta exec hangs on parallel runs" \
        "fix/42-meta-exec-hangs-on-parallel-runs"
    expect 7 "  Weird --- Title!!!  " \
        "feature/7-weird-title"
    expect 8 "UPPER_Case/Mixed 2.0" \
        "feature/8-upper-case-mixed-2-0"
    expect 9 "!!!" \
        "feature/9-issue"
    [[ "$failed" -eq 0 ]] || { echo "self-check failed" >&2; exit 1; }
    echo "self-check ok"
    exit 0
fi

for tool in gh jq; do
    if ! command -v "$tool" &> /dev/null; then
        echo "Error: $tool is required." >&2
        echo "Install gh from https://cli.github.com/ and jq from https://stedolan.github.io/jq/" >&2
        exit 1
    fi
done

if ! command -v meta &> /dev/null; then
    echo "Error: meta is not on PATH." >&2
    echo "Install it from the workspace root with: cargo install --path meta" >&2
    exit 1
fi

if [[ ! "$ISSUE" =~ ^[0-9]+$ ]]; then
    echo "Error: pass an issue number, for example: $0 141 (see --help)." >&2
    exit 1
fi

if ! ISSUE_JSON=$(gh issue view "$ISSUE" --json number,title,body,labels,state 2>&1); then
    echo "Error: could not read issue #$ISSUE." >&2
    echo "$ISSUE_JSON" | sed 's/^/  /' >&2
    exit 1
fi

TITLE=$(echo "$ISSUE_JSON" | jq -r '.title')
STATE=$(echo "$ISSUE_JSON" | jq -r '.state')
HAS_TRIAGE=$(echo "$ISSUE_JSON" | jq -r 'if any(.labels[]; .name == "needs-triage") then "yes" else "no" end')
BRANCH=$(branch_for "$ISSUE" "$TITLE")

log "Issue #$ISSUE ($STATE): $TITLE"
log "Branch: $BRANCH"

if git worktree list --porcelain | grep -qxF "branch refs/heads/$BRANCH"; then
    echo "Error: a worktree for $BRANCH already exists:" >&2
    git worktree list | grep -F "[$BRANCH]" >&2 || true
    exit 1
fi

# meta prompts for a project when it cannot infer one; without a terminal that
# would hang an automated caller, so fail fast instead.
if [[ ! -t 0 ]]; then
    META_ARGS+=(--non-interactive fail)
fi

if [[ "$DRY_RUN" == "true" ]]; then
    log "Dry run, nothing was changed. Would run:"
    log "  meta worktree add $BRANCH ${META_ARGS[*]:-}"
    log "  gh label create $IN_PROGRESS_LABEL --color $IN_PROGRESS_COLOR --description '$IN_PROGRESS_DESC' --force"
    log "  gh issue edit $ISSUE --add-label $IN_PROGRESS_LABEL$([[ "$HAS_TRIAGE" == "yes" ]] && echo " --remove-label needs-triage") --add-assignee @me"
    echo "$BRANCH"
    exit 0
fi

log "Creating worktree..."
meta worktree add "$BRANCH" ${META_ARGS[@]+"${META_ARGS[@]}"} >&2

WORKTREE_PATH=$(git worktree list --porcelain | awk -v b="branch refs/heads/$BRANCH" '
    /^worktree / { path = substr($0, 10) }
    $0 == b { print path; exit }')

if [[ -z "$WORKTREE_PATH" ]]; then
    echo "Error: no worktree for $BRANCH in this repository after meta worktree add." >&2
    echo "meta creates worktrees inside .meta projects that are git repositories of" >&2
    echo "their own, under <project>/.worktrees/<branch>. Run this script from inside" >&2
    echo "the project the ticket touches, or name it with: $0 $ISSUE -- --project NAME" >&2
    echo "and check 'meta worktree list --workspace'. Issue #$ISSUE was left untouched." >&2
    exit 1
fi

# The label does not ship with the repo; create it on first use rather than
# letting gh reject an unknown label.
gh label create "$IN_PROGRESS_LABEL" \
    --color "$IN_PROGRESS_COLOR" \
    --description "$IN_PROGRESS_DESC" \
    --force > /dev/null

EDIT_ARGS=(--add-label "$IN_PROGRESS_LABEL" --add-assignee @me)
if [[ "$HAS_TRIAGE" == "yes" ]]; then
    EDIT_ARGS+=(--remove-label needs-triage)
fi

# The worktree already exists at this point; a labelling failure is worth a
# warning but must not hide the path the caller asked for.
if gh issue edit "$ISSUE" "${EDIT_ARGS[@]}" > /dev/null; then
    log "Issue #$ISSUE labelled $IN_PROGRESS_LABEL and assigned to you."
else
    echo "Warning: could not update labels or assignee on #$ISSUE; the worktree is ready." >&2
fi

echo "$WORKTREE_PATH"
