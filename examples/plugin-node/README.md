# metarepo plugin template — Node.js

A minimal, single-file metarepo plugin written in Node.js. Speaks the v1
protocol directly over stdin/stdout — no dependencies, no framework.

Use this as a starting point for a Node-based protocol plugin. For shell /
argv-only plugins, prefer a [manifest plugin](../metarepo-plugin-shell)
instead.

## Quick start

```bash
chmod +x hello.mjs
meta plugin install hello --from file:./hello.mjs
meta hello greet Ada
# -> Hello, Ada! (cwd: ...)
```

> **Install name must match the registered command.** A protocol plugin's
> command name comes from its `RegisterCommands` response (`hello` here), not
> from the `meta plugin install <name>` argument. Install it under that same
> name — `meta plugin install hello` — or `meta <name> ...` won't resolve.
> (Manifest plugins read the name from the manifest, so they don't have this
> constraint.)

## Smoke test (without installing)

```bash
METAREPO_PLUGIN_MODE=1 ./hello.mjs <<'EOF'
{"type":"GetInfo"}
{"type":"RegisterCommands"}
EOF
```

You should see two JSON lines: an `Info` response and a `Commands` response.

## What the file does

- Detects `METAREPO_PLUGIN_MODE=1`. Outside that mode it prints a banner.
- In subprocess mode, reads newline-delimited JSON requests on stdin and
  writes one JSON response per line on stdout, flushing each.
- Dispatches on `GetInfo`, `RegisterCommands`, `HandleCommand` and returns
  `Info`, `Commands`, `Success`, or `Error`.

About 80 lines total. The wire format is documented in
[`docs/PLUGIN_PROTOCOL_V1.md`](../../docs/PLUGIN_PROTOCOL_V1.md).

## Requirements

Node.js 18+ (uses `node:readline` and `import` syntax).
