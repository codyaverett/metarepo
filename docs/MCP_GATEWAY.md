# MCP gateway & workspace scoping — design

Status: **all phases implemented** (#86, #87, #88, #89). Tracks how the
experimental `meta mcp` plugin grew into (a) a progressive-disclosure gateway in
front of other MCP servers, and (b) a workspace-scoped, permission-aware server.

### Phase 1 — shipped

- `meta mcp serve --meta <file-or-dir>` pins the server to one workspace. Every
  spawned tool subprocess runs with `--experimental --config <that .meta>` and a
  fixed working directory, so tools cannot drift to another workspace. (The flag
  is `--meta`, not `--workspace`, because `--workspace`/`-w` is the global
  all-projects scope flag.)
- An opt-in `[mcp.serve]` policy in `.meta`, declared via `MetaPlugin::settings()`
  (so `meta config` edits it): `mode` (`full` default / `read-write` /
  `read-only`), `allow-exec` (default true), and `tools` (allowlist). Defaults
  preserve full access. `tools/list` is filtered to permitted tools and calls are
  rejected with the policy summary.
- The active workspace + policy are reported in the MCP `initialize`
  `instructions`.
- `meta mcp config` now emits `-x` and a `--meta` arg in the generated client
  block.
- Deferred to a later sub-task: the `mcp.serve.projects` restriction (not yet
  enforced, so not yet declared as a setting).

### Phase 2 — shipped (Tier A)

`meta mcp serve` now fronts the saved downstream servers in
`~/.config/meta/mcp/servers.json` with four navigational meta-tools, so the
top-level surface stays small:

- `mcp_catalog` — list saved downstream servers (no connection; cheap).
- `mcp_list_tools(server)` — connect on demand and list one server's tools.
- `mcp_search_tools(query)` — case-insensitive search across all servers' tool
  names/descriptions.
- `mcp_call(server, tool, arguments)` — proxy a downstream tool call.

Downstream connections are lazy (only spawned when `list_tools`/`search`/`call`
needs them), tool lists are cached per server for the server's lifetime, and each
connection has a 30s timeout. The sync server loop bridges to the async client
via a per-call current-thread runtime. The browse tools are reads; `mcp_call` is
a write (blocked under `read-only`). Not yet applied: the saved `working_dir`/
`env` on a downstream config (same limitation as `meta mcp connect`).

### Phase 4 — shipped

- **Allowlist mode**: `meta mcp serve --allow-workspaces a,b,c` hosts several
  workspaces from one server. Workspace tools take a `workspace` argument
  (advertised in their schema, validated against the list); the new
  `mcp_workspaces` tool lists the hosted workspaces. Each workspace's own
  `[mcp.serve]` policy applies per call; gateway meta-tools stay
  workspace-independent. Omitting `workspace` with several hosts is an error.
- **`mcp.serve.projects`** is now enforced: the `exec` tool defaults to the
  allowlisted projects and rejects any project outside it.
- **`meta mcp config`** generates client blocks: `--meta a,b,c` emits one pinned
  entry per workspace (`metarepo-<name>`); add `--allow-workspaces` to emit a
  single allowlist entry instead.

### Phase 3 — shipped (Tier B)

- `mcp_enable(server[, tool])` promotes a downstream server's tools (or one
  named tool) into the gateway's top-level `tools/list` under namespaced names
  (`server__tool`); `mcp_disable([server[, tool]])` removes them.
- After enable/disable the server emits `notifications/tools/list_changed`, and
  it advertises `capabilities.tools.listChanged: true` at `initialize`, so a
  client that honors the notification re-fetches and can call the promoted tools
  directly. Promoted tools proxy to the downstream server (gated like `mcp_call`,
  so blocked under `read-only`). Clients that don't refresh can still reach the
  same tools via `mcp_call`.

All four phases are now implemented. The remaining sections are the original
design notes.

## 1. Where we are today

`meta mcp` is gated behind `--experimental` (`meta/src/plugins/mcp/plugin.rs:26`,
`.experimental(true)`). It already plays both roles:

- **Server** — `meta mcp serve` (`mcp_server.rs`): a stdio JSON-RPC server,
  MCP protocol `2025-06-18`. It answers `initialize`, `tools/list`,
  `tools/call`, `resources/list`, `prompts/list`. It exposes **13 fixed tools**
  (`help`, `git_status`, `git_diff`, `git_commit`, `git_pull`, `git_push`,
  `project_list`, `project_add`, `project_remove`, `exec`, `mcp_add_server`,
  `mcp_list_servers`, `mcp_remove_server`). Each tool simply shells out to a
  `meta <subcommand>` subprocess (`execute_tool`).
- **Client** — `meta mcp connect|list-tools|list-resources|call-tool`
  (`client.rs`): spawns another MCP server over stdio and speaks the same
  protocol. Saved servers live in `~/.config/meta/mcp/servers.json`
  (`config.rs`), one `McpServerConfig { name, command, args, working_dir, env }`
  per entry.

Two gaps motivate this design:

1. **No aggregation / disclosure.** The server only exposes its own 13 tools;
   the saved downstream servers are not reachable through it. Naively surfacing
   every downstream tool would flood the client's context (the well-known
   "too many MCP tools" problem).
2. **No scoping.** `serve` inherits the launch cwd; every tool subprocess
   re-discovers `.meta` from that cwd. Nothing pins the workspace and nothing
   limits which operations run. `exec`/`git_commit`/`git_push` can mutate any
   reachable repo — a real concern for an AI-driven server.

## 2. Goals & chosen defaults

- Turn `meta mcp serve` into an optional **gateway** that fronts the saved
  downstream MCP servers using **progressive disclosure**, modeled on how skills
  list `name + description` before the full body loads.
- Give the user **explicit control over which workspace(s)** a server touches.
- **Preserve today's behavior by default.** Per the decision for this design:
  the default permission posture is **full access** (no restrictions). All
  tightening is **opt-in**. Scoping supports **both** a pinned single-workspace
  model and an allowlist multi-workspace model.

## 3. Part A — progressive-disclosure gateway

### 3.1 The disclosure model (mirror skills)

Skills keep the listing cheap: only frontmatter (`name`, `description`) is shown
until a skill is "opened" and its body loads. The MCP analog keeps the top-level
`tools/list` tiny no matter how many downstream servers/tools exist, by exposing
a small fixed set of **navigational meta-tools** instead of every downstream
tool:

| Meta-tool | Purpose (disclosure level) |
| --- | --- |
| `mcp_catalog` | List configured downstream servers, each with a one-line description and **tool count**. The cheap "frontmatter" view. |
| `mcp_list_tools(server)` | On demand: the tool list (names + descriptions) for one server. The "open the skill" step. |
| `mcp_search_tools(query)` | Fuzzy search across all downstream tool names/descriptions; returns matches without loading every server. |
| `mcp_call(server, tool, args)` | Proxy the actual downstream invocation. |

Downstream tools never enter the top-level `tools/list`, so the initial surface
stays small regardless of how many servers are configured. metarepo's own native
tools stay top-level (they are the primary value); only downstream tools sit
behind disclosure.

### 3.2 Lazy connection & caching

Disclosure is also a runtime cost win:

- **Lazy connect** — do not spawn a downstream server until `mcp_list_tools` or
  `mcp_call` first touches it. Reuse `client.rs::McpClient`.
- **Cache** each server's `tools/list` after first fetch (optional TTL; or
  invalidate on a `tools/list_changed` notification from the downstream).
- **Per-downstream timeout** (new setting `mcp.gateway.timeout-ms`) so one dead
  server cannot hang the gateway.
- **Namespacing** — proxied tools are referenced as `server__tool` to avoid
  collisions (and to make Tier B promotion unambiguous).

### 3.3 Tier B — true load-on-demand (later, capability-gated)

Add `mcp_enable(server[, tool])` that **promotes** selected downstream tools into
the gateway's own `tools/list` and emits an MCP `notifications/tools/list_changed`
so the client re-fetches — the literal "load a skill, its tools appear" behavior.

This is a bigger lift: the current server is a pure request/response loop
(`mcp_server.rs` only writes responses for requests with an `id`) and does not
push notifications, and not all clients honor `list_changed`. Ship Tier A first
(works with any client); gate Tier B on the client advertising the capability in
`initialize`.

### 3.4 Where downstream servers are configured

- Global saved servers stay in `~/.config/meta/mcp/servers.json` (unchanged).
- A workspace may **scope** which downstream servers its gateway exposes via the
  `.meta` `[mcp]` block (see Part B), so disclosure and scoping share one surface.

## 4. Part B — workspace scoping & permissions

### 4.1 Scoping models (both supported)

**Model 1 — pinned single workspace (default; shipped in phase 1).**
`meta mcp serve --meta <path>` resolves the `.meta` **once** at startup, records
its root, and forces every spawned tool subprocess to run with
`--config <that file>` and a fixed working directory. A tool can no longer drift
to another workspace via cwd. Multi-workspace is expressed as **one client entry
per workspace**:

```json
{
  "mcpServers": {
    "metarepo-acme":     { "command": "meta", "args": ["-x", "mcp", "serve", "--meta", "/work/acme"] },
    "metarepo-personal": { "command": "meta", "args": ["-x", "mcp", "serve", "--meta", "/home/me/personal"] }
  }
}
```

`meta mcp config` would generate these per-workspace blocks.

**Model 2 — allowlist, one server many workspaces (opt-in).**
A global config lists allowed workspace roots. Tools accept a `workspace`
argument that must resolve to an allowlisted root; anything else is rejected.
One server can then serve several workspaces, at the cost of a larger surface.
Enabled explicitly (e.g. `meta mcp serve --allow-workspaces a,b,c` or a global
`mcp.gateway.workspaces` list).

Both models force `--config` on the spawned `meta` subprocesses so discovery
cannot wander outside the intended root(s).

### 4.2 Permission policy (opt-in; default full access)

A `[mcp]` block in the workspace `.meta`, declared through
`MetaPlugin::settings()` so `meta config` can edit it and it travels with the
repo. **Defaults preserve current behavior (full access).** Setting any of these
tightens the server:

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `mcp.serve.mode` | `full` \| `read-write` \| `read-only` | `full` | `read-only` denies write tools (commit/push/project_add/remove); `read-write` allows git/project writes but not `exec`. |
| `mcp.serve.allow-exec` | bool | `true` | When false, the arbitrary-shell `exec` tool is rejected even in `full`/`read-write`. |
| `mcp.serve.tools` | string list | unset (all) | If set, an explicit allowlist of exposed tool names. |
| `mcp.serve.projects` | string list | unset (all) | Restrict operations to a subset of the workspace's projects. |

Launch flags (`--read-only`, `--allow-exec=false`, …) override the `.meta`
policy, which overrides global defaults. **Precedence: flags > workspace
`.meta [mcp]` > global defaults (full access).**

The active policy is reported in the `initialize` response (and/or a `whoami`
tool) so the model knows its boundaries up front.

### 4.3 Security note

The default chosen for this design is **full access**, which keeps the current
behavior but means an AI client driving the server can run arbitrary shell
(`exec`) and push to any in-scope repo. This is a genuine RCE / confused-deputy
surface. Recommended hardening for anyone exposing the server to an autonomous
client, even though it is not the default:

- set `mcp.serve.mode = read-only` (or `read-write` without `exec`),
- set `mcp.serve.allow-exec = false`,
- pin a single `--workspace`,
- log every tool call.

A future revision may flip the default to read-only; that would be a deliberate,
documented breaking change.

## 5. Config surface summary

| Concern | Location |
| --- | --- |
| Saved downstream servers | `~/.config/meta/mcp/servers.json` (existing) |
| Gateway tuning (timeouts, allowlisted workspaces) | global `mcp.gateway.*` settings |
| Per-workspace server policy | `.meta` `[mcp]` block (`mcp.serve.*`), via `MetaPlugin::settings()` |
| Per-launch overrides | `meta mcp serve` flags (`--workspace`, `--read-only`, `--allow-exec`, `--allow-workspaces`) |

## 6. Implementation phasing

1. **Scope pinning + policy plumbing.** Add `--workspace`, force `--config` and a
   fixed cwd on spawned tool subprocesses in `mcp_server.rs::execute_tool`; add a
   `MetaPlugin::settings()` block for `mcp.serve.*`; enforce the policy before
   dispatch; report it in `initialize`. Defaults keep full access, so this is
   backward compatible.
2. **Gateway Tier A.** Add `mcp_catalog` / `mcp_list_tools` / `mcp_search_tools`
   / `mcp_call`, backed by lazy `McpClient` connections with caching and a
   timeout. Bridge the sync server loop with the async client (spawn a runtime or
   make the loop async).
3. **Gateway Tier B.** Dynamic tool promotion + `tools/list_changed`
   notifications, gated on client capability.
4. **Allowlist scoping (Model 2)** and `meta mcp config` per-workspace block
   generation.

## 7. Code touch-points

- `meta/src/plugins/mcp/mcp_server.rs` — tool dispatch, `--config` injection,
  policy enforcement, `initialize` payload, new gateway meta-tools.
- `meta/src/plugins/mcp/client.rs` — reused for downstream connections; add
  pooling/caching/timeouts.
- `meta/src/plugins/mcp/plugin.rs` — `serve` flags, `settings()` declaration,
  `config` block generation.
- `meta-core` — `ConfigSetting`s for `mcp.*`; reuse `scoped_keys` /
  `create_runtime_config_full` for project scoping.

## 8. Open questions

- Sync→async bridge: make `mcp_server` fully async (tokio) vs. spawn a runtime
  per gateway call. Affects how connections are pooled.
- Should native metarepo tools also be reducible via disclosure for very large
  tool sets, or always top-level?
- Whether to ever flip the default posture to read-only (breaking change).
- Downstream auth: servers needing secrets/headers — extend `McpServerConfig`.
