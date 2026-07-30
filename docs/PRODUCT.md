# Product identity: dual surface

Metarepo is **one binary** with **two product surfaces**.

## Surface A — Multi-repo CLI (stable, default)

The origin and primary promise: manage many git repositories as one workspace.

| Area | Role |
|------|------|
| `meta init` / `meta project` | Workspace and project lifecycle |
| `meta git` | Fleet git ops: clone, status, update, pull, **push**, **fetch**, **checkout** |
| `meta exec` / `meta run` | Fan-out commands and named scripts |
| `meta worktree` | Bare-first multi-project worktrees |
| `meta config` / `meta status` | Config and interactive multi-repo status |
| Scoping flags | Directory-aware scope, `-w` / `--root` |

This surface is what crates.io and the short README pitch describe. New work here
should prefer **depth on multi-repo workflows** over new adjacent product lines.

## Surface B — Agent / extension profile (explicit, opt-in or labeled)

Capabilities aimed at agent harnesses, Claude Code, and extensibility:

| Area | Status | Notes |
|------|--------|--------|
| `meta skill` | Stable | Claude skill install/steal/audit/registry — largest plugin |
| `meta module` | Stable | Bundle plugin + skills as one unit |
| `meta plugin` | Stable | External protocol/manifest plugins, integrity |
| `meta -x mcp` | Experimental | MCP server + progressive gateway |
| `meta -x rules` | Experimental | Project structure rules engine |

Experimental commands require `-x` / `--experimental` and are **not** stability
guarantees. They may graduate (drop `-x`) or be removed after review.

## Boundary rules

1. **Default help and README lead with Surface A.** Agent features are secondary
   sections, never the only story.
2. **Do not grow Surface B** without a named consumer and a path to graduate or
   document as permanent agent tooling.
3. **`meta git` parity beats new TUI polish** when choosing between multi-repo
   daily drivers and editor niceties.
4. **Extension machinery** (protocol, modules, integrity) stays, but dogfood at
   least one real module/plugin before adding more install/scaffold surface.
5. **Experimental forever is a bug.** Track graduation or deletion as product
   issues (see backlog).

## Why dual (not a rewrite)

The codebase already implements both. Pretending only multi-repo exists hides
skill/MCP complexity; pretending only agent-OS exists abandons the Node-meta
mission and under-delivers on `meta git`. Dual boundaries keep both honest.
