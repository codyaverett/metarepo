# Meta Modules

> **Status:** implemented (v1). The `meta module` commands and `meta project add`
> discovery described below are live. See [Out of scope](#out-of-scope) for the pieces
> intentionally deferred.

A **module** is a single repository that bundles everything it needs to extend a
metarepo workspace: the command capability it adds (a **plugin**) and the Claude Code
automation that drives it (one or more **skills**). Drop a module repo into a workspace
and meta can discover it, register its plugin, and install its skills as one unit.

## Why modules

metarepo already has two extension layers, but they are separate and both are wired up at
the *workspace* level:

- **Plugins** add commands to the `meta` CLI — built-in, protocol (subprocess + SDK), or
  manifest (`plugin.manifest.toml` + script). Declared in `.meta` under `plugins`.
  See [PLUGIN_DEVELOPMENT.md](PLUGIN_DEVELOPMENT.md) and
  [PLUGIN_PROTOCOL_V1.md](PLUGIN_PROTOCOL_V1.md).
- **Skills** are Claude Code automation: `SKILL.md` directories managed by the skill
  plugin (`meta skill install/steal/scan/audit/locations`). See
  [SKILL_TOOLS.md](SKILL_TOOLS.md).

Today a child repo added as a project in `.meta` is **passive** — meta never looks inside
it for anything it could contribute. If a repo wants to add a `meta` command *and* ship
the Claude skills that operate it, the author has to publish a plugin separately and tell
users to `meta skill steal` the skills by hand. There is no self-contained, discoverable
unit that says "this repo provides X command and these Y skills."

A module closes that gap. It is the *distribution unit*; the plugin and skills are its
*contents*.

### module vs plugin vs skill

| Term       | What it is                                  | Lives where                          | Consumed by |
| ---------- | ------------------------------------------- | ------------------------------------ | ----------- |
| **module** | A repo bundling a plugin and/or skills      | repo root: `meta.module.toml`        | `meta` (discovers & wires up) |
| **plugin** | A command capability added to the `meta` CLI | manifest or binary inside the module | `meta` (runs the command)     |
| **skill**  | Claude Code automation (`SKILL.md`)         | `SKILL.md` dir inside the module     | Claude Code (loaded as a skill) |

Rule of thumb: if a file adds a `meta <verb>` command it belongs to the **plugin**; if it
is a `SKILL.md` tree Claude loads, it belongs to a **skill**; the `meta.module.toml` that
points at both is the **module**.

Analogy: an npm package (module) that contains a CLI (plugin) and its docs. A module may
ship only a plugin, only skills, or both.

## Conceptual model

```
module  (the deliverable: one repo)
├── plugin(s)   → commands meta gains          (existing concept, unchanged)
└── skill(s)    → Claude automation for them   (existing concept, unchanged)
```

The module manifest does not reinvent either half. The plugin half reuses the existing
`PluginManifest` schema (`plugin.manifest.toml`) or a protocol-plugin binary; the skill
half reuses the existing `SKILL.md` format and the skill audit/install path. The module
manifest is a thin index that references both.

## Module manifest (`meta.module.toml`)

A module repo declares itself with a manifest at its root. Recognized filenames, in
priority order (mirroring `MANIFEST_FILENAMES` in
`meta-core/src/plugin_manifest.rs`):

```
meta.module.toml
meta.module.yaml
meta.module.yml
meta.module.json
```

### Schema

```toml
[module]
name = "example"                  # unique module id within a workspace
version = "0.1.0"                 # semver; used for display and (future) pinning
description = "Example meta module bundling a plugin and its skills"
author = ""                       # optional
repository = ""                   # optional, source URL
min_meta_version = "0.27.0"       # optional; refuse to wire up on older meta

# The plugin(s) this module provides. Zero or more.
# A module with no plugins is a skill-only module (valid).
[[module.plugins]]
manifest = "plugin/plugin.manifest.toml"   # path (relative to repo root) to an
                                           # existing plugin.manifest.* — reuses the
                                           # manifest-plugin schema verbatim.
# OR, for a protocol/SDK plugin instead of a manifest plugin:
# binary   = "target/release/metarepo-plugin-example"
# protocol = true                          # speak Plugin Protocol v1 over stdio

# The skill(s) this module ships. Zero or more.
# Each is a directory containing a SKILL.md, installed through the existing
# skill path (and subject to the same audit gate).
[[module.skills]]
path = "skills/example-skill"              # dir containing SKILL.md

[[module.skills]]
path = "skills/example-helper"
```

### Field semantics

- `module.name` — identifies the module in `.meta` and in `meta module` output. Must be
  unique within a workspace.
- `module.version` / `module.min_meta_version` — display today; the hooks for future
  pinning and integrity (see [PLUGIN_INTEGRITY.md](PLUGIN_INTEGRITY.md) for the model this
  would mirror).
- `module.plugins[]` — each entry is **either** `manifest = <path>` (a manifest plugin,
  executed via the existing manifest-plugin path) **or** `binary = <path>` with optional
  `protocol = true` (a protocol/SDK plugin). All paths are relative to the repo root and
  resolve *inside* the module repo; they are validated by the existing plugin path policy
  before anything is spawned.
- `module.skills[]` — each `path` is a directory containing a `SKILL.md`. Installed via
  the same routine as `meta skill steal`, including the audit gate.

A manifest with no `[[module.plugins]]` and no `[[module.skills]]` is an error (a module
must contribute something).

## Canonical module repo layout

```
my-module/
├── meta.module.toml
├── plugin/
│   ├── plugin.manifest.toml      # reuses the existing manifest-plugin schema
│   └── run.sh                    # or a built / protocol binary
└── skills/
    └── example-skill/
        └── SKILL.md
```

A skill-only module omits `plugin/`; a plugin-only module omits `skills/`. The directory
names `plugin/` and `skills/` are convention only — the manifest paths are authoritative.

## Commands

```
meta module status <repo>      Preview what enabling a module would wire up (dry run)
meta module enable <repo>      Stage the module's plugin(s) and install its skill(s)
meta module disable <name>     Reverse an enable
meta module list               List enabled modules
meta module scan <dir>         Walk a directory and list module manifests found
```

`enable` accepts `--force` (install skills despite HIGH-severity audit findings) and
`--overwrite` (replace already-registered plugins/skills of the same name).

## Discovery & lifecycle

1. A repo is added as a project via `meta project add`. After the add, meta checks the
   repo root for a `meta.module.*` manifest.
2. If found, meta surfaces the module (name, version, the plugins and skills it would wire
   up). In a TTY it **asks for confirmation** before changing anything; non-interactively
   it just prints the `meta module enable <repo>` command to run. Discovery is passive;
   activation is always explicit.
3. On `enable`, for each declared item:
   - **plugin** — the plugin's files are **staged** into
     `<workspace>/.meta-modules/<module>/<plugin>/` (a copy, so the workspace owns them and
     integrity applies), and a `file:` spec is recorded in the `.meta` `plugins` map. The
     staged path is an allowed root for the plugin path policy (`validate_plugin_path`), so
     the plugin then loads through the unchanged external-plugin machinery on the next run.
   - **skill** — the `SKILL.md` directory is installed through the existing skill `steal`
     path, which runs the audit gate: HIGH-severity findings (e.g. `curl … | sh`, `rm -rf`,
     wildcard `allowed-tools`) block the install unless `--force` is passed. See
     [SKILL_TOOLS.md](SKILL_TOOLS.md) for the audit severities.
4. meta records the module under a new `.meta` `modules` map (`name -> repo-relative path`)
   so it can be listed and disabled. `disable` re-derives the contributed names from the
   manifest at that path and removes the staged plugins, their `plugins`/lockfile entries,
   the installed skills, and the `modules` entry.

A module's plugin commands nest under the plugin's own name, exactly as a standalone
manifest plugin would — e.g. an `example` plugin with an `example-hello` command runs as
`meta example example-hello`.

If `min_meta_version` is set and the running meta is older, `enable`/`status` report the
required version and refuse to wire the module up.

> Staging uses `.meta-modules/`, **not** `.metarepo/`: the canonical config filename is
> `.metarepo`, so a `.metarepo/` directory would shadow the config during discovery and be
> impossible to create when the config is itself a `.metarepo` file. (Config discovery now
> also matches config *files* only, so a like-named directory is never treated as config.)

## Relationship to existing systems

The module layer is deliberately thin — it indexes existing systems rather than
duplicating them:

- **Plugin half** reuses `PluginManifest` / `MANIFEST_FILENAMES`
  (`meta-core/src/plugin_manifest.rs`), the manifest-plugin execution path
  (`meta/src/plugins/manifest_plugin.rs`), and the path-security policy
  (`meta/src/plugins/plugin_loader.rs`). A module plugin is just a workspace plugin whose
  files happen to live inside a project repo.
- **Skill half** reuses the skill frontmatter parser (`meta/src/plugins/skill/`), the
  audit gate (`audit.rs`), and the install/copy routine (`steal.rs` / `plugin.rs`). A
  module skill is just a stolen skill whose source is a project repo.
- **Workspace config** — modules compose with the existing `plugins` map in `MetaConfig`
  (`meta-core/src/lib.rs`); they do not replace it.

Because the halves are unchanged, an author who already ships a manifest plugin or a skill
can turn their repo into a module by adding a `meta.module.toml` that points at what they
already have.

## Out of scope

Intentionally deferred:

- Module-level version pinning of its own (module plugins reuse the existing plugin
  lockfile, but there is no `modules`-level pin or `.meta.lock` equivalent yet). See
  [PLUGIN_INTEGRITY.md](PLUGIN_INTEGRITY.md).
- A `meta module init` scaffolder and a publishing/registry story beyond "it is a git
  repo in the workspace."
- A `meta sync` discovery sweep — discovery currently hooks `meta project add` only.

See also: [PLUGIN_DEVELOPMENT.md](PLUGIN_DEVELOPMENT.md),
[PLUGIN_PROTOCOL_V1.md](PLUGIN_PROTOCOL_V1.md),
[PLUGIN_INTEGRITY.md](PLUGIN_INTEGRITY.md), [SKILL_TOOLS.md](SKILL_TOOLS.md).
