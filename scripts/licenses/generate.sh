#!/usr/bin/env bash
# Regenerate THIRD-PARTY-LICENSES.txt for the crates linked into umber-py.
# Requires cargo-about 0.9.2: cargo install --locked cargo-about@0.9.2 --features cli
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

cargo about generate \
    --locked \
    --manifest-path crates/umber-py/Cargo.toml \
    --config about.toml \
    --output-file THIRD-PARTY-LICENSES.txt \
    about.hbs
