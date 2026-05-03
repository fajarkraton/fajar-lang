#!/usr/bin/env bash
# check_stdlib_docs.sh — Verify every `pub fn` in src/stdlib_v3/ has
# a /// doc comment block.
#
# P6.E2 prevention layer per CLAUDE.md §6.8 R3.
#
# Walks backward through #[cfg(...)] / #[derive(...)] / other attributes
# so doc comments above a cfg-gated fn are still recognized.
#
# Usage:
#   bash scripts/check_stdlib_docs.sh             # threshold 100% (default)
#   STDLIB_DOC_THRESHOLD=95 bash scripts/check_stdlib_docs.sh

set -euo pipefail

THRESHOLD="${STDLIB_DOC_THRESHOLD:-100}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

audit_file() {
    local f="$1"
    awk 'BEGIN{ doc=0; total=0 }
        /^[[:space:]]*pub fn /{
            total++
            # Walk backward through buffered lines looking for /// docs.
            # Skip blank lines and #[...] attribute lines.
            found_doc = 0
            for (i = idx; i > 0; i--) {
                line = buf[i]
                # Skip cfg/derive/other attributes
                if (line ~ /^[[:space:]]*#\[/) continue
                # Skip blank lines
                if (line ~ /^[[:space:]]*$/) continue
                # Found a doc-comment line
                if (line ~ /^[[:space:]]*\/\/\//) {
                    found_doc = 1
                    break
                }
                # Hit a non-doc, non-attr line — stop searching.
                break
            }
            if (found_doc) doc++
        }
        { idx++; buf[idx] = $0 }
        END { printf "%d %d\n", doc, total }
    ' "$f"
}

cd "$ROOT"

TOTAL_DOC=0
TOTAL_FN=0
echo "Per-file coverage:"
echo "----"
for f in src/stdlib_v3/*.rs; do
    [ -f "$f" ] || continue
    name="$(basename "$f")"
    if [ "$name" = "mod.rs" ]; then continue; fi
    read -r doc total < <(audit_file "$f")
    if [ "$total" -eq 0 ]; then continue; fi
    pct=$(awk -v d="$doc" -v t="$total" 'BEGIN { printf "%.1f", (d/t)*100 }')
    printf "%-30s %d/%d (%s%%)\n" "$name" "$doc" "$total" "$pct"
    TOTAL_DOC=$((TOTAL_DOC + doc))
    TOTAL_FN=$((TOTAL_FN + total))
done
echo "----"

if [ "$TOTAL_FN" -eq 0 ]; then
    echo "FAIL: no pub fn found in src/stdlib_v3/"
    exit 2
fi

COVERAGE=$(awk -v d="$TOTAL_DOC" -v t="$TOTAL_FN" 'BEGIN { printf "%.1f", (d/t)*100 }')
echo "Total: $TOTAL_DOC / $TOTAL_FN ($COVERAGE%)"
echo "Threshold: $THRESHOLD%"

PASS=$(awk -v c="$COVERAGE" -v t="$THRESHOLD" 'BEGIN { print (c >= t) ? 1 : 0 }')
if [ "$PASS" = "1" ]; then
    echo "PASS"
    exit 0
else
    echo "FAIL: coverage $COVERAGE% < $THRESHOLD%"
    exit 1
fi
