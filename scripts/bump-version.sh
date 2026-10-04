#!/usr/bin/env bash
# Bump the release version everywhere it is tracked: the single workspace
# version, the installer default, README examples and the ROADMAP dev version.
# Never commits or tags; review the diff and commit manually.
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

usage() {
    printf 'Usage: bash scripts/bump-version.sh vX.Y.Z\n' >&2
    exit 2
}

[[ $# -eq 1 ]] || usage
new=$1
[[ "$new" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || usage
new_bare=${new#v}

old=$(sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)"$/\1/p' Cargo.toml | head -n 1)
[[ -n "$old" ]] || { printf 'Cargo.toml: cannot read the workspace version\n' >&2; exit 1; }
[[ "$old" != "$new_bare" ]] || { printf 'Version is already %s\n' "$new" >&2; exit 0; }

esc_old=${old//./\\.}
esc_new=${new_bare//./\\.}

# Workspace manifest (members inherit version.workspace), installer default
# and README examples all use the same two textual shapes.
for file in Cargo.toml install.sh README.md; do
    sed -i.bak "s/v$esc_old/$new/g; s/version = \"$esc_old\"/version = \"$esc_new\"/g" "$file"
    rm -- "$file.bak"
done

# ROADMAP keeps historical baselines untouched; only the dev version moves.
sed -i.bak \
    -e "s/当前开发版本为 \`$esc_old\`/当前开发版本为 \`$esc_new\`/g" \
    -e "s/workspace version: \`$esc_old\`/workspace version: \`$esc_new\`/g" \
    ROADMAP.md
rm -- ROADMAP.md.bak

# The lockfile follows the manifest; never hand-edit dependency versions.
cargo update -w --offline >/dev/null

# Fail loudly on leftovers so nothing silently drifts.
if grep -rnF --include='*.toml' --include='*.sh' --include='*.md' -- "v$old" .; then
    printf 'Stale v%s references remain; fix them and rerun.\n' "$old" >&2
    exit 1
fi
for manifest in */Cargo.toml; do
    grep -q '^version\.workspace = true' "$manifest" || {
        printf '%s must inherit version.workspace\n' "$manifest" >&2
        exit 1
    }
done

git diff --stat
printf 'Bumped v%s -> %s. Review the diff, then commit and tag manually.\n' "$old" "$new"