# afk

A quiet break without penalties.

```sh
afk 5m
afk 15m
afk 30s
afk 1h
```

Duration must be ASCII decimal digits followed by lowercase `s`, `m` or `h`,
positive and no more than 24 hours (86,400 seconds). Leading zeros are accepted;
signs, fractions, spaces, missing units, zero and overflow are rejected with exit
2. A duration is required; no `--until`, loops or extra business flags.

Timing uses `Instant`, not wall-clock time. The tool waits quietly, then prints
`Break complete.` to stdout. I/O errors return 1; normal completion returns 0.

## Deliberate terminal fallback

**This version uses the roadmap's plain-text fallback on every terminal**, not
just non-TTY or `TERM=dumb`. ASCII growth and `the grass noticed.` key notices
are deferred. No terminal dependency has been added: safely managing raw mode
across errors, suspend/resume and signals would require a separate lifecycle
implementation and validation.

There is no raw mode, keyboard reader, alternate screen, cursor manipulation or
signal handler. Terminal attributes are never changed, so nothing needs restoring
on normal completion, output failure, Ctrl+C, SIGTERM or SIGHUP. Ctrl+C uses the
normal foreground terminal signal and cancels without a completion message
(typically exit 130 in the invoking shell). No keys are consumed, no penalties are
issued and input cannot reset or extend the timer. Normal terminal line discipline
still applies: input may echo, queue for the shell, suspend the process or pause
terminal output. This is not an input filter or enforced rest.

No state is saved, no network/desktop services are called. No lockscreen, daemon,
notifications, streaks or productivity scoring. Linux/macOS/WSL Unix are targets;
this delivery was tested on Linux, with PTY checks for unchanged terminal settings
on completion, argument/output errors and cancellation/signals. macOS/WSL remain
pending; no raw-mode restoration claim is made because raw mode is not used.

Supports `--help`, `--version`, `--verison`. Messages use the first nonempty
`LC_ALL` → `LC_MESSAGES` → `LANG` (zh-CN/zh_CN/zh: Chinese; otherwise English).

Hidden test hook: exactly `AFK_FAST=1` scales the wait to 1% after normal duration
validation; other values are ignored. Not shown in help.

Tests: `cargo test -p afk` (Python 3 required for PTY tests).
