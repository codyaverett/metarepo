# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.78.0] - 2026-09-03

### Added

- *(plugin)* takeover commands (protocol 1.3): external plugin commands declared with takeover are launched by re-invoking the plugin binary directly (exec on Unix) with argv and METAREPO_PLUGIN_CONFIG instead of wire dispatch, so long-running servers and TUIs can own stdin/stdout; groundwork for extracting the mcp plugin (#137)
- *(plugin)* takeover commands accept raw trailing arguments (flags included) and pass them through to the plugin binary verbatim, so external plugins keep their full clap surfaces

### Changed

- *(workspace)* this repository now dogfoods both extracted plugins: rules and mcp are version-pinned under plugins in .meta and resolved from ~/.cargo/bin; CI installs them with cargo install --path and smoke-tests the wire protocol and takeover exec dispatch through the meta binary on every push (#138)

- *(rules)* the rules plugin moved out of the meta binary into the external metarepo-plugin-rules crate (plugins/metarepo-plugin-rules), graduating it from experimental: install it and register it under plugins in .meta, then use meta rules without -x; every command is declared takeover so the plugin keeps its full clap surface, and the binary also runs standalone in any workspace (#132, #136)
- *(mcp)* the mcp plugin moved out of the meta binary into the external metarepo-plugin-mcp crate (plugins/metarepo-plugin-mcp), graduating it from experimental: install it and register it under plugins in .meta, then use meta mcp without -x; every command is declared takeover so serve owns stdin/stdout for the MCP stdio transport, and the binary also runs standalone (MCP client configs can point at it directly); generated client config blocks no longer emit -x (#132, #137)

### Packaging

- *(release)* metarepo-plugin-rules and metarepo-plugin-mcp publish to crates.io for the first time at 0.78.0; the plugin manager resolves `"rules": "0.78.0"` / `"mcp": "0.78.0"` pins from there (earlier pin examples referencing 0.77.0 never resolved because those crates were unpublished)

## [0.77.0] - 2026-07-30

### Added

- *(git)* `meta git push` — fan-out push with upstream preflight, bare worktree expansion, parallel by default
- *(git)* `meta git fetch` — fan-out fetch (bare roots, no dirty skip)
- *(git)* `meta git checkout` / `switch` — branch switch across repos with optional `-b/--create`, dirty skip
- *(docs)* dual-product identity document (`docs/PRODUCT.md`) and README surface split
- *(docs)* archive historical implementation plan under `docs/history/`

### Changed

- *(git)* shared fan-out preflight for pull/push/fetch/checkout (clean/upstream/bare policies)

## [0.76.0] - 2026-07-29 (approx)

Skill and config polish through mid/late July releases. Notable themes in the
0.54–0.76 band (not every micro-release listed):

### Added (summary of 0.54–0.76)

- *(project)* `meta project check` workspace hygiene drift; nested `project init`
- *(config)* cascade writes and run-script cascade; enum/choices picker; security toggles via config
- *(status)* interactive multi-repo dashboard with inline fetch/pull
- *(worktree)* interactive worktree manager TUI
- *(run)* interactive script picker with live per-project output
- *(tui)* shared tree-shell and keybindings in meta-core
- *(skill)* configurable audit rules, dest roots; honor `[skill] dest` for bundled skill
- *(git)* shallow clone (`--depth`) and `pull --shallow` re-truncation
- *(config)* disable projects via `enabled` / disabled list

### Fixed

- *(config)* preserve unknown top-level `.meta` blocks for external plugin settings
- *(git)* shallow re-truncate after pull (not before) to avoid divergent-branch failures
- *(plugins)* Windows executable extension resolution for plugin binaries

## [0.53.x] - 2026-06

### Added

- *(security)* supply-chain threat model document
- *(skill)* steal/add from git ref; refuse clobber of modified skills on update
- *(mcp)* gateway phases: workspace pin, progressive meta-tools, tool promotion, allowlist hosting

## [0.42.0 – 0.51.0] - 2026-06

### Added

- *(config)* nested config cascade (reads); catalog-driven config TUI CRUD
- *(plugin)* subprocess plugins declare settings over protocol
- *(help)* man-page-style `helpDescription` sections
- *(module)* `meta module` system bundling plugins and skills
- *(skill)* scan/audit/locations/steal; skills.sh search/add

## [0.27.0 – 0.41.0] - 2026-05 – 2026-06

### Added

- *(skill)* full Claude skill lifecycle tooling
- *(config)* extensible plugin settings via `meta config list/get/set`
- *(scoping)* directory-aware scope, `--workspace` / `--root` (epic)

## [0.20.0 – 0.26.0] - 2026-05

### Added

- *(plugin)* protocol v1, metarepo-plugin-sdk, manifest plugins, cross-language templates
- *(plugin)* install/list/remove/update, version pins and integrity lockfile
- *(init)* multi-format config (JSON/YAML/TOML), idempotent init
- *(worktree)* context-aware commands, path repair

## [0.17.0](https://github.com/codyaverett/metarepo/compare/v0.13.0...v0.17.0) - 2026-05-14

### Added

- *(init)* idempotent meta init
- *(worktree)* repair command for moved worktrees; context-aware commands
- *(security)* harden config and plugin trust boundaries (v0.14.0)

## [0.13.0](https://github.com/codyaverett/metarepo/compare/v0.12.0...v0.13.0) - 2026-04-23

### Added

- *(ci)* release-plz for automated release PRs and publishing
- *(tests)* security test suite against real metarepo APIs (v0.12.2)
- *(git)* dirty-tree detection on `meta git pull` (v0.12.1)

### Fixed

- *(ci)* resolve CI and security workflow issues

---

Older releases: see git tags (`v0.2.0` … `v0.13.0`). Detailed per-commit history
is available via `git log` and GitHub releases.
