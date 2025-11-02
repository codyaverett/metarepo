# Testing Recommendations

This document provides detailed, actionable recommendations for improving the metarepo testing strategy.

---

## Priority 1: Immediate Actions (This Week)

### 1. Set Up Code Coverage Tracking

**Effort:** 1 day
**Impact:** 10/10
**Risk Reduction:** Enables measurement and prevents regression

#### Implementation Steps

1. **Add coverage tool to dev dependencies:**

```toml
# Cargo.toml
[dev-dependencies]
# ... existing dependencies
```

2. **Update CI workflow:**

```yaml
# .github/workflows/ci.yml

name: CI

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  # ... existing test job

  coverage:
    name: Code Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Generate coverage
        run: |
          cargo tarpaulin \
            --workspace \
            --timeout 300 \
            --out Xml \
            --output-dir coverage \
            --exclude-files 'tests/*' 'benches/*'

      - name: Upload to codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./coverage/cobertura.xml
          fail_ci_if_error: false

      - name: Archive coverage results
        uses: actions/upload-artifact@v3
        with:
          name: code-coverage-report
          path: coverage/
```

3. **Add codecov configuration:**

```yaml
# codecov.yml
coverage:
  status:
    project:
      default:
        target: 70%
        threshold: 1%
    patch:
      default:
        target: 60%

comment:
  layout: "reach, diff, flags, files"
  behavior: default

ignore:
  - "tests/"
  - "benches/"
```

4. **Add coverage badges to README:**

```markdown
[![codecov](https://codecov.io/gh/yourusername/metarepo/branch/main/graph/badge.svg)](https://codecov.io/gh/yourusername/metarepo)
```

#### Success Criteria

- [ ] Coverage runs on every PR
- [ ] Baseline coverage established
- [ ] Coverage trends visible in PRs
- [ ] Team can see untested code

---

### 2. Add Git Operation Integration Tests

**Effort:** 3-4 days
**Impact:** 10/10
**Risk Reduction:** 70% of critical production issues

#### Create Test Infrastructure

1. **Create integration test directory:**

```bash
mkdir -p tests
touch tests/common/mod.rs
```

2. **Create test utilities:**

```rust
// tests/common/mod.rs
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use std::process::Command;

pub struct TestWorkspace {
    pub root: TempDir,
    pub meta_file: PathBuf,
}

impl TestWorkspace {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let meta_file = root.path().join(".meta");
        Self { root, meta_file }
    }

    pub fn init(&self) -> &Self {
        // Initialize .meta file
        std::fs::write(&self.meta_file, "{}").unwrap();
        self
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn create_test_git_repo(&self, name: &str) -> PathBuf {
        let repo_path = self.root.path().join(name);
        std::fs::create_dir_all(&repo_path).unwrap();

        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        repo_path
    }
}
```

#### Test Git Clone Operations

```rust
// tests/git_operations.rs
mod common;
use common::TestWorkspace;
use metarepo::plugins::git::clone_repository;

#[test]
fn test_clone_repository_https() {
    let workspace = TestWorkspace::new();
    let source = workspace.create_test_git_repo("source");
    let target = workspace.path().join("cloned");

    let result = clone_repository(
        source.to_str().unwrap(),
        &target,
    );

    assert!(result.is_ok());
    assert!(target.exists());
    assert!(target.join(".git").exists());
}

#[test]
fn test_clone_repository_invalid_url() {
    let workspace = TestWorkspace::new();
    let target = workspace.path().join("cloned");

    let result = clone_repository(
        "not-a-valid-url",
        &target,
    );

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("invalid") || error_msg.contains("failed"));
}

#[test]
fn test_clone_repository_nonexistent() {
    let workspace = TestWorkspace::new();
    let target = workspace.path().join("cloned");

    let result = clone_repository(
        "https://github.com/nonexistent/repo-that-does-not-exist-12345.git",
        &target,
    );

    assert!(result.is_err());
}

#[test]
fn test_clone_missing_repos() {
    let workspace = TestWorkspace::new().init();

    // Create source repos
    let repo1 = workspace.create_test_git_repo("repo1");
    let repo2 = workspace.create_test_git_repo("repo2");

    // Add projects to .meta (not yet cloned)
    let config = format!(
        r#"{{
            "projects": {{
                "repo1": "{}",
                "repo2": "{}"
            }}
        }}"#,
        repo1.to_str().unwrap(),
        repo2.to_str().unwrap()
    );
    std::fs::write(&workspace.meta_file, config).unwrap();

    // Test clone_missing_repos
    // TODO: Implement when function signature is confirmed
}

#[test]
fn test_git_status_aggregation() {
    let workspace = TestWorkspace::new().init();

    // Create repos with different states
    let clean_repo = workspace.create_test_git_repo("clean");
    let dirty_repo = workspace.create_test_git_repo("dirty");

    // Make dirty repo dirty
    std::fs::write(dirty_repo.join("newfile.txt"), "content").unwrap();

    // TODO: Test status aggregation when available
}
```

#### Test SSH Authentication

```rust
// tests/git_ssh_auth.rs
mod common;

#[test]
#[ignore] // Run separately as requires SSH setup
fn test_clone_with_ssh_key() {
    // Test SSH authentication with key file
    // Requires test environment with SSH keys set up
}

#[test]
#[ignore]
fn test_clone_with_ssh_agent() {
    // Test SSH authentication with agent
    // Requires ssh-agent running
}

#[test]
fn test_ssh_auth_fallback() {
    // Test fallback from agent to key file
}
```

#### Success Criteria

- [ ] Clone operations tested (HTTP/HTTPS)
- [ ] Error scenarios tested (invalid URL, auth failure)
- [ ] SSH authentication tested (with ignore flag for CI)
- [ ] Status aggregation tested
- [ ] Tests run in CI

---

## Priority 2: Short Term (Next 2 Weeks)

### 3. Add Project Management Tests

**Effort:** 2-3 days
**Impact:** 9/10
**Risk Reduction:** Prevents data loss and corruption

#### Test Project Creation

```rust
// tests/project_management.rs
mod common;

#[test]
fn test_project_create_workflow() {
    let workspace = TestWorkspace::new().init();

    // Create a source repository
    let source_repo = workspace.create_test_git_repo("source");
    let source_url = source_repo.to_str().unwrap();

    // Test: meta project create myproject <url>
    // TODO: Use actual project create API

    // Verify:
    // 1. .meta file updated
    // 2. Repository cloned to correct location
    // 3. Project is accessible

    let meta_content = std::fs::read_to_string(&workspace.meta_file).unwrap();
    assert!(meta_content.contains("myproject"));
    assert!(meta_content.contains(source_url));
}

#[test]
fn test_project_import_existing() {
    let workspace = TestWorkspace::new().init();

    // Create existing local repo
    let existing_repo = workspace.create_test_git_repo("existing");

    // Test: meta project import existing
    // TODO: Use actual project import API

    // Verify .meta updated without re-cloning
}

#[test]
fn test_project_rename() {
    let workspace = TestWorkspace::new().init();

    // Create project
    let repo = workspace.create_test_git_repo("original");

    // Add to .meta
    // Rename: original -> renamed
    // TODO: Use actual rename API

    // Verify:
    // 1. .meta updated with new name
    // 2. Old name no longer in .meta
    // 3. Project still accessible
}

#[test]
fn test_project_basename_resolution() {
    // Test resolving project by basename when multiple matches
}

#[test]
fn test_project_alias_resolution() {
    // Test resolving project by alias
}
```

---

### 4. Add Worktree Operation Tests

**Effort:** 2 days
**Impact:** 8/10
**Risk Reduction:** Validates v0.8.2 features

```rust
// tests/worktree_operations.rs
mod common;

#[test]
fn test_worktree_create() {
    let workspace = TestWorkspace::new();
    let main_repo = workspace.create_test_git_repo("main");

    // Create worktree
    // TODO: Use actual worktree API

    // Verify worktree created
    // Verify worktree is functional git repo
}

#[test]
fn test_worktree_create_with_post_create_hook() {
    let workspace = TestWorkspace::new().init();

    // Configure post-create hook in .meta
    // Create worktree
    // Verify hook executed
}

#[test]
fn test_bare_repository_conversion() {
    let workspace = TestWorkspace::new();
    let repo = workspace.create_test_git_repo("normal");

    // Convert to bare
    // TODO: Use conversion utility

    // Verify bare repository structure
    // Create worktree from bare
    // Verify worktree works
}

#[test]
fn test_worktree_cleanup() {
    // Create worktree
    // Remove worktree
    // Verify cleanup
    // Test prune
}
```

---

### 5. Add CLI Integration Tests

**Effort:** 2-3 days
**Impact:** 7/10
**Risk Reduction:** Validates user-facing behavior

#### Add Test Dependencies

```toml
# Cargo.toml
[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.0"
```

#### Create CLI Tests

```rust
// tests/cli_integration.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn test_meta_help() {
    Command::cargo_bin("meta")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Multi-Project Management Tool"));
}

#[test]
fn test_meta_version() {
    Command::cargo_bin("meta")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_meta_init_creates_file() {
    let temp = tempdir().unwrap();

    Command::cargo_bin("meta")
        .unwrap()
        .arg("init")
        .current_dir(&temp)
        .assert()
        .success();

    assert!(temp.path().join(".meta").exists());
}

#[test]
fn test_meta_init_existing_file() {
    let temp = tempdir().unwrap();
    std::fs::write(temp.path().join(".meta"), "{}").unwrap();

    Command::cargo_bin("meta")
        .unwrap()
        .arg("init")
        .current_dir(&temp)
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn test_meta_project_without_init() {
    let temp = tempdir().unwrap();

    Command::cargo_bin("meta")
        .unwrap()
        .arg("project")
        .arg("list")
        .current_dir(&temp)
        .assert()
        .failure()
        .stderr(predicate::str::contains(".meta"));
}
```

---

## Priority 3: Medium Term (Next Month)

### 6. Add End-to-End Workflow Tests

**Effort:** 3-4 days
**Impact:** 8/10

```rust
// tests/e2e_workflows.rs
mod common;

#[test]
fn test_complete_new_user_workflow() {
    let workspace = TestWorkspace::new();

    // Simulate complete workflow:
    // 1. meta init
    // 2. meta project create app1 <url>
    // 3. meta project create app2 <url>
    // 4. meta git status
    // 5. meta exec -- git pull
    // 6. Verify everything worked
}

#[test]
fn test_monorepo_migration_workflow() {
    // Simulate migrating from monorepo to meta
    // 1. Create existing projects
    // 2. meta init
    // 3. Import existing projects
    // 4. Verify structure
}

#[test]
fn test_team_onboarding_workflow() {
    // Simulate new team member cloning workspace
    // 1. Clone workspace with .meta
    // 2. meta git clone-missing
    // 3. Verify all projects cloned
}

#[test]
fn test_parallel_exec_workflow() {
    // Test parallel command execution across projects
    // 1. Create multiple projects
    // 2. Run parallel exec
    // 3. Verify all commands executed
}
```

---

### 7. Add Property-Based Testing

**Effort:** 2-3 days
**Impact:** 6/10

```toml
# Cargo.toml
[dev-dependencies]
proptest = "1.0"
```

```rust
// meta-core/src/lib.rs tests
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_config_serialization_roundtrip(
        projects in prop::collection::hash_map(".*", ".*", 0..10)
    ) {
        let mut config = MetaConfig::default();
        config.projects = projects;

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: MetaConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.projects, deserialized.projects);
    }

    #[test]
    fn test_project_name_validation(name in "[a-zA-Z0-9_-]{1,100}") {
        // Test that valid names are accepted
        // Test that invalid names are rejected
    }

    #[test]
    fn test_pattern_matching(
        pattern in "[a-z*?]{1,20}",
        name in "[a-z]{1,20}"
    ) {
        // Test glob pattern matching properties
        // e.g., if exact match works, pattern match should work
    }
}
```

---

### 8. Add Performance Benchmarks

**Effort:** 2 days
**Impact:** 6/10

```toml
# Cargo.toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "benchmarks"
harness = false
```

```rust
// benches/benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use metarepo::config::MetaConfig;
use metarepo::plugins::exec::iterator::ProjectIterator;

fn benchmark_project_iterator(c: &mut Criterion) {
    let config = create_config_with_n_projects(1000);

    c.bench_function("iterate 1000 projects", |b| {
        b.iter(|| {
            ProjectIterator::new(&config, Path::new("/tmp"))
                .collect::<Vec<_>>()
        });
    });
}

fn benchmark_project_filtering(c: &mut Criterion) {
    let config = create_config_with_n_projects(1000);

    c.bench_function("filter 1000 projects with pattern", |b| {
        b.iter(|| {
            ProjectIterator::new(&config, Path::new("/tmp"))
                .with_include_patterns(vec!["lib-*".to_string()])
                .collect::<Vec<_>>()
        });
    });
}

fn benchmark_config_load(c: &mut Criterion) {
    let temp = create_large_meta_file(1000);

    c.bench_function("load .meta with 1000 projects", |b| {
        b.iter(|| {
            MetaConfig::load(temp.path()).unwrap()
        });
    });
}

criterion_group!(
    benches,
    benchmark_project_iterator,
    benchmark_project_filtering,
    benchmark_config_load
);
criterion_main!(benches);
```

---

## Priority 4: Long Term (Next Quarter)

### 9. Create Testing Guidelines

Create `docs/qa/TESTING_GUIDELINES.md` with:

- Required test coverage levels per component
- Testing patterns and anti-patterns
- How to write good unit tests
- How to write integration tests
- How to mock Git operations
- CI/CD testing requirements
- Performance testing guidelines
- When to use which type of test

### 10. Establish TDD Workflow

For all new features:
1. Write integration test defining expected behavior
2. Write unit tests for components
3. Implement feature (tests fail)
4. Make tests pass
5. Refactor with test safety net
6. Verify coverage meets targets

### 11. Add Missing Test Types

- **Security Tests:** Path traversal, injection, secrets exposure
- **Concurrency Tests:** Race conditions, parallel operations
- **Error Recovery Tests:** Rollback, partial failures
- **Documentation Tests:** Validate examples in docs

---

## Testing Checklist for PRs

All PRs should include:

- [ ] Unit tests for new functions
- [ ] Integration tests for new features
- [ ] Updated existing tests if behavior changed
- [ ] All tests passing locally
- [ ] Coverage not decreased (or justified)
- [ ] Performance benchmarks if applicable
- [ ] Error scenarios tested

---

## Success Metrics

Track these metrics monthly:

| Metric | Current | Target |
|--------|---------|--------|
| Overall Coverage | 30-35% | 70% |
| Core Library Coverage | 75% | 90% |
| Critical Plugins Coverage | 0% | 70% |
| Integration Tests | 0 | 20+ |
| E2E Tests | 0 | 10+ |
| Performance Benchmarks | 0 | 10+ |
| PRs with Tests | ~40% | 100% |
| Test Execution Time | ~2s | <30s |

---

## Conclusion

These recommendations provide a roadmap from the current state (5/10) to a robust testing strategy (9/10). Focus on Priority 1 and 2 items first to address critical gaps, then gradually implement Priority 3 and 4 items to achieve comprehensive coverage.

The key is to start testing critical paths (Git, Project, Worktree) immediately while building infrastructure for long-term testing excellence.
