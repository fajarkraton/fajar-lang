#!/bin/bash
# qemu_arm64_boot_test.sh — Automated QEMU ARM64 boot test for FajarOS
#
# Builds the kernel, boots in QEMU, captures serial output, verifies boot.
# Exit code: 0 = PASS, 1 = FAIL
#
# Usage: bash tests/qemu_arm64_boot_test.sh

set -e
cd "$(dirname "$0")/.."

KERNEL_SRC="examples/fajaros_arm64_boot.fj"
KERNEL_BIN="examples/fajaros_arm64_boot"
TIMEOUT=5
PASS=0
FAIL=0

echo "=== FajarOS ARM64 QEMU Boot Test ==="
echo ""

# Step 1: Build
echo "[1/4] Building kernel..."
cargo run --features native -- build "$KERNEL_SRC" --target aarch64-unknown-none --no-std 2>&1 | tail -1
if [ ! -f "$KERNEL_BIN" ]; then
    echo "FAIL: kernel binary not found"
    exit 1
fi
echo "  Binary: $(ls -la "$KERNEL_BIN" | awk '{print $5}') bytes"
echo ""

# Step 2: Verify ELF format
echo "[2/4] Verifying ELF..."
FILE_INFO=$(file "$KERNEL_BIN")
if echo "$FILE_INFO" | grep -q "ELF 64-bit.*ARM aarch64"; then
    echo "  PASS: ELF 64-bit ARM aarch64"
    PASS=$((PASS + 1))
else
    echo "  FAIL: not ARM64 ELF: $FILE_INFO"
    FAIL=$((FAIL + 1))
fi
if echo "$FILE_INFO" | grep -q "statically linked"; then
    echo "  PASS: statically linked"
    PASS=$((PASS + 1))
else
    echo "  FAIL: not statically linked"
    FAIL=$((FAIL + 1))
fi
echo ""

# Step 3: Boot in QEMU and capture output
echo "[3/4] Booting in QEMU (${TIMEOUT}s timeout)..."
OUTPUT=$(timeout $TIMEOUT qemu-system-aarch64 \
    -M virt,gic-version=3 \
    -cpu cortex-a72 \
    -nographic \
    -kernel "$KERNEL_BIN" 2>&1 || true)

echo "--- Serial Output ---"
echo "$OUTPUT"
echo "--- End Output ---"
echo ""

# Step 4: Verify output
echo "[4/4] Verifying output..."

check() {
    local pattern="$1"
    local label="$2"
    if echo "$OUTPUT" | grep -q "$pattern"; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label (expected: $pattern)"
        FAIL=$((FAIL + 1))
    fi
}

check "FajarOS ARM64" "Boot banner"
check "EL:" "Exception level EL1"
check "Timer:" "Timer frequency"
check "GIC+Timer ready" "GIC + Timer initialized"
check "fjsh>" "Shell prompt"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ $FAIL -eq 0 ]; then
    echo "ALL TESTS PASSED"
    exit 0
else
    echo "SOME TESTS FAILED"
    exit 1
fi
