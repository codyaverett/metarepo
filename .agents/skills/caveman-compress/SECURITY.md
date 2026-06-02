# Security

## Snyk High Risk Rating

`caveman-compress` receives a Snyk High Risk rating due to static analysis heuristics. This document explains what the skill does and does not do.

## Snyk Findings & Mitigations

### W007 (HIGH) — Secret exfiltration to the LLM

**Finding:** the skill ships file contents to the Anthropic API, so an embedded secret would be reproduced by the model.

**Mitigation (two layers, both fail before the network call):**

1. `is_sensitive_path()` — filename denylist (`.env`, `credentials.*`, `*.pem`, `id_rsa`, `~/.ssh`, `~/.aws`, name tokens like `secret`/`token`/`apikey`).
2. `scan_for_secret_content()` — content scan for secrets pasted into prose files (private-key blocks, AWS/GitHub/Slack/Google/Anthropic/OpenAI/Stripe keys, JWTs, `api_key = …` assignments). Match → refuse before any API call.

Either match aborts with a clear message; the user removes/renames and retries. Files over 500KB are rejected up front.

### W012 (MEDIUM) — Untrusted model output controls file writes

**Finding:** the API response is used to overwrite files without verification.

**Mitigation:** the model response is validated **in memory** (`validate_text`) before it reaches disk — code blocks, inline code, URLs, headings, and paths must be preserved. Failures trigger targeted-fix retries (max 2). If it never validates, the primary file is left untouched and no backup is written. Only validated output is committed, after a backup readback check.

### What triggers the rating

1. **subprocess usage**: The skill calls the `claude` CLI via `subprocess.run()` as a fallback when `ANTHROPIC_API_KEY` is not set. The subprocess call uses a fixed argument list — no shell interpolation occurs. User file content is passed via stdin, not as a shell argument.

2. **File read/write**: The skill reads the file the user explicitly points it at, compresses it, and writes the result back to the same path. A `.original.md` backup is saved alongside it. No files outside the user-specified path are read or written.

### What the skill does NOT do

- Does not execute user file content as code
- Does not make network requests except to Anthropic's API (via SDK or CLI)
- Does not access files outside the path the user provides
- Does not use shell=True or string interpolation in subprocess calls
- Does not collect or transmit any data beyond the file being compressed

### Auth behavior

If `ANTHROPIC_API_KEY` is set, the skill uses the Anthropic Python SDK directly (no subprocess). If not set, it falls back to the `claude` CLI, which uses the user's existing Claude desktop authentication.

### File size limit

Files larger than 500KB are rejected before any API call is made.

### Reporting a vulnerability

If you believe you've found a genuine security issue, please open a GitHub issue with the label `security`.
