# GitHub Issue Creation Scripts

Automated scripts for creating GitHub issues programmatically or interactively.

## Overview

These scripts support multiple input modes, making them perfect for:
- **Interactive use** - Manual issue creation via prompts
- **Command-line automation** - Pass arguments directly
- **Environment variables** - Set values in the shell environment
- **JSON input** - Pipe structured data for AI agents and automation
- **Silent mode** - Suppress output for clean automation

All scripts return the created issue URL on success, making them ideal for scripting and automation workflows.

## Prerequisites

- [GitHub CLI (`gh`)](https://cli.github.com/) - Required
- [`jq`](https://stedolan.github.io/jq/) - Required only for JSON input mode

## Scripts

### 1. Bug Reports (`new-bug.sh`)

Create bug reports with environment details.

**Interactive:**
```bash
.github/scripts/new-bug.sh
```

**Command-line arguments:**
```bash
.github/scripts/new-bug.sh \
  "App crashes on startup" \
  "The application crashes immediately when launched" \
  "1. Install app 2. Run meta init 3. App crashes" \
  "App should start normally" \
  "App crashes with segfault"
```

**Environment variables:**
```bash
export BUG_TITLE="App crashes on startup"
export BUG_DESC="The application crashes immediately"
export BUG_STEPS="1. Install app 2. Run meta init 3. App crashes"
export BUG_EXPECTED="App should start normally"
export BUG_ACTUAL="App crashes with segfault"
.github/scripts/new-bug.sh
```

**JSON input (perfect for Claude agents):**
```bash
echo '{
  "title": "App crashes on startup",
  "description": "The application crashes immediately when launched",
  "steps": "1. Install app\n2. Run meta init\n3. App crashes",
  "expected": "App should start normally",
  "actual": "App crashes with segfault"
}' | .github/scripts/new-bug.sh --json
```

**Silent mode (returns only the issue URL):**
```bash
ISSUE_URL=$(echo '{
  "title": "Memory leak in worker threads",
  "description": "Workers accumulate memory over time",
  "steps": "Run with --parallel for 1 hour",
  "expected": "Memory stays constant",
  "actual": "Memory grows to 2GB+"
}' | .github/scripts/new-bug.sh --json --silent)

echo "Created issue: $ISSUE_URL"
```

### 2. Feature Requests (`new-feature.sh`)

Propose new features with problem statements and solutions.

**Interactive:**
```bash
.github/scripts/new-feature.sh
```

**Command-line arguments:**
```bash
.github/scripts/new-feature.sh \
  "Add Docker support" \
  "Run metarepo in containerized environments" \
  "Users need to run meta in Docker containers" \
  "Add Dockerfile and docker-compose.yml" \
  "high"
```

**Environment variables:**
```bash
export FEATURE_TITLE="Add Docker support"
export FEATURE_SUMMARY="Run metarepo in containerized environments"
export FEATURE_PROBLEM="Users need to run meta in Docker containers"
export FEATURE_SOLUTION="Add Dockerfile and docker-compose.yml"
export FEATURE_PRIORITY="high"
.github/scripts/new-feature.sh
```

**JSON input:**
```bash
echo '{
  "title": "Add Docker support",
  "summary": "Run metarepo in containerized environments",
  "problem": "Users need to run meta in Docker containers",
  "solution": "Add Dockerfile and docker-compose.yml",
  "priority": "high"
}' | .github/scripts/new-feature.sh --json
```

**Silent mode:**
```bash
ISSUE_URL=$(echo '{
  "title": "Add SSH key management",
  "summary": "Manage SSH keys across projects",
  "problem": "Switching between projects with different SSH keys is tedious",
  "solution": "Add meta ssh command to manage keys per-project"
}' | .github/scripts/new-feature.sh --json --silent)

echo "Feature request: $ISSUE_URL"
```

### 3. Quick Ideas (`new-idea.sh`)

Capture ideas quickly with minimal friction.

**Interactive:**
```bash
.github/scripts/new-idea.sh
```

**Command-line arguments:**
```bash
# Just title
.github/scripts/new-idea.sh "Add plugin marketplace"

# Title with notes
.github/scripts/new-idea.sh \
  "Add plugin marketplace" \
  "Users could browse and install community plugins easily"
```

**Environment variables:**
```bash
export IDEA_TITLE="Add plugin marketplace"
export IDEA_NOTES="Users could browse and install community plugins"
.github/scripts/new-idea.sh
```

**JSON input:**
```bash
echo '{
  "title": "Add plugin marketplace",
  "notes": "Browse and install community plugins"
}' | .github/scripts/new-idea.sh --json
```

**Silent mode:**
```bash
ISSUE_URL=$(echo '{
  "title": "Support for Mercurial repositories"
}' | .github/scripts/new-idea.sh --json --silent)
```

## Usage with Claude Agents

These scripts are designed to work seamlessly with AI agents like Claude. Here's how:

### Example: Bug Report from Agent

```bash
# Claude agent generates JSON from conversation analysis
cat << 'EOF' | .github/scripts/new-bug.sh --json --silent
{
  "title": "TUI crashes when terminal is resized rapidly",
  "description": "The TUI configuration editor crashes if the terminal window is resized quickly multiple times",
  "steps": "1. Open config editor with 'meta config'\n2. Rapidly resize terminal window 10+ times\n3. TUI crashes with panic",
  "expected": "TUI should handle resize events gracefully",
  "actual": "TUI panics with 'thread panicked at layout calculation'"
}
EOF
```

### Example: Feature Request from Agent

```bash
# Agent extracts feature idea from user conversation
cat << 'EOF' | .github/scripts/new-feature.sh --json --silent
{
  "title": "Add workspace health check command",
  "summary": "Verify workspace integrity and configuration",
  "problem": "Users have no easy way to validate their workspace setup is correct",
  "solution": "Add 'meta doctor' command that checks: git status, missing repos, broken symlinks, config validity, etc.",
  "priority": "medium"
}
EOF
```

### Example: Batch Idea Capture

```bash
# Agent captures multiple ideas from brainstorming session
for idea in "Add tab completion" "Improve error messages" "Add telemetry opt-in"; do
  .github/scripts/new-idea.sh --silent "$idea"
done
```

## Exit Codes

All scripts follow standard exit code conventions:

- `0` - Success (issue created)
- `1` - Error (validation failed, gh CLI error, etc.)

## Output

- **Interactive/Normal mode**: Progress messages + issue URL
- **Silent mode** (`--silent`): Issue URL only (errors still go to stderr)

## Help

Each script has built-in help:

```bash
.github/scripts/new-bug.sh --help
.github/scripts/new-feature.sh --help
.github/scripts/new-idea.sh --help
```

## Integration Examples

### GitHub Actions Workflow

```yaml
name: Create Issue from Analysis
on:
  workflow_dispatch:
    inputs:
      bug_report:
        description: 'Bug report JSON'
        required: true

jobs:
  create-issue:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Create Bug Report
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          echo '${{ github.event.inputs.bug_report }}' | \
            .github/scripts/new-bug.sh --json --silent
```

### CI/CD Pipeline

```bash
# In CI script - create issue if build fails
if ! cargo build --release; then
  ISSUE_URL=$(echo '{
    "title": "Build failure in CI",
    "description": "Release build failed in CI pipeline",
    "steps": "Check CI logs for run #'"$CI_RUN_ID"'",
    "expected": "Build should succeed",
    "actual": "Build failed with errors"
  }' | .github/scripts/new-bug.sh --json --silent)

  echo "Created issue: $ISSUE_URL"
  exit 1
fi
```

### Monitoring/Alerting Integration

```bash
# Create issue from monitoring alert
create_issue_from_alert() {
  local alert_json="$1"

  echo "$alert_json" | jq -r '{
    title: .alert_name,
    description: .alert_description,
    steps: .steps_to_reproduce,
    expected: "Service should be healthy",
    actual: .actual_state
  }' | .github/scripts/new-bug.sh --json --silent
}
```

## Best Practices

1. **Use `--silent` in scripts** - Clean output for parsing
2. **Check exit codes** - Always validate success/failure
3. **Capture URLs** - Store returned URLs for tracking
4. **Provide context** - Include as much detail as possible
5. **Set priorities** - Use priority fields appropriately
6. **Use JSON mode** - Most robust for automation

## Error Handling

```bash
# Robust error handling example
if ISSUE_URL=$(echo "$JSON_DATA" | .github/scripts/new-bug.sh --json --silent 2>&1); then
  echo "Success: $ISSUE_URL"
  # Store URL, update tracking system, etc.
else
  echo "Failed to create issue: $ISSUE_URL" >&2
  # Handle error - retry, log, alert, etc.
  exit 1
fi
```

## Makefile Integration

These scripts are also available via Makefile targets:

```bash
make issue-bug      # Interactive bug report
make issue-feature  # Interactive feature request
make issue-idea     # Interactive idea capture
make list-issues    # List recent issues
```

See the main [Makefile](../../Makefile) for implementation details.

---

**Questions or issues?** Open a [discussion](https://github.com/caavere/metarepo/discussions) or create an issue!
