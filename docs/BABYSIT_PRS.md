# Babysit PRs Guide

Keep open pull requests moving without a human watching them: notice red CI,
answer review comments, and nudge reviews that have stalled.

## Table of Contents

- [Overview](#overview)
- [Running a Pass](#running-a-pass)
- [Running on a Loop](#running-on-a-loop)
- [The Report](#the-report)
- [Idempotency and State](#idempotency-and-state)
- [Safety Rules](#safety-rules)
- [Troubleshooting](#troubleshooting)
- [Future Work](#future-work)

## Overview

Two pieces, split by who should make the call:

| Piece | Path | Job |
|-------|------|-----|
| Script | `.github/scripts/babysit-prs.sh` | Deterministic, read-only gathering: open PRs, CI state, failing log tails, new review comments, review age |
| Skill | `.claude/skills/babysit-prs/SKILL.md` | Judgment and writes: fix vs. diagnose, commits, inline replies, re-requesting review, the stalled digest |

The script needs only `gh` and `jq`, never writes to GitHub, and never talks to
Slack. The skill posts the stalled digest through a Slack tool when the
session has one, and prints it otherwise.

## Running a Pass

In a Claude Code session in this repository:

```
/babysit-prs
```

To see what a pass would act on without acting:

```bash
.github/scripts/babysit-prs.sh            # human summary
.github/scripts/babysit-prs.sh --json     # what the skill reads
```

Change the stalled threshold with `--stale-hours 48` or
`BABYSIT_STALE_HOURS=48`.

## Running on a Loop

```
/loop 30m /babysit-prs
```

Every 30 minutes is plenty: CI runs take minutes and reviewers take hours. For
an overnight run from the cloud, use `/schedule` with the same `/babysit-prs`
prompt. Because state lives in the local clone (see below), the scheduled
machine has to keep its checkout between runs, or it will repeat the day's
stalled digest and re-examine old failures once.

## The Report

`--json` prints:

- `prs` - every open PR with `ci` (`failing`, `pending`, `passing`, `none`)
  and `waiting_hours`.
- `ci_failures` - one item per PR head commit with failing checks. Each check
  has `name`, `url`, `job_id`, `run_id`, and `log_tail`. The item also carries
  `author_is_bot`, `cross_repo`, and `maintainer_can_modify`, which decide
  whether the skill may push a fix at all.
- `review_comments` - unresolved threads whose latest comment is not from the
  viewer (`kind: thread`, with `reply_to`, `path`, `line`, and the thread
  `context`), plus review bodies that request changes or comment
  (`kind: review`).
- `stalled_reviews` - non-draft PRs not approved and not waiting on the author,
  with `waiting_hours` at or above the threshold.

## Idempotency and State

Every work item has a key:

| Key | Acked when | Comes back when |
|-----|-----------|-----------------|
| `ci:<pr>:<sha>` | CI was fixed or diagnosed | A new push fails again |
| `thread:<comment-id>` | A reply was posted | The reviewer answers (new last comment) |
| `review:<review-id>` | The review was answered | A new review is submitted |
| `stalled:<pr>:<yyyy-mm-dd>` | The digest was delivered | The next day, if still waiting |

The skill runs `babysit-prs.sh --ack <key>` right after each action succeeds.
Nothing is ever marked handled by gathering alone, so a pass that crashes
halfway leaves the rest for the next pass.

Acked keys live in `.git/babysit-prs/handled` (resolved through
`git rev-parse --git-common-dir`, so all worktrees share one file and it can
never be committed). Delete the file to start fresh; set `BABYSIT_STATE_FILE`
to put it elsewhere.

## Safety Rules

The skill follows these on every pass:

- No pushes to bot-authored PRs (dependabot rebases would be broken), or to
  fork branches that do not allow maintainer edits. Those get a diagnosis
  comment instead.
- Fixes go in a throwaway worktree on a `babysit/<pr>` branch, pushed with
  `HEAD:<branch>`. Never force-push, merge, close, or push to `main`.
- Commits follow CLAUDE.md: conventional, shell-safe, signed, no attribution.
- A denied GitHub write is reported and left unacked, never retried in a loop.

## Troubleshooting

**`log unavailable: failed to get run log: HTTP 410`** - GitHub expired the
Actions log (the default retention is 90 days). Re-run the job to get a fresh
log, or rebase the PR.

**Every dependabot PR shows as stalled** - correct if nobody reviewed them. The
digest is still sent at most once per PR per day. Raise `--stale-hours` if that
is too loud.

**Tail shows cleanup lines, not the error** - the window ends on the last
`##[error]` line; a job that fails without one falls back to the last lines of
the log. Increase `--log-lines`.

**Offline check** - `.github/scripts/babysit-prs.sh --self-check` runs a
fixture through the same report builder with no network.

## Future Work

- CI-failure classification (#162) reads the per-check `log_tail` field to
  sort failures into flaky, infrastructure, and real before the skill decides
  fix vs. diagnose.
- Pagination beyond 50 open PRs or 50 threads per PR.
