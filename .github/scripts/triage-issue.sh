#!/usr/bin/env bash
#
# Triage open issues with TypeSafe (Jev): suggest priority, area, kind, and
# duplicates, then apply the labels that clear the confidence threshold.
#
# Usage:
#   Triage one issue:
#     .github/scripts/triage-issue.sh 145
#
#   Triage every open issue labelled needs-triage:
#     .github/scripts/triage-issue.sh --all
#
#   Preview without touching the issue:
#     .github/scripts/triage-issue.sh 145 --dry-run
#
#   Options:
#     --all           Triage all open needs-triage issues
#     --dry-run       Print the judgments; do not label or comment
#     --threshold N   Confidence needed to auto-apply (default 0.7)
#     --raw           Emit the raw judgments as JSON instead of prose
#     --self-check    Validate the request payload builder and exit
#     --help, -h      Show this help message
#
# Requires: gh, jq, curl, and TYPESAFE_API_KEY in the environment.

set -euo pipefail

TYPESAFE_URL="${TYPESAFE_BASE_URL:-https://api.typesafe.ai}/v1/systemone"
TYPESAFE_MODEL="${TYPESAFE_DEFAULT_MODEL:-jev-latest}"
THRESHOLD="${TRIAGE_THRESHOLD:-0.7}"
DRY_RUN=false
ALL=false
JSON_OUT=false
SELF_CHECK=false
ISSUE=""

if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    head -n 24 "$0" | tail -n +3 | sed 's/^# //' | sed 's/^#//'
    exit 0
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all) ALL=true ;;
        --dry-run) DRY_RUN=true ;;
        --raw) JSON_OUT=true ;;
        --self-check) SELF_CHECK=true ;;
        --threshold)
            if [[ -z "${2:-}" ]]; then
                echo "Error: --threshold needs a value." >&2
                exit 1
            fi
            THRESHOLD="$2"; shift ;;
        -*) echo "Error: unknown option $1" >&2; exit 1 ;;
        *) ISSUE="$1" ;;
    esac
    shift
done

for tool in jq curl; do
    if ! command -v "$tool" &> /dev/null; then
        echo "Error: $tool is required." >&2
        exit 1
    fi
done

# Labels this script is allowed to apply. Area is reported in the comment only,
# because the repo has no area labels and gh rejects unknown ones.
PRIORITY_LABELS="P0 P1 P2 P3"
KIND_LABELS="bug enhancement documentation question"

# build_payload <title> <body> <candidates-json>
# candidates-json: [{"number":N,"title":"..."}] for duplicate detection.
build_payload() {
    local title="$1" body="$2" candidates="$3"

    # One choice per candidate issue plus "none" - a single selection is cheaper
    # than a yes/no per pair and cannot return two conflicting duplicates.
    local dup_criteria
    dup_criteria=$(echo "$candidates" | jq '
        reduce .[] as $c ({ none: "This issue describes work no other open issue covers" };
            . + { ("issue_\($c.number)"): "Same underlying work as: \($c.title)" })')

    jq -n \
        --arg model "$TYPESAFE_MODEL" \
        --arg title "$title" \
        --arg body "$body" \
        --argjson candidates "$candidates" \
        --argjson dup "$dup_criteria" '
    {
      model: $model,
      state: {
        issue: { title: $title, body: $body },
        other_open_issues: $candidates
      },
      questions: {
        priority: {
          type: "choice",
          instructions: "How urgent is this issue for metarepo, a Rust multi-repo workspace CLI?",
          criteria: {
            P0: "Breaks the build, a release, or loses user data. Must be done now.",
            P1: "Should land in the next release; blocks users or other work.",
            P2: "Wanted and worth doing, but nothing is blocked on it.",
            P3: "Icebox. Nice to have, no timeline."
          }
        },
        area: {
          type: "choice",
          instructions: "Which part of the metarepo codebase does this issue mainly touch?",
          criteria: {
            git: "Fleet git operations: clone, pull, push, fetch, checkout, status",
            exec: "Running shell commands or scripts across projects",
            worktree: "Bare-first worktree creation and management",
            plugins: "Plugin loading, the plugin SDK, or built-in plugin behavior",
            mcp: "The MCP gateway plugin and its transports",
            rules: "The structure rules plugin and its validators",
            config: "Config file discovery, formats, and the config cascade",
            release: "Packaging, versioning, publishing, supply chain, CI release flow",
            docs: "Documentation, README, or examples only",
            other: "Fits none of the above"
          }
        },
        kind: {
          type: "choice",
          instructions: "Is this reporting a defect in existing behavior or requesting new work?",
          criteria: {
            bug: "Existing behavior is wrong, crashes, or contradicts its documentation",
            enhancement: "Asks for a capability or improvement that does not exist yet",
            documentation: "Only asks for docs to be written, corrected, or clarified",
            question: "Asks for information rather than a change to the project"
          }
        },
        duplicate: {
          type: "choice",
          instructions: "Does one of the other open issues already cover this same work? Choose none unless the overlap is substantial.",
          criteria: $dup
        }
      }
    }'
}

if [[ "$SELF_CHECK" == "true" ]]; then
    payload=$(build_payload "Test title" "Test body" '[{"number":7,"title":"Other work"}]')
    echo "$payload" | jq -e '
        (.questions | length) == 4
        and (.questions | to_entries | all(.value.type == "choice"))
        and (.questions.duplicate.criteria | has("none") and has("issue_7"))
        and (.state.issue.title == "Test title")
    ' > /dev/null
    echo "self-check ok"
    exit 0
fi

if ! command -v gh &> /dev/null; then
    echo "Error: GitHub CLI (gh) is not installed." >&2
    exit 1
fi

if [[ -z "${TYPESAFE_API_KEY:-}" ]]; then
    echo "Error: TYPESAFE_API_KEY is not set." >&2
    exit 1
fi

# triage_one <issue-number>
triage_one() {
    local number="$1"
    local issue title body candidates payload response

    issue=$(gh issue view "$number" --json number,title,body)
    title=$(echo "$issue" | jq -r '.title')
    body=$(echo "$issue" | jq -r '.body // "" | .[0:4000]')

    # Candidates for duplicate detection: other open issues, this one excluded.
    candidates=$(gh issue list --state open --limit 50 --json number,title \
        | jq --argjson n "$number" '[ .[] | select(.number != $n) ]')

    payload=$(build_payload "$title" "$body" "$candidates")

    response=$(curl -sS --fail-with-body -X POST "$TYPESAFE_URL" \
        -H "Authorization: Bearer $TYPESAFE_API_KEY" \
        -H "Content-Type: application/json" \
        -d "$payload")

    local answers priority priority_conf area area_conf kind kind_conf dup dup_conf
    answers=$(echo "$response" | jq '.answers')
    priority=$(echo "$answers" | jq -r '.priority.choice')
    priority_conf=$(echo "$answers" | jq -r '.priority.confidence // 0')
    area=$(echo "$answers" | jq -r '.area.choice')
    area_conf=$(echo "$answers" | jq -r '.area.confidence // 0')
    kind=$(echo "$answers" | jq -r '.kind.choice')
    kind_conf=$(echo "$answers" | jq -r '.kind.confidence // 0')
    dup=$(echo "$answers" | jq -r '.duplicate.choice')
    dup_conf=$(echo "$answers" | jq -r '.duplicate.confidence // 0')

    if [[ "$JSON_OUT" == "true" ]]; then
        echo "$answers" | jq --argjson n "$number" '{ issue: $n, answers: . }'
    else
        printf '#%s %s\n' "$number" "$title"
        printf '  priority: %-4s (%.2f)\n' "$priority" "$priority_conf"
        printf '  area:     %-13s (%.2f)\n' "$area" "$area_conf"
        printf '  kind:     %-13s (%.2f)\n' "$kind" "$kind_conf"
        printf '  duplicate:%-13s (%.2f)\n' "$dup" "$dup_conf"
    fi

    [[ "$DRY_RUN" == "true" ]] && return 0

    # Apply only the judgments that clear the threshold and name a real label.
    local add=()
    if awk "BEGIN{exit !($priority_conf >= $THRESHOLD)}" \
        && [[ " $PRIORITY_LABELS " == *" $priority "* ]]; then
        add+=("$priority")
    fi
    if awk "BEGIN{exit !($kind_conf >= $THRESHOLD)}" \
        && [[ " $KIND_LABELS " == *" $kind "* ]]; then
        add+=("$kind")
    fi

    local comment="Automated triage (Jev, confidence in parentheses):

- priority: ${priority} (${priority_conf})
- area: ${area} (${area_conf})
- kind: ${kind} (${kind_conf})
- duplicate: ${dup} (${dup_conf})"

    if [[ "$dup" != "none" ]] && awk "BEGIN{exit !($dup_conf >= $THRESHOLD)}"; then
        comment+="

Possible duplicate of #${dup#issue_} - confirm before closing either one."
    fi

    if [[ ${#add[@]} -gt 0 ]]; then
        # Both judgments cleared the bar, so the issue no longer needs a human
        # to sort it; anything below threshold keeps needs-triage for review.
        local args=()
        for label in "${add[@]}"; do
            args+=(--add-label "$label")
        done
        if [[ ${#add[@]} -eq 2 ]]; then
            args+=(--remove-label needs-triage)
        fi
        gh issue edit "$number" "${args[@]}" > /dev/null
        comment+="

Applied: ${add[*]}"
    else
        comment+="

Nothing cleared the ${THRESHOLD} confidence threshold; left for manual triage."
    fi

    gh issue comment "$number" --body "$comment" > /dev/null
}

if [[ "$ALL" == "true" ]]; then
    numbers=$(gh issue list --state open --label needs-triage --limit 100 --json number -q '.[].number')
    if [[ -z "$numbers" ]]; then
        echo "No open issues labelled needs-triage."
        exit 0
    fi
    for n in $numbers; do
        triage_one "$n"
    done
elif [[ -n "$ISSUE" ]]; then
    triage_one "$ISSUE"
else
    echo "Error: pass an issue number or --all (see --help)." >&2
    exit 1
fi
