## Git / Commits

- After completing a task or coherent phase of work, **stage and commit automatically** without waiting for the user to say commit. Do not only paste a suggested message and stop.
- Commit messages must use commitizen / Conventional Commits formatting, be detailed about the files and intent of the change, and include package version numbers when the version changed.
- Prefer GPG-signed commits (`git commit -S`). After committing, verify the signature (`git log --show-signature -1`). If signing fails or no signing key is available, fall back to an unsigned commit and note that in the reply.
- Shell-safe messages only: no backticks, exclamation marks, angle brackets, em-dashes, or double quotes in the commit message body/subject (they break zsh/heredocs).
- DO NOT attribute Claude, Grok, or other agents in commit messages (no Generated with ... footers).
- After a feature or milestone set of commits, bump the Cargo.toml version appropriately, then create and push a git tag when that step is part of the release flow. Pushing the branch and tags still requires normal caution for shared remotes; prefer pushing when the user is shipping, or when they have already asked for autonomous end-to-end delivery.
- When closing multiple issues, use a separate Closes #N line per issue.
- Never commit secrets, credentials, or `.env` files.

## GitHub Issue Creation

When you identify bugs, feature opportunities, or have ideas during development, you can programmatically create GitHub issues using the scripts in `.github/scripts/`. These scripts support JSON input, making them ideal for automation.

### When to Create Issues

Create issues when you discover:
- **Bugs**: Problems, crashes, or unexpected behavior
- **Features**: New functionality or enhancements that would improve the project
- **Ideas**: Quick thoughts, TODOs, or future improvements
- **Technical debt**: Code that needs refactoring or improvement

### How to Create Issues Programmatically

**Bug Reports:**
```bash
echo '{
  "title": "Brief bug description",
  "description": "Detailed explanation of the bug",
  "steps": "1. Step one\n2. Step two\n3. Bug occurs",
  "expected": "What should happen",
  "actual": "What actually happens"
}' | .github/scripts/new-bug.sh --json --silent
```

**Feature Requests:**
```bash
echo '{
  "title": "Feature name",
  "summary": "Brief feature summary",
  "problem": "Problem this solves",
  "solution": "Proposed solution",
  "priority": "medium"
}' | .github/scripts/new-feature.sh --json --silent
```

**Quick Ideas:**
```bash
echo '{
  "title": "Idea title",
  "notes": "Optional additional context"
}' | .github/scripts/new-idea.sh --json --silent
```

**Plan decomposition:** once a plan doc in `docs/plans/` has a `## Tasks`
section, create every issue at once (preview with `--dry-run` first):
```bash
.github/scripts/plan-to-issues.sh docs/plans/PLAN_X.md --dry-run
.github/scripts/plan-to-issues.sh docs/plans/PLAN_X.md
```
The script writes the issue numbers back into the doc and prints the URLs.

### Examples

**Example 1: Bug found during code review**
```bash
# You notice a potential race condition
echo '{
  "title": "Potential race condition in parallel exec",
  "description": "The exec plugin may have a race condition when running commands in parallel mode",
  "steps": "Run meta exec --parallel with multiple projects simultaneously",
  "expected": "Commands execute safely in parallel",
  "actual": "Occasionally see output corruption or crashes"
}' | .github/scripts/new-bug.sh --json --silent
```

**Example 2: Feature idea during development**
```bash
# You realize a feature would be useful
echo '{
  "title": "Add dry-run mode to all commands",
  "summary": "Allow users to preview what would happen before executing",
  "problem": "Users are hesitant to run destructive commands without knowing what will happen",
  "solution": "Add --dry-run flag to all commands that shows planned actions without executing",
  "priority": "medium"
}' | .github/scripts/new-feature.sh --json --silent
```

**Example 3: Quick idea capture**
```bash
# Quick thought during implementation
echo '{
  "title": "Add progress bar for git clone operations"
}' | .github/scripts/new-idea.sh --json --silent
```

### Best Practices

1. **Use `--silent` flag**: Returns only the issue URL for clean output
2. **Create issues proactively**: Don't wait to be asked - if you spot something worth tracking, create an issue
3. **Be descriptive**: Provide enough context for others to understand and act on
4. **Set appropriate priorities**: Use "critical", "high", "medium", or "low" for features
5. **Include reproduction steps**: For bugs, always include clear steps to reproduce

### Output

All scripts return the created issue URL, which you can capture:
```bash
ISSUE_URL=$(echo '{"title":"..."}' | .github/scripts/new-idea.sh --json --silent)
echo "Created issue: $ISSUE_URL"
```

### Full Documentation

See `.github/scripts/README.md` for complete documentation including:
- All input modes (JSON, env vars, command-line args)
- Integration examples
- Error handling
- CI/CD usage patterns
- After completing a feature or major milestone, stage and commit the work automatically (see Git / Commits above); do not wait for an explicit commit request.
