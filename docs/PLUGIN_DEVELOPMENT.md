# Metarepo Plugin Development Guide

How to extend the `meta` CLI with new commands. This is the single source of
truth for plugin development; it describes what works **today** and flags what
is planned.

## Plugin kinds

Metarepo has two kinds of plugins:

1. **Built-in plugins** — compiled into the `meta` binary (`init`, `skill`,
   `git`, `project`, `config`, `exec`, `rules`, `worktree`, `run`, and the
   plugin manager). You don't install these; they ship with metarepo.
2. **External plugins** — separate executables that metarepo runs as
   subprocesses, communicating over a newline-delimited JSON protocol on
   stdin/stdout. This is what you build to add your own commands.

External plugins can be written in **any language** that can read stdin, write
stdout, and parse JSON. For Rust, the `metarepo-plugin-sdk` crate hides the
protocol entirely. For other languages you implement the protocol directly
(it's small — see `docs/PLUGIN_PROTOCOL_V1.md`).

> **Status note.** Some management ergonomics are still in progress and are
> called out as **Planned** below: the `meta plugin install/list/remove/update`
> CLI (#24), version pinning + checksums (#25), manifest (argv-only) plugins
> (#26), and first-party cross-language templates (#27). Until those land,
> external plugins are installed by placing a binary in an allowed directory
> and referencing it from `.metarepo`.

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

## Other languages

Any executable that speaks the protocol works. Detect `METAREPO_PLUGIN_MODE=1`,
then loop over stdin lines: parse each JSON request, dispatch on its `type`,
write one JSON response line, and **flush stdout**. A ~30-line Python or
~50-line Bash plugin is realistic. See `docs/PLUGIN_PROTOCOL_V1.md` for the
exact messages and a transcript.

> **Planned (#27):** first-party templates for Node, Python, and Go under
> `examples/`. Until then, the protocol doc plus the Rust example are the
> reference.

## Installing a plugin

External plugins are loaded as subprocesses. The host enforces a path policy
(see Security) and reads the plugin list from your workspace config.

1. Build a release binary and place it in an **allowed directory**:

   ```bash
   cargo build --release
   mkdir -p ~/.config/metarepo/plugins
   cp target/release/metarepo-plugin-hello ~/.config/metarepo/plugins/
   ```

2. Reference it from `.metarepo` under `plugins`:

   ```jsonc
   {
     "projects": {},
     "plugins": {
       "hello": "file:~/.config/metarepo/plugins/metarepo-plugin-hello"
     }
   }
   ```

3. Run it:

   ```bash
   meta hello greet Ada
   ```

Plugin spec forms in `.metarepo`:

- `file:<path>` — a local executable (supported today).
- `git+<url>` — **Planned (#24)**: clone + build. Not yet implemented.
- bare version string (e.g. `"1.2.0"`) — resolves to
  `~/.cargo/bin/metarepo-plugin-<name>` if present; full registry resolution is
  **Planned (#24/#25)**.

> **Planned (#24):** `meta plugin install / list / remove / update` to manage
> all of the above without hand-editing `.metarepo`. See issue #24.

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
