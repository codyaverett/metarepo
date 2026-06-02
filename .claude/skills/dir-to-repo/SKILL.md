---
name: dir-to-repo
description: This skill should be used when the user asks to "convert a directory to a git repo", "make a folder a repository", "git init this directory", "turn a folder into a repo", "initialize git for these directories", or batch-convert loose directories in a metarepo workspace into their own git repositories (optionally creating a GitHub remote and registering them as meta projects).
version: 0.1.0
---

# dir-to-repo

Convert loose local directories into their own git repositories — git init, a
default `.gitignore`, an initial commit, an optional GitHub remote, and optional
registration as a project in the surrounding metarepo workspace.

All work is done by `.github/scripts/dir-to-repo.sh`. This skill explains when
and how to drive it.

## When to use

- "Make this folder a git repo"
- "git init these directories and push them to GitHub"
- "Turn each subfolder under `./packages` into its own repo"
- "Convert this directory and add it to my metarepo workspace"

## Workflow

The script runs a local-first pipeline per directory:

1. **Validate** — the path exists, is a directory, is not already a repo (idempotent skip), and is not nested inside another repo (refused).
2. **git init** — initializes on the `main` branch.
3. **.gitignore** — writes a default cross-language ignore file, but never clobbers an existing one.
4. **Initial commit** — `git add -A` + `chore: initialize repository` (uses `--allow-empty` for empty dirs).
5. **Optional remote** (`--remote`/`--push`) — `gh repo create --source . --push` (private by default, `--public` for public). Requires an authenticated `gh`; if missing/unauthenticated it warns and leaves the local repo intact.
6. **Optional registration** (default on) — registers the dir as a workspace project via `meta project add <name> --init-git`. Skipped with `--no-register`.

## Invocation

Single directory (local-only, registered into the workspace):

```bash
.github/scripts/dir-to-repo.sh ./my-folder
```

Single directory, no workspace registration:

```bash
.github/scripts/dir-to-repo.sh ./my-folder --no-register
```

Single directory + create and push a public GitHub remote:

```bash
.github/scripts/dir-to-repo.sh ./my-folder --remote --public
```

Batch — convert every loose subdirectory of a parent (already-repo subdirs are skipped):

```bash
.github/scripts/dir-to-repo.sh --scan ./packages
.github/scripts/dir-to-repo.sh --all          # same as --scan .
```

Automation (JSON stdin, quiet — emits only `converted:` summary lines):

```bash
echo '{"dir":"./my-folder","remote":true,"public":false,"register":true}' \
  | .github/scripts/dir-to-repo.sh --json --silent
```

Run `.github/scripts/dir-to-repo.sh --help` for the full flag list.

## Flags

| Flag | Effect |
|------|--------|
| `<dir>` | Single directory to convert |
| `--scan <parent>` | Batch: convert each loose subdir of `<parent>` |
| `--all` | Shorthand for `--scan .` |
| `--remote`, `--push` | Create a GitHub repo (gh) and push (private) |
| `--public` | Make the created GitHub repo public (implies `--remote`) |
| `--no-register` | Skip `meta project add` |
| `--gitignore-template <name>` | Select a `.gitignore` template (default: `default`) |
| `--json` | Read input from JSON stdin |
| `--silent` | Suppress non-error output (still prints `converted:` summary lines) |
| `--help`, `-h` | Show help |

## Important constraints

- **Registration requires the dir to live inside the workspace root.** `meta
  project add` takes a *relative project name* within the workspace (the dir
  containing `.meta`/`.metarepo`), not an arbitrary path. A directory outside the
  workspace is still git-initialized, but registration is skipped with a warning.
- **Registration reads `.meta` at the workspace root.** `meta project add`
  currently only honors a `.meta` file, not the `.metarepo` that `meta init`
  writes by default (tracked as issue #72). When `meta project add` cannot find a
  `.meta` file, the script warns and continues — the local repo is fully created
  regardless. Use `--no-register`, or add a `.meta` workspace config, until #72
  lands.
- **`meta` binary discovery** — the script uses `meta` on `PATH`, else
  `target/debug/meta` / `target/release/meta`, else `$META_BIN`. Build the binary
  (`cargo build`) or install it if registration is needed.

## Edge cases

- Directory already a repo → skipped (idempotent).
- Empty directory → still gets a valid initial commit (`--allow-empty`).
- Directory nested inside an existing repo → refused.
- Workspace name collision → `meta project add` reports "already exists"; the
  script warns and continues (local repo still created).
- `gh` not installed/authenticated → remote step skipped, local repo intact.

## See also

- `.github/scripts/dir-to-repo.sh` — the implementation.
- `meta project add` — the underlying workspace registration command.
- `references/CHANGELOG_NOTES.md` — version history for this skill.
