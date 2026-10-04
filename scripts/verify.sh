#!/usr/bin/env bash
# Local validation only: never installs binaries or edits shell configuration.
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
for script in install.sh scripts/*.sh; do bash -n "$script"; done
python3 -m unittest discover -s tests -p 'test_installer.py' -v
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
host=$(rustc -vV | awk '/^host: / { print $2 }')
[[ -n "$host" ]]
cargo build --locked --release --workspace --target "$host" --target-dir "$ROOT/target/verify"
python3 tests/report_records_pty.py "$ROOT/target/verify/$host/release"
printf 'Validation passed. Binaries: %s\n' "$ROOT/target/verify/$host/release"
