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
#[no_mangle]
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
            unsafe { *dst64.add(i) = *src64.add(i) };
        }
        // Copy remaining bytes
        let remaining = count % 8;
        let offset = words * 8;
        for i in 0..remaining {
            unsafe { *dst.add(offset + i) = *src.add(offset + i) };
        }
    } else {
        // Byte-by-byte fallback
        for i in 0..count {
            unsafe { *dst.add(i) = *src.add(i) };
        }
    }
    dst
}

/// Bare-metal memset: fill `n` bytes at `dst` with `val`.
///
/// # Safety
/// Caller must ensure `dst` is a valid pointer.
#[no_mangle]
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
            unsafe { *dst64.add(i) = fill_word };
        }
        let remaining = count % 8;
        let offset = words * 8;
        for i in 0..remaining {
            unsafe { *dst.add(offset + i) = byte };
        }
    } else {
        for i in 0..count {
            unsafe { *dst.add(i) = byte };
        }
    }
    dst
}

/// Bare-metal memcmp: compare `n` bytes at `a` and `b`.
/// Returns 0 if equal, <0 if a<b, >0 if a>b.
#[no_mangle]
pub extern "C" fn fj_rt_bare_memcmp(a: *const u8, b: *const u8, n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let count = n as usize;
    for i in 0..count {
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
#[no_mangle]
pub extern "C" fn fj_rt_bare_print(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    for i in 0..len as usize {
        uart_putc(unsafe { *ptr.add(i) });
    }
}

/// Bare-metal print integer to UART.
#[no_mangle]
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
#[no_mangle]
pub extern "C" fn fj_rt_bare_set_uart_base(addr: u64) {
    UART_BASE.store(addr, Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════
// Panic Handler
// ═══════════════════════════════════════════════════════════════════════

/// Bare-metal panic: print "PANIC" + halt CPU in WFE loop.
#[no_mangle]
pub extern "C" fn fj_rt_bare_panic() {
    let msg = b"PANIC: kernel halt\n";
    fj_rt_bare_print(msg.as_ptr(), msg.len() as i64);
    fj_rt_bare_halt();
}

/// Halt the CPU in an infinite WFE (wait-for-event) loop.
#[no_mangle]
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
#[no_mangle]
pub extern "C" fn fj_rt_bare_heap_init(base: u64, size: u64) {
    HEAP_BASE.store(base, Ordering::Relaxed);
    HEAP_END.store(base + size, Ordering::Relaxed);
    BUMP_PTR.store(base, Ordering::Relaxed);
}

/// Bump allocator: allocate `size` bytes aligned to 8 bytes.
/// Returns pointer to allocated memory, or 0 (null) if OOM.
#[no_mangle]
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
#[no_mangle]
pub extern "C" fn fj_rt_bare_free(_ptr: u64, _size: i64) {
    // No-op: bump allocator doesn't support individual frees
}

/// Returns the current heap usage in bytes.
#[no_mangle]
pub extern "C" fn fj_rt_bare_heap_used() -> u64 {
    let base = HEAP_BASE.load(Ordering::Relaxed);
    let ptr = BUMP_PTR.load(Ordering::Relaxed);
    ptr.saturating_sub(base)
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
}
