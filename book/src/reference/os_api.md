# OS API

Fajar Lang provides low-level OS primitives for kernel and systems development. These functions require `@kernel` or `@unsafe` context unless noted otherwise.

## Memory Management

### `mem_alloc(size: usize) -> *mut u8`

Allocates `size` bytes of raw memory. Returns a pointer to the allocated block.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn alloc_buffer() -> *mut u8 {
    let buf = mem_alloc(4096)  // allocate 4KB
    buf
}
```

### `mem_free(ptr: *mut u8) -> void`

Frees a previously allocated memory block.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn free_buffer(buf: *mut u8) {
    mem_free(buf)
}
```

### `mem_read(ptr: *const u8, offset: usize) -> u8`

Reads a byte from memory at `ptr + offset`.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn read_byte(ptr: *const u8) -> u8 {
    mem_read(ptr, 0)
}
```

### `mem_write(ptr: *mut u8, offset: usize, value: u8) -> void`

Writes a byte to memory at `ptr + offset`.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn write_byte(ptr: *mut u8, offset: usize, val: u8) {
    mem_write(ptr, offset, val)
}
```

## Page Management

### `page_map(virt: usize, phys: usize, flags: u64) -> void`

Maps a virtual address to a physical address with the given page flags.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn map_framebuffer() {
    let virt_addr: usize = 0xFFFF_8000_0000_0000
    let phys_addr: usize = 0x00000000_FD000000
    let flags: u64 = 0x03  // PRESENT | WRITABLE
    page_map(virt_addr, phys_addr, flags)
}
```

### `page_unmap(virt: usize) -> void`

Unmaps a virtual address, removing its page table entry.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn unmap_page(addr: usize) {
    page_unmap(addr)
}
```

## Additional Memory Operations

### `memory_copy(dst: *mut u8, src: *const u8, count: usize) -> void`

Copies `count` bytes from `src` to `dst`. Regions must not overlap.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn copy_buffer(dst: *mut u8, src: *const u8, len: usize) {
    memory_copy(dst, src, len)
}
```

### `memory_set(dst: *mut u8, value: u8, count: usize) -> void`

Sets `count` bytes at `dst` to the given value.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn zero_page(page: *mut u8) {
    memory_set(page, 0, 4096)
}
```

### `memory_compare(a: *const u8, b: *const u8, count: usize) -> i32`

Compares `count` bytes. Returns 0 if equal, negative if a < b, positive if a > b.

**Context:** `@kernel`, `@unsafe`

## Interrupt Management

### `irq_register(irq: u8, handler: fn() -> void) -> void`

Registers a handler function for the given IRQ number.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn setup_timer() {
    irq_register(0, timer_handler)
}

@kernel
fn timer_handler() {
    // handle timer tick
}
```

### `irq_unregister(irq: u8) -> void`

Removes the handler for the given IRQ number.

**Context:** `@kernel`, `@unsafe`

### `irq_enable(irq: u8) -> void`

Enables (unmasks) the given IRQ line.

**Context:** `@kernel`, `@unsafe`

### `irq_disable(irq: u8) -> void`

Disables (masks) the given IRQ line.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn configure_irqs() {
    irq_disable(1)    // disable keyboard IRQ
    // ... reconfigure ...
    irq_enable(1)     // re-enable keyboard IRQ
}
```

## System Calls

### `syscall_define(number: u64, handler: fn(args: [u64]) -> u64) -> void`

Registers a system call handler for the given syscall number.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn init_syscalls() {
    syscall_define(1, sys_write)
    syscall_define(60, sys_exit)
}

@kernel
fn sys_write(args: [u64]) -> u64 {
    let fd = args[0]
    let buf = args[1]
    let len = args[2]
    // write implementation
    len
}
```

### `syscall_dispatch(number: u64, args: [u64]) -> u64`

Dispatches a system call by number, invoking the registered handler.

**Context:** `@kernel`, `@unsafe`

## Port I/O

### `port_read(port: u16) -> u8`

Reads a byte from an I/O port.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn read_serial() -> u8 {
    port_read(0x3F8)  // COM1 data port
}
```

### `port_write(port: u16, value: u8) -> void`

Writes a byte to an I/O port.

**Context:** `@kernel`, `@unsafe`

```fajar
@kernel
fn write_serial(byte: u8) {
    port_write(0x3F8, byte)  // COM1 data port
}
```

## Context Requirements Summary

| Function | `@safe` | `@kernel` | `@device` | `@unsafe` |
|----------|---------|-----------|-----------|-----------|
| `mem_alloc` / `mem_free` | -- | OK | -- | OK |
| `mem_read` / `mem_write` | -- | OK | -- | OK |
| `page_map` / `page_unmap` | -- | OK | -- | OK |
| `irq_register` / `irq_enable` | -- | OK | -- | OK |
| `syscall_define` / `syscall_dispatch` | -- | OK | -- | OK |
| `port_read` / `port_write` | -- | OK | -- | OK |
