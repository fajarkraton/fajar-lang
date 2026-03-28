#!/bin/bash
# Fajar Lang Benchmark Suite — Cross-Language Performance Comparison
#
# Runs each benchmark in Fajar Lang (interpreter) and reports timing.
# Optionally compares with Rust, C, and Python equivalents if available.
#
# Usage:
#   ./run_benchmarks.sh              # Run all benchmarks
#   ./run_benchmarks.sh fib_recursive  # Run one benchmark
#   ./run_benchmarks.sh --compare    # Include Rust/C/Python comparison

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
FJ="$PROJECT_DIR/target/release/fj"

# Build release binary if not present
if [ ! -f "$FJ" ]; then
    echo "[bench] Building release binary..."
    (cd "$PROJECT_DIR" && cargo build --release)
fi

BENCHMARKS=(
    fib_recursive
    fib_iterative
    quicksort
    string_concat
    matrix_multiply
    pattern_match
    closure_overhead
    mandelbrot
    nbody
    binary_trees
)

WARMUP=1
ITERATIONS=3

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

run_bench() {
    local name="$1"
    local file="$SCRIPT_DIR/${name}.fj"

    if [ ! -f "$file" ]; then
        echo "  SKIP — $file not found"
        return
    fi

    # Warmup
    for i in $(seq 1 $WARMUP); do
        "$FJ" run "$file" >/dev/null 2>&1 || true
    done

    # Timed runs
    local total_ms=0
    local min_ms=999999
    local max_ms=0

    for i in $(seq 1 $ITERATIONS); do
        local start_ns=$(date +%s%N)
        "$FJ" run "$file" >/dev/null 2>&1 || true
        local end_ns=$(date +%s%N)
        local elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
        total_ms=$((total_ms + elapsed_ms))
        if [ $elapsed_ms -lt $min_ms ]; then min_ms=$elapsed_ms; fi
        if [ $elapsed_ms -gt $max_ms ]; then max_ms=$elapsed_ms; fi
    done

    local avg_ms=$((total_ms / ITERATIONS))
    printf "  ${GREEN}%-20s${NC} avg: ${CYAN}%6d ms${NC}  min: %d ms  max: %d ms\n" \
        "$name" "$avg_ms" "$min_ms" "$max_ms"
}

echo ""
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}  Fajar Lang Benchmark Suite${NC}"
echo -e "${YELLOW}  Interpreter mode | ${ITERATIONS} iterations | ${WARMUP} warmup${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo ""

if [ -n "$1" ] && [ "$1" != "--compare" ]; then
    run_bench "$1"
else
    for bench in "${BENCHMARKS[@]}"; do
        run_bench "$bench"
    done
fi

echo ""
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo "Done. For native compiled benchmarks: fj build --release <file>.fj"
echo ""
