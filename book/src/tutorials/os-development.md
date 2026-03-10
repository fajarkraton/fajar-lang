# Write a Kernel Module

This tutorial shows how to write bare-metal code with Fajar Lang's `@kernel` context.

## Kernel Context

The `@kernel` annotation restricts code to safe OS primitives only:

```fajar
@kernel fn init_system() {
    // Allowed: memory management, IRQ, syscalls, port I/O
    let page = page_map(0x1000, PageFlags::ReadWrite)
    irq_register(0x21, keyboard_handler)

    // Forbidden (compile-time error):
    // let s = String::new()     // KE001: no heap in kernel
    // let t = zeros(3, 3)       // KE002: no tensor in kernel
}
```

## Memory Management

```fajar
@kernel fn setup_memory() {
    // Allocate physical page
    let ptr = mem_alloc(4096)

    // Map virtual address
    page_map(0xB8000, PageFlags::ReadWrite)

    // Direct memory access
    mem_write(ptr, 0, 0xDEADBEEF)
    let val = mem_read(ptr, 0)
}
```

## Interrupt Handlers

```fajar
@kernel fn keyboard_handler() {
    let scancode = port_read(0x60)
    // Process key press
}

@kernel fn init() {
    irq_register(0x21, keyboard_handler)
    irq_enable(0x21)
}
```

## System Calls

```fajar
@kernel fn setup_syscalls() {
    syscall_define(1, sys_write)
    syscall_define(2, sys_read)
}

@kernel fn sys_write(fd: i64, buf: ptr, len: i64) -> i64 {
    // Write to file descriptor
    len
}
```

## Bare Metal Entry Point

```fajar
#[no_std]
#[panic_handler]
fn panic_handler() { loop {} }

@entry
@kernel fn _start() {
    // First code to run after boot
    setup_memory()
    init_interrupts()
    main_loop()
}
```

## Cross-Compilation

Build for embedded targets:

```bash
fj build --target aarch64 --no-std kernel.fj   # ARM64
fj build --target riscv64 --no-std kernel.fj   # RISC-V
```

## Linker Scripts

Control memory layout:

```fajar
@section(".text.boot")
@entry
@kernel fn _start() {
    // Placed at boot address
}

@section(".bss")
const STACK_SIZE: usize = 4096
```
