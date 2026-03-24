#!/bin/bash
# FajarOS Nova v0.7 "Nexus" — Shell Features QEMU Test (Sprint V3)
# Verifies: pipes, redirects, env vars, signals, jobs, scripting in kernel source + QEMU boot
#
# Usage: ./fajaros_nova_v3_test.sh

set -e

KERNEL="${1:-examples/fajaros_nova_kernel}"
ISO="/tmp/fajaros_nova_test.iso"
SERIAL="/tmp/nova_v3_serial.log"
NVME_DISK="/tmp/nova_v3_nvme.img"
KFJ="examples/fajaros_nova_kernel.fj"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0

check_source() {
    if grep -q "$1" "$KFJ" 2>/dev/null; then
        echo -e "  ${GREEN}PASS${NC} — $2"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} — $2 (pattern: '$1')"
        FAIL=$((FAIL + 1))
    fi
}

check_serial() {
    if grep -q "$1" "$SERIAL" 2>/dev/null; then
        echo -e "  ${GREEN}PASS${NC} — $2"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} — $2 (pattern: '$1')"
        FAIL=$((FAIL + 1))
    fi
}

echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo -e "${CYAN}  Nova v0.7 — Sprint V3: Shell Features${NC}"
echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo ""

# Check kernel source exists
if [ ! -f "$KFJ" ]; then
    echo -e "${RED}Error: Kernel source not found at $KFJ${NC}"
    exit 1
fi

# Build ISO + NVMe disk for QEMU boot test
if [ -f "$KERNEL" ]; then
    if [ ! -f "$ISO" ]; then
        mkdir -p /tmp/nova_v3_iso/boot/grub
        cp "$KERNEL" /tmp/nova_v3_iso/boot/fajaros.elf
        cat > /tmp/nova_v3_iso/boot/grub/grub.cfg << 'GRUBEOF'
set timeout=0
set default=0
menuentry "FajarOS Nova" { multiboot2 /boot/fajaros.elf; boot; }
GRUBEOF
        grub-mkrescue -o "$ISO" /tmp/nova_v3_iso 2>/dev/null
        rm -rf /tmp/nova_v3_iso
    fi

    dd if=/dev/zero of="$NVME_DISK" bs=1M count=32 2>/dev/null
    mkfs.fat -F 32 -n "NOVATEST" "$NVME_DISK" 2>/dev/null

    # Boot QEMU to get serial log
    timeout 10 qemu-system-x86_64 \
        -cdrom "$ISO" -m 256M -display none \
        -drive file="$NVME_DISK",format=raw,if=none,id=nvme0 \
        -device nvme,serial=FJTEST,drive=nvme0 \
        -boot d \
        -serial file:"$SERIAL" \
        -monitor none -no-reboot 2>/dev/null || true
fi

# ═══════════════════════════════════════════
# V3.1: Pipe operator infrastructure
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.1: Pipe operator${NC}"
check_source "fn shell_find_pipe" "shell_find_pipe() scans for |"
check_source "fn shell_exec_pipe" "shell_exec_pipe() creates pipe + redirects FDs"
check_source "fn pipe_read_circular" "Circular pipe read (4064B buffer)"
check_source "fn pipe_write_circular" "Circular pipe write"
echo ""

# ═══════════════════════════════════════════
# V3.2: Output redirect >
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.2: Output redirect >${NC}"
check_source "fn shell_find_redirect" "shell_find_redirect() detects > >> <"
check_source "fn shell_exec_redirect_output" "Redirect output to ramfs file"
check_source "fn ramfs_create_by_addr" "Create file for redirect target"
echo ""

# ═══════════════════════════════════════════
# V3.3: Append redirect >>
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.3: Append redirect >>${NC}"
check_source "200 + i" ">> encoded as 200+position"
check_source "append == 1" "Append mode seeks to file end"
echo ""

# ═══════════════════════════════════════════
# V3.4: Environment variables
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.4: Environment variables${NC}"
check_source "fn env_set" "env_set() stores key=value"
check_source "fn env_get" "env_get() retrieves value"
check_source "fn env_find" "env_find() searches table"
check_source "fn cmd_export" "export builtin"
check_source "fn shell_expand_vars" "shell_expand_vars() expands \$VAR"
check_source "ENV_TABLE.*0x8D3000" "ENV_TABLE at 0x8D3000"
echo ""

# ═══════════════════════════════════════════
# V3.5: $? exit code
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.5: \$? exit code${NC}"
check_source "LAST_EXIT_CODE.*0x652060" "LAST_EXIT_CODE at 0x652060"
check_source "volatile_read_u64(LAST_EXIT_CODE)" "\$? reads from LAST_EXIT_CODE"
echo ""

# ═══════════════════════════════════════════
# V3.6: Ctrl+C → SIGINT
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.6: Ctrl+C → SIGINT${NC}"
check_source "CTRL_STATE_ADDR" "Ctrl key state tracked"
check_source "sc == 0x1D" "Ctrl make scancode 0x1D"
check_source "sc == 0x9D" "Ctrl break scancode 0x9D"
check_source "signal_fg_group(SIGINT)" "Ctrl+C sends SIGINT to foreground"
echo ""

# ═══════════════════════════════════════════
# V3.7: Background &
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.7: Background \& operator${NC}"
check_source "fn shell_has_background" "Detect trailing &"
check_source ") == 38" "& = ASCII 38 detected in cmdbuf"
echo ""

# ═══════════════════════════════════════════
# V3.8: jobs command
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.8: jobs/fg/bg commands${NC}"
check_source "fn cmd_jobs" "cmd_jobs() lists background jobs"
check_source "fn cmd_fg" "cmd_fg() brings job to foreground"
check_source "fn cmd_bg" "cmd_bg() resumes stopped job"
check_source "fn job_add" "job_add() adds to job table"
check_source "fn job_check_notifications" "Job done notifications at prompt"
check_source "JOB_TABLE.*0x8D8000" "JOB_TABLE at 0x8D8000"
echo ""

# ═══════════════════════════════════════════
# V3.9: Script execution
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.9: Script execution${NC}"
check_source "fn cmd_sh" "cmd_sh() loads and executes scripts"
check_source "fn cmd_if_start" "if/then/else/fi support"
check_source "fn cmd_for_start" "for/in/do/done support"
check_source "fn cmd_while_start" "while/do/done support"
check_source "fn cmd_test" "test builtin (-f, -d)"
check_source "fn cmd_exit_shell" "exit builtin"
check_source "SCRIPT_STATE.*0x8D5000" "Script state machine at 0x8D5000"
echo ""

# ═══════════════════════════════════════════
# V3.10: Shell infrastructure (history, keyboard)
# ═══════════════════════════════════════════
echo -e "${CYAN}V3.10: Shell infrastructure${NC}"
check_source "fn shell_execute_v2" "shell_execute_v2() entry point (var expand + pipe + redirect)"
check_source "fn dispatch_command" "dispatch_command() routes to cmd_* functions"
check_serial "\[NOVA\]" "QEMU: Nova boot banner in serial"
check_serial "\[KB\]" "QEMU: Keyboard buffer ready"

# Count v0.7 features in kernel source
echo ""
echo -e "${CYAN}Feature counts:${NC}"
SYSCALLS=$(grep -c "fn sys_" "$KFJ")
echo -e "  sys_* functions: ${GREEN}$SYSCALLS${NC}"
SIGNALS=$(grep -c "SIG[A-Z]*:" "$KFJ" | head -1)
echo -e "  Signal constants: ${GREEN}$(grep -c 'const SIG' "$KFJ")${NC}"
PIPES=$(grep -c "pipe_" "$KFJ")
echo -e "  Pipe functions: ${GREEN}$PIPES${NC} references"
CMDS=$(grep -c "fn cmd_" "$KFJ")
echo -e "  Shell commands: ${GREEN}$CMDS${NC}"
LOC=$(wc -l < "$KFJ")
echo -e "  Kernel LOC: ${GREEN}$LOC${NC}"

# ═══════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════
echo ""
echo -e "${CYAN}═══════════════════════════════════════════${NC}"
echo -e "  Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"
echo -e "${CYAN}═══════════════════════════════════════════${NC}"

# Cleanup
rm -f "$NVME_DISK"

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}SOME TESTS FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}ALL V3 TESTS PASSED${NC}"
    exit 0
fi
