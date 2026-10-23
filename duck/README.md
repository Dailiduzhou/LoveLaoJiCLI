# duck

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

## Known long-input limitation

Interactive input still uses the system canonical line buffer, whose **byte**
capacity can be smaller than the application's character limit. Long pastes can
lose bytes/newlines or wait indefinitely; macOS reproduces this with 600 Chinese
characters in duck. This is **not fixed** by application length validation.
For long answers use explicit `--expected / --actual / --last-change / --experiment` values (never secrets); a pipe is not
an interactive answer. Ctrl+C cancels a stuck prompt. A safe shared input redesign
needs separate lifecycle/dependency approval; afk's cbreak code is not reused here.

The opt-in known-failing regression is
`cargo test -p duck long_interactive_input_contract -- --ignored --nocapture`.
It is ignored by the normal suite, not counted as passed. It covers canonical
buffer and character limits with generated ASCII/CJK/mixed pastes and chunked input.
See [ROADMAP.md](../ROADMAP.md).

Tests: `cargo test -p duck` (Python 3 required for terminal tests).
