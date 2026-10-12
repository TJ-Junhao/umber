#!/usr/bin/env bash
# Build the release wheel and sdist into dist/ with local paths remapped, then
# verify that no build-machine path is embedded in the wheel.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }$(scripts/release/remap-rustflags.sh)"
export RUSTFLAGS

rm -rf dist
mkdir dist
uv run maturin build --release --out dist
uv run maturin sdist --out dist

scripts/release/check-wheel-paths.sh dist/*.whl
