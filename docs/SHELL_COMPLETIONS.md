# Shell Completions

`meta` can generate tab-completion scripts for your shell. Completions cover
every built-in subcommand and its flags.

```bash
meta completions <shell>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

The script is written to **stdout** — redirect it to the location your shell
loads completions from.

## Notes

- Output is **deterministic**: completions are always generated from the stable
  command set, so the script does not change depending on whether
  `-x`/`--experimental` was passed. Experimental subcommands are intentionally
  not included.
- The completion surface reflects the built-in commands compiled into your
  `meta` binary. Regenerate the script after upgrading `meta` to pick up new
  commands or flags.

## Install

### Bash

```bash
# System-wide (requires the bash-completion package)
meta completions bash | sudo tee /etc/bash_completion.d/meta > /dev/null

# Or per-user
meta completions bash > ~/.local/share/bash-completion/completions/meta
```

Reload your shell or `source` the file.

### Zsh

```bash
# Pick a directory that is on your $fpath, e.g.:
mkdir -p ~/.zsh/completion
meta completions zsh > ~/.zsh/completion/_meta
```

Ensure the directory is on `$fpath` and `compinit` runs in your `~/.zshrc`:

```zsh
fpath=(~/.zsh/completion $fpath)
autoload -U compinit && compinit
```

### Fish

```bash
meta completions fish > ~/.config/fish/completions/meta.fish
```

Fish loads it automatically on the next shell start.

### PowerShell

```powershell
meta completions powershell | Out-String | Invoke-Expression
```

To persist it, append the output to your PowerShell profile:

```powershell
meta completions powershell >> $PROFILE
```

### Elvish

```bash
meta completions elvish > ~/.config/elvish/lib/meta.elv
```

Then `use meta` in your `rc.elv`.
