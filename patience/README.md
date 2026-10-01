# patience

[简体中文](README-zh.md)

A fake progress bar wrapper for a real command: it runs the command, performs a whole show around it, and hands the exit code back untouched.

```sh
patience make -j4
patience sleep 5
patience patience sleep 3   # one more layer, one more bar
```

The top-level [`README.md`](../README.md) covers usage; this file is for the curious: the decisions and implementation behind every little touch.

## A subcommand, not options

patience has zero user-facing flags (beyond `--help` / `--version`). The thing you wait on is naturally a whole command line, so it is swallowed verbatim via `trailing_var_arg`: in `patience make -j4`, `-j4` belongs to make, not to patience. Zero flags means zero decision fatigue.

When the command itself starts with `--` (e.g. `patience -- git status`), one `--` separates them.

## The nesting special case

`patience patience sleep 3` shows two bars at once. The implementation is a **leading-token special case, not recursive processes**:

- Each leading `patience` token is stripped and adds one bar; the real command is whatever follows them (`./patience` or `Patience` do not count).
- No nested process is spawned: the inner one's stderr would be captured into the outer pipe, never a TTY, so its bar would never render — "multiple bars at once" would be impossible. All bars are drawn by the outermost process, sharing the one real child.
- Every bar performs independently: its own curve, stalls and comfort messages. The random seed is mixed with the bar index (splitmix64), so a fixed `PATIENCE_SEED` reproduces every bar frame by frame.
- Rainbow phases are spread by bar index (`index × 30 / bars`), so stacked bars never look like photocopies.
- Repaint climbs back with `\x1b[{N-1}A`; with a single bar it degrades to the exact byte-identical single-line refresh of old.
- No depth cap: it is your command line — nest as deep a waterfall as you like.

## The random show

Each run picks one of six speed curves at random: linear, ease-in, ease-out, sigmoid, stepped (like a badly written installer), exponential (looks busy, never quite arrives).

The script alternates climbs with 1..=4 short stalls (0.5–2s), stall points strictly increasing and at least 3% apart, ending in an **eternal hold** at 97–99.9%: as long as the child lives, the bar never moves again.

Only on success comes the victory lap: about 3.3% per frame, 100% in half a second; on failure the bar stays parked, never filled — the result never lies. A minimum show time of 0.5–1.5s runs even when the child exits instantly.

## Comfort while stalled

While stalled, a localized comfort message plays: a shuffled deck, drawn in order, never the same card twice in a row, reshuffled when empty.

## The rainbow marquee

The palette is sampled from the seamless looping gradient in `colors.png`: 13 anchors, the bar width spans exactly one full rainbow cycle.

Band boundaries slant 45° by default: each cell is a `▀` half-block carrying two colors, sampled as `color(x, y) = palette(x + k·y + phase)` — in a 2:1 cell grid `k = cot(45°) = 1` gives exactly a 45° edge. The phase advances one cell per tick (a full rainbow slides past in ~1.5s), so even a still screenshot shows the slant.

Colors degrade with the terminal: truecolor → 256 → 16 → plain, honoring `NO_COLOR` and `TERM=dumb`. On Windows conhost a single `SetConsoleMode` call enables VT escapes when needed.

## The behavior contract

- The exit code passes through untouched (128+N on signals); a missing command exits 127; no command at all exits 2.
- Child output is captured per stream (1 MiB cap), replayed after the show, with a note when truncated.
- Not a TTY: no animation at all, just the replay and the result line — pipes and CI stay clean.

## Test hooks

Everything is environment variables, never in `--help`.

| Variable | Effect |
| --- | --- |
| `PATIENCE_SEED=<u64>` | fixes all randomness, including every bar's derived seed |
| `PATIENCE_FAST=1` | scales every timing to 1% |
| `PATIENCE_COLOR=truecolor\|256\|16\|none` | force a color level, overriding detection |
| `PATIENCE_ANGLE=<0..90>` | band boundary angle, 45° by default |