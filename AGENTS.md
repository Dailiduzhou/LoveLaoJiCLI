build multiple CLI tools in Rust, using clap crate.
keep minimal feature. each supports basic `--verison`, `--help`, and the CLI name itself.
each tool has their own folders.
support zh-CN and en-US. change display language according to locale (LC_ALL -> LC_MESSAGES -> LANG).
share locale detection and clap scaffolding in `cli-common/`.

dependencies: clap/rand plus narrowly scoped serde/serde_json (records), blake3 (stable keyed fingerprints), and rustix (Unix ownership/foreground and stdin-type checks) are allowed for all three batches. jiff is additionally allowed only for goodnight/proof local dates and DST boundaries, through the optional cli-common/local-time feature (std/tz-system/tzdb-zoneinfo; no bundled database). For afk only, ratatui (default features off, crossterm_0_29), signal-hook (signal flags), and libc (scoped Unix job-control signal masks) are additionally allowed after terminal-lifecycle evaluation. rustix may snapshot/restore afk termios. Prefer std for everything else. workspace version stays in sync across all crates.
love/happiness/joy keep only basic flags. New tools support only the MVP business arguments in project.md part 2. Test hooks are environment variables, documented in README, never in `--help`.

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
   - no crossterm raw mode/event loop or cursor queries; cbreak keeps terminal signals.
   - Ratatui/signal-hook/libc are scoped to afk; duck/one keep their existing dependencies.
   - one state failures fail closed; duck/afk never open state.
   - ONE_SEED, AFK_FAST and AFK_TEST_FAILURE are hidden test hooks, not CLI options.

12. goodnight - read-only local time, Git and explicit execution summary. (done)
13. proof - current-HEAD/current-workspace evidence for the local calendar day. (done)
14. poke - private global name list, exact deduplication and uniform random reminder. (done)
   - goodnight/proof never create state or prune records; unavailable sources stay unknown.
   - proof uses exact configured author name AND email, committer timestamps, non-merge numstat.
   - poke fails closed on state errors; POKE_SEED is a hidden test hook.
   - third-batch targets remain Linux/macOS/WSL Unix; runtime is offline and local-only.

behavior specs live in README.md (bilingual); per-tool deep dives live in the tool's own README.md (en-US) and README-zh.md (zh-CN). keep install.sh, README.md and workspace members in sync when adding or changing a tool.
