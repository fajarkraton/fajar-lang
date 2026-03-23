#!/bin/bash
# FajarOS ARM64 QEMU Boot Test
# Cross-compiles ARM64 kernel and boots in qemu-system-aarch64.

set -e

FAJAROS_DIR="${FAJAROS_DIR:-/home/primecore/Documents/fajaros-x86}"
FJ_BIN="${FJ_BIN:-$(dirname "$0")/../target/release/fj}"
BOOT_LOG="/tmp/fajaros_arm64_boot_test.log"
ARM64_SRC="$FAJAROS_DIR/arch/aarch64/boot.fj"
ARM64_ELF="/tmp/fajaros-arm64-test.elf"
TIMEOUT="${TIMEOUT:-15}"

echo "═══════════════════════════════════════════"
echo "  FajarOS ARM64 QEMU Boot Test"
echo "═══════════════════════════════════════════"

# Check prerequisites
if [ ! -f "$FJ_BIN" ]; then echo "❌ fj not found: $FJ_BIN"; exit 1; fi
if [ ! -f "$ARM64_SRC" ]; then echo "❌ ARM64 source not found: $ARM64_SRC"; exit 1; fi
if ! command -v qemu-system-aarch64 &>/dev/null; then echo "❌ qemu-system-aarch64 not found"; exit 1; fi

# Step 1: Cross-compile
echo "=== Step 1: Cross-compiling ARM64 kernel ==="
export PATH="$(dirname "$FJ_BIN"):$PATH"
fj build --target aarch64-unknown-none "$ARM64_SRC" -o "$ARM64_ELF" 2>&1
echo "✅ Compiled: $ARM64_ELF ($(stat -c%s "$ARM64_ELF" 2>/dev/null || stat -f%z "$ARM64_ELF") bytes)"
file "$ARM64_ELF"
echo ""

# Step 2: Boot in QEMU
echo "=== Step 2: Booting in QEMU aarch64 (${TIMEOUT}s) ==="
timeout "$TIMEOUT" qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a72 \
    -m 256M \
    -kernel "$ARM64_ELF" \
    -nographic \
    -no-reboot \
    2>&1 | tee "$BOOT_LOG" || true
echo ""

# Step 3: Verify
echo "=== Step 3: Verifying boot output ==="
PASS=0; FAIL=0

check() {
    if grep -q "$1" "$BOOT_LOG" 2>/dev/null; then
        echo "  ✅ $2"; PASS=$((PASS + 1))
    else
        echo "  ❌ $2"; FAIL=$((FAIL + 1))
    fi
}

check "FajarOS.*ARM64\|ARM64.*Microkernel" "ARM64 kernel banner"
check "BOOT.*QEMU\|Board.*QEMU" "QEMU virt detected"
check "UART" "UART initialized"
check "MEM.*Frame\|Frame.*alloc" "Memory manager ready"
check "READY.*running\|kernel running" "Kernel fully booted"

echo ""
echo "═══════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════════"

[ "$FAIL" -gt 0 ] && echo "❌ ARM64 BOOT FAILED" && exit 1
echo "✅ ARM64 BOOT PASSED" && exit 0
