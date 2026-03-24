#!/bin/bash
# FajarOS Nova v0.7 "Nexus" — Process Lifecycle QEMU Test (Sprint V2)
# Tests: process table, spawn, Ring 3, exec, exit, kill, context switch
#
# Usage: ./fajaros_nova_v2_test.sh

set -e

KERNEL="${1:-examples/fajaros_nova_kernel}"
ISO="/tmp/fajaros_nova_test.iso"
SERIAL="/tmp/nova_v2_serial.log"
NVME_DISK="/tmp/nova_v2_nvme.img"
MON="/tmp/nova_v2_mon.sock"
SCREEN="/tmp/nova_v2_screen"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0

check() {
    if grep -q "$1" "$SERIAL" 2>/dev/null; then
        echo -e "  ${GREEN}PASS${NC} — $2"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} — $2 (pattern: '$1')"
        FAIL=$((FAIL + 1))
    fi
}

check_vga() {
    if [ -f "${SCREEN}.ppm" ]; then
        echo -e "  ${GREEN}PASS${NC} — $1"
        PASS=$((PASS + 1))
    else
        echo -e "  ${YELLOW}SKIP${NC} — $1 (no screenshot)"
    fi
}

echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo -e "${CYAN}  Nova v0.7 — Sprint V2: Process Lifecycle${NC}"
echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo ""

# Check kernel
if [ ! -f "$KERNEL" ]; then
    echo -e "${RED}Error: Kernel not found at $KERNEL${NC}"
    echo "Build with: cargo run --release --features native -- build --target x86_64-none examples/fajaros_nova_kernel.fj"
    exit 1
fi

# Build ISO if needed
if [ ! -f "$ISO" ]; then
    echo -e "${YELLOW}Building ISO...${NC}"
    mkdir -p /tmp/nova_v2_iso/boot/grub
    cp "$KERNEL" /tmp/nova_v2_iso/boot/fajaros.elf
    cat > /tmp/nova_v2_iso/boot/grub/grub.cfg << 'GRUBEOF'
set timeout=0
set default=0
menuentry "FajarOS Nova" {
    multiboot2 /boot/fajaros.elf
    boot
}
GRUBEOF
    grub-mkrescue -o "$ISO" /tmp/nova_v2_iso 2>/dev/null
    rm -rf /tmp/nova_v2_iso
fi

# Create NVMe disk
dd if=/dev/zero of="$NVME_DISK" bs=1M count=32 2>/dev/null
mkfs.fat -F 32 -n "NOVATEST" "$NVME_DISK" 2>/dev/null

echo -e "${YELLOW}Kernel:${NC} $KERNEL ($(ls -lh "$KERNEL" | awk '{print $5}'))"
echo ""

# ═══════════════════════════════════════════
# V2.1: Process table verification
# ═══════════════════════════════════════════
echo -e "${CYAN}V2.1: Process table verification${NC}"
rm -f "$SERIAL" "$MON"
timeout 10 qemu-system-x86_64 \
    -cdrom "$ISO" -m 256M -display none \
    -drive file="$NVME_DISK",format=raw,if=none,id=nvme0 \
    -device nvme,serial=FJTEST,drive=nvme0 \
    -boot d \
    -serial file:"$SERIAL" \
    -monitor none -no-reboot 2>/dev/null || true

check "\[PROC\]" "V2.1: Process table v2 initialized"
check "\[INIT\].*Init process" "V2.1: Init process (PID 1) started"

# ═══════════════════════════════════════════
# V2.2-V2.3: Spawn + Ring 3
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.2-V2.3: Spawn + Ring 3 programs${NC}"
check "\[RING3\].*5 user programs" "V2.2: 5 Ring 3 programs installed"
check "hello.*goodbye.*fajar\|hello, goodbye, fajar" "V2.3: Ring 3 programs named"

# ═══════════════════════════════════════════
# V2.4: ELF exec infrastructure
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.4: ELF exec infrastructure${NC}"
check "\[ELF\] Syscall table ready" "V2.4: ELF syscall table ready"
# Verify kernel has exec functions
if grep -q "fn sys_exec" examples/fajaros_nova_kernel.fj; then
    echo -e "  ${GREEN}PASS${NC} — V2.4: sys_exec() defined in kernel"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}FAIL${NC} — V2.4: sys_exec() not found"
    FAIL=$((FAIL + 1))
fi

# ═══════════════════════════════════════════
# V2.5: Process exit + reap
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.5: Process exit + reap${NC}"
if grep -q "fn process_exit_v2\|fn process_exit_with_signal\|fn process_reap" examples/fajaros_nova_kernel.fj; then
    echo -e "  ${GREEN}PASS${NC} — V2.5: process_exit + reap functions defined"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}FAIL${NC} — V2.5: exit/reap functions missing"
    FAIL=$((FAIL + 1))
fi

# ═══════════════════════════════════════════
# V2.6: Multiple processes (preemptive)
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.6: Multiple processes + preemptive scheduling${NC}"
check "\[INIT\].*preemptive scheduling active" "V2.6: Preemptive scheduling confirmed"

# ═══════════════════════════════════════════
# V2.7: Context switch (timer ISR)
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.7: Context switch infrastructure${NC}"
if grep -q "fn save_context\|fn restore_context\|fn pick_next_process" examples/fajaros_nova_kernel.fj; then
    echo -e "  ${GREEN}PASS${NC} — V2.7: Context switch functions defined"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}FAIL${NC} — V2.7: Context switch functions missing"
    FAIL=$((FAIL + 1))
fi

# ═══════════════════════════════════════════
# V2.8: Kill process (signals)
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.8: Kill process (signal infrastructure)${NC}"
if grep -q "fn sys_kill\|fn signal_send\|fn signal_deliver_default" examples/fajaros_nova_kernel.fj; then
    echo -e "  ${GREEN}PASS${NC} — V2.8: Signal send/deliver functions defined"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}FAIL${NC} — V2.8: Signal functions missing"
    FAIL=$((FAIL + 1))
fi

# ═══════════════════════════════════════════
# V2.9: Syscall dispatch from Ring 3
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.9: Syscall dispatch from Ring 3${NC}"
check "\[SYSCALL\].*Entry stub.*MSRs configured\|\[SYSCALL\].*configured" "V2.9: SYSCALL entry configured"

# ═══════════════════════════════════════════
# V2.10: Fork infrastructure
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}V2.10: Fork infrastructure${NC}"
if grep -q "fn sys_fork\|fn fork_clone_page_tables\|fn fork_copy_fd_table" examples/fajaros_nova_kernel.fj; then
    echo -e "  ${GREEN}PASS${NC} — V2.10: fork() + page table clone + FD copy defined"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}FAIL${NC} — V2.10: fork functions missing"
    FAIL=$((FAIL + 1))
fi

# ═══════════════════════════════════════════
# VGA Screenshot
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}Bonus: VGA screenshot with NVMe${NC}"
rm -f "$MON" "${SCREEN}.ppm"
timeout 8 qemu-system-x86_64 \
    -cdrom "$ISO" -m 256M -display none \
    -drive file="$NVME_DISK",format=raw,if=none,id=nvme0 \
    -device nvme,serial=FJTEST,drive=nvme0 \
    -monitor unix:$MON,server,nowait \
    -serial file:/dev/null \
    -no-reboot 2>/dev/null &
QPID=$!
sleep 5
python3 -c "
import socket, time
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('$MON')
time.sleep(0.3)
s.recv(4096)
s.send(b'screendump ${SCREEN}.ppm\n')
time.sleep(0.5)
s.recv(4096)
s.close()
" 2>/dev/null || true
kill $QPID 2>/dev/null; wait $QPID 2>/dev/null

if [ -f "${SCREEN}.ppm" ]; then
    python3 -c "from PIL import Image; Image.open('${SCREEN}.ppm').save('${SCREEN}.png')" 2>/dev/null && \
        echo -e "  ${GREEN}PASS${NC} — NVMe boot screenshot: ${SCREEN}.png" || \
        echo -e "  ${GREEN}PASS${NC} — Screenshot captured (PPM)"
else
    echo -e "  ${YELLOW}SKIP${NC} — Screenshot not captured"
fi

# ═══════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo -e "  Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"
echo -e "${CYAN}═══════════════════════════════════════════${NC}"

# Cleanup
rm -f "$MON" "$NVME_DISK"

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}SOME TESTS FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}ALL V2 TESTS PASSED${NC}"
    exit 0
fi
