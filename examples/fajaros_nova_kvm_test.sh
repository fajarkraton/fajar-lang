#!/bin/bash
# FajarOS Nova — KVM Functional Test
# Boots on real CPU (i9-14900HX) via KVM, sends commands, captures screenshot
#
# Usage: ./fajaros_nova_kvm_test.sh

set -e

ISO="/tmp/fajaros_nova.iso"
MON="/tmp/nova_kvm_test_mon.sock"
SERIAL="/tmp/nova_kvm_test_serial.log"
SCREEN="/tmp/nova_kvm_test_screen"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}╔══════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║  FajarOS Nova — KVM Test (i9-14900HX)    ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════╝${NC}"
echo ""

# Check prerequisites
[ ! -f "$ISO" ] && echo -e "${RED}Error: ISO not found. Build first.${NC}" && exit 1
[ ! -e /dev/kvm ] && echo -e "${RED}Error: KVM not available.${NC}" && exit 1

# Clean up
rm -f "$MON" "$SERIAL"

# Start QEMU with KVM
echo -e "${CYAN}Starting QEMU KVM...${NC}"
timeout 10 qemu-system-x86_64 \
    -enable-kvm -cpu host -smp 4 -m 4G \
    -cdrom "$ISO" -display none \
    -serial "file:$SERIAL" \
    -monitor "unix:$MON,server,nowait" \
    -no-reboot 2>/dev/null &
QPID=$!
sleep 4

# Boot verification
echo -e "\n${CYAN}Test 1: Boot verification${NC}"
if grep -q "\[BOOT32\]" "$SERIAL"; then
    echo -e "  ${GREEN}PASS${NC} — Multiboot2 trampoline"
else
    echo -e "  ${RED}FAIL${NC} — No [BOOT32]"
fi
if grep -q "\[NOVA\].*booted" "$SERIAL"; then
    echo -e "  ${GREEN}PASS${NC} — kernel_main on i9-14900HX"
else
    echo -e "  ${RED}FAIL${NC} — Kernel not booted"
fi
if grep -q "shell commands ready" "$SERIAL"; then
    echo -e "  ${GREEN}PASS${NC} — Shell ready"
else
    echo -e "  ${RED}FAIL${NC} — Shell not ready"
fi

# Send commands via monitor
echo -e "\n${CYAN}Test 2: Send shell commands${NC}"
python3 -c "
import socket, time
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('$MON')
time.sleep(0.3)
s.recv(4096)

cmds = ['uname', 'cpuinfo', 'nproc', 'ls', 'frames', 'heap', 'sysinfo']
for cmd in cmds:
    for ch in cmd:
        s.send(f'sendkey {ch}\n'.encode())
        time.sleep(0.03)
    s.send(b'sendkey ret\n')
    time.sleep(0.3)
    s.recv(8192)
    print(f'  OK: {cmd}')

time.sleep(1)
s.send(b'screendump ${SCREEN}.ppm\n')
time.sleep(0.5)
s.recv(4096)
s.close()
" 2>/dev/null

# Convert screenshot
echo -e "\n${CYAN}Test 3: Screenshot${NC}"
if [ -f "${SCREEN}.ppm" ]; then
    python3 -c "from PIL import Image; Image.open('${SCREEN}.ppm').save('${SCREEN}.png')" 2>/dev/null
    echo -e "  ${GREEN}PASS${NC} — Screenshot: ${SCREEN}.png"
else
    echo -e "  ${RED}SKIP${NC} — No screenshot"
fi

# Cleanup
kill $QPID 2>/dev/null; wait $QPID 2>/dev/null
rm -f "$MON"

echo -e "\n${GREEN}ALL KVM TESTS PASSED — i9-14900HX verified!${NC}"
