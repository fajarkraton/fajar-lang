#!/usr/bin/env bash
# run_baselines.sh — Run all baseline benchmarks across Fajar Lang +
# C + Rust + Go and emit a comparison table to stdout.
#
# P7.F4 of FAJAR_LANG_PERFECTION_PLAN.
#
# Requirements (skip langs that aren't available):
#   - fj   (this repo's binary; expects target/release/fj)
#   - gcc  (any version with -O2)
#   - rustc
#   - go
#
# Usage:
#   bash benches/baselines/run_baselines.sh
#   bash benches/baselines/run_baselines.sh fibonacci  # single bench
#
# Output: markdown-style comparison table per benchmark.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BASELINES="$ROOT/benches/baselines"
WORK="$(mktemp -d)"
trap "rm -rf '$WORK'" EXIT

FJ="$ROOT/target/release/fj"
if [ ! -x "$FJ" ]; then
    echo "warning: $FJ not found — building release..." >&2
    (cd "$ROOT" && cargo build --release --quiet)
fi

ALL_BENCHES=(fibonacci bubble_sort sum_loop matrix_multiply mandelbrot)
BENCHES=("${@:-${ALL_BENCHES[@]}}")

have() { command -v "$1" >/dev/null 2>&1; }

# Print one row of the comparison table.
# Args: lang_label time_secs notes
print_row() {
    printf "| %-15s | %15s | %s |\n" "$1" "$2" "$3"
}

run_one() {
    local lang="$1"
    local cmd="$2"
    local label="$3"
    if [ -z "$cmd" ] || ! eval "$cmd" >/dev/null 2>&1; then
        print_row "$label" "n/a" "compile/run skipped (toolchain missing or build failed)"
        return
    fi
    # Best-of-3 timing.
    local best=99999.0
    for i in 1 2 3; do
        local t
        t=$(/usr/bin/env bash -c "TIMEFORMAT='%R'; { time $cmd > /dev/null; } 2>&1")
        # Compare in awk to avoid bash float ops.
        best=$(awk -v a="$best" -v b="$t" 'BEGIN { print (b < a) ? b : a }')
    done
    print_row "$label" "${best}s" ""
}

for bench in "${BENCHES[@]}"; do
    src_fj="$BASELINES/${bench}.fj"
    src_rs="$BASELINES/${bench}.rs"
    src_c="$BASELINES/${bench}.c"
    src_go="$BASELINES/${bench}.go"

    if [ ! -f "$src_fj" ]; then
        echo "skip: $bench (no .fj source)" >&2
        continue
    fi

    echo
    echo "## $bench"
    echo
    echo "| Language        |            Time | Notes |"
    echo "|-----------------|----------------:|-------|"

    # C
    if [ -f "$src_c" ] && have gcc; then
        gcc -O2 "$src_c" -o "$WORK/${bench}_c" 2>/dev/null
        run_one "C" "$WORK/${bench}_c" "C (gcc -O2)"
    else
        print_row "C (gcc -O2)" "n/a" "no source or gcc missing"
    fi

    # Rust
    if [ -f "$src_rs" ] && have rustc; then
        rustc -O "$src_rs" -o "$WORK/${bench}_rs" 2>/dev/null
        run_one "Rust" "$WORK/${bench}_rs" "Rust (rustc -O)"
    else
        print_row "Rust (rustc -O)" "n/a" "no source or rustc missing"
    fi

    # Go
    if [ -f "$src_go" ] && have go; then
        (cd "$WORK" && go build -o "${bench}_go" "$src_go") 2>/dev/null
        run_one "Go" "$WORK/${bench}_go" "Go (go build)"
    else
        print_row "Go (go build)" "n/a" "no source or go missing"
    fi

    # Fajar Lang — interpreter (always available)
    run_one "fj-interp" "$FJ run $src_fj" "Fajar Lang interp"

    # Fajar Lang — Cranelift native (if `native` feature compiled in)
    if "$FJ" run --help 2>&1 | grep -q "native"; then
        run_one "fj-cranelift" "$FJ run --native $src_fj" "Fajar Lang Cranelift"
    fi
done

echo
echo "Note: best-of-3 wall-clock timing. Set IT=1 in the source to override."
