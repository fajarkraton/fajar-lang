#!/bin/bash
# Cross-compilation + QEMU test for Fajar Lang
# Usage: ./tests/cross/test_qemu.sh <arch> <expected_exit_code>
#   arch: aarch64 | riscv64
#   expected_exit_code: integer (default 42)

set -euo pipefail

ARCH="${1:-aarch64}"
EXPECTED="${2:-42}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORK_DIR=$(mktemp -d)
trap "rm -rf $WORK_DIR" EXIT

OBJ_FILE="$WORK_DIR/fj_program.o"
BIN_FILE="$WORK_DIR/fj_program"
RT_FILE="$SCRIPT_DIR/rt_entry.c"

echo "=== Fajar Lang Cross-Compilation Test ==="
echo "Target: $ARCH | Expected exit: $EXPECTED"

# The Rust test will produce the .o file; this script just links + runs.
# For standalone usage, the .o file path can be passed as $3.
OBJ_INPUT="${3:-$OBJ_FILE}"

if [ ! -f "$OBJ_INPUT" ]; then
    echo "ERROR: Object file not found: $OBJ_INPUT"
    exit 1
fi

case "$ARCH" in
    aarch64)
        CC=aarch64-linux-gnu-gcc
        QEMU=qemu-aarch64
        ;;
    riscv64)
        CC=riscv64-linux-gnu-gcc
        QEMU=qemu-riscv64
        ;;
    *)
        echo "ERROR: Unknown arch: $ARCH (use aarch64 or riscv64)"
        exit 1
        ;;
esac

echo "Linking with $CC..."
$CC -static -o "$BIN_FILE" "$OBJ_INPUT" "$RT_FILE" -lc

echo "Running on $QEMU..."
set +e
$QEMU "$BIN_FILE"
ACTUAL=$?
set -e

echo "Exit code: $ACTUAL (expected: $EXPECTED)"
if [ "$ACTUAL" -eq "$EXPECTED" ]; then
    echo "PASS"
    exit 0
else
    echo "FAIL: expected $EXPECTED, got $ACTUAL"
    exit 1
fi
