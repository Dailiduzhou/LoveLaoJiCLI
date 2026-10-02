# sprinkle

A managed, isolated HEAD copy with a few kind comments. [中文](README-zh.md)

```sh
sprinkle                 # equivalent to sprinkle worktree
sprinkle diff            # creation-time additions, not later user edits
sprinkle undo            # return to the source workspace first
```

Supports `-h/--help`, `-V/--version`, and `--verison`. No branch mode,
`--force`, seed flag, density/theme/language options, or `clean` command.
Language follows `LC_ALL → LC_MESSAGES → LANG`.

## Creation and safety

The source must be a clean, writable, non-bare repository with HEAD, without
untracked nonignored files or in-progress Git operations. Submodules and
configured checkout/clean filters (including global LFS drivers) are conservatively
rejected. Hooks are disabled per Git invocation; no user config is changed.
Git probes disable external diff/textconv, fsmonitor, lazy fetching and network
protocols. Nothing is built, tested, installed, committed or pushed.

A private manifest is persisted before creating an exclusive ref and a unique
0700 worktree next to the source. The source's files, index and branch do not
change; the common Git directory necessarily gains a ref/worktree registration.
This is **HEAD, not a backup of your current disk**. Ignored files are not copied.
The source snapshot is budgeted (20,000 entries, 256 MiB reads, five seconds).
The first version does not support repositories that require those limits to be
exceeded, reftable-only reflogs, or unreliable filesystem permissions/locks.

Creation phases are persisted: prepared → ref-created → worktree-created →
planned → active. The manifest records raw-byte paths, branch, HEAD, worktree
backlinks, index/ref-log digests, each file's mode and original/expected digest,
insert offsets/text/message IDs, counts and the zero-context patch. State records
are authenticated and bound to their paths. Failed operations keep their recorded
phase and report its location; they never delete resources with uncertain ownership.

## Conservative comments

Candidates are tracked ordinary UTF-8 files with `.md`, `.go`, `.rs`, `.c`, `.h`,
`.cc`, `.cpp`, `.cxx`, `.hh`, `.hpp`, `.hxx` extensions, at most 1 MiB.
Exclude `.git`, `target`, `vendor`, `node_modules`, `dist`, `build`, `third_party`,
`generated` path components. Skip symlinks, generated headers (`@generated` or
`Code generated … DO NOT EDIT`), NUL, empty files, missing final newline,
mixed line endings, incomplete literals/comments/brackets and uncertain syntax.

The recognizer understands Rust nested comments/raw strings/lifetimes, Go raw
strings, C++ raw-string delimiters, and Markdown fences/comments. It deliberately
skips C preprocessing except simple includes/`#pragma once`, C line splicing and
trigraph/digraph ambiguity, Markdown front matter and non-comment embedded HTML.
It is **not a full parser or a guarantee of compilation**. EOF comments can still
affect source locations, snapshots, checksums and specialized tools.

Only one independent EOF position is considered. Sorted safe files have independent
25% selection probability, at most one comment each and 100 in total. Zero additions
is success and leaves a managed clean copy. Original bytes, BOM, newline style and
executable bits are preserved. The patch/expected hashes are saved before writes.
`SPRINKLE_SEED=<u64>` is a hidden testing hook: same version/content/locale/order
reproduces selection; resource IDs remain random. It never appears in help.

## Session selection and undo

Inside a recorded copy, `diff` selects that session. In its source workspace,
`diff`/`undo` select the newest non-undone session. Other worktrees cannot claim it
by branch name. Undoing the latest exposes the previous session.

Undo verifies metadata authentication, repository identity, canonical path,
bidirectional gitdir links, worktree registration/lock, HEAD/ref/reflog, index,
every tracked file/mode, and absence of **all extra untracked/ignored files**.
New commits, staged changes, manual edits, extra build products, replaced paths,
missing metadata or unknown intermediate phases cause refusal. There is no force
option. Stop editing/building before undo; our locks cannot stop other programs.

After verification, undo removes only recorded insertion bytes, verifies original
hashes and a completely clean checkout, then uses **non-force** worktree removal
and expected-old-OID ref deletion. No `reset --hard`, `git clean`, unknown branch
removal, or source checkout restoration. Durable restoring/removing/removed/
deleting-ref phases permit verified retry after interruptions. Uncertain partial
creation/removal remains for manual inspection. Completed manifests are retained
as small authenticated tombstones so sessions cannot be re-adopted accidentally.

`diff` emits only the saved creation patch; if the copy has diverged it warns on
stderr. Use ordinary `git diff` inside the copy for current edits.

## Privacy and failures

State and patches live in `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`
with 0700 directories and 0600 files; unsafe permissions, symlinks and lock failures
stop operations. Paths and patches are sensitive local data, not encrypted.
Uninstall preserves sessions and worktrees. No telemetry or remote access.
Success is 0, business/state/Git failure 1, usage failure 2.
