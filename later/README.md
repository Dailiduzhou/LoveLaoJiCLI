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
checks out a branch. The next action is always first, followed by a rough elapsed
age (just now / minutes / hours / elapsed days) and the saved snapshot. Future
times or clock rollback say the saved time cannot be compared. Only the displayed
`refs/heads/` prefix is simplified; detached HEAD and other references stay distinct.
Full HEAD, file list/truncation and the latest failure remain visible. An empty
diff says `No tracked-file differences`, not that the workspace is clean;
untracked files still count, and unavailable diff data says `unknown`.
It warns if the current branch/HEAD differs; unavailable current Git cannot
establish a comparison. The card schema and stored timestamps are unchanged. No card is a
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

## Long interactive input

Prompts read the controlling terminal in non-canonical (cbreak) mode, one byte
per read, instead of the kernel canonical line buffer. That buffer is
byte-limited (1024 on macOS) and used to lose input or hang on long pastes. Now
answers up to 2,000 characters succeed exactly, and longer or invalid ones
are rejected (exit 125) instead of hanging. Only bytes up to the submitted
newline are consumed.

- Editing: Backspace/erase, Ctrl+U (kill line) and Ctrl+W (erase word) use the
  terminal's configured keys; arrow/function key sequences are ignored.
  Ctrl+D on an empty line is EOF.
- Terminal signals stay live (ISIG): Ctrl+C/Ctrl+\ and SIGTERM/SIGHUP restore
  termios before terminating; Ctrl+Z restores before stopping, and `fg` shows the
  question and partial answer again. In the background it neither reads nor draws.
- A rejected or cancelled paste is flushed, so the remainder never reaches the
  shell or a wrapped child.
- SIGKILL, external SIGSTOP, abort and terminal disconnect cannot guarantee
  restoration.

Explicit `--next` values skip the prompt entirely (never enter secrets; a pipe is
not an interactive answer). `cargo test -p later long_interactive_input_contract`
covers canonical-buffer and character boundaries with ASCII/CJK/mixed pastes and
chunked input. See [ROADMAP.md](../ROADMAP.md).

Integration tests require Git; all fixtures isolate HOME/state. Terminal-independent
operation works without DISPLAY, Wayland, clipboard or desktop services.
