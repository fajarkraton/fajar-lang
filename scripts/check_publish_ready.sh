#!/usr/bin/env bash
# check_publish_ready.sh — Detect crates.io publish blockers in
# Cargo.toml.
#
# P7.F3 prevention layer per CLAUDE.md §6.8 R3.
#
# Exits 0 if `cargo publish --dry-run` would likely succeed; non-zero
# with a list of blockers otherwise.
#
# Usage:
#   bash scripts/check_publish_ready.sh
#
# Output: human-readable blocker list + count.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO="$ROOT/Cargo.toml"

if [ ! -f "$CARGO" ]; then
    echo "FAIL: Cargo.toml not found at $CARGO"
    exit 2
fi

BLOCKERS=()

# 1. git deps in [dependencies] / [dev-dependencies].
# Match lines like `name = { git = "..." }` outside comment context.
GIT_DEPS=$(grep -nE '^[a-zA-Z_-]+\s*=\s*\{[^}]*\bgit\s*=' "$CARGO" || true)
if [ -n "$GIT_DEPS" ]; then
    BLOCKERS+=("git deps (crates.io rejects these):")
    while IFS= read -r line; do
        BLOCKERS+=("    $line")
    done <<< "$GIT_DEPS"
fi

# 2. path deps in [dependencies] / [dev-dependencies] (not [patch.*]).
PATH_DEPS=$(awk '
    /^\[dependencies\]/         { in_deps = 1; in_patch = 0; next }
    /^\[dev-dependencies\]/     { in_deps = 1; in_patch = 0; next }
    /^\[patch\./               { in_patch = 1; in_deps = 0; next }
    /^\[/                       { in_deps = 0; in_patch = 0; next }
    in_deps && /^[a-zA-Z_-]+[[:space:]]*=[[:space:]]*\{[^}]*\bpath[[:space:]]*=/ {
        print NR": "$0
    }
' "$CARGO" || true)
if [ -n "$PATH_DEPS" ]; then
    BLOCKERS+=("path deps in [dependencies] (rewrite for publish):")
    while IFS= read -r line; do
        BLOCKERS+=("    $line")
    done <<< "$PATH_DEPS"
fi

# 3. [patch.crates-io] blocks (allowed but not honored downstream).
if grep -qE '^\[patch\.crates-io\]' "$CARGO"; then
    BLOCKERS+=("[patch.crates-io] block present:")
    BLOCKERS+=("    line $(grep -nE '^\[patch\.crates-io\]' "$CARGO")")
    BLOCKERS+=("    (allowed by cargo publish but downstream consumers")
    BLOCKERS+=("     will pull upstream — see docs/CRATES_IO_PUBLISH_PLAN.md)")
fi

# 4. required metadata fields.
REQUIRED_FIELDS=(name version edition description license)
for f in "${REQUIRED_FIELDS[@]}"; do
    if ! grep -qE "^${f}[[:space:]]*=" "$CARGO"; then
        BLOCKERS+=("missing Cargo.toml field: $f")
    fi
done

# 5. recommended metadata.
RECOMMENDED_FIELDS=(repository readme keywords categories)
MISSING_RECOMMENDED=()
for f in "${RECOMMENDED_FIELDS[@]}"; do
    if ! grep -qE "^${f}[[:space:]]*=" "$CARGO"; then
        MISSING_RECOMMENDED+=("$f")
    fi
done

echo "Cargo.toml publish-readiness check:"
echo "----"
if [ "${#BLOCKERS[@]}" -eq 0 ]; then
    echo "PASS — no blockers detected"
else
    echo "FAIL — ${#BLOCKERS[@]} blocker line(s):"
    for b in "${BLOCKERS[@]}"; do
        echo "  $b"
    done
fi

if [ "${#MISSING_RECOMMENDED[@]}" -gt 0 ]; then
    echo
    echo "Recommended metadata missing (won't block publish):"
    for f in "${MISSING_RECOMMENDED[@]}"; do
        echo "  - $f"
    done
fi

echo "----"

# Exit code = blocker count (capped at 1 for non-zero).
if [ "${#BLOCKERS[@]}" -eq 0 ]; then
    exit 0
else
    exit 1
fi
