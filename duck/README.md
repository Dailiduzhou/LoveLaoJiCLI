# duck

[简体中文](README-zh.md)

Explain a problem; no advice, AI, search or persistence.

```sh
duck
duck --expected "success" --actual "timeout" --last-change "retry logic" --experiment "disable retries"
duck --expected "success"    # asks only the other three questions
```

Questions and output use this fixed order:

```text
Expected: success
Actual: timeout
Last change: retry logic
Next experiment: disable retries
quack.
```

Answers are trimmed, nonempty single lines, at most 2,000 Unicode characters,
without control characters. Explicit invalid arguments return 2. All four flags
skip interaction. Otherwise, missing answers are asked in order using the shared
foreground-terminal check (stdin/stderr must be terminals and stdin must belong
to the foreground group). Prompts go to `/dev/tty`, never stdout. EOF, blank or
invalid interactive input, or unavailable interaction returns 125 with no partial
summary. A pipe is never interpreted as answers. Ctrl+C uses normal terminal
signal handling; no terminal settings are changed.

The complete summary goes to stdout; diagnostics go to stderr. Redirect stdout
yourself to save it. The tool does not open local state or invoke any service.
Arguments may still appear in shell history/process lists; do not enter secrets.
I/O errors return 1, success returns 0. There are no hidden test hooks.

`--help`, `--version` and the compatibility alias `--verison` are supported.
Language follows the first nonempty `LC_ALL` → `LC_MESSAGES` → `LANG`;
zh-CN/zh_CN/zh use Chinese, otherwise English. Linux/macOS/WSL Unix are the target
platforms; this delivery was tested on Linux, including foreground PTYs.

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

Explicit `--expected / --actual / --last-change / --experiment` values skip the prompt entirely (never enter secrets; a pipe is
not an interactive answer). `cargo test -p duck long_interactive_input_contract`
covers canonical-buffer and character boundaries with ASCII/CJK/mixed pastes and
chunked input. See [ROADMAP.md](../ROADMAP.md).

Tests: `cargo test -p duck` (Python 3 required for terminal tests).
