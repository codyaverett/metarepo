# Plugin Development (moved)

This document previously described an aspirational plugin architecture
(scaffold templates, decorator-based Python/JS SDKs, `meta plugin
scaffold/install/dev/test/validate`, a `plugin_runner`, etc.). Much of that was
never implemented and the content was misleading, so it has been retired to
avoid sending plugin authors down dead ends.

**For external plugins, use the current guide:**
[`PLUGIN_DEVELOPMENT.md`](./PLUGIN_DEVELOPMENT.md). It covers the
`metarepo-plugin-sdk` quick start, installation, the v0.14+ security policy,
testing, publishing, and troubleshooting — all accurate to what ships today.

For the wire protocol, see [`PLUGIN_PROTOCOL_V1.md`](./PLUGIN_PROTOCOL_V1.md).

## Built-in plugins (in-binary)

The one piece worth keeping from the old guide: built-in plugins compiled into
the `meta` binary can be defined with the builder API from `metarepo-core`
(`plugin()`, `command()`, `arg()`), which exists today. See the built-in
plugins under `meta/src/plugins/` (for example `init`, `skill`, `config`) for
real, current implementations of the `MetaPlugin` / `BasePlugin` traits.

Note that built-in plugins use `MetaPlugin` (clap-based, in-process), while
external plugins use the SDK's `Plugin` trait (declarative commands over the
subprocess protocol). They are different traits for different execution models;
don't mix them.

## Reference extractions: rules and mcp

The formerly built-in `rules` and `mcp` plugins now live as external plugin
crates in this repository (`plugins/metarepo-plugin-rules`,
`plugins/metarepo-plugin-mcp`) and are the reference implementations for both
execution models at once: each keeps its full clap surface by declaring every
command `takeover` (protocol 1.3, see
[`PLUGIN_PROTOCOL_V1.md`](./PLUGIN_PROTOCOL_V1.md)), so the host execs the
plugin binary with raw argv instead of dispatching over the wire. Use this
pattern when a plugin needs option flags, nested subcommands, or a long-running
command that owns stdin/stdout (the mcp stdio server).

The workspace dogfoods both: they are version-pinned under `plugins` in this
repo's `.meta`, installed to `~/.cargo/bin` with
`cargo install --path plugins/metarepo-plugin-<name>`, and CI reinstalls and
smoke-tests them through the `meta` binary on every push.

## Planned work

The management ergonomics the old guide assumed are tracked under the plugin
epic (#21): `meta plugin install/list/remove/update` (#24), version pinning and
checksums (#25), manifest/argv-only plugins (#26), and cross-language templates
(#27).
