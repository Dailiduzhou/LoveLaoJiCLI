#!/usr/bin/env bash
# Native release packaging; used by Actions and reproducible from a checkout.
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
tag=${1:?Usage: package-release.sh vX.Y.Z target-triple}
target=${2:?Usage: package-release.sh vX.Y.Z target-triple}
[[ $# == 2 && "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
case "$target" in
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin) ;;
    *) echo "Unsupported release target: $target" >&2; exit 1 ;;
esac
host=$(rustc -vV | awk '/^host: / { print $2 }')
[[ "$target" == "$host" ]] || { echo 'Use a native runner for build and smoke tests.' >&2; exit 1; }
staging=$(mktemp -d)
trap 'rm -rf -- "$staging"' EXIT
cargo metadata --locked --no-deps --format-version 1 > "$staging/metadata.json"
python3 - "$tag" "$staging/metadata.json" > "$staging/tools" <<'PY'
import json
from pathlib import Path
import re
import sys

tag = sys.argv[1]
metadata = json.loads(Path(sys.argv[2]).read_text())
packages = [p for p in metadata['packages'] if p['id'] in metadata['workspace_members']]
if any(p['version'] != tag[1:] for p in packages):
    sys.exit('Release tag must match every workspace package version.')
tools = sorted(t['name'] for p in packages for t in p['targets'] if 'bin' in t['kind'])
installer = Path('install.sh').read_text()
configured = re.search(r'^TOOLS=\(([^)]+)\)$', installer, re.M).group(1).split()
if tools != sorted(configured):
    sys.exit('Installer tool list differs from workspace binaries.')
if f'RELEASE_VERSION=${{LOVELAOJI_VERSION:-{tag}}}' not in installer:
    sys.exit('Update the installer default version before tagging.')
print('\n'.join(tools))
PY
cargo build --locked --release --workspace --target "$target" --target-dir "$ROOT/target/package"
mkdir "$staging/bin"
tools=()
while IFS= read -r tool; do
    binary="$ROOT/target/package/$target/release/$tool"
    for locale in en_US.UTF-8 zh_CN.UTF-8; do
        actual=$(LC_ALL="$locale" "$binary" --version)
        [[ "$actual" == "$tool ${tag#v}" ]]
        LC_ALL="$locale" "$binary" --help >/dev/null
    done
    install -m 755 "$binary" "$staging/bin/$tool"
    tools+=("$tool")
done < "$staging/tools"
mkdir -p "$ROOT/dist"
asset="lovelaojicli-$tag-$target.tar.gz"
# Explicit members: no leading './', directories, symlinks or extra files.
COPYFILE_DISABLE=1 tar -czf "$ROOT/dist/$asset" -C "$staging/bin" "${tools[@]}"
cd "$ROOT/dist"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$asset" > "$asset.sha256"
else
    shasum -a 256 "$asset" > "$asset.sha256"
fi
printf 'Packaged %s\n' "$ROOT/dist/$asset"
