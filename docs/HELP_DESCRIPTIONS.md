# Command help descriptions (`helpDescription`)

Plugins, modules, and commands can carry a long, man-page-style help body that
renders as a `Description:` section at the bottom of `--help` output. Think of it
as the DESCRIPTION block of a man page, embedded in the subcommand.

- It renders **only on `--help`** (the long form), not `-h`.
- It appears **after** the auto-generated Options and Commands sections.
- End users can **override** it per command from `.meta` (see below).

## How it renders

```
$ meta project --help
Project management operations

Usage: meta project [OPTIONS] [COMMAND]

Options:
  -v, --version  Print version information

Commands:
  add   Add a project to the workspace
  list  List all projects in the workspace
  ...

Description:
  Manage the set of repositories tracked in a workspace's .meta file.

  A project is an entry under "projects" in .meta, mapping a local path
  to a git URL ...
```

## Declaring it (plugin/module authors)

### Built-in / Rust builder plugins

`PluginBuilder` and `CommandBuilder` both expose `.help_description(...)`:

```rust
plugin("project")
    .description("Project management operations")        // short, one line (-h)
    .help_description("Manage the set of repositories ...") // long man-page body (--help)
    .command(
        command("add")
            .about("Add a project to the workspace")
            .help_description("Clone, symlink, or import an existing repo ...")
    )
```

### Manifest plugins (`plugin.manifest.toml`)

Set `help_description` (alias `helpDescription`) on the plugin or any command:

```toml
[plugin]
name = "example"
description = "An example plugin"
help_description = """
The example plugin demonstrates the manifest format.

This text renders as a Description section on `meta example --help`.
"""

[[commands]]
name = "run"
description = "Run the example"
help_description = "Long, multi-paragraph help for the run command."
```

### Subprocess (protocol) plugins

The wire protocol (v1.2+) carries an optional `help_description` on `CommandInfo`.
Using `metarepo-plugin-sdk`:

```rust
CommandInfo::new("run", "Run the example")
    .help_description("Long, multi-paragraph help shown on --help.")
```

Older plugins that omit the field simply render no `Description:` section — it is
additive and backward compatible (the protocol major version is unchanged).

## Overriding from `.meta` (end users)

Add a `help_descriptions` map to `.meta`, keyed by **dotted command path**. A match
replaces whatever the plugin/module declared for that command:

```json
{
  "help_descriptions": {
    "project": "Our team's notes about how we use project tracking.",
    "project.add": "Always pass --bare for service repos; see CONTRIBUTING."
  }
}
```

- Key `"project"` targets `meta project`; `"project.add"` targets `meta project add`.
- The override wins over the author-declared description.
- Works at any depth.

## Notes

- The `Description:` header is styled to match the other help section headers
  (bold bright cyan); body lines are indented two spaces.
- `meta <cmd> help` shows the same output as `meta <cmd> --help`, so the section
  appears either way.
