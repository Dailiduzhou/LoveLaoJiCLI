# encourage

[中文说明](README-zh.md)

Build one gentle sentence from short word fragments. Explicit text is **seed material only**: encourage does not interpret feelings, answer questions or select a theme from the meaning of your input. It runs offline, never reads Git or stdin as seed text, and saves no input, seed, history or trace.

## Usage

Build from the repository root (Rust 1.89+, Cargo; Linux/macOS/WSL Unix):

```sh
cargo build --locked --release -p encourage
./target/release/encourage
./target/release/encourage hello
./target/release/encourage --duration 30s hello
./target/release/encourage --duration 600s hello | cat
./target/release/encourage "one small step"
./target/release/encourage one small step
./target/release/encourage ""
./target/release/encourage -- --help
./target/release/encourage --help
./target/release/encourage --version
LC_ALL=zh_CN.UTF-8 ./target/release/encourage hello
```

`encourage [--duration SECONDS] [TEXT ...]` accepts optional UTF-8 seed text, `--duration`, `-h/--help`, `-V/--version` and the hidden compatibility alias `--verison`. Playback defaults to **15s**; `--duration` accepts integer seconds **10s through 600s**, inclusive, with the lowercase `s` required. Fractions and minute/hour units are rejected. This is a display option, never seed material; `encourage -- --duration 30s` instead treats those two arguments as literal seed text. There are no seed, theme, language, speed or color options and no production test environment hooks. Invalid UTF-8, duration values and unknown options return a localized usage error without echoing input.

Language follows the first nonempty `LC_ALL → LC_MESSAGES → LANG`; Simplified Chinese and English use the shared locale rules. An explicit empty argument is deterministic text, not a request for randomness. Spaces, case, Unicode encoding and argument boundaries are preserved: `"one small step"` and `one small step` are different seed inputs. `42` is text, not the numeric seed 42. Different inputs may still produce the same sentence.

## Plain output and stage replay

- Without a suitable terminal, print the generated sentence and one newline immediately: no ANSI, delay, seed, JSON or stage statistics. Plain generation does not collect replay previews.
- TUI startup requires stdin/stdout/stderr on the same foreground terminal, a nonempty non-`dumb` `TERM`, a controlling `/dev/tty` and at least **80 columns × 24 rows**. Redirection, pipes, background startup and initially small terminals use plain output without changing termios.
- A suitable terminal replays the actual, already-computed dataflow stages across the requested total budget (15s by default), distributed evenly without per-stage rounding drift. Manual pauses, small-window pauses, suspension, background time and the final confirmation wait do not consume that budget. This is **not** Rust instruction execution order: the iterator is lazy and operations execute interleaved per item.
- **Space** pauses/resumes. **Enter** may skip to the final stage before ten seconds; a subsequent Enter confirms the result. Automatic completion waits indefinitely for confirmation rather than exiting at the deadline. **q always cancels**, including on the completed screen, without printing a final sentence.
- The upper box contains **only text**, preserving order and duplicates: no `Word`, `Atom`, `Ok(...)`, type names, IDs, debug labels or counters. Before the first textual output, it shows the actual locale corpus's fixed words, candidate variants and punctuation in declaration order, labelled **Candidate pool** in the status bar. This seed-independent opening context is not a current-stage output or assembled sentence. Once textual outputs begin, it switches to **Current output** and displays only that stage's typed textual payloads; later textless outputs remain blank rather than falling back to the pool. Text wraps, with clipping/omission reported outside the box. The pool uses the same 256-piece / 4,096-byte bounds and never fills the trace or assembly preview.
- A separate **Current assembly preview** joins the actual selected material available at that stage. It can be empty, partial, shuffled, not yet capitalized or awaiting editorial deletion. It changes when real words are inserted/deleted; it is not a typewriter reveal of a precomputed final sentence. Alternatives do not pretend to be one sentence. Only completion changes the title to **Final sentence**.
- Confirmation restores the original screen, then writes exactly the same plain sentence once. Resize below the minimum pauses an active replay instead of switching to plain output. `--duration` never delays plain output, even at `600s`.
- Nonempty `NO_COLOR` disables highlighting without changing generation, playback or eligibility.

The executing and displayed code are the same compile-time embedded `src/pipeline.rs`; no runtime source file, compiler, parser, LSP or external command is needed. Its **34 operations are 28 content operations plus 6 explicitly labelled reversible transitions**, not 34 semantic content transformations. The three transition pairs attach/drop ordinals, wrap/flatten singletons, and shuffle/restore fragments. Actual editorial word insertion and deletion follow them. The opening pool reads the actual locale corpus without seed-dependent selection; after textual outputs begin, the raw box and preview use typed observations of actual outputs, never parsed Debug strings or words borrowed from later stages. Legacy labelled diagnostic observations remain bounded in memory but are not rendered as raw material; missing global item IDs and unavailable counts remain unknown rather than fabricated.

## Generation protocol: encourage-gen-v2

This identifier is independent of the Cargo package version. V2 adds structured material snapshots and upgrades the observation contract; it **retains v1 argv seeds, random consumption, corpus/candidate ordering and final sentences**, verified against all 68 ordered v1 combinations and fixed vectors. Changing those mappings or semantic observations requires a new generation version; drawing-only fixes do not. Playback duration is not part of the seed or generation protocol.

- Explicit argv is encoded as `b"encourage/argv/v1\0"`, `u64_le(argc)`, then each argument's `u64_le(UTF-8 byte length)` and unchanged bytes. No joining, trimming, normalization or locale mixing.
- A public BLAKE3 keyed hash uses ASCII `encourage/argv-seed/v1` right-padded with zeros to 32 bytes; its first eight digest bytes interpreted little-endian become the seed. This fingerprint is **not input anonymization**; do not pass secrets (shell history and process lists may expose arguments).
- Only absent text obtains eight bytes through the fallible `rand::rngs::OsRng` interface. Failure returns 1; no time/PID/fixed-seed fallback. An internally retained random seed is a diagnostic identifier, not a positional replay interface.
- The explicit wrapping SplitMix64 implementation chooses one of four themes, one of its two structures, then candidates in grammar-slot order. Rejection sampling avoids modulo bias; rejected words retry the same slot. This is hierarchical uniform selection, not uniform selection over all sentences. The generator is not cryptographic.
- Chinese and English share theme/structure intent for the same seed, not literal translations or necessarily equal subsequent draws. Each language has eight structures and 34 distinct combinations, assembled from words and punctuation rather than stored whole sentences.
- Limits are 4,096 selection words, 64 final word/punctuation atoms and 512 UTF-8 bytes. Budget/invariant errors fail instead of truncating, retrying with a different seed or selecting a fallback sentence.
- The reversible Fisher–Yates transition uses an independent SplitMix64 seed of 2 for each fragment, with its own 4,096-word limit; it never advances the content-selection RNG. Original positions restore duplicates and end markers exactly.
- Per-stage material snapshots retain a prefix of at most 256 text pieces / 4,096 UTF-8 bytes and count omitted pieces explicitly. End boundaries have no invented text. The fold snapshot is its actual scalar output, not concatenated accumulator history. Plain mode collects no material snapshots.

## Terminal lifecycle and exits

Ratatui renders via an independently opened `/dev/tty`. Cbreak disables canonical input/echo but retains `ISIG`; there is no Crossterm raw mode, event loop or cursor-position query. Signal flags and a scoped UI-thread job-control mask coordinate cleanup. Session guards restore termios, cursor and alternate screen, without changing inherited nonblocking I/O modes.

Ctrl+Z restores before stopping. On `fg`, the tool rechecks ownership and takes a fresh snapshot; paused/background time does not advance playback. Losing foreground ownership discards the old snapshot and suppresses stale termios writes, drawing and cleanup escapes. Background jobs do not read keys. SIGINT, SIGTERM, SIGHUP and SIGQUIT restore before default signal termination (`128 + signal` in a shell).

Success and `q` return 0; invalid arguments return 2; entropy, generation, terminal or output failures return 1. Initialization/drawing failures after attempting a TUI are errors, not a silent successful plain fallback. SIGKILL, external SIGSTOP while no recovery action can run, abort and terminal disconnection cannot guarantee cleanup.

## Validation

```sh
cargo fmt --all -- --check
cargo test --locked -p encourage --all-targets
cargo clippy --locked -p encourage --all-targets -- -D warnings
cargo build --locked -p encourage
python3 encourage/tests/replay_pty.py target/debug/encourage
python3 encourage/tests/ownership_pty.py target/debug/encourage
```

Tests cover fixed argv/RNG vectors, all 68 v1-compatible bilingual combinations, 10,000 bilingual seeds against an independent oracle, budgets/rejection, typed output/partial-preview fidelity, traced/plain equivalence, source metadata, raw-only rendering/clipping, seed-independent opening candidates, the switch to exact current outputs without later fallback or trace mutation, duration validation and rational scheduling, CLI/locale/invalid UTF-8/closed output, and real CLI PTYs. PTYs check labelled opening candidates in both languages and the switch to actual outputs, time default 15s and configured 10s/17s automatic playback, check completion waits for Enter, q cancels after completion, 600s can be skipped and never delays redirected output, plus pause, termination signals, resize, Ctrl+Z/resume, foreground-owner replacement and conservative plain fallback. They inspect exact termios restoration and screen/cursor cleanup. Entropy failures are injected through a pure test seam, not production environment variables.

See [the repository roadmap](../ROADMAP.md) for actual platform results and remaining acceptance. WSL and real terminal-emulator visual acceptance must not be inferred from PTY checks. Full startup/drawing fault injection and panic-unwind PTY coverage remain follow-up work.

The [isolated experiments](experiments/README.md) remain historical references, not workspace members or installer binaries. `examples/expanded_replay.rs` is a seed-0 developer driver; CLI validation targets the actual `encourage` binary.
