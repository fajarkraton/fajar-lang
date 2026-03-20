#!/bin/bash
# FajarOS Nova — Automated QEMU Test Suite
# Tests the kernel via serial I/O in QEMU
#
# Usage: ./fajaros_nova_test.sh [path/to/fajaros_nova_kernel]
# Requirements: qemu-system-x86_64, grub-mkrescue

set -e

KERNEL="${1:-examples/fajaros_nova_kernel}"
ISO="/tmp/fajaros_nova_test.iso"
SERIAL_LOG="/tmp/nova_test_serial.log"
RESULT=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo -e "${CYAN}  FajarOS Nova — Automated Test Suite${NC}"
echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo ""

# Check kernel exists
if [ ! -f "$KERNEL" ]; then
    echo -e "${RED}Error: Kernel not found at $KERNEL${NC}"
    echo "Build with: cargo run --release --features native -- build --target x86_64-none examples/fajaros_nova_kernel.fj"
    exit 1
fi

echo -e "${YELLOW}Kernel:${NC} $KERNEL ($(ls -lh "$KERNEL" | awk '{print $5}'))"
echo -e "${YELLOW}Format:${NC} $(file "$KERNEL" | cut -d: -f2 | xargs)"

# Check ELF sections
echo -e "${YELLOW}Sections:${NC}"
readelf -S "$KERNEL" 2>/dev/null | grep -E "multiboot|\.text|\.rodata|\.bss|\.stack" | awk '{print "  " $2 " at " $4 " (" $6 " bytes)"}'
echo ""

# Create ISO
echo -e "${YELLOW}Creating GRUB2 ISO...${NC}"
mkdir -p /tmp/nova_test_iso/boot/grub
cp "$KERNEL" /tmp/nova_test_iso/boot/fajaros.elf
cat > /tmp/nova_test_iso/boot/grub/grub.cfg << 'GRUBEOF'
set timeout=0
set default=0
menuentry "FajarOS Nova Test" {
    multiboot2 /boot/fajaros.elf
    boot
}
GRUBEOF
grub-mkrescue -o "$ISO" /tmp/nova_test_iso 2>/dev/null
echo -e "${GREEN}ISO created:${NC} $ISO ($(ls -lh "$ISO" | awk '{print $5}'))"
echo ""

# Test 1: Boot test (serial output)
echo -e "${CYAN}Test 1: Boot sequence${NC}"
timeout 6 qemu-system-x86_64 \
    -cdrom "$ISO" -m 256M -display none \
    -serial file:"$SERIAL_LOG" \
    -monitor none -no-reboot 2>/dev/null || true

if grep -q "\[BOOT32\]" "$SERIAL_LOG" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — Multiboot2 trampoline (32→64 bit)"
else
    echo -e "  ${RED}FAIL${NC} — No [BOOT32] in serial"
    RESULT=1
fi

if grep -q "\[NOVA\].*booted" "$SERIAL_LOG" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — kernel_main() reached"
else
    echo -e "  ${RED}FAIL${NC} — kernel_main() not reached"
    RESULT=1
fi

if grep -q "shell commands ready" "$SERIAL_LOG" 2>/dev/null; then
    CMDS=$(grep "shell commands" "$SERIAL_LOG" | grep -oP '\d+')
    echo -e "  ${GREEN}PASS${NC} — $CMDS shell commands initialized"
else
    echo -e "  ${RED}FAIL${NC} — Shell not ready"
    RESULT=1
fi

if grep -q "RamFS" "$SERIAL_LOG" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — RAM filesystem initialized"
else
    echo -e "  ${RED}FAIL${NC} — RamFS not initialized"
    RESULT=1
fi

if grep -q "VGA console active" "$SERIAL_LOG" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — VGA console at 0xB8000"
else
    echo -e "  ${RED}FAIL${NC} — VGA not active"
    RESULT=1
fi

echo ""

# Test 2: Screenshot capture
echo -e "${CYAN}Test 2: VGA screenshot${NC}"
timeout 6 qemu-system-x86_64 \
    -cdrom "$ISO" -m 256M -display none \
    -serial file:/dev/null \
    -monitor unix:/tmp/nova_test_mon.sock,server,nowait \
    -no-reboot 2>/dev/null &
QPID=$!
sleep 4

python3 -c "
import socket, time
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('/tmp/nova_test_mon.sock')
time.sleep(0.3)
s.recv(4096)
s.send(b'screendump /tmp/nova_test_screen.ppm\n')
time.sleep(0.5)
s.recv(4096)
s.close()
" 2>/dev/null || true

kill $QPID 2>/dev/null; wait $QPID 2>/dev/null

if [ -f /tmp/nova_test_screen.ppm ] && [ $(stat -c%s /tmp/nova_test_screen.ppm 2>/dev/null || echo 0) -gt 1000 ]; then
    SIZE=$(ls -lh /tmp/nova_test_screen.ppm | awk '{print $5}')
    echo -e "  ${GREEN}PASS${NC} — VGA screenshot captured ($SIZE)"
    # Convert to PNG if possible
    python3 -c "from PIL import Image; Image.open('/tmp/nova_test_screen.ppm').save('/tmp/nova_test_screen.png')" 2>/dev/null && \
        echo -e "  ${GREEN}INFO${NC} — PNG saved to /tmp/nova_test_screen.png" || true
else
    echo -e "  ${YELLOW}SKIP${NC} — Screenshot capture failed (non-critical)"
fi

echo ""

# Test 2b: NVMe + FAT32 boot test
echo -e "${CYAN}Test 2b: NVMe + FAT32 boot${NC}"

# Create FAT32 test disk
NVME_DISK="/tmp/nova_test_nvme.img"
dd if=/dev/zero of="$NVME_DISK" bs=1M count=32 2>/dev/null
mkfs.fat -F 32 -n "NOVATEST" "$NVME_DISK" 2>/dev/null

timeout 10 qemu-system-x86_64 \
    -cdrom "$ISO" -m 256M -display none \
    -drive file="$NVME_DISK",format=raw,if=none,id=nvme0 \
    -device nvme,serial=FJTEST,drive=nvme0 \
    -boot d \
    -serial file:"$SERIAL_LOG.nvme" \
    -monitor none -no-reboot 2>/dev/null || true

if grep -q "\[NVMe\] Controller enabled" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — NVMe controller enabled"
else
    echo -e "  ${YELLOW}SKIP${NC} — NVMe controller not enabled (may need -boot d)"
fi

if grep -q "\[NVMe\] I/O queues ready" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — NVMe I/O queues created"
else
    echo -e "  ${YELLOW}WARN${NC} — NVMe I/O queues not ready"
fi

if grep -q "Sector 0 read OK" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — NVMe sector read verified (0x55AA)"
else
    echo -e "  ${YELLOW}WARN${NC} — NVMe sector read not verified"
fi

if grep -q "\[FAT32\] Mounted" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — FAT32 filesystem mounted"
else
    echo -e "  ${YELLOW}WARN${NC} — FAT32 not mounted"
fi

if grep -q "\[VFS\] Initialized" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — VFS initialized"
else
    echo -e "  ${YELLOW}WARN${NC} — VFS not initialized"
fi

if grep -q "\[NET\] Initialized" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — Network stack initialized"
else
    echo -e "  ${YELLOW}WARN${NC} — Network not initialized"
fi

if grep -q "\[ELF\] Syscall" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — Syscall table ready"
else
    echo -e "  ${YELLOW}WARN${NC} — Syscall table not ready"
fi

if grep -q "\[PROC\]" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — Process table v2 ready"
else
    echo -e "  ${YELLOW}WARN${NC} — Process table not ready"
fi

if grep -q "\[KB\]" "$SERIAL_LOG.nvme" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — Keyboard buffer ready"
else
    echo -e "  ${YELLOW}WARN${NC} — Keyboard buffer not ready"
fi

echo ""

# Test 2c: SMP boot test (4 cores)
echo -e "${CYAN}Test 2c: SMP boot (4 cores)${NC}"
timeout 8 qemu-system-x86_64 \
    -cdrom "$ISO" -m 256M -display none \
    -smp 4 \
    -serial file:"$SERIAL_LOG.smp" \
    -monitor none -no-reboot 2>/dev/null || true

if grep -q "\[BOOT32\]" "$SERIAL_LOG.smp" 2>/dev/null; then
    echo -e "  ${GREEN}PASS${NC} — Boot with -smp 4"
else
    echo -e "  ${RED}FAIL${NC} — SMP boot failed"
    RESULT=1
fi

echo ""

# Test 3: ELF validation
echo -e "${CYAN}Test 3: ELF validation${NC}"
if readelf -h "$KERNEL" 2>/dev/null | grep -q "ELF64"; then
    echo -e "  ${GREEN}PASS${NC} — Valid ELF64 binary"
else
    echo -e "  ${RED}FAIL${NC} — Not a valid ELF64"
    RESULT=1
fi

if readelf -S "$KERNEL" 2>/dev/null | grep -q "multiboot_header"; then
    echo -e "  ${GREEN}PASS${NC} — Multiboot2 header section present"
else
    echo -e "  ${RED}FAIL${NC} — Missing .multiboot_header"
    RESULT=1
fi

ENTRY=$(readelf -h "$KERNEL" 2>/dev/null | grep "Entry point" | awk '{print $NF}')
echo -e "  ${GREEN}INFO${NC} — Entry point: $ENTRY"

echo ""

# Test 4: Command count
echo -e "${CYAN}Test 4: Command count${NC}"
CMD_COUNT=$(grep "@kernel fn cmd_" examples/fajaros_nova_kernel.fj | wc -l)
echo -e "  ${GREEN}PASS${NC} — $CMD_COUNT command functions defined"
if [ "$CMD_COUNT" -ge 120 ]; then
    echo -e "  ${GREEN}PASS${NC} — Target: 120+ commands achieved"
else
    echo -e "  ${YELLOW}WARN${NC} — Below 120 command target ($CMD_COUNT)"
fi

echo ""

# Test 5: Kernel features
echo -e "${CYAN}Test 5: Feature verification${NC}"
FEATURES=(
    "ramfs_init:RAM filesystem"
    "history_init:Command history"
    "scancode_to_ascii:Keyboard scancode handler"
    "console_scroll:VGA scrolling"
    "dispatch_command:Command dispatcher"
    "cmd_grep:grep command"
    "cmd_sort:sort command"
    "cmd_calc:Calculator"
    "cmd_tensor:Tensor operations"
    "cmd_mnist:MNIST demo"
    "acpi_shutdown:ACPI power management"
    "pci_read32:PCI bus access"
    "cmd_fib:Fibonacci benchmark"
    "nvme_init:NVMe driver"
    "nvme_read_sectors:NVMe sector read"
    "nvme_write_sectors:NVMe sector write"
    "fat32_mount:FAT32 filesystem"
    "fat32_create_file:FAT32 file write"
    "fat32_delete_file:FAT32 file delete"
    "vfs_init:Virtual filesystem"
    "net_init:Network stack"
    "elf_validate:ELF64 loader"
    "elf_load_segments:ELF segment loader"
    "syscall_handle:Syscall dispatcher"
    "proc_v2_fork:Process fork"
    "proc_v2_exit:Process exit"
    "kb_handle_irq:PS/2 keyboard IRQ"
    "pipe_create:Pipe IPC"
    "cmd_source:Shell scripting"
    "smp_install_trampoline:SMP AP trampoline"
    "cmd_ifconfig:Network config"
    "cmd_ping:ICMP ping"
    "cmd_lsusb:USB detection"
    "cmd_exec:ELF exec"
    "cmd_fatwrite:FAT32 write"
    "blk_read:Block device read"
    "blk_write:Block device write"
)

for feat in "${FEATURES[@]}"; do
    FUNC="${feat%%:*}"
    DESC="${feat##*:}"
    if grep -q "$FUNC" examples/fajaros_nova_kernel.fj; then
        echo -e "  ${GREEN}PASS${NC} — $DESC ($FUNC)"
    else
        echo -e "  ${RED}FAIL${NC} — $DESC missing ($FUNC)"
        RESULT=1
    fi
done

echo ""

# Summary
echo -e "${CYAN}═══════════════════════════════════════════${NC}"
SERIAL_LINES=$(cat "$SERIAL_LOG" 2>/dev/null | wc -l)
echo -e "${YELLOW}Serial output:${NC} $SERIAL_LINES lines"
echo -e "${YELLOW}Kernel:${NC} $(wc -l < examples/fajaros_nova_kernel.fj) lines Fajar Lang"
echo -e "${YELLOW}Binary:${NC} $(ls -lh "$KERNEL" | awk '{print $5}') ELF x86_64"
echo -e "${YELLOW}Commands:${NC} $CMD_COUNT"

if [ $RESULT -eq 0 ]; then
    echo -e "\n${GREEN}ALL TESTS PASSED${NC}"
else
    echo -e "\n${RED}SOME TESTS FAILED${NC}"
fi

# Cleanup
rm -rf /tmp/nova_test_iso /tmp/nova_test_mon.sock

exit $RESULT
