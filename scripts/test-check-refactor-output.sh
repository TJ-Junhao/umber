#!/usr/bin/env bash
# Asserts the command-line contract of check-refactor-output.sh: every malformed command line
# prints usage and exits 2 without building anything. Works with macOS Bash 3.2.

set -u

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check-refactor-output.sh"
failures=0

# expect_usage_error <description> <args...>
expect_usage_error() {
  local what="$1" status
  shift
  "$script" "$@" >/dev/null 2>&1
  status=$?
  if [[ "$status" -eq 2 ]]; then
    echo "ok   $what"
  else
    echo "FAIL $what: expected exit 2, got $status" >&2
    failures=$((failures + 1))
  fi
}

expect_usage_error "no command"
expect_usage_error "unknown command" frobnicate
expect_usage_error "check with a misspelled flag" check --pgn
expect_usage_error "check with an extra argument" check --png extra
expect_usage_error "check with --no-png" check --no-png
expect_usage_error "compare without a revision" compare
expect_usage_error "compare with an empty revision" compare ""
expect_usage_error "compare with a misspelled flag" compare HEAD --nopng
expect_usage_error "compare with an extra argument" compare HEAD --no-png extra
expect_usage_error "compare with both flags" compare HEAD --png --no-png
expect_usage_error "manifest without a path" manifest
expect_usage_error "manifest with two paths" manifest a b
expect_usage_error "update with an argument" update now

if [[ "$failures" -ne 0 ]]; then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "all command-line checks passed"
