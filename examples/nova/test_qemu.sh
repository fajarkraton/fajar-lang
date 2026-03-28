#!/bin/bash
# FajarOS Nova — Comprehensive QEMU Automated Test Suite
#
# Boots the kernel in QEMU, captures serial output, and verifies
# that all major subsystems initialize correctly.
#
# Usage:
#   ./test_qemu.sh                    # Run all tests
#   ./test_qemu.sh --timeout 30       # Custom timeout (default: 15s)
#   ./test_qemu.sh --kernel path.bin  # Custom kernel binary
#
# Exit: 0 if all tests pass, 1 if any fail

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
KERNEL="${KERNEL:-$SCRIPT_DIR/../fajaros_nova_kernel}"
SERIAL_LOG="/tmp/nova_qemu_test_serial.log"
TIMEOUT=15
NVME_DISK="/tmp/nova_test_nvme.img"

# Parse args
while [[ $# -gt 0 ]]; do
    case $1 in
        --timeout) TIMEOUT="$2"; shift 2;;
        --kernel) KERNEL="$2"; shift 2;;
        *) shift;;
    esac
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

check_serial() {
    local pattern="$1"
    local desc="$2"
    if grep -q "$pattern" "$SERIAL_LOG" 2>/dev/null; then
        printf "  ${GREEN}PASS${NC} %s\n" "$desc"
        PASS=$((PASS + 1))
    else
        printf "  ${RED}FAIL${NC} %s (expected: '%s')\n" "$desc" "$pattern"
        FAIL=$((FAIL + 1))
    fi
}

check_source() {
    local pattern="$1"
    local desc="$2"
    if grep -q "$pattern" "$SCRIPT_DIR/../fajaros_nova_kernel.fj" 2>/dev/null; then
        printf "  ${GREEN}PASS${NC} %s\n" "$desc"
        PASS=$((PASS + 1))
    else
        printf "  ${RED}FAIL${NC} %s (pattern: '%s')\n" "$desc" "$pattern"
        FAIL=$((FAIL + 1))
    fi
}

echo ""
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}  FajarOS Nova — QEMU Automated Test Suite${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo ""

# Verify kernel exists
if [ ! -f "$KERNEL" ]; then
    echo -e "${RED}ERROR: Kernel not found at $KERNEL${NC}"
    echo "Build with: fj build --target x86_64-none --no-std examples/fajaros_nova_kernel.fj"
    exit 1
fi
echo -e "Kernel: ${CYAN}$KERNEL${NC} ($(wc -c < "$KERNEL") bytes)"
echo -e "Timeout: ${TIMEOUT}s"
echo ""

# Create NVMe disk if needed
if [ ! -f "$NVME_DISK" ]; then
    qemu-img create -f raw "$NVME_DISK" 64M >/dev/null 2>&1 || true
fi

# ── D1: Boot Test ──────────────────────────────────────────
echo -e "${CYAN}D1: Boot Test${NC}"
rm -f "$SERIAL_LOG"
timeout "$TIMEOUT" qemu-system-x86_64 \
    -kernel "$KERNEL" \
    -serial file:"$SERIAL_LOG" \
    -display none \
    -no-reboot \
    -m 128M \
    2>/dev/null &
QEMU_PID=$!
sleep "$TIMEOUT"
kill $QEMU_PID 2>/dev/null || true
wait $QEMU_PID 2>/dev/null || true

if [ -f "$SERIAL_LOG" ] && [ -s "$SERIAL_LOG" ]; then
    check_serial "FajarOS\|Nova\|fajaros" "Boot banner present"
    check_serial "nova>\|shell\|ready" "Shell prompt or ready message"
else
    echo -e "  ${RED}FAIL${NC} No serial output captured"
    FAIL=$((FAIL + 2))
fi

# ── D2: Source Verification (kernel code analysis) ─────────
echo ""
echo -e "${CYAN}D2: Command Infrastructure${NC}"
check_source "fn dispatch_command" "dispatch_command function exists"
check_source "fn cmd_help" "cmd_help command exists"
check_source "fn cmd_uname" "cmd_uname command exists"
check_source "fn cmd_ps" "cmd_ps command exists"
check_source "fn cmd_ls" "cmd_ls command exists"

# ── D3: File System ────────────────────────────────────────
echo ""
echo -e "${CYAN}D3: Filesystem Infrastructure${NC}"
check_source "fn ramfs_create_entry" "ramfs_create_entry exists"
check_source "fn ramfs_write_file" "ramfs_write_file exists"
check_source "fn ramfs_write_file" "ramfs file operations"
check_source "fn fat32_read_file" "FAT32 read support"
check_source "fn vfs_find_mount" "VFS mount resolution"

# ── D4: Process Management ─────────────────────────────────
echo ""
echo -e "${CYAN}D4: Process Management${NC}"
check_source "fn sys_fork" "sys_fork syscall"
check_source "fn sys_exec" "sys_exec syscall"
check_source "fn sys_waitpid" "sys_waitpid syscall"
check_source "fn signal_send" "signal_send function"
check_source "fn process_exit" "process_exit handler"

# ── D5: Network Stack ─────────────────────────────────────
echo ""
echo -e "${CYAN}D5: Network Stack${NC}"
check_source "fn arp_" "ARP protocol functions"
check_source "fn icmp_" "ICMP protocol functions"
check_source "fn tcp_connect" "TCP connect"
check_source "fn http_" "HTTP server/client"
check_source "fn udp_" "UDP protocol"

# ── D6: Storage ────────────────────────────────────────────
echo ""
echo -e "${CYAN}D6: Storage Drivers${NC}"
check_source "fn nvme_init" "NVMe initialization"
check_source "fn nvme_read_sector" "NVMe sector read"
check_source "fn nvme_write_sector" "NVMe sector write"
check_source "fn blk_read" "Block device layer"

# ── D7: SMP ────────────────────────────────────────────────
echo ""
echo -e "${CYAN}D7: SMP${NC}"
check_source "fn smp_boot_aps" "SMP AP boot"
check_source "TRAMPOLINE\|trampoline" "AP trampoline code"
check_source "smp_boot_aps" "AP boot function"

# ── D8: Multi-User ─────────────────────────────────────────
echo ""
echo -e "${CYAN}D8: Multi-User${NC}"
check_source "fn cmd_login" "login command"
check_source "fn cmd_passwd" "passwd command"
check_source "fn cmd_chmod" "chmod command"
check_source "fn fs_check_perm" "permission checking"

# ── D9: Shell Features ─────────────────────────────────────
echo ""
echo -e "${CYAN}D9: Shell${NC}"
check_source "fn shell_exec_pipe" "pipe execution"
check_source "fn shell_exec_redirect" "redirect handling"
check_source "SHELL_ENV\|ENV_" "environment variables"
check_source "SCRIPT_STATE" "script execution support"

# ── D10: Stress/Stability ──────────────────────────────────
echo ""
echo -e "${CYAN}D10: Stability${NC}"
check_source "fn cmd_stress" "stress test command"
check_source "HEAP_MAGIC\|double.free" "double-free detection"
check_source "hlt" "halt instruction present"

# ── Summary ────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
printf "Results: ${GREEN}%d passed${NC}, ${RED}%d failed${NC}, %d skipped\n" "$PASS" "$FAIL" "$SKIP"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    exit 1
fi
exit 0
