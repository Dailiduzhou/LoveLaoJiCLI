build multiple CLI tools in Rust, using clap crate.
keep minimal feature. each supports basic `--verison`, `--help`, and the CLI name itself.
each tool has their own folders.
support zh-CN and en-US. change display language according to locale (LC_ALL -> LC_MESSAGES -> LANG).
share locale detection and clap scaffolding in `cli-common/`.
Tool-specific CLI arguments, business policies, localized text, assets and test scenarios belong inside that tool's folder. `cli-common/` owns only reusable mechanisms and cross-tool data contracts, never tool-name business dispatch or dependencies on tool crates. Root `tests/` may contain reusable harnesses, parameterized shared contracts, cross-tool acceptance and installer tests; tool-specific scenarios stay under `<tool>/tests/`. Legacy shared record validation stays centralized and serialization-compatible.

dependencies: clap/rand plus narrowly scoped serde/serde_json (records), blake3 (stable keyed fingerprints), and rustix (Unix ownership/foreground and stdin-type checks) are allowed for all three batches. jiff is additionally allowed only for goodnight/proof local dates and DST boundaries, through the optional cli-common/local-time feature (std/tz-system/tzdb-zoneinfo; no bundled database). For afk and encourage only, ratatui (default features off, crossterm_0_29) is additionally allowed after terminal-lifecycle evaluation. signal-hook (signal flags) and libc (scoped Unix job-control signal masks) are allowed for afk, encourage, and the shared interactive line input, through the optional cli-common/line-input feature enabled only by duck/later/stuck. rustix may snapshot/restore afk, encourage and line-input termios. Prefer std for everything else. workspace version stays in sync across all crates.
love/happiness/joy keep only basic flags. New tools support only the MVP business arguments in project.md part 2, plus the explicitly requested afk `--color rainbow|parts|white` and encourage `--duration 10s..600s` display options. Test hooks are environment variables, documented in README, never in `--help`.

tools (status):
1. love - one localized blessing. (done)
2. happiness - same pattern. (done)
3. joy - same pattern. (done)
4. patience - fake progress bar wrapper for a real subcommand. (done)
   - exit code passes through from the child (128+N on signals).
   - not a TTY: no animation, plain result line.

5. sprinkle - managed HEAD worktree, conservative EOF comments, saved diff, verified undo. (done)
6. later - one latest per-workspace context card (`--next`, `resume`). (done)
7. enough - five-minute stable-success reminder (`--again`). (done)
8. stuck - three identical failures, then one hypothesis (`--hypothesis`). (done)
   - first-batch tools support Linux/macOS/WSL Unix; Git CLI is local-only.
   - state failures fail open for wrappers, fail closed for later/sprinkle.
   - preserve argv boundaries, private state, and destructive-operation ownership checks.

9. duck - four fixed questions or explicit answers; no persistence. (done)
10. one - global local tasks, stable selection until done; isolated piped selection. (done)
11. afk - monotonic break timer, Ratatui grass scene/key notice, conservative plain fallback. (done)
   - afk owns termios/alternate-screen restoration, foreground checks and Ctrl+Z/fg/bg lifecycle.
   - afk --color defaults to parts; rainbow animates artwork, white uses ANSI white. Nonempty NO_COLOR overrides all modes; plain fallback stays uncolored.
   - no crossterm raw mode/event loop or cursor queries; cbreak keeps terminal signals.
   - Ratatui is scoped to afk and encourage. signal-hook/libc are scoped to afk, encourage and cli-common/line-input (duck/later/stuck); one keeps its existing dependencies.
   - shared prompts (cli-common/line-input) own non-canonical line input: cbreak keeps ISIG, one byte per read so later input stays queued, self-echo/editing, Ctrl+C/Ctrl+Z/fg/bg restoration, and no stale-snapshot writes after foreground loss. Without the feature, prompts fail closed as "no answer".
   - one state failures fail closed; duck/afk never open state.
   - ONE_SEED, AFK_FAST and AFK_TEST_FAILURE are hidden test hooks, not CLI options.

12. goodnight - read-only local time, Git and explicit execution summary. (done)
13. proof - current-HEAD/current-workspace evidence for the local calendar day. (done)
14. poke - private global name list, exact deduplication and uniform random reminder. (done)
   - goodnight/proof never create state or prune records; unavailable sources stay unknown.
   - proof uses exact configured author name AND email, committer timestamps, non-merge numstat.
   - poke fails closed on state errors; POKE_SEED is a hidden test hook.
   - third-batch targets remain Linux/macOS/WSL Unix; runtime is offline and local-only.

15. encourage - deterministic word-fragment encouragement with a Ratatui stage replay. (CLI integrated; encourage-gen-v2 typed observations, v1-compatible text)
   - the current `pipeline.rs` has 34 executed operations (28 content + 6 explicitly marked reversible transitions), promoted from the isolated P0 lab; do not describe the transitions as semantic content work.
   - the raw upper panel contains only text (no Debug parsing, type names, IDs or counters). The leading non-text prefix shows a seed-independent candidate pool read from the actual locale corpus, explicitly labelled in the status bar; once textual outputs begin, show only typed current-output text, leaving later textless outputs blank. Clipping notices stay outside. The display-only pool never fills trace snapshots or the separate assembly preview, which follows actual material and edits, not a typewriter reveal of the final result. V2 upgrades observations, not seed mapping/RNG/corpus/final sentences; retain the ordered v1 fixtures.
   - the TUI owns bounded dynamic playback, fixed Rust highlighting, cbreak/alternate-screen restoration and foreground/suspend lifecycle; execution and display share embedded source and real labelled traces, with unknown IDs/counts left unknown. Real CLI debug/release PTYs pass on macOS and Linux aarch64; WSL/GUI and the complete fault-injection matrix remain pending.
   - `encourage [--duration SECONDS] [TEXT ...]` uses boundary-preserving UTF-8 seed text or fallible OS entropy only when text is absent. Playback defaults to 15s; --duration accepts integer 10s..600s, excludes paused/small/suspended/background time and never delays plain output or enters the seed. Enter skips then confirms; q always cancels, including at completion. No stdin seed, persistence, external commands, other business options or production test hooks. The versioned protocol, plain output, workspace membership, root lockfile, both root READMEs and install.sh are integrated. Keep `encourage/experiments/` isolated; bump versions before publishing a new fifteen-tool release.

behavior specs live in root README.md (en-US) and README-zh.md (zh-CN); per-tool deep dives follow the same README.md/README-zh.md naming. Keep both languages equivalent and link to the matching language. keep install.sh, both root READMEs and workspace members in sync when adding or changing a tool.

shell portability lessons (install.sh, macOS /bin/bash 3.2):
- never write a bare `$VAR` immediately followed by a multibyte character (full-width ：（）；，？). Apple's bash 3.2 misparses the UTF-8 lead byte as part of the variable name; with `set -u` it spuriously aborts with `unbound variable` and the error message carries a raw 0xEF byte that crashes strict UTF-8 decoding of captured stderr in tests. always brace such expansions: `${VAR}：`.
- apple bash 3.2 differs from bash 4/5 beyond this bug (e.g. vanilla bash 3.2 ignores `set -e` for subshells called from if-contexts); `bash -n` on linux proves nothing for macOS. validate install.sh changes on a real macOS runner (temp debug branch + workflow_dispatch) before re-tagging a release.
- run `bash scripts/bump-version.sh vX.Y.Z` before tagging; it syncs workspace version, Cargo.lock, install.sh default, README examples and the ROADMAP dev version in one step, and refuses stale references.
