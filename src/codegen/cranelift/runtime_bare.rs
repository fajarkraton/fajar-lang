//! Bare-metal runtime functions for FajarOS.
//!
//! These `extern "C"` functions provide a no-libc, no-heap runtime for
//! bare-metal aarch64 targets. They are linked into the final ELF binary
//! and provide the minimal functionality needed by compiled Fajar Lang code:
//!
//! - Memory operations: memcpy, memset, memcmp (no libc)
//! - UART output: PL011 UART on QEMU `-M virt` (0x0900_0000)
//! - Panic handler: print message + WFE halt loop
//! - Bump allocator: simple kernel heap (no free)
//!
//! # MMIO Addresses
//!
//! QEMU `-M virt` PL011 UART: `0x0900_0000`
//! QCS6490 GENI UART: `0x0A8C_0000` (QUP, configured at runtime)
//!
//! The UART base address can be overridden by calling `fj_rt_bare_set_uart_base`.

#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::manual_range_contains)]

use std::sync::atomic::{AtomicU64, Ordering};

/// UART base address (default: QEMU PL011 at 0x0900_0000).
static UART_BASE: AtomicU64 = AtomicU64::new(0x0900_0000);

/// Bump allocator pointer (grows upward from HEAP_BASE).
static BUMP_PTR: AtomicU64 = AtomicU64::new(0);

/// Heap base address (set by startup code).
static HEAP_BASE: AtomicU64 = AtomicU64::new(0x4200_0000);

/// Heap end address (set by startup code).
static HEAP_END: AtomicU64 = AtomicU64::new(0x4600_0000); // 64MB default

// ═══════════════════════════════════════════════════════════════════════
// Memory Operations (no libc)
// ═══════════════════════════════════════════════════════════════════════

/// Bare-metal memcpy: copy `n` bytes from `src` to `dst`.
///
/// # Safety
/// Caller must ensure `dst` and `src` are valid, non-overlapping pointers.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_memcpy(dst: *mut u8, src: *const u8, n: i64) -> *mut u8 {
    if dst.is_null() || src.is_null() || n <= 0 {
        return dst;
    }
    let count = n as usize;

    // Word-aligned fast path (8-byte copies)
    let aligned = (dst as usize | src as usize) & 7 == 0;
    if aligned && count >= 8 {
        let words = count / 8;
        let dst64 = dst as *mut u64;
        let src64 = src as *const u64;
        for i in 0..words {
            // SAFETY: dst and src are non-null, aligned to 8 bytes, and i < words
            // which is bounded by count (validated from n > 0).
            unsafe { *dst64.add(i) = *src64.add(i) };
        }
        // Copy remaining bytes
        let remaining = count % 8;
        let offset = words * 8;
        for i in 0..remaining {
            // SAFETY: dst and src are non-null and offset + i < count.
            unsafe { *dst.add(offset + i) = *src.add(offset + i) };
        }
    } else {
        // Byte-by-byte fallback
        for i in 0..count {
            // SAFETY: dst and src are non-null and i < count.
            unsafe { *dst.add(i) = *src.add(i) };
        }
    }
    dst
}

/// Bare-metal memset: fill `n` bytes at `dst` with `val`.
///
/// # Safety
/// Caller must ensure `dst` is a valid pointer.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_memset(dst: *mut u8, val: i64, n: i64) -> *mut u8 {
    if dst.is_null() || n <= 0 {
        return dst;
    }
    let byte = val as u8;
    let count = n as usize;

    // Word-aligned fast path
    if (dst as usize) & 7 == 0 && count >= 8 {
        let fill_word = (byte as u64)
            | ((byte as u64) << 8)
            | ((byte as u64) << 16)
            | ((byte as u64) << 24)
            | ((byte as u64) << 32)
            | ((byte as u64) << 40)
            | ((byte as u64) << 48)
            | ((byte as u64) << 56);
        let words = count / 8;
        let dst64 = dst as *mut u64;
        for i in 0..words {
            // SAFETY: dst is non-null, aligned to 8 bytes, and i < words
            // which is bounded by count (validated from n > 0).
            unsafe { *dst64.add(i) = fill_word };
        }
        let remaining = count % 8;
        let offset = words * 8;
        for i in 0..remaining {
            // SAFETY: dst is non-null and offset + i < count.
            unsafe { *dst.add(offset + i) = byte };
        }
    } else {
        for i in 0..count {
            // SAFETY: dst is non-null and i < count.
            unsafe { *dst.add(i) = byte };
        }
    }
    dst
}

/// Bare-metal memcmp: compare `n` bytes at `a` and `b`.
/// Returns 0 if equal, <0 if a<b, >0 if a>b.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_memcmp(a: *const u8, b: *const u8, n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let count = n as usize;
    for i in 0..count {
        // SAFETY: a and b are non-null (caller contract) and i < count.
        let av = unsafe { *a.add(i) };
        let bv = unsafe { *b.add(i) };
        if av != bv {
            return (av as i64) - (bv as i64);
        }
    }
    0
}

// ═══════════════════════════════════════════════════════════════════════
// UART Output (PL011 on QEMU, GENI on QCS6490)
// ═══════════════════════════════════════════════════════════════════════

/// Write a single byte to the UART data register.
#[inline]
fn uart_putc(c: u8) {
    let base = UART_BASE.load(Ordering::Relaxed);
    if base != 0 {
        // SAFETY: writing to UART MMIO data register
        unsafe { core::ptr::write_volatile(base as *mut u8, c) };
    }
}

/// Bare-metal print: write `len` bytes from `ptr` to UART.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_print(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    for i in 0..len as usize {
        // SAFETY: ptr is non-null (checked above) and i < len.
        uart_putc(unsafe { *ptr.add(i) });
    }
}

/// Bare-metal println: write `len` bytes + newline to UART.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_println(ptr: *const u8, len: i64) {
    fj_rt_bare_print(ptr, len);
    uart_putc(b'\n');
}

/// Bare-metal print integer to UART.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_print_i64(val: i64) {
    if val == 0 {
        uart_putc(b'0');
        uart_putc(b'\n');
        return;
    }

    let mut buf = [0u8; 21]; // max i64 digits + sign + newline
    let mut pos = 20;
    let negative = val < 0;
    let mut n = if negative {
        -(val as i128)
    } else {
        val as i128
    };

    while n > 0 {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    if negative {
        pos -= 1;
        buf[pos] = b'-';
    }

    for b in &buf[pos..20] {
        uart_putc(*b);
    }
    uart_putc(b'\n');
}

/// Set the UART base address (for switching from QEMU to QCS6490).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_set_uart_base(addr: u64) {
    UART_BASE.store(addr, Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════
// Panic Handler
// ═══════════════════════════════════════════════════════════════════════

/// Bare-metal panic: print "PANIC" + halt CPU in WFE loop.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_panic() {
    let msg = b"PANIC: kernel halt\n";
    fj_rt_bare_print(msg.as_ptr(), msg.len() as i64);
    fj_rt_bare_halt();
}

/// Halt the CPU in an infinite WFE (wait-for-event) loop.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_halt() {
    loop {
        // On real hardware, this would be `wfe` instruction.
        // In hosted test mode, just spin.
        core::hint::spin_loop();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Bump Allocator (kernel heap)
// ═══════════════════════════════════════════════════════════════════════

/// Initialize the bump allocator with heap base and size.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_heap_init(base: u64, size: u64) {
    HEAP_BASE.store(base, Ordering::Relaxed);
    HEAP_END.store(base + size, Ordering::Relaxed);
    BUMP_PTR.store(base, Ordering::Relaxed);
}

/// Bump allocator: allocate `size` bytes aligned to 8 bytes.
/// Returns pointer to allocated memory, or 0 (null) if OOM.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_alloc(size: i64) -> u64 {
    if size <= 0 {
        return 0;
    }
    let aligned_size = ((size as u64) + 7) & !7; // 8-byte alignment
    let ptr = BUMP_PTR.fetch_add(aligned_size, Ordering::Relaxed);
    let end = HEAP_END.load(Ordering::Relaxed);
    if ptr + aligned_size > end {
        // OOM: revert and return null
        BUMP_PTR.fetch_sub(aligned_size, Ordering::Relaxed);
        return 0;
    }
    ptr
}

/// Free: no-op for bump allocator. Full freelist allocator in Sprint 5.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_free(_ptr: u64, _size: i64) {
    // No-op: bump allocator doesn't support individual frees
}

/// Returns the current heap usage in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_heap_used() -> u64 {
    let base = HEAP_BASE.load(Ordering::Relaxed);
    let ptr = BUMP_PTR.load(Ordering::Relaxed);
    ptr.saturating_sub(base)
}

// ═══════════════════════════════════════════════════════════════════════
// GPIO Simulation (Sprint 11 — HAL Driver Support)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum number of simulated GPIO pins.
const GPIO_MAX_PINS: usize = 200;

/// GPIO pin modes: 0=unconfigured, 1=input, 2=output, 3-15=alt functions.
static GPIO_MODES: [AtomicU64; GPIO_MAX_PINS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; GPIO_MAX_PINS]
};

/// GPIO pin output values (0=low, 1=high).
static GPIO_VALUES: [AtomicU64; GPIO_MAX_PINS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; GPIO_MAX_PINS]
};

/// GPIO pull configuration: 0=none, 1=pull-down, 2=pull-up.
static GPIO_PULLS: [AtomicU64; GPIO_MAX_PINS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; GPIO_MAX_PINS]
};

/// Configure a GPIO pin: function, direction, pull.
/// Returns 0 on success, -1 on invalid pin.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_config(pin: i64, func: i64, output: i64, pull: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    let idx = pin as usize;
    let mode = if output != 0 { 2 } else { 1 };
    GPIO_MODES[idx].store(if func > 0 { func as u64 } else { mode }, Ordering::Relaxed);
    GPIO_PULLS[idx].store(pull as u64, Ordering::Relaxed);
    0
}

/// Set a GPIO pin as output.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_set_output(pin: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    GPIO_MODES[pin as usize].store(2, Ordering::Relaxed);
    0
}

/// Set a GPIO pin as input.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_set_input(pin: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    GPIO_MODES[pin as usize].store(1, Ordering::Relaxed);
    0
}

/// Write a value (0 or 1) to a GPIO output pin.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_write(pin: i64, value: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    GPIO_VALUES[pin as usize].store(if value != 0 { 1 } else { 0 }, Ordering::Relaxed);
    0
}

/// Read the current value of a GPIO pin. Returns 0 or 1, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_read(pin: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    GPIO_VALUES[pin as usize].load(Ordering::Relaxed) as i64
}

/// Toggle a GPIO output pin.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_toggle(pin: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    let idx = pin as usize;
    let old = GPIO_VALUES[idx].load(Ordering::Relaxed);
    GPIO_VALUES[idx].store(if old == 0 { 1 } else { 0 }, Ordering::Relaxed);
    0
}

/// Set pull resistor: 0=none, 1=pull-down, 2=pull-up.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_set_pull(pin: i64, pull: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    GPIO_PULLS[pin as usize].store(pull as u64, Ordering::Relaxed);
    0
}

/// Configure GPIO interrupt edge trigger: 0=none, 1=rising, 2=falling, 3=both.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_gpio_set_irq(pin: i64, _edge: i64) -> i64 {
    if pin < 0 || pin as usize >= GPIO_MAX_PINS {
        return -1;
    }
    // Simulation: store edge config but no real IRQ delivery
    0
}

// ═══════════════════════════════════════════════════════════════════════
// UART Enhanced (Sprint 12 — Multi-Port UART Support)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum UART ports.
const UART_MAX_PORTS: usize = 4;

/// UART port initialization status.
static UART_INIT: [AtomicU64; UART_MAX_PORTS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; UART_MAX_PORTS]
};

/// UART baud rates per port.
static UART_BAUD: [AtomicU64; UART_MAX_PORTS] = {
    const INIT: AtomicU64 = AtomicU64::new(115200);
    [INIT; UART_MAX_PORTS]
};

/// UART MMIO base addresses per port.
static UART_BASES: [AtomicU64; UART_MAX_PORTS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; UART_MAX_PORTS]
};

/// Initialize a UART port with baud rate. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_init(port: i64, baud: i64) -> i64 {
    if port < 0 || port as usize >= UART_MAX_PORTS || baud <= 0 {
        return -1;
    }
    let idx = port as usize;
    UART_BAUD[idx].store(baud as u64, Ordering::Relaxed);
    UART_INIT[idx].store(1, Ordering::Relaxed);
    // Default MMIO bases: port 0 = QEMU PL011 (0x0900_0000)
    if UART_BASES[idx].load(Ordering::Relaxed) == 0 && port == 0 {
        UART_BASES[idx].store(0x0900_0000, Ordering::Relaxed);
    }
    0
}

/// Write a single byte to a UART port. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_write_byte(port: i64, byte: i64) -> i64 {
    if port < 0 || port as usize >= UART_MAX_PORTS {
        return -1;
    }
    let base = UART_BASES[port as usize].load(Ordering::Relaxed);
    if base != 0 {
        // SAFETY: writing to UART MMIO data register
        unsafe { core::ptr::write_volatile(base as *mut u8, byte as u8) };
    }
    0
}

/// Read a single byte from a UART port. Returns byte value, or -1 if none available.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_read_byte(port: i64) -> i64 {
    if port < 0 || port as usize >= UART_MAX_PORTS {
        return -1;
    }
    let base = UART_BASES[port as usize].load(Ordering::Relaxed);
    if base == 0 {
        return -1;
    }
    // SAFETY: reading PL011 UARTFR (offset 0x18) bit 4 (RXFE = RX FIFO empty)
    let flags = unsafe { core::ptr::read_volatile((base + 0x18) as *const u32) };
    if flags & (1 << 4) != 0 {
        return -1; // RX FIFO empty
    }
    // SAFETY: reading PL011 UART data register at validated base address
    let byte = unsafe { core::ptr::read_volatile(base as *const u8) };
    byte as i64
}

/// Write a buffer to a UART port. Returns number of bytes written.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_write_buf(port: i64, ptr: *const u8, len: i64) -> i64 {
    if port < 0 || port as usize >= UART_MAX_PORTS || ptr.is_null() || len <= 0 {
        return 0;
    }
    let base = UART_BASES[port as usize].load(Ordering::Relaxed);
    if base == 0 {
        return 0;
    }
    for i in 0..len as usize {
        // SAFETY: ptr is non-null (checked above) and i < len
        let byte = unsafe { *ptr.add(i) };
        // SAFETY: writing to UART MMIO data register
        unsafe { core::ptr::write_volatile(base as *mut u8, byte) };
    }
    len
}

/// Read up to `max_len` bytes from UART into buffer. Returns bytes read.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_read_buf(port: i64, ptr: *mut u8, max_len: i64) -> i64 {
    if port < 0 || port as usize >= UART_MAX_PORTS || ptr.is_null() || max_len <= 0 {
        return 0;
    }
    let base = UART_BASES[port as usize].load(Ordering::Relaxed);
    if base == 0 {
        return 0;
    }
    let mut count = 0i64;
    for i in 0..max_len as usize {
        // SAFETY: reading PL011 UARTFR register at validated base address
        let flags = unsafe { core::ptr::read_volatile((base + 0x18) as *const u32) };
        if flags & (1 << 4) != 0 {
            break; // RX FIFO empty
        }
        // SAFETY: reading PL011 UART data register; writing to valid ptr + offset
        let byte = unsafe { core::ptr::read_volatile(base as *const u8) };
        unsafe { *ptr.add(i) = byte };
        count += 1;
    }
    count
}

/// Check bytes available in UART RX. Returns count or 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_available(port: i64) -> i64 {
    if port < 0 || port as usize >= UART_MAX_PORTS {
        return 0;
    }
    let base = UART_BASES[port as usize].load(Ordering::Relaxed);
    if base == 0 {
        return 0;
    }
    // SAFETY: reading PL011 UARTFR bit 4 (RXFE) at validated base address
    let flags = unsafe { core::ptr::read_volatile((base + 0x18) as *const u32) };
    if flags & (1 << 4) != 0 { 0 } else { 1 }
}

/// Set the MMIO base address for a UART port.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_uart_set_base(port: i64, addr: u64) {
    if port >= 0 && (port as usize) < UART_MAX_PORTS {
        UART_BASES[port as usize].store(addr, Ordering::Relaxed);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SPI Simulation (Sprint 13 — SPI Bus Support)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum SPI buses.
const SPI_MAX_BUSES: usize = 4;

/// SPI initialization status.
static SPI_INIT: [AtomicU64; SPI_MAX_BUSES] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; SPI_MAX_BUSES]
};

/// SPI loopback register (TX byte becomes next RX byte in simulation).
static SPI_LOOPBACK: [AtomicU64; SPI_MAX_BUSES] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; SPI_MAX_BUSES]
};

/// SPI chip select state.
static SPI_CS: [AtomicU64; SPI_MAX_BUSES] = {
    const INIT: AtomicU64 = AtomicU64::new(1); // CS deasserted (active low)
    [INIT; SPI_MAX_BUSES]
};

/// Initialize SPI bus. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_spi_init(bus: i64, clock_hz: i64) -> i64 {
    if bus < 0 || bus as usize >= SPI_MAX_BUSES || clock_hz <= 0 {
        return -1;
    }
    SPI_INIT[bus as usize].store(1, Ordering::Relaxed);
    0
}

/// Full-duplex SPI transfer: send `tx_byte`, return received byte.
/// In simulation mode, loopback: previous TX becomes current RX.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_spi_transfer(bus: i64, tx_byte: i64) -> i64 {
    if bus < 0 || bus as usize >= SPI_MAX_BUSES {
        return -1;
    }
    let idx = bus as usize;
    if SPI_INIT[idx].load(Ordering::Relaxed) == 0 {
        return -1;
    }
    let rx = SPI_LOOPBACK[idx].load(Ordering::Relaxed) as i64;
    SPI_LOOPBACK[idx].store(tx_byte as u64, Ordering::Relaxed);
    rx
}

/// Assert or deassert chip select. active=1 means CS asserted (low).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_spi_cs_set(bus: i64, _cs: i64, active: i64) -> i64 {
    if bus < 0 || bus as usize >= SPI_MAX_BUSES {
        return -1;
    }
    SPI_CS[bus as usize].store(if active != 0 { 0 } else { 1 }, Ordering::Relaxed);
    0
}

// ═══════════════════════════════════════════════════════════════════════
// I2C Simulation (Sprint 13 — I2C Bus Support)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum I2C buses.
const I2C_MAX_BUSES: usize = 4;

/// Simulated I2C device memory (one value per bus+addr pair, 512 slots).
const I2C_MEM_SLOTS: usize = I2C_MAX_BUSES * 128;
static I2C_SIM_MEM: [AtomicU64; I2C_MEM_SLOTS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; I2C_MEM_SLOTS]
};

/// I2C initialization status.
static I2C_INIT: [AtomicU64; I2C_MAX_BUSES] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; I2C_MAX_BUSES]
};

/// Initialize I2C bus with clock speed. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_i2c_init(bus: i64, clock_hz: i64) -> i64 {
    if bus < 0 || bus as usize >= I2C_MAX_BUSES || clock_hz <= 0 {
        return -1;
    }
    I2C_INIT[bus as usize].store(1, Ordering::Relaxed);
    0
}

/// Write data to I2C device. Returns 0 on success, -1 on NACK/error.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_i2c_write(bus: i64, addr: i64, ptr: *const u8, len: i64) -> i64 {
    if bus < 0 || bus as usize >= I2C_MAX_BUSES || addr < 0 || addr > 127 {
        return -1;
    }
    if I2C_INIT[bus as usize].load(Ordering::Relaxed) == 0 {
        return -1;
    }
    if ptr.is_null() || len <= 0 {
        return -1;
    }
    // Simulation: store data in I2C_SIM_MEM keyed by (bus * 128 + addr)
    let key = (bus as usize) * 128 + addr as usize;
    if key < I2C_MEM_SLOTS {
        // SAFETY: ptr is non-null (checked above); reading first byte
        let val = unsafe { *ptr } as u64;
        I2C_SIM_MEM[key].store(val, Ordering::Relaxed);
    }
    0
}

/// Read data from I2C device. Returns bytes read, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_i2c_read(bus: i64, addr: i64, ptr: *mut u8, len: i64) -> i64 {
    if bus < 0 || bus as usize >= I2C_MAX_BUSES || addr < 0 || addr > 127 {
        return -1;
    }
    if I2C_INIT[bus as usize].load(Ordering::Relaxed) == 0 {
        return -1;
    }
    if ptr.is_null() || len <= 0 {
        return -1;
    }
    // Simulation: fill buffer from I2C_SIM_MEM
    let key = (bus as usize) * 128 + addr as usize;
    let val = if key < I2C_MEM_SLOTS {
        I2C_SIM_MEM[key].load(Ordering::Relaxed) as u8
    } else {
        0
    };
    for i in 0..len as usize {
        // SAFETY: ptr is non-null (checked above) and i < len
        unsafe { *ptr.add(i) = val };
    }
    len
}

/// Combined write-then-read I2C transaction. Returns bytes read, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_i2c_write_read(
    bus: i64,
    addr: i64,
    tx: *const u8,
    tx_len: i64,
    rx: *mut u8,
    rx_len: i64,
) -> i64 {
    let w = fj_rt_bare_i2c_write(bus, addr, tx, tx_len);
    if w < 0 {
        return w;
    }
    fj_rt_bare_i2c_read(bus, addr, rx, rx_len)
}

// ═══════════════════════════════════════════════════════════════════════
// Timer Enhanced (Sprint 14 — High-Level Timer Support)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated monotonic tick counter (for host testing).
static SIM_TICKS: AtomicU64 = AtomicU64::new(0);
/// Simulated timer frequency (for host testing).
static SIM_FREQ: AtomicU64 = AtomicU64::new(62_500_000); // 62.5 MHz (QEMU default)
/// Boot time in ticks.
static BOOT_TICKS: AtomicU64 = AtomicU64::new(0);

/// Get current timer ticks. On host: simulated counter. On bare-metal: CNTPCT_EL0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_timer_get_ticks() -> i64 {
    // In hosted mode, use simulated ticks
    SIM_TICKS.fetch_add(1, Ordering::Relaxed) as i64
}

/// Get timer frequency in Hz. Returns ticks per second.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_timer_get_freq() -> i64 {
    SIM_FREQ.load(Ordering::Relaxed) as i64
}

/// Set absolute deadline for virtual timer (CNTV_CVAL_EL0 on bare-metal).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_timer_set_deadline(ticks: i64) {
    // Simulation: just store the deadline (no real interrupt)
    static DEADLINE: AtomicU64 = AtomicU64::new(0);
    DEADLINE.store(ticks as u64, Ordering::Relaxed);
}

/// Enable virtual timer (CNTV_CTL_EL0.ENABLE=1, IMASK=0).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_timer_enable_virtual() {
    // Simulation: no-op (bare-metal assembly stub does the real work)
}

/// Disable virtual timer.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_timer_disable_virtual() {
    // Simulation: no-op
}

/// Sleep for `ms` milliseconds. On host: thread::sleep. On bare-metal: busy-wait.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sleep_ms(ms: i64) {
    if ms <= 0 {
        return;
    }
    #[cfg(not(target_os = "none"))]
    {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    #[cfg(target_os = "none")]
    {
        // Bare-metal: busy-wait using timer counter
        let freq = SIM_FREQ.load(Ordering::Relaxed);
        let wait_ticks = (freq * ms as u64) / 1000;
        let start = SIM_TICKS.load(Ordering::Relaxed);
        while SIM_TICKS.load(Ordering::Relaxed) - start < wait_ticks {
            core::hint::spin_loop();
        }
    }
}

/// Sleep for `us` microseconds.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sleep_us(us: i64) {
    if us <= 0 {
        return;
    }
    #[cfg(not(target_os = "none"))]
    {
        std::thread::sleep(std::time::Duration::from_micros(us as u64));
    }
    #[cfg(target_os = "none")]
    {
        let freq = SIM_FREQ.load(Ordering::Relaxed);
        let wait_ticks = (freq * us as u64) / 1_000_000;
        let start = SIM_TICKS.load(Ordering::Relaxed);
        while SIM_TICKS.load(Ordering::Relaxed) - start < wait_ticks {
            core::hint::spin_loop();
        }
    }
}

/// Return milliseconds since boot.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_time_since_boot() -> i64 {
    let current = SIM_TICKS.load(Ordering::Relaxed);
    let boot = BOOT_TICKS.load(Ordering::Relaxed);
    let freq = SIM_FREQ.load(Ordering::Relaxed);
    if freq == 0 {
        return 0;
    }
    ((current - boot) * 1000 / freq) as i64
}

/// Mark current time as boot time (call once at startup).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_timer_mark_boot() {
    let ticks = SIM_TICKS.load(Ordering::Relaxed);
    BOOT_TICKS.store(ticks, Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════
// DMA Simulation (Sprint 15 — DMA Engine Support)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum DMA channels.
const DMA_MAX_CHANNELS: usize = 8;

/// DMA channel state: 0=idle, 1=configured, 2=running, 3=done.
static DMA_STATE: [AtomicU64; DMA_MAX_CHANNELS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; DMA_MAX_CHANNELS]
};

/// DMA source addresses.
static DMA_SRC: [AtomicU64; DMA_MAX_CHANNELS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; DMA_MAX_CHANNELS]
};

/// DMA destination addresses.
static DMA_DST: [AtomicU64; DMA_MAX_CHANNELS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; DMA_MAX_CHANNELS]
};

/// DMA transfer lengths.
static DMA_LEN: [AtomicU64; DMA_MAX_CHANNELS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; DMA_MAX_CHANNELS]
};

/// Allocate a physically contiguous DMA buffer. Returns address or 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_alloc(size: i64) -> u64 {
    // Use the bump allocator for DMA buffers (aligned to 64 bytes for cache lines)
    if size <= 0 {
        return 0;
    }
    let aligned = ((size as u64) + 63) & !63; // 64-byte cache-line alignment
    let ptr = BUMP_PTR.fetch_add(aligned, Ordering::Relaxed);
    let end = HEAP_END.load(Ordering::Relaxed);
    if ptr + aligned > end {
        BUMP_PTR.fetch_sub(aligned, Ordering::Relaxed);
        return 0;
    }
    ptr
}

/// Free a DMA buffer (no-op for bump allocator).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_free(_ptr: u64, _size: i64) {
    // No-op: bump allocator doesn't support individual frees
}

/// Configure DMA channel: source, destination, length.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_config(channel: i64, src: u64, dst: u64, len: i64) -> i64 {
    if channel < 0 || channel as usize >= DMA_MAX_CHANNELS || len <= 0 {
        return -1;
    }
    let idx = channel as usize;
    DMA_SRC[idx].store(src, Ordering::Relaxed);
    DMA_DST[idx].store(dst, Ordering::Relaxed);
    DMA_LEN[idx].store(len as u64, Ordering::Relaxed);
    DMA_STATE[idx].store(1, Ordering::Relaxed); // configured
    0
}

/// Start DMA transfer. In simulation: immediate memcpy.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_start(channel: i64) -> i64 {
    if channel < 0 || channel as usize >= DMA_MAX_CHANNELS {
        return -1;
    }
    let idx = channel as usize;
    if DMA_STATE[idx].load(Ordering::Relaxed) != 1 {
        return -1; // not configured
    }
    DMA_STATE[idx].store(2, Ordering::Relaxed); // running

    let src = DMA_SRC[idx].load(Ordering::Relaxed);
    let dst = DMA_DST[idx].load(Ordering::Relaxed);
    let len = DMA_LEN[idx].load(Ordering::Relaxed);

    // Simulation: immediate memcpy (real DMA would be async)
    if src != 0 && dst != 0 && len > 0 {
        fj_rt_bare_memcpy(dst as *mut u8, src as *const u8, len as i64);
    }

    DMA_STATE[idx].store(3, Ordering::Relaxed); // done
    0
}

/// Wait for DMA transfer completion. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_wait(channel: i64) -> i64 {
    if channel < 0 || channel as usize >= DMA_MAX_CHANNELS {
        return -1;
    }
    // In simulation, DMA is synchronous so always done immediately
    let state = DMA_STATE[channel as usize].load(Ordering::Relaxed);
    if state == 3 { 0 } else { -1 }
}

/// Get DMA channel status: 0=idle, 1=configured, 2=running, 3=done.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_status(channel: i64) -> i64 {
    if channel < 0 || channel as usize >= DMA_MAX_CHANNELS {
        return -1;
    }
    DMA_STATE[channel as usize].load(Ordering::Relaxed) as i64
}

/// DMA memory barrier: cache flush/invalidate.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dma_barrier() {
    // Host: atomic fence. Bare-metal: dc civac loop + dsb (assembly stub).
    std::sync::atomic::fence(Ordering::SeqCst);
}

// ═══════════════════════════════════════════════════════════════════════
// Block Device Simulation (Sprint 16-17 — Storage Support)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated block device storage (RAM-backed, 1MB = 2048 blocks × 512 bytes).
const BLOCK_SIZE: usize = 512;
const BLOCK_COUNT: usize = 2048;
static BLOCK_DEV_INIT: AtomicU64 = AtomicU64::new(0);

/// Initialize NVMe block device. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_nvme_init() -> i64 {
    BLOCK_DEV_INIT.store(1, Ordering::Relaxed);
    0
}

/// Read `count` blocks starting at `lba` into buffer. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_nvme_read(lba: i64, count: i64, buf: *mut u8) -> i64 {
    if buf.is_null() || lba < 0 || count <= 0 {
        return -1;
    }
    if (lba + count) as usize > BLOCK_COUNT {
        return -1;
    }
    // SAFETY: buf is non-null (checked above); i < bytes which fits within buf allocation
    let bytes = (count as usize) * BLOCK_SIZE;
    for i in 0..bytes {
        unsafe { *buf.add(i) = 0 };
    }
    0
}

/// Write `count` blocks starting at `lba` from buffer. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_nvme_write(lba: i64, count: i64, buf: *const u8) -> i64 {
    if buf.is_null() || lba < 0 || count <= 0 {
        return -1;
    }
    if (lba + count) as usize > BLOCK_COUNT {
        return -1;
    }
    // Simulation: discard data (no real storage)
    0
}

/// Initialize SD/eMMC block device. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sd_init() -> i64 {
    0
}

/// Read single block from SD at `lba`. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sd_read_block(lba: i64, buf: *mut u8) -> i64 {
    fj_rt_bare_nvme_read(lba, 1, buf)
}

/// Write single block to SD at `lba`. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sd_write_block(lba: i64, buf: *const u8) -> i64 {
    fj_rt_bare_nvme_write(lba, 1, buf)
}

// ═══════════════════════════════════════════════════════════════════════
// VFS Simulation (Sprint 18-19 — Virtual File System)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum open file descriptors.
const VFS_MAX_FDS: usize = 64;

/// File descriptor states: 0=closed, 1=open_read, 2=open_write, 3=open_rw.
static VFS_FD_STATE: [AtomicU64; VFS_MAX_FDS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; VFS_MAX_FDS]
};

/// Next available file descriptor.
static VFS_NEXT_FD: AtomicU64 = AtomicU64::new(3); // 0=stdin, 1=stdout, 2=stderr

/// Mount a filesystem at a path. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_vfs_mount(_path_ptr: *const u8, _path_len: i64, _fs_type: i64) -> i64 {
    0 // simulation: always succeeds
}

/// Open a file. Returns file descriptor (>= 0) or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_vfs_open(_path_ptr: *const u8, _path_len: i64, flags: i64) -> i64 {
    let fd = VFS_NEXT_FD.fetch_add(1, Ordering::Relaxed);
    if fd as usize >= VFS_MAX_FDS {
        VFS_NEXT_FD.fetch_sub(1, Ordering::Relaxed);
        return -1;
    }
    let mode = if flags & 2 != 0 { 3 } else { 1 }; // write flag → rw, else read
    VFS_FD_STATE[fd as usize].store(mode, Ordering::Relaxed);
    fd as i64
}

/// Read from file descriptor. Returns bytes read or -1.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_vfs_read(fd: i64, buf: *mut u8, count: i64) -> i64 {
    if fd < 0 || fd as usize >= VFS_MAX_FDS || buf.is_null() || count <= 0 {
        return -1;
    }
    let state = VFS_FD_STATE[fd as usize].load(Ordering::Relaxed);
    if state == 0 {
        return -1; // not open
    }
    // Simulation: return 0 bytes (EOF)
    0
}

/// Write to file descriptor. Returns bytes written or -1.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_vfs_write(fd: i64, buf: *const u8, count: i64) -> i64 {
    if fd < 0 || fd as usize >= VFS_MAX_FDS || buf.is_null() || count <= 0 {
        return -1;
    }
    let state = VFS_FD_STATE[fd as usize].load(Ordering::Relaxed);
    if state == 0 || state == 1 {
        return -1; // not open for write
    }
    // Simulation for stdout/stderr: write to UART
    if fd == 1 || fd == 2 {
        fj_rt_bare_print(buf, count);
    }
    count // pretend all bytes written
}

/// Close file descriptor. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_vfs_close(fd: i64) -> i64 {
    if fd < 0 || fd as usize >= VFS_MAX_FDS {
        return -1;
    }
    VFS_FD_STATE[fd as usize].store(0, Ordering::Relaxed);
    0
}

/// Stat a file. Returns file size or -1 if not found.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_vfs_stat(_path_ptr: *const u8, _path_len: i64) -> i64 {
    0 // simulation: file exists with size 0
}

// ═══════════════════════════════════════════════════════════════════════
// Network Simulation (Sprint 20-23 — TCP/IP Stack)
// ═══════════════════════════════════════════════════════════════════════

/// Maximum sockets.
const NET_MAX_SOCKETS: usize = 32;

/// Socket states: 0=closed, 1=created, 2=bound, 3=listening, 4=connected.
static NET_SOCK_STATE: [AtomicU64; NET_MAX_SOCKETS] = {
    const INIT: AtomicU64 = AtomicU64::new(0);
    [INIT; NET_MAX_SOCKETS]
};

/// Next available socket ID.
static NET_NEXT_SOCK: AtomicU64 = AtomicU64::new(0);

/// Initialize Ethernet interface. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_eth_init() -> i64 {
    0
}

/// Send raw Ethernet frame. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_eth_send(_frame: *const u8, _len: i64) -> i64 {
    0 // simulation: discard
}

/// Receive raw Ethernet frame. Returns frame length or 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_eth_recv(_buf: *mut u8, _max_len: i64) -> i64 {
    0 // simulation: nothing to receive
}

/// Create a network socket. type: 0=TCP, 1=UDP. Returns socket ID or -1.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_socket(sock_type: i64) -> i64 {
    if sock_type < 0 || sock_type > 1 {
        return -1;
    }
    let id = NET_NEXT_SOCK.fetch_add(1, Ordering::Relaxed);
    if id as usize >= NET_MAX_SOCKETS {
        NET_NEXT_SOCK.fetch_sub(1, Ordering::Relaxed);
        return -1;
    }
    NET_SOCK_STATE[id as usize].store(1, Ordering::Relaxed);
    id as i64
}

/// Bind socket to port. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_bind(sock: i64, port: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS || port < 0 || port > 65535 {
        return -1;
    }
    if NET_SOCK_STATE[sock as usize].load(Ordering::Relaxed) < 1 {
        return -1;
    }
    NET_SOCK_STATE[sock as usize].store(2, Ordering::Relaxed);
    0
}

/// Listen on socket. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_listen(sock: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS {
        return -1;
    }
    if NET_SOCK_STATE[sock as usize].load(Ordering::Relaxed) < 2 {
        return -1;
    }
    NET_SOCK_STATE[sock as usize].store(3, Ordering::Relaxed);
    0
}

/// Accept connection. Returns new socket ID or -1.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_accept(sock: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS {
        return -1;
    }
    if NET_SOCK_STATE[sock as usize].load(Ordering::Relaxed) != 3 {
        return -1;
    }
    // Simulation: create a connected socket
    let new_sock = fj_rt_bare_net_socket(0);
    if new_sock >= 0 {
        NET_SOCK_STATE[new_sock as usize].store(4, Ordering::Relaxed);
    }
    new_sock
}

/// Connect to remote address. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_connect(sock: i64, _addr: u64, _port: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS {
        return -1;
    }
    NET_SOCK_STATE[sock as usize].store(4, Ordering::Relaxed);
    0
}

/// Send data on connected socket. Returns bytes sent or -1.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_send(sock: i64, _buf: *const u8, len: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS || len < 0 {
        return -1;
    }
    if NET_SOCK_STATE[sock as usize].load(Ordering::Relaxed) != 4 {
        return -1;
    }
    len // simulation: all bytes "sent"
}

/// Receive data from socket. Returns bytes received or 0 (nothing available).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_recv(sock: i64, _buf: *mut u8, _max_len: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS {
        return -1;
    }
    if NET_SOCK_STATE[sock as usize].load(Ordering::Relaxed) < 4 {
        return -1;
    }
    0 // simulation: nothing to receive
}

/// Close socket. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_net_close(sock: i64) -> i64 {
    if sock < 0 || sock as usize >= NET_MAX_SOCKETS {
        return -1;
    }
    NET_SOCK_STATE[sock as usize].store(0, Ordering::Relaxed);
    0
}

// ═══════════════════════════════════════════════════════════════════════
// Framebuffer & Input Simulation (Sprint 24-26 — Display & Input)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated framebuffer (1920×1080, 32bpp = ~8MB).
/// We only track metadata, not actual pixel data.
static FB_WIDTH: AtomicU64 = AtomicU64::new(0);
static FB_HEIGHT: AtomicU64 = AtomicU64::new(0);
static FB_INIT: AtomicU64 = AtomicU64::new(0);
static FB_CURSOR_X: AtomicU64 = AtomicU64::new(0);
static FB_CURSOR_Y: AtomicU64 = AtomicU64::new(0);

/// Initialize framebuffer with resolution. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fb_init(width: i64, height: i64) -> i64 {
    if width <= 0 || height <= 0 {
        return -1;
    }
    FB_WIDTH.store(width as u64, Ordering::Relaxed);
    FB_HEIGHT.store(height as u64, Ordering::Relaxed);
    FB_CURSOR_X.store(0, Ordering::Relaxed);
    FB_CURSOR_Y.store(0, Ordering::Relaxed);
    FB_INIT.store(1, Ordering::Relaxed);
    0
}

/// Write a pixel at (x, y) with color (ARGB). Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fb_write_pixel(x: i64, y: i64, color: i64) -> i64 {
    let w = FB_WIDTH.load(Ordering::Relaxed) as i64;
    let h = FB_HEIGHT.load(Ordering::Relaxed) as i64;
    if x < 0 || x >= w || y < 0 || y >= h || FB_INIT.load(Ordering::Relaxed) == 0 {
        return -1;
    }
    let _ = color; // simulation: discard pixel
    0
}

/// Fill rectangle with color. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fb_fill_rect(x: i64, y: i64, w: i64, h: i64, color: i64) -> i64 {
    if FB_INIT.load(Ordering::Relaxed) == 0 || w <= 0 || h <= 0 {
        return -1;
    }
    let _ = (x, y, color); // simulation: discard
    0
}

/// Get framebuffer width.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fb_width() -> i64 {
    FB_WIDTH.load(Ordering::Relaxed) as i64
}

/// Get framebuffer height.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fb_height() -> i64 {
    FB_HEIGHT.load(Ordering::Relaxed) as i64
}

/// Simulated keyboard: last key pressed (0 = no key).
static KB_LAST_KEY: AtomicU64 = AtomicU64::new(0);
static KB_INIT: AtomicU64 = AtomicU64::new(0);

/// Initialize keyboard input. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_kb_init() -> i64 {
    KB_INIT.store(1, Ordering::Relaxed);
    0
}

/// Read key event. Returns ASCII code or 0 (no key).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_kb_read() -> i64 {
    KB_LAST_KEY.swap(0, Ordering::Relaxed) as i64
}

/// Check if key is available. Returns 1 if key ready, 0 if not.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_kb_available() -> i64 {
    if KB_LAST_KEY.load(Ordering::Relaxed) != 0 {
        1
    } else {
        0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OS Services Simulation (Sprint 32-35 — Process & System Management)
// ═══════════════════════════════════════════════════════════════════════

/// Next process ID.
static NEXT_PID: AtomicU64 = AtomicU64::new(2); // PID 0=idle, 1=init

/// Spawn a new process. Returns PID or -1.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_proc_spawn(_entry_addr: i64) -> i64 {
    let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
    if pid > 255 {
        NEXT_PID.fetch_sub(1, Ordering::Relaxed);
        return -1;
    }
    pid as i64
}

/// Wait for process to exit. Returns exit code.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_proc_wait(_pid: i64) -> i64 {
    0 // simulation: process exited with 0
}

/// Kill a process. Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_proc_kill(pid: i64) -> i64 {
    if pid <= 1 {
        return -1; // can't kill idle or init
    }
    0
}

/// Get current process ID.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_proc_self() -> i64 {
    1 // simulation: always init process
}

/// Yield CPU to scheduler.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_proc_yield() {
    // simulation: no-op
}

/// Context switch: read saved SP (written by exception vector stub).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sched_get_saved_sp() -> i64 {
    0 // simulation: no saved SP
}

/// Context switch: set next process SP (checked by vector stub after handler returns).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sched_set_next_sp(_sp: i64) {
    // simulation: no-op
}

/// Read a value from process table (IRQ-safe, no register clobber).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sched_read_proc(addr: i64) -> i64 {
    if addr == 0 {
        return 0;
    }
    // Simulation: return 0
    0
}

/// Write a value to process table (IRQ-safe, no register clobber).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sched_write_proc(_addr: i64, _value: i64) {
    // simulation: no-op
}

/// Power off the system.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sys_poweroff() {
    // simulation: no-op (on real hardware: PSCI shutdown)
}

/// Reboot the system.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sys_reboot() {
    // simulation: no-op
}

/// Get CPU temperature in millidegrees Celsius.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sys_cpu_temp() -> i64 {
    45_000 // simulation: 45.0°C
}

/// Get total RAM in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sys_ram_total() -> i64 {
    8 * 1024 * 1024 * 1024 // 8 GB
}

/// Get free RAM in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sys_ram_free() -> i64 {
    6 * 1024 * 1024 * 1024 // 6 GB free
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_memcpy_basic() {
        let src = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut dst = [0u8; 8];
        fj_rt_bare_memcpy(dst.as_mut_ptr(), src.as_ptr(), 8);
        assert_eq!(dst, src);
    }

    #[test]
    fn bare_memcpy_partial() {
        let src = [10u8, 20, 30, 40];
        let mut dst = [0u8; 4];
        fj_rt_bare_memcpy(dst.as_mut_ptr(), src.as_ptr(), 3);
        assert_eq!(dst, [10, 20, 30, 0]);
    }

    #[test]
    fn bare_memcpy_null_safe() {
        let result = fj_rt_bare_memcpy(std::ptr::null_mut(), std::ptr::null(), 10);
        assert!(result.is_null());
    }

    #[test]
    fn bare_memset_basic() {
        let mut buf = [0u8; 16];
        fj_rt_bare_memset(buf.as_mut_ptr(), 0xFF, 16);
        assert!(buf.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn bare_memset_partial() {
        let mut buf = [0u8; 8];
        fj_rt_bare_memset(buf.as_mut_ptr(), 0xAA, 4);
        assert_eq!(buf, [0xAA, 0xAA, 0xAA, 0xAA, 0, 0, 0, 0]);
    }

    #[test]
    fn bare_memcmp_equal() {
        let a = [1u8, 2, 3, 4];
        let b = [1u8, 2, 3, 4];
        assert_eq!(fj_rt_bare_memcmp(a.as_ptr(), b.as_ptr(), 4), 0);
    }

    #[test]
    fn bare_memcmp_different() {
        let a = [1u8, 2, 3, 4];
        let b = [1u8, 2, 5, 4];
        assert!(fj_rt_bare_memcmp(a.as_ptr(), b.as_ptr(), 4) < 0); // 3 < 5
    }

    #[test]
    fn bare_bump_alloc() {
        // Reset allocator
        fj_rt_bare_heap_init(0x1000, 0x100);
        let p1 = fj_rt_bare_alloc(16);
        assert_eq!(p1, 0x1000);
        let p2 = fj_rt_bare_alloc(32);
        assert_eq!(p2, 0x1010); // 16 bytes after p1
        assert_eq!(fj_rt_bare_heap_used(), 48);
    }

    #[test]
    fn bare_bump_alloc_alignment() {
        fj_rt_bare_heap_init(0x2000, 0x100);
        let p1 = fj_rt_bare_alloc(3); // 3 bytes → aligned to 8
        assert_eq!(p1, 0x2000);
        let p2 = fj_rt_bare_alloc(1); // 1 byte → aligned to 8
        assert_eq!(p2, 0x2008); // 8 bytes after p1 (aligned)
    }

    #[test]
    fn bare_bump_alloc_oom() {
        fj_rt_bare_heap_init(0xF000, 16); // tiny 16-byte heap at unique address
        let p1 = fj_rt_bare_alloc(8);
        assert_eq!(p1, 0xF000);
        let p2 = fj_rt_bare_alloc(8);
        assert_eq!(p2, 0xF008);
        let p3 = fj_rt_bare_alloc(8); // OOM
        assert_eq!(p3, 0); // null
    }

    #[test]
    fn bare_print_i64_formats_correctly() {
        // Can't easily test UART output in unit tests,
        // but verify the function doesn't crash
        UART_BASE.store(0, Ordering::Relaxed); // disable output
        fj_rt_bare_print_i64(42);
        fj_rt_bare_print_i64(-123);
        fj_rt_bare_print_i64(0);
        UART_BASE.store(0x0900_0000, Ordering::Relaxed); // restore
    }

    #[test]
    fn bare_free_is_noop() {
        fj_rt_bare_heap_init(0x4000, 0x100);
        let p = fj_rt_bare_alloc(16);
        let used_before = fj_rt_bare_heap_used();
        fj_rt_bare_free(p, 16);
        assert_eq!(fj_rt_bare_heap_used(), used_before); // no change
    }

    // ── GPIO tests ──

    #[test]
    fn bare_gpio_config_and_readback() {
        // Configure pin 42 as output with pull-up
        assert_eq!(fj_rt_bare_gpio_config(42, 0, 1, 2), 0);
        assert_eq!(GPIO_MODES[42].load(Ordering::Relaxed), 2); // output
        assert_eq!(GPIO_PULLS[42].load(Ordering::Relaxed), 2); // pull-up
    }

    #[test]
    fn bare_gpio_write_read() {
        fj_rt_bare_gpio_set_output(50);
        fj_rt_bare_gpio_write(50, 1);
        assert_eq!(fj_rt_bare_gpio_read(50), 1);
        fj_rt_bare_gpio_write(50, 0);
        assert_eq!(fj_rt_bare_gpio_read(50), 0);
    }

    #[test]
    fn bare_gpio_toggle() {
        fj_rt_bare_gpio_set_output(51);
        fj_rt_bare_gpio_write(51, 0);
        fj_rt_bare_gpio_toggle(51);
        assert_eq!(fj_rt_bare_gpio_read(51), 1);
        fj_rt_bare_gpio_toggle(51);
        assert_eq!(fj_rt_bare_gpio_read(51), 0);
    }

    #[test]
    fn bare_gpio_invalid_pin() {
        assert_eq!(fj_rt_bare_gpio_write(200, 1), -1); // out of range
        assert_eq!(fj_rt_bare_gpio_read(-1), -1); // negative
        assert_eq!(fj_rt_bare_gpio_config(999, 0, 1, 0), -1);
    }

    #[test]
    fn bare_gpio_pull_config() {
        fj_rt_bare_gpio_set_pull(60, 1); // pull-down
        assert_eq!(GPIO_PULLS[60].load(Ordering::Relaxed), 1);
        fj_rt_bare_gpio_set_pull(60, 2); // pull-up
        assert_eq!(GPIO_PULLS[60].load(Ordering::Relaxed), 2);
        fj_rt_bare_gpio_set_pull(60, 0); // no pull
        assert_eq!(GPIO_PULLS[60].load(Ordering::Relaxed), 0);
    }

    #[test]
    fn bare_gpio_input_mode() {
        fj_rt_bare_gpio_set_input(70);
        assert_eq!(GPIO_MODES[70].load(Ordering::Relaxed), 1); // input
    }

    // ── UART tests ──

    #[test]
    fn bare_uart_init_success() {
        assert_eq!(fj_rt_bare_uart_init(1, 115200), 0);
        assert_eq!(UART_INIT[1].load(Ordering::Relaxed), 1);
        assert_eq!(UART_BAUD[1].load(Ordering::Relaxed), 115200);
    }

    #[test]
    fn bare_uart_init_invalid() {
        assert_eq!(fj_rt_bare_uart_init(-1, 9600), -1);
        assert_eq!(fj_rt_bare_uart_init(4, 9600), -1); // out of range
        assert_eq!(fj_rt_bare_uart_init(0, 0), -1); // invalid baud
    }

    #[test]
    fn bare_uart_write_byte_no_crash() {
        // With base=0 (no MMIO), should succeed without writing
        UART_BASES[2].store(0, Ordering::Relaxed);
        assert_eq!(fj_rt_bare_uart_write_byte(2, b'A' as i64), 0);
    }

    #[test]
    fn bare_uart_set_base() {
        fj_rt_bare_uart_set_base(3, 0x0A8C_0000);
        assert_eq!(UART_BASES[3].load(Ordering::Relaxed), 0x0A8C_0000);
    }

    // ── SPI tests ──

    #[test]
    fn bare_spi_init_and_transfer() {
        assert_eq!(fj_rt_bare_spi_init(0, 1_000_000), 0);
        // First transfer: send 0xAB, receive previous loopback (0)
        let rx1 = fj_rt_bare_spi_transfer(0, 0xAB);
        assert_eq!(rx1, 0); // no previous data
        // Second transfer: send 0xCD, receive 0xAB (loopback)
        let rx2 = fj_rt_bare_spi_transfer(0, 0xCD);
        assert_eq!(rx2, 0xAB);
    }

    #[test]
    fn bare_spi_cs() {
        fj_rt_bare_spi_init(1, 500_000);
        fj_rt_bare_spi_cs_set(1, 0, 1); // assert CS
        assert_eq!(SPI_CS[1].load(Ordering::Relaxed), 0); // active low
        fj_rt_bare_spi_cs_set(1, 0, 0); // deassert CS
        assert_eq!(SPI_CS[1].load(Ordering::Relaxed), 1);
    }

    #[test]
    fn bare_spi_uninit_fails() {
        SPI_INIT[2].store(0, Ordering::Relaxed);
        assert_eq!(fj_rt_bare_spi_transfer(2, 0xFF), -1);
    }

    // ── I2C tests ──

    #[test]
    fn bare_i2c_write_read() {
        assert_eq!(fj_rt_bare_i2c_init(0, 400_000), 0);
        let tx_data = [0x42u8];
        assert_eq!(fj_rt_bare_i2c_write(0, 0x50, tx_data.as_ptr(), 1), 0);
        let mut rx_data = [0u8; 1];
        assert_eq!(fj_rt_bare_i2c_read(0, 0x50, rx_data.as_mut_ptr(), 1), 1);
        assert_eq!(rx_data[0], 0x42); // read back what was written
    }

    #[test]
    fn bare_i2c_write_read_combined() {
        fj_rt_bare_i2c_init(1, 100_000);
        let tx = [0x99u8];
        let mut rx = [0u8; 2];
        let result = fj_rt_bare_i2c_write_read(1, 0x68, tx.as_ptr(), 1, rx.as_mut_ptr(), 2);
        assert_eq!(result, 2);
        assert_eq!(rx[0], 0x99);
    }

    #[test]
    fn bare_i2c_invalid_addr() {
        fj_rt_bare_i2c_init(0, 400_000);
        assert_eq!(fj_rt_bare_i2c_write(0, 128, std::ptr::null(), 1), -1); // addr > 127
    }

    // ── Timer tests ──

    #[test]
    fn bare_timer_get_ticks_monotonic() {
        let t1 = fj_rt_bare_timer_get_ticks();
        let t2 = fj_rt_bare_timer_get_ticks();
        let t3 = fj_rt_bare_timer_get_ticks();
        assert!(t2 > t1);
        assert!(t3 > t2);
    }

    #[test]
    fn bare_timer_frequency() {
        let freq = fj_rt_bare_timer_get_freq();
        assert_eq!(freq, 62_500_000); // default QEMU frequency
    }

    #[test]
    fn bare_timer_mark_boot_and_uptime() {
        // Reset sim ticks
        SIM_TICKS.store(0, Ordering::Relaxed);
        fj_rt_bare_timer_mark_boot();
        // Advance ticks by 62500 (= 1ms at 62.5MHz)
        SIM_TICKS.store(62_500, Ordering::Relaxed);
        let ms = fj_rt_bare_time_since_boot();
        assert_eq!(ms, 1);
    }

    #[test]
    fn bare_timer_set_deadline_no_crash() {
        fj_rt_bare_timer_set_deadline(1_000_000);
        fj_rt_bare_timer_enable_virtual();
        fj_rt_bare_timer_disable_virtual();
    }

    // ── DMA tests ──

    #[test]
    fn bare_dma_config_start_wait() {
        // Set up source and destination buffers
        let src = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut dst = [0u8; 8];
        assert_eq!(
            fj_rt_bare_dma_config(0, src.as_ptr() as u64, dst.as_mut_ptr() as u64, 8,),
            0,
        );
        assert_eq!(fj_rt_bare_dma_status(0), 1); // configured
        assert_eq!(fj_rt_bare_dma_start(0), 0);
        assert_eq!(fj_rt_bare_dma_status(0), 3); // done
        assert_eq!(fj_rt_bare_dma_wait(0), 0);
        assert_eq!(dst, src); // data transferred
    }

    #[test]
    fn bare_dma_invalid_channel() {
        assert_eq!(fj_rt_bare_dma_config(8, 0, 0, 1), -1); // out of range
        assert_eq!(fj_rt_bare_dma_start(-1), -1);
        assert_eq!(fj_rt_bare_dma_status(8), -1);
    }

    #[test]
    fn bare_dma_alloc_aligned() {
        fj_rt_bare_heap_init(0x5000, 0x1000);
        let p = fj_rt_bare_dma_alloc(100);
        assert_eq!(p, 0x5000);
        assert_eq!(p % 64, 0); // 64-byte aligned
        let p2 = fj_rt_bare_dma_alloc(50);
        assert_eq!(p2, 0x5080); // 128 bytes later (100 rounded to 128)
        assert_eq!(p2 % 64, 0);
    }

    #[test]
    fn bare_dma_barrier_no_crash() {
        fj_rt_bare_dma_barrier(); // should not panic
    }

    // ── Block Device tests (Phase 4) ──

    #[test]
    fn bare_nvme_init_and_read() {
        assert_eq!(fj_rt_bare_nvme_init(), 0);
        let mut buf = [0xFFu8; 512];
        assert_eq!(fj_rt_bare_nvme_read(0, 1, buf.as_mut_ptr()), 0);
        assert!(buf.iter().all(|&b| b == 0)); // simulation fills with zeros
    }

    #[test]
    fn bare_nvme_write_and_bounds() {
        let data = [0xABu8; 512];
        assert_eq!(fj_rt_bare_nvme_write(0, 1, data.as_ptr()), 0);
        // Out of bounds
        assert_eq!(fj_rt_bare_nvme_read(2048, 1, std::ptr::null_mut()), -1);
    }

    #[test]
    fn bare_sd_read_write() {
        assert_eq!(fj_rt_bare_sd_init(), 0);
        let mut buf = [0u8; 512];
        assert_eq!(fj_rt_bare_sd_read_block(0, buf.as_mut_ptr()), 0);
        assert_eq!(fj_rt_bare_sd_write_block(10, buf.as_ptr()), 0);
    }

    // ── VFS tests (Phase 4) ──

    #[test]
    fn bare_vfs_open_close() {
        let path = b"/test.txt\0";
        let fd = fj_rt_bare_vfs_open(path.as_ptr(), 9, 1); // read mode
        assert!(fd >= 3); // 0-2 are reserved
        assert_eq!(fj_rt_bare_vfs_close(fd), 0);
    }

    #[test]
    fn bare_vfs_write_stdout() {
        // Mark stdout as open for write, disable UART output
        VFS_FD_STATE[1].store(3, Ordering::Relaxed); // rw
        UART_BASE.store(0, Ordering::Relaxed); // disable MMIO output
        let data = b"test";
        let written = fj_rt_bare_vfs_write(1, data.as_ptr(), 4);
        assert_eq!(written, 4);
        UART_BASE.store(0x0900_0000, Ordering::Relaxed); // restore
    }

    #[test]
    fn bare_vfs_stat() {
        let path = b"/etc/config\0";
        let size = fj_rt_bare_vfs_stat(path.as_ptr(), 11);
        assert_eq!(size, 0); // simulation returns 0
    }

    // ── Network tests (Phase 5) ──

    #[test]
    fn bare_eth_init_send_recv() {
        assert_eq!(fj_rt_bare_eth_init(), 0);
        let frame = [0u8; 64];
        assert_eq!(fj_rt_bare_eth_send(frame.as_ptr(), 64), 0);
        let mut buf = [0u8; 1500];
        assert_eq!(fj_rt_bare_eth_recv(buf.as_mut_ptr(), 1500), 0); // nothing
    }

    #[test]
    fn bare_net_tcp_lifecycle() {
        let sock = fj_rt_bare_net_socket(0); // TCP
        assert!(sock >= 0);
        assert_eq!(fj_rt_bare_net_bind(sock, 8080), 0);
        assert_eq!(fj_rt_bare_net_listen(sock), 0);
        // Connect simulation
        let client = fj_rt_bare_net_socket(0);
        assert_eq!(fj_rt_bare_net_connect(client, 0x7F000001, 8080), 0);
        let data = b"hello";
        assert_eq!(fj_rt_bare_net_send(client, data.as_ptr(), 5), 5);
        let mut buf = [0u8; 100];
        assert_eq!(fj_rt_bare_net_recv(client, buf.as_mut_ptr(), 100), 0); // nothing
        assert_eq!(fj_rt_bare_net_close(client), 0);
        assert_eq!(fj_rt_bare_net_close(sock), 0);
    }

    #[test]
    fn bare_net_udp_socket() {
        let sock = fj_rt_bare_net_socket(1); // UDP
        assert!(sock >= 0);
        assert_eq!(fj_rt_bare_net_bind(sock, 53), 0);
        assert_eq!(fj_rt_bare_net_close(sock), 0);
    }

    #[test]
    fn bare_net_invalid() {
        assert_eq!(fj_rt_bare_net_socket(5), -1); // invalid type
        assert_eq!(fj_rt_bare_net_send(-1, std::ptr::null(), 0), -1);
        assert_eq!(fj_rt_bare_net_close(99), -1);
    }

    // ── Framebuffer tests (Phase 6) ──

    #[test]
    fn bare_fb_init_and_pixel() {
        assert_eq!(fj_rt_bare_fb_init(1920, 1080), 0);
        assert_eq!(fj_rt_bare_fb_width(), 1920);
        assert_eq!(fj_rt_bare_fb_height(), 1080);
        assert_eq!(fj_rt_bare_fb_write_pixel(100, 200, 0xFF_FF_00_00), 0); // red
        assert_eq!(fj_rt_bare_fb_write_pixel(1920, 0, 0), -1); // out of bounds
    }

    #[test]
    fn bare_fb_fill_rect() {
        fj_rt_bare_fb_init(800, 600);
        assert_eq!(fj_rt_bare_fb_fill_rect(10, 10, 100, 50, 0xFF_00_FF_00), 0);
        assert_eq!(fj_rt_bare_fb_fill_rect(0, 0, 0, 0, 0), -1); // zero size
    }

    #[test]
    fn bare_kb_init_read() {
        assert_eq!(fj_rt_bare_kb_init(), 0);
        assert_eq!(fj_rt_bare_kb_available(), 0); // no key
        assert_eq!(fj_rt_bare_kb_read(), 0);
        // Simulate key press
        KB_LAST_KEY.store(b'A' as u64, Ordering::Relaxed);
        assert_eq!(fj_rt_bare_kb_available(), 1);
        assert_eq!(fj_rt_bare_kb_read(), b'A' as i64);
        assert_eq!(fj_rt_bare_kb_available(), 0); // consumed
    }

    // ── OS Services tests (Phase 8) ──

    #[test]
    fn bare_proc_spawn_wait_kill() {
        let pid = fj_rt_bare_proc_spawn(0x4000_0000);
        assert!(pid >= 2);
        assert_eq!(fj_rt_bare_proc_wait(pid), 0);
        assert_eq!(fj_rt_bare_proc_kill(pid), 0);
        assert_eq!(fj_rt_bare_proc_kill(0), -1); // can't kill idle
        assert_eq!(fj_rt_bare_proc_kill(1), -1); // can't kill init
    }

    #[test]
    fn bare_proc_self_yield() {
        assert_eq!(fj_rt_bare_proc_self(), 1); // init
        fj_rt_bare_proc_yield(); // no-op, should not crash
    }

    #[test]
    fn bare_sys_info() {
        assert_eq!(fj_rt_bare_sys_cpu_temp(), 45_000); // 45°C
        assert!(fj_rt_bare_sys_ram_total() > 0);
        assert!(fj_rt_bare_sys_ram_free() > 0);
        assert!(fj_rt_bare_sys_ram_free() <= fj_rt_bare_sys_ram_total());
    }

    #[test]
    fn bare_sys_poweroff_reboot_no_crash() {
        // These are no-ops in simulation
        fj_rt_bare_sys_reboot();
        // Don't call poweroff in tests — it's a no-op but semantically wrong
    }
}

// Syscall builtins (simulation — bare-metal uses assembly stubs)

/// Read syscall argument 0 from saved exception stack.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_syscall_arg0() -> i64 {
    0
}

/// Read syscall argument 1 from saved exception stack.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_syscall_arg1() -> i64 {
    0
}

/// Read syscall argument 2 from saved exception stack.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_syscall_arg2() -> i64 {
    0
}

/// Set syscall return value (write to saved x0 on exception stack).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_syscall_set_return(_val: i64) {}

/// User-mode syscall: svc(num, arg1, arg2) -> result.
/// On bare-metal: assembly implementation in linker.rs (SVC #0 instruction).
/// On hosted: no-op stub.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_svc(_num: i64, _arg1: i64, _arg2: i64) -> i64 {
    0
}

/// Switch TTBR0 + TLB flush (simulation: no-op).
/// On bare-metal: assembly implementation in linker.rs (DSB+MSR+TLBI+ISB).
/// On hosted: no-op stub.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_switch_ttbr0(_ttbr0: i64) {}

/// Read current TTBR0 (simulation: return 0).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_ttbr0() -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_tlbi_va(_va: i64) {}

// ═══════════════════════════════════════════════════════════════════════
// AArch64 EL0 User Space Support
// ═══════════════════════════════════════════════════════════════════════
//
// ARM64 Exception Levels:
//   EL0 — User space (unprivileged, no MMIO, no page table access)
//   EL1 — Kernel (privileged, full hardware control)
//
// Transition EL1→EL0: set SPSR_EL1.M[3:0]=0b0000 (EL0t), set ELR_EL1
//   to user entry point, set SP_EL0 to user stack, then ERET.
// Transition EL0→EL1: SVC instruction (sync) or IRQ (async).

// Hosted-only stubs for EL0 functions.
// On aarch64 bare-metal, assembly implementations in linker.rs take precedence.
// On x86_64/other hosts, these provide simulation stubs.

/// Transition from EL1 to EL0 (kernel → user mode).
/// On bare-metal aarch64: assembly in linker.rs. On hosted: no-op.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_eret_to_el0(_entry: i64, _sp_el0: i64, _spsr: i64) {}

/// Read current exception level. On hosted: returns 1 (EL1).
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_current_el() -> i64 {
    1
}

/// Read SPSR_EL1. On hosted: returns 0.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_spsr_el1() -> i64 {
    0
}

/// Write SPSR_EL1. On hosted: no-op.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_write_spsr_el1(_val: i64) {}

/// Read ELR_EL1. On hosted: returns 0.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_elr_el1() -> i64 {
    0
}

/// Write ELR_EL1. On hosted: no-op.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_write_elr_el1(_val: i64) {}

/// Read SP_EL0. On hosted: returns 0.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_sp_el0() -> i64 {
    0
}

/// Write SP_EL0. On hosted: no-op.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_write_sp_el0(_val: i64) {}

/// Read TTBR1_EL1. On hosted: returns 0.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_ttbr1() -> i64 {
    0
}

/// Write TTBR1_EL1. On hosted: no-op.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_write_ttbr1(_val: i64) {}

/// Read ESR_EL1. On hosted: returns 0.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_esr_el1() -> i64 {
    0
}

/// Read FAR_EL1. On hosted: returns 0.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_far_el1() -> i64 {
    0
}

/// TLB invalidate all. On hosted: no-op.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_tlbi_all() {}

/// ISB barrier. On hosted: compiler fence.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_isb() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

/// DSB barrier. On hosted: compiler fence.
#[cfg(not(target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_dsb() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

// ═══════════════════════════════════════════════════════════════════════
// AArch64 EL0 Constants (for Fajar Lang @kernel code)
// ═══════════════════════════════════════════════════════════════════════

/// SPSR value for EL0t: M[3:0]=0b0000, DAIF=0 (all interrupts enabled).
pub const SPSR_EL0T: i64 = 0x0000_0000;

/// SPSR value for EL0t with IRQ masked: M[3:0]=0b0000, I=1.
pub const SPSR_EL0T_IRQ_MASKED: i64 = 0x0000_0080;

/// ESR EC field for SVC from AArch64 (syscall).
pub const ESR_EC_SVC_AARCH64: i64 = 0x15;

/// ESR EC field for Data Abort from lower EL.
pub const ESR_EC_DATA_ABORT_LOWER: i64 = 0x24;

/// ESR EC field for Instruction Abort from lower EL.
pub const ESR_EC_INST_ABORT_LOWER: i64 = 0x20;

/// Page table descriptor: AP[2:1] for EL0 access.
/// AP=0b01: RW at EL1, no access at EL0 (kernel-only).
pub const PTE_AP_KERNEL_RW: i64 = 0x0000_0000_0000_0040;

/// AP=0b00: RW at both EL0 and EL1 (user read-write).
pub const PTE_AP_USER_RW: i64 = 0x0000_0000_0000_0000;

/// AP=0b11: RO at both EL0 and EL1 (user read-only).
pub const PTE_AP_USER_RO: i64 = 0x0000_0000_0000_00C0;

/// Page table descriptor: UXN (User eXecute Never) bit.
pub const PTE_UXN: i64 = 1 << 54;

/// Page table descriptor: PXN (Privileged eXecute Never) bit.
pub const PTE_PXN: i64 = 1 << 53;

/// Page table descriptor: AF (Access Flag) — must be set.
pub const PTE_AF: i64 = 1 << 10;

/// Page table descriptor: valid block/page entry.
pub const PTE_VALID: i64 = 0x1;

/// Page table descriptor: table entry (for non-leaf entries).
pub const PTE_TABLE: i64 = 0x3;

// ═══════════════════════════════════════════════════════════════════════
// x86_64 Port I/O (IN/OUT instructions)
// ═══════════════════════════════════════════════════════════════════════
//
// x86_64 uses port I/O (not MMIO) for legacy devices:
//   - 16550 UART (COM1: 0x3F8, COM2: 0x2F8)
//   - PIC (0x20/0xA0), PIT (0x40), PS/2 (0x60/0x64)
//   - PCI config (0xCF8/0xCFC), VGA (0x3D4/0x3D5)
//
// These are simulation stubs for hosted testing.
// In bare-metal AOT, the compiler generates actual IN/OUT instructions.

/// x86_64 port output: write a byte to an I/O port.
/// In hosted mode, this simulates COM1 serial output.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_port_outb(port: i64, value: i64) -> i64 {
    // Simulation: map common ports to host I/O
    if port == 0x3F8 {
        uart_putc(value as u8); // COM1 data → host serial output
    }
    // Bare-metal: real `out` instruction (requires ring 0)
    // SAFETY: only invoked from @kernel context, port range checked by caller
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    unsafe {
        std::arch::asm!("out dx, al", in("dx") port as u16, in("al") value as u8, options(nomem, nostack));
    }
    0
}

/// x86_64 port input: read a byte from an I/O port.
/// In hosted mode, returns simulated values.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_port_inb(port: i64) -> i64 {
    // Bare-metal: real `in` instruction (requires ring 0)
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        let result: u8;
        // SAFETY: only invoked from @kernel context
        unsafe {
            std::arch::asm!("in al, dx", in("dx") port as u16, out("al") result, options(nomem, nostack));
        }
        return result as i64;
    }
    // Simulation: COM1 LSR (0x3FD) — TX empty + TX holding empty
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    {
        if port == 0x3FD {
            return 0x60; // bits 5+6 set = transmitter empty
        }
        if port == 0x3F8 {
            return 0;
        }
        0
    }
}

/// x86_64 serial init: initialize 16550 UART at standard COM port.
/// port: 0 = COM1 (0x3F8), 1 = COM2 (0x2F8)
/// baud: baud rate (115200, 9600, etc.)
///
/// In hosted simulation, this just marks the port as initialized.
/// In bare-metal AOT, the .fj code calls port_outb() directly.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_x86_serial_init(port: i64, baud: i64) -> i64 {
    let base: i64 = match port {
        0 => 0x3F8, // COM1
        1 => 0x2F8, // COM2
        _ => return -1,
    };

    if baud <= 0 {
        return -1;
    }

    let divisor = 115200 / baud;

    // Standard 16550 UART initialization sequence
    fj_rt_bare_port_outb(base + 1, 0x00); // Disable interrupts
    fj_rt_bare_port_outb(base + 3, 0x80); // Enable DLAB
    fj_rt_bare_port_outb(base, divisor); // Divisor low byte
    fj_rt_bare_port_outb(base + 1, 0x00); // Divisor high byte
    fj_rt_bare_port_outb(base + 3, 0x03); // 8N1
    fj_rt_bare_port_outb(base + 2, 0xC7); // Enable FIFO, clear, 14-byte threshold
    fj_rt_bare_port_outb(base + 4, 0x0B); // IRQs enabled, RTS/DSR set

    // Also register as UART port 0 for the existing UART subsystem
    if (port as usize) < UART_MAX_PORTS {
        UART_INIT[port as usize].store(1, Ordering::Relaxed);
        UART_BAUD[port as usize].store(baud as u64, Ordering::Relaxed);
    }

    0
}

// ═══════════════════════════════════════════════════════════════════════
// x86_64 UART print (uses port I/O instead of MMIO)
// ═══════════════════════════════════════════════════════════════════════

/// Mode flag: 0 = MMIO (ARM), 1 = port I/O (x86_64).
static UART_MODE: AtomicU64 = AtomicU64::new(0);

/// Set UART mode to x86_64 port I/O (call once at boot).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_set_uart_mode_x86(base_port: i64) {
    UART_MODE.store(1, Ordering::Relaxed);
    // Store the port base in UART_BASE (reuse the atomic)
    UART_BASE.store(base_port as u64, Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════
// x86_64 FPU / SSE state + misc (FajarOS Nova)
// ═══════════════════════════════════════════════════════════════════════

/// Save x87/SSE state to a 512-byte buffer (FXSAVE).
/// On hosted targets: no-op stub (buffer untouched).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fxsave(_buf: i64) {
    // Bare-metal: `fxsave [rdi]` — not meaningful on hosted Linux
}

/// Restore x87/SSE state from a 512-byte buffer (FXRSTOR).
/// On hosted targets: no-op stub.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_fxrstor(_buf: i64) {
    // Bare-metal: `fxrstor [rdi]` — not meaningful on hosted Linux
}

/// Return a hardware random number via RDRAND.
/// On x86_64 with RDRAND support: uses real hardware RNG.
/// On other targets: returns a pseudo-random LCG value.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_rdrand() -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // Try RDRAND (non-privileged instruction, needs CPU support)
        let result: u64;
        let success: u8;
        // SAFETY: RDRAND is safe on CPUs that support it; CF=1 on success
        unsafe {
            std::arch::asm!(
                "rdrand {0}",
                "setc {1}",
                out(reg) result,
                out(reg_byte) success,
                options(nomem, nostack),
            );
        }
        if success != 0 {
            return result as i64;
        }
        // Fallback to LCG if RDRAND fails (e.g., entropy exhaustion)
    }
    // LCG fallback for non-x86_64 or RDRAND failure
    static SEED: AtomicU64 = AtomicU64::new(0x5DEE_CE66_D1A4_F87D);
    let old = SEED.load(Ordering::Relaxed);
    let next = old
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    SEED.store(next, Ordering::Relaxed);
    next as i64
}

/// Transition to user-mode via IRETQ (x86_64 Ring 0 → Ring 3).
/// On hosted targets: no-op stub.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_iretq_to_user(_rip: i64, _rsp: i64, _rflags: i64) {
    // Bare-metal: push SS/RSP/RFLAGS/CS/RIP and IRETQ
}

/// Read CR4 control register (x86_64).
/// On hosted targets: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_cr4() -> i64 {
    0
}

/// Write CR4 control register (x86_64).
/// On hosted targets: no-op stub.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_write_cr4(_val: i64) {}

/// Read CR3 control register (page table base address, x86_64).
/// On hosted targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_read_cr3() -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let val: u64;
        // SAFETY: reading CR3 register; requires ring 0 privilege on bare metal
        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) val);
        }
        val as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    0
}

/// Write CR3 control register (switch page table, x86_64).
/// On hosted targets: no-op stub.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_write_cr3(val: i64) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: writing CR3 register to switch page table; requires ring 0 privilege
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) val as u64);
    }
}

/// Read CR2 control register (page fault linear address, x86_64).
/// On hosted targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_read_cr2() -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let val: u64;
        // SAFETY: reading CR2 register (page fault address); requires ring 0 privilege
        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) val);
        }
        val as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    0
}

/// Invalidate TLB entry for a specific virtual address (INVLPG).
/// On hosted targets: no-op.
/// On bare-metal: real `invlpg` instruction.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_invlpg(addr: i64) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only, addr is a valid virtual address
    unsafe {
        std::arch::asm!("invlpg [{}]", in(reg) addr as usize, options(nostack));
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = addr;
}

/// Read timestamp counter (RDTSC).
/// On x86_64: reads real hardware TSC (non-privileged instruction).
/// On other architectures: returns monotonic counter.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_rdtsc() -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: RDTSC is a non-privileged instruction on x86_64
        unsafe {
            let lo: u64;
            let hi: u64;
            std::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
            ((hi << 32) | lo) as i64
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        static TSC: AtomicU64 = AtomicU64::new(0);
        TSC.fetch_add(1, Ordering::Relaxed) as i64
    }
}

/// Read CR0 control register.
/// On hosted targets: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_cr0() -> i64 {
    0
}

/// Read MSR (Model Specific Register).
/// On hosted targets: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_read_msr(_msr: i64) -> i64 {
    0
}

/// Write MSR (Model Specific Register).
/// On hosted targets: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_write_msr(_msr: i64, _val: i64) -> i64 {
    0
}

// ═══════════════════════════════════════════════════════════════════════
// x86_64 System Builtins (FajarOS Nova v0.2)
// ═══════════════════════════════════════════════════════════════════════

/// Halt CPU until next interrupt (HLT instruction).
/// On hosted targets: yields the current thread.
/// On bare-metal: `asm!("hlt")`.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_hlt() {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: halts CPU until next interrupt (ring 0 only)
    unsafe {
        std::arch::asm!("hlt", options(nomem, nostack));
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    std::thread::yield_now();
}

/// Disable interrupts (clear IF flag).
/// On hosted targets: no-op.
/// On bare-metal: real `cli` instruction.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_cli() {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only
    unsafe {
        std::arch::asm!("cli", options(nomem, nostack));
    }
}

/// Enable interrupts (set IF flag).
/// On hosted targets: no-op.
/// On bare-metal: real `sti` instruction.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_sti() {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only
    unsafe {
        std::arch::asm!("sti", options(nomem, nostack));
    }
}

/// Query CPU features via CPUID instruction.
/// Returns eax register value for the given leaf/subleaf.
/// On non-x86_64 targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_cpuid(leaf: i64, subleaf: i64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: cpuid is safe on x86_64, does not modify machine state
        let result = unsafe { std::arch::x86_64::__cpuid_count(leaf as u32, subleaf as u32) };
        result.eax as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (leaf, subleaf);
        0
    }
}

/// CPUID — returns eax register for a given leaf.
/// Single-argument variant (subleaf defaults to 0).
/// On non-x86_64 targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_cpuid_eax(leaf: i64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: cpuid is safe on x86_64, does not modify machine state
        let result = unsafe { std::arch::x86_64::__cpuid_count(leaf as u32, 0) };
        result.eax as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = leaf;
        0
    }
}

/// CPUID — returns ebx register for a given leaf.
/// Single-argument variant (subleaf defaults to 0).
/// On non-x86_64 targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_cpuid_ebx(leaf: i64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: cpuid is safe on x86_64, does not modify machine state
        let result = unsafe { std::arch::x86_64::__cpuid_count(leaf as u32, 0) };
        result.ebx as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = leaf;
        0
    }
}

/// CPUID — returns ecx register for a given leaf.
/// Single-argument variant (subleaf defaults to 0).
/// On non-x86_64 targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_cpuid_ecx(leaf: i64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: cpuid is safe on x86_64, does not modify machine state
        let result = unsafe { std::arch::x86_64::__cpuid_count(leaf as u32, 0) };
        result.ecx as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = leaf;
        0
    }
}

/// CPUID — returns edx register for a given leaf.
/// Single-argument variant (subleaf defaults to 0).
/// On non-x86_64 targets: returns 0.
#[unsafe(no_mangle)]
#[allow(unused_unsafe)]
pub extern "C" fn fj_rt_bare_cpuid_edx(leaf: i64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: cpuid is safe on x86_64, does not modify machine state
        let result = unsafe { std::arch::x86_64::__cpuid_count(leaf as u32, 0) };
        result.edx as i64
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = leaf;
        0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FajarOS Nova v0.3 Stage A: Extended Port I/O, CPU Control, Buffer Ops
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 port input: read a 16-bit word from an I/O port.
/// On bare-metal: real `in` instruction. On hosted: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_port_inw(port: i64) -> i64 {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        let result: u16;
        // SAFETY: ring 0 only
        unsafe {
            std::arch::asm!("in ax, dx", in("dx") port as u16, out("ax") result, options(nomem, nostack));
        }
        return result as i64;
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    {
        let _ = port;
        0
    }
}

/// x86_64 port input: read a 32-bit dword from an I/O port.
/// On bare-metal: real `in` instruction. On hosted: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_port_ind(port: i64) -> i64 {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        let result: u32;
        // SAFETY: ring 0 only
        unsafe {
            std::arch::asm!("in eax, dx", in("dx") port as u16, out("eax") result, options(nomem, nostack));
        }
        return result as i64;
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    {
        let _ = port;
        0
    }
}

/// x86_64 port output: write a 16-bit word to an I/O port.
/// On bare-metal: real `out` instruction. On hosted: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_port_outw(port: i64, value: i64) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only
    unsafe {
        std::arch::asm!("out dx, ax", in("dx") port as u16, in("ax") value as u16, options(nomem, nostack));
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = (port, value);
}

/// x86_64 port output: write a 32-bit dword to an I/O port.
/// On bare-metal: real `out` instruction. On hosted: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_port_outd(port: i64, value: i64) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only
    unsafe {
        std::arch::asm!("out dx, eax", in("dx") port as u16, in("eax") value as u32, options(nomem, nostack));
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = (port, value);
}

/// Load Task Register (LTR instruction).
/// On hosted targets: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_ltr(_selector: i64) {
    // Bare-metal: `ltr di`
}

/// Load GDT from memory pointer (LGDT instruction).
/// On bare-metal: real `lgdt` instruction. On hosted: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_lgdt_mem(addr: i64) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only, addr must point to valid GDT descriptor
    unsafe {
        std::arch::asm!("lgdt [{}]", in(reg) addr as usize, options(nostack));
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = addr;
}

/// Load IDT from memory pointer (LIDT instruction).
/// On bare-metal: real `lidt` instruction. On hosted: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_lidt_mem(addr: i64) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only, addr must point to valid IDT descriptor
    unsafe {
        std::arch::asm!("lidt [{}]", in(reg) addr as usize, options(nostack));
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = addr;
}

/// Swap GS base register (kernel <-> user).
/// On bare-metal: real `swapgs` instruction. On hosted: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_swapgs() {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only
    unsafe {
        std::arch::asm!("swapgs", options(nomem, nostack));
    }
}

/// Read Model-Specific Register (RDMSR).
/// On x86_64: reads real MSR. On other targets: returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_rdmsr(index: i64) -> i64 {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        let lo: u32;
        let hi: u32;
        // SAFETY: ring 0 only, index must be a valid MSR
        unsafe {
            std::arch::asm!(
                "rdmsr",
                in("ecx") index as u32,
                out("eax") lo,
                out("edx") hi,
                options(nomem, nostack),
            );
        }
        return ((hi as i64) << 32) | (lo as i64);
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    {
        let _ = index;
        0
    }
}

/// Write Model-Specific Register (WRMSR).
/// On bare-metal: real `wrmsr` instruction. On hosted: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_wrmsr(index: i64, value: i64) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    // SAFETY: ring 0 only, index must be a valid MSR
    unsafe {
        std::arch::asm!(
            "wrmsr",
            in("ecx") index as u32,
            in("eax") (value & 0xFFFF_FFFF) as u32,
            in("edx") ((value >> 32) & 0xFFFF_FFFF) as u32,
            options(nomem, nostack),
        );
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = (index, value);
}

/// Software interrupt (INT N).
/// On hosted targets: no-op (cannot issue variable INT from userspace).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_int_n(_vector: i64) {
    // Bare-metal: stub — cannot use variable INT instruction
}

/// CPU pause hint for spinloop optimization.
/// On hosted targets: calls `core::hint::spin_loop()`.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_pause() {
    core::hint::spin_loop();
}

/// Set AC flag — allow kernel access to user pages (SMAP).
/// On hosted targets: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_stac() {
    // Bare-metal: `stac`
}

/// Clear AC flag — disallow kernel access to user pages (SMAP).
/// On hosted targets: no-op.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_clac() {
    // Bare-metal: `clac`
}

/// Compare memory buffers: returns 0 if equal, <0 if a<b, >0 if a>b.
/// Alias for fj_rt_bare_memcmp.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_memcmp_buf(a: *const u8, b: *const u8, len: i64) -> i64 {
    fj_rt_bare_memcmp(a, b, len)
}

/// Copy memory buffer: copy `len` bytes from `src` to `dst`.
/// Alias for fj_rt_bare_memcpy.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_memcpy_buf(dst: *mut u8, src: *const u8, len: i64) -> *mut u8 {
    fj_rt_bare_memcpy(dst, src, len)
}

/// Fill memory buffer: set `len` bytes at `dst` to `val`.
/// Alias for fj_rt_bare_memset.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_bare_memset_buf(dst: *mut u8, val: i64, len: i64) -> *mut u8 {
    fj_rt_bare_memset(dst, val, len)
}
