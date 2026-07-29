# Metarepo Example Plugin

The canonical reference for writing a metarepo external plugin with
[`metarepo-plugin-sdk`](../../metarepo-plugin-sdk). It implements the
[`Plugin`] trait and calls `serve()` — the SDK handles the v1 stdio wire
protocol (framing, JSON, the version handshake) so this crate contains no
protocol boilerplate.

## Commands

- `meta example hello [name]` — greet someone
- `meta example info` — show information about the current meta repository
- `meta example count` — count projects in the repository
- `meta example config` — show the resolved `[example]` settings and where each
  value came from

## How it works

- `src/lib.rs` implements the SDK `Plugin` trait: `name`, `version`,
  `commands` (a declarative command tree), `settings` (the config keys this
  plugin understands), and `handle` (the dispatch).
- `src/main.rs` is just `serve(ExamplePlugin::new())` in subprocess mode.

There is no hand-written stdin loop, no JSON parsing, and no protocol type
definitions — those all live in the SDK.

## Configuration

This plugin doubles as the worked example for
[plugin configuration](../../docs/PLUGIN_CONFIG.md). It declares its keys from
`Plugin::settings`, so the host folds them into the `meta config` catalog:

```bash
meta config list | grep example
meta config set example.greeting metarepo
```

At runtime it mirrors its `[example]` block as a typed struct and reads it with
one call:

```rust
#[derive(Debug, Default, Deserialize)]
struct ExampleSettings {
    greeting: Option<String>,
    #[serde(rename = "max-projects")]
    max_projects: Option<usize>,
}

let settings: ExampleSettings = config.plugin_config("example").unwrap_or_default();
```

Every field is optional and the struct is `Default`, so the plugin works with no
`[example]` block — or no `.meta` at all. Values resolve argument > `[example]` >
built-in default:

```toml
# .meta
[example]
greeting = "metarepo"
max-projects = 25
```

```bash
meta example hello        # "Hello, metarepo!"  (from [example])
meta example hello Ada    # "Hello, Ada!"       (argument wins)
meta example config       # each value plus its source
```

Declaring a key and reading it are separate steps kept in sync by hand: a key in
`settings()` must have a matching field on `ExampleSettings`.

## Build

```bash
cargo build --release
```

This produces the executable `target/release/metarepo-plugin-example`. The host
runs it as a subprocess; there is no dynamic-library mode.

## Install

Place the binary in one of metarepo's allowed plugin directories and reference
it from `.metarepo`:

```bash
mkdir -p ~/.config/metarepo/plugins
cp target/release/metarepo-plugin-example ~/.config/metarepo/plugins/
```

```jsonc
// .metarepo
{
  "projects": {},
  "plugins": {
    "example": "file:~/.config/metarepo/plugins/metarepo-plugin-example"
  }
}
```

Allowed locations (enforced by the host, see the security policy in
`docs/PLUGIN_DEVELOPMENT.md`): `~/.config/metarepo/plugins/`, `~/.cargo/bin/`,
and `<workspace>/.metarepo/plugins/`. For local iteration outside those, set
`METAREPO_PLUGIN_ALLOW_ANY_PATH=1`.

Then the commands are available:

```bash
meta example hello World
meta example info
meta example count
```

## Develop and debug

Run the unit tests (they exercise the trait directly):

```bash
cargo test
```

Run standalone to see the usage banner:

```bash
cargo run
```

Drive the protocol by hand in subprocess mode:

```bash
METAREPO_PLUGIN_MODE=1 ./target/release/metarepo-plugin-example <<'EOF'
{"type":"GetInfo"}
{"type":"RegisterCommands"}
{"type":"HandleCommand","command":"example","args":["hello","World"],"config":{"meta_config":{"projects":{}},"working_dir":"/tmp","meta_file_path":null,"experimental":false}}
EOF
```

## Use as a template

1. Copy this directory.
2. Update `Cargo.toml` (name `metarepo-plugin-<yours>`, your metadata).
3. Rewrite the `Plugin` impl in `src/lib.rs`; leave `src/main.rs` as-is.
4. `cargo test`, then build and install as above.

See `docs/PLUGIN_DEVELOPMENT.md` for the full guide and
`docs/PLUGIN_PROTOCOL_V1.md` for the wire protocol.

## License

MIT — see the main metarepo project for details.

[`Plugin`]: ../../metarepo-plugin-sdk/src/lib.rs
