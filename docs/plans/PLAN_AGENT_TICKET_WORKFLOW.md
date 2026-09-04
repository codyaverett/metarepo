# Plan: Agent-driven ticket workflow and review de-bottlenecking

Status: proposed
Related issues: #141 (ticket-start command), #142 (babysit-prs loop), #143 (plan-to-issues generator)
Date: 2026-08-11

## Goal

Make the full delivery lifecycle - planning, ticket tracking, driving ticketed
work to completion, and human code review - runnable end-to-end by agents on
top of the metarepo workspace, with human review latency (the slowest stage)
actively managed rather than passively waited on.

## Current state

- Planning convention exists: plan docs in `docs/plans/`, issues created
  before implementation via `.github/scripts/new-feature.sh` and friends
  (JSON stdin, agent-callable, return URLs).
- Worktrees give per-ticket isolation (`meta worktree add feature/x`, hooks
  run `worktree_init`), and `meta worktree list` doubles as an in-progress
  board. But nothing enforces the issue-to-branch-to-label convention; it is
  tribal knowledge.
- Nothing watches open PRs. Red CI, unanswered review comments, and stalled
  reviews are only noticed when a human happens to look.
- Decomposing a plan doc into issues is N manual script invocations plus
  hand-editing the Related issues line back into the doc.

## Approach

Three additive tools, no changes to the meta binary. Each is a script or
skill layered on existing surfaces (`gh`, `meta worktree`, the
`.github/scripts` family, Slack via MCP), so they ship independently and in
any order. Together they close the loop:

```
plan doc --(#143)--> issues --(#141)--> worktree + labels
   ^                                        |
   |                                   implement, test
   |                                        v
   +------ retro / next plan <--(#142)-- PR babysitting
```

### Workstream 1: ticket-start (#141)

One command from issue number to ready-to-code worktree: fetch issue via
`gh`, derive `feature/NNN-slug` branch, `meta worktree add` (hooks install
deps), move label to in-progress and self-assign. Optional `ticket-finish`
counterpart flips the label to in-review when the PR opens. Delivered as a
script wired into `.meta` scripts or a Claude skill; must work with
`--non-interactive`.

### Workstream 2: babysit-prs (#142)

A recurring, idempotent pass over open PRs suitable for `/loop` or a
scheduled agent: fix or diagnose red CI in the PR branch worktree, address
new review comments with commits and inline replies then re-request review,
and post a Slack digest for PRs waiting on review past a threshold
(default 24h). This is the direct attack on review latency: same-day
feedback turnaround plus reviewer nudges.

### Workstream 3: plan-to-issues (#143)

Shipped: `.github/scripts/plan-to-issues.sh` takes a plan doc path and
either a `## Tasks` section (numbered items, indented body, `blocked by: N`
and `priority:` markers) or a JSON array via `--json`, creates each task as a
feature issue through `new-feature.sh`, comments `Blocked by #N` on dependent
issues, writes `(#N)` back onto each task line so re-runs are idempotent, and
appends the numbers to the doc's Related issues line. Stdout is the URL list,
which chains into ticket-start. `--dry-run` previews. Documented in
`.github/scripts/README.md`.

## Review policy (process, not code)

Alongside the tooling, adopt conventions that make PRs cheap to approve:
single-ticket PRs sized for a five-minute review; agent pre-review
(code-review, security-review, clippy, fmt, tests green) before humans are
requested; PR descriptions that state what, why, risk, and a suggested
reading order; and a tiered policy where docs-only, test-only, and
mechanical changes need only a lightweight approval. These land as a short
CONTRIBUTING or docs update once the tooling proves out.

## Sequencing and risks

Suggested order: #141 first (smallest, establishes the convention), then
#143 (front of the funnel), then #142 (largest, needs Slack config and
unattended-safety care). Risks: babysit-prs must be conservative about
pushing to PR branches it did not author (prefer diagnose-and-comment for
those); label taxonomy (todo / in-progress / in-review) needs to exist in
the repo before #141 lands.
