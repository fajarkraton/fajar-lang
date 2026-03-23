# FajarOS fj.toml — Multi-Binary Manifest

> Reference manifest for building FajarOS with `fj build --all`.
> To be placed in the fajaros-x86 repo root.

```toml
[package]
name = "fajaros-nova"
version = "3.0.0"
description = "FajarOS Nova — microkernel OS in Fajar Lang"
authors = ["Fajar <fajar@primecore.id>"]
license = "MIT"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
sources = [
    "kernel/boot/",
    "kernel/core/",
    "kernel/mm/",
    "kernel/ipc/",
    "kernel/sched/",
    "kernel/syscall/",
    "kernel/interrupts/",
    "kernel/security/",
    "kernel/hw/",
    "kernel/stubs/",
    "drivers/",
    "fs/",
    "shell/",
]
linker-script = "linker.ld"

[[service]]
name = "init"
entry = "services/init/main.fj"
target = "x86_64-user"

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"
target = "x86_64-user"

[[service]]
name = "blk"
entry = "services/blk/main.fj"
target = "x86_64-user"
sources = ["services/blk/"]

[[service]]
name = "net"
entry = "services/net/main.fj"
target = "x86_64-user"
sources = ["services/net/"]

[[service]]
name = "shell"
entry = "services/shell/main.fj"
target = "x86_64-user"

[[service]]
name = "display"
entry = "services/display/main.fj"
target = "x86_64-user"

[[service]]
name = "input"
entry = "services/input/main.fj"
target = "x86_64-user"

[[service]]
name = "gpu"
entry = "services/gpu/main.fj"
target = "x86_64-user"

[[service]]
name = "gui"
entry = "services/gui/main.fj"
target = "x86_64-user"

[[service]]
name = "auth"
entry = "services/auth/main.fj"
target = "x86_64-user"
```

## ARM64 Variant

For Radxa Dragon Q6A, add to the same fj.toml:

```toml
[kernel.arm64]
entry = "arch/aarch64/boot.fj"
target = "aarch64-unknown-none"
sources = ["kernel/core/", "arch/aarch64/"]

[[service]]
name = "npu"
entry = "services/npu/main.fj"
target = "aarch64-user"
```

## Build Commands

```bash
# Build all targets (kernel + 9 services)
fj build --all

# Build kernel only
fj build kernel/

# Build single service
fj build services/vfs/

# Pack services into initramfs
fj pack

# Full pipeline
fj build --all && fj pack
```
