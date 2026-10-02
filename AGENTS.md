build multiple CLI tools in Rust, using clap crate.
keep minimal feature. each supports basic `--verison`, `--help`, and the CLI name itself.
each tool has their own folders.
support zh-CN and en-US. change display language according to locale (LC_ALL -> LC_MESSAGES -> LANG).
share locale detection and clap scaffolding in `cli-common/`.

dependencies: clap/rand plus narrowly scoped serde/serde_json (records), blake3 (stable keyed fingerprints), and rustix (Unix ownership/foreground checks) are allowed for the first batch. Prefer std for everything else. workspace version stays in sync across all crates.
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

behavior specs live in README.md (bilingual); per-tool deep dives live in the tool's own README.md (en-US) and README-zh.md (zh-CN). keep install.sh, README.md and workspace members in sync when adding or changing a tool.
