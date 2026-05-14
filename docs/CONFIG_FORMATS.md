# Config file formats

Metarepo supports JSON, YAML, and TOML for the workspace config file. The
format is detected from the filename — you never have to tell the tool which
format you're using.

## Recognized filenames

| Filename | Format | Notes |
|----------|--------|-------|
| `.metarepo` | JSON | New canonical name |
| `.metarepo.json` | JSON | Explicit JSON variant |
| `.metarepo.yaml` | YAML | |
| `.metarepo.yml` | YAML | |
| `.metarepo.toml` | TOML | |
| `.meta` | JSON | Legacy name, fully supported |

Discovery walks up from the current directory and uses the first match it
finds. If two or more recognized files exist in the same directory the tool
**errors out** rather than silently picking one — see "Multi-file conflicts"
below.

## Choosing a format at init

```bash
meta init                  # writes .metarepo (JSON)
meta init --format yaml    # writes .metarepo.yaml
meta init --format toml    # writes .metarepo.toml
```

If a recognized config already exists, `meta init` is idempotent — it leaves
the existing file alone regardless of the requested format.

## Overriding discovery

The global `--config <path>` flag (or `METAREPO_CONFIG` env var) bypasses
discovery and loads the supplied file directly:

```bash
meta --config ./tools/.metarepo.yaml git status
METAREPO_CONFIG=./tools/.metarepo.yaml meta git status
```

This is the recommended escape hatch when you have a multi-file conflict and
just want to run one command.

## Migrating between formats

```bash
meta config migrate yaml                # Write .metarepo.yaml next to current; keep original
meta config migrate toml --replace      # Migrate then delete the original
meta config migrate json --to .metarepo # Migrate to an explicit destination
meta config migrate yaml --force        # Overwrite an existing destination
```

`migrate` reads the active config (auto-discovered or supplied via
`--config`/`METAREPO_CONFIG`) and writes it back in the chosen format. By
default the original is preserved so you can verify the result before
removing it.

## Multi-file conflicts

If a directory contains two or more recognized config files (e.g., both
`.meta` and `.metarepo.yaml`), every metarepo command will refuse to run with
an error like:

```
multiple metarepo config files found in /path/to/dir:
  - .meta
  - .metarepo.yaml
Pick one of: pass --config <path>, run `meta config migrate` to consolidate,
or remove the unwanted file(s).
```

This is intentional — silently preferring one would mask aborted migrations or
git merge artifacts. To resolve:

- One-off: pass `--config <path>` for the current command.
- Permanent: run `meta config migrate <format>` and then delete the other.
- Manual: `rm` or `mv` the file you don't want.

## What sanitization runs at load time

Regardless of format, every loaded config goes through the same hardening:

- Project keys with path-traversal (`..`), null bytes, or absolute paths are
  dropped with a stderr warning.
- Environment variables known to subvert subprocesses (`LD_PRELOAD`,
  `BASH_ENV`, `NODE_OPTIONS`, etc.) are stripped.

See [security](./security/) for the full list.
