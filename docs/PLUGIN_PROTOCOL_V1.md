# Metarepo Plugin Protocol v1

This document specifies the wire format and lifecycle for metarepo's external
plugin protocol. Any plugin that speaks this protocol can extend the `meta`
CLI with new subcommands, regardless of implementation language.

Status: **stable** as of metarepo v0.19.0.

A high-level Rust SDK, [`metarepo-plugin-sdk`](../metarepo-plugin-sdk), ships
as of v0.20.0 (#23): it implements this protocol as a `Plugin` trait plus a
`serve()` helper, so Rust authors never touch the wire format. This document is
the source of truth for the protocol itself and for everything else (Node,
Python, Go, hand-written implementations). For a task-oriented guide see
[`PLUGIN_DEVELOPMENT.md`](./PLUGIN_DEVELOPMENT.md).

> **Looking for a simpler option?** If your plugin is a shell script or any
> binary that just wants parsed argv and an exit code, see the
> manifest-plugins doc (#26) instead. Manifest plugins do not speak this
> protocol — they declare their commands in a static file and metarepo
> dispatches via argv. Protocol plugins are for richer integrations that
> need access to workspace state.

## Glossary

- **Plugin binary** — an executable that metarepo spawns as a subprocess.
- **Host** — the `meta` process that spawns the plugin.
- **Subprocess mode** — the plugin's runtime when invoked with the
  `METAREPO_PLUGIN_MODE=1` environment variable set. Outside subprocess mode
  the binary is free to print whatever it likes (e.g., a usage banner).

## Transport and framing

- **Transport**: plugin stdin / stdout, with stderr inherited from the host so
  panics and trace output reach the user.
- **Framing**: **newline-delimited JSON**. The host writes one JSON object per
  line to the plugin's stdin and reads one JSON object per line from the
  plugin's stdout.
- **Encoding**: UTF-8. No BOM.
- **Buffering**: the plugin **must flush** stdout after every response.
  Otherwise the host will block waiting for output that's sitting in a
  language-level buffer.

The host sends a single request and blocks until it reads the matching
response. There is no message ID — request/response order is implicit.

## Lifecycle

For each command invocation:

1. The host spawns the plugin binary with `METAREPO_PLUGIN_MODE=1` set.
2. The host sends `GetInfo`. The plugin replies with `Info`.
3. The host validates the reported `protocol_version` (see Compatibility).
   On mismatch, the host kills the subprocess and prints an error.
4. The host sends `RegisterCommands`. The plugin replies with `Commands`.
5. The host sends one or more `HandleCommand` requests. The plugin replies
   with `Success` (optionally carrying a message) or `Error`.
6. The host closes the plugin's stdin and reaps the subprocess.

A plugin should exit cleanly when stdin is closed. Use process exit code 0
on normal shutdown; non-zero only if the plugin itself crashes.

## Compatibility

Every `Info` response declares `protocol_version` (e.g. `"1.0"`). The host
treats compatibility by **major version**:

- Same major as the host's supported version → compatible. Additive minor
  changes are backwards-compatible.
- Different major → rejected with a clear error pointing the user at the
  appropriate SDK version.

`protocol_version` is a JSON string. The current value is `"1.0"`. A future
minor revision (e.g. `"1.1"`) might add optional fields; plugins built
against `"1.0"` continue to load. A breaking change requires bumping the
major version and is a deliberate event.

Plugins that omit `protocol_version` (legacy plugins built against
pre-v0.19 metarepo) are rejected with a hint to rebuild.

## Security

External plugins are sandboxed only by OS process boundaries. Metarepo
applies a path-policy check (#12) before spawning a plugin:

- The resolved binary path must not contain `..` segments.
- The canonical path must live inside one of:
  - `~/.config/metarepo/plugins/`
  - `~/.cargo/bin/` (where `cargo install metarepo-plugin-*` lands)
  - `<workspace>/.metarepo/plugins/`
- The escape hatch `METAREPO_PLUGIN_ALLOW_ANY_PATH=1` skips the location check
  but never the `..` check. Use only for local plugin development.

## Requests

All requests are JSON objects tagged by a top-level `"type"` field.

### GetInfo

```json
{ "type": "GetInfo" }
```

Sent immediately after spawn. Expect an `Info` response.

### RegisterCommands

```json
{ "type": "RegisterCommands" }
```

Asks the plugin to describe its CLI surface. Expect a `Commands` response.

### HandleCommand

```json
{
  "type": "HandleCommand",
  "command": "example",
  "args": ["hello", "world"],
  "config": { "...": "see RuntimeConfigDto" }
}
```

Asks the plugin to run a subcommand. `command` is the top-level subcommand
name; `args` is the remainder of the argv as parsed by the host. `config`
is a serialized snapshot of the host's runtime configuration (see below).

## Responses

All responses are JSON objects tagged by a top-level `"type"` field.

### Info

```json
{
  "type": "Info",
  "name": "example",
  "version": "0.1.0",
  "experimental": false,
  "protocol_version": "1.0"
}
```

- `name` — the plugin's top-level CLI namespace (e.g. `example` enables
  `meta example ...`).
- `version` — the plugin's own semver. Reported to the user and compared
  against the version pinned in `.metarepo` (#25).
- `experimental` — when true the plugin is only loaded if the user runs
  metarepo with `--experimental`.
- `protocol_version` — required. See Compatibility.

### Commands

```json
{
  "type": "Commands",
  "commands": [
    {
      "name": "example",
      "about": "Example plugin commands",
      "args": [],
      "subcommands": [
        {
          "name": "hello",
          "about": "Print a greeting message",
          "args": [
            { "name": "name", "help": "Name to greet", "required": true }
          ],
          "subcommands": []
        }
      ]
    }
  ]
}
```

The shape mirrors clap's command tree. `args` is positional + named
parameters as a flat list (named with `--flag` are reported with their long
name, no leading dashes). `subcommands` nests arbitrarily.

### Success

```json
{ "type": "Success", "message": null }
```

`message` is optional. If present the host prints it to stdout.

### Error

```json
{ "type": "Error", "message": "what went wrong" }
```

Surfaces to the user as `Plugin error: <message>`. Use this rather than
panicking — panics are observable on stderr but produce a worse user
experience.

## RuntimeConfigDto

A snapshot of host state sent with each `HandleCommand`. The canonical
definition lives in `metarepo_core::protocol` (re-exported by
`metarepo-plugin-sdk`). Fields:

- `meta_config` — the entire deserialized `.metarepo` (after sanitization, so
  dangerous env vars and traversal project keys are already stripped). See
  the `MetaConfig` type in `meta-core` for the schema.
- `working_dir` — string path; the host's cwd when the command was invoked.
- `meta_file_path` — optional string path; the resolved config file the host
  loaded, or null if running with no config.
- `experimental` — bool; whether `--experimental` was set.

Plugins should treat this as read-only context. There is no facility in v1
for a plugin to mutate config and have the host pick up the change.

## Example plugin transcript

```
$ meta example hello world
# host spawns plugin, METAREPO_PLUGIN_MODE=1

host -> plugin: {"type":"GetInfo"}
plugin -> host: {"type":"Info","name":"example","version":"0.1.0","experimental":false,"protocol_version":"1.0"}

host -> plugin: {"type":"RegisterCommands"}
plugin -> host: {"type":"Commands","commands":[ ...as above... ]}

host -> plugin: {"type":"HandleCommand","command":"example","args":["hello","world"],"config":{ ... }}
plugin -> host: {"type":"Success","message":"Hello, world!"}

# host closes stdin, plugin exits 0
```

## Minimal hand-written plugin (any language)

A plugin only needs to:

1. Detect `METAREPO_PLUGIN_MODE=1`. If unset, print usage and exit.
2. Read lines from stdin in a loop.
3. For each line, parse the JSON, dispatch on `type`, and write one JSON
   response line. **Flush stdout.**
4. Exit when stdin closes.

A 30-line Python plugin or 50-line Bash plugin is entirely feasible. The
canonical reference is `examples/metarepo-plugin-example/` (Rust).

## Versioning policy

- **Patch** (`1.0` → `1.0.x`): host accepts; no schema changes.
- **Minor** (`1.0` → `1.1`): additive only. New optional fields, new request
  / response types tolerated by older hosts that ignore unknown variants.
- **Major** (`1.x` → `2.0`): breaking. Existing plugins must be rebuilt.

Plugin authors should pin to the highest minor they support; the host's
major-version check does the rest.

## Roadmap tracked in #21

Done:

- `metarepo-plugin-sdk` Rust crate (#23) — implements this protocol as a
  `Plugin` trait and a `serve()` helper. Shipped in v0.20.0.

Planned:

- `meta plugin install/list/remove/update` (#24) — moves install/management
  out of `cargo install` + hand-edited config.
- Version pinning + checksum integrity (#25).
- Manifest-based plugins for shell/Python/argv-only use cases (#26).
- Cross-language templates (Node, Python, Go) (#27).
