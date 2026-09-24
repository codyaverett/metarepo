#!/usr/bin/env bash
#
# Gather what needs attention on open PRs: red CI (with failing log tails),
# unanswered review comments, and reviews stalled past a threshold. Read-only
# against GitHub; the babysit-prs skill decides and acts on the result.
#
# Usage:
#   Human summary of the current repo:
#     .github/scripts/babysit-prs.sh
#
#   Machine-readable report (what the skill consumes):
#     .github/scripts/babysit-prs.sh --json
#
#   Mark items handled so the next pass skips them:
#     .github/scripts/babysit-prs.sh --ack ci:142:abc123 thread:98765
#
#   Options:
#     --json            Emit the report as JSON on stdout
#     --all             Include items already acked (flagged handled: true)
#     --stale-hours N   Review wait that counts as stalled (default 24)
#     --log-lines N     Failing log lines kept per check (default 40)
#     --ack KEY...      Record keys as handled in the state file and exit
#     --dry-run         Never write the state file (acks are only printed)
#     --silent          Suppress progress output on stderr
#     --self-check      Run a fixture through the report builder and exit
#     --help, -h        Show this help message
#
# Requires: gh and jq. Target another repo with GH_REPO=owner/name.

set -euo pipefail

STALE_HOURS="${BABYSIT_STALE_HOURS:-24}"
LOG_LINES="${BABYSIT_LOG_LINES:-40}"
JSON_OUT=false
ALL=false
DRY_RUN=false
SILENT=false
SELF_CHECK=false
ACK=false
ACK_KEYS=()

if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    head -n 29 "$0" | tail -n +3 | sed 's/^# //' | sed 's/^#//'
    exit 0
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --json) JSON_OUT=true ;;
        --all) ALL=true ;;
        --dry-run) DRY_RUN=true ;;
        --silent) SILENT=true ;;
        --self-check) SELF_CHECK=true ;;
        --ack) ACK=true ;;
        --stale-hours|--log-lines)
            if [[ ! "${2:-}" =~ ^[0-9]+$ ]]; then
                echo "Error: $1 needs a whole number." >&2
                exit 1
            fi
            if [[ "$1" == "--stale-hours" ]]; then STALE_HOURS="$2"; else LOG_LINES="$2"; fi
            shift ;;
        -*) echo "Error: unknown option $1" >&2; exit 1 ;;
        *)
            if [[ "$ACK" != "true" ]]; then
                echo "Error: unexpected argument $1 (keys go after --ack)." >&2
                exit 1
            fi
            ACK_KEYS+=("$1") ;;
    esac
    shift
done

if ! command -v jq &> /dev/null; then
    echo "Error: jq is required." >&2
    exit 1
fi

log() {
    if [[ "$SILENT" == "false" ]]; then
        echo "$@" >&2
    fi
}

# build_report <graphql-response> <handled-keys-json-array> <now-epoch>
# Pure jq: the live run and --self-check both go through here.
#
# Item keys, one per thing the skill acts on:
#   ci:<pr>:<head-sha>        all failing checks at that commit; a new push is a new key
#   thread:<comment-id>       an unresolved review thread, keyed on its latest comment
#   review:<review-id>        a review body (changes requested or commented)
#   stalled:<pr>:<yyyy-mm-dd> one stalled-review nudge per PR per day
build_report() {
    jq --argjson handled "$2" --argjson now "$3" --argjson stale "$STALE_HOURS" \
        --argjson all "$ALL" '
    def hours_since($t): (($now - ($t | fromdateiso8601)) / 3600 | floor);
    def mark: . + { handled: (.key as $k | $handled | index($k) != null) };
    def keep: map(mark) | if $all then . else map(select(.handled | not)) end;

    .data.viewer.login as $me
    | [ .data.repository.pullRequests.nodes[] | . as $pr
        | ([ .commits.nodes[0].commit.statusCheckRollup.contexts.nodes[]? ]) as $ctx
        | {
            number, title, url, branch: .headRefName, head_sha: .headRefOid,
            draft: .isDraft, author: .author.login,
            author_is_bot: (.author.__typename == "Bot"),
            cross_repo: .isCrossRepository, maintainer_can_modify: .maintainerCanModify,
            review_decision: .reviewDecision,
            requested_reviewers: [ .reviewRequests.nodes[].requestedReviewer | (.login // .name) ],
            latest_review_state: ([ .latestReviews.nodes[] ] | sort_by(.submittedAt) | last | .state?),
            # ponytail: waits are measured from creation or the latest review, not from
            # the review request event; switch to timelineItems if that proves too coarse.
            waiting_since: ([ .createdAt, (.latestReviews.nodes[].submittedAt) ] | max),
            failing: [ $ctx[]
                | select((.conclusion // .state) as $c
                    | ["FAILURE","TIMED_OUT","STARTUP_FAILURE","ACTION_REQUIRED","ERROR"] | index($c))
                | if .__typename == "CheckRun" then
                    { name, url: .detailsUrl, job_id: .databaseId,
                      run_id: ((.detailsUrl // "") | [capture("/runs/(?<r>[0-9]+)").r | tonumber] | first) }
                  else
                    { name: .context, url: .targetUrl, job_id: null, run_id: null }
                  end
                # Hook for #162: log_tail is the input a CI-failure classifier reads.
                | . + { log_tail: null } ],
            check_count: ($ctx | length),
            pending: any($ctx[]; (.status // .state) as $s
                | ["QUEUED","IN_PROGRESS","PENDING","WAITING","REQUESTED","EXPECTED"] | index($s)),
            threads: [ .reviewThreads.nodes[]
                | select(.isResolved | not)
                | (.comments.nodes | last) as $last
                | select($last.author.login != $me)
                | { key: "thread:\($last.databaseId)", pr: $pr.number, kind: "thread",
                    path, line, outdated: .isOutdated,
                    reply_to: .comments.nodes[0].databaseId,
                    author: $last.author.login, url: $last.url,
                    body: $last.body,
                    context: [ .comments.nodes[] | { author: .author.login, body } ] } ],
            reviews: [ .latestReviews.nodes[]
                | select((.state == "CHANGES_REQUESTED" or .state == "COMMENTED")
                    and (.body // "") != "" and .author.login != $me)
                | { key: "review:\(.databaseId)", pr: $pr.number, kind: "review",
                    state, author: .author.login, url, body } ]
          }
      ] as $prs
    | {
        generated_at: ($now | todateiso8601),
        viewer: $me,
        stale_hours: $stale,
        prs: [ $prs[] | {
            number, title, url, author, draft, review_decision,
            ci: (if (.failing | length) > 0 then "failing" elif .pending then "pending"
                 elif .check_count == 0 then "none" else "passing" end),
            waiting_hours: hours_since(.waiting_since) } ],
        ci_failures: ([ $prs[] | select(.failing | length > 0)
            | { key: "ci:\(.number):\(.head_sha)", pr: .number, title, branch, head_sha,
                author, author_is_bot, cross_repo, maintainer_can_modify,
                checks: .failing } ] | keep),
        review_comments: ([ $prs[] | .threads[], .reviews[] ] | keep),
        stalled_reviews: ([ $prs[]
            | select((.draft | not)
                and (.review_decision != "APPROVED")
                and (.review_decision != "CHANGES_REQUESTED")
                and (.latest_review_state != "CHANGES_REQUESTED")
                and hours_since(.waiting_since) >= $stale)
            | { key: "stalled:\(.number):\($now | strftime("%Y-%m-%d"))", pr: .number,
                title, url, author, requested_reviewers,
                waiting_hours: hours_since(.waiting_since) } ] | keep)
      }' <<< "$1"
}

print_summary() {
    jq -r '
    "Open PRs: \(.prs | length)   (stalled threshold \(.stale_hours)h, viewer \(.viewer))",
    (.prs[] | "  #\(.number) [\(.ci)] \(.title)\(if .draft then " (draft)" else "" end)"),
    "",
    "Red CI: \(.ci_failures | length)",
    (.ci_failures[] | "  #\(.pr) \(.key)", (.checks[] | "    - \(.name) \(.url // "")")),
    "",
    "Review comments to address: \(.review_comments | length)",
    (.review_comments[] | "  #\(.pr) \(.key) by \(.author): \(.body | split("\n")[0] | .[0:80])"),
    "",
    "Stalled reviews: \(.stalled_reviews | length)",
    (.stalled_reviews[] | "  #\(.pr) waiting \(.waiting_hours)h: \(.title)")
    ' <<< "$1"
}

if [[ "$SELF_CHECK" == "true" ]]; then
    now=$(jq -n '"2026-09-24T12:00:00Z" | fromdateiso8601')
    fixture=$(cat <<'EOF'
{"data":{"viewer":{"login":"me"},"repository":{"pullRequests":{"nodes":[
 {"number":1,"title":"red check run","url":"u1","isDraft":false,"createdAt":"2026-09-20T00:00:00Z",
  "headRefName":"b1","headRefOid":"sha1","isCrossRepository":false,"maintainerCanModify":true,
  "reviewDecision":null,"author":{"login":"me","__typename":"User"},
  "reviewRequests":{"nodes":[{"requestedReviewer":{"login":"rev"}}]},"latestReviews":{"nodes":[]},
  "commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[
    {"__typename":"CheckRun","databaseId":555,"name":"Test","status":"COMPLETED","conclusion":"FAILURE",
     "detailsUrl":"https://github.com/o/r/actions/runs/777/job/555"},
    {"__typename":"StatusContext","context":"ext/ci","state":"ERROR","targetUrl":"https://ci.example/1"},
    {"__typename":"CheckRun","databaseId":556,"name":"Lint","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":null}]}}}}]},
  "reviewThreads":{"nodes":[
    {"isResolved":false,"isOutdated":false,"path":"a.rs","line":3,"comments":{"nodes":[
      {"databaseId":10,"author":{"login":"rev"},"body":"rename this","createdAt":"x","url":"c10"}]}},
    {"isResolved":false,"isOutdated":false,"path":"b.rs","line":4,"comments":{"nodes":[
      {"databaseId":20,"author":{"login":"rev"},"body":"why?","createdAt":"x","url":"c20"},
      {"databaseId":21,"author":{"login":"me"},"body":"because","createdAt":"x","url":"c21"}]}},
    {"isResolved":true,"isOutdated":false,"path":"c.rs","line":5,"comments":{"nodes":[
      {"databaseId":30,"author":{"login":"rev"},"body":"done","createdAt":"x","url":"c30"}]}}]}},
 {"number":2,"title":"pending and acked thread","url":"u2","isDraft":false,"createdAt":"2026-09-24T06:00:00Z",
  "headRefName":"b2","headRefOid":"sha2","isCrossRepository":false,"maintainerCanModify":true,
  "reviewDecision":"REVIEW_REQUIRED","author":{"login":"me","__typename":"User"},
  "reviewRequests":{"nodes":[]},"latestReviews":{"nodes":[
    {"databaseId":900,"author":{"login":"rev"},"state":"CHANGES_REQUESTED","body":"please split","submittedAt":"2026-09-24T07:00:00Z","url":"r900"}]},
  "commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[
    {"__typename":"CheckRun","databaseId":600,"name":"Test","status":"IN_PROGRESS","conclusion":null,"detailsUrl":"x"}]}}}}]},
  "reviewThreads":{"nodes":[
    {"isResolved":false,"isOutdated":false,"path":"d.rs","line":1,"comments":{"nodes":[
      {"databaseId":40,"author":{"login":"rev"},"body":"acked already","createdAt":"x","url":"c40"}]}}]}},
 {"number":3,"title":"draft","url":"u3","isDraft":true,"createdAt":"2026-09-01T00:00:00Z",
  "headRefName":"b3","headRefOid":"sha3","isCrossRepository":true,"maintainerCanModify":false,
  "reviewDecision":null,"author":{"login":"dependabot","__typename":"Bot"},
  "reviewRequests":{"nodes":[]},"latestReviews":{"nodes":[]},
  "commits":{"nodes":[{"commit":{"statusCheckRollup":null}}]},"reviewThreads":{"nodes":[]}}
]}}}}
EOF
)
    report=$(build_report "$fixture" '["thread:40"]' "$now")
    echo "$report" | jq -e '
        (.prs | map(.ci)) == ["failing","pending","none"]
        and (.ci_failures | length) == 1
        and .ci_failures[0].key == "ci:1:sha1"
        and (.ci_failures[0].checks | map(.name)) == ["Test","ext/ci"]
        and .ci_failures[0].checks[0].job_id == 555
        and .ci_failures[0].checks[0].run_id == 777
        and .ci_failures[0].checks[1].job_id == null
        and (.ci_failures[0].checks | all(has("log_tail")))
        and (.review_comments | map(.key)) == ["thread:10","review:900"]
        and .review_comments[0].reply_to == 10
        and (.stalled_reviews | map(.key)) == ["stalled:1:2026-09-24"]
        and .stalled_reviews[0].waiting_hours == 108
    ' > /dev/null || { echo "self-check failed:" >&2; echo "$report" >&2; exit 1; }
    ALL=true
    build_report "$fixture" '["thread:40"]' "$now" | jq -e '
        (.review_comments | map(select(.handled)) | map(.key)) == ["thread:40"]' > /dev/null \
        || { echo "self-check failed: --all did not keep acked items" >&2; exit 1; }
    print_summary "$report" > /dev/null
    echo "self-check ok"
    exit 0
fi

if ! command -v gh &> /dev/null; then
    echo "Error: GitHub CLI (gh) is not installed." >&2
    exit 1
fi

# State lives in the common git dir: shared by every worktree, never committed.
if ! git_dir=$(git rev-parse --path-format=absolute --git-common-dir 2> /dev/null); then
    echo "Error: run this inside a git repository." >&2
    exit 1
fi
STATE_FILE="${BABYSIT_STATE_FILE:-$git_dir/babysit-prs/handled}"

if [[ "$ACK" == "true" ]]; then
    if [[ ${#ACK_KEYS[@]} -eq 0 ]]; then
        echo "Error: --ack needs at least one key." >&2
        exit 1
    fi
    for key in "${ACK_KEYS[@]}"; do
        if [[ ! "$key" =~ ^(ci|thread|review|stalled):[A-Za-z0-9:_-]+$ ]]; then
            echo "Error: $key is not a babysit-prs key." >&2
            exit 1
        fi
        if [[ "$DRY_RUN" == "true" ]]; then
            echo "[dry-run] would ack $key"
        elif ! grep -qxF "$key" "$STATE_FILE" 2> /dev/null; then
            mkdir -p "$(dirname "$STATE_FILE")"
            # ponytail: append-only, keys for closed PRs are never pruned; the file
            # grows by a few lines per PR, trim it by hand if it ever matters.
            echo "$key" >> "$STATE_FILE"
            log "acked $key"
        fi
    done
    exit 0
fi

handled='[]'
if [[ -f "$STATE_FILE" ]]; then
    handled=$(jq -R . "$STATE_FILE" | jq -s .)
fi

log "Fetching open PRs..."
# ponytail: first 50 open PRs, 50 threads each, no pagination; add cursors past that.
raw=$(gh api graphql -F owner='{owner}' -F name='{repo}' -f query='
query($owner: String!, $name: String!) {
  viewer { login }
  repository(owner: $owner, name: $name) {
    pullRequests(states: OPEN, first: 50, orderBy: {field: CREATED_AT, direction: ASC}) {
      nodes {
        number title url isDraft createdAt headRefName headRefOid
        isCrossRepository maintainerCanModify reviewDecision
        author { login __typename }
        reviewRequests(first: 10) { nodes { requestedReviewer { ... on User { login } ... on Team { name } } } }
        latestReviews(first: 20) { nodes { databaseId author { login } state body submittedAt url } }
        commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 50) { nodes {
          __typename
          ... on CheckRun { databaseId name status conclusion detailsUrl }
          ... on StatusContext { context state targetUrl }
        } } } } } }
        reviewThreads(first: 50) { nodes {
          isResolved isOutdated path line
          comments(first: 20) { nodes { databaseId author { login } body createdAt url } }
        } }
      }
    }
  }
}')

report=$(build_report "$raw" "$handled" "$(date +%s)")

# Fill log tails for unhandled failures only; a missing or expired log must not
# end the pass, so each fetch failure becomes a note in log_tail instead.
while IFS=$'\t' read -r i j job; do
    log "Fetching failing log for job $job..."
    if tail_text=$(gh run view --job "$job" --log-failed 2>&1); then
        # The tail of a failed job is post-step cleanup, so end the window on
        # the last ##[error] line instead of the last line.
        tail_text=$(awk -v L="$LOG_LINES" '/##\[error\]/ { n = NR } { a[NR] = $0 }
            END { e = n ? n : NR; for (i = (e - L + 1 > 1 ? e - L + 1 : 1); i <= e; i++) print a[i] }' \
            <<< "$tail_text")
    else
        tail_text="log unavailable: $(tail -n 1 <<< "$tail_text")"
    fi
    report=$(jq --argjson i "$i" --argjson j "$j" --arg t "$tail_text" \
        '.ci_failures[$i].checks[$j].log_tail = ($t | split("\n")
            | map(gsub("(\u001b|\\^\\[)\\[[0-9;]*m"; "")
                | sub("^[^\t]*\t[^\t]*\t[0-9T:.-]+Z "; ""))
            | join("\n"))' <<< "$report")
done < <(jq -r '.ci_failures | to_entries[] | select(.value.handled | not) | .key as $i
    | .value.checks | to_entries[] | select(.value.job_id != null)
    | [$i, .key, .value.job_id] | @tsv' <<< "$report")

if [[ "$JSON_OUT" == "true" ]]; then
    jq --arg state "$STATE_FILE" '. + { state_file: $state }' <<< "$report"
else
    print_summary "$report"
fi
