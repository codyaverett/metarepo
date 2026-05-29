# metarepo plugin template — Go

A minimal, single-file metarepo plugin written in Go. Speaks the v1 protocol
directly — stdlib only.

For shell / argv-only plugins, prefer a [manifest plugin](../metarepo-plugin-shell)
instead; this template is for a real Go binary that needs the full protocol.

## Quick start

```bash
go build -o metarepo-plugin-hello
meta plugin install hello --from file:./metarepo-plugin-hello
meta hello greet Ada
# -> Hello, Ada! (cwd: ...)
```

## Smoke test (without installing)

```bash
go build -o metarepo-plugin-hello
METAREPO_PLUGIN_MODE=1 ./metarepo-plugin-hello <<'EOF'
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

About 130 lines, stdlib only. The wire format is documented in
[`docs/PLUGIN_PROTOCOL_V1.md`](../../docs/PLUGIN_PROTOCOL_V1.md).

## Requirements

Go 1.21+.
