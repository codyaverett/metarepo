# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.52.0](https://github.com/codyaverett/metarepo/compare/v0.51.0...v0.52.0) - 2026-06-11

### Added

- *(skill)* refuse to clobber modified or newer installed skills on update (v0.52.0)

### Fixed

- *(ci)* replace mem forget with TempDir keep in module enable tests (v0.51.2)
- *(plugins)* resolve Windows executable extensions for plugin binaries (v0.51.1)

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
