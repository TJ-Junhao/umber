#!/usr/bin/env bash
# Asserts the command-line contract of check-refactor-output.sh: every malformed command line
# prints usage and exits 2 without building anything, and `extend` classifies manifest changes
# as documented. Works with macOS Bash 3.2.

set -u

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check-refactor-output.sh"
failures=0

# Every corpus module must travel with the driver when comparing against another revision.
root="$(dirname "$(dirname "$script")")"
driver_files=()
in_driver=0
while IFS= read -r line; do
  if [[ "$line" == 'driver_files=(' ]]; then
    in_driver=1
  elif [[ "$in_driver" -eq 1 && "$line" == ')' ]]; then
    in_driver=0
  elif [[ "$in_driver" -eq 1 ]]; then
    driver_files+=("${line// /}")
  fi
done <"$script"
in_corpus=0
while IFS= read -r line; do
  if [[ "$line" == 'mod corpus {' ]]; then
    in_corpus=1
  elif [[ "$in_corpus" -eq 1 && "$line" == '}' ]]; then
    in_corpus=0
  elif [[ "$in_corpus" -eq 1 && "$line" =~ ^[[:space:]]*pub[[:space:]]+mod[[:space:]]+([a-zA-Z_][a-zA-Z_0-9]*)\; ]]; then
    module="${BASH_REMATCH[1]}"
    path="crates/umber-core/tests/support/corpus/$module.rs"
    found=0
    for file in "${driver_files[@]}"; do
      if [[ "$file" == "$path" ]]; then found=1; fi
    done
    if [[ "$found" -eq 0 ]]; then
      printf 'FAIL corpus module missing from driver_files: %s\n' "$path" >&2
      failures=$((failures + 1))
    else
      printf 'ok   corpus module %s\n' "$module"
    fi
  fi
done <"$root/crates/umber-core/tests/support/refactor_cases.rs"
# The driver list can be checked without constructing the command-line fixtures below.
if [[ "${1:-}" == --drivers-only ]]; then exit "$failures"; fi

# Stand-ins for cargo and git that leave a marker, so a case that starts a build or creates a
# worktree is caught even when it would also exit 2.
shims="$(mktemp -d "${TMPDIR:-/tmp}/umber-gate-cli.XXXXXX")"
trap 'rm -rf "$shims"' EXIT
marker="$shims/ran"
for tool in cargo git; do
  printf '#!/bin/sh\necho "%s $*" >>"%s"\nexit 99\n' "$tool" "$marker" >"$shims/$tool"
  chmod +x "$shims/$tool"
done

# expect_usage_error <description> <args...>
expect_usage_error() {
  local what="$1" status
  shift
  rm -f "$marker"
  PATH="$shims:$PATH" "$script" "$@" >/dev/null 2>&1
  status=$?
  if [[ "$status" -ne 2 ]]; then
    echo "FAIL $what: expected exit 2, got $status" >&2
    failures=$((failures + 1))
  elif [[ -e "$marker" ]]; then
    echo "FAIL $what: ran $(tr '\n' ';' <"$marker")" >&2
    failures=$((failures + 1))
  else
    echo "ok   $what"
  fi
}

expect_usage_error "no command"
expect_usage_error "unknown command" frobnicate
expect_usage_error "check with a misspelled flag" check --pgn
expect_usage_error "check with an extra argument" check --png extra
expect_usage_error "check with --no-png" check --no-png
expect_usage_error "check with an empty flag" check ""
expect_usage_error "compare without a revision" compare
expect_usage_error "compare with an empty revision" compare ""
expect_usage_error "compare with an empty flag" compare HEAD ""
expect_usage_error "compare with a misspelled flag" compare HEAD --nopng
expect_usage_error "compare with an extra argument" compare HEAD --no-png extra
expect_usage_error "compare with both flags" compare HEAD --png --no-png
expect_usage_error "manifest without a path" manifest
expect_usage_error "manifest with two paths" manifest a b
expect_usage_error "update with an argument" update now
expect_usage_error "extend without a revision" extend
expect_usage_error "extend with an empty revision" extend ""
expect_usage_error "extend with a misspelled flag" extend HEAD --pgn
expect_usage_error "extend with --png" extend HEAD --png
expect_usage_error "extend with --no-png" extend HEAD --no-png
expect_usage_error "extend with an extra argument" extend HEAD extra

# `extend` reads two manifests and builds nothing, so it runs against a throwaway repository whose
# first commit holds the base manifest.
manifest_rel="crates/umber-core/tests/refactor/baseline-manifest.txt"
intended_rel="crates/umber-core/tests/refactor/intended-changes.txt"
repo="$(mktemp -d "${TMPDIR:-/tmp}/umber-gate-extend.XXXXXX")"
trap 'rm -rf "$shims" "$repo"' EXIT
mkdir -p "$repo/scripts" "$repo/$(dirname "$manifest_rel")"
cp "$script" "$repo/scripts/check-refactor-output.sh"
base_manifest() {
  printf '# header\n'
  printf '%s  scene/fig-a.txt\n' aaa1
  printf '%s  svg/fig-a.editable.svg\n' aaa2
  printf '%s  png/fig-a.dpi72.png\n' aaa3
  printf '%s  scene/fig-b.txt\n' bbb1
  printf '%s  errors/api.txt\n' eee1
  printf '%s  scene/api.txt\n' sss1
  printf '%s  api/constants.txt\n' ccc1
  printf '%s  scene/constants.txt\n' ccc2
  printf '%s  scene/plot.txt.txt\n' ppp1
  printf '%s  svg/plot.txt.editable.svg\n' ppp2
  printf '%s  png/plot.txt.dpi72.png\n' ppp3
  printf '%s  scene/plot.txt\n' ppp4
  printf '%s  png/x.dpi72.png.dpi200.png\n' xxx1
  printf '%s  png/x.dpi72.png\n' xxx2
}
base_manifest >"$repo/$manifest_rel"
: >"$repo/$intended_rel"
git -C "$repo" init -q
git -C "$repo" add -A
git -C "$repo" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q -m base

# expect_extend <description> <expected-status> <expected-output-fragment> <manifest-sed> <intended>
# Rewrites the tree's manifest with the sed script, writes the intended list, runs
# `extend HEAD`, and checks the exit status and a fragment of the output.
expect_extend() {
  local what="$1" want="$2" fragment="$3" edit="$4" listed="$5" status out
  base_manifest | sed -e "$edit" >"$repo/$manifest_rel"
  printf '%b' "$listed" >"$repo/$intended_rel"
  out="$("$repo/scripts/check-refactor-output.sh" extend HEAD 2>&1)"
  status=$?
  if [[ "$status" -ne "$want" ]]; then
    echo "FAIL $what: expected exit $want, got $status" >&2
    echo "$out" >&2
    failures=$((failures + 1))
  elif ! printf '%s\n' "$out" | grep -q -F -- "$fragment"; then
    echo "FAIL $what: output lacks '$fragment'" >&2
    echo "$out" >&2
    failures=$((failures + 1))
  else
    echo "ok   $what"
  fi
}

# These are sed append commands for the last line, not shell expansions.
# shellcheck disable=SC2016
add_line='$a\
ddd1  scene/fig-c.txt'
# shellcheck disable=SC2016
add_png='$a\
ddd3  png/fig-c.dpi72.png'
# shellcheck disable=SC2016
dup_line='$a\
zzz1  scene/fig-b.txt'
expect_extend "no change" 0 "0 added, 0 changed, 0 removed" 's/^$//' ""
expect_extend "pure addition" 0 "1 added, 0 changed, 0 removed" "$add_line" ""
expect_extend "added png" 0 "1 added" "$add_png" ""
expect_extend "unlisted change" 1 "fig-b.txt (UNLISTED)" 's/^bbb1/bbb9/' ""
expect_extend "listed change" 0 "1 changed" 's/^bbb1/bbb9/' "fig-b\n"
expect_extend "listed change with comments and blanks" 0 "1 changed" 's/^bbb1/bbb9/' "# why\n\n  fig-b  # trailing\n"
expect_extend "stale entry" 1 "listed but unchanged: fig-b" 's/^$//' "fig-b\n"
expect_extend "stale entry next to a real change" 1 "listed but unchanged: fig-z" 's/^bbb1/bbb9/' "fig-b\nfig-z\n"
expect_extend "unlisted removal" 1 "removed errors/api.txt (UNLISTED)" '/^eee1/d' ""
expect_extend "listed removal" 0 "1 removed" '/^eee1/d' "errors/api\n"
expect_extend "one figure name covers scene, svg and png" 0 "3 changed" 's/^aaa[123]/zzz/' "fig-a\n"
expect_extend "png change is compared" 1 "png/fig-a.dpi72.png (UNLISTED)" 's/^aaa3/zzz/' ""
expect_extend "listed png change passes" 0 "1 changed" 's/^aaa3/zzz/' "fig-a\n"

expect_extend "duplicate path in the tree manifest" 1 "listed twice in a manifest: scene/fig-b.txt" "$dup_line" ""

# A standalone output and a figure of the same name are different cases.
expect_extend "errors/api is not the figure api" 1 "errors/api.txt (UNLISTED)" 's/^eee1/eee9/' "api\n"
expect_extend "the figure api does not authorize errors/api" 1 "listed but unchanged: api" 's/^eee1/eee9/' "api\n"
expect_extend "errors/api authorizes only itself" 0 "1 changed" 's/^eee1/eee9/' "errors/api\n"
expect_extend "the figure api is not errors/api" 1 "listed but unchanged: errors/api" 's/^sss1/sss9/' "errors/api\n"
expect_extend "the figure api authorizes only itself" 0 "1 changed" 's/^sss1/sss9/' "api\n"
expect_extend "api/constants is not the figure constants" 1 "api/constants.txt (UNLISTED)" 's/^ccc1/ccc9/' "constants\n"
expect_extend "api/constants authorizes only itself" 0 "1 changed" 's/^ccc1/ccc9/' "api/constants\n"
expect_extend "the figure constants authorizes only itself" 1 "listed but unchanged: api/constants" 's/^ccc2/ccc9/' "api/constants\n"

# Exactly one suffix is removed, the one of the output's kind.
expect_extend "figure named plot.txt covers its kinds" 0 "3 changed" 's/^ppp[123]/zzz/' "plot.txt\n"
expect_extend "figure named plot.txt is not plot" 1 "scene/plot.txt.txt (UNLISTED)" 's/^ppp1/zzz/' "plot\n"
expect_extend "figure plot.txt leaves figure plot alone" 1 "listed but unchanged: plot.txt" 's/^ppp4/zzz/' "plot.txt\n"
expect_extend "png figure named x.dpi72.png" 0 "1 changed" 's/^xxx1/zzz/' "x.dpi72.png\n"
expect_extend "png figure x is a different case" 0 "1 changed" 's/^xxx2/zzz/' "x\n"

# Every path of both manifests follows the naming rule: a known kind, one level, and a file name
# of ASCII letters, digits, '.', '_' and '-'.
nbsp="$(printf '\302\240')"
bad_line() {
  # shellcheck disable=SC2016
  printf '$a\\\nbad1  %s\n' "$1"
}
expect_extend "nested figure path" 1 "scene/errors/api.txt" "$(bad_line 'scene/errors/api.txt')" ""
expect_extend "unknown kind" 1 "other/a.txt" "$(bad_line 'other/a.txt')" ""
expect_extend "no kind directory" 1 "a.txt" "$(bad_line 'a.txt')" ""
expect_extend "hash in a name" 1 "scene/a#b.txt" "$(bad_line 'scene/a#b.txt')" ""
expect_extend "space in a name" 1 "scene/a b.txt" "$(bad_line 'scene/a b.txt')" ""
expect_extend "tab in a name" 1 "scene/a" "$(bad_line "$(printf 'scene/a\tb.txt')")" ""
expect_extend "no-break space in a name" 1 "scene/aM-BM- b.txt" "$(bad_line "scene/a${nbsp}b.txt")" ""
expect_extend "non-ASCII letter in a name" 1 "scene/M-CM-)" "$(bad_line "scene/$(printf '\303\251').txt")" ""
expect_extend "leading dot in a name" 1 "scene/.a.txt" "$(bad_line 'scene/.a.txt')" ""
expect_extend "a valid new name" 0 "1 added" "$(bad_line 'scene/a_b-c.1.txt')" ""

# The base manifest is held to the rule too.
repo_bad="$(mktemp -d "${TMPDIR:-/tmp}/umber-gate-extend.XXXXXX")"
trap 'rm -rf "$shims" "$repo" "$repo_bad" "$cshims" "$work"' EXIT
mkdir -p "$repo_bad/scripts" "$repo_bad/$(dirname "$manifest_rel")"
cp "$script" "$repo_bad/scripts/check-refactor-output.sh"
{ base_manifest; printf 'bad1  scene/a#b.txt\n'; } >"$repo_bad/$manifest_rel"
git -C "$repo_bad" init -q
git -C "$repo_bad" add -A
git -C "$repo_bad" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q -m base
out="$("$repo_bad/scripts/check-refactor-output.sh" extend HEAD 2>&1)"
status=$?
if [[ "$status" -ne 1 ]] || ! printf '%s\n' "$out" | grep -q -F "scene/a#b.txt"; then
  echo "FAIL bad path in the base manifest: exit $status" >&2
  echo "$out" >&2
  failures=$((failures + 1))
else
  echo "ok   bad path in the base manifest"
fi

# `manifest` validates the generated files. A stand-in cargo writes the files named in
# UMBER_TEST_FILES (one per line, relative to the output directory) into the output directory.
cshims="$(mktemp -d "${TMPDIR:-/tmp}/umber-gate-cargo.XXXXXX")"
work="$(mktemp -d "${TMPDIR:-/tmp}/umber-gate-work.XXXXXX")"
cat >"$cshims/cargo" <<'SHIM'
#!/bin/bash
# The output directory is the last argument.
for dir in "$@"; do :; done
while IFS= read -r f; do
  mkdir -p "$dir/$(dirname "$f")"
  echo x >"$dir/$f"
done <"$UMBER_TEST_FILES"
SHIM
chmod +x "$cshims/cargo"

# expect_manifest <description> <expected-status> <expected-output-fragment> <file-list>
expect_manifest() {
  local what="$1" want="$2" fragment="$3" list="$4" status out
  printf '%s\n' "$list" >"$work/files"
  out="$(UMBER_TEST_FILES="$work/files" PATH="$cshims:$PATH" \
    "$repo/scripts/check-refactor-output.sh" manifest "$work/out.txt" 2>&1)"
  status=$?
  if [[ "$status" -ne "$want" ]]; then
    echo "FAIL $what: expected exit $want, got $status" >&2
    echo "$out" >&2
    failures=$((failures + 1))
  elif ! printf '%s\n' "$out" | grep -q -F -- "$fragment"; then
    echo "FAIL $what: output lacks '$fragment'" >&2
    echo "$out" >&2
    failures=$((failures + 1))
  else
    echo "ok   $what"
  fi
}

many() {
  local i
  for i in $(seq 1 1500); do echo "scene/a-long-and-unremarkable-figure-name-number-$i.txt"; done
}
expect_manifest "generated files that follow the rule" 0 "wrote" "$(many)"
expect_manifest "generated nested path" 1 "scene/errors/api.txt" "scene/errors/api.txt"
expect_manifest "generated unknown kind" 1 "other/a.txt" "other/a.txt"
expect_manifest "generated hash in a name" 1 "scene/a#b.txt" "scene/a#b.txt"
expect_manifest "generated space in a name" 1 "scene/a b.txt" "scene/a b.txt"
expect_manifest "generated no-break space" 1 "scene/aM-BM- b.txt" "scene/a${nbsp}b.txt"
expect_manifest "generated space among 1500 files" 1 "scene/a b.txt" "$(printf 'scene/a b.txt\n%s' "$(many)")"
expect_manifest "generated no-break space among 1500 files" 1 "M-BM-" "$(printf 'scene/a%sb.txt\n%s' "$nbsp" "$(many)")"

# A revision without a manifest is an error, not a pass.
out="$("$repo/scripts/check-refactor-output.sh" extend no-such-rev 2>&1)"
status=$?
if [[ "$status" -ne 1 ]]; then
  echo "FAIL unknown revision: expected exit 1, got $status" >&2
  failures=$((failures + 1))
else
  echo "ok   unknown revision"
fi

if [[ "$failures" -ne 0 ]]; then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "all command-line checks passed"
