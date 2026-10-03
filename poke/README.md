# poke

```sh
poke add "Alex"
poke
poke remove "Alex"
```

A small global local name/nickname list. A reminder may say “If you like, say hello
to: Alex.” It never sends a message, reads system contacts or calls an API.

## Minimal behavior

- Add/remove trim surrounding whitespace. Names are nonempty single lines, at
  most 200 Unicode characters without controls; up to 1,000 names are accepted.
- Deduplicate by exact text, including case and Unicode representation: `Alex`
  and `alex` differ. No real-name requirement or Unicode normalization.
- Duplicate add and missing remove are friendly successes without rewriting the
  list. Empty lists suggest `poke add <name>` and return 0.
- Bare `poke` uniformly chooses a list entry; consecutive repeats are allowed.
  No persistent selection, anti-repeat algorithm, contact history, timestamps,
  reminder debt, scoring or scheduled background service.
- No stdin reads or pipe import, no contact lookup, no network or messaging.

## State and privacy

Plaintext names live in
`${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/poke/names.json`.
Relative/empty XDG falls back to absolute HOME. This list is shared across working
directories. Shared private state uses directories 0700, files 0600, atomic JSON
replacement, keyed checksums and `poke/names.lock` nonblocking OS locks.

Read-modify-write is under one lock. Busy, corrupt, unknown-schema, symlinked or
unsafe-permission state fails closed with exit 1; nothing is silently reset.
Concurrent calls may need retrying after contention. Selection never writes a
history or modifies the list, though first use can create the state/identity/lock.
Uninstall preserves names. Removing is not secure erasure from disks/backups.
Never enter secrets: names are not encrypted, and CLI text can enter shell history
or process lists.

## CLI and tests

`--help`, `--version`, `--verison` plus the three MVP forms above. Messages follow
the first nonempty `LC_ALL` → `LC_MESSAGES` → `LANG`; zh-CN/zh_CN/zh selects Chinese,
otherwise English. Names themselves are not translated. Success is 0, state/I/O
failure 1, invalid CLI input 2. Default Ctrl+C behavior, no terminal-mode changes.

Hidden hook: `POKE_SEED=<u64>` fixes uniform selection for the same version and list
order. Invalid values fall back to entropy. This is not a CLI option and does not
appear in help.

`cargo test -p poke` covers CLI/locales, exact deduplication, capacity, deterministic
repeats, global scope, private permissions, corrupt/symlinked/locked state,
concurrency and non-UTF-8 state paths. Linux/macOS/WSL are targets; only Linux was
validated this round. Full disk-failure injection and macOS/WSL remain pending.
