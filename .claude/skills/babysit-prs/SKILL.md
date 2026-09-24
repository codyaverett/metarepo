---
name: babysit-prs
description: This skill should be used when the user asks to "babysit PRs", "watch my open PRs", "keep CI green on open PRs", "answer review comments", "nudge stalled reviews", or runs /babysit-prs under /loop or a scheduled agent. One idempotent pass over open PRs - fix or diagnose red CI, address new review comments, and produce a stalled-review digest.
version: 0.1.0
---

# babysit-prs

One pass over the repo's open pull requests. Safe to repeat: every item has a
key, and a key is acked only after its action succeeded, so a pass that dies
halfway is simply retried next time.

State gathering is done by `.github/scripts/babysit-prs.sh`, which only reads
from GitHub. Every decision and every write happens here, in the skill.

## When to use

- "Babysit my PRs" / "keep an eye on open PRs"
- `/loop 30m /babysit-prs` for a recurring pass in this session
- A scheduled agent that should keep PRs moving overnight

## Pass

### 1. Gather

```bash
.github/scripts/babysit-prs.sh --json --silent
```

The report has three work lists; each item carries a `key`:

| List | Key | What it means |
|------|-----|---------------|
| `ci_failures` | `ci:<pr>:<sha>` | Failing checks at the PR head, each with `log_tail` |
| `review_comments` | `thread:<id>` / `review:<id>` | Unresolved thread whose last word is not yours, or a review body |
| `stalled_reviews` | `stalled:<pr>:<date>` | Non-draft PR waiting on review past `--stale-hours` (default 24) |

Already-acked items are omitted. If all three lists are empty, say so in one
line and stop.

### 2. Red CI - fix or diagnose

For each `ci_failures` item, read `checks[].log_tail` (null means a non-Actions
status with only a `url`; a `log unavailable:` prefix means the log expired).

**Only fix** when all of these hold, otherwise diagnose:

- `author_is_bot` is false (pushing to a dependabot branch breaks its rebasing;
  comment instead, or ask dependabot to `@dependabot rebase`)
- `cross_repo` is false, or `maintainer_can_modify` is true
- The cause is clear from the log and the fix is small and local to the branch
- The failure is not infrastructure (runner outage, expired log, flaky network)

**Fix** in a dedicated worktree, never the user's checkout:

```bash
git fetch origin <branch>
git worktree add -b babysit/<pr> ../babysit-<pr> origin/<branch>
cd ../babysit-<pr>
# make the change, then run the project checks
# (Rust: cargo build, cargo test, cargo clippy -- -D warnings, cargo fmt --check)
git commit -S -m "fix(ci): <what and why>"
git push origin HEAD:<branch>
cd - && git worktree remove ../babysit-<pr> && git branch -D babysit/<pr>
```

Follow the repo commit rules in CLAUDE.md (conventional commit, shell-safe
message, signed; no attribution lines). Never force-push.

**Diagnose** otherwise with one PR comment: which check failed, the key log
lines, the likely cause, and the suggested fix.

Then ack: `.github/scripts/babysit-prs.sh --ack ci:<pr>:<sha>`.
A later push produces a new sha and therefore a new key, so a re-break is seen.

### 3. Review comments - address and reply

For each `review_comments` item (`context` holds the thread so far):

1. If it asks for a change you can make (same eligibility as a CI fix), commit
   it in the PR worktree as above and push.
2. Reply in place. Threads reply to `reply_to` (the thread's first comment):
   ```bash
   gh api repos/{owner}/{repo}/pulls/<pr>/comments/<reply_to>/replies -f body='...'
   ```
   Review bodies get a PR comment: `gh pr comment <pr> --body '...'`.
   Say what changed and name the commit, or why no change was made.
3. Ack the item's key.

When every item on a PR is done and a fix was pushed, re-request review from
the reviewers who commented:
`gh pr edit <pr> --add-reviewer <login>`.

Do not resolve threads yourself; the reviewer resolves.

### 4. Stalled-review digest

Build one digest from `stalled_reviews`:

```
Stalled reviews (waiting over 24h)
- #123 feat: title - 52h, reviewers: alice, bob - <url>
```

If the session has a Slack tool (for example a `send_message` MCP tool) and
the user named a channel, post it there. Otherwise print it. The script never
talks to Slack and holds no Slack credentials. Ack each `stalled:` key after
the digest is delivered, which limits nudges to one per PR per day.

## Unattended safety

- Every GitHub write is a skill action, never the script. Preview a pass with
  the report alone; `--dry-run` also stops `--ack` from writing state.
- If a GitHub write is denied (auto mode may block raw `gh` writes), do not
  retry. List the intended action in the pass summary and leave the key
  unacked so the next pass, or a human, picks it up.
- Never push to `main`, never force-push, never merge, never close a PR.
- End each pass with a short summary: fixed, diagnosed, replied, digested,
  skipped (and why).

## Running on a loop

```
/loop 30m /babysit-prs
```

For an overnight scheduled agent, use `/schedule` with the same prompt. State
lives in `.git/babysit-prs/handled` (the common git dir, shared by worktrees),
so a machine that runs the loop must keep its clone between runs.

## Reference

- Script flags and output: `.github/scripts/README.md` (Babysit PRs section)
- Guide: `docs/BABYSIT_PRS.md`
