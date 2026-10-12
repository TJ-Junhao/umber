#!/usr/bin/env bash
# Install a wheel into a clean virtual environment and test it there: the smoke script, then
# the font and license tests run against the installed package, never the source tree.
#
# Usage: scripts/release/test-wheel.sh WHEEL PYTHON_VERSION
# Requires uv. Works under Git Bash on Windows.
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 WHEEL PYTHON_VERSION" >&2
    exit 2
fi
wheel="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
python_version="$2"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

native() {
    if command -v cygpath > /dev/null; then
        cygpath -w "$1"
    else
        printf '%s\n' "$1"
    fi
}

uv venv --no-project --python "$python_version" "$(native "$work/venv")"
if [[ -x "$work/venv/Scripts/python.exe" ]]; then
    python="$work/venv/Scripts/python.exe"
else
    python="$work/venv/bin/python"
fi
uv pip install --python "$(native "$python")" "$(native "$wheel")" pytest

# The tests run from a directory holding only copies of the test files and an empty pytest
# configuration, so neither the repository's conftest nor its source tree is importable.
mkdir "$work/tests"
cp "$root/tests/python/test_fonts.py" "$root/tests/python/test_wheel_licenses.py" "$work/tests/"
touch "$work/tests/pytest.ini"

cd "$work/tests"
"$python" "$(native "$root/scripts/release/smoke_wheel.py")"
"$python" -m pytest -q -p no:cacheprovider test_fonts.py test_wheel_licenses.py
