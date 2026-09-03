# Plan: Externalize rules and mcp as managed external plugins

Status: done (shipped in 0.78.0, 2026-09-03; phases 1-4 complete, see meta/CHANGELOG.md)
Related issues: #132 (graduate or remove experimental plugins), #133 (dogfood a real meta module)
Date: 2026-07-30

## Goal

End the permanent `-x` limbo for the `rules` and `mcp` built-in plugins by
extracting them into external plugin crates served over the v1 stdio protocol
(`metarepo-plugin-sdk`), installed and integrity-pinned via the plugin
manager, and enabled per-workspace in `.meta` `plugins:{}`. This resolves #132
(they graduate to stable-but-opt-in) and #133 (the external plugin path gets a
real daily-use consumer) at the same time.

## Current state

- `meta/src/plugins/rules/` and `meta/src/plugins/mcp/` are compiled into the
  `meta` binary and gated at runtime by `-x/--experimental`
  (`meta/src/cli.rs`, `plugin_loader.rs`).
- `metarepo-plugin-sdk` owns the v1 stdio wire protocol; authors implement one
  `Plugin` trait and call `serve`.
- `meta/src/plugins/plugin_manager/` provides install, lockfile, spec, and
  checksum/version verification (`docs/PLUGIN_INTEGRITY.md`).
- This workspace's `.meta` has an empty `plugins:{}`; the external path is
  unproven in daily use.

## Approach

Extract in-repo first: new workspace member crates, one per plugin, published
like the other crates. Same repo keeps CI simple and avoids a premature repo
split; they can move out later without changing the install story.

- `plugins/metarepo-plugin-rules/` - move `meta/src/plugins/rules/*` behind
  the SDK `Plugin` trait; binary target `metarepo-plugin-rules`.
- `plugins/metarepo-plugin-mcp/` - same for `meta/src/plugins/mcp/*`.
- Shared logic that both the core and plugins need moves down into
  `meta-core` (or stays duplicated temporarily if trivial).

## Phases

### Phase 1: Extract rules
1. Scaffold `plugins/metarepo-plugin-rules` (plugin manager `scaffold` can
   generate the skeleton - dogfood it).
2. Move rules engine/config/validators code; adapt clap surface to
   `CommandInfo` declarations.
3. Delete the built-in registration; `meta -x rules` prints a pointer to the
   install command for one release, then is removed.
4. Register in this workspace's `.meta` `plugins:{}` with pinned version and
   checksum; verify `meta rules ...` round-trips through the stdio protocol.

### Phase 2: Extract mcp
Same steps for `mcp`. Note the mcp plugin runs a long-lived server
(`mcp_server.rs`); confirm the v1 protocol supports long-running commands or
extend the SDK first (spike early - this is the main technical risk).

### Phase 3: Dogfood and document (closes #133)
1. Keep both plugins enabled in this workspace's `.meta`; CI job installs via
   the plugin manager and smoke-tests both commands.
2. Document the end-to-end workflow (install, pin, verify, upgrade) in
   `docs/PLUGIN_DEVELOPMENT_V2.md` / `docs/PLUGIN_INTEGRITY.md`.
3. Update `docs/PRODUCT.md` dual-surface policy: built-ins are stable core;
   experimental features live as external plugins, not behind `-x`.

### Phase 4: Cleanup
1. Remove `-x` gating for these two plugins entirely; decide whether the
   `-x` flag itself survives.
2. Changelog, version bumps, release with the standard flow.

## Risks

- Long-running mcp server over stdio protocol may need SDK protocol work
  (spike in Phase 2 before committing).
- Clap-rich subcommand surfaces must map onto `CommandInfo`; if the SDK's
  command model is too flat, extend the SDK rather than flattening UX.
- Install-from-workspace vs install-from-crates.io during development: plugin
  manager spec should support a local path source for dogfooding.

## Definition of done

- `meta rules` and `meta mcp` work without `-x`, served by external plugin
  binaries pinned in `.meta`.
- No rules/mcp code remains compiled into the `meta` binary.
- CI proves the install/verify/run path on every push.
- #132 and #133 closed.
