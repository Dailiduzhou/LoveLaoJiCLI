# enough

A recent-success reminder, not a build cache. [中文](README-zh.md)

```sh
enough [--again] [--] <command> [args...]
enough cargo test
enough --again cargo test
```

Wrapper options must precede the child; later arguments pass through unchanged.
No implicit shell: use `sh -c` explicitly when needed. No command exits 2.
Supports localized help, version and `--verison`.

## State machine

A real success is eligible only when pre/post code snapshots agree and input is
comparable. The next invocation must match canonical cwd, byte-preserving argv
boundaries and the same snapshot. Within **five minutes of real completion**,
it may skip execution, returning 0 with empty stdout and a stderr notice:

1. `It already passed.`
2. `Nothing changed.`
3. `enough.`
4. `no.` (also subsequent skips)

Notices have Chinese equivalents. Skips increment only the notice counter: they
never refresh the completion time or create a successful CommandRun. Clock
rollback/future records invalidate eligibility. `--again` always runs; failure,
spawn failure, interruption or an unstable snapshot invalidates old success.

Git snapshots hash HEAD/index plus tracked and nonignored untracked bytes,
paths/types/executable modes/symlink text and cwd, not mtimes or diff statistics.
Two scans detect races; pre/post execution snapshots must match. The conservative
limits are 20,000 entries, 256 MiB total reads and five seconds. Submodules,
conflicts, filters, budgets, state corruption, lock failures, missing Git/non-Git
workspaces and incomparable input **run the real command**. Pipeline/redirected
stdin or lack of a confirmed foreground terminal never triggers a skip.
Overlapping executions cannot establish a reliable new baseline. An execution
lease is held separately from the short-lived state write lock.

Child stdin/stdout/stderr are inherited; tool notices use stderr. Child exit
codes pass through, signal exits map to 128+N, missing programs use 127 and
nonexecutable programs 126. Record failures never replace the child's result.
Ctrl+C uses the shared foreground group, without detached children or signal
handlers. This does not control intentionally daemonized descendants.

## Not proof and not appropriate for everything

Ignored files, environment, services, databases, external files, time, randomness
and stdin contents are not proven unchanged. Avoid this wrapper for deployment,
commands with side effects, external-state-sensitive tests, and pipelines that
need output; run directly or use `--again`. Command names do not imply safety.

State is local under `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`:
0700 directories, 0600 versioned authenticated JSON, atomic writes and OS locks.
Only a keyed argv digest and program basename are retained, never the raw argv,
shell body, environment or command output. Real run records expire lazily after
30 days; repeated success eligibility is only five minutes. State corruption is
not silently repaired; key loss invalidates old records. Uninstall preserves state.
Paths and program names can still be sensitive; digests are not encryption.
See the root README for private-directory and filesystem limitations.

Tests: `cargo test -p enough` (Git and Python 3; real foreground PTYs test skipping,
concurrency, stdin preservation and Ctrl+C). No safety-bypass test flags exist.
