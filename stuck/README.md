# stuck

A tiny hypothesis → experiment → observe loop. Not a debugger. [中文](README-zh.md)

```sh
stuck [--hypothesis <text>] [--] <command> [args...]
stuck cargo test
stuck --hypothesis "increase etcd dial timeout" cargo test
```

After three consecutive identical failures, the fourth matching call asks
**What are you changing?** on the foreground controlling terminal. EOF, blank or
invalid answers return 125 without running the child or erasing the existing
count. A valid explicit/interactive hypothesis grants exactly one execution and
starts a new chain (its first failure counts as one). It can be supplied before
any invocation; it is not a permanent bypass. No hypothesis quality judgement,
NLP, AI, debugger, automatic retries or command rewriting.

Hypotheses trim surrounding whitespace and must be nonempty single lines of at
most 512 Unicode characters. Invalid explicit values exit 2. Wrapper flags must
precede the child, whose argv remains unchanged; shell syntax requires explicit
`sh -c`. Help/version/`--verison` and messages follow the common locale contract.

## Exact failure identity

The signature includes cwd/argv digest, a stable Git code snapshot, ordinary
nonzero exit code, and independent stdout/stderr byte digests and lengths.
Timestamps, colors and paths are not normalized. A different command, code,
output, exit code, success or overlap resets the chain. Signal termination,
spawn errors and incomplete reads/forwarding are not reliable failure samples.
Old repeat records expire after 30 days.

Each pipe is drained concurrently and forwarded immediately to its original
stream while incrementally computing keyed BLAKE3 with fixed-size buffers.
No full output is retained; ordering is preserved within each stream but not
across the two. The child sees pipes: this is **not a transparent PTY**, and
full-screen interactive programs are unsuitable. stdin is always inherited;
questions use `/dev/tty` only after foreground stdin/stderr checks. Unexpected
pipe input is never read as an answer.

Non-Git, missing Git, conflicts/submodules/filters, unavailable snapshots,
state/locking faults or incomparable input fail open and run the command. In
particular redirected/piped stdin cannot trigger interception even if an old
failure chain exists. Prompts with unusable answers return 125 only when a
reliable comparable invocation reached the gate. Record failure does not
change the real child exit code or falsely announce hypothesis persistence.

Execution uses separate real argv, shared foreground process groups and no
signal handler. Child exit codes pass through (signals 128+N); not found is 127,
not executable 126. Real children may themselves return 125; records distinguish
that from an interception. No detached child is created; intentional daemonization
by the child remains outside this contract.

## Privacy and persistence

Hypotheses are **local plaintext: never enter passwords or tokens**.
`--hypothesis` may also appear in shell history or process listings. Only the
program basename and a local keyed argv digest are recorded, not shell bodies,
raw argv, environment or output. Paths/program names may still be sensitive.
There is no telemetry, network, history scanning, shell hook or background agent.

State uses the common XDG/HOME state root, 0700 directories/0600 files, atomic
versioned JSON, keyed checksums, symlink rejection and OS locks. Execution leases
are separate from short write locks; overlapping/late results cannot overwrite
new baselines. Recognized inactive runs/hypotheses older than 30 days are lazily
pruned. Uninstall preserves state. See the root README for snapshot budgets,
key-loss behavior and filesystem limitations.

Tests: `cargo test -p stuck` requires Git and Python 3, with foreground PTYs,
large simultaneous binary streams, cancellation and invalid-input cases.
