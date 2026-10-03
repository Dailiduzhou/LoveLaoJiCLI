# goodnight

```sh
goodnight
```

A read-only session ending. Prints local time with UTC offset, branch or detached
HEAD, unfinished Git paths and one latest explicitly recorded command status.
It may suggest `later --next <text>` when unfinished paths exist, but never invokes
later, saves a card, shuts down programs, locks the shell or judges your schedule.
Outside Git it still prints time and a closing line.

## Evidence, not assumptions

Git discovery/status uses the existing bounded, local-only probes with optional
Git index writes disabled. Unborn/bare repositories, unavailable Git, conflicts,
submodules and configured filters can produce unknown context. Renames count as
one unfinished path. User paths are not listed; displayed branch/program text is
escaped to prevent terminal controls.

Only existing `enough`/`stuck` execution records in the current worktree are read
(cwd outside Git). “exit 0” refers to that command, **not** inferred test success or
all commands passing. A launch failure is distinguished from execution failure.
Incomplete/unfinished or future results remain unknown. Ordering uses recorded
start seconds; when multiple latest commands started in the same second, their
order is unknown rather than guessed. Records can be old: the start Unix timestamp
is printed, with no claim that it ran today. No records means unknown.

State uses the shared authenticated private store in
`${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`. Read-only access does not
create directories, identity keys, lock files or records, does not repair corrupt
state and does not prune old runs. Existing shared locks coordinate with writers;
contention, unsafe permissions, symlinks, missing identity or bad records make the
execution source unknown. Scans are limited to 10,000 records / 32 MiB, with a
5-second budget checked between records. No cross-workspace scans, history or
terminal activity inference.

Local time uses optional `cli-common/local-time` with `jiff`'s system timezone /
zoneinfo support, respecting `TZ`. No bundled timezone data, network or UTC
fallback: unavailable timezones produce `Local time: unknown`.

## CLI and validation

No business options or stdin reads. `--help`, `--version`, `--verison` work as
usual. English/Chinese use the first nonempty `LC_ALL` → `LC_MESSAGES` → `LANG`.
Success (including unknown sources) returns 0, output errors 1, usage errors 2.
Default terminal signal handling allows Ctrl+C. No hidden test hook.

Targets Linux/macOS/WSL Unix. Tested on Linux only in this delivery; macOS/WSL and
full filesystem-fault injection remain pending.

```sh
cargo test -p goodnight
cargo build --locked --workspace --release
python3 tests/report_records_pty.py target/release
```
