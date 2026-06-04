# Agent Hooks

Project-scoped [Claude Code hooks](https://docs.claude.com/en/docs/claude-code/hooks)
that run automatically while agents work in this repo. They live in
`.claude/settings.json` (committed, team-wide) and the scripts under
`.claude/hooks/`.

## Pre-commit checks on `Stop`

**What:** when an agent finishes a turn, the fast half of CI runs automatically —
but only if Rust sources changed in the working tree. If anything fails, the agent
is blocked from finishing and gets the failure output to fix.

**Files:**

- `.claude/settings.json` — registers a `Stop` hook.
- `.claude/hooks/precommit-fast.sh` — the check script.

**Checks run (mirrors `ci.yml`, minus the slow test suite):**

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo build --all`

The test suite (`cargo test --all`, ~2 min) is intentionally left out to keep the
turn-end loop fast. Run it manually (or rely on CI) before tagging a release.

**Gating:** the script no-ops unless `git status --porcelain` shows a changed
`*.rs`, `Cargo.toml`, or `Cargo.lock`. Non-code turns finish instantly.

**On failure:** the script emits `{"decision": "block", "reason": "<output>"}`, so
Claude Code keeps the turn open and feeds the failing check output back to the
agent to fix. The loop ends naturally once the checks pass. (If `jq` is missing it
falls back to exit code 2 + stderr, which has the same blocking effect.)

### Enabling / disabling

- A freshly added `.claude/settings.json` may need a config reload: open `/hooks`
  once, or restart Claude Code, so the watcher picks it up.
- Review or toggle the hook anytime from the `/hooks` menu.
- Set `"disableAllHooks": true` in `.claude/settings.local.json` to turn off all
  hooks locally without touching the committed config.

### Tuning

Edit `.claude/hooks/precommit-fast.sh`:

- To include tests, add `run "cargo test --all" cargo test --all` (expect a much
  slower turn end).
- To only lint, drop the `cargo build --all` line.
- The `timeout` (seconds) for the whole hook lives in `.claude/settings.json`.
