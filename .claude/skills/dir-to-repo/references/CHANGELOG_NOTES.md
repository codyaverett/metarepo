# dir-to-repo — Changelog

## 0.1.0

- Initial release.
- Single-directory and batch (`--scan` / `--all`) conversion of loose
  directories into git repositories.
- Local-first pipeline: validate → `git init -b main` → default `.gitignore`
  (never clobbered) → initial commit (`--allow-empty` fallback).
- Optional GitHub remote via `gh repo create --source . --push`
  (`--remote`/`--public`), gated on an authenticated `gh`.
- Optional registration into the metarepo workspace via
  `meta project add <name> --init-git` (default on; `--no-register` to skip).
- Input modes: positional arg, `--json` stdin, environment variables,
  interactive. `--silent` for automation.
- Edge cases: idempotent skip of existing repos, refusal of dirs nested inside
  another repo, graceful skip when outside the workspace root or when `gh`/`meta`
  are unavailable.
