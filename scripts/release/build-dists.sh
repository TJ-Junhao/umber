#!/usr/bin/env bash
# Build the release wheel and sdist into dist/ with local paths remapped, then
# verify that no build-machine path is embedded in the wheel.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"

# When several mappings match a path, rustc uses the last one, so the most
# specific prefixes come last.
remap=(
    "--remap-path-prefix=$HOME=/home"
    "--remap-path-prefix=$cargo_home=/cargo"
    "--remap-path-prefix=$rustup_home=/rustup"
    "--remap-path-prefix=$root=/build"
)
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }${remap[*]}"

rm -rf dist
mkdir dist
uv run maturin build --release --out dist
uv run maturin sdist --out dist

scan="$(mktemp -d)"
trap 'rm -rf "$scan"' EXIT
unzip -q dist/*.whl -d "$scan"

status=0
for needle in "$root" "$HOME" "/Users/"; do
    if grep -rIl -a -F -- "$needle" "$scan" || grep -rl -a -F -- "$needle" "$scan"; then
        echo "error: wheel contains build-machine path: $needle" >&2
        status=1
    fi
done
exit "$status"
