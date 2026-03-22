//! FajarOS Bare-Metal Runtime Library
//!
//! Provides extern "C" functions required by Fajar Lang bare-metal codegen.
//! Compiled as static library (.a) and linked into bare-metal ELFs.
//!
//! Build: cargo build --release --target aarch64-unknown-linux-gnu
//!   (or aarch64-unknown-none for true no_std)

#![no_std]

use core::ptr;

// ── Volatile Memory Access ──

#[no_mangle]
pub extern "C" fn fj_rt_volatile_read_u64(addr: *const u64) -> i64 {
    unsafe { ptr::read_volatile(addr) as i64 }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_write_u64(addr: *mut u64, value: i64) {
    unsafe { ptr::write_volatile(addr, value as u64) }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_read_u32(addr: *const u32) -> i64 {
    unsafe { ptr::read_volatile(addr) as i64 }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_write_u32(addr: *mut u32, value: i64) {
    unsafe { ptr::write_volatile(addr, value as u32) }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_read_u16(addr: *const u16) -> i64 {
    unsafe { ptr::read_volatile(addr) as i64 }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_write_u16(addr: *mut u16, value: i64) {
    unsafe { ptr::write_volatile(addr, value as u16) }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_read_u8(addr: *const u8) -> i64 {
    unsafe { ptr::read_volatile(addr) as i64 }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_write_u8(addr: *mut u8, value: i64) {
    unsafe { ptr::write_volatile(addr, value as u8) }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_read(addr: *const i64) -> i64 {
    unsafe { ptr::read_volatile(addr) }
}

#[no_mangle]
pub extern "C" fn fj_rt_volatile_write(addr: *mut i64, value: i64) {
    unsafe { ptr::write_volatile(addr, value) }
}

// ── String Operations ──

#[no_mangle]
pub extern "C" fn fj_rt_str_len(ptr: *const u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let mut len: i64 = 0;
    unsafe {
        while *ptr.offset(len as isize) != 0 {
            len += 1;
            if len > 65536 {
                break;
            }
        }
    }
    len
}

#[no_mangle]
pub extern "C" fn fj_rt_str_byte_at(ptr: *const u8, index: i64) -> i64 {
    if ptr.is_null() || index < 0 {
        return 0;
    }
    unsafe { *ptr.offset(index as isize) as i64 }
}

// ── Timer (ARM64 Generic Timer) ──

#[no_mangle]
pub extern "C" fn fj_rt_bare_timer_get_freq() -> i64 {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq);
    }
    freq as i64
}

#[no_mangle]
pub extern "C" fn fj_rt_bare_timer_get_ticks() -> i64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntpct_el0", out(reg) ticks);
    }
    ticks as i64
}

// ── Memory Operations ──

#[no_mangle]
pub extern "C" fn fj_rt_bare_memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dst.is_null() || src.is_null() {
        return dst;
    }
    unsafe {
        for i in 0..n {
            *dst.add(i) = *src.add(i);
        }
    }
    dst
}

#[no_mangle]
pub extern "C" fn fj_rt_bare_memset(dst: *mut u8, val: i32, n: usize) -> *mut u8 {
    if dst.is_null() {
        return dst;
    }
    unsafe {
        for i in 0..n {
            *dst.add(i) = val as u8;
        }
    }
    dst
}

// ── Print (UART PL011 at 0x09000000 for QEMU virt) ──

const UART_BASE: usize = 0x0900_0000;

#[no_mangle]
pub extern "C" fn fj_rt_bare_print(ptr: *const u8, len: i64) {
    if ptr.is_null() {
        return;
    }
    let uart = UART_BASE as *mut u8;
    for i in 0..len as usize {
        unsafe {
            let ch = *ptr.add(i);
            if ch == b'\n' {
                ptr::write_volatile(uart, b'\r');
            }
            ptr::write_volatile(uart, ch);
        }
    }
}

#[no_mangle]
pub extern "C" fn fj_rt_bare_println(ptr: *const u8, len: i64) {
    fj_rt_bare_print(ptr, len);
    let uart = UART_BASE as *mut u8;
    unsafe {
        ptr::write_volatile(uart, b'\r');
        ptr::write_volatile(uart, b'\n');
    }
}

// ── Atomic Operations ──

#[no_mangle]
pub extern "C" fn fj_rt_bare_atomic_cas(addr: *mut i64, expected: i64, desired: i64) -> i64 {
    unsafe {
        let atomic = &*(addr as *const core::sync::atomic::AtomicI64);
        match atomic.compare_exchange(
            expected,
            desired,
            core::sync::atomic::Ordering::SeqCst,
            core::sync::atomic::Ordering::SeqCst,
        ) {
            Ok(old) => old,
            Err(old) => old,
        }
    }
}

#[no_mangle]
pub extern "C" fn fj_rt_bare_atomic_load(addr: *const i64) -> i64 {
    unsafe {
        let atomic = &*(addr as *const core::sync::atomic::AtomicI64);
        atomic.load(core::sync::atomic::Ordering::SeqCst)
    }
}

#[no_mangle]
pub extern "C" fn fj_rt_bare_atomic_store(addr: *mut i64, value: i64) {
    unsafe {
        let atomic = &*(addr as *const core::sync::atomic::AtomicI64);
        atomic.store(value, core::sync::atomic::Ordering::SeqCst);
    }
}

// ── Panic Handler (required by #![no_std]) ──

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Write "PANIC" to UART
    let uart = UART_BASE as *mut u8;
    for ch in b"PANIC\r\n" {
        unsafe {
            core::ptr::write_volatile(uart, *ch);
        }
    }
    loop {}
}
