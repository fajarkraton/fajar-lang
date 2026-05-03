#!/usr/bin/env bash
# check_doc_coverage.sh — Measure pub-item doc coverage.
#
# P6.E4 prevention layer per CLAUDE.md §6.8 R3.
#
# Reports:
#   - missing_docs warnings count (lint -W missing_docs)
#   - approximate total pub items
#   - coverage ratio
#
# Exit codes:
#   0 — coverage ≥ COVERAGE_THRESHOLD (default 95)
#   1 — coverage below threshold
#   2 — measurement failed
#
# Usage:
#   bash scripts/check_doc_coverage.sh                # threshold 95
#   COVERAGE_THRESHOLD=90 bash scripts/check_doc_coverage.sh

set -euo pipefail

THRESHOLD="${COVERAGE_THRESHOLD:-95}"

# Count missing-docs warnings.
MISSING=$(cargo rustdoc --lib -- -W missing_docs 2>&1 \
    | grep -cE "warning: missing documentation" || true)

# Count pub declarations that need docs (fn/struct/enum/trait/const/static/type/union).
PUB_DECLS=$(grep -rohE "^\s*pub\s+(fn|struct|enum|trait|const|static|type|union|async fn|unsafe fn)\s+\w+" src/ 2>/dev/null | wc -l)

# Count pub fields in pub structs/enums (rough approximation; over-counts
# nested pub fields in private structs but under-counts variant data).
PUB_FIELDS=$(grep -rohE "^\s+pub\s+\w+\s*:" src/ 2>/dev/null | wc -l)

TOTAL_PUB=$((PUB_DECLS + PUB_FIELDS))
DOCUMENTED=$((TOTAL_PUB - MISSING))

if [ "$TOTAL_PUB" -eq 0 ]; then
    echo "FAIL: could not count pub items"
    exit 2
fi

# Compute percentage with bc for fractional precision.
COVERAGE=$(awk -v doc="$DOCUMENTED" -v tot="$TOTAL_PUB" 'BEGIN {printf "%.2f", (doc/tot)*100}')

echo "Pub declarations: $PUB_DECLS"
echo "Pub fields:       $PUB_FIELDS"
echo "Total pub:        $TOTAL_PUB"
echo "Missing docs:     $MISSING"
echo "Documented:       $DOCUMENTED"
echo "Coverage:         $COVERAGE% (threshold: $THRESHOLD%)"

# bash arithmetic comparison via awk.
PASS=$(awk -v cov="$COVERAGE" -v thr="$THRESHOLD" 'BEGIN {print (cov >= thr) ? 1 : 0}')

if [ "$PASS" = "1" ]; then
    echo "PASS"
    exit 0
else
    echo "FAIL: coverage $COVERAGE% < threshold $THRESHOLD%"
    exit 1
fi
