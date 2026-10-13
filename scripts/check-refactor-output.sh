#!/usr/bin/env bash
# Exact-output gate for refactors: proves a change did not alter what umber-core produces.
#
# The corpus lives in crates/umber-core/tests/support/refactor_cases.rs and is rendered by
# crates/umber-core/examples/refactor_outputs.rs. For every case the gate hashes
#   scene/   the Scene in a fixed-schema serialization with floats as to_bits(), so equal text
#            means equal bits and NaN payloads are distinguished
#   svg/     the SVG bytes, in both text modes
#   png/     the encoded PNG bytes at 72 and 200 dpi, or the error text
#   errors/  category, context, message and Display text of errors from public APIs, plus the
#            state of the object after a failed call
#   api/     theme getters, public constants and text-layout metrics
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
#   scripts/check-refactor-output.sh extend <git-rev>
#       The check for changes that add behaviour. It compares the committed manifest at
#       <git-rev> with this tree's committed manifest, PNG lines included, and builds and
#       renders nothing, so it holds on any machine. Every entry of the base must still be
#       present and unchanged, except for the cases named in
#       crates/umber-core/tests/refactor/intended-changes.txt. Entries that only exist in this
#       tree are added outputs and are listed. A changed or removed entry whose case is not
#       named in the file fails, and so does a named case that did not change, so the list
#       cannot go stale. A manifest that lists one path twice fails.
#       The case of a scene, SVG or PNG output is its figure name: the file name without
#       the one suffix of its kind (.txt, .editable.svg or .outlines.svg, .dpiN.png), so one
#       name covers all of a figure's files. Any other output is its own case, named by its
#       kind and file without .txt: errors/api.txt is the case "errors/api" and
#       api/constants.txt is "api/constants".
#   scripts/check-refactor-output.sh manifest <file>
#       Write the manifest of this tree to <file>.
#   scripts/check-refactor-output.sh update
#       Overwrite the committed baseline with this tree's manifest. Only do this from a
#       revision whose output is known to be right, and say so in the commit message.
#
# Procedure for a pull request that adds a feature:
#   1. Do not label it `refactor`: `compare` builds this tree's corpus against the base revision,
#      and the base cannot compile a corpus that calls APIs it does not have.
#   2. Put new cases into new output files: new scene, SVG and PNG files, a new
#      errors/<area>.txt, or a new api/constants-<area>.txt. Never append to errors/api.txt,
#      api/themes.txt or api/constants.txt, because that changes an existing hash. Each feature
#      edits only its own module under crates/umber-core/tests/support/corpus/. Recorders may
#      gain match arms for new marks; existing record formats do not change.
#   3. Regenerate the manifest with `update` on the platform of the committed baseline (macOS
#      arm64), so that its PNG lines stay meaningful, and commit it.
#   4. List the cases whose outputs change on purpose in intended-changes.txt, one name per
#      line (blank lines and # comments are ignored), and replace the file's contents in every
#      pull request. Everything else must stay byte-identical.
#   5. Run `extend <base-rev>` locally. CI runs it on every pull request, and `check` proves
#      that the committed manifest matches the tree.
#   6. Regenerate golden images only for the images the change declares.
#
# Naming rule: an output is <kind>/<file> with kind one of scene, svg, png, errors or api and no
# deeper directories. A file name starts with an ASCII letter or digit and continues with ASCII
# letters, digits, '.', '_' or '-'; so it has no '/', no whitespace and no '#', and nothing
# outside ASCII. The driver refuses to write any other name, `manifest` and `extend` fail on
# one, and intended-changes.txt can therefore name every case.
#
# Conditions: the same Rust toolchain, Cargo.lock and bundled font files on both sides; the
# corpus uses the in-memory fallback font and never reads system fonts. Everything is built
# in release mode with UPDATE_GOLDEN and INSTA_UPDATE unset.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
baseline="crates/umber-core/tests/refactor/baseline-manifest.txt"
# UMBER_REFACTOR_BASELINE (an absolute path) replaces the committed baseline in `check`.
baseline_file="${UMBER_REFACTOR_BASELINE:-$root/$baseline}"
intended="crates/umber-core/tests/refactor/intended-changes.txt"
driver_files=(
  crates/umber-core/examples/refactor_outputs.rs
  crates/umber-core/tests/support/refactor_cases.rs
  crates/umber-core/tests/support/corpus/area.rs
  crates/umber-core/tests/support/corpus/bars.rs
  crates/umber-core/tests/support/corpus/grid.rs
  crates/umber-core/tests/support/corpus/hist.rs
  crates/umber-core/tests/support/corpus/inputs.rs
  crates/umber-core/tests/support/corpus/legend.rs
  crates/umber-core/tests/support/corpus/plot_height.rs
  crates/umber-core/tests/support/corpus/size.rs
  crates/umber-core/tests/support/corpus/ticks.rs
  crates/umber-core/tests/support/corpus/x_end_label.rs
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

# The naming rule of the header, as an extended regular expression.
path_rule='^(scene|svg|png|errors|api)/[A-Za-z0-9][A-Za-z0-9._-]*$'

# reject_bad_paths <what>: reads paths on stdin and exits 1, naming each one, if any breaks the rule.
reject_bad_paths() {
  local bad
  bad="$(LC_ALL=C awk -v rule="$path_rule" '$0 !~ rule { print }' | LC_ALL=C cat -v)"
  if [[ -n "$bad" ]]; then
    echo "error: $1 break the output naming rule (see the script header):" >&2
    awk '{ print "  " $0 }' <<<"$bad" >&2
    exit 1
  fi
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
    # Both checks read the whole listing, so no early exit of a reader can skip them. The first
    # catches any byte outside the allowed set, including newlines and non-ASCII, in the C locale.
    bad="$(LC_ALL=C find . -type f -path '*[!A-Za-z0-9._/-]*' | LC_ALL=C cat -v)"
    if [[ -n "$bad" ]]; then
      echo "error: an output path has a character outside the naming rule:" >&2
      awk '{ print "  " $0 }' <<<"$bad" >&2
      exit 1
    fi
    find . -type f | sed 's|^\./||' | reject_bad_paths "the generated outputs"
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
  LC_ALL=C grep -v '^#' "$file" | LC_ALL=C sed 's/  */ /' | if [[ "$png" == 1 ]]; then cat; else grep -v ' png/'; fi
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

# extend_check <git-rev>: see the header.
extend_check() {
  local rev="$1" intended_file="$root/$intended"
  if ! git -C "$root" show "$rev:$baseline" >"$tmp/base-manifest.txt" 2>"$tmp/git.err"; then
    echo "error: cannot read $baseline at $rev" >&2
    cat "$tmp/git.err" >&2
    exit 1
  fi
  if [[ ! -f "$intended_file" ]]; then intended_file=/dev/null; fi
  select_lines "$tmp/base-manifest.txt" 1 >"$tmp/a.sel"
  select_lines "$root/$baseline" 1 >"$tmp/b.sel"
  local sel
  for sel in a b; do
    LC_ALL=C awk '{ path = $0; sub(/^[^ ]+ +/, "", path); if (path == $0) path = ""; print path }' \
      "$tmp/$sel.sel" | reject_bad_paths "the $([[ $sel == a ]] && echo "manifest at $rev" || echo "committed manifest")"
  done
  # The awk program is single-quoted on purpose: nothing in it is a shell expansion.
  # shellcheck disable=SC2016
  awk -v intended="$intended_file" '
    function chop(text, suffix) {
      if (length(text) > length(suffix) && substr(text, length(text) - length(suffix) + 1) == suffix) {
        return substr(text, 1, length(text) - length(suffix))
      }
      return ""
    }
    # The case of an output: a figure name for scene, svg and png files, kind/stem otherwise.
    function casename(path,    slash, kind, file, stem) {
      slash = index(path, "/")
      kind = substr(path, 1, slash - 1)
      file = substr(path, slash + 1)
      stem = ""
      if (kind == "scene") {
        stem = chop(file, ".txt")
      } else if (kind == "svg") {
        stem = chop(file, ".editable.svg")
        if (stem == "") stem = chop(file, ".outlines.svg")
      } else if (kind == "png") {
        if (match(file, /\.dpi[0-9]+\.png$/)) stem = substr(file, 1, RSTART - 1)
      } else {
        stem = chop(file, ".txt")
        if (stem != "") return kind "/" stem
      }
      return stem != "" ? stem : path
    }
    BEGIN {
      while ((getline line < intended) > 0) {
        sub(/#.*/, "", line)
        sub(/^[ \t]+/, "", line)
        sub(/[ \t\r]+$/, "", line)
        if (line != "") listed[line] = 1
      }
    }
    # A manifest line is "<hash> <path>"; the path is everything after the first space.
    {
      hash = $1
      path = $0
      sub(/^[^ ]+ +/, "", path)
      if (FNR == NR) {
        if (path in base) print "duplicate\t" path "\t-"
        base[path] = hash
      } else {
        if (path in tree) print "duplicate\t" path "\t-"
        tree[path] = hash
      }
    }
    END {
      for (path in base) {
        name = casename(path)
        if (!(path in tree)) kind = "removed"
        else if (tree[path] != base[path]) kind = "changed"
        else continue
        touched[name] = 1
        print kind "\t" path "\t" (name in listed ? "intended" : "UNLISTED")
      }
      for (path in tree) if (!(path in base)) print "added\t" path "\t-"
      for (name in listed) if (!(name in touched)) print "stale\t" name "\t-"
    }
  ' "$tmp/a.sel" "$tmp/b.sel" | LC_ALL=C sort >"$tmp/extend.out"
  local added changed removed unlisted stale duplicate summary
  added="$(grep -c '^added	' "$tmp/extend.out" || true)"
  changed="$(grep -c '^changed	' "$tmp/extend.out" || true)"
  removed="$(grep -c '^removed	' "$tmp/extend.out" || true)"
  unlisted="$(grep -c '	UNLISTED$' "$tmp/extend.out" || true)"
  stale="$(grep -c '^stale	' "$tmp/extend.out" || true)"
  duplicate="$(grep -c '^duplicate	' "$tmp/extend.out" || true)"
  local kind path note
  while IFS='	' read -r kind path note; do
    case "$kind" in
      added) echo "  added    $path" ;;
      changed | removed) echo "  $kind $path ($note)" ;;
      stale) echo "  listed but unchanged: $path" ;;
      duplicate) echo "  listed twice in a manifest: $path" ;;
    esac
  done <"$tmp/extend.out"
  summary="$added added, $changed changed, $removed removed"
  if [[ "$unlisted" -ne 0 || "$stale" -ne 0 || "$duplicate" -ne 0 ]]; then
    echo "refactor output gate (extend): FAIL ($summary; $unlisted unlisted, $stale stale, $duplicate duplicate)" >&2
    echo "A changed or removed output needs its case in $intended, and a listed case has to change." >&2
    exit 1
  fi
  echo "refactor output gate (extend): PASS ($summary)"
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
    # An absent flag is told from an empty argument by the argument count.
    if [[ $# -gt 1 ]]; then usage_error "too many arguments"; fi
    if [[ $# -eq 1 ]]; then
      case "$1" in
        --png) ;;
        --no-png) png=0 ;;
        *) usage_error "unknown argument: '$1'" ;;
      esac
    fi
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
  extend)
    if [[ $# -lt 1 || -z "$1" ]]; then usage_error "extend needs a git revision"; fi
    rev="$1"
    shift
    if [[ $# -ne 0 ]]; then usage_error "extend takes only a revision"; fi
    extend_check "$rev"
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
