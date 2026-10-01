build multiple CLI tools in Rust, using clap crate.
keep minimal feature. each supports basic `--verison`, `--help`, and the CLI name itself.
each tool has their own folders.
support zh-CN and en-US. change display language according to locale (LC_ALL -> LC_MESSAGES -> LANG).
share locale detection and clap scaffolding in `cli-common/`.

dependencies: clap and rand are allowed. keep everything else std-only. workspace version stays in sync across all crates.
no user-facing flags beyond the basics. test hooks are environment variables, documented in README, never in `--help`.

tools (status):
1. love - one localized blessing. (done)
2. happiness - same pattern. (done)
3. joy - same pattern. (done)
4. patience - fake progress bar wrapper for a real subcommand. (done)
   - exit code passes through from the child (128+N on signals).
   - not a TTY: no animation, plain result line.

behavior specs live in README.md (bilingual). keep install.sh, README.md and workspace members in sync when adding or changing a tool.