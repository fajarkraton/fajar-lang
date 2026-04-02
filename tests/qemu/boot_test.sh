#!/bin/bash
# QEMU Boot Verification Test
# Builds a minimal multiboot kernel, boots it in QEMU, and checks serial output.
#
# Usage: ./tests/qemu/boot_test.sh
# Returns: 0 on success, 1 on failure

set -e
cd "$(dirname "$0")"

echo "=== QEMU Boot Verification ==="

# Build kernel
echo "[1/3] Building kernel..."
as --32 -o boot.o boot.S
ld -m elf_i386 -T linker.ld -o kernel.elf boot.o
echo "      kernel.elf: $(stat -c %s kernel.elf) bytes"

# Boot in QEMU
echo "[2/3] Booting in QEMU..."
rm -f /tmp/qemu_boot_test.txt
timeout 5 qemu-system-i386 \
    -kernel kernel.elf \
    -display none \
    -serial file:/tmp/qemu_boot_test.txt \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -no-reboot \
    -monitor none 2>/dev/null || true

# Check output
echo "[3/3] Verifying serial output..."
SERIAL=$(cat /tmp/qemu_boot_test.txt 2>/dev/null)
if echo "$SERIAL" | grep -q "FAJAR-BOOT-OK"; then
    echo "      Serial: $SERIAL"
    echo "      PASS: Kernel booted and wrote to serial"
    rm -f boot.o kernel.elf /tmp/qemu_boot_test.txt
    exit 0
else
    echo "      FAIL: Expected 'FAJAR-BOOT-OK', got: '$SERIAL'"
    rm -f boot.o kernel.elf /tmp/qemu_boot_test.txt
    exit 1
fi
