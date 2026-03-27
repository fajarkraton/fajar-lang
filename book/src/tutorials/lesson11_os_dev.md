# Lesson 11: OS Development

## Objectives

By the end of this lesson, you will be able to:

- Understand context annotations (`@kernel`, `@device`, `@safe`, `@unsafe`)
- Write `@kernel` functions for bare-metal programming
- Use low-level primitives: memory allocation, port I/O, interrupts
- Understand the bridge pattern between `@kernel` and `@device`

## Context Annotations

Fajar Lang's unique feature is **context-aware compilation**. Different parts of your code run in different safety contexts, and the compiler enforces strict isolation between them.

| Context | Purpose | Allowed | Forbidden |
|---------|---------|---------|-----------|
| `@safe` | Default application code | Basic types, functions, control flow | Raw pointers, hardware, tensor |
| `@kernel` | OS kernel, drivers | Raw pointers, memory, IRQ, port I/O | Heap allocation, tensors |
| `@device` | ML inference, GPU | Tensors, neural network ops | Raw pointers, IRQ |
| `@unsafe` | Escape hatch | Everything | Nothing forbidden |

## Your First @kernel Function

```fajar
@kernel
fn write_to_port(port: u16, value: u8) {
    port_write(port, value)
}

@kernel
fn read_from_port(port: u16) -> u8 {
    port_read(port)
}

@kernel
fn serial_init() {
    // COM1 port initialization
    write_to_port(0x3F8 + 1, 0x00)   // Disable interrupts
    write_to_port(0x3F8 + 3, 0x80)   // Enable DLAB
    write_to_port(0x3F8 + 0, 0x03)   // Baud rate divisor lo (38400)
    write_to_port(0x3F8 + 1, 0x00)   // Baud rate divisor hi
    write_to_port(0x3F8 + 3, 0x03)   // 8 bits, no parity, 1 stop
}
```

## Memory Management

In `@kernel` context, you manage memory directly:

```fajar
@kernel
fn allocate_page() -> ptr {
    let page = mem_alloc(4096)   // Allocate 4KB page
    mem_set(page, 0, 4096)       // Zero the page
    page
}

@kernel
fn free_page(page: ptr) {
    mem_free(page, 4096)
}

@kernel
fn write_value(addr: ptr, offset: i64, value: u32) {
    mem_write(addr + offset, value)
}

@kernel
fn read_value(addr: ptr, offset: i64) -> u32 {
    mem_read(addr + offset)
}
```

## Interrupt Handling

Register interrupt handlers for hardware events:

```fajar
@kernel
fn timer_handler() {
    // Called every timer tick
    let tick_count = read_value(0xFFFF0000 as ptr, 0)
    write_value(0xFFFF0000 as ptr, 0, tick_count + 1)
}

@kernel
fn setup_interrupts() {
    irq_disable()                        // Disable during setup
    irq_register(0, timer_handler)       // IRQ 0 = timer
    irq_enable()                         // Re-enable
}
```

## The Bridge Pattern

The key insight of Fajar Lang is that OS kernels and ML inference can coexist safely. The bridge pattern connects `@kernel` (hardware) to `@device` (ML):

```fajar
@kernel
fn read_sensor() -> [f32; 4] {
    // Read 4 sensor values from hardware registers
    let base: ptr = 0x40001000 as ptr
    [
        mem_read(base) as f32,
        mem_read(base + 4) as f32,
        mem_read(base + 8) as f32,
        mem_read(base + 12) as f32
    ]
}

@device
fn infer(data: Tensor) -> Tensor {
    // Run ML model on sensor data
    let hidden = relu(matmul(data, weights))
    softmax(matmul(hidden, output_weights))
}

@safe
fn process_sensor() -> str {
    // Bridge: safe code orchestrates kernel and device
    let raw = read_sensor()                    // @kernel call
    let tensor = from_data([raw])              // Convert to tensor
    let prediction = infer(tensor)             // @device call

    match argmax(prediction) {
        0 => "normal",
        1 => "warning",
        2 => "critical",
        _ => "unknown"
    }
}
```

The compiler enforces:
- `@kernel` cannot use tensors
- `@device` cannot use raw pointers
- `@safe` can call both but cannot use raw pointers or tensors directly

## Page Table Management

```fajar
@kernel
fn map_page(virt: u64, phys: u64, flags: u64) {
    let pte_addr = page_table_base() + (virt >> 12) * 8
    mem_write(pte_addr as ptr, phys | flags)
}

@kernel
fn unmap_page(virt: u64) {
    let pte_addr = page_table_base() + (virt >> 12) * 8
    mem_write(pte_addr as ptr, 0)
}
```

## Inline Assembly

For the most hardware-specific operations:

```fajar
@kernel
fn halt_cpu() {
    asm!("hlt")
}

@kernel
fn read_cr3() -> u64 {
    let val: u64
    asm!("mov {}, cr3", out(reg) val)
    val
}

@kernel
fn enable_interrupts() {
    asm!("sti")
}
```

## A Minimal Kernel Entry Point

```fajar
@kernel
fn kernel_main() {
    // Initialize serial for debug output
    serial_init()
    serial_print("Booting FajarOS...\n")

    // Set up memory
    let heap_start = allocate_page()
    serial_print("Heap initialized\n")

    // Set up interrupts
    setup_interrupts()
    serial_print("Interrupts ready\n")

    // Main loop
    loop {
        let cmd = serial_read_line()
        handle_command(cmd)
    }
}
```

## Exercises

### Exercise 11.1: Context Rules (*)

For each function call below, determine whether it would compile or produce an error. Write your answers as comments:

```fajar
@kernel fn k1() { mem_alloc(4096) }
@kernel fn k2() { zeros(3, 3) }
@device fn d1() { relu(ones(2, 2)) }
@device fn d2() { mem_alloc(4096) }
@safe   fn s1() { println("hello") }
@safe   fn s2() { mem_alloc(4096) }
```

**Expected answers:**

```
k1: OK (memory allocation is allowed in @kernel)
k2: ERROR KE002 (tensors forbidden in @kernel)
d1: OK (tensor ops allowed in @device)
d2: ERROR DE002 (memory allocation forbidden in @device)
s1: OK (println is allowed in @safe)
s2: ERROR (raw memory forbidden in @safe)
```

### Exercise 11.2: LED Blink (**)

Write a `@kernel` function that blinks an LED by toggling a GPIO register. The GPIO base address is `0x40020000`, the output register is at offset `0x14`, and bit 5 controls the LED. Write a `blink(count: i64, delay: i64)` function that toggles the LED `count` times.

**Expected pseudocode structure:**

```fajar
@kernel
fn blink(count: i64) {
    let gpio_base: ptr = 0x40020000 as ptr
    for i in 0..count {
        // Read current value, toggle bit 5, write back
        let val = mem_read(gpio_base + 0x14)
        mem_write(gpio_base + 0x14, val ^ (1 << 5))
        // Delay loop would go here
    }
}
```
