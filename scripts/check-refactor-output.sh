#!/usr/bin/env bash
# Exact-output gate for refactors: proves a change did not alter what umber-core produces.
#
# The corpus lives in crates/umber-core/tests/support/refactor_cases.rs and is rendered by
# crates/umber-core/examples/refactor_outputs.rs. For every case the gate hashes
#   scene/   the Scene debug serialization (shortest round-trip floats, so equal text means
#            equal bits; NaN payloads are not distinguished)
#   svg/     the SVG bytes, in both text modes
#   png/     the encoded PNG bytes at 72 and 200 dpi, or the error text
#   errors/  Display and Debug text of errors from public APIs
# and compares the hashes with a baseline. Any difference fails the gate.
#
# Usage:
#   scripts/check-refactor-output.sh check [--png]
#       Compare this tree with the committed baseline manifest
#       (crates/umber-core/tests/refactor/baseline-manifest.txt, made at b2e0207).
#       Scene, SVG and error hashes are compared by default. PNG hashes depend on the machine's
#       floating-point and SIMD behaviour, so --png is only meaningful on the machine that
#       generated the baseline (a macOS arm64 laptop for the committed one).
#   scripts/check-refactor-output.sh compare <git-rev> [--png|--no-png]
#       Build the corpus at <git-rev> (a temporary worktree that gets this tree's corpus
#       files) and at this tree, in the same environment, and compare everything including
#       PNG. This is the check that holds on any machine and needs no committed baseline.
#   scripts/check-refactor-output.sh manifest <file>
#       Write the manifest of this tree to <file>.
#   scripts/check-refactor-output.sh update
#       Overwrite the committed baseline with this tree's manifest. Only do this from a
#       revision whose output is known to be right, and say so in the commit message.
#
# Conditions: the same Rust toolchain, Cargo.lock and bundled font files on both sides; the
# corpus uses the in-memory fallback font and never reads system fonts. Everything is built
# in release mode with UPDATE_GOLDEN and INSTA_UPDATE unset.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
baseline="crates/umber-core/tests/refactor/baseline-manifest.txt"
# UMBER_REFACTOR_BASELINE (an absolute path) replaces the committed baseline in `check`.
baseline_file="${UMBER_REFACTOR_BASELINE:-$root/$baseline}"
driver_files=(
  crates/umber-core/examples/refactor_outputs.rs
  crates/umber-core/tests/support/refactor_cases.rs
)

tmp="$(mktemp -d "${TMPDIR:-/tmp}/umber-refactor-gate.XXXXXX")"
extra_worktree=""
cleanup() {
  if [[ -n "$extra_worktree" ]]; then
    git -C "$root" worktree remove --force "$extra_worktree" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

# manifest_of <source-tree> <target-dir> <out-file>
manifest_of() {
  local tree="$1" target="$2" out="$3" dump
  dump="$(mktemp -d "$tmp/dump.XXXXXX")"
  (
    cd "$tree"
    # An empty target means the tree's own target directory, so repeated runs stay incremental.
    if [[ -n "$target" ]]; then export CARGO_TARGET_DIR="$target"; fi
    env -u UPDATE_GOLDEN -u INSTA_UPDATE \
      cargo run --release --locked -q -p umber-core --example refactor_outputs -- "$dump" >&2
    cd "$dump"
    find . -type f | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r file; do
      sha256_of "$file"
    done
  ) >"$out.body"
  {
    echo "# umber refactor output manifest: sha256, then kind/file"
    echo "# revision: $(git -C "$tree" rev-parse HEAD 2>/dev/null || echo unknown)"
    echo "# platform: $(uname -sm)"
    echo "# rustc: $(rustc --version)"
    cat "$out.body"
  } >"$out"
  rm -f "$out.body"
}

# select <manifest> <with-png: 1|0>: the comparable lines, with the "  " separator normalised.
select_lines() {
  local file="$1" png="$2"
  grep -v '^#' "$file" | sed 's/  */ /' | if [[ "$png" == 1 ]]; then cat; else grep -v ' png/'; fi
}

compare_manifests() {
  local a="$1" b="$2" png="$3" a_name="$4" b_name="$5"
  select_lines "$a" "$png" >"$tmp/a.sel"
  select_lines "$b" "$png" >"$tmp/b.sel"
  local count
  count="$(wc -l <"$tmp/a.sel" | tr -d ' ')"
  if diff -u --label "$a_name" --label "$b_name" "$tmp/a.sel" "$tmp/b.sel" >"$tmp/diff.txt"; then
    echo "refactor output gate: PASS ($count outputs identical; png compared: $([[ $png == 1 ]] && echo yes || echo no))"
  else
    echo "refactor output gate: FAIL, outputs differ ($a_name vs $b_name)" >&2
    cat "$tmp/diff.txt" >&2
    exit 1
  fi
}

usage() {
  sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
}

# usage_error <message>: every malformed command line ends here with exit status 2.
usage_error() {
  echo "error: $1" >&2
  usage
  exit 2
}

# parse_flag <allowed-flag> <args...>: set `flag` to 1 when the only argument is the allowed flag.
# Any other argument, or more than one, is a usage error.
flag=0
parse_flag() {
  local allowed="$1"
  shift
  flag=0
  if [[ $# -gt 1 ]]; then usage_error "too many arguments"; fi
  if [[ $# -eq 1 ]]; then
    if [[ "$1" == "$allowed" ]]; then flag=1; else usage_error "unknown argument: $1"; fi
  fi
}

cmd="${1:-}"
if [[ $# -gt 0 ]]; then shift; fi
case "$cmd" in
  check)
    parse_flag --png "$@"
    png=$flag
    manifest_of "$root" "" "$tmp/candidate.txt"
    compare_manifests "$baseline_file" "$tmp/candidate.txt" "$png" "baseline" "candidate"
    ;;
  compare)
    if [[ $# -lt 1 || -z "$1" ]]; then usage_error "compare needs a git revision"; fi
    rev="$1"
    shift
    # PNG is compared unless --no-png is given; --png states the default explicitly.
    png=1
    if [[ $# -gt 1 ]]; then usage_error "too many arguments"; fi
    case "${1:-}" in
      "" | --png) ;;
      --no-png) png=0 ;;
      *) usage_error "unknown argument: $1" ;;
    esac
    extra_worktree="$tmp/baseline-tree"
    git -C "$root" worktree add --detach "$extra_worktree" "$rev" >&2
    for f in "${driver_files[@]}"; do
      mkdir -p "$extra_worktree/$(dirname "$f")"
      cp "$root/$f" "$extra_worktree/$f"
    done
    # The corpus file includes a fixture by relative path; the baseline revision has it too.
    manifest_of "$extra_worktree" "$tmp/target-baseline" "$tmp/baseline.txt"
    manifest_of "$root" "" "$tmp/candidate.txt"
    compare_manifests "$tmp/baseline.txt" "$tmp/candidate.txt" "$png" "$rev" "working tree"
    ;;
  manifest)
    if [[ $# -ne 1 || -z "$1" ]]; then usage_error "manifest needs exactly one output path"; fi
    out="$1"
    manifest_of "$root" "" "$tmp/m.txt"
    cp "$tmp/m.txt" "$out"
    echo "wrote $out"
    ;;
  update)
    if [[ $# -ne 0 ]]; then usage_error "update takes no arguments"; fi
    manifest_of "$root" "" "$tmp/m.txt"
    mkdir -p "$(dirname "$root/$baseline")"
    cp "$tmp/m.txt" "$root/$baseline"
    echo "wrote $baseline"
    ;;
  *)
    usage_error "unknown or missing command: ${cmd:-<none>}"
    ;;
esac
