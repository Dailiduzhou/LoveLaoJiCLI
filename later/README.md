# later

Persist one next action before leaving. [中文](README-zh.md)

```sh
later
later --next "check why refreshToken is never called"
later resume
```

`later` asks only **What is the very next thing?**, through `/dev/tty` after
confirming foreground stdin/stderr terminals. Without that or `--next`, exit 125;
pipeline data is not an answer. EOF, empty or invalid answers never replace a card.
`--next` trims surrounding whitespace and requires a nonempty single line of at
most 2,000 Unicode characters. Invalid explicit input exits 2.
All commands support help/version and the `--verison` compatibility alias; prompt,
help and summaries follow `LC_ALL → LC_MESSAGES → LANG`.

## One latest card, not a task system

A Git worktree uses its canonical root: calling from a subdirectory retrieves the
same card, but another worktree has a separate card. Without usable Git, canonical
cwd is the workspace and the card explicitly lacks Git context. No account,
network, shell history, editor automation, full diff, or history/list/show/clear.

The card contains time (Unix seconds), workspace, optional repository/branch/HEAD,
porcelain file statuses (at most 200 paths plus total/truncation), a bounded diff
stat and latest commit ID/time, the next action, and the most recent real failed
enough/stuck execution within 30 days (program basename/time/exit status only).
Paths are stored reversibly as Unix bytes and terminal controls are escaped when
shown. No commit messages or command output are stored. Git failures degrade to
a directory-only card. patience is not retrofitted with event logging.

`resume` only displays the saved context; it never executes the next action or
checks out a branch. It warns if the current branch/HEAD differs. No card is a
friendly success (0); damaged/unrecognized records are errors (1), not partial
cards. An explicit valid save can replace an old card atomically. The success
message is printed only after the complete record is synced and replaced.

## State and privacy

Cards use `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`, falling back from
invalid XDG paths to absolute HOME, never cwd or `/tmp`. Managed directories/files
are 0700/0600; symlinked state, unsafe permissions and lock contention fail closed.
Writes use same-directory atomic replacement and a short OS workspace lock.
Uninstallation preserves cards; the latest card does not expire automatically.
See the root README for shared locking, key-loss and filesystem limitations.

**The next action is local plaintext. Do not enter passwords, tokens or other
secrets.** `--next` may also enter shell history/process listings. Paths, filenames,
Git metadata and program basenames can be sensitive; fingerprints do not encrypt
records. No telemetry or background collection.

```sh
cargo test -p later
```

Integration tests require Git; all fixtures isolate HOME/state. Terminal-independent
operation works without DISPLAY, Wayland, clipboard or desktop services.
