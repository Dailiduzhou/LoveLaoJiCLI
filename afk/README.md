# afk

A break without penalties, with a small Ratatui grass scene when a suitable
foreground terminal is available.

```sh
afk 5m
afk 30s
afk 1h
```

Duration remains ASCII decimal digits followed by lowercase `s`, `m` or `h`,
positive and at most 24h. Leading zeros are accepted; signs, fractions, spaces,
missing units, zero and overflow return 2. No new business flags or subcommands.
Normal completion prints `Break complete.` after leaving the scene; I/O failures
return 1. Ctrl+C cancels without a completion message (normally shell status 130).

## Scene and fallback

- A colored ASCII garden grows through four stages based only on elapsed
  `Instant` time. A small sun animates, with remaining time and control hints.
  Room permitting, a figlet-style `remaining` banner at its native size tops a
  large matching clock whose equal-width cells keep colons and neighbors fixed
  while digits change, with narrow glyphs centered inside their cells; glyphs
  scale squarely so slanted strokes stay 45°.
  A localized gentle or encouraging line rotates every
  eight seconds.
  The banner is cyan, the clock bright cyan, grass green, sun/ground yellow,
  and flowers pink with bright yellow centers. Only the basic 16-color palette
  is used, with the terminal's default background; exact shades follow its theme.
  Rendering is capped at 10 frames/second and a 240×100 logical viewport.
- Ordinary input shows `the grass noticed.` for two seconds. It never resets,
  extends or penalizes the break. `q`, Escape and arrow keys are not quit commands.
  Only this terminal is read; no global idle/keyboard monitoring. At most 256
  input bytes are read per frame, so a key/paste flood cannot starve the timer.
- Ctrl+C cancels. Ctrl+Z restores the terminal **before** stopping. Time continues
  during process suspension. `fg` can resume the scene; `bg` does not capture keys
  or draw. If the deadline passed while stopped, continuing finishes immediately.
- The scene requires stdin/stdout/stderr to refer to the same foreground terminal,
  a nonempty `TERM` other than `dumb`, and at least 24×10 cells at startup. Otherwise
  there is a quiet wait, no key capture, no mode changes and one completion line.
  Redirecting stdout intentionally disables the TUI; output stays script-friendly.
  Shrinking an active scene shows compact text; oversized terminal dimensions are
  clipped to the bounded viewport. A nonempty `NO_COLOR` (e.g. `NO_COLOR=1 afk 5m`)
  keeps the same artwork monochrome; unset or empty enables colors. It does not
  disable screen/cursor escape sequences in TUI mode. Colors are reset on cleanup.

No state, network, external helper program, desktop notification, lockscreen,
background daemon, streak, scoring, `--until` or Pomodoro loop.

## Why Ratatui, and its cost

Crossterm alone would be sufficient for this one animation and would be lighter.
Ratatui is useful for buffer-diff rendering, clipping/layout and `TestBackend`
scene tests, particularly if the scene evolves. It is **not** a terminal recovery
framework. This implementation accepts the dependency cost but keeps all scene
and lifecycle code in `afk/`, with no new shared TUI framework.

- Ratatui 0.30.2: defaults disabled, only `crossterm_0_29` selected. Its declared
  Rust minimum is 1.88; the workspace still declares 1.89. The dependency metadata
  was checked, but this delivery did not execute tests under Rust 1.89 itself.
- Crossterm is used through Ratatui's re-export as a rendering backend. Its event
  dependencies are still transitively compiled; our code does not call its event
  reader or global raw-mode API. No async runtime, event thread or mouse capture.
- Existing rustix owns the termios snapshot/restore. `signal-hook` supplies atomic
  signal flags; narrowly scoped libc calls temporarily mask Unix job-control I/O
  signals on the UI thread, avoiding suspension halfway through restoration.
- The initial dependency resolution added 67 lockfile entries (including optional
  and target-specific packages, not all compiled on Linux). The local release
  binary grew from about 1.06 MiB to about 1.45 MiB; not a universal size guarantee.

## Terminal lifecycle

`src/tui/terminal.rs` owns a separate `/dev/tty` descriptor, the exact original
termios and an RAII guard; `src/tui.rs` controls their lifetime. It uses noncanonical/no-echo input with `ISIG` retained, **not**
Crossterm raw mode. Flow-control keys are treated as input while the scene is live.
Separately opened nonblocking input/output descriptors do not change the shell's
inherited file flags. Keyboard bytes/paste queued during the scene are discarded
before returning/suspending while still in the foreground, not replayed to the shell.

While the UI owns the terminal, signal handlers only update atomics. The event loop handles SIGINT/SIGTERM/SIGHUP/
SIGQUIT by restoring termios, showing the cursor and leaving the alternate screen,
then re-raising the signal with default behavior. SIGTSTP restores first, then stops;
continuation rechecks foreground ownership and snapshots settings again on reentry.
Errors and ordinary panic unwinding also run the guard. Cleanup attempts termios,
color, cursor and screen operations even when an earlier operation fails. Writes
are nonblocking so output backpressure fails instead of freezing with cbreak live.

One evaluated upstream pitfall: Ratatui 0.30 `Terminal::clear()` asks for cursor
position, and Crossterm temporarily enters raw mode while waiting for that reply.
We avoid that API, using backend clear and a fixed viewport. On resize the bounded
terminal buffer is rebuilt instead of using backend size fallbacks (which can run
`tput`). This avoids cursor-response input consumption and external helper calls.

**Limits:** cleanup cannot run after SIGKILL, an externally delivered SIGSTOP,
process abort, hardware failure or a non-unwinding crash. Only the listed signals
have managed cleanup. A disconnected/unwritable terminal may reject restoration
or screen/cursor escape sequences; cleanup is best-effort in that case, not a
promise to repair a terminal that no longer exists. Unexpected terminal ownership
changes are detected between frames, not an atomic lease against other programs.

## Language, hooks and tests

Supports `--help`, `--version`, `--verison`; English/Chinese follows the first
nonempty `LC_ALL` → `LC_MESSAGES` → `LANG`. No secrets or answers are requested.

Hidden environment-only test hooks (never in help):

- `AFK_FAST=1`: wait for 1% of the validated duration; other values ignored.
- `AFK_TEST_FAILURE=enter|draw|panic`: fail after terminal entry, inject a backend
  write error, or panic after a rendered frame. Only applies to TUI mode; unknown
  values are ignored. For isolated tests, not normal use.

`cargo test -p afk` includes TestBackend snapshots and Python-backed real PTYs:
color/monochrome rendering and cleanup, completion, key floods, no deadline
extension or queued-input leakage, resize,
Chinese output, Ctrl+C/SIGINT/SIGTERM/SIGHUP/SIGQUIT, setup/write/panic failures,
Ctrl+Z/fg/bg, suspension past the deadline, dumb/small/redirected fallback and exact
termios restoration. Tests also reject accidental cursor-position queries.

Linux validated. macOS/WSL, native terminal-emulator visual checks and every I/O
failure timing remain pending; no native Windows promise.
