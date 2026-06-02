---
name: example-skill
description: Example Claude Code skill shipped by the example meta module. Demonstrates how a module bundles automation alongside its plugin.
version: 0.1.0
---

# example-skill

This skill ships inside the example meta module. When the module is enabled
(`meta module enable <repo>`), this skill is installed into the workspace's
`.claude/skills/` via the audit-gated steal path.

It pairs with the module's `example-hello` command: use it to drive or explain
that command from Claude Code.
