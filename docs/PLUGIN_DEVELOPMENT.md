# Metarepo Plugin Development Guide

How to extend the `meta` CLI with new commands. This is the single source of
truth for plugin development; it describes what works **today** and flags what
is planned.

## Plugin kinds

Metarepo has three kinds of plugins:

1. **Built-in plugins** — compiled into the `meta` binary (`init`, `skill`,
   `git`, `project`, `config`, `exec`, `rules`, `worktree`, `run`, and the
   plugin manager). You don't install these; they ship with metarepo.
2. **Protocol plugins** — separate executables that metarepo runs as
   subprocesses, communicating over a newline-delimited JSON protocol on
   stdin/stdout. Best for richer integrations that want structured access to
   workspace state. For Rust, the `metarepo-plugin-sdk` crate hides the
   protocol entirely; other languages implement it directly (it's small — see
   `docs/PLUGIN_PROTOCOL_V1.md`).
3. **Manifest plugins** — any executable plus a `plugin.manifest.*` file that
   declares its commands. metarepo execs the binary with parsed argv and
   context env vars; the binary never speaks the protocol. Best for shell /
   Python / Go scripts. See [Manifest plugins](#manifest-plugins).

> **Status note.** The `meta plugin install/list/remove/update` CLI, both
> protocol and manifest plugins, and cross-language templates (Node, Python,
> Go) are all available. Still **Planned**: version pinning + checksum
> enforcement (#25).

## Quick start (Rust, with the SDK)

The recommended path. Authoring a plugin is implementing one trait and calling
`serve()`.

### 1. Create the crate

```bash
cargo new --bin metarepo-plugin-hello
cd metarepo-plugin-hello
```

### 2. Depend on the SDK

```toml
# Cargo.toml
[package]
name = "metarepo-plugin-hello"
version = "0.1.0"
edition = "2021"

[dependencies]
metarepo-plugin-sdk = "0.20"
anyhow = "1.0"
```

### 3. Implement `Plugin` and call `serve`

```rust
use metarepo_plugin_sdk::{serve, ArgInfo, CommandInfo, Plugin, RuntimeConfigDto};

struct Hello;

impl Plugin for Hello {
    fn name(&self) -> &str {
        "hello"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    // Declarative command tree. The host rebuilds clap commands from this for
    // `meta --help` and argument routing.
    fn commands(&self) -> Vec<CommandInfo> {
        vec![CommandInfo::new("hello", "Greeting commands").subcommand(
            CommandInfo::new("greet", "Print a greeting")
                .arg(ArgInfo::new("name", "Name to greet", true)),
        )]
    }

    // `command` is the top-level name ("hello"); `args` is the parsed argv that
    // followed it (e.g. ["greet", "Ada"]). Return an optional message to print,
    // or an error to report failure.
    fn handle(
        &self,
        _command: &str,
        args: &[String],
        config: &RuntimeConfigDto,
    ) -> anyhow::Result<Option<String>> {
        match args.split_first() {
            Some((sub, rest)) if sub == "greet" => {
                let name = rest.first().map(String::as_str).unwrap_or("world");
                Ok(Some(format!(
                    "Hello, {name}! (cwd: {})",
                    config.working_dir.display()
                )))
            }
            _ => Ok(Some("usage: meta hello greet <name>".into())),
        }
    }
}

fn main() -> anyhow::Result<()> {
    serve(Hello)
}
```

That's the whole plugin. `serve` runs the request loop, handles framing and
parse errors, and answers the protocol-version handshake for you.

A complete, tested reference lives in
[`examples/metarepo-plugin-example`](../examples/metarepo-plugin-example).

### The `Plugin` trait

| Method | Required | Purpose |
| --- | --- | --- |
| `name(&self) -> &str` | yes | Top-level command namespace (`meta <name> ...`). |
| `version(&self) -> &str` | yes | Plugin semver, reported in `Info`. |
| `is_experimental(&self) -> bool` | no (default `false`) | If true, only loaded under `--experimental`. |
| `commands(&self) -> Vec<CommandInfo>` | yes | Declarative command tree. |
| `handle(&self, command, args, config) -> Result<Option<String>>` | yes | Execute a command; `Ok(Some(msg))` prints `msg`, `Err(e)` reports failure. |

`CommandInfo` and `ArgInfo` have builder helpers (`new`, `.arg`,
`.subcommand`). `RuntimeConfigDto` is a read-only snapshot of host state
(`meta_config`, `working_dir`, `meta_file_path`, `experimental`).

## Manifest plugins

For a shell script or any executable that just wants parsed arguments and an
exit code, skip the protocol entirely: ship a `plugin.manifest.*` describing the
commands. metarepo registers them without spawning the binary, and on
invocation execs it with the resolved subcommand and parsed args as **argv**,
plus context and per-argument **env vars**.

A manifest (`plugin.manifest.toml`, `.yaml`, or `.json`) declares the plugin and
its command tree, and points at the executable:

```toml
[plugin]
name = "greet"
version = "0.1.0"
description = "Example manifest plugin"

[[commands]]
name = "hello"
description = "Print a greeting"

[[commands.args]]
name = "name"
help = "Who to greet"
required = true
takes_value = true       # positional (no long/short) — passed as argv

[[commands.args]]
name = "loud"
long = "loud"            # a --loud boolean flag
help = "Shout the greeting"

[config.execution]
binary = "./greet.sh"    # relative to the manifest
```

The script receives:

- **argv** — the subcommand chain and args, e.g. `meta greet hello Ada --loud`
  runs the binary with `hello Ada --loud`.
- **`METAREPO_ARG_<NAME>`** — each parsed argument (`METAREPO_ARG_NAME=Ada`,
  `METAREPO_ARG_LOUD=1`).
- **context** — `METAREPO_ROOT`, `METAREPO_CONFIG_PATH`, `METAREPO_PROJECT`
  (when invoked inside a project), so it need not rediscover the workspace.

```bash
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  hello)
    name="${METAREPO_ARG_NAME:-world}"
    echo "Hello, ${name}!"
    ;;
  *) echo "usage: meta greet hello <name>" >&2; exit 1 ;;
esac
```

Exit `0` is success; a non-zero exit surfaces as a plugin error. A complete
example is in
[`examples/metarepo-plugin-shell`](../examples/metarepo-plugin-shell).

#### Manifest reference

`[plugin]`:

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | Top-level command (`meta <name> ...`). |
| `version` | yes | Reported by `meta plugin list`. |
| `description` | yes | Shown in `meta --help`. |
| `experimental` | no | If true, only loaded under `--experimental`. |
| `min_meta_version`, `author`, `license`, `homepage`, `repository` | no | Metadata only. |

`[[commands]]` (may nest via `subcommands`):

| Field | Notes |
| --- | --- |
| `name`, `description` | Required. |
| `aliases` | Visible aliases for the subcommand. |
| `subcommands` | Nested `[[commands]]` for arbitrary depth. |
| `args` | See below. |

`[[commands.args]]`:

| Field | Notes |
| --- | --- |
| `name` | Required. Used as the positional name and to derive `METAREPO_ARG_<NAME>`. |
| `help` | Required. |
| `long` / `short` | Omit both for a positional; supply one for a flag. |
| `takes_value` | A flag with `takes_value = true` accepts a value (e.g. `--name Ada`); without it, the flag is boolean. |
| `required`, `default_value`, `possible_values` | Standard clap semantics. |

`[config.execution]`:

| Field | Notes |
| --- | --- |
| `binary` | Path to the executable, relative to the manifest. Required. |

Install it the same way as any plugin — `install` detects the manifest, copies
it and the binary into `~/.config/metarepo/plugins/<name>/`, and registers it:

```bash
meta plugin install greet --from file:./examples/metarepo-plugin-shell
meta greet hello Ada --loud
```

`--from git+<url>` also works: if the repo root has a `plugin.manifest.*`, the
checked-in binary is used as-is (a cargo build runs only if the referenced
binary is missing and the repo is a cargo project).

## Other languages (protocol)

Any executable that speaks the protocol works without the SDK. Detect
`METAREPO_PLUGIN_MODE=1`, then loop over stdin lines: parse each JSON request,
dispatch on its `type`, write one JSON response line, and **flush stdout**. See
`docs/PLUGIN_PROTOCOL_V1.md` for the exact messages and a transcript. (For the
common argv-only case, prefer a manifest plugin above.)

Single-file starter templates are in-tree — copy one and edit:

| Language | Template | Notes |
| --- | --- | --- |
| Node.js | [`examples/plugin-node`](../examples/plugin-node) | `chmod +x hello.mjs` and install as `file:`. Node 18+. |
| Python | [`examples/plugin-python`](../examples/plugin-python) | `chmod +x hello.py` and install as `file:`. Python 3.8+, stdlib only. |
| Go | [`examples/plugin-go`](../examples/plugin-go) | `go build` then install the binary as `file:`. Go 1.21+. |

Each template is ~80–130 lines and implements `GetInfo`, `RegisterCommands`,
and `HandleCommand` directly. They share the same hello-world surface as the
Rust example, so the smoke-test JSON is interchangeable.

## Installing a plugin

Use `meta plugin install`, which installs the binary and registers it under
`plugins.<name>` in the active `.metarepo` so it loads on the next run (and
appears in `meta --help`). No hand-editing required.

```bash
# From crates.io (default crate: metarepo-plugin-<name>)
meta plugin install hello
meta plugin install hello --version 0.2.0

# From a local build
meta plugin install hello --from file:./target/release/metarepo-plugin-hello

# From a git repository (clones and runs cargo build --release)
meta plugin install hello --from git+https://github.com/me/metarepo-plugin-hello.git
```

Then:

```bash
meta hello greet Ada
```

### Managing plugins

```bash
meta plugin list              # status legend below
meta plugin update hello      # reinstall from the recorded spec
meta plugin update            # update all (crates/git sources)
meta plugin remove hello      # unregister from .metarepo
meta plugin remove hello --purge   # also delete the installed binary
```

`meta plugin list` status symbols:

| Symbol | Meaning |
| --- | --- |
| `✓ <name> [<source>] installed (vX)` | Binary present and (for protocol plugins) probes to vX, or manifest declares vX. |
| `✓ <name> [<source>] installed at <path>` | Binary present but not probeable (e.g. blocked by the allowed-path policy, or not protocol-speaking). |
| `⚠ <name> [<source>] version mismatch` | Spec declares one version; the installed binary reports another. Run `meta plugin update <name>`. |
| `✗ <name> [<source>] missing` | Registered in `.metarepo` but not installed. Run `meta plugin install <name>`. |

### Spec forms in `.metarepo`

`meta plugin install` writes one of these under `plugins.<name>`; you can also
set them by hand:

- `crates:<crate>` or `crates:<crate>@<version>` — install from crates.io
  (default crate `metarepo-plugin-<name>`). A bare version string like `"1.2.0"`
  is also accepted for back-compat.
- `file:<path>` — a local executable. `install` copies it into the plugins
  directory and records the destination path.
- `git+<url>` — clone and `cargo build --release`; the built
  `metarepo-plugin-*` binary is copied into the plugins directory.

Binaries land in an **allowed directory** (see Security): crates.io installs go
to `~/.cargo/bin`; `file:`/`git+` installs go to `~/.config/metarepo/plugins/`.

## Security policy (v0.14+)

Metarepo will only spawn a plugin binary whose path passes these checks:

- The path must not contain `..` segments (traversal guard).
- The canonical path must live inside one of:
  - `~/.config/metarepo/plugins/`
  - `~/.cargo/bin/` (where `cargo install metarepo-plugin-*` lands)
  - `<workspace>/.metarepo/plugins/`
- `METAREPO_PLUGIN_ALLOW_ANY_PATH=1` skips the **location** check (never the
  `..` check). Use it only for local development.

The `config` snapshot handed to plugins is sanitized first: dangerous env vars
and traversal-prone project keys are stripped before serialization.

> **Planned (#25):** version pinning and optional checksum integrity so a
> pinned plugin can be verified before it runs.

## Testing

Because the SDK separates your logic (the trait) from the transport (`serve`),
you can unit-test the trait directly:

```rust
#[test]
fn greets() {
    let out = Hello
        .handle("hello", &["greet".into(), "Ada".into()], &dto())
        .unwrap()
        .unwrap();
    assert!(out.contains("Hello, Ada!"));
}
```

To test the wire loop end to end, the SDK exposes `serve_io(plugin, reader,
writer)` so you can drive it with in-memory buffers (see the SDK's own tests).
You can also exercise the built binary directly:

```bash
METAREPO_PLUGIN_MODE=1 ./target/release/metarepo-plugin-hello <<'EOF'
{"type":"GetInfo"}
{"type":"RegisterCommands"}
EOF
```

## Publishing

Rust plugins publish to crates.io like any crate:

```toml
[package]
name = "metarepo-plugin-yourname"
version = "0.1.0"
license = "MIT OR Apache-2.0"
description = "..."
repository = "https://github.com/you/metarepo-plugin-yourname"

[dependencies]
metarepo-plugin-sdk = "0.20"
anyhow = "1.0"
```

```bash
cargo publish
```

Users install with `cargo install metarepo-plugin-yourname` (lands in
`~/.cargo/bin`, an allowed directory) and reference it from `.metarepo`.

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| `Plugin path ... is not in an allowed plugins directory` | Binary is outside the allowed roots. Move it, or set `METAREPO_PLUGIN_ALLOW_ANY_PATH=1` for dev. |
| `Plugin path must not contain '..' segments` | Resolve the path; the traversal guard never relaxes. |
| `Plugin does not declare a protocol_version` / `failed protocol check` | Plugin predates v1 or speaks a different major. Rebuild against the current SDK. |
| Host hangs after spawning | Plugin isn't flushing stdout after each response. |
| Command doesn't appear in `meta --help` | `commands()` returned an empty tree, or the plugin isn't listed/loadable from `.metarepo`. |

Enable debug logging:

```bash
RUST_LOG=debug meta hello greet Ada
```

## References

- Wire protocol: [`PLUGIN_PROTOCOL_V1.md`](./PLUGIN_PROTOCOL_V1.md)
- Reference plugin: [`examples/metarepo-plugin-example`](../examples/metarepo-plugin-example)
- SDK source: [`metarepo-plugin-sdk`](../metarepo-plugin-sdk)
- Plugin epic and roadmap: GitHub issue #21
