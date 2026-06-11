# Skill Tools Guide

The `meta skill` command manages the bundled meta-tool Claude Code skill and also
discovers, audits, and copies **other** Claude Code skills between repositories.
The discovery/audit/copy capability is adapted from
[galaxy-gateway/steal-skill](https://github.com/galaxy-gateway/steal-skill) and
folded into the built-in `skill` plugin so it shares one binary, one frontmatter
parser, and one install location.

## Table of Contents

- [Overview](#overview)
- [Subcommands](#subcommands)
  - [scan](#scan)
  - [audit](#audit)
  - [locations](#locations)
  - [steal](#steal)
  - [search](#search)
  - [add](#add)
- [Installing from skills.sh](#installing-from-skillssh)
- [The audit safety gate](#the-audit-safety-gate)
- [Risk patterns flagged by audit](#risk-patterns-flagged-by-audit)
- [How this maps to the upstream steal-skill](#how-this-maps-to-the-upstream-steal-skill)

## Overview

A Claude Code skill is a directory containing a `SKILL.md` (YAML frontmatter +
markdown body) plus optional supporting files. Skills are resolved from, in
order: `$CLAUDE_SKILLS_HOME`, the workspace `./.claude/skills`, then
`~/.claude/skills`.

The `skill` plugin now covers the full lifecycle of working with skills:

| Concern | Subcommands |
| --- | --- |
| The bundled meta-tool skill | `install`, `update`, `status` (default), `remove` |
| Finding skills | `scan`, `locations`, `search` |
| Vetting skills | `audit` |
| Importing skills | `steal` (local path or git URL), `add` (skills.sh) |

### Bundled skill update safety

`install` and `update` record a sha256 fingerprint of the files they write in
`.claude/skills/meta-tool/.skill-lock.json`. `meta skill update` only rewrites
the skill when the installed files still match that fingerprint (a pristine
copy of an older bundle). It refuses — leaving the files untouched and
explaining why — when:

- the installed files were locally modified,
- the installed version is newer than the bundled one (downgrade), or
- no fingerprint was recorded (legacy or hand-rolled install).

Pass `--force` / `-f` to overwrite anyway. `update` no longer installs the
skill when absent; `meta skill install` (or `meta init --with-skill`) is the
opt-in entry point. Pristine legacy installs gain a fingerprint on their next
successful `update`/`install`.

## Subcommands

### scan

Walk a directory tree and list every skill found (`.git`, `node_modules`, and
`target` are skipped).

```bash
meta skill scan ~/Projects        # list skills anywhere under ~/Projects
meta skill scan                   # defaults to the current directory
```

Each result shows the skill name, its description, and the path to its
`SKILL.md`.

### audit

Inspect a single skill and flag risky patterns before you trust or copy it.

```bash
meta skill audit ~/Downloads/some-skill
meta skill audit ./.claude/skills/meta-tool/SKILL.md   # a path to SKILL.md also works
```

Findings are graded `HIGH` / `MED` / `LOW`. See
[Risk patterns](#risk-patterns-flagged-by-audit). The auditor is heuristic and
substring-based, so it can produce false positives (for example a skill that
merely *documents* a `curl ... | sh` pattern will be flagged). Treat findings as
prompts to read the file, not as a verdict.

### locations

Print the candidate skill destination directories in resolution order, marking
which already exist.

```bash
meta skill locations
```

### steal

Copy one or more external skills into a local skills directory, audit-gated. The
source can be:

- a **single skill** — a directory containing a `SKILL.md`, or a `SKILL.md` path;
- a **directory tree** containing many skills;
- a **git URL** — cloned shallowly (`git clone --depth 1`), then treated as a tree.
  Pin a branch, tag, or commit SHA with `--ref` (alias `--branch`) or the inline
  `url#ref` syntax; the default branch is used otherwise.

When the source holds more than one skill you choose which to take. In a terminal
this opens a full-screen **picker**: a static header describing the source repo
(`url@commit`), a scrollable `Skill | Description` table where selected rows are
highlighted with a `✓` (HIGH-risk skills flagged `⚠`), type-to-filter, and mouse
click/scroll. Space toggles, `a` toggles all, enter confirms, esc cancels. When
scripted (no TTY) use `--all` / `--name` instead. Each chosen skill lands at
`<dest-root>/<name>`, where `<name>` comes from the frontmatter (falling back to
the source directory name), and each copy passes the audit gate independently.

```bash
meta skill steal ~/Downloads/some-skill                 # copy one local skill
meta skill steal ./skills                               # pick from a local tree
meta skill steal https://github.com/owner/repo.git      # clone, pick, copy
meta skill steal https://github.com/owner/repo.git --ref v2.1  # steal from a branch/tag/SHA
meta skill steal https://github.com/owner/repo.git#dev   # same, inline ref syntax
meta skill steal https://github.com/owner/repo.git --preview   # preview all, copy none
meta skill steal https://github.com/owner/repo.git --all       # copy every skill
meta skill steal <git-url> --name foo --name bar        # copy named skills (scriptable)
meta skill steal ~/Downloads/some-skill --dest ~/.claude/skills
meta skill steal ~/Downloads/some-skill --overwrite     # replace an existing copy
meta skill steal ~/Downloads/some-skill --force         # copy despite HIGH findings
```

Flags:

- `--dest <dir>` — destination skills root. Resolution order: `--dest` > the
  configured `[skill] dest` (see below) > `$CLAUDE_SKILLS_HOME` > `./.claude/skills`
  > `~/.claude/skills`.
- `--ref <ref>` (alias `--branch`) — for git URL sources, the branch, tag, or
  commit SHA to steal from. Branches and tags clone shallowly at the ref; a
  commit SHA is fetched into the clone and checked out. `url#ref` is equivalent
  (an explicit `--ref` must agree if both are given). The ref is recorded as
  `ref = "..."` in the `.meta-source.toml` provenance file alongside the
  resolved commit. A ref that does not exist on the remote fails with a
  distinct error naming the ref.
- `--all` — steal every skill found in the source.
- `--name <name>` — steal the skill(s) with this name (repeatable). Matches the
  frontmatter name or the source directory name (case-insensitive).
- `--preview` — print a preview (audit findings + body excerpt) of every skill
  found and copy nothing.
- `--adapt [purpose]` — after installing, run a **headless AI command** to adapt
  each stolen skill to this repo. `--adapt` alone tailors to the repo; `--adapt
  "fit our CI"` adds a purpose. The command defaults to Claude but is configurable
  (see `[skill]` below). Skipped if that command is not on `PATH`.
- `--overwrite` — replace an existing skill of the same name (skips it otherwise).
- `--force` / `-f` — proceed even when the audit reports HIGH-severity findings.

**Review marking.** Whenever a stolen skill has audit findings, steal records a
review trail in the installed copy (independent of `--force`/`--adapt`, so it
survives even with no Claude available):

- a sidecar `.meta-review.md` listing each finding as `file:line [SEVERITY]
  message` with the offending line quoted; and
- inline comment markers (`<!-- meta:review [HIGH] … -->`, or `#`/`//` per file
  type) inserted directly above each flagged line in comment-safe files. Files
  where a stray comment would corrupt them (json/yaml/toml/unknown) are left
  untouched — they still appear in the sidecar.

Audit findings now report `file:line` so you can jump straight to the risky line.

**Adaptation (`--adapt`).** The skill is backed up (to the OS temp dir under
`meta-skill-backups/`), then the configured adapt command runs with the working
directory set to the installed skill so the agent can edit its files in place —
tailoring them to the repo's name, detected languages, and layout, plus any
purpose you give. The adapted skill is re-audited afterward; if that introduces a
new HIGH-severity pattern it is reported and the backup path is printed for manual
rollback. This is opt-in and lets the agent modify files in the installed skill
directory.

**Multi-file skills (scripts + data).** A skill is rarely just `SKILL.md`. It can
ship supporting scripts, extra markdown, templates, and data files, usually in
subdirectories:

```
some-skill/
  SKILL.md            # entry point — only this is auto-loaded into context
  scripts/build.sh    # supporting executables
  reference/api.md    # extra markdown the body links to
  templates/...        # data files
```

`steal` copies the **entire skill directory recursively**, not just `SKILL.md`.
The copy (`copy_tree`) walks the whole tree and preserves the relative layout, so
`scripts/build.sh` lands at `<dest>/<name>/scripts/build.sh`. Three directories are
skipped as noise: `.git`, `node_modules`, and `target`. Symlinks are not followed.

Notes for multi-file skills:

- **Audit covers every file**, not just `SKILL.md`. The risk scan walks the full
  tree, so a dangerous pattern in `scripts/build.sh` still trips the HIGH gate and
  gets a `.meta-review.md` entry and an inline marker (in comment-safe files).
- **Executable bits are reported** — a shipped executable is itself a flagged
  pattern. After copying, re-set exec bits if the skill needs them
  (`chmod +x scripts/*.sh`); std `fs::copy` carries Unix permissions, but verify.
- **Relative paths only.** The body references siblings by relative path; those
  survive the copy. Any absolute path baked into `SKILL.md` or a script breaks at
  the new location — grep for them after stealing.
- Supporting files are **not auto-loaded** into context; only `SKILL.md`'s
  frontmatter and body are. The agent reads or runs the rest on demand when the
  body points at them, which keeps context small.

### Configuration — `[skill]` in `.meta`

Both the default install location and the adapt command are configurable per
workspace via a `[skill]` block. `meta skill locations` prints the resolved values.

```toml
[skill]
# Default install dir for stolen skills (tilde-expanded). Overridden by --dest.
dest = "~/.config/agent-skills"

# The AI command run by --adapt. Defaults to Claude when unset.
# `{prompt}` is replaced with the generated adaptation prompt; the command runs
# with its working directory set to the installed skill so it can edit files.
adapt-command = "claude"
adapt-args = ["-p", "{prompt}", "--permission-mode", "acceptEdits"]
```

Use a different agent by overriding `adapt-command`/`adapt-args`:

```toml
# codex
adapt-command = "codex"
adapt-args = ["exec", "{prompt}"]

# opencode
adapt-command = "opencode"
adapt-args = ["run", "{prompt}"]
```

(JSON/YAML `.meta` work too; the keys are `skill.dest`, `skill.adapt-command`,
`skill.adapt-args`.)

In a non-interactive run (no TTY) against a multi-skill source, you must pass
`--all` or `--name`; otherwise `steal` errors and lists the skills it found. A
git source requires `git` on `PATH`.

**Provenance.** When the source skill lives inside a git repository — a git-URL
clone, or a local checkout — `steal` records where it came from: it prints a
`source: <url>@<commit> (<subpath>)` line and writes a `.meta-source.toml` into
the copied skill with the remote `url`, the `commit` SHA, the skill's `subpath`
within the repo, and a `dirty` flag (true when the working tree had uncommitted
changes). This keeps a stolen skill traceable and re-fetchable. Sources not under
git are copied without a provenance file.

### search

Search the [skills.sh](https://skills.sh) registry for Claude Code skills. Uses
the public, unauthenticated search endpoint.

```bash
meta skill search react              # top matches for "react"
meta skill search "next js" --limit 50
```

Each result prints its install count and canonical `owner/repo/skill` id. Install
a result with `meta skill add <id>`.

### add

Install a skill from skills.sh by its id (audit-gated, like `steal`).

```bash
meta skill add vercel-labs/agent-skills/vercel-react-best-practices
meta skill add <id> --dest ~/.claude/skills --overwrite
meta skill add <id> --ref v2         # resolve from a branch, tag, or SHA
meta skill add <id> --force          # install despite HIGH findings
```

Flags mirror `steal`: `--dest`, `--overwrite`, `--force`/`-f`, and `--ref`
(alias `--branch`). Because the skills.sh API only serves the latest registry
copy, `--ref` always resolves via the GitHub path, even when an API key is set.

## Installing from skills.sh

`add` resolves a skill's files one of two ways, chosen automatically:

- **Keyed** — when `SKILLS_SH_API_KEY` (an `sk_live_...` key from skills.sh) is
  set, files are fetched from the authenticated `/api/v1/skills/{id}` endpoint.
  This is exact and reliable.
- **Keyless** (default) — the skill's source GitHub repo is shallow-cloned and
  the matching skill directory is located by fuzzy-matching the registry slug
  against the repo's skill directories and frontmatter names. The skills.sh slug
  does not map 1:1 to a repo path (for example `vercel-react-best-practices`
  lives at `skills/react-best-practices/`), so on an ambiguous or missing match
  `add` lists the skills it found and suggests setting `SKILLS_SH_API_KEY`.

Either way the resolved skill is run through the same [audit gate](#the-audit-safety-gate)
as `steal` before anything is written. Keyless install requires `git`; both paths
require `curl` for the skills.sh HTTP calls.

## The audit safety gate

`steal` always runs `audit` on the source first and prints the findings. If any
finding is **HIGH** severity, the copy is refused unless `--force` is given. This
prevents silently importing a skill that fetches and executes remote code, runs
`sudo`, or grants unrestricted tool access. When `--force` is used to override,
the copy still completes but a `⚠` reminder is printed to review before use.

## Risk patterns flagged by audit

**HIGH**

- `curl` / `wget` (network fetch)
- `| sh` / `| bash` (piping into a shell — remote-exec pattern)
- `rm -rf` (destructive delete)
- `sudo`
- `eval` (dynamic code execution)
- `allowed-tools` frontmatter granting a wildcard (`"*"`, `bash(*)`)

**MEDIUM**

- Executable files shipped with the skill (unix permission bit)
- `chmod +x`
- `git push`
- `--no-verify` (bypasses git hooks)
- `aws_secret` / `api_key` (possible credential references)
- `ssh`

**LOW**

- Missing `name` in frontmatter
- Missing `description` in frontmatter

## How this maps to the upstream steal-skill

| Upstream `learn-skill` | metarepo equivalent |
| --- | --- |
| `learn-skill locations` | `meta skill locations` |
| `learn-skill scan <path>` | `meta skill scan <path>` |
| `learn-skill audit <path>` | `meta skill audit <path>` |
| *(advertised, not implemented)* | `meta skill steal <path>` |

The upstream project is a standalone Rust CLI (`learn-skill`) using the same
dependency stack as metarepo (clap, serde, walkdir, anyhow, colored). Rather than
vendor it as a separate binary, its modules were ported into
`meta/src/plugins/skill/` (`skill_file.rs`, `scan.rs`, `audit.rs`,
`locations.rs`) and the `audit` module was refactored to *return* findings so the
new `steal` command can gate on them. The "copy" feature its README advertised
but never shipped is implemented here as `steal`.
