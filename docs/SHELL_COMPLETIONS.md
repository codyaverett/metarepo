# Shell Completions

`meta` installs tab-completion scripts as part of `meta init` — there is no
separate completions command. Completions cover every built-in subcommand and
its flags.

## Installing

When you run `meta init` interactively, it detects your shell (from `$SHELL`)
and offers to install completions:

```
$ meta init
  📦 Initializing meta repository
  ✓ Created .metarepo with default configuration
  ✓ Updated .gitignore
  → Install zsh shell completions? [Y/n] y
  ✓ Installed zsh completions at ~/.oh-my-zsh/completions/_meta
  ✓ Refreshed shell completion cache
  · Restart your shell to load completions
```

To install without the prompt (or in scripts / CI), pass the flag:

```bash
meta init --with-completions      # just completions (plus normal init)
meta init --all                   # every optional component, incl. completions
```

In a non-interactive run (piped, `--non-interactive`) completions are installed
**only** when `--with-completions`/`--all` is given — otherwise `init` leaves
your home directory untouched.

## Where completions are written

`meta` auto-detects your shell and writes to the conventional location:

| Shell | Location | Notes |
|-------|----------|-------|
| zsh (oh-my-zsh) | `$ZSH/completions/_meta` | Already on `$fpath`; cache auto-refreshed |
| zsh (other) | `~/.zsh/completions/_meta` | Needs an `$fpath` entry — see below |
| fish | `~/.config/fish/completions/meta.fish` | Auto-loaded |
| bash | `~/.local/share/bash-completion/completions/meta` | Needs the bash-completion v2 package |
| elvish | `~/.config/elvish/lib/meta.elv` | Add `use meta` to `rc.elv` |

PowerShell and unrecognized shells are not auto-installed; `init` reports a
skipped step.

### zsh without oh-my-zsh

If you don't use oh-my-zsh, the script goes to `~/.zsh/completions/_meta` and
`init` prints the snippet to add to `~/.zshrc` (before `compinit` runs):

```zsh
fpath=(~/.zsh/completions $fpath)
autoload -U compinit && compinit
```

## Refreshing after an upgrade

The completion script reflects the commands compiled into your `meta` binary.
After upgrading `meta` (new commands or flags), re-run:

```bash
meta init --with-completions
```

For zsh this also clears `~/.zcompdump*` so the next shell rebuilds its cache.

## Notes

- Completions are generated from the **stable** command set. Experimental
  subcommands (`-x rules` / `plugin` / `mcp`) are intentionally excluded.
- `init` never edits your rc files automatically. For the non-oh-my-zsh zsh case
  it only prints the one-line `$fpath` snippet for you to add.
