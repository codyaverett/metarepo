# metarepo-plugin-sdk

SDK for authoring external plugins for the [metarepo](https://github.com/codyaverett/metarepo)
CLI. Implement one trait, call `serve()`, and the v1 stdio wire protocol —
request framing, JSON, dispatch, error handling, and the protocol-version
handshake — is handled for you.

```rust
use metarepo_plugin_sdk::{serve, ArgInfo, CommandInfo, Plugin, RuntimeConfigDto};

struct Hello;

impl Plugin for Hello {
    fn name(&self) -> &str { "hello" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }

    fn commands(&self) -> Vec<CommandInfo> {
        vec![CommandInfo::new("hello", "Greeting commands").subcommand(
            CommandInfo::new("greet", "Print a greeting")
                .arg(ArgInfo::new("name", "Name to greet", true)),
        )]
    }

    fn handle(
        &self,
        _command: &str,
        args: &[String],
        _config: &RuntimeConfigDto,
    ) -> anyhow::Result<Option<String>> {
        let name = args.get(1).map(String::as_str).unwrap_or("world");
        Ok(Some(format!("Hello, {name}!")))
    }
}

fn main() -> anyhow::Result<()> {
    serve(Hello)
}
```

The protocol types (`PluginRequest`, `PluginResponse`, `CommandInfo`, `ArgInfo`,
`RuntimeConfigDto`) are re-exported from `metarepo_core::protocol`, so a plugin
depends only on this crate.

- Full guide: [`docs/PLUGIN_DEVELOPMENT.md`](https://github.com/codyaverett/metarepo/blob/main/docs/PLUGIN_DEVELOPMENT.md)
- Wire protocol: [`docs/PLUGIN_PROTOCOL_V1.md`](https://github.com/codyaverett/metarepo/blob/main/docs/PLUGIN_PROTOCOL_V1.md)
- Configuration: [`docs/PLUGIN_CONFIG.md`](https://github.com/codyaverett/metarepo/blob/main/docs/PLUGIN_CONFIG.md) — declare settings, edit via `meta config`, read with `plugin_config`
- Reference plugin: [`examples/metarepo-plugin-example`](https://github.com/codyaverett/metarepo/tree/main/examples/metarepo-plugin-example)

## Testing your plugin

`serve_io(plugin, reader, writer)` runs the request loop against arbitrary
streams, so you can drive it with in-memory buffers in unit tests. You can also
unit-test the `Plugin` trait methods directly, independent of the transport.

## License

MIT
