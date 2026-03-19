# FajarOS Nova — Command Reference

## System Commands (19)

| Command | Usage | Description |
|---------|-------|-------------|
| `help` | `help` | Show all commands grouped by category |
| `version` | `version` | Detailed kernel version info |
| `about` | `about` | About FajarOS Nova (vision, author, links) |
| `uname` | `uname` | System identification string |
| `sysinfo` | `sysinfo` | Comprehensive system info (CPU, memory, PCI, uptime) |
| `uptime` | `uptime` | Show ticks and elapsed seconds |
| `date` | `date` | Uptime in H:MM:SS format |
| `hostname` | `hostname` | Print hostname ("fajaros-nova") |
| `whoami` | `whoami` | Print current user ("root") |
| `arch` | `arch` | Print architecture ("x86_64") |
| `dmesg` | `dmesg` | Simulated boot log with timestamps |
| `env` | `env` | Show environment variables |
| `printenv` | `printenv [VAR]` | Print specific env variable |
| `id` | `id` | Print user/group info |
| `cal` | `cal` | Show March 2026 calendar |
| `history` | `history` | Show command history (up to 8) |
| `man` | `man <cmd>` | Manual page (ls, cat, ps) |
| `which` | `which <cmd>` | Show command location |
| `banner` | `banner <text>` | ASCII art banner with text |

## Hardware Commands (8)

| Command | Usage | Description |
|---------|-------|-------------|
| `cpuinfo` | `cpuinfo` | CPU features from CPUID (FPU, SSE, AVX2, APIC) |
| `meminfo` | `meminfo` | Memory layout (kernel, heap, stack) |
| `free` | `free` | Detailed memory usage |
| `nproc` | `nproc` | CPU core count from ACPI MADT |
| `lspci` | `lspci` | List PCI devices (vendor:device + class) |
| `acpi` | `acpi` | ACPI info (RSDP address, CPU count, TSC) |
| `tsc` | `tsc` | Read x86 timestamp counter (hex + decimal) |
| `time` | `time` | Benchmark 100K loop with cycle count |

## Process Commands (6)

| Command | Usage | Description |
|---------|-------|-------------|
| `ps` | `ps` | List processes (PID, state, ticks) |
| `top` | `top` | Detailed process view with CPU% |
| `kill` | `kill <pid>` | Kill process by PID (sets state to dead) |
| `sleep` | `sleep <N>` | Busy-wait N seconds (max 30) |
| `reboot` | `reboot` | Reboot via keyboard controller (0x64←0xFE) |
| `shutdown` | `shutdown` | ACPI power-off |

## File Commands (22)

| Command | Usage | Description |
|---------|-------|-------------|
| `ls` | `ls` | List all files (type, size, name) |
| `dir` | `dir` | Alias for `ls` |
| `cat` | `cat <file>` | Print file contents |
| `more` | `more <file>` | Alias for `cat` |
| `touch` | `touch <file>` | Create empty file |
| `rm` | `rm <file>` | Delete file |
| `cp` | `cp <src> <dst>` | Copy file |
| `mv` | `mv <src> <dst>` | Rename/move file |
| `mkdir` | `mkdir <dir>` | Create directory |
| `rmdir` | `rmdir <dir>` | Remove directory |
| `pwd` | `pwd` | Print working directory ("/") |
| `write` | `write <file> <text>` | Write text to file (overwrite) |
| `append` | `append <file> <text>` | Append text to file |
| `head` | `head <file>` | Show first 5 lines |
| `tail` | `tail <file>` | Show last 5 lines |
| `wc` | `wc <file>` | Count lines, words, bytes |
| `grep` | `grep <pattern> <file>` | Search for pattern in file |
| `sort` | `sort <file>` | Sort file lines alphabetically |
| `uniq` | `uniq <file>` | Suppress adjacent duplicate lines |
| `nl` | `nl <file>` | Number lines |
| `cut` | `cut <N> <file>` | Print first N chars of each line |
| `strings` | `strings <file>` | Print printable strings (≥4 chars) |

## File Info Commands (7)

| Command | Usage | Description |
|---------|-------|-------------|
| `stat` | `stat <file>` | File type, size, data address, inode index |
| `xxd` | `xxd <file>` | Hex dump (offset + hex + ASCII, max 128 bytes) |
| `md5` | `md5 <file>` | Hash checksum (custom hash, not real MD5) |
| `df` | `df` | Filesystem disk free (size, used, avail, %) |
| `du` | `du` | Per-file disk usage with total |
| `count` | `count` | Count files and directories |
| `dd` | `dd` | Block device info (ramfs blocks) |

## AI / Compute Commands (4)

| Command | Usage | Description |
|---------|-------|-------------|
| `tensor` | `tensor` | 3×3 matrix multiply demo (A×I, timed with rdtsc) |
| `mnist` | `mnist` | MNIST inference simulation (784→128→10 MLP, cycle timing) |
| `bench` | `bench` | Run benchmarks (fib(30), 1MB write, 4×4 matmul) |
| `fib` | `fib <N>` | Compute fibonacci(N) with cycle timing (max 90) |

## Math / Text Commands (10)

| Command | Usage | Description |
|---------|-------|-------------|
| `calc` | `calc <a> <op> <b>` | Calculator: `+` `-` `*` `/` `%` |
| `hex` | `hex <N>` | Convert decimal to hexadecimal |
| `base` | `base <N> <radix>` | Convert to binary (2) or hex (16) |
| `factor` | `factor <N>` | Prime factorization |
| `prime` | `prime <N>` | Primality test |
| `len` | `len <text>` | Print text length |
| `echo` | `echo <text>` | Echo text to console |
| `rev` | `rev <text>` | Reverse text |
| `upcase` | `upcase <text>` | Convert to UPPERCASE |
| `downcase` | `downcase <text>` | Convert to lowercase |

## Text Processing Commands (4)

| Command | Usage | Description |
|---------|-------|-------------|
| `tr` | `tr <a> <b> <text>` | Replace character a with b in text |
| `grep` | `grep <pat> <file>` | Search file (see File Commands) |
| `sort` | `sort <file>` | Sort lines (see File Commands) |
| `uniq` | `uniq <file>` | Unique lines (see File Commands) |

## Utility Commands (17)

| Command | Usage | Description |
|---------|-------|-------------|
| `clear` | `clear` | Clear VGA screen |
| `cls` | `cls` | Alias for `clear` |
| `exit` | `exit` | ACPI shutdown |
| `seq` | `seq [N]` | Print 1 to N (default 10, max 1000) |
| `true` | `true` | Do nothing (exit success) |
| `false` | `false` | Do nothing |
| `yes` | `yes` | Print "y" 20 times |
| `dice` | `dice` | Roll a die (1-6, based on timer ticks) |
| `logo` | `logo` | FajarOS ASCII art logo |
| `splash` | `splash` | NOVA ASCII art splash screen |
| `color` | `color` | Color test pattern (16 background colors) |
| `cowsay` | `cowsay [msg]` | ASCII cow with message |
| `fortune` | `fortune` | Random quote (8 quotes, tick-based) |
| `repeat` | `repeat <N> <text>` | Repeat text N times (max 100) |
| `alias` | `alias` | Show command aliases |
| `motd` | `motd` | Message of the day |
| `set` | `set` | Show shell settings (shift, caps, history, files) |

## Process Management (5)

| Command | Usage | Description |
|---------|-------|-------------|
| `spawn` | `spawn <name>` | Create process in process table |
| `wait` | `wait <pid>` | Wait for process exit (1s simulated) |
| `nice` | `nice` | Show process priorities |
| `demo` | `demo` | Interactive OS demo (5 stages) |
| `neofetch` | `neofetch` | System info with ASCII art + colors |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| **Enter** | Execute command |
| **Backspace** | Delete last character |
| **Up Arrow** | Previous command (history) |
| **Down Arrow** | Next command (history) |
| **Tab** | Insert 4 spaces |
| **Shift** | Uppercase letters + symbols (!@#$%^&*...) |
| **CapsLock** | Toggle uppercase letters |

---

*FajarOS Nova Command Reference v1.0 — 102 commands — March 2026*
