#!/bin/bash
# Fuzz smoke test — runs each fuzzer for a short duration
# to catch obvious panics. Suitable for CI (60s total).
#
# Prerequisites: cargo-fuzz (`cargo install cargo-fuzz`)
#
# Usage: bash tools/fuzz_smoke.sh [SECONDS_PER_TARGET]

set -e

SECONDS_PER_TARGET="${1:-15}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR/.."

echo "═══════════════════════════════════════════"
echo "  Fajar Lang Fuzz Smoke Test"
echo "  ${SECONDS_PER_TARGET}s per target"
echo "═══════════════════════════════════════════"

# Check cargo-fuzz is installed
if ! cargo fuzz --help &>/dev/null 2>&1; then
    echo "⚠️  cargo-fuzz not installed. Install with:"
    echo "   cargo install cargo-fuzz"
    echo ""
    echo "Skipping fuzz test (non-fatal)."
    exit 0
fi

cd "$PROJECT_DIR"

TARGETS=("fuzz_lexer" "fuzz_parser" "fuzz_analyzer" "fuzz_interpreter")
PASS=0
FAIL=0

for target in "${TARGETS[@]}"; do
    echo ""
    echo "--- Fuzzing: $target (${SECONDS_PER_TARGET}s) ---"
    if timeout $((SECONDS_PER_TARGET + 5)) \
        cargo fuzz run "$target" -- \
        -max_total_time="$SECONDS_PER_TARGET" \
        -max_len=1024 \
        2>&1 | tail -3; then
        echo "  ✅ $target: no panics found"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $target: PANIC or ERROR"
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "═══════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════════"

[ "$FAIL" -gt 0 ] && echo "❌ FUZZ SMOKE FAILED" && exit 1
echo "✅ FUZZ SMOKE PASSED" && exit 0
