# Harness Integration Guide

How to make an AI agent **harness** — Claude Code, opencode, Cursor, Zed, a custom
agent TUI — fluent in metarepo. This is the strategy and how-to for *external* tools
that drive `meta`, not for extending `meta` itself.

> Looking to add a new `meta` subcommand? That's a different kind of plugin — see
> [Plugin Development](PLUGIN_DEVELOPMENT.md). This guide is about the *other* side:
> teaching an agent harness to use the `meta` you already have.

---

## 1. Two meanings of "plugin"

The word "plugin" is overloaded, and untangling it answers most questions:

| | **Internal `meta` plugin** | **Harness integration** |
|---|---|---|
| Extends | the `meta` CLI itself | an external agent/harness |
| Examples | `git`, `project`, `worktree`, a manifest plugin | a Claude Code skill, an opencode tool, an MCP registration |
| Lives in | the `meta` binary or `~/.config/metarepo/plugins/` | the harness's config (`.claude/`, opencode config, `.mcp.json`) |
| Doc | [PLUGIN_DEVELOPMENT.md](PLUGIN_DEVELOPMENT.md) | **this file** |

If a harness wants a *new* metarepo capability (say, `meta release`), the right move is
to write an internal plugin once and let every harness reach it through the surfaces
below — not to reimplement the logic per harness.

---

## 2. Types of harness integration

There is no single "harness plugin". Integrations come in four types, and a good
metarepo setup uses all four together:

1. **Context packs** — pure text that teaches the model *what meta is and when to reach
   for it*: a Claude Code skill, a `CLAUDE.md` / `AGENTS.md`, a system-prompt snippet, an
   output style. No execution; just knowledge.
2. **Tool / capability plugins** — give the agent *callable actions*: an MCP server
   (universal), opencode custom tools, Claude Code MCP registration. This is what lets the
   model actually run `meta git status` or `meta exec`.
3. **Workflow plugins** — encode *multi-step* meta procedures: slash commands, subagents,
   hooks, scheduled routines (e.g. "spin a worktree per open PR, run tests in each").
4. **Configuration** — the glue that loads the above: permission allowlists, MCP server
   registration, workspace discovery (`.meta` / `.metarepo`), environment. Not a "plugin"
   itself, but nothing works without it.

Mapped across the harnesses in scope:

| Type | Claude Code | opencode | MCP-capable (any) | `meta tui` (own surface) |
|---|---|---|---|---|
| Context | `meta-tool` skill, `CLAUDE.md` | `AGENTS.md` | tool descriptions | in-app help panel |
| Tools | MCP registration | TS/JS tool or MCP | **`meta mcp serve`** | native (it *is* meta) |
| Workflow | slash commands, subagents, hooks | agents/commands | client-side | keybindings/actions |
| Config | `.claude/settings.json`, `.mcp.json` | opencode config | client config | — |

---

## 3. What to expose to a harness

metarepo's useful surface area for an agent:

**Read / context (safe, idempotent):**
- project list and tree (`meta project list`, `meta project tree`)
- workspace config (`.meta` / `.metarepo`)
- cross-repo git status (`meta git status`) and diff (`meta git diff`)
- available scripts (`meta run` targets), worktree list (`meta worktree list`)

**Actions (mutating — gate behind confirmation):**
- run a command across repos (`meta exec`) or a named script (`meta run`)
- worktree create / remove (`meta worktree ...`)
- project add / remove (`meta project ...`)
- git clone / update / pull / commit / push (`meta git ...`)

**Guardrails.** metarepo is interactive-by-default but degrades safely: see
`meta-core/src/interactive.rs` (`is_interactive()`, `NonInteractiveMode::{Fail,Defaults}`)
and the global `--non-interactive` flag. For agent use, run mutating commands with
`--non-interactive` and surface destructive ones (worktree remove, project remove, push)
to the harness's own permission/confirmation model rather than auto-approving.

---

## 4. The keystone: metarepo as an MCP server

**Invest here first.** metarepo already ships an MCP server — `meta -x mcp serve`
(`meta/src/plugins/mcp/mcp_server.rs`). It speaks JSON-RPC over stdio (protocol
`2025-06-18`, `tools` capability) and any MCP-capable harness — Claude Code, opencode,
Cursor, Zed, Claude Desktop, a custom client — gets metarepo for free by pointing at it.
One server, every harness. Skills and slash commands then become thin sugar on top
instead of N reimplementations that drift apart.

### Mind the client/server duality

`meta -x mcp` does **two opposite things** — don't conflate them:

- **metarepo as MCP _client_/manager**: `add`, `list`, `remove`, `connect`, `list-tools`,
  `list-resources`, `call-tool` manage and call *other* MCP servers
  (`meta/src/plugins/mcp/server.rs`, `config.rs`, `client.rs`). Saved configs live in
  `~/.config/meta/mcp/servers.json`.
- **metarepo as MCP _server_**: `serve` exposes *metarepo itself*
  (`mcp_server.rs`). `config` prints a ready-to-paste client config.

> `mcp` is experimental, so commands need the `-x` flag: `meta -x mcp serve`.

### Tools exposed today (13)

`help`, `git_status`, `git_diff`, `git_commit`, `git_pull`, `git_push`, `project_list`,
`project_add`, `project_remove`, `exec`, `mcp_add_server`, `mcp_list_servers`,
`mcp_remove_server`. Each is a thin shell-out to the `meta` binary with a JSON input
schema (see `build_tools()` in `mcp_server.rs`).

### Gaps worth closing (roadmap input)

- **No `worktree` tools** — the worktree workflow is a prime agent use case.
- **No `run` tool** — scripts from `.meta` aren't exposed.
- **No resources** — `capabilities.resources` is `None`; project tree and `.meta`
  config would be natural read-only MCP *resources* rather than tools.
- **No `--non-interactive`** is passed by `execute_tool()`, so a mutating tool could
  block waiting on a prompt. Harden before promoting out of experimental.

### Register it in any client

```bash
meta -x mcp config        # prints a Claude Desktop / VS Code style block
```

```json
{
  "mcpServers": {
    "metarepo": { "command": "/path/to/meta", "args": ["mcp", "serve"], "env": {} }
  }
}
```

---

## 5. Per-harness how-to

### Claude Code

Already the most developed target:

- **Context** — `.claude/skills/meta-tool/SKILL.md` is a full command reference, triggered
  by phrases like "meta exec", "meta worktree", "multi-repo". Keep its version header in
  sync with `Cargo.toml` (note: v0.10.5 **removed** `--output-format` and `--ai`; don't
  reintroduce them in examples). `CLAUDE.md` carries commit/issue conventions.
- **Tools** — register `meta -x mcp serve` in `.mcp.json` / `.claude/settings.json` so the
  agent can *act*, not just recall syntax.
- **Workflow** — add slash commands for repeated procedures (e.g. a worktree-per-PR sweep),
  a subagent for multi-repo audits, and hooks (e.g. run `meta git status` after a clone).
- **Config** — permission allowlists already live in `.claude/settings.local.json`.

### opencode

- **Context** — add an `AGENTS.md` (opencode's `CLAUDE.md` analog) summarizing the meta
  surface from §3.
- **Tools** — preferred: point opencode at the same `meta -x mcp serve` server. Alternative:
  a thin TS/JS opencode plugin that shells out to `meta`. Prefer MCP so there's one schema.
- **Workflow** — opencode agents/commands wrapping common sweeps.

### Custom agent TUI

Two flavors:

1. **metarepo's own `meta tui`** — a dashboard built on the in-tree framework
   (`meta-core/src/tui/`, ratatui/crossterm; the menuconfig-style `meta config edit` in
   `meta/src/plugins/config/tui_editor.rs` is a working model). Show projects, worktrees,
   and cross-repo git status at a glance; launch `exec`/`run` from the UI. This is metarepo
   as *its own* harness surface.
2. **Embed the MCP server** — any third-party TUI agent registers `meta -x mcp serve` and
   gets the same tools as everyone else.

---

## 6. What this offers the software as a whole

- **Discoverability** — agents (and humans) find meta capabilities instead of hand-rolling
  `for repo in */; do git ...` loops.
- **Safer actions** — one permissioned, `--non-interactive`-aware tool surface instead of
  freeform shell.
- **One source of truth** — the MCP tool schema, not N drifting skill files. Add a tool
  once; every harness sees it.
- **A path to standalone** — the same primitives power a `meta tui`, so metarepo can be a
  harness, not just a thing harnesses call.

---

## 7. Recommended roadmap

| Phase | Work | Why first |
|---|---|---|
| **1 — MCP keystone** | Harden `meta -x mcp serve`: add `worktree`/`run` tools, expose project tree + `.meta` as resources, pass `--non-interactive`, document the schema, graduate from `-x`. | Unlocks all four harnesses at once. |
| **2 — Claude Code depth** | Slash commands + multi-repo subagent + MCP registration on top of the existing skill; keep skill version-synced. | Highest-traffic harness today. |
| **3 — opencode pack** | `AGENTS.md` + opencode pointed at the Phase 1 server. | Cheap once Phase 1 exists. |
| **4 — `meta tui`** | Standalone dashboard on `meta-core/src/tui/`. | metarepo as its own surface. |

**Start with Phase 1** — it's the single highest-leverage move and every later phase
builds on it.

---

## See also

- [Plugin Development](PLUGIN_DEVELOPMENT.md) — authoring *internal* `meta` plugins
- [Plugin Protocol v1](PLUGIN_PROTOCOL_V1.md) — external-plugin wire protocol
- [Architecture](ARCHITECTURE.md) — overall system design
- `.claude/skills/meta-tool/SKILL.md` — the Claude Code skill
- `meta-core/src/tui/` — the TUI framework powering a future `meta tui`
