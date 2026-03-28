#!/bin/bash
# FajarOS Surya — ARM64 QEMU Automated Test Suite
#
# Tests ARM64 cross-compilation and QEMU boot for all arm64 examples.
#
# Usage:
#   ./test_qemu_arm64.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
FJ="$PROJECT_DIR/target/debug/fj"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0

check() {
    if [ $1 -eq 0 ]; then
        printf "  ${GREEN}PASS${NC} %s\n" "$2"
        PASS=$((PASS + 1))
    else
        printf "  ${RED}FAIL${NC} %s\n" "$2"
        FAIL=$((FAIL + 1))
    fi
}

echo ""
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}  FajarOS Surya — ARM64 Test Suite${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo ""

# Build compiler if needed
if [ ! -f "$FJ" ]; then
    echo "[build] Building fj compiler..."
    (cd "$PROJECT_DIR" && cargo build --features native)
fi

# E1: Cross-compile all ARM64 examples
echo -e "${CYAN}E1: Cross-Compilation${NC}"
for f in "$SCRIPT_DIR"/../fajaros_arm64_*.fj; do
    name=$(basename "$f" .fj)
    result=$("$FJ" build --target aarch64-unknown-none-elf --no-std "$f" 2>&1 | tail -1)
    if echo "$result" | grep -q "Built:"; then
        check 0 "$name compiled"
    else
        check 1 "$name compile failed"
    fi
done

# E2: Verify ELF format
echo ""
echo -e "${CYAN}E2: Binary Verification${NC}"
for f in "$SCRIPT_DIR"/../fajaros_arm64_boot "$SCRIPT_DIR"/../fajaros_arm64_mmu "$SCRIPT_DIR"/../fajaros_arm64_shell; do
    if [ -f "$f" ]; then
        fmt=$(file "$f" | grep -c "ARM aarch64")
        check $((1 - fmt)) "$(basename $f) is ARM64 ELF"
    fi
done

# E3: QEMU ARM64 boot test (if qemu-system-aarch64 available)
echo ""
echo -e "${CYAN}E3: QEMU Boot${NC}"
if which qemu-system-aarch64 >/dev/null 2>&1; then
    SERIAL="/tmp/surya_serial.log"
    rm -f "$SERIAL"
    timeout 5 qemu-system-aarch64 \
        -M virt -cpu cortex-a72 -m 128M \
        -nographic \
        -kernel "$SCRIPT_DIR/../fajaros_arm64_boot" \
        -serial file:"$SERIAL" \
        2>/dev/null &
    QEMU_PID=$!
    sleep 5
    kill $QEMU_PID 2>/dev/null || true
    wait $QEMU_PID 2>/dev/null || true

    if [ -f "$SERIAL" ] && [ -s "$SERIAL" ]; then
        check 0 "QEMU ARM64 produced serial output"
    else
        check 1 "QEMU ARM64 no serial output (may need bootloader)"
    fi
else
    echo -e "  ${YELLOW}SKIP${NC} qemu-system-aarch64 not installed"
fi

# E4: Q6A BSP tests (Rust side)
echo ""
echo -e "${CYAN}E4: Q6A BSP Tests${NC}"
BSP_RESULT=$(cd "$PROJECT_DIR" && RUST_MIN_STACK=8388608 cargo test --lib -- bsp::dragon_q6a 2>&1 | grep "test result")
BSP_PASS=$(echo "$BSP_RESULT" | grep -oP '\d+ passed' | grep -oP '\d+')
BSP_FAIL=$(echo "$BSP_RESULT" | grep -oP '\d+ failed' | grep -oP '\d+')
check ${BSP_FAIL:-1} "Q6A BSP: ${BSP_PASS:-0} tests passed"

# Summary
echo ""
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
printf "Results: ${GREEN}%d passed${NC}, ${RED}%d failed${NC}\n" "$PASS" "$FAIL"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"

if [ $FAIL -gt 0 ]; then exit 1; fi
exit 0
