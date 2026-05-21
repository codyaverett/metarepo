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

## Planned work

The management ergonomics the old guide assumed are tracked under the plugin
epic (#21): `meta plugin install/list/remove/update` (#24), version pinning and
checksums (#25), manifest/argv-only plugins (#26), and cross-language templates
(#27).
