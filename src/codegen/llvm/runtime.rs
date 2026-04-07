//! Runtime functions for the LLVM JIT backend.
//!
//! These `extern "C"` functions are mapped into the LLVM JIT execution engine
//! so that compiled code can call into the Rust runtime for I/O and memory.
//!
//! When the `native` feature is also enabled, the Cranelift runtime_fns module
//! is used instead (it has the full set of ~200+ functions). This module
//! provides the essential subset needed for LLVM-only builds.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::io::Write;

// ═══════════════════════════════════════════════════════════════════════
// I/O: Integer print
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_println_int(val: i64) {
    println!("{val}");
}

pub extern "C" fn fj_rt_print_int(val: i64) {
    print!("{val}");
    let _ = std::io::stdout().flush();
}

// ═══════════════════════════════════════════════════════════════════════
// I/O: Float print
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_println_f64(val: f64) {
    println!("{val}");
}

pub extern "C" fn fj_rt_print_f64(val: f64) {
    print!("{val}");
    let _ = std::io::stdout().flush();
}

// ═══════════════════════════════════════════════════════════════════════
// I/O: Bool print
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_println_bool(val: i64) {
    println!("{}", if val != 0 { "true" } else { "false" });
}

pub extern "C" fn fj_rt_print_bool(val: i64) {
    print!("{}", if val != 0 { "true" } else { "false" });
    let _ = std::io::stdout().flush();
}

// ═══════════════════════════════════════════════════════════════════════
// I/O: String print (ptr + len)
// ═══════════════════════════════════════════════════════════════════════

/// # Safety
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_println_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        println!();
        return;
    }
    // SAFETY: string data lives in the JIT module's static data section.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    println!("{s}");
}

/// # Safety
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_print_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    // SAFETY: string data lives in the JIT module's static data section.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    print!("{s}");
    let _ = std::io::stdout().flush();
}

// ═══════════════════════════════════════════════════════════════════════
// I/O: Stderr (eprintln / eprint)
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_eprintln_int(val: i64) {
    eprintln!("{val}");
}

pub extern "C" fn fj_rt_eprint_int(val: i64) {
    eprint!("{val}");
    let _ = std::io::stderr().flush();
}

/// # Safety
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_eprintln_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        eprintln!();
        return;
    }
    // SAFETY: string data lives in the JIT module's static data section.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    eprintln!("{s}");
}

/// # Safety
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_eprint_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    // SAFETY: string data lives in the JIT module's static data section.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    eprint!("{s}");
    let _ = std::io::stderr().flush();
}

pub extern "C" fn fj_rt_eprintln_f64(val: f64) {
    eprintln!("{val}");
}

pub extern "C" fn fj_rt_eprintln_bool(val: i64) {
    eprintln!("{}", if val != 0 { "true" } else { "false" });
}

// ═══════════════════════════════════════════════════════════════════════
// Memory: alloc / free
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_alloc(size: i64) -> *mut u8 {
    if size <= 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: size > 0, layout is valid.
    unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(size as usize, 8);
        std::alloc::alloc(layout)
    }
}

pub extern "C" fn fj_rt_free(ptr: *mut u8, size: i64) {
    if ptr.is_null() || size <= 0 {
        return;
    }
    // SAFETY: ptr was allocated with the same layout.
    unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(size as usize, 8);
        std::alloc::dealloc(ptr, layout);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// String operations
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_str_len(_ptr: *const u8, len: i64) -> i64 {
    len
}

/// # Safety
/// Both string pointers must be valid UTF-8 data of at least their respective lengths.
pub extern "C" fn fj_rt_str_concat(
    ptr1: *const u8,
    len1: i64,
    ptr2: *const u8,
    len2: i64,
) -> *const u8 {
    let s1 = if ptr1.is_null() || len1 <= 0 {
        &[]
    } else {
        // SAFETY: caller guarantees valid pointer.
        unsafe { std::slice::from_raw_parts(ptr1, len1 as usize) }
    };
    let s2 = if ptr2.is_null() || len2 <= 0 {
        &[]
    } else {
        // SAFETY: caller guarantees valid pointer.
        unsafe { std::slice::from_raw_parts(ptr2, len2 as usize) }
    };
    let mut result = Vec::with_capacity(s1.len() + s2.len());
    result.extend_from_slice(s1);
    result.extend_from_slice(s2);
    let ptr = result.as_ptr();
    std::mem::forget(result);
    ptr
}

// ═══════════════════════════════════════════════════════════════════════
// Type conversion (for f-strings)
// ═══════════════════════════════════════════════════════════════════════

/// Converts an i64 to a heap-allocated string, writing (ptr, len) to out params.
pub extern "C" fn fj_rt_int_to_string(val: i64, out_ptr: *mut *mut u8, out_len: *mut i64) {
    let s = val.to_string();
    let len = s.len();
    // SAFETY: layout is valid (len >= 1 for any integer, align 8).
    let buf = unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(len.max(1), 8);
        std::alloc::alloc(layout)
    };
    // SAFETY: buf is freshly allocated with sufficient size.
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len);
        *out_ptr = buf;
        *out_len = len as i64;
    }
}

/// Converts an f64 to a heap-allocated string, writing (ptr, len) to out params.
pub extern "C" fn fj_rt_float_to_string(val: f64, out_ptr: *mut *mut u8, out_len: *mut i64) {
    let s = format!("{val}");
    let len = s.len();
    // SAFETY: layout is valid.
    let buf = unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(len.max(1), 8);
        std::alloc::alloc(layout)
    };
    // SAFETY: buf is freshly allocated with sufficient size.
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len);
        *out_ptr = buf;
        *out_len = len as i64;
    }
}

/// Converts a bool (i64: 0=false, nonzero=true) to a heap-allocated string.
pub extern "C" fn fj_rt_bool_to_string(val: i64, out_ptr: *mut *mut u8, out_len: *mut i64) {
    let s = if val != 0 { "true" } else { "false" };
    let len = s.len();
    // SAFETY: layout is valid.
    let buf = unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(len, 8);
        std::alloc::alloc(layout)
    };
    // SAFETY: buf is freshly allocated with sufficient size.
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len);
        *out_ptr = buf;
        *out_len = len as i64;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Assert
// ═══════════════════════════════════════════════════════════════════════

pub extern "C" fn fj_rt_assert(cond: i64) {
    if cond == 0 {
        eprintln!("assertion failed!");
        std::process::exit(1);
    }
}

pub extern "C" fn fj_rt_assert_eq(a: i64, b: i64) {
    if a != b {
        eprintln!("assertion failed: {a} != {b}");
        std::process::exit(1);
    }
}
