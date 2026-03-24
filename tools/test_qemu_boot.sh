#!/bin/bash
# FajarOS x86_64 QEMU Boot Test
# Verifies that the Fajar Lang compiler produces a bootable OS kernel.
#
# Prerequisites:
#   - fj binary in PATH (or FJ_BIN set)
#   - fajaros-x86 repo (FAJAROS_DIR)
#   - qemu-system-x86_64
#   - grub-mkrescue, xorriso, mtools

set -e

FAJAROS_DIR="${FAJAROS_DIR:-/home/primecore/Documents/fajaros-x86}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FJ_BIN="${FJ_BIN:-$SCRIPT_DIR/../target/release/fj}"
BOOT_LOG="${TMPDIR:-/tmp}/fajaros_boot_test.log"
TIMEOUT="${TIMEOUT:-12}"

echo "═══════════════════════════════════════════"
echo "  FajarOS x86_64 QEMU Boot Test"
echo "═══════════════════════════════════════════"
echo "Compiler: $FJ_BIN"
echo "FajarOS:  $FAJAROS_DIR"
echo ""

# Check prerequisites
if [ ! -f "$FJ_BIN" ]; then
    echo "❌ fj binary not found: $FJ_BIN"
    echo "   Run: cargo build --release --features native"
    exit 1
fi

if [ ! -d "$FAJAROS_DIR" ]; then
    echo "❌ FajarOS repo not found: $FAJAROS_DIR"
    exit 1
fi

if ! command -v qemu-system-x86_64 &>/dev/null; then
    echo "❌ qemu-system-x86_64 not found"
    exit 1
fi

# Step 1: Build kernel
echo "=== Step 1: Building FajarOS kernel ==="
cd "$FAJAROS_DIR"
export PATH="$(dirname "$FJ_BIN"):$PATH"
make build 2>&1
echo ""

# Step 2: Verify ELF
echo "=== Step 2: Verifying kernel ELF ==="
ELF="$FAJAROS_DIR/build/fajaros.elf"
if [ ! -f "$ELF" ]; then
    echo "❌ Kernel ELF not found: $ELF"
    exit 1
fi
SIZE=$(stat -c%s "$ELF" 2>/dev/null || stat -f%z "$ELF" 2>/dev/null)
echo "✅ Kernel: $ELF ($SIZE bytes)"
file "$ELF"
echo ""

# Step 3: Build ISO
echo "=== Step 3: Building bootable ISO ==="
make iso 2>&1
echo ""

# Step 4: Boot in QEMU
echo "=== Step 4: Booting in QEMU (${TIMEOUT}s timeout) ==="
timeout "$TIMEOUT" qemu-system-x86_64 \
    -cdrom build/fajaros.iso \
    -nographic \
    -no-reboot \
    -m 512M \
    -cpu qemu64,+avx2,+sse4.2 \
    2>&1 | tee "$BOOT_LOG" || true
echo ""

# Step 5: Check boot output
echo "=== Step 5: Verifying boot output ==="
PASS=0
FAIL=0

check() {
    if grep -q "$1" "$BOOT_LOG" 2>/dev/null; then
        echo "  ✅ $2"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $2 (pattern: '$1')"
        FAIL=$((FAIL + 1))
    fi
}

check "BOOT32" "32-bit trampoline started"
check "VFS" "VFS subsystem initialized"
check "NET" "Network subsystem initialized"
check "IPC2" "IPC subsystem ready"
check "SYSCALL" "Syscall entry configured"
check "PROC" "Process table ready"
check "RING3" "Ring 3 user programs installed"
check "FajarOS Nova\|NOVA" "FajarOS kernel banner"
check "nova>" "Shell prompt appeared"

echo ""
echo "═══════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
    echo "❌ BOOT TEST FAILED"
    exit 1
else
    echo "✅ BOOT TEST PASSED"
    exit 0
fi
