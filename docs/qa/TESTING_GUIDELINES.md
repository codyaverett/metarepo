# Testing Guidelines

This document provides practical guidelines for writing and maintaining tests in the metarepo project.

---

## Testing Philosophy

**Core Principles:**
1. **Test critical paths first** - Focus on user-facing workflows
2. **Test behavior, not implementation** - Tests should survive refactoring
3. **Fast feedback** - Unit tests must be fast; integration tests should be reasonable
4. **Isolation** - Tests should not depend on each other
5. **Clarity** - Test names should describe what they test
6. **Maintainability** - Tests should be as easy to maintain as production code

---

## Test Types and When to Use Them

### Unit Tests (70% of tests)

**Purpose:** Test individual functions and modules in isolation

**When to use:**
- Testing pure functions
- Testing data structures and methods
- Testing business logic
- Testing edge cases and boundary conditions

**Characteristics:**
- Fast (< 10ms per test)
- No I/O operations (use mocks/fakes)
- No external dependencies
- Deterministic (same input → same output)

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_name_validation_valid() {
        assert!(validate_project_name("my-project").is_ok());
        assert!(validate_project_name("my_project").is_ok());
        assert!(validate_project_name("MyProject").is_ok());
    }

    #[test]
    fn test_project_name_validation_invalid() {
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name("my project").is_err());
        assert!(validate_project_name("my/project").is_err());
    }
}
```

---

### Integration Tests (20% of tests)

**Purpose:** Test how multiple components work together

**When to use:**
- Testing plugin interactions
- Testing Git operations with real repositories
- Testing file system operations
- Testing configuration loading and saving

**Characteristics:**
- Slower than unit tests (< 1s per test)
- May use real file system (in temp directories)
- May use real Git repositories (local)
- Test cross-component behavior

**Location:** `/tests` directory

**Example:**
```rust
// tests/git_operations.rs
mod common;

#[test]
fn test_clone_and_update_workflow() {
    let workspace = common::TestWorkspace::new();
    let source = workspace.create_test_git_repo("source");

    // Clone repository
    let cloned = workspace.path().join("cloned");
    clone_repository(source.to_str().unwrap(), &cloned).unwrap();

    // Make change in source
    workspace.commit_to_repo(&source, "new-file.txt", "content");

    // Update cloned repository
    update_repository(&cloned).unwrap();

    // Verify change pulled
    assert!(cloned.join("new-file.txt").exists());
}
```

---

### End-to-End Tests (10% of tests)

**Purpose:** Test complete user workflows from CLI

**When to use:**
- Testing complete user scenarios
- Testing CLI output and error messages
- Testing multi-step workflows
- Validating user documentation examples

**Characteristics:**
- Slowest tests (1-10s per test)
- Use actual CLI binary
- Test complete workflows
- Most realistic user simulation

**Example:**
```rust
// tests/e2e_workflows.rs
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_new_user_onboarding() {
    let workspace = common::TestWorkspace::new();

    // Initialize workspace
    Command::cargo_bin("meta")
        .unwrap()
        .arg("init")
        .current_dir(workspace.path())
        .assert()
        .success();

    // Create project
    Command::cargo_bin("meta")
        .unwrap()
        .args(["project", "create", "myapp", "https://github.com/user/repo.git"])
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created project"));

    // Verify project in .meta
    let meta = std::fs::read_to_string(workspace.meta_file()).unwrap();
    assert!(meta.contains("myapp"));
}
```

---

## Coverage Guidelines

### Overall Targets

| Component Type | Minimum | Target | Rationale |
|---------------|---------|--------|-----------|
| Core Library | 80% | 90% | Foundation for everything |
| Critical Plugins (Git, Project, Exec) | 70% | 80% | Business critical |
| Standard Plugins | 60% | 70% | Important but less critical |
| CLI Layer | 50% | 60% | Thin layer, mostly integration |
| Utilities | 70% | 80% | Reused across codebase |

### What to Always Test

✅ **Must be tested:**
- Public API functions
- Error handling paths
- Data validation
- State transitions
- Business logic
- Security-sensitive code
- Data persistence

⚠️ **Should be tested:**
- Private helper functions (if complex)
- Configuration parsing
- File I/O operations
- String formatting

❌ **Can skip:**
- Trivial getters/setters
- Direct pass-through functions
- Generated code
- Third-party library wrappers (test integration, not library)

---

## Test Organization

### File Structure

```
metarepo/
├── meta/
│   ├── src/
│   │   ├── plugins/
│   │   │   ├── git/
│   │   │   │   ├── mod.rs
│   │   │   │   └── #[cfg(test)] mod tests {...}
│   │   │   └── ...
│   │   └── ...
│   └── tests/           # Integration tests
│       ├── common/
│       │   └── mod.rs    # Shared test utilities
│       ├── git_operations.rs
│       ├── project_management.rs
│       └── e2e_workflows.rs
├── benches/             # Performance benchmarks
│   └── benchmarks.rs
└── tests/               # Workspace-level integration tests
```

### Test Module Organization

```rust
// For unit tests in src/plugins/git/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // Helper functions (private to test module)
    fn create_test_repo() -> PathBuf {
        // ...
    }

    // Group related tests
    mod clone_tests {
        use super::*;

        #[test]
        fn test_clone_https_url() { }

        #[test]
        fn test_clone_ssh_url() { }

        #[test]
        fn test_clone_invalid_url() { }
    }

    mod authentication_tests {
        use super::*;

        #[test]
        fn test_ssh_key_auth() { }

        #[test]
        fn test_ssh_agent_auth() { }
    }
}
```

---

## Naming Conventions

### Test Function Names

**Pattern:** `test_<what>_<scenario>`

**Good examples:**
```rust
#[test]
fn test_clone_repository_with_https_url() { }

#[test]
fn test_clone_repository_with_invalid_url_returns_error() { }

#[test]
fn test_project_iterator_filters_by_pattern() { }

#[test]
fn test_config_load_when_file_missing_returns_error() { }
```

**Bad examples:**
```rust
#[test]
fn test1() { }                    // Not descriptive

#[test]
fn test_clone() { }               // Too vague

#[test]
fn it_works() { }                 // Doesn't describe what works

#[test]
fn test_clone_repository_works_correctly() { }  // "works correctly" is redundant
```

### Test Module Names

```rust
// Group related tests
mod validation_tests { }
mod serialization_tests { }
mod error_handling_tests { }

// Or by component
mod iterator_tests { }
mod builder_tests { }
mod parser_tests { }
```

---

## Test Structure

### AAA Pattern (Arrange-Act-Assert)

**Always use:**
1. **Arrange:** Set up test data and dependencies
2. **Act:** Execute the code being tested
3. **Assert:** Verify the results

```rust
#[test]
fn test_project_create() {
    // Arrange
    let workspace = TestWorkspace::new();
    let project_name = "myproject";
    let project_url = "https://github.com/user/repo.git";

    // Act
    let result = create_project(
        workspace.path(),
        project_name,
        project_url,
    );

    // Assert
    assert!(result.is_ok());
    assert!(workspace.meta_contains(project_name));
    assert!(workspace.project_dir(project_name).exists());
}
```

### Use Descriptive Assertions

```rust
// Good - Clear what failed
assert_eq!(projects.len(), 3, "Expected 3 projects but found {}", projects.len());
assert!(project_exists("myapp"), "Project 'myapp' should exist in .meta");

// Better - Use custom assertion helpers
assert_project_exists(&workspace, "myapp");
assert_meta_contains(&workspace, "myapp", "https://github.com/user/repo.git");

// Bad - Unclear what failed
assert!(x);
assert_eq!(a, b);
```

---

## Test Isolation

### Each Test Must Be Independent

**DO:**
```rust
#[test]
fn test_a() {
    let workspace = TestWorkspace::new(); // Fresh workspace
    // Test in isolation
}

#[test]
fn test_b() {
    let workspace = TestWorkspace::new(); // Fresh workspace
    // Test in isolation
}
```

**DON'T:**
```rust
static mut SHARED_STATE: Option<Workspace> = None;

#[test]
fn test_a() {
    unsafe { SHARED_STATE = Some(create_workspace()); }
    // Modifies shared state
}

#[test]
fn test_b() {
    // Depends on test_a running first - BAD!
    unsafe { SHARED_STATE.as_ref().unwrap() }
}
```

### Use Temporary Directories

```rust
use tempfile::tempdir;

#[test]
fn test_with_filesystem() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("test.txt");

    // Test operations
    std::fs::write(&test_file, "content").unwrap();

    // Temp directory automatically cleaned up when dropped
}
```

---

## Testing Error Cases

### Test Both Happy and Unhappy Paths

```rust
// Happy path
#[test]
fn test_parse_valid_url() {
    let url = "https://github.com/user/repo.git";
    assert!(parse_git_url(url).is_ok());
}

// Unhappy paths - test different error scenarios
#[test]
fn test_parse_empty_url() {
    assert!(parse_git_url("").is_err());
}

#[test]
fn test_parse_invalid_protocol() {
    let result = parse_git_url("ftp://example.com/repo.git");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unsupported protocol"));
}

#[test]
fn test_parse_malformed_url() {
    assert!(parse_git_url("not a url").is_err());
}
```

### Test Boundary Conditions

```rust
#[test]
fn test_project_name_length_boundaries() {
    // Empty (minimum boundary)
    assert!(validate_project_name("").is_err());

    // Single character (minimum valid)
    assert!(validate_project_name("a").is_ok());

    // Maximum length (if defined)
    let max_name = "a".repeat(255);
    assert!(validate_project_name(&max_name).is_ok());

    // Over maximum
    let too_long = "a".repeat(256);
    assert!(validate_project_name(&too_long).is_err());
}
```

---

## Mocking and Test Doubles

### When to Mock

**Mock when:**
- External service calls (GitHub API)
- Network operations
- Slow operations (large file I/O)
- Non-deterministic operations (time, randomness)

**Don't mock when:**
- Testing integration
- Operation is fast enough
- Operation is deterministic
- Real implementation is simple

### Example: Mocking Git Operations

```rust
// Define trait for testability
trait GitOps {
    fn clone(&self, url: &str, path: &Path) -> Result<()>;
    fn pull(&self, path: &Path) -> Result<()>;
}

// Real implementation
struct RealGitOps;
impl GitOps for RealGitOps {
    fn clone(&self, url: &str, path: &Path) -> Result<()> {
        // Real git clone
    }
}

// Test double
struct MockGitOps {
    clone_calls: RefCell<Vec<(String, PathBuf)>>,
}

impl GitOps for MockGitOps {
    fn clone(&self, url: &str, path: &Path) -> Result<()> {
        self.clone_calls.borrow_mut().push((url.to_string(), path.to_path_buf()));
        Ok(())
    }
}

#[test]
fn test_clone_missing_with_mock() {
    let git_ops = MockGitOps::new();
    clone_missing_repos(&git_ops, &config);

    assert_eq!(git_ops.clone_calls.borrow().len(), 3);
}
```

---

## Performance Testing

### When to Add Benchmarks

Add benchmarks for:
- Operations called frequently
- Operations in critical paths
- Operations with performance requirements
- Operations likely to regress

### Writing Benchmarks

```rust
// benches/benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_project_iteration(c: &mut Criterion) {
    let config = create_config_with_projects(1000);

    c.bench_function("iterate 1000 projects", |b| {
        b.iter(|| {
            // Use black_box to prevent compiler optimization
            ProjectIterator::new(black_box(&config), black_box(Path::new("/tmp")))
                .count()
        });
    });
}

fn benchmark_with_setup(c: &mut Criterion) {
    c.bench_function("parse large .meta", |b| {
        b.iter_batched(
            || create_large_meta_file(1000), // Setup
            |meta_file| parse_meta_file(&meta_file), // Operation
            criterion::BatchSize::SmallInput
        );
    });
}

criterion_group!(benches, benchmark_project_iteration, benchmark_with_setup);
criterion_main!(benches);
```

---

## Testing Patterns

### Table-Driven Tests

```rust
#[test]
fn test_url_validation() {
    let test_cases = vec![
        ("https://github.com/user/repo.git", true),
        ("git@github.com:user/repo.git", true),
        ("http://example.com/repo.git", true),
        ("invalid-url", false),
        ("", false),
        ("ftp://example.com/repo.git", false),
    ];

    for (url, expected_valid) in test_cases {
        let result = validate_git_url(url);
        assert_eq!(
            result.is_ok(),
            expected_valid,
            "URL '{}' validation failed", url
        );
    }
}
```

### Builder Pattern for Test Data

```rust
// tests/common/builders.rs
pub struct ConfigBuilder {
    projects: HashMap<String, String>,
    settings: HashMap<String, String>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
            settings: HashMap::new(),
        }
    }

    pub fn with_project(mut self, name: &str, url: &str) -> Self {
        self.projects.insert(name.to_string(), url.to_string());
        self
    }

    pub fn with_setting(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_string(), value.to_string());
        self
    }

    pub fn build(self) -> MetaConfig {
        MetaConfig {
            projects: self.projects,
            settings: self.settings,
            ..Default::default()
        }
    }
}

// Usage in tests
#[test]
fn test_with_builder() {
    let config = ConfigBuilder::new()
        .with_project("app1", "https://github.com/user/app1.git")
        .with_project("app2", "https://github.com/user/app2.git")
        .with_setting("parallel", "true")
        .build();

    // Test with config
}
```

---

## Common Anti-Patterns to Avoid

### ❌ Testing Implementation Details

```rust
// Bad - Tests internal implementation
#[test]
fn test_internal_cache_structure() {
    let parser = Parser::new();
    assert_eq!(parser.internal_cache.len(), 0); // Testing internal state
}

// Good - Tests behavior
#[test]
fn test_parser_caches_results() {
    let parser = Parser::new();
    parser.parse("input");
    parser.parse("input"); // Second call should be faster
    // Measure behavior, not implementation
}
```

### ❌ Testing Multiple Things in One Test

```rust
// Bad
#[test]
fn test_everything() {
    test_clone();
    test_pull();
    test_push();
    test_status();
}

// Good - Separate tests
#[test]
fn test_clone() { }

#[test]
fn test_pull() { }
```

### ❌ Excessive Mocking

```rust
// Bad - Mocking everything
#[test]
fn test_with_too_many_mocks() {
    let mock_fs = MockFilesystem::new();
    let mock_git = MockGit::new();
    let mock_config = MockConfig::new();
    let mock_logger = MockLogger::new();
    // Testing nothing real
}

// Good - Mock only external dependencies
#[test]
fn test_with_minimal_mocks() {
    let real_config = create_test_config();
    let mock_git = MockGit::new(); // Only mock external service
    // Test real code with real config
}
```

### ❌ Brittle Tests

```rust
// Bad - Depends on exact string matching
#[test]
fn test_error_message() {
    let err = do_something().unwrap_err();
    assert_eq!(err.to_string(), "Error: Failed to clone repository from https://github.com/user/repo.git");
}

// Good - Test important parts
#[test]
fn test_error_message() {
    let err = do_something().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Failed to clone"));
    assert!(msg.contains("github.com/user/repo.git"));
}
```

---

## CI/CD Integration

### Tests in Pull Requests

All PRs must:
- [ ] Have all tests passing
- [ ] Not decrease overall coverage (without justification)
- [ ] Include tests for new features
- [ ] Update existing tests if behavior changed

### CI Test Configuration

```yaml
# .github/workflows/ci.yml
- name: Run tests
  run: cargo test --all-features --workspace

- name: Run integration tests
  run: cargo test --test '*' --workspace

- name: Run doc tests
  run: cargo test --doc --workspace

- name: Check coverage
  run: |
    cargo tarpaulin --workspace --out Xml
    # Fail if coverage below threshold
```

---

## Documentation Tests

### Add Examples to Public APIs

```rust
/// Clone a Git repository to a target path
///
/// # Arguments
/// * `url` - Git repository URL (HTTPS or SSH)
/// * `path` - Target directory path
///
/// # Examples
/// ```
/// use metarepo::git::clone_repository;
/// use std::path::Path;
/// # use tempfile::tempdir;
///
/// # fn main() -> anyhow::Result<()> {
/// # let temp = tempdir()?;
/// # let target = temp.path().join("repo");
/// clone_repository(
///     "https://github.com/user/repo.git",
///     &target
/// )?;
/// # assert!(target.exists());
/// # Ok(())
/// # }
/// ```
pub fn clone_repository(url: &str, path: &Path) -> Result<()> {
    // ...
}
```

---

## Resources

- [Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Rust by Example - Testing](https://doc.rust-lang.org/rust-by-example/testing.html)
- [Testing strategies in Rust](https://matklad.github.io/2021/05/31/how-to-test.html)
- [Effective testing principles](https://martinfowler.com/articles/practical-test-pyramid.html)

---

## Questions?

For questions about testing strategy or specific testing scenarios, see:
- [TESTING_STRATEGY.md](./TESTING_STRATEGY.md) - Overall strategy and assessment
- [TESTING_RECOMMENDATIONS.md](./TESTING_RECOMMENDATIONS.md) - Specific improvements
- Ask in team discussions or code reviews
