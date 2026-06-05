# Disabling projects

Sometimes a project should stay in the `.meta` config but be excluded from
day-to-day commands — a stale repo, a broken clone, or work that is paused. Two
mechanisms turn a project off without removing its entry. Both feed the same
rule, so they behave identically everywhere:

> A project is **managed** when its per-project `enabled` flag is not `false`
> **and** its canonical key is not in the `disabled` set.

A disabled project is excluded from the directory-aware scope, from `--all`, and
from `--workspace`. It is reachable only when a command is explicitly told to
include it (see [Overriding](#overriding)).

## Option A — per-project `enabled: false`

Set `enabled: false` on a project entry in the full (metadata) form. Use this
when the project is structurally off and you want that documented next to the
project itself.

```json
{
  "projects": {
    "services/legacy-api": {
      "url": "git@github.com:org/legacy-api.git",
      "enabled": false
    }
  }
}
```

`enabled` only exists on the metadata form. A bare `"name": "url"` string entry
cannot carry it — use Option B for those.

## Option B — top-level `disabled` list

Add a `disabled` array at the top level of the config. Use this for bulk or
pattern-based muting, for turning off string-form projects, and when you want a
single place that lists everything currently off.

```json
{
  "disabled": ["services/legacy-api", "experiments/*", "frontend"],
  "projects": {
    "services/legacy-api": "git@github.com:org/legacy-api.git",
    "experiments/spike-a": "git@github.com:org/spike-a.git",
    "services/web": {
      "url": "git@github.com:org/web.git",
      "aliases": ["frontend"]
    }
  }
}
```

Each entry may be:

- a **project key** — `services/legacy-api`
- a **path or basename** — `legacy-api`
- an **alias** — `frontend` (resolves to `services/web`)
- a **wildcard** — `experiments/*` (matched against project keys)

Non-wildcard entries are resolved to a canonical project key before matching, so
an alias in the list disables the project it points to.

## Aliases cannot bypass a disable

Disabling is enforced on the **resolved canonical key**, not on the string the
user typed. Whichever name a project is referenced by — key, basename, or alias
— it collapses to the same key before the disable check runs. So a disabled
project cannot be reached by naming it with one of its aliases:

```json
{
  "disabled": ["old-thing"],
  "projects": {
    "old-thing": { "url": "...", "aliases": ["legacy"] }
  }
}
```

Here both `meta exec -p old-thing ...` and `meta exec -p legacy ...` skip the
project.

## Overriding

`meta exec` accepts `--include-disabled` to operate on disabled projects when
you explicitly want to:

```sh
# Skips disabled projects (default)
meta exec --all git status

# Includes disabled projects this run
meta exec --all --include-disabled git status

# Explicitly targeting a disabled project without the flag warns and skips:
meta exec -p services/legacy-api git status
#   Skipping disabled project 'services/legacy-api' (use --include-disabled to run it)
```

## Notes

- `disabled` is omitted from serialized configs when empty, and `enabled` is
  omitted when unset, so existing `.meta` files are unchanged on rewrite.
- The `ignore` field is unrelated: it lists gitignore-style file/dir patterns,
  not projects.
