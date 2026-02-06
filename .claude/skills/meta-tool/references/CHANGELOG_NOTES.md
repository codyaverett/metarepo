# Meta CLI Changelog Notes

This document tracks version history and breaking changes for the meta CLI tool to help maintain the skill documentation.

## Version History

### v0.11.0 (Current)

**Release Type:** Minor release with CI/dev improvements

**Key Changes:**
- Resolved clippy warnings for CI compliance
- Added pre-commit hooks
- Security vulnerability fixes

**Breaking Changes:** None

### v0.10.6

**Key Changes:**
- Smart branch detection for worktree commands
- Added `--from` flag for `meta worktree add` to specify starting point for non-existent branches
- Improved worktree creation workflow

### v0.10.5

**Key Changes:**
- Removed `--output-format` flag from all commands
- Removed `--ai` flag from all commands
- Simplified command output handling

**Breaking Changes:**
- Removed: `--output-format` flag (previously supported json/text output)
- Removed: `--ai` flag (AI assistance features removed)

### Pre-v0.10.5

Earlier versions had different flag configurations. If you encounter documentation referencing `--output-format` or `--ai` flags, those are outdated.

---

## Deprecated/Removed Features

| Feature | Removed In | Replacement |
|---------|------------|-------------|
| `--output-format` | v0.10.5 | Use `meta config show --format` for config output |
| `--ai` | v0.10.5 | N/A - AI features removed |

---

## Experimental Feature Roadmap

| Feature | Introduced | Status | Notes |
|---------|------------|--------|-------|
| `rules` | v0.9.x | Experimental | Project structure enforcement |
| `plugin` | v0.9.x | Experimental | External plugin management via crates.io |
| `mcp` | v0.10.x | Experimental | Model Context Protocol server integration |

### When Experimental Features May Become Stable

Monitor these indicators:
- Feature completion in `meta/src/plugins/*/plugin.rs`
- Removal of `is_experimental(&self) -> bool { true }` in plugin implementations
- Changelog announcements of stabilization

---

## Testing Coverage Notes

The following commands have dedicated tests:
- `meta init` - Basic initialization
- `meta git clone/status/update` - Git operations
- `meta project add/list/remove` - Project management
- `meta exec` - Command execution
- `meta worktree` - Worktree operations

Tests are located in:
- `meta/src/plugins/*/plugin.rs` (inline `#[cfg(test)]` modules)
- Integration tests in `meta/tests/`

---

## V1 Considerations

When approaching v1.0.0, watch for:

1. **Stabilization of experimental features**
   - `rules` plugin may become stable
   - `plugin` manager may become stable
   - `mcp` integration may become stable

2. **API stability guarantees**
   - Flag names and behaviors should be locked
   - `.meta` file format should be finalized

3. **Potential breaking changes before v1**
   - Configuration format changes
   - Command structure reorganization
   - Flag renaming for consistency

---

## Updating This Skill

### When to Update

1. **After any version bump** in `meta/Cargo.toml`
2. **When new commands are added** to any plugin
3. **When flags are modified** (added, removed, or behavior changed)
4. **When experimental features stabilize**

### Update Checklist

1. [ ] Update version in `SKILL.md` frontmatter
2. [ ] Review each plugin's `plugin.rs` for command changes
3. [ ] Check `cli.rs` for global flag changes
4. [ ] Update command reference in `SKILL.md`
5. [ ] Add version entry to this changelog
6. [ ] Test trigger phrases still work

### Critical Source Files

| File | Contains |
|------|----------|
| `meta/Cargo.toml` | Version number |
| `meta/src/cli.rs` | Global flags, experimental detection |
| `meta/src/plugins/init/plugin.rs` | `init` command |
| `meta/src/plugins/git/plugin.rs` | `git clone/status/update` |
| `meta/src/plugins/project/plugin.rs` | `project add/list/remove/rename/etc` |
| `meta/src/plugins/exec/plugin.rs` | `exec` command |
| `meta/src/plugins/run/plugin.rs` | `run` command |
| `meta/src/plugins/config/plugin.rs` | `config` command |
| `meta/src/plugins/worktree/plugin.rs` | `worktree` command |
| `meta/src/plugins/rules/plugin.rs` | `rules` command (experimental) |
| `meta/src/plugins/plugin_manager/plugin.rs` | `plugin` command (experimental) |
| `meta/src/plugins/mcp/plugin.rs` | `mcp` command (experimental) |
