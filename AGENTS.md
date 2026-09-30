build multiple CLI tools in Rust, using clap crate.
keep minimal feature. each supports basic `--verison`, `--help`, and the CLI name itself.
each tool has their own folders.
support zh-CN and en-US. change display language according to locale (LC_ALL -> LC_MESSAGES -> LANG).
share locale detection and clap scaffolding in `cli-common/`.

dependencies: clap and rand are allowed. keep everything else std-only. workspace version stays in sync (0.1.0).

tools:
1. love - print one localized blessing. (done)
2. happiness - same pattern. (done)
3. joy - same pattern. (done)
4. patience - fake progress bar wrapper for a real subcommand: `patience <command> [args...]`.
   - runs the child, renders a single-line fake progress bar (unicode blocks, fixed width, `\r` refresh; plain result line when not a TTY).
   - each run picks one random speed curve: linear / ease-in / ease-out / sigmoid / stepped / exponential.
   - multiple random climbs and stalls, then holds at 97-99.9% while the child still runs; rapid-fills to 100% only on success. minimum show time 0.5-1.5s even if the child exits fast.
   - localized comfort messages while stalled (shuffled deck, no immediate repeat); success/failure lines with elapsed time and exit code; failure never fills the bar.
   - exit code passes through from the child (128+N on signals, 130 on forwarded Ctrl+C). bare run prints a localized quip + usage, exit 2. child output is captured (1MB cap) and replayed after the bar.
   - zero extra user-facing flags. hidden test hooks: `PATIENCE_SEED=<u64>` fixes randomness, `PATIENCE_FAST=1` scales all timings by 1/100.

keep install.sh, README.md and workspace members in sync when adding a tool.