#!/usr/bin/env bash
# Fail if a wheel embeds a build-machine path: the repository root, the home directory or a
# typical user or CI home prefix.
#
# Usage: scripts/release/check-wheel-paths.sh WHEEL...
set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "usage: $0 WHEEL..." >&2
    exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
needles=("$root" "$HOME" "/Users/" "\\Users\\" "/root/." "/home/runner")
if command -v cygpath > /dev/null; then
    needles+=("$(cygpath -w "$root")" "$(cygpath -w "$HOME")")
fi

status=0
for wheel in "$@"; do
    scan="$(mktemp -d)"
    unzip -q "$wheel" -d "$scan"
    clean=1
    for needle in "${needles[@]}"; do
        if grep -rIl -a -F -- "$needle" "$scan" || grep -rl -a -F -- "$needle" "$scan"; then
            echo "error: $(basename "$wheel") contains build-machine path: $needle" >&2
            clean=0
            status=1
        fi
    done
    rm -rf "$scan"
    if [[ $clean -eq 1 ]]; then
        echo "$(basename "$wheel"): no build-machine paths"
    fi
done
exit "$status"
