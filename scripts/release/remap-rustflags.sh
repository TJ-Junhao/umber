#!/usr/bin/env bash
# Print the rustc flags that replace this machine's paths with fixed placeholders, for use as
# RUSTFLAGS when building a release wheel. Paths are printed in the form rustc sees them, so on
# Windows they are converted to native paths.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"

native() {
    if command -v cygpath > /dev/null; then
        cygpath -w "$1"
    else
        printf '%s\n' "$1"
    fi
}

# When several mappings match a path, rustc uses the last one, so the most specific prefixes
# come last.
flags=(
    # Native code does not inherit CPython's frame-pointer flags (PEP 831).
    "-C force-frame-pointers=yes"
    "--remap-path-prefix=$(native "$HOME")=/home"
    "--remap-path-prefix=$(native "$cargo_home")=/cargo"
    "--remap-path-prefix=$(native "$rustup_home")=/rustup"
    "--remap-path-prefix=$(native "$root")=/build"
)
echo "${flags[*]}"
