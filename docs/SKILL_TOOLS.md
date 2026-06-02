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
| Finding skills | `scan`, `locations` |
| Vetting skills | `audit` |
| Importing skills | `steal` |

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

Copy an external skill into a local skills directory. This is the "copy" half of
the workflow: `scan` finds skills, `audit` vets them, and `steal` brings a chosen
one in. The skill lands at `<dest-root>/<name>`, where `<name>` comes from the
frontmatter (falling back to the source directory name).

```bash
meta skill steal ~/Downloads/some-skill              # copy into the default dest
meta skill steal ~/Downloads/some-skill --dest ~/.claude/skills
meta skill steal ~/Downloads/some-skill --overwrite  # replace an existing copy
meta skill steal ~/Downloads/some-skill --force      # copy despite HIGH findings
```

Flags:

- `--dest <dir>` — destination skills root. Defaults to the first existing
  candidate from `meta skill locations` (else the workspace `./.claude/skills`).
- `--overwrite` — replace an existing skill of the same name (refuses otherwise).
- `--force` / `-f` — proceed even when the audit reports HIGH-severity findings.

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
