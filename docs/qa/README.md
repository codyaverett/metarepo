# QA Documentation

This directory contains quality assurance documentation for the metarepo project.

## Documents

### [TESTING_STRATEGY.md](./TESTING_STRATEGY.md)
**Comprehensive analysis of the current testing strategy** (Rating: 5/10)

Contains:
- Current state assessment and test inventory
- Test coverage analysis by component
- Strengths and weaknesses
- Critical gaps and risks
- Coverage targets and metrics

**Read this first** to understand the current state of testing.

### [TESTING_RECOMMENDATIONS.md](./TESTING_RECOMMENDATIONS.md)
**Detailed, actionable recommendations for improvement**

Contains:
- Priority 1: Immediate actions (this week)
  - Set up code coverage tracking
  - Add Git operation integration tests
- Priority 2: Short-term (next 2 weeks)
  - Project management tests
  - Worktree operation tests
  - CLI integration tests
- Priority 3: Medium-term (next month)
  - E2E workflow tests
  - Property-based testing
  - Performance benchmarks
- Priority 4: Long-term (next quarter)
  - Testing guidelines and documentation
  - TDD workflow establishment

**Read this** to understand what needs to be done and in what order.

### [TESTING_GUIDELINES.md](./TESTING_GUIDELINES.md)
**Practical guidelines for writing and maintaining tests**

Contains:
- Testing philosophy and principles
- When to use each type of test (unit, integration, E2E)
- Coverage guidelines and targets
- Test organization and structure
- Naming conventions
- Testing patterns and anti-patterns
- Mocking strategies
- Performance testing
- CI/CD integration

**Reference this** when writing new tests or reviewing test code.

---

## Quick Start

### For New Contributors

1. Read [TESTING_GUIDELINES.md](./TESTING_GUIDELINES.md) to understand testing practices
2. Run existing tests: `cargo test --workspace`
3. Check coverage: `cargo tarpaulin --workspace`
4. Follow the guidelines when adding new tests

### For Maintainers

1. Review [TESTING_STRATEGY.md](./TESTING_STRATEGY.md) to understand current state
2. Review [TESTING_RECOMMENDATIONS.md](./TESTING_RECOMMENDATIONS.md) for priorities
3. Track progress on test coverage improvements
4. Ensure PRs meet testing requirements

---

## Current State Summary

**Overall Rating:** 5/10 (Fair)

**Strengths:**
- ✅ Good unit testing for core utilities
- ✅ Well-organized test structure
- ✅ CI/CD integration

**Critical Gaps:**
- ❌ No Git operation tests (CRITICAL)
- ❌ No project management tests (CRITICAL)
- ❌ No integration or E2E tests
- ❌ No coverage tracking

**Immediate Priorities:**
1. Set up code coverage (1 day)
2. Add Git integration tests (3-4 days)
3. Add project management tests (2-3 days)

---

## Test Coverage Targets

| Component | Current | Target (3 months) |
|-----------|---------|-------------------|
| Overall | ~30-35% | 70% |
| Core Library | ~75% | 90% |
| Critical Plugins | ~0% | 70% |
| Integration Tests | 0 | 20+ tests |
| E2E Tests | 0 | 10+ tests |

---

## Test Pyramid

**Target distribution:**
- 70% Unit Tests (fast, isolated)
- 20% Integration Tests (cross-component)
- 10% E2E Tests (full workflow)

**Current distribution:**
- 100% Unit Tests
- 0% Integration Tests ⚠️
- 0% E2E Tests ⚠️

---

## Running Tests

### All tests
```bash
cargo test --workspace
```

### Unit tests only
```bash
cargo test --lib --workspace
```

### Integration tests only
```bash
cargo test --test '*' --workspace
```

### Specific test file
```bash
cargo test --test git_operations
```

### With coverage
```bash
cargo tarpaulin --workspace --out Html --output-dir coverage
open coverage/index.html
```

### Benchmarks
```bash
cargo bench
```

---

## PR Testing Checklist

Before submitting a PR:

- [ ] All tests passing (`cargo test --workspace`)
- [ ] Added tests for new functionality
- [ ] Updated tests if behavior changed
- [ ] Coverage not decreased (check with tarpaulin)
- [ ] Tests follow guidelines in TESTING_GUIDELINES.md
- [ ] CI checks passing

---

## Contributing

When contributing tests:

1. Follow [TESTING_GUIDELINES.md](./TESTING_GUIDELINES.md)
2. Write tests before or alongside implementation
3. Ensure tests are isolated and deterministic
4. Use descriptive test names
5. Test both happy and error paths
6. Add comments for complex test setups

---

## Questions?

- Review the guidelines documents in this directory
- Check existing tests for examples
- Ask in code reviews or team discussions

---

**Last Updated:** 2025-11-01
**Version:** 0.8.2
