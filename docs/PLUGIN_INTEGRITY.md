# Plugin Version Pinning & Checksum Integrity

Status: implemented (issue #25, part of #21)

This document describes how metarepo enforces that an installed external plugin
matches what the workspace declared — both its **version** and, optionally, its
**exact bytes**.

## Motivation

Before this feature the `version` field on each `plugins.<name>` entry in
`.metarepo` was cosmetic: plugin loading only checked that the binary *existed*,
never what version it reported or whether it had been modified after install.
Two problems followed:

1. A pinned version (`crates:metarepo-plugin-foo@1.2.3`) was not actually
   honored — any installed version loaded silently.
2. A binary swapped out on disk after install (tampering, or an unrelated
   `cargo install` overwriting `~/.cargo/bin`) loaded without any check.

## Two layers

### 1. Version enforcement (always on)

When a plugin is loaded, its self-reported version (from the protocol `Info`
handshake, or the manifest's `[plugin].version`) is compared against the version
declared by its spec in `.metarepo`:

- **`crates:<crate>@X.Y.Z`** — the reported version must satisfy the declared
  version interpreted as a semver requirement. A bare `X.Y.Z` is treated as a
  caret requirement (`^X.Y.Z`), matching Cargo's default; explicit requirements
  (`=1.2.3`, `>=1.2, <2.0`) are honored as written.
- **`crates:<crate>`** (no version), **`file:`**, **`git+`** — no version is
  declared, so there is nothing to enforce and the check is skipped. (A `file:`
  or `git+` plugin's identity is better protected by the checksum layer below.)

On mismatch the plugin **fails to load**: its commands are not registered and a
clear, actionable error is printed. The rest of the CLI keeps working so the
user can fix the pin or reinstall. Pass `--allow-version-mismatch` (or set
`METAREPO_ALLOW_VERSION_MISMATCH=1`) to downgrade the error to a warning and
load the plugin anyway.

Because plugins are loaded *before* the command line is parsed (their commands
have to exist for `--help` and routing), the override is detected by scanning
the raw arguments and the environment variable, in addition to being registered
as a global flag so it parses cleanly.

### 2. Checksum integrity (opt-in)

Opt in per workspace by setting in `.metarepo`:

```toml
plugins-integrity = "required"
```

(Values: `off` — the default — or `required`.)

When `required`, `meta plugin install` / `update` records the SHA-256 of the
resolved binary in a sibling **`.metarepo.lock`** file. On load, the binary's
SHA-256 is recomputed and compared:

- match → load proceeds;
- mismatch → the plugin is refused (hard error, no override — a changed binary
  under `required` is exactly what this mode exists to catch);
- missing lockfile entry while `required` → refused, with a hint to reinstall so
  the checksum is recorded.

When integrity is `off`, checksums are still *recorded* on install (cheap, and
makes turning the mode on later seamless) but never *enforced* on load.

## Installing, updating, and re-pinning

- `meta plugin install <name> [--version X]` installs and records the spec plus
  a lockfile entry. The recorded version is the **actual** version the installed
  plugin reports (read from the manifest, or probed from the binary), not just
  the declared pin — so even an unpinned `meta plugin install foo` captures a
  concrete version rather than `*`.
- `meta plugin update <name>` reinstalls from the recorded spec and refreshes
  the lockfile, printing the version change (`old → new`).
- `meta plugin update <name> --version X` re-pins a crates.io plugin to `X`
  (rewriting the spec in `.metarepo`) and then updates. Re-pinning a
  `file:`/`git+` plugin is rejected, since those carry no crates version.

## On-demand verification & reporting

- `meta plugin verify [name]` recomputes the SHA-256 of each installed plugin
  binary and compares it to `.metarepo.lock`. It exits non-zero if any plugin's
  checksum does not match, so it works as a CI gate. With a `name` it checks a
  single plugin; without one it checks all. Plugins with no recorded checksum
  are reported but do not fail the run.
- `meta plugin list` annotates each entry with its integrity state. A checksum
  **MISMATCH** is always surfaced (tampering matters regardless of mode); the
  `ok` / `not recorded` / `unverifiable` states are shown only when the
  workspace enforces integrity, to keep output quiet otherwise.

## When the check happens (load-time, not per-run)

Enforcement runs when plugins are loaded — which, for this CLI, is immediately
before any command is dispatched. A plugin that fails the version or checksum
check is never registered, so its command cannot be invoked: loading *is* the
pre-run gate. A separate re-check inside each command dispatch was considered
and deliberately skipped — `meta` loads then runs within a single short-lived
process, so a re-hash on every invocation would add cost (hashing the binary
each time) for no meaningful gain over the load-time check and the cross-run
protection the lockfile already provides. `meta plugin verify` covers the
explicit, on-demand case.

## Lockfile format (`.metarepo.lock`)

TOML, sitting next to the `.metarepo` config file. One entry per installed
plugin:

```toml
[plugins.foo]
version = "1.2.3"          # actual version the plugin reported at install time
source = "crates:metarepo-plugin-foo@1.2.3"
sha256 = "9f86d0818..."     # hex digest of the resolved binary

[plugins.greet]
version = "0.1.0"
source = "file:/Users/me/.config/metarepo/plugins/greet/plugin.manifest.toml"
sha256 = "2c26b46b6..."     # digest of the manifest's resolved binary
```

The lockfile is managed automatically:

- `meta plugin install <name>` / `update` — writes/refreshes the entry.
- `meta plugin remove <name>` — drops the entry.

It is safe to commit `.metarepo.lock` to version control; doing so is what lets
`plugins-integrity = "required"` protect every clone of the workspace.

## Dependencies

This feature adds two well-vetted crates:

- [`sha2`](https://crates.io/crates/sha2) (RustCrypto) — SHA-256 digests.
- [`semver`](https://crates.io/crates/semver) — the same crate Cargo uses for
  version requirement matching.

## Testing

- Unit: semver matching across `X.Y.Z`, `=`, ranges, and the skip cases.
- Integration: a pinned version that the plugin does not satisfy fails to load;
  a binary mutated after install fails the checksum check when integrity is
  `required`; `--allow-version-mismatch` loads with a warning; `meta plugin
  verify` exits non-zero on tamper; `meta plugin list` surfaces a mismatch even
  when integrity is not enforced.
