# Metarepo Manifest Plugin Example (shell)

A minimal **manifest plugin**: a shell script plus a `plugin.manifest.toml`. It
speaks no JSON protocol — metarepo reads the manifest to learn the commands and
execs the script with the parsed arguments.

## Files

- `plugin.manifest.toml` — declares the `greet` command, its `hello` subcommand,
  args, and `config.execution.binary = "./greet.sh"`.
- `greet.sh` — receives the subcommand and args as argv, plus `METAREPO_ROOT`,
  `METAREPO_CONFIG_PATH`, `METAREPO_PROJECT`, and `METAREPO_ARG_<NAME>` env vars.

## Install and run

```bash
meta plugin install greet --from file:./examples/metarepo-plugin-shell
meta greet hello Ada
meta greet hello Bob --loud
meta plugin remove greet --purge
```

`install` detects the manifest, copies it and `greet.sh` into
`~/.config/metarepo/plugins/greet/`, and registers it in `.metarepo`. The
manifest's version shows in `meta plugin list`.

## When to use a manifest plugin vs a protocol plugin

- **Manifest plugin** (this example): any executable that wants parsed argv and
  an exit code. Best for shell/Python/Go scripts. No dependency on the SDK.
- **Protocol plugin** (`../metarepo-plugin-example`): richer integrations that
  want structured access to workspace state over JSON. Use `metarepo-plugin-sdk`
  for Rust.

See `docs/PLUGIN_DEVELOPMENT.md` for both.
