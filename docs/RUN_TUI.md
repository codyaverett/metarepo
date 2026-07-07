# Interactive Run View (meta run --tui)

`meta run --tui` opens an interactive way to run workspace scripts: a fuzzy
picker to choose a script, followed by a live view that streams each project's
output as it runs. It is built on the same shared TUI foundations as
`meta status` and `meta worktree tui`, and reuses the `OutputManager` that backs
the parallel-run path.

```bash
meta run --tui            # pick a script, then watch it run live
meta run build --tui      # skip the picker; run "build" directly in the live view
```

`--tui` requires an interactive terminal; in a non-TTY (for example a CI job) it
exits with an error so scripted runs never hang waiting for input.

## Phase 1: the script picker

The picker lists the workspace's scripts, merged across the config cascade
(outer `.meta` defaults plus nearer overrides), each with its command and the
number of in-scope projects that define it.

| Key            | Action                                  |
| -------------- | --------------------------------------- |
| type any text  | Fuzzy-filter by script name or command  |
| `↑` / `↓`      | Move the cursor                         |
| `Enter`        | Run the highlighted script              |
| `Backspace`    | Delete a filter character               |
| `Esc`          | Clear the filter, or cancel if empty    |

This surface is a text-input field first: printable keys extend the filter
rather than triggering shortcuts. If a script name is given on the command line
(`meta run <script> --tui`) the picker is skipped.

## Phase 2: the live run view

The chosen script runs concurrently across every in-scope project that defines
it. Each project runs on its own worker thread; stdout and stderr are streamed
into a shared buffer that the view samples on a tick, so output appears as it is
produced.

- **Left pane** — one row per project with a status glyph (pending / spinner /
  `✓` done / `✗` failed) and elapsed time.
- **Right pane** — the selected project's combined stdout+stderr, tailing as it
  grows.
- **Footer** — the script name, a running/done/failed summary, and the keys.

| Key            | Action                                            |
| -------------- | ------------------------------------------------- |
| `j` / `k`, arrows | Select a project                               |
| `f`            | Toggle follow (auto-scroll to the newest output)  |
| `PgUp` / `PgDn`| Scroll the output pane                             |
| `q` / `Esc` / `Ctrl-C` | Cancel running jobs; press again (or once all are done) to exit |

Cancellation is cooperative: the first quit asks in-flight children to stop
(they are killed and marked `[cancelled]`); once everything has settled the view
stays open for inspection until you quit again. When the batch finishes on its
own, the view remains open so you can review output, and a per-project summary is
printed to the normal screen on exit.

## Scope of v1

- Ships in `meta run`. `meta exec --tui` (arbitrary commands, reusing the same
  runner and live view) is a tracked follow-up.
- Project selection matches the CLI default: the in-scope projects that define
  the chosen script. There is no in-view project multi-select yet.
- Output is rendered as plain text; ANSI color/escape parsing is deferred.
- Output buffers are unbounded in v1; a tail cap for very chatty scripts is a
  follow-up.

These cuts are tracked against issue #126 (epic #117).
