# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.55.2](https://github.com/codyaverett/metarepo/compare/v0.52.0...v0.55.2) - 2026-07-03

### Added

- *(git)* add --shallow flag to meta git pull
- *(git)* add shallow clone support via --depth flag
- *(skill)* steal and add skills from a specific git branch, tag, or commit (v0.53.0)

### Fixed

- *(git)* re-truncate shallow history after pull instead of before

### Other

- *(security)* upgrade anyhow to 1.0.103 for RUSTSEC-2026-0190
- *(deps)* bump git2 from 0.18.3 to 0.21.0
- *(deps)* bump sha2 from 0.10.9 to 0.11.0
- *(deps)* bump toml from 0.8.23 to 1.1.2+spec-1.1.0
- *(deps)* bump colored from 2.2.0 to 3.1.1
- *(security)* add supply-chain threat model document (v0.53.2)
- *(backlog)* add 2026-06-12 backlog grooming record (v0.53.1)

## [0.17.0](https://github.com/codyaverett/metarepo/compare/v0.13.0...v0.17.0) - 2026-05-14

### Added

- *(init)* idempotent meta init with
- *(worktree)* add repair command for moved
- *(worktree)* make commands context-aware
- *(security)* harden config and plugin trust boundaries (v0.14.0)

## [0.13.0](https://github.com/codyaverett/metarepo/compare/v0.12.0...v0.13.0) - 2026-04-23

### Added

- *(ci)* adopt release-plz for automated release PRs and publishing (v0.13.0)
- *(tests)* add security test suite against real metarepo APIs (v0.12.2)
- *(git)* add dirty-tree detection to meta git pull, skip repos with uncommitted changes (v0.12.1)

### Fixed

- *(ci)* resolve CI and security
