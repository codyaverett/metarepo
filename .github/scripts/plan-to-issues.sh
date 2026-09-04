#!/usr/bin/env bash
#
# Decompose a plan doc in docs/plans/ into linked GitHub issues.
#
# Usage:
#   .github/scripts/plan-to-issues.sh docs/plans/PLAN_X.md [options]
#
#   Reads tasks from the doc's "## Tasks" section (see below), or from a JSON
#   array given with --json. Creates one issue per task through
#   new-feature.sh, comments "Blocked by #N" on dependent issues, writes the
#   created issue number back onto each task line, and appends the numbers to
#   the doc's "Related issues:" header line. Prints created issue URLs on
#   stdout, one per line; everything else goes to stderr.
#
#   Options:
#     --json FILE     Read tasks from a JSON array instead of the Tasks section
#                     (FILE may be - for stdin). Each element: {"title", "summary",
#                     "problem", "solution", "priority", "blocked_by": [1-based
#                     indexes into the array]}. Only title is required.
#     --dry-run       Parse and print the plan; create nothing, edit nothing.
#     --no-doc-edit   Create issues but leave the plan doc untouched.
#     --silent        Suppress progress output on stderr.
#     --help, -h      Show this help message.
#
#   Tasks section convention (Markdown):
#
#     ## Tasks
#
#     1. Short imperative title
#        Free-text body lines, indented, become the issue body.
#        blocked by: 2
#        priority: high
#     2. Another title (#57)
#
#   Numbered items start a task. Indented continuation lines are its body.
#   "blocked by: N, M" marks dependencies on other items by number.
#   "priority: low|medium|high|critical" sets the issue priority.
#   A title ending in "(#N)" already has an issue and is skipped, which makes
#   re-running the script safe.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLAN=""
JSON_SRC=""
DRY_RUN=false
DOC_EDIT=true
SILENT=false

if [[ $# -eq 0 ]]; then
    echo "Usage: $0 docs/plans/PLAN_X.md [--json FILE] [--dry-run] [--no-doc-edit] [--silent]" >&2
    exit 1
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h)
            head -n 40 "$0" | tail -n +3 | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        --json)
            JSON_SRC="${2:-}"
            if [[ -z "$JSON_SRC" ]]; then
                echo "Error: --json requires a file path or -" >&2
                exit 1
            fi
            shift 2
            ;;
        --dry-run) DRY_RUN=true; shift ;;
        --no-doc-edit) DOC_EDIT=false; shift ;;
        --silent) SILENT=true; shift ;;
        -*)
            echo "Error: unknown option $1" >&2
            exit 1
            ;;
        *)
            if [[ -n "$PLAN" ]]; then
                echo "Error: only one plan doc may be given" >&2
                exit 1
            fi
            PLAN="$1"; shift
            ;;
    esac
done

if [[ -z "$PLAN" || ! -f "$PLAN" ]]; then
    echo "Error: plan doc not found: ${PLAN:-<none>}" >&2
    exit 1
fi

for tool in gh jq; do
    if ! command -v "$tool" &> /dev/null; then
        echo "Error: $tool is required." >&2
        exit 1
    fi
done

log() {
    if [[ "$SILENT" == "false" ]]; then
        echo "$@" >&2
    fi
}

PLAN_TITLE=$(grep -m1 '^# ' "$PLAN" | sed -e 's/^# *//' -e 's/^Plan: *//' || true)
PLAN_TITLE="${PLAN_TITLE:-$(basename "$PLAN")}"

# ---------------------------------------------------------------------------
# Collect tasks as a JSON array:
#   [{"index": 1, "title": "...", "summary": "...", "problem": "...",
#     "solution": "...", "priority": "medium", "blocked_by": [2],
#     "existing": null | 57, "line": <1-based line number in doc or 0>}]
# ---------------------------------------------------------------------------
if [[ -n "$JSON_SRC" ]]; then
    if [[ "$JSON_SRC" == "-" ]]; then
        RAW=$(cat)
    else
        RAW=$(cat "$JSON_SRC")
    fi
    TASKS=$(echo "$RAW" | jq -c --arg plan "$PLAN" --arg ptitle "$PLAN_TITLE" '
        if type != "array" then error("--json input must be a JSON array") else . end
        | to_entries | map(
            .value as $t | (.key + 1) as $i
            | if ($t.title // "") == "" then error("task \($i): title is required") else . end
            | {
                index: $i,
                title: $t.title,
                summary: ($t.summary // $t.title),
                problem: ($t.problem // "Part of the plan \($ptitle) (\($plan))."),
                solution: ($t.solution // ""),
                priority: ($t.priority // "medium"),
                blocked_by: ($t.blocked_by // []),
                existing: null,
                line: 0
              })')
else
    # Parse the "## Tasks" section. awk emits one record per task with
    # unit-separator (0x1f) delimited fields; body lines are joined with 0x1e.
    RECORDS=$(awk '
        BEGIN { in_tasks = 0; n = 0 }
        /^## / {
            if (in_tasks) { flush(); in_tasks = 0 }
            if ($0 ~ /^## Tasks[[:space:]]*$/) in_tasks = 1
            next
        }
        !in_tasks { next }
        /^[0-9]+\.[[:space:]]+/ {
            flush()
            n++
            line = NR
            title = $0
            sub(/^[0-9]+\.[[:space:]]+/, "", title)
            body = ""
            next
        }
        n > 0 && /^[[:space:]]+[^[:space:]]/ {
            t = $0
            sub(/^[[:space:]]+/, "", t)
            body = (body == "" ? t : body "\036" t)
            next
        }
        END { if (in_tasks) flush() }
        function flush() {
            if (n > 0 && title != "") {
                printf "%d\037%d\037%s\037%s\n", n, line, title, body
            }
            title = ""
        }
    ' "$PLAN")

    if [[ -z "$RECORDS" ]]; then
        echo "Error: no '## Tasks' section with numbered items found in $PLAN (or pass --json)" >&2
        exit 1
    fi

    TASKS="[]"
    while IFS=$'\x1f' read -r idx line title body; do
        [[ -z "$idx" ]] && continue
        existing="null"
        if [[ "$title" =~ [[:space:]]*\(#([0-9]+)\)[[:space:]]*$ ]]; then
            existing="${BASH_REMATCH[1]}"
            title="${title%(*}"
            title="${title%"${title##*[![:space:]]}"}"
        fi
        priority="medium"
        blocked=""
        summary=""
        solution_lines=()
        IFS=$'\x1e' read -r -a body_lines <<< "$body"
        for bl in "${body_lines[@]:-}"; do
            [[ -z "$bl" ]] && continue
            lower=$(echo "$bl" | tr '[:upper:]' '[:lower:]')
            if [[ "$lower" =~ ^blocked[[:space:]]+by:[[:space:]]*(.*)$ ]]; then
                blocked="${BASH_REMATCH[1]}"
            elif [[ "$lower" =~ ^priority:[[:space:]]*([a-z]+) ]]; then
                priority="${BASH_REMATCH[1]}"
            elif [[ -z "$summary" ]]; then
                summary="$bl"
            else
                solution_lines+=("$bl")
            fi
        done
        solution=$(printf '%s\n' "${solution_lines[@]:-}" | sed '/^$/d')
        blocked_json=$(printf '%s' "$blocked" | tr -c '0-9' '\n' | grep -E '^[0-9]+$' | jq -sc 'map(tonumber)') || true
        [[ -z "$blocked_json" ]] && blocked_json='[]'
        TASKS=$(echo "$TASKS" | jq -c \
            --argjson idx "$idx" --argjson line "$line" --arg title "$title" \
            --arg summary "${summary:-$title}" \
            --arg problem "Part of the plan $PLAN_TITLE ($PLAN)." \
            --arg solution "$solution" --arg priority "$priority" \
            --argjson blocked "$blocked_json" --argjson existing "$existing" \
            '. + [{index: $idx, line: $line, title: $title, summary: $summary, problem: $problem,
                   solution: $solution, priority: $priority, blocked_by: $blocked, existing: $existing}]')
    done <<< "$RECORDS"
fi

COUNT=$(echo "$TASKS" | jq 'length')
PENDING=$(echo "$TASKS" | jq '[.[] | select(.existing == null)] | length')

# Validate blocked_by references.
BAD=$(echo "$TASKS" | jq -r --argjson n "$COUNT" '.[] | .index as $i | .blocked_by[] | select(. < 1 or . > $n or . == $i) | "task \($i) blocked by invalid task \(.)"')
if [[ -n "$BAD" ]]; then
    echo "Error: $BAD" >&2
    exit 1
fi

log "Plan: $PLAN_TITLE ($PLAN)"
log "Tasks: $COUNT total, $PENDING to create"
echo "$TASKS" | jq -r '.[] | "  \(.index). \(.title)\(if .existing then " (#\(.existing), skip)" else "" end)\(if (.blocked_by|length) > 0 then "  [blocked by \(.blocked_by|map(tostring)|join(", "))]" else "" end)  priority=\(.priority)"' >&2

if [[ "$DRY_RUN" == "true" ]]; then
    log "Dry run: nothing created."
    exit 0
fi
if [[ "$PENDING" -eq 0 ]]; then
    log "Nothing to do."
    exit 0
fi

# ---------------------------------------------------------------------------
# Create issues.
# ---------------------------------------------------------------------------
NUMBERS="{}"   # index -> issue number
URLS=()
for i in $(seq 1 "$COUNT"); do
    task=$(echo "$TASKS" | jq -c --argjson i "$i" '.[] | select(.index == $i)')
    existing=$(echo "$task" | jq -r '.existing // empty')
    if [[ -n "$existing" ]]; then
        NUMBERS=$(echo "$NUMBERS" | jq -c --argjson i "$i" --argjson n "$existing" '. + {($i|tostring): $n}')
        continue
    fi
    title=$(echo "$task" | jq -r '.title')
    log "Creating issue for task $i: $title"
    url=$(echo "$task" | jq -c '{title, summary, problem, solution, priority}' \
        | "$SCRIPT_DIR/new-feature.sh" --silent --json | tail -n 1)
    if [[ ! "$url" =~ /issues/([0-9]+)$ ]]; then
        echo "Error: could not create issue for task $i (got: $url)" >&2
        exit 1
    fi
    num="${BASH_REMATCH[1]}"
    NUMBERS=$(echo "$NUMBERS" | jq -c --argjson i "$i" --argjson n "$num" '. + {($i|tostring): $n}')
    URLS+=("$url")

    # Write the number back onto the task line immediately so a failure later
    # in the run does not leave the doc and GitHub out of sync.
    line=$(echo "$task" | jq -r '.line')
    if [[ "$DOC_EDIT" == "true" && "$line" -gt 0 ]]; then
        awk -v ln="$line" -v n="$num" 'NR == ln { sub(/[[:space:]]*$/, ""); $0 = $0 " (#" n ")" } { print }' "$PLAN" > "$PLAN.tmp" && mv "$PLAN.tmp" "$PLAN"
    fi
done

# ---------------------------------------------------------------------------
# Cross-link dependencies.
# ---------------------------------------------------------------------------
echo "$TASKS" | jq -r '.[] | select(.existing == null) | select((.blocked_by|length) > 0) | "\(.index) \(.blocked_by|map(tostring)|join(" "))"' \
| while read -r idx deps; do
    num=$(echo "$NUMBERS" | jq -r --arg i "$idx" '.[$i]')
    refs=""
    for d in $deps; do
        dn=$(echo "$NUMBERS" | jq -r --arg i "$d" '.[$i] // empty')
        [[ -n "$dn" ]] && refs="$refs #$dn"
    done
    [[ -z "$refs" ]] && continue
    log "Linking #$num blocked by${refs}"
    gh issue comment "$num" --body "Blocked by${refs}" > /dev/null
done

# ---------------------------------------------------------------------------
# Append to the Related issues header line.
# ---------------------------------------------------------------------------
if [[ "$DOC_EDIT" == "true" ]]; then
    additions=$(echo "$TASKS" | jq -r --argjson nums "$NUMBERS" '
        [.[] | select(.existing == null) | "#\($nums[.index|tostring]) (\(.title))"] | join(", ")')
    if [[ -n "$additions" ]]; then
        if grep -q '^Related issues:' "$PLAN"; then
            awk -v add="$additions" '
                !done && /^Related issues:/ {
                    sub(/[[:space:]]*$/, "")
                    if ($0 ~ /^Related issues:[[:space:]]*(none)?$/) $0 = "Related issues: " add
                    else $0 = $0 ", " add
                    done = 1
                }
                { print }' "$PLAN" > "$PLAN.tmp" && mv "$PLAN.tmp" "$PLAN"
        elif grep -q '^Status:' "$PLAN"; then
            awk -v add="$additions" '{ print } !done && /^Status:/ { print "Related issues: " add; done = 1 }' "$PLAN" > "$PLAN.tmp" && mv "$PLAN.tmp" "$PLAN"
        else
            awk -v add="$additions" 'NR == 1 { print; print ""; print "Related issues: " add; next } { print }' "$PLAN" > "$PLAN.tmp" && mv "$PLAN.tmp" "$PLAN"
        fi
        log "Updated Related issues line in $PLAN"
    fi
fi

printf '%s\n' "${URLS[@]}"
