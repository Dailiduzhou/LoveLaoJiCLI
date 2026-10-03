# proof

```sh
proof
```

Today's available local evidence, not a score or an estimate of your effort.
No arguments beyond help/version. Never reads stdin, shell history, desktop
activity or other repositories. Nothing is written, executed on your behalf or
sent over the network (only bounded local Git probes run).

## Git counting contract

- Scope: this checkout's pinned current HEAD and its reachable local ancestors,
  not all branches, reflogs or other repositories. Dirty working files do not count.
- Author: both the effective Git `user.name` **and** `user.email` must exactly match
  the raw author fields; no regex, case folding or mailmap. Missing/empty config
  makes Git evidence unknown, not “zero commits by you.”
- Date: **committer timestamp**, not author timestamp. Filter within the local
  calendar day at invocation and no later than the captured current second.
  Traversal does not use `--since` pruning: an old child can have a newer parent.
- Commit total includes authored merges and empty commits. Line totals sum only
  non-merge `--numstat` patches, including root commits. This is cumulative churn,
  not net changes since midnight or effort.
- Paths are deduplicated as raw bytes across those patches. Rename detection is
  disabled: old/new paths are separate. Binary paths count as paths without line
  counts. Tab/newline/non-UTF-8 filenames are parsed with NUL delimiters and are
  not printed. External diff, textconv, signature display, hooks and lazy network
  fetching are disabled for the relevant probes.
- Shallow history, missing objects, missing Git/HEAD, changed HEAD while collecting,
  invalid output or exceeded budgets makes the whole Git source unknown, never a
  partial number. Unborn/bare repositories currently have unknown Git evidence.

Git probes cap each stream at 32 MiB and each process at 5 seconds. History is
capped at 20,000 commits, today's matching authored commits at 1,000, unique changed
paths at 20,000 and cumulative numstat output at 32 MiB. A 10-second collection
budget is checked between probes; a running probe retains its own 5-second limit.
Large histories may intentionally report unknown even with few commits today.

## Local dates

Optional `cli-common/local-time` uses `jiff` with only std/system-timezone/zoneinfo
features. It respects the system timezone or `TZ`, without a bundled database or
network lookup. Start/end are calendar-day boundaries; DST days need not last
24 hours. Missing/invalid timezone data is unknown, not a silent UTC fallback.
Future-dated commits and execution records are excluded. No public or hidden clock
override; DST/boundary tests inject timestamps into the internal helper.

## Explicit execution records

The source is existing enough/stuck run records in this exact worktree, or this
cwd outside Git. No record means no **recorded** execution, not no work. The date
is the completion time (so a run started yesterday and finished today can count).

Only complete real `exited`/`signal` outcomes count as completed executions; exit
0 is a successful execution, **not necessarily a test**. Spawn/wait failures,
uncompleted runs, incomplete forwarded streams, future results, enough skips and
stuck gates never count. Repeat baselines are not execution records. Multiple
real executions count individually even if their repeat optimization was disabled.

Reading does not create directories/keys/lock files, prune history, repair state
or modify selection in other tools. It uses existing shared identity/workspace
locks; unsafe/corrupt/unknown records or busy locks make the execution source
unknown. Scans cap at 10,000 entries / 32 MiB with a 5-second budget checked between
records. Missing state is empty; present state with missing identity is unavailable.
The wrappers' existing retention/fail-open rules mean this is not a complete audit
log of all shell commands.

When neither source provides evidence, prints `no recorded work.` and keeps any
unknown-source labels plus a reminder that missing evidence says nothing about
your work. No ranking, streak, cross-repository discovery or terminal-activity guess.

## CLI and tests

`--help`, `--version`, `--verison`; locale priority is the first nonempty `LC_ALL`
→ `LC_MESSAGES` → `LANG` (zh-CN/zh_CN/zh: Chinese, otherwise English). Success or
unknown evidence is 0; output failure 1, usage error 2. Default signal handling;
no stdin/TTY requirement. Linux/macOS/WSL targets; only Linux validated this round.

```sh
cargo test -p proof
cargo test -p cli-common --features local-time
cargo build --locked --workspace --release
python3 tests/report_records_pty.py target/release
```

Tests cover DST, author/time/reachability, merge/root/binary/rename statistics,
record validation, locks, read-only behavior and actual wrapper executions/skips/gates.
macOS/WSL, every historical timezone anomaly and full resource-fault injection are
not claimed as verified.
