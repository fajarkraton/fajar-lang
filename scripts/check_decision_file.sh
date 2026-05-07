#!/usr/bin/env bash
# Fajar Lang — decision-file structure checker.
#
# Verifies a decision file under docs/decisions/ has all 7 sections required
# by CLAUDE.md §6.8 R6 ("Decisions must be committed files that pre-commit
# hooks can mechanically check") and enumerated in
# docs/FJARR_LEAK_PLAN.md §1.3.
#
# Usage:
#   bash scripts/check_decision_file.sh <path-to-decision-file>
#
# Exit codes:
#   0  — all 7 sections present
#   1  — file missing OR ≥1 section missing (diagnostic on stderr)
#   2  — usage error (wrong arg count)
#
# Design notes:
# - Section headers are matched line-anchored (`^## <name>`) so that a section
#   like `## Choice rationale` does NOT satisfy the `## Choice` requirement.
# - "## Rationale" matches "## Rationale (≥3 sentences)" because the canonical
#   header includes a parenthetical hint; we accept any prefix-match at line start.
# - "@kernel-future-compat" needs both forms since markdown renders the @ but
#   some authors might omit. Strict literal match per the canonical convention.

set -e

if [ $# -ne 1 ]; then
    echo "usage: $0 <path-to-decision-file>" >&2
    echo "" >&2
    echo "Verifies a docs/decisions/*.md file has all 7 sections required by" >&2
    echo "CLAUDE.md §6.8 R6 + FJARR_LEAK_PLAN §1.3." >&2
    exit 2
fi

FILE="$1"

if [ ! -f "$FILE" ]; then
    echo "❌ decision file not found: $FILE" >&2
    exit 1
fi

# 7 canonical sections per FJARR_LEAK_PLAN §1.3.
# Each entry is a line-start regex; anchoring prevents false positives from
# inline mentions in prose (e.g. "this revisits the choice from ...").
REQUIRED_PATTERNS=(
    "^## Choice"
    "^## Rationale"
    "^## @kernel-future-compat"
    "^## Migration path"
    "^## Surprise budget"
    "^## Rejected candidates"
    "^## Reverse-cost"
)

REQUIRED_NAMES=(
    "## Choice"
    "## Rationale (≥3 sentences)"
    "## @kernel-future-compat"
    "## Migration path"
    "## Surprise budget"
    "## Rejected candidates"
    "## Reverse-cost"
)

MISSING=()
for i in "${!REQUIRED_PATTERNS[@]}"; do
    pattern="${REQUIRED_PATTERNS[$i]}"
    name="${REQUIRED_NAMES[$i]}"
    if ! grep -qE "$pattern" "$FILE"; then
        MISSING+=("$name")
    fi
done

if [ ${#MISSING[@]} -ne 0 ]; then
    echo "❌ decision file missing required sections: $FILE" >&2
    for s in "${MISSING[@]}"; do
        echo "    - $s" >&2
    done
    echo "" >&2
    echo "Per CLAUDE.md §6.8 R6, every decision file must include all 7" >&2
    echo "sections. Canonical list: docs/FJARR_LEAK_PLAN.md §1.3." >&2
    exit 1
fi

exit 0
