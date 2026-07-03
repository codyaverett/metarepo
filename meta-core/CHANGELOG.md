# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.54.0](https://github.com/codyaverett/metarepo/compare/metarepo-core-v0.52.0...metarepo-core-v0.54.0) - 2026-07-03

### Added

- *(git)* add shallow clone support via --depth flag
- *(skill)* steal and add skills from a specific git branch, tag, or commit (v0.53.0)

### Other

- *(deps)* bump toml from 0.8.23 to 1.1.2+spec-1.1.0
- *(security)* add supply-chain threat model document (v0.53.2)
- *(backlog)* add 2026-06-12 backlog grooming record (v0.53.1)

## [0.17.0](https://github.com/codyaverett/metarepo/compare/metarepo-core-v0.13.0...metarepo-core-v0.17.0) - 2026-05-14

### Added

- *(init)* idempotent meta init with
- *(worktree)* add repair command for moved
- *(worktree)* make commands context-aware
- *(security)* harden config and plugin trust boundaries (v0.14.0)

## [0.13.0](https://github.com/codyaverett/metarepo/compare/metarepo-core-v0.12.0...metarepo-core-v0.13.0) - 2026-04-23

### Added

- *(ci)* adopt release-plz for automated release PRs and publishing (v0.13.0)
- *(tests)* add security test suite against real metarepo APIs (v0.12.2)
- *(git)* add dirty-tree detection to meta git pull, skip repos with uncommitted changes (v0.12.1)
