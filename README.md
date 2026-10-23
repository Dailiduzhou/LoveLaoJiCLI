# LoveLaoJiCLI

[简体中文](README-zh.md)

Love you, dear myself CLI tools, powered by Rust and clap.

See [ROADMAP.md](ROADMAP.md) for implementation status, limitations, outstanding validation and upcoming batches. This README documents implemented interfaces; planned interfaces are not yet available.

## Build and run

Requires Rust 1.89+ and Cargo. All three batches target Linux, macOS and WSL Linux; Git features require local Git.

```sh
cargo build --workspace --release

./target/release/love
./target/release/happiness
./target/release/joy
./target/release/patience sleep 3

# Or:
cargo run -p love -- --help
cargo run -p happiness -- --version
cargo run -p joy
cargo run -p patience -- sh -c 'echo hi'
```

All fourteen tools have independent directories: `love/`, `happiness/`, `joy/`, `patience/`, `sprinkle/`, `later/`, `enough/`, `stuck/`, `duck/`, `one/`, `afk/`, `goodnight/`, `proof/` and `poke/`. Each is independently usable; `cli-common/` shares CLI, locale, private state, Git snapshots and execution plumbing.

## Interactive installation and uninstallation

Run with Bash on Linux/macOS/WSL, without sudo or Rust. Requires `curl`, `tar`, `sha256sum` (Linux) or `shasum` (macOS), and standard Unix tools.

Download the script from raw.githubusercontent.com, inspect it, then execute it. Binaries come from GitHub Releases, not the raw source tree. Do not pipe into Bash: confirmation needs stdin.

```sh
curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
  https://raw.githubusercontent.com/Dailiduzhou/LoveLaoJiCLI/main/install.sh \
  --output install.sh
# After inspecting the script:
bash install.sh install
```

```sh
bash install.sh                 # Menu: release install / uninstall / local build / exit
bash install.sh install         # Download the script's release version
bash install.sh uninstall       # Confirm uninstallation
bash install.sh --help
LOVELAOJI_VERSION=v0.5.0 bash install.sh install  # Pin a version

# Source checkout only; local build installation still requires Rust/Cargo:
./install.sh install-local
```

The script defaults to `v0.5.0`, not a mutable latest release. Assets must be published first; missing releases/network failures are errors, never a silent source-build fallback. Replace `main` in the raw URL with a release tag to pin the script too.

| Platform | Release target | OS baseline |
| --- | --- | --- |
| Linux / WSL x86_64 | `x86_64-unknown-linux-gnu` | glibc 2.35+ (Ubuntu 22.04+) |
| Linux / WSL ARM64 | `aarch64-unknown-linux-gnu` | glibc 2.39+ (Ubuntu 24.04+) |
| macOS Intel | `x86_64-apple-darwin` | macOS 13+ |
| macOS Apple Silicon | `aarch64-apple-darwin` | macOS 13+ |

Platform selection uses uname. Linux packages use glibc, not static musl; Alpine/musl and native Windows are unsupported. Try source builds on older systems. Git features still require local Git; local dates require system timezone data. macOS binaries are not Apple-signed or notarized.

- Enter `y` to confirm installation; an empty answer or EOF cancels without downloads or configuration changes.
- The platform archive contains all fourteen tools. Its SHA-256, exact member list and executable versions are checked before updating; verification failures preserve the existing installation. Checksums provide integrity, not independent signatures; trust in the repository and HTTPS is still required.
- Install into `${XDG_DATA_HOME:-$HOME/.local/share}/lovelaojicli/bin`. `install-local` uses locked dependencies for a native release build in `target/installer/<target>/release/`.
- PATH setup follows `$SHELL`: Bash uses `.bashrc` and the effective login profile; Zsh uses `.zshrc` and `.zprofile` under `${ZDOTDIR:-$HOME}`; Fish uses `${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/lovelaojicli.fish`.
- Project-marked PATH blocks make reinstallation idempotent. The installation directory takes precedence over the existing PATH; same-named programs elsewhere are not overwritten.
- Offline uninstallation needs no Rust. It removes only the fourteen managed binaries and project PATH blocks, preserving other configuration, files, user state and source-build artifacts.
- Installer prompts follow the locale. Keep the same custom `XDG_DATA_HOME` when uninstalling.

Open a new terminal after installation/uninstallation. To use the tools immediately, run the PATH command printed by the installer. Execute the installer; do not source it.

## Releases

The [release workflow](.github/workflows/release.yml) runs on stable version tags or manual dispatch with an existing tag. Full Linux validation precedes four native builds and bilingual help/version smoke tests. Each platform provides one archive containing all fourteen tools plus a SHA-256 file. A draft is published only after all builds succeed; published releases cannot be overwritten by a rerun.

Before tagging, run `bash scripts/bump-version.sh vX.Y.Z`; it synchronizes the workspace version, lockfile, installer default, examples in both `README.md` and `README-zh.md`, and the ROADMAP dev version, and rejects stale references or crates not inheriting the workspace version. Review the diff, commit, then tag. Packaging rejects version mismatches and installer/workspace tool-list drift. Enable Actions and permit the publish job's `contents: write`; other jobs are read-only.

```sh
# Use a version that has not already been published:
git tag vX.Y.Z
git push origin vX.Y.Z

# Optional native packaging; does not publish anything:
bash scripts/package-release.sh vX.Y.Z "$(rustc -vV | awk '/^host: / { print $2 }')"
# Output: dist/lovelaojicli-vX.Y.Z-<target>.tar.gz[.sha256]
```

## Features

- `love/happiness/joy` print a localized blessing with no arguments.
- `--help` / `-h`: show help.
- `--version` / `-V`: show version.
- `--verison`: accepted as a compatibility alias.

No subcommands or additional arguments for love, happiness or joy.

### patience: Fake progress bar wrapper

`patience <command> [args...]` runs a real command and performs a fake progress bar show around it:

- Each run picks one random speed curve (linear, ease-in, ease-out, sigmoid, stepped, exponential, square-root).
- Multiple climbs and short stalls, then a hold at 97–99.9%; the bar never moves again until the child exits.
- On success the bar rapidly fills to 100%; on failure it stays parked and never fills.
- Localized comfort messages while stalled (shuffled, never repeated back-to-back); the result line carries the real elapsed time and exit code.
- Exit codes pass through from the child (128+N on signals); Ctrl+C reaches the child via the shared process group.
- Child output is captured (1 MiB per stream) and replayed after the bar.
- The bar is a rainbow marquee: a seamless looping palette of 13 anchors, band boundaries slanted 45° by default, sliding one cell per frame; the filled part is the animated rainbow, the empty part stays gray `░`.
- Colors degrade with the terminal's capability: truecolor → 256 → 16 → plain; `NO_COLOR` and `TERM=dumb` are honored, and non-TTY runs still render nothing.
- Special case: each extra leading `patience` token adds one more independently performing bar to the show; the real command is whatever follows them (`patience patience sleep 3` shows two bars at once).
- Stderr-sized animation reserves a column/row, omits messages before shortening bars, and falls back when geometry/capabilities are unsafe. Any observed resize permanently stops redraw for this invocation; the child still runs once. `NO_COLOR` disables color only.
- Non-TTY and conservative fallbacks emit no animation controls, just replay and the result line.

```sh
patience make -j4        # Pretend to compile very seriously
patience sleep 5        # Watch it stall near 98%
patience -- git status  # Use -- to separate wrapper and child arguments
patience patience sleep 5  # nest it to watch two bars at once
```

Implementation notes and design decisions: [`patience/README.md`](patience/README.md).

Hidden test hooks (not in `--help`): `PATIENCE_SEED=<u64>` fixes randomness; `PATIENCE_FAST=1` scales every timing to 1%.

## First batch

The first-batch MVP is implemented, not every option in the original ideas; see the [roadmap](ROADMAP.md) for status and follow-up validation. The local `project.md` retains the detailed design but is not currently distributed with the repository.

```sh
sprinkle                       # isolated HEAD copy
sprinkle worktree              # same as the default
sprinkle diff                  # original insertion patch
sprinkle undo                  # verified undo only
later --next "check token expiry"  # save
later                          # one interactive question
later resume                   # display, never execute
enough cargo test             # recent-success reminder
enough --again cargo test     # always execute
stuck cargo test               # hypothesis after three equal failures
stuck --hypothesis "increase timeout" cargo test
```

- **sprinkle**: Requires a clean source; adds conservative EOF comments to Markdown, Go, Rust and C/C++ files only in its new managed worktree. No build or validation scripts are executed. Fixed 25% selection, one per file, 100 total; zero additions is valid. The original checkout stays unchanged, but shared Git metadata gains a branch/worktree. This is a HEAD copy, not a backup or compilation guarantee.
- **later**: One latest card per worktree (per cwd without Git). `--next` is a nonempty single line, at most 2,000 characters. Without a foreground terminal or explicit answer it returns 125, without consuming pipes or replacing the old card.
- **enough**: Can skip only a matching invocation with a stable snapshot within five minutes of real success. Notices use stderr; skips have empty stdout and do not extend the window. `--again` always runs and invalidates old success on failure. This is not a build cache or proof about external state.
- **stuck**: Three equal failures gate the fourth call. A nonempty single-line hypothesis (512 characters maximum) grants one execution, not permanent bypass. Success, changes, overlap or incomplete output break the chain. Streams are forwarded live and hashed separately with bounded memory; no output bodies are stored. The child sees pipes, not a PTY.

`later resume` puts the next action first, then elapsed age and the saved snapshot; empty tracked diffs are distinct from unknown data or a clean worktree. Full HEAD, files/truncation and context warnings remain. Every `enough` skip explicitly says it was skipped and suggests `enough --again <command> [args...]`, without echoing real argv.

**Known long-input limitation:** Interactive `duck/later/stuck` still use byte-limited system canonical buffers; long pastes can lose newlines or hang (duck's 600-CJK case reproduced on macOS). This is not fixed: use explicit answer options for long text, never secrets or piped answers; Ctrl+C cancels. A safe input redesign awaits separate approval. Explicitly ignored long-input regressions are not passes. See [ROADMAP.md §3.6](ROADMAP.md#36-实测修复方案实施--improvement-plan-implementation).

Put wrapper options before the child command. Child arguments retain their boundaries; use `--` for disambiguation. No implicit shell is involved, and child stdin is inherited.

Wrappers fail open when Git/state/locks/snapshots are unavailable or stdin is not comparable (including pipelines and redirected input). Launch errors use 127/126; real child codes pass through, signals map to 128+N. Tool errors use 1, usage errors 2, and unavailable answers/interception 125. Foreground signals reach the shared process group; deliberately daemonizing commands are outside this guarantee.

All first-batch tools are desktop-free, offline and local-only, with no telemetry, history scanning or shell hooks. Tool-generated output has no ANSI escapes; child output stays untranslated and unchanged.

Deep dives: [sprinkle](sprinkle/README.md) · [later](later/README.md) · [enough](enough/README.md) · [stuck](stuck/README.md).

If symlinked state is rejected, inspect HOME/XDG_STATE_HOME components (for example macOS `/tmp → /private/tmp`); product checks never auto-canonicalize an untrusted path to bypass rejection. Trusted test temp paths are resolved first; only explicit unsupported non-UTF-8 filename capabilities are skipped, not unrelated I/O errors.

### State, privacy and failures

State lives under `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`. Invalid XDG paths fall back to absolute HOME, never cwd or `/tmp`. Managed directories/files use 0700/0600, symlinks and unsafe permissions are rejected. Records use same-directory atomic replacement, sync, nonblocking OS locks, versioned JSON and keyed BLAKE3 checksums. Locks coordinate these tools only; filesystems without reliable permission/lock semantics are not promised equivalent safety.

Wrapper arguments are retained only as a keyed, boundary-preserving digest plus program basename, not raw argv, output bodies or environment. **Next actions and hypotheses are plaintext: never enter secrets.** CLI values can also appear in shell history/process lists. Paths, filenames, Git metadata and sprinkle patches can be sensitive. Lost/corrupt identity keys invalidate old records; hashing is not encryption.

Recognized inactive runs/hypotheses older than 30 days are lazily pruned; unknown/corrupt records are left alone. Cards remain until successfully replaced; active sprinkle sessions never expire. Uninstallation preserves all user state and worktrees.

Snapshots include HEAD, index entries, contents/deletions/types/modes, symlink text, nonignored untracked files and cwd. Two scans detect changes, and execution has pre/post snapshots. Budgets are 20,000 entries, 256 MiB total reads and five seconds; overflow, conflicts, submodules, configured filters or races disable optimization. This is not a filesystem transaction or evidence about ignored/external state.

Hidden test hook: `SPRINKLE_SEED=<u64>` fixes comment selection for the same version/content/locale/candidate order, not resource IDs. Wrapper tests use real PTYs, not safety-bypass flags.

## Second batch

```sh
duck                          # four foreground questions
duck --expected "success" --actual "timeout" --last-change "retry logic" --experiment "disable retries"
one add "check token expiry"
one                           # stable selection
one done                      # complete persistent selection
cat TODO.md | one             # temporary selection
afk 5m                        # quiet break; Ctrl+C cancels
afk 5m --color rainbow        # rainbow marquee
afk 5m --color parts          # default component colors
afk 5m --color white          # all white
```

- **duck**: Four fixed answers in order, ending with `quack.`. Complete explicit arguments skip prompts; missing answers require a foreground terminal. Prompts use the controlling terminal, the complete summary uses stdout. Answers are trimmed, nonempty, single-line, at most 2,000 characters, without control characters. Missing interaction, EOF or invalid interactive input returns 125 with empty stdout; pipes are not answers. No persistence, AI, search or advice.
- **one**: One global local list (1,000 tasks maximum; each a trimmed, nonempty single line of up to 2,000 characters, no controls). Selection is persisted until `done` removes it; adding tasks does not change it. The next bare invocation selects again. Piped/redirected UTF-8 input is temporary (1 MiB / 1,000 tasks maximum), filtering blanks, ATX headings and checked items and stripping common list prefixes; not a full Markdown parser. Temporary input never opens state. `add`/`done` ignore stdin; `/dev/null` uses persistent tasks. Empty lists succeed; `done` without selection does not select one. Corruption, unknown schemas or lock contention fail closed with exit 1.
- **afk**: Positive integer plus `s/m/h`, up to 24h, timed monotonically. A suitable shared foreground terminal gets a colored Ratatui ASCII garden and remaining time (`--color parts` keeps default component colors, `rainbow` animates color bands, `white` is all white; nonempty `NO_COLOR` overrides all modes with the default foreground). Keys show `the grass noticed.` without penalties or timer changes. Ctrl+C cancels; Ctrl+Z restores before stopping, time keeps advancing, fg can resume and bg never reads keys. Non-TTY, redirected output, missing/dumb TERM, background startup or terminals smaller than 24×10 wait quietly and emit one completion line. Cbreak-style input retains signals, with exact termios and screen/cursor RAII restoration—not Crossterm raw mode. On foreground ownership loss, stale settings and cleanup escapes are abandoned rather than applied to the new owner. Ratatui/signal-hook/libc are scoped to afk. SIGKILL, external SIGSTOP, abort or terminal loss cannot guarantee cleanup; see the tool README for evaluation and boundaries.

Persistent empty `one` lists suggest `one add "next small step"`; empty/fully filtered temporary input says `No selectable tasks in the input.` without opening global state. `done` without selection still never chooses a task. Nonempty output remains task text only.

Tasks are local plaintext in `one/tasks.json` under the state root, with shared private permissions, atomic writes, version validation and nonblocking locks. Completion removes the task without history; uninstall preserves state. Do not enter secrets; CLI arguments can enter shell history/process lists. Duck/afk never create state; all three are offline. Usage errors return 2, I/O/state errors 1, success 0; duck without an answer returns 125.

Hidden test hooks (not in help): `ONE_SEED=<u64>` fixes random selection for the same version/candidate order, not task IDs or existing selections; invalid values are ignored. `AFK_FAST=1` scales waits to 1% without relaxing duration validation. `AFK_TEST_FAILURE=enter|draw|panic` injects post-entry/backend-write/panic failures; `read-stop` raises SIGSTOP once between the foreground check and first input read so a PTY controller can transfer ownership and resume the process to reproduce the race. TUI mode only; unknown values are ignored, for isolated tests only.

Deep dives: [duck](duck/README.md) · [one](one/README.md) · [afk](afk/README.md).

## Third batch

```sh
goodnight                     # read-only session ending
proof                         # available local evidence today
poke add "Xiao Ming"
poke                          # suggests, never sends
poke remove "Xiao Ming"
```

- **goodnight**: Local time with UTC offset, branch/detached HEAD, Git unfinished-path count and the latest explicitly recorded command in the current workspace. Missing/corrupt/busy or ambiguously ordered data stays unknown. It does not identify test results. Unfinished paths can prompt a manual later invocation, never an automatic call. No saved card, shell lock, shutdown or enforced schedule; non-Git still gets time and a closing line.
- **proof**: Only locally reachable history from current HEAD, matching **both** configured author name and email exactly. Commits are filtered by committer timestamp within today's local calendar boundaries, excluding future times. Commit count includes merges; line totals sum non-merge patches including root commits, with byte-exact path deduplication. Rename detection is off (old/new paths count separately); binaries contribute paths but no line counts. Each patch uses its own committed attributes, isolated from working-tree/index and global/system attribute files; local `info/attributes` or unsupported required `--attr-source` makes Git evidence unknown. No scanning of other refs/repositories. Missing identity, shallow history, failed probes or budget exhaustion makes Git evidence unknown.
- **Execution evidence**: Both reports read existing enough/stuck records only. Proof counts today's complete real executions by completion time in this worktree (cwd outside Git). Successful executions are not called passed tests. Skips, gates, spawn failures, unfinished/incompletely forwarded runs and future records do not count. Without available evidence it says `no recorded work.`, retaining unknown-source labels—not a claim of no work or a score. Reads never create directories/keys/lock files, repair state or prune history; existing shared locks fail conservatively on contention.
- **poke**: Global local list: trimmed nonempty single-line names/nicknames, at most 200 characters without controls, up to 1,000 names. Exact case/Unicode deduplication; empty lists, duplicate adds and missing removes succeed kindly. Uniform random selection permits repeats. Suggestions only: no selection/contact history, reminder debt, address-book access, API or messaging.

Only goodnight/proof enable optional `jiff` (system timezone and zoneinfo, no bundled database). System timezone/`TZ` failures stay unknown, never silently UTC. Local days use calendar boundaries, not fixed 24-hour DST assumptions. Git probes retain 5-second/32-MiB limits; proof caps history at 20,000 commits, today's matching commits at 1,000, changed paths at 20,000 and cumulative numstat at 32 MiB, checking a 10-second collection budget between probes. Record scans cap at 10,000 entries/32 MiB with a 5-second inter-record budget. Budget failure suppresses that source's partial numbers.

Poke stores plaintext names in `poke/names.json` under the state root, using existing private permissions, atomic writes, validation and nonblocking locks. Corruption/unknown schema/unsafe permissions/contention returns 1 without reset. Do not enter secrets; uninstall preserves the list. Hidden `POKE_SEED=<u64>` fixes choices for the same version/list order; invalid values are ignored, and the hook is absent from help.

Third-batch tools never read stdin and work headlessly. Success/conservative unknown reports return 0, output/state-write errors 1, usage errors 2; Ctrl+C uses default signal handling.

Deep dives: [goodnight](goodnight/README.md) · [proof](proof/README.md) · [poke](poke/README.md).

## Language

The first nonempty value in `LC_ALL` → `LC_MESSAGES` → `LANG` selects the language.

- `zh_CN`, `zh-CN` (also `zh`): Simplified Chinese.
- `en_US`, `en-US`: English.
- Missing values, `C`, `POSIX` and unsupported locales fall back to English.

Encoding/modifier suffixes are accepted; locale names are case-insensitive.

```sh
LANG=zh_CN.UTF-8 ./target/release/love
# 爱你老己。
LC_ALL=en_US.UTF-8 ./target/release/love
# Love yourself, dear me.
LC_ALL=zh_CN.UTF-8 ./target/release/joy --help
```

Help and default messages are localized; versions are language-neutral, and argument errors use clap's English diagnostics.

## Code structure

Modules separate responsibilities without changing CLI arguments or persisted record formats.

- `afk/src/tui.rs`: Timer/event loop, terminal restoration, and signal/job-control guards; guard lifetime ordering stays in the loop.
- `sprinkle/src/`: Session metadata, creation transaction, insertion planning, syntax checks, ownership verification, and resumable undo.
- `one/src/`: Isolated stdin parsing, validated task model/stable selection, and CLI/locked persistence.
- `later/src/`: CLI/lock orchestration, card schema/Git capture, and read-only presentation.
- `enough/src/`, `stuck/src/`: Each tool owns its CLI/help and policy: success reminders or failure gates, prompts, and baseline decisions.
- `cli-common/src/repeat/`: Shared fail-open execution, lease/lock/snapshot/baseline persistence, and cross-tool record formats/readers; no tool-name business dispatch.
- Tool-specific code, text, arguments, test scenarios and assets live under `<tool>/`. Root `tests/` holds reusable harnesses/parameterized contracts, cross-tool acceptance and installer tests; `scripts/` holds workspace automation.
- `cli-common/` never depends on tool crates. Historical record formats used across tools, including legacy producer validation/completeness rules, remain a shared compatibility contract; module moves do not change persisted formats.

## Tests

The local verification script never installs or edits shell configuration. Requires Rust (rustfmt/clippy), Cargo, Git and Python 3. It checks README links/language parity and version synchronization, then runs isolated installer tests, formatting, workspace tests, clippy, a locked release build and cross-tool PTY acceptance. Binaries remain in `target/verify/<target>/release/`.

Current macOS automation (see [ROADMAP.md §3.6](ROADMAP.md#36-实测修复方案实施--improvement-plan-implementation)): 191 Rust passes, 3 explicitly ignored known long-input regressions, and 25 installer passes. Ignored/unsupported-capability cases are not passes; current Linux/WSL reruns, GUI acceptance and a safe long-input redesign remain pending.

```sh
./scripts/verify.sh

# Or run checks individually:
cargo fmt --all -- --check
cargo test --locked --workspace    # Git + Python 3 required
cargo clippy --locked --workspace --all-targets -- -D warnings

# Documentation links, examples and isolated version-bump tests
python3 -m unittest discover -s tests -p 'test_documentation.py' -v

# Installer tests (isolated HOME; mock compiler/downloads, no network)
bash -n install.sh
python3 -m unittest discover -s tests -p 'test_installer.py' -v

# Cross-tool real execution, skip and gate checks
cargo build --locked --workspace --release
python3 tests/report_records_pty.py target/release
```
