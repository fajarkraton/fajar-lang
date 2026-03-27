# OS Development Guide

Fajar Lang is designed for writing operating systems. The `@kernel` context
annotation gives you access to hardware primitives while the compiler
enforces safety rules. This guide covers building an OS kernel.

## Context Annotations

The `@kernel` context enables bare-metal operations:

```fajar
@kernel
fn init_memory() {
    let page = page_alloc(4096)
    page_map(page, 0x1000, PAGE_PRESENT | PAGE_WRITE)
}
```

What `@kernel` allows:
- Raw pointer operations (`*mut T`, `*const T`)
- Memory allocation (`alloc!`, `page_alloc`, `page_map`)
- Interrupt handling (`irq_register`, `irq_enable`)
- Port I/O (`port_read`, `port_write`)
- System call definitions

What `@kernel` forbids:
- Heap allocations (`String::new()`, `Vec::new()`) -- KE001
- Tensor operations (`zeros()`, `matmul()`) -- KE002
- Calling `@device` functions -- KE003

## Minimal Kernel

```fajar
@kernel
fn kernel_main() {
    // Initialize serial for output
    serial_init(0x3F8)
    serial_write(0x3F8, "Fajar OS booting...\n")

    // Set up GDT and IDT
    gdt_init()
    idt_init()

    // Enable interrupts
    irq_enable()

    // Initialize memory management
    let heap_start = 0x100000 as *mut u8
    let heap_size = 16 * 1024 * 1024  // 16 MB
    memory_init(heap_start, heap_size)

    serial_write(0x3F8, "Kernel ready.\n")

    // Enter idle loop
    loop {
        halt()
    }
}
```

## Memory Management

```fajar
@kernel
fn setup_paging() {
    // Allocate a page table
    let pml4 = page_alloc(4096)
    memory_set(pml4, 0, 4096)

    // Identity-map the first 2MB
    let pdpt = page_alloc(4096)
    let pd = page_alloc(4096)

    // Map: virtual 0x0 -> physical 0x0 (2MB huge page)
    page_map_huge(pd, 0, 0x0, PAGE_PRESENT | PAGE_WRITE | PAGE_HUGE)

    // Load the new page table
    set_cr3(pml4 as u64)
}
```

## Interrupt Handling

```fajar
@kernel
fn setup_interrupts() {
    irq_register(0, timer_handler)      // PIT timer
    irq_register(1, keyboard_handler)   // Keyboard
    irq_register(14, disk_handler)      // IDE disk
    irq_enable()
}

@kernel
fn timer_handler() {
    // Preemptive scheduling: switch to next task
    scheduler_tick()
    irq_eoi(0)  // End of interrupt
}

@kernel
fn keyboard_handler() {
    let scancode = port_read(0x60) as u8
    process_key(scancode)
    irq_eoi(1)
}
```

## System Calls

Define system calls for user-space programs:

```fajar
@kernel
fn syscall_dispatch(num: u64, arg1: u64, arg2: u64) -> u64 {
    match num {
        0 => sys_exit(arg1 as i32),
        1 => sys_write(arg1, arg2),
        2 => sys_read(arg1, arg2),
        3 => sys_fork(),
        4 => sys_exec(arg1, arg2),
        _ => { return -1 as u64 }
    }
}
```

## Testing with QEMU

Build and run your kernel in QEMU:

```bash
fj build --target x86_64-bare
qemu-system-x86_64 -kernel target/kernel.bin -serial stdio -no-reboot
```

For debugging:

```bash
qemu-system-x86_64 -kernel target/kernel.bin -s -S &
gdb -ex "target remote :1234" -ex "symbol-file target/kernel.elf"
```

## Cross-Domain Bridge

Connect kernel code to ML inference safely:

```fajar
@kernel fn read_sensor() -> [f32; 4] {
    let raw = [
        port_read(0x200) as f32,
        port_read(0x201) as f32,
        port_read(0x202) as f32,
        port_read(0x203) as f32,
    ]
    raw
}

@device fn classify(data: Tensor) -> i32 {
    let model = load_quantized("model.bin")
    argmax(model.forward(data))
}

@safe fn bridge() -> Action {
    let raw = read_sensor()
    let tensor = Tensor::from_slice(raw)
    let class = classify(tensor)
    Action::from_class(class)
}
```

## FajarOS Reference

For a complete OS implementation, see:
- **FajarOS Nova** (x86_64): `examples/fajaros_nova_kernel.fj` -- 20K+ lines
- **FajarOS Surya** (ARM64): github.com/fajarkraton/fajar-os
