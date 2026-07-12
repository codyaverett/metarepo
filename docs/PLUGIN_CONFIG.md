# Plugin & Module Configuration

How plugins declare configurable settings, how users edit them through
`meta config`, and how a plugin reads its own settings at runtime.

## Overview

Settings live as typed blocks in the workspace config (`.meta`). A plugin
**declares** the settings it understands; the host **aggregates** them so they
are discoverable via `meta config`; the plugin **reads** its own block with a
typed accessor. No hand-editing of `.meta` and no guessing block names.

```
declare (MetaPlugin::settings)
   │
   ├─ meta config list / get / set   ← users discover & edit
   │
   └─ config.plugin_config::<T>(name) ← plugin reads at runtime
```

## 1. Declare settings

Implement `MetaPlugin::settings`, returning one `ConfigSetting` per option.
Keys are dotted and namespaced by the plugin (`skill.dest`, `skill.search-url`).

```rust
use metarepo_core::{ConfigSetting, ConfigValueType};

fn settings(&self) -> Vec<ConfigSetting> {
    vec![
        ConfigSetting::new("skill.dest",
            "Default install directory for skills (overridden by --dest)",
            ConfigValueType::String),
        ConfigSetting::new("skill.search-limit",
            "Default number of hits for skill search",
            ConfigValueType::Integer).with_default("25"),
        ConfigSetting::new("plugins-integrity",
            "Plugin checksum-integrity enforcement",
            ConfigValueType::String)
            .with_default("off")
            .with_choices(["off", "required"]),
    ]
}
```

`ConfigValueType` is one of `String`, `Bool`, `Integer`, `StringList`. The type
drives validation (`meta config set` rejects mismatched input) and display.

Builder options refine a setting further:

- `.with_default("...")` — value shown (and used) when the key is unset.
- `.with_env("ENV_VAR")` — an environment variable that also controls the
  setting; `meta config list` flags when it is currently overriding the config.
- `.with_choices(["a", "b"])` — constrain a `String` setting to a fixed set of
  values. `meta config set` then rejects anything outside the list, `meta config
  list` shows the allowed `choices:`, and the interactive editor offers an
  inline cycle-picker (press the edit key to advance to the next value) instead
  of free-text entry, mirroring the in-place toggle for `Bool` settings.

## 2. Edit via `meta config`

- `meta config list` — every declared setting with type, description, default,
  and current value.
- `meta config get <ns>.<key>` — effective value (falls back to the declared
  default when unset).
- `meta config set <ns>.<key> <value>` — validated against the declared type;
  creates the owning block if absent. `StringList` accepts a comma-separated
  list or a JSON array; values may start with `-`.

```console
$ meta config set skill.search-limit 50
✓ Config updated: skill.search-limit = 50
$ meta config get skill.search-limit
50
```

### Interactive editor — `meta config edit`

`meta config edit` opens a full-screen TUI with a **Config Tree** pane on the left
and a detail/edit panel on the right. Keys:

| Key | Action |
| --- | --- |
| `↑`/`k`, `↓`/`j` | Move selection up / down |
| `→`/`l`, `Enter`, `Space` | Expand the node (or start editing a leaf value) |
| `←`/`h` | Collapse the current node; if already collapsed (or a leaf), jump to and collapse its **parent**. Repeats up the tree at any depth. |
| `PgUp`/`PgDn`, `Home`/`g`, `End`/`G` | Page / jump to top / bottom |
| `e` | Edit the selected value · `a` add · `d` delete · `/` search |
| `s` / `Ctrl-w` | Save · `q`/`Esc` quit (guards unsaved edits) |

**Scrolling.** The tree viewport follows the selection: navigation keeps the
selected row on screen, and **expanding a branch scrolls down to reveal as much of
the newly shown children as possible** while keeping the parent row visible. If the
expanded subtree already fits, nothing shifts; if it is taller than the pane, the
parent is pinned to the top so the maximum number of children show beneath it. This
holds at every tree depth.

## 3. Read settings at runtime

Define a `Deserialize` struct mirroring your block and read it with
`RuntimeConfig::plugin_config`:

```rust
#[derive(serde::Deserialize, Default)]
struct MySettings {
    #[serde(rename = "search-limit")]
    search_limit: Option<usize>,
}

let settings: MySettings = config.plugin_config("myplugin").unwrap_or_default();
```

Built-in plugins may instead read a typed field directly (e.g.
`config.meta_config.skill`).

## Precedence

Resolve in this order, stopping at the first set value:

```
CLI flag  >  environment variable  >  plugin config (.meta)  >  built-in default
```

Example — the skill plugin's search limit is `--limit` flag, else
`[skill] search-limit`, else `25`; its API key is `SKILLS_SH_API_KEY` env, else
`[skill] api-key`. Keep secrets in the environment, not in `.meta`.

## External (subprocess) plugins

External plugins receive the config snapshot over the wire and can call the same
`plugin_config` accessor on the DTO's `meta_config`.

They also **declare** their settings to the host: implement `Plugin::settings()`
in the SDK (returns `Vec<ConfigSetting>`). The host requests them over the
protocol (`GetSettings`, protocol 1.1+) at load time and folds them into the
`meta config` catalog, so `meta config list` / `get` / `set` cover external
plugins exactly like built-in ones. A 1.0 plugin that predates this simply
declares nothing.

## Reference

- Types: `metarepo-core/src/config_setting.rs`
- Trait method: `MetaPlugin::settings` (`metarepo-core/src/lib.rs`)
- Accessor: `MetaConfig::plugin_settings` / `RuntimeConfig::plugin_config`
- Command: `meta/src/plugins/config/plugin.rs`
- First consumer: `meta/src/plugins/skill/plugin.rs`
