#!/bin/bash
# FajarOS Nova — Module Concatenation Build
#
# Concatenates all module .fj files into a single kernel source.
# This is the build method until the Fajar Lang compiler supports
# multi-file `use` imports for bare-metal targets.
#
# Usage:
#   ./build.sh                    # concatenate only
#   ./build.sh --compile          # concatenate + native compile
#   ./build.sh --check            # concatenate + type check

set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
OUTPUT="$SCRIPT_DIR/../fajaros_nova_kernel_built.fj"
FJ="$PROJECT_DIR/target/release/fj"

# Module order matters — constants and helpers must come before users
MODULES=(
    memory.fj       # Frame allocator, page tables, heap (585 lines)
    ipc.fj          # Message queues (110 lines)
    security.fj     # Users, permissions (55 lines)
    smp.fj          # SMP, AP trampoline (293 lines)
    nvme.fj         # NVMe driver, block devices (737 lines)
    fat32.fj        # FAT32 filesystem (750 lines)
    vfs.fj          # VFS, ramfs, devfs, procfs (322 lines)
    network.fj      # TCP, UDP, ARP, HTTP (1,458 lines)
    syscall.fj      # Syscall dispatch (410 lines)
    process.fj      # Fork, exec, scheduler (780 lines)
    shell_core.fj   # Shell engine, pipes, redirects (4,500 lines)
    shell_commands.fj # 240+ commands (4,000 lines)
    services.fj     # Init, syslog, packages (3,100 lines)
    extensions.fj   # Late additions (4,111 lines)
)

echo "[nova] Concatenating ${#MODULES[@]} modules..."
> "$OUTPUT"
for mod in "${MODULES[@]}"; do
    if [ -f "$SCRIPT_DIR/$mod" ]; then
        echo "// ═══ Module: $mod ═══" >> "$OUTPUT"
        cat "$SCRIPT_DIR/$mod" >> "$OUTPUT"
        echo "" >> "$OUTPUT"
    else
        echo "WARNING: $mod not found"
    fi
done

LINES=$(wc -l < "$OUTPUT")
echo "[nova] Built: $OUTPUT ($LINES lines)"

if [ "$1" = "--check" ]; then
    echo "[nova] Type checking..."
    "$FJ" check "$OUTPUT"
fi

if [ "$1" = "--compile" ]; then
    echo "[nova] Native compiling..."
    "$FJ" build --target x86_64-none --no-std "$OUTPUT"
fi
