#!/usr/bin/env bash
# Fail unless the combined line coverage of the given LCOV reports reaches the minimum percentage.
# Usage: check_gate.sh MIN_PERCENT REPORT.lcov...
set -euo pipefail
minimum="$1"
shift
awk -v minimum="$minimum" '
    FNR == 1 { file = FILENAME }
    /^LF:/ { found[file] += substr($0, 4); total_found += substr($0, 4) }
    /^LH:/ { hit[file] += substr($0, 4); total_hit += substr($0, 4) }
    END {
        for (name in found) printf "%s: %d/%d lines (%.2f %%)\n", name, hit[name], found[name], 100 * hit[name] / found[name]
        if (total_found == 0) { print "no line records found"; exit 1 }
        percent = 100 * total_hit / total_found
        printf "combined: %d/%d lines (%.2f %%), minimum %s %%\n", total_hit, total_found, percent, minimum
        exit (percent + 1e-9 < minimum)
    }' "$@"
