# one

One next task, not a project manager.

```sh
one add "check token expiry"
one
one done
cat TODO.md | one
one < TODO.md
```

## Persistent tasks

All directories share one local list, not one list per project. `add` accepts a
trimmed nonempty single line (up to 2,000 Unicode characters, no control
characters). Up to 1,000 unfinished tasks are accepted; duplicates are allowed.
Adding does not select a task or change an existing selection.

Bare `one` uniformly chooses one unfinished task and persists `selected_task_id`
before displaying just that task. Subsequent invocations keep that task regardless
of RNG seed or cwd. `done` removes the selected task and clears its ID; the next
bare invocation selects again. There is no completion history. With no selected
task, `done` succeeds without choosing/completing anything. Empty lists succeed
with `Nothing to choose from.` No priorities, projects, deadlines, tags or scores.

State: `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/one/tasks.json`.
The shared store provides mode 0700 directories, mode 0600 files, nonblocking OS
locks (`one/tasks.lock`), atomic replacement and keyed, versioned JSON validation.
Invalid/relative XDG falls back to absolute HOME. Corruption, unknown schemas,
untrusted permissions, symlinks and lock contention fail closed: exit 1, no reset
or task overwrite. Locks cover read/select/write as one operation; concurrent
callers may need to retry on exit 1. State is retained on uninstall.

Tasks are **local plaintext**, not encrypted. Never enter secrets. Command-line
text may also enter shell history/process lists. Only unfinished tasks remain in
the current record; deletion does not promise secure erasure from storage/backups.

## Temporary input

Pipes, sockets and redirected regular files supply a temporary list. `/dev/null`
and terminal stdin use persistent tasks instead. `add` and `done` ignore stdin;
`done` never completes a temporary selection.

Input must be UTF-8, at most 1 MiB and 1,000 remaining tasks; each task has the same
2,000-character limit. Reads wait for EOF. Invalid encoding, control characters or
limits return 1; no partial choice is displayed. Filtering is deliberately small:

- Trim lines; drop empty lines and ATX headings (`#`–`######` followed by whitespace/end).
- Strip `-`, `*`, `+`, or decimal digits followed by `.`/`)` and whitespace.
- Drop `[x]`/`[X]` tasks, strip `[ ]` (checkbox requires whitespace/end after `]`).
- Other text remains literal. No full Markdown parsing, setext/fence handling or inference.

Exactly one remaining line is uniformly selected; duplicate lines remain separate
candidates. Empty lists succeed. Temporary selection **never opens local state**,
so it works even when persistent state is unavailable or corrupt and cannot
change the persistent selection. Nothing is executed or fetched from the network.

## CLI and tests

Success is 0, I/O/state errors 1, invalid CLI arguments 2. Supports `--help`,
`--version`, `--verison`, with English/Chinese help and messages selected by the
first nonempty `LC_ALL` → `LC_MESSAGES` → `LANG`. User task text is not translated.
Linux/macOS/WSL Unix are targets; this delivery was tested on Linux.

Hidden hook: `ONE_SEED=<u64>` fixes choices for the same version and candidate
order. Invalid/unset values use entropy. It does not affect IDs or an existing
selection and is intentionally absent from help.

Tests: `cargo test -p one` (selection, pipe isolation, record validation,
permissions/symlinks, held locks, concurrent writes, locale and non-UTF-8 paths).
