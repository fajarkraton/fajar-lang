# Write a Kernel Module in Fajar Lang

Learn to build OS-level code using Fajar Lang's `@kernel` context: memory management, interrupts, and syscalls.

## Prerequisites

- Fajar Lang installed
- Understanding of basic OS concepts (virtual memory, interrupts)
- (Optional) QEMU for testing

## Step 1: Understanding Context Annotations

Fajar Lang uses annotations to enforce safety at compile time:

```fajar
@kernel   // OS primitives only — no heap, no tensor, no strings
@device   // ML/tensor ops only — no raw pointers, no IRQ
@safe     // Default — safest subset, no hardware access
@unsafe   // Full access to everything
```

The `@kernel` context is for OS code: it prevents accidental use of heap-allocated types (tensors, strings) that could cause problems in interrupt handlers or boot code.

## Step 2: Memory Management

Fajar Lang provides a simulated memory manager for OS development:

```fajar
// Allocate a page-aligned memory region
let addr = mem_alloc(4096)  // Allocate 4KB

// Write to memory
mem_write(addr, 42)

// Read from memory
let value = mem_read(addr)

// Free the memory
mem_free(addr, 4096)
```

### Page Table Operations

```fajar
// Map a virtual page to a physical frame
page_map(0x1000, 0x80000, "rw")  // virt, phys, flags

// Unmap a virtual page
page_unmap(0x1000)
```

### Memory Operations

```fajar
// Copy memory regions
memory_copy(dst_addr, src_addr, 256)  // 256 bytes

// Fill memory with a value
memory_set(addr, 0, 4096)  // Zero out 4KB

// Compare memory regions
let equal = memory_compare(addr1, addr2, 64)
```

## Step 3: Interrupt Handling

Register handlers for hardware interrupts:

```fajar
// Define an interrupt handler
fn timer_handler() -> void {
    // Handle timer tick
    let count = mem_read(0x1000)
    mem_write(0x1000, count + 1)
}

fn keyboard_handler() -> void {
    // Read scancode from keyboard port
    let scancode = port_read(0x60)
    // Process the key...
}

// Register handlers
irq_register(0x20, "timer_handler")     // Timer IRQ
irq_register(0x21, "keyboard_handler")  // Keyboard IRQ

// Enable interrupts
irq_enable()
```

### Priority-Based Interrupts

Handlers can have priority levels to support nested interrupts:

- **CRITICAL (255)**: Non-maskable, safety-critical
- **HIGH (192)**: Time-critical (timer, DMA complete)
- **NORMAL (128)**: Standard I/O (keyboard, UART)
- **LOW (0)**: Background tasks

A higher-priority interrupt can preempt a lower-priority handler.

## Step 4: System Calls

Define and dispatch system calls:

```fajar
// Define a syscall handler
fn sys_write(fd: i64, buf: i64, len: i64) -> i64 {
    // Write buffer contents to file descriptor
    // ... implementation ...
    len  // Return bytes written
}

// Register the syscall
syscall_define(1, "sys_write")  // syscall #1 = write

// Dispatch from user mode
let result = syscall_dispatch(1, 1, buf_addr, 64)  // write(stdout, buf, 64)
```

## Step 5: Port I/O

Direct hardware communication via port I/O:

```fajar
// Write to a hardware port
port_write(0x3F8, 65)  // Write 'A' to COM1

// Read from a hardware port
let status = port_read(0x3FD)  // Read COM1 status register

// PL011 UART (ARM, QEMU virt)
const uart_base: i64 = 150994944  // 0x09000000
fn uart_putc(c: i64) -> void {
    port_write(uart_base, c)
}
```

## Step 6: Cross-Domain Bridge Pattern

The real power of Fajar Lang: safely combining OS and ML in one program.

```fajar
@kernel fn read_sensor() -> i64 {
    // Read raw sensor data via memory-mapped I/O
    let raw = port_read(0x40000000)
    raw
}

@device fn classify(input: Tensor) -> i64 {
    // Run ML inference
    let output = tensor_softmax(tensor_matmul(input, weights))
    tensor_argmax(output)
}

@safe fn main() {
    // Bridge: kernel reads sensor, device runs inference
    let raw = read_sensor()
    let input = tensor_from_data([to_float(raw), 0.0, 0.0, 0.0], [1, 4])
    let class = classify(input)

    if class == 2 {
        // Alert! Activate hardware response
        port_write(0x50000000, 1)  // Turn on buzzer
    }
}
```

The compiler enforces isolation:
- `@kernel` code cannot use tensors or heap allocation
- `@device` code cannot use raw pointers or IRQ
- `@safe` code can call both but cannot do hardware access directly

## Step 7: Testing with QEMU

Cross-compile and test on QEMU:

```bash
# Compile for aarch64
fj build --target aarch64-unknown-linux-gnu kernel_module.fj

# Link with cross-compiler
aarch64-linux-gnu-gcc -static -o kernel_test kernel_module.o rt_entry.c

# Run on QEMU
qemu-aarch64 ./kernel_test
```

## Complete Example

See `examples/memory_map.fj` for a working memory management example:

```bash
fj run examples/memory_map.fj
```

## Key Takeaways

1. **Context annotations** prevent entire classes of bugs at compile time
2. **No runtime overhead** — context checks happen at compile time only
3. **Cross-domain bridges** safely combine OS and ML code
4. **QEMU testing** validates your code runs on real architectures
5. **Familiar patterns** — if you know Rust or C kernel development, Fajar Lang feels natural
