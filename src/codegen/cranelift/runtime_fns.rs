//! Runtime functions callable from compiled Fajar Lang code.
//!
//! These `extern "C"` functions are registered as JIT symbols (or imported in AOT mode)
//! so that compiled code can call into the Rust runtime for I/O, memory management,
//! string operations, math, and file I/O.
//!
//! # Safety
//! All functions in this module use `extern "C"` ABI with raw pointer arguments.
//! Callers (JIT/AOT compiled code) guarantee valid, non-null pointers.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::runtime_bare;

// ═══════════════════════════════════════════════════════════════════════
// I/O: Integer, Float, Bool, String print
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: prints an i64 followed by a newline.
pub extern "C" fn fj_rt_print_i64(val: i64) {
    println!("{val}");
}

/// Runtime: prints an i64 without a newline.
pub extern "C" fn fj_rt_print_i64_no_newline(val: i64) {
    print!("{val}");
}

/// Runtime: prints an f64 followed by a newline.
pub extern "C" fn fj_rt_println_f64(val: f64) {
    println!("{val}");
}

/// Runtime: prints an f64 without a newline.
pub extern "C" fn fj_rt_print_f64_no_newline(val: f64) {
    print!("{val}");
}

/// Runtime: prints a string (ptr + len) followed by a newline.
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_println_str(ptr: *const u8, len: i64) {
    // SAFETY: string data lives in the static data section, guaranteed valid for program lifetime.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    println!("{s}");
}

/// Runtime: prints a string (ptr + len) without a newline.
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_print_str(ptr: *const u8, len: i64) {
    // SAFETY: string data lives in the static data section, guaranteed valid for program lifetime.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    print!("{s}");
}

/// Runtime: prints "true" or "false" followed by a newline.
pub extern "C" fn fj_rt_println_bool(val: i64) {
    if val != 0 {
        println!("true");
    } else {
        println!("false");
    }
}

/// Runtime: prints `[dbg] <i64>` to stderr and returns the value.
pub extern "C" fn fj_rt_dbg_i64(val: i64) {
    eprintln!("[dbg] {val}");
}

/// Runtime: prints `[dbg] <str>` to stderr.
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_dbg_str(ptr: *const u8, len: i64) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    eprintln!("[dbg] {s}");
}

/// Runtime: prints `[dbg] <f64>` to stderr.
pub extern "C" fn fj_rt_dbg_f64(val: f64) {
    eprintln!("[dbg] {val}");
}

// ═══════════════════════════════════════════════════════════════════════
// Stderr output
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: prints an i64 to stderr followed by a newline.
pub extern "C" fn fj_rt_eprintln_i64(val: i64) {
    eprintln!("{val}");
}

/// Runtime: prints a string (ptr + len) to stderr followed by a newline.
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_eprintln_str(ptr: *const u8, len: i64) {
    // SAFETY: caller guarantees valid string slice
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    eprintln!("{s}");
}

/// Runtime: prints an f64 to stderr followed by a newline.
pub extern "C" fn fj_rt_eprintln_f64(val: f64) {
    eprintln!("{val}");
}

/// Runtime: prints a bool to stderr followed by a newline.
pub extern "C" fn fj_rt_eprintln_bool(val: i64) {
    if val != 0 {
        eprintln!("true");
    } else {
        eprintln!("false");
    }
}

/// Runtime: prints an i64 to stderr without a newline.
pub extern "C" fn fj_rt_eprint_i64(val: i64) {
    eprint!("{val}");
}

/// Runtime: prints a string (ptr + len) to stderr without a newline.
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_eprint_str(ptr: *const u8, len: i64) {
    // SAFETY: caller guarantees valid string slice
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = std::str::from_utf8(slice).unwrap_or("<invalid utf-8>");
    eprint!("{s}");
}

// ═══════════════════════════════════════════════════════════════════════
// String formatting & parsing
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: formats a template string with arguments, producing a heap-allocated result.
///
/// `args_ptr` points to an array of `(type_tag: i64, val1: i64, val2: i64)` triples.
/// - type 0: i64 (val1 = value, val2 = unused)
/// - type 1: f64 (val1 = f64 bits as i64, val2 = unused)
/// - type 2: bool (val1 = 0/1, val2 = unused)
/// - type 3: string (val1 = ptr as i64, val2 = len)
///
/// # Safety
///
/// Template pointer must be valid UTF-8. `args_ptr` must point to `num_args * 3` i64 values.
pub extern "C" fn fj_rt_format(
    tpl_ptr: *const u8,
    tpl_len: i64,
    args_ptr: *const i64,
    num_args: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid inputs
    let tpl = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(tpl_ptr, tpl_len as usize))
    };
    let args = unsafe { std::slice::from_raw_parts(args_ptr, (num_args as usize) * 3) };

    let mut result = String::new();
    let mut arg_idx: usize = 0;
    let mut chars = tpl.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'}') {
            chars.next(); // consume '}'
            if arg_idx < num_args as usize {
                let base = arg_idx * 3;
                let tag = args[base];
                let val1 = args[base + 1];
                let val2 = args[base + 2];
                match tag {
                    0 => result.push_str(&val1.to_string()),
                    1 => {
                        let f = f64::from_bits(val1 as u64);
                        result.push_str(&format!("{f}"));
                    }
                    2 => result.push_str(if val1 != 0 { "true" } else { "false" }),
                    3 => {
                        // SAFETY: val1 is a valid pointer, val2 is length
                        let s = unsafe {
                            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                                val1 as *const u8,
                                val2 as usize,
                            ))
                        };
                        result.push_str(s);
                    }
                    _ => result.push_str("{}"),
                }
                arg_idx += 1;
            } else {
                result.push_str("{}");
            }
        } else {
            result.push(c);
        }
    }

    let bytes = result.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("format alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: parses a string to i64, returns (tag, value).
/// tag=0 → Ok(value), tag=1 → Err(0).
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_parse_int(ptr: *const u8, len: i64, out_tag: *mut i64, out_val: *mut i64) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    match s.trim().parse::<i64>() {
        Ok(n) => unsafe {
            *out_tag = 0;
            *out_val = n;
        },
        Err(_) => unsafe {
            *out_tag = 1;
            *out_val = 0;
        },
    }
}

/// Runtime: parses a string to f64, returns (tag, value_bits).
/// tag=0 → Ok(value), tag=1 → Err(0).
///
/// # Safety
///
/// The caller must ensure `ptr` points to valid UTF-8 data of at least `len` bytes.
pub extern "C" fn fj_rt_parse_float(
    ptr: *const u8,
    len: i64,
    out_tag: *mut i64,
    out_val: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    match s.trim().parse::<f64>() {
        Ok(f) => unsafe {
            *out_tag = 0;
            *out_val = f.to_bits() as i64;
        },
        Err(_) => unsafe {
            *out_tag = 1;
            *out_val = 0;
        },
    }
}

/// Runtime: prints "true" or "false" without a newline.
pub extern "C" fn fj_rt_print_bool(val: i64) {
    if val != 0 {
        print!("true");
    } else {
        print!("false");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Global allocator dispatch
// ═══════════════════════════════════════════════════════════════════════

use std::sync::atomic::AtomicPtr;

/// Default system allocator: allocates using `std::alloc::alloc`.
extern "C" fn default_alloc(size: i64) -> *mut u8 {
    let layout = std::alloc::Layout::from_size_align(size as usize, 8).expect("invalid alloc size");
    // SAFETY: layout is valid (size > 0, align = 8)
    unsafe { std::alloc::alloc(layout) }
}

/// Default system deallocator: frees using `std::alloc::dealloc`.
extern "C" fn default_free(ptr: *mut u8, size: i64) {
    let layout = std::alloc::Layout::from_size_align(size as usize, 8).expect("invalid free size");
    // SAFETY: ptr was allocated with the same layout.
    unsafe { std::alloc::dealloc(ptr, layout) }
}

/// Global allocator function pointer (alloc).
static GLOBAL_ALLOC: AtomicPtr<()> = AtomicPtr::new(default_alloc as *mut ());

/// Global deallocator function pointer (free).
static GLOBAL_FREE: AtomicPtr<()> = AtomicPtr::new(default_free as *mut ());

// ═══════════════════════════════════════════════════════════════════════
// Memory management
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: allocates `size` bytes through the global allocator.
///
/// # Safety
///
/// Caller must eventually call `fj_rt_free` on the returned pointer.
pub extern "C" fn fj_rt_alloc(size: i64) -> *mut u8 {
    use std::sync::atomic::Ordering;
    let alloc_fn: extern "C" fn(i64) -> *mut u8 =
        // SAFETY: GLOBAL_ALLOC always points to a valid extern "C" fn(i64) -> *mut u8
        unsafe { std::mem::transmute(GLOBAL_ALLOC.load(Ordering::Relaxed)) };
    alloc_fn(size)
}

/// Runtime: frees memory through the global allocator.
///
/// # Safety
///
/// `ptr` must have been returned by `fj_rt_alloc` with the same `size`.
pub extern "C" fn fj_rt_free(ptr: *mut u8, size: i64) {
    use std::sync::atomic::Ordering;
    let free_fn: extern "C" fn(*mut u8, i64) =
        // SAFETY: GLOBAL_FREE always points to a valid extern "C" fn(*mut u8, i64)
        unsafe { std::mem::transmute(GLOBAL_FREE.load(Ordering::Relaxed)) };
    free_fn(ptr, size)
}

/// Runtime: sets a custom global allocator.
///
/// `alloc_fn_ptr` is a function pointer: `extern "C" fn(size: i64) -> *mut u8`
/// `free_fn_ptr` is a function pointer: `extern "C" fn(ptr: *mut u8, size: i64)`
///
/// Pass 0 (null) for either to reset to the default system allocator.
pub extern "C" fn fj_rt_set_global_allocator(alloc_fn_ptr: i64, free_fn_ptr: i64) {
    use std::sync::atomic::Ordering;
    if alloc_fn_ptr == 0 || free_fn_ptr == 0 {
        GLOBAL_ALLOC.store(default_alloc as *mut (), Ordering::Relaxed);
        GLOBAL_FREE.store(default_free as *mut (), Ordering::Relaxed);
    } else {
        GLOBAL_ALLOC.store(alloc_fn_ptr as *mut (), Ordering::Relaxed);
        GLOBAL_FREE.store(free_fn_ptr as *mut (), Ordering::Relaxed);
    }
}

/// Runtime: resets global allocator to the default system allocator.
pub extern "C" fn fj_rt_reset_global_allocator() {
    use std::sync::atomic::Ordering;
    GLOBAL_ALLOC.store(default_alloc as *mut (), Ordering::Relaxed);
    GLOBAL_FREE.store(default_free as *mut (), Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════
// Dynamic arrays (Vec-backed)
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: creates a new heap-allocated dynamic array with the given capacity.
///
/// Returns an opaque pointer to a `Box<Vec<i64>>` on the Rust heap.
pub extern "C" fn fj_rt_array_new(cap: i64) -> *mut u8 {
    let vec: Vec<i64> = Vec::with_capacity(cap.max(0) as usize);
    Box::into_raw(Box::new(vec)) as *mut u8
}

/// Runtime: pushes a value onto a dynamic array.
///
/// # Safety
///
/// `arr` must have been returned by `fj_rt_array_new`.
pub extern "C" fn fj_rt_array_push(arr: *mut u8, val: i64) {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &mut *(arr as *mut Vec<i64>) };
    vec.push(val);
}

/// Runtime: gets the element at `idx` in a dynamic array.
///
/// # Safety
///
/// `arr` must have been returned by `fj_rt_array_new`.
/// `idx` must be in range `0..len`.
pub extern "C" fn fj_rt_array_get(arr: *mut u8, idx: i64) -> i64 {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &*(arr as *mut Vec<i64>) };
    vec[idx as usize]
}

/// Runtime: sets the element at `idx` in a dynamic array.
///
/// # Safety
///
/// `arr` must have been returned by `fj_rt_array_new`.
/// `idx` must be in range `0..len`.
pub extern "C" fn fj_rt_array_set(arr: *mut u8, idx: i64, val: i64) {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &mut *(arr as *mut Vec<i64>) };
    vec[idx as usize] = val;
}

/// Runtime: returns the length of a dynamic array.
///
/// # Safety
///
/// `arr` must have been returned by `fj_rt_array_new`.
pub extern "C" fn fj_rt_array_len(arr: *mut u8) -> i64 {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &*(arr as *mut Vec<i64>) };
    vec.len() as i64
}

/// Runtime: pops the last element from a dynamic array, returns 0 if empty.
///
/// # Safety
///
/// `arr` must have been returned by `fj_rt_array_new`.
pub extern "C" fn fj_rt_array_pop(arr: *mut u8) -> i64 {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &mut *(arr as *mut Vec<i64>) };
    vec.pop().unwrap_or(0)
}

/// Runtime: frees a dynamic array created by `fj_rt_array_new`.
///
/// # Safety
///
/// `arr` must have been returned by `fj_rt_array_new` and must not be used after this call.
pub extern "C" fn fj_rt_array_free(arr: *mut u8) {
    if arr.is_null() {
        return;
    }
    // SAFETY: arr was created by fj_rt_array_new and is non-null
    let _ = unsafe { Box::from_raw(arr as *mut Vec<i64>) };
}

/// Runtime: checks if a heap array contains a given element.
pub extern "C" fn fj_rt_array_contains(arr: *mut u8, val: i64) -> i64 {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &*(arr as *mut Vec<i64>) };
    if vec.contains(&val) {
        1
    } else {
        0
    }
}

/// Runtime: checks if a heap array is empty.
pub extern "C" fn fj_rt_array_is_empty(arr: *mut u8) -> i64 {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &*(arr as *mut Vec<i64>) };
    if vec.is_empty() {
        1
    } else {
        0
    }
}

/// Runtime: reverses a heap array in place, returns 0.
pub extern "C" fn fj_rt_array_reverse(arr: *mut u8) -> i64 {
    // SAFETY: arr was created by fj_rt_array_new
    let vec = unsafe { &mut *(arr as *mut Vec<i64>) };
    vec.reverse();
    0
}

// ═══════════════════════════════════════════════════════════════════════
// String operations
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: concatenates two strings (ptr+len pairs), returns new (ptr, len).
///
/// The result is heap-allocated and must be freed by the caller.
///
/// # Safety
///
/// Both input pointers must point to valid UTF-8 data of the given lengths.
pub extern "C" fn fj_rt_str_concat(
    a_ptr: *const u8,
    a_len: i64,
    b_ptr: *const u8,
    b_len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    let a_sz = a_len as usize;
    let b_sz = b_len as usize;
    let total = a_sz + b_sz;
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("concat alloc");
    // SAFETY: layout is valid (size >= 1)
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: copy only when pointers are valid (non-null, len > 0)
    unsafe {
        if a_sz > 0 && !a_ptr.is_null() {
            std::ptr::copy_nonoverlapping(a_ptr, buf, a_sz);
        }
        if b_sz > 0 && !b_ptr.is_null() {
            std::ptr::copy_nonoverlapping(b_ptr, buf.add(a_sz), b_sz);
        }
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: checks whether `haystack` contains `needle`.
///
/// # Safety
///
/// Both pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_eq(a_ptr: *const u8, a_len: i64, b_ptr: *const u8, b_len: i64) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let a = unsafe { std::slice::from_raw_parts(a_ptr, a_len as usize) };
    let b = unsafe { std::slice::from_raw_parts(b_ptr, b_len as usize) };
    if a == b {
        1
    } else {
        0
    }
}

/// Runtime: checks if haystack contains needle.
///
/// Both pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_contains(
    h_ptr: *const u8,
    h_len: i64,
    n_ptr: *const u8,
    n_len: i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let haystack = unsafe { std::slice::from_raw_parts(h_ptr, h_len as usize) };
    let needle = unsafe { std::slice::from_raw_parts(n_ptr, n_len as usize) };
    let h = unsafe { std::str::from_utf8_unchecked(haystack) };
    let n = unsafe { std::str::from_utf8_unchecked(needle) };
    if h.contains(n) {
        1
    } else {
        0
    }
}

/// Runtime: checks whether `haystack` starts with `prefix`.
///
/// # Safety
///
/// Both pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_starts_with(
    h_ptr: *const u8,
    h_len: i64,
    p_ptr: *const u8,
    p_len: i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let haystack = unsafe { std::slice::from_raw_parts(h_ptr, h_len as usize) };
    let prefix = unsafe { std::slice::from_raw_parts(p_ptr, p_len as usize) };
    let h = unsafe { std::str::from_utf8_unchecked(haystack) };
    let p = unsafe { std::str::from_utf8_unchecked(prefix) };
    if h.starts_with(p) {
        1
    } else {
        0
    }
}

/// Runtime: checks whether `haystack` ends with `suffix`.
///
/// # Safety
///
/// Both pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_ends_with(
    h_ptr: *const u8,
    h_len: i64,
    s_ptr: *const u8,
    s_len: i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let haystack = unsafe { std::slice::from_raw_parts(h_ptr, h_len as usize) };
    let suffix = unsafe { std::slice::from_raw_parts(s_ptr, s_len as usize) };
    let h = unsafe { std::str::from_utf8_unchecked(haystack) };
    let s = unsafe { std::str::from_utf8_unchecked(suffix) };
    if h.ends_with(s) {
        1
    } else {
        0
    }
}

/// Runtime: finds the first occurrence of `needle` in `haystack`.
///
/// Returns the byte offset (as `i64`) or `-1` if not found.
///
/// # Safety
///
/// Both pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_index_of(
    h_ptr: *const u8,
    h_len: i64,
    n_ptr: *const u8,
    n_len: i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let haystack = unsafe { std::slice::from_raw_parts(h_ptr, h_len as usize) };
    let needle = unsafe { std::slice::from_raw_parts(n_ptr, n_len as usize) };
    let h = unsafe { std::str::from_utf8_unchecked(haystack) };
    let n = unsafe { std::str::from_utf8_unchecked(needle) };
    match h.find(n) {
        Some(pos) => pos as i64,
        None => -1,
    }
}

/// Runtime: joins array elements into a string with a separator.
///
/// `arr_ptr` is a `*mut Vec<i64>` (heap array). Each element is treated as i64.
/// Returns a new heap-allocated string via `out_ptr`/`out_len`.
///
/// # Safety
///
/// `arr_ptr` must point to a valid `Vec<i64>`. Separator must be valid UTF-8.
pub extern "C" fn fj_rt_array_join(
    arr_ptr: *mut u8,
    sep_ptr: *const u8,
    sep_len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid vec pointer and string slice
    let arr: &Vec<i64> = unsafe { &*(arr_ptr as *const Vec<i64>) };
    let sep = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(sep_ptr, sep_len as usize))
    };
    let result: String = arr
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(sep);
    let bytes = result.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("join alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: repeats a string `n` times, returns heap-allocated result.
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_repeat(
    ptr: *const u8,
    len: i64,
    count: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let result = s.repeat(count.max(0) as usize);
    let bytes = result.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("repeat alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: reverses a string's characters, returns heap-allocated result.
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_rev(
    ptr: *const u8,
    len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let result: String = s.chars().rev().collect();
    let bytes = result.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("rev alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: returns a heap array of Unicode code-points (char as i64) from a string.
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_chars(ptr: *const u8, len: i64) -> *mut u8 {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let vec: Vec<i64> = s.chars().map(|c| c as i64).collect();
    Box::into_raw(Box::new(vec)) as *mut u8
}

/// Runtime: returns a heap array of raw bytes (u8 as i64) from a string.
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_bytes(ptr: *const u8, len: i64) -> *mut u8 {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let vec: Vec<i64> = s.iter().map(|&b| b as i64).collect();
    Box::into_raw(Box::new(vec)) as *mut u8
}

/// Runtime: splits a string by a delimiter, returns a heap-allocated array.
///
/// Each element in the returned `Vec<i64>` is a pair `(ptr_as_i64, len)`.
/// The logical length is `vec.len() / 2`.
///
/// # Safety
///
/// Input pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_split(
    ptr: *const u8,
    len: i64,
    sep_ptr: *const u8,
    sep_len: i64,
) -> *mut u8 {
    // SAFETY: caller guarantees valid string slices
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let sep = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(sep_ptr, sep_len as usize))
    };
    let parts: Vec<String> = s.split(sep).map(|p| p.to_string()).collect();
    // Flatten into pairs of (ptr, len) as i64 values
    let mut flat: Vec<i64> = Vec::with_capacity(parts.len() * 2);
    // We need to keep the allocated strings alive — leak them into heap
    for part in parts {
        let bytes = part.into_bytes();
        let part_len = bytes.len() as i64;
        let layout =
            std::alloc::Layout::from_size_align(bytes.len().max(1), 8).expect("split alloc");
        // SAFETY: layout is valid
        let buf = unsafe { std::alloc::alloc(layout) };
        // SAFETY: buf is valid for bytes.len() bytes
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
        }
        flat.push(buf as i64);
        flat.push(part_len);
    }
    Box::into_raw(Box::new(flat)) as *mut u8
}

/// Runtime: returns the number of string parts from a split result.
///
/// # Safety
///
/// Pointer must be from `fj_rt_str_split`.
pub extern "C" fn fj_rt_split_len(arr_ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid Box<Vec<i64>> pointer
    let vec = unsafe { &*(arr_ptr as *const Vec<i64>) };
    (vec.len() / 2) as i64
}

/// Runtime: gets the i-th string from a split result as (ptr, len).
///
/// # Safety
///
/// Pointer must be from `fj_rt_str_split`, index must be in bounds.
pub extern "C" fn fj_rt_split_get(
    arr_ptr: *mut u8,
    index: i64,
    out_ptr: *mut *const u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid Box<Vec<i64>> pointer
    let vec = unsafe { &*(arr_ptr as *const Vec<i64>) };
    let i = index as usize;
    // SAFETY: out_ptr and out_len are valid stack slot pointers
    unsafe {
        *out_ptr = vec[i * 2] as *const u8;
        *out_len = vec[i * 2 + 1];
    }
}

// ═══════════════════════════════════════════════════════════════════════
// String trimming & case conversion
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: trims whitespace from both ends of a string, returns new (ptr, len).
///
/// The result points into the *original* string (no allocation needed for trim).
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_trim(
    ptr: *const u8,
    len: i64,
    out_ptr: *mut *const u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let trimmed = s.trim();
    // SAFETY: out_ptr and out_len are valid stack slot pointers from the caller
    unsafe {
        *out_ptr = trimmed.as_ptr();
        *out_len = trimmed.len() as i64;
    }
}

pub extern "C" fn fj_rt_str_trim_start(
    ptr: *const u8,
    len: i64,
    out_ptr: *mut *const u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let trimmed = s.trim_start();
    // SAFETY: out_ptr and out_len are valid stack slot pointers from the caller
    unsafe {
        *out_ptr = trimmed.as_ptr();
        *out_len = trimmed.len() as i64;
    }
}

pub extern "C" fn fj_rt_str_trim_end(
    ptr: *const u8,
    len: i64,
    out_ptr: *mut *const u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let trimmed = s.trim_end();
    // SAFETY: out_ptr and out_len are valid stack slot pointers from the caller
    unsafe {
        *out_ptr = trimmed.as_ptr();
        *out_len = trimmed.len() as i64;
    }
}

/// Runtime: converts a string to uppercase, returns new heap-allocated string.
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_to_uppercase(
    ptr: *const u8,
    len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let upper = s.to_uppercase();
    let bytes = upper.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("uppercase alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: converts a string to lowercase, returns new heap-allocated string.
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
pub extern "C" fn fj_rt_str_to_lowercase(
    ptr: *const u8,
    len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slice
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
    let lower = s.to_lowercase();
    let bytes = lower.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("lowercase alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: replaces all occurrences of `old` with `new` in a string.
///
/// Returns a new heap-allocated string.
///
/// # Safety
///
/// All input pointers must point to valid UTF-8 of the given lengths.
pub extern "C" fn fj_rt_str_replace(
    h_ptr: *const u8,
    h_len: i64,
    old_ptr: *const u8,
    old_len: i64,
    new_ptr: *const u8,
    new_len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid string slices
    let h =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(h_ptr, h_len as usize)) };
    let old = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(old_ptr, old_len as usize))
    };
    let new = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(new_ptr, new_len as usize))
    };
    let result = h.replace(old, new);
    let bytes = result.into_bytes();
    let total = bytes.len();
    let layout = std::alloc::Layout::from_size_align(total.max(1), 8).expect("replace alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is valid for total bytes
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, total);
        *out_ptr = buf;
        *out_len = total as i64;
    }
}

/// Runtime: returns a substring from `start` to `end` (byte offsets).
///
/// Returns a pointer into the original string (no allocation).
///
/// # Safety
///
/// Input pointer must point to valid UTF-8 of the given length.
/// `start` and `end` must be valid byte offsets within the string.
pub extern "C" fn fj_rt_str_substring(
    ptr: *const u8,
    len: i64,
    start: i64,
    end: i64,
    out_ptr: *mut *const u8,
    out_len: *mut i64,
) {
    let s_start = (start as usize).min(len as usize);
    let s_end = (end as usize).min(len as usize);
    let actual_end = s_end.max(s_start);
    // SAFETY: caller guarantees valid string slice and out pointers
    unsafe {
        *out_ptr = ptr.add(s_start);
        *out_len = (actual_end - s_start) as i64;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Math functions
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: wraps libm `sin`.
pub extern "C" fn fj_rt_math_sin(x: f64) -> f64 {
    x.sin()
}
/// Runtime: wraps libm `cos`.
pub extern "C" fn fj_rt_math_cos(x: f64) -> f64 {
    x.cos()
}
/// Runtime: wraps libm `tan`.
pub extern "C" fn fj_rt_math_tan(x: f64) -> f64 {
    x.tan()
}
/// Runtime: wraps libm `powf`.
pub extern "C" fn fj_rt_math_pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}
/// Runtime: wraps libm `log2`.
pub extern "C" fn fj_rt_math_log(x: f64) -> f64 {
    x.ln()
}

pub extern "C" fn fj_rt_math_log2(x: f64) -> f64 {
    x.log2()
}
/// Runtime: wraps libm `log10`.
pub extern "C" fn fj_rt_math_log10(x: f64) -> f64 {
    x.log10()
}

// ═══════════════════════════════════════════════════════════════════════
// Type conversion
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: converts an i64 to a heap-allocated string, returning (ptr, len).
pub extern "C" fn fj_rt_int_to_string(val: i64, out_ptr: *mut *mut u8, out_len: *mut i64) {
    let s = val.to_string();
    let len = s.len();
    let layout = std::alloc::Layout::from_size_align(len.max(1), 8).expect("int_to_string alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is freshly allocated with sufficient size
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len);
        *out_ptr = buf;
        *out_len = len as i64;
    }
}

/// Runtime: converts an f64 to a heap-allocated string, returning (ptr, len).
pub extern "C" fn fj_rt_float_to_string(val: f64, out_ptr: *mut *mut u8, out_len: *mut i64) {
    let s = format!("{val}");
    let len = s.len();
    let layout = std::alloc::Layout::from_size_align(len.max(1), 8).expect("float_to_string alloc");
    // SAFETY: layout is valid
    let buf = unsafe { std::alloc::alloc(layout) };
    // SAFETY: buf is freshly allocated with sufficient size
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len);
        *out_ptr = buf;
        *out_len = len as i64;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// File I/O
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: writes `content` to `path`. Returns 0 on success, 1 on error.
pub extern "C" fn fj_rt_write_file(
    path_ptr: *const u8,
    path_len: i64,
    content_ptr: *const u8,
    content_len: i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };
    let content = unsafe { std::slice::from_raw_parts(content_ptr, content_len as usize) };
    match std::fs::write(path, content) {
        Ok(()) => 0, // Ok tag
        Err(_) => 1, // Err tag
    }
}

/// Runtime: reads file at `path`. On success (return 0), writes content to out params.
/// On error (return 1), out params are zeroed.
pub extern "C" fn fj_rt_read_file(
    path_ptr: *const u8,
    path_len: i64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };
    match std::fs::read(path) {
        Ok(data) => {
            let len = data.len();
            let layout =
                std::alloc::Layout::from_size_align(len.max(1), 8).expect("read_file alloc");
            // SAFETY: layout is valid
            let buf = unsafe { std::alloc::alloc(layout) };
            // SAFETY: buf is freshly allocated
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), buf, len);
                *out_ptr = buf;
                *out_len = len as i64;
            }
            0 // Ok tag
        }
        Err(_) => {
            // SAFETY: out params are valid pointers
            unsafe {
                *out_ptr = std::ptr::null_mut();
                *out_len = 0;
            }
            1 // Err tag
        }
    }
}

/// Runtime: appends `content` to file at `path`.
pub extern "C" fn fj_rt_append_file(
    path_ptr: *const u8,
    path_len: i64,
    content_ptr: *const u8,
    content_len: i64,
) -> i64 {
    // SAFETY: caller guarantees valid string slices
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };
    let content = unsafe { std::slice::from_raw_parts(content_ptr, content_len as usize) };
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
    {
        Ok(mut f) => match f.write_all(content) {
            Ok(()) => 0,
            Err(_) => 1,
        },
        Err(_) => 1,
    }
}

/// Runtime: checks if a file exists. Returns 1 for true, 0 for false.
pub extern "C" fn fj_rt_file_exists(path_ptr: *const u8, path_len: i64) -> i64 {
    // SAFETY: caller guarantees valid string slice
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };
    if std::path::Path::new(path).exists() {
        1
    } else {
        0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HashMap runtime functions
// ═══════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

/// Runtime: creates a new empty HashMap<String, i64> and returns an opaque pointer.
pub extern "C" fn fj_rt_map_new() -> *mut u8 {
    let map = Box::new(HashMap::<String, i64>::new());
    Box::into_raw(map) as *mut u8
}

/// Runtime: inserts an integer value into a HashMap.
pub extern "C" fn fj_rt_map_insert_int(
    map_ptr: *mut u8,
    key_ptr: *const u8,
    key_len: i64,
    value: i64,
) {
    // SAFETY: caller guarantees valid map pointer and string slice
    unsafe {
        let map = &mut *(map_ptr as *mut HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        map.insert(key.to_string(), value);
    }
}

/// Runtime: inserts a float value into a HashMap (stored as i64 bits).
pub extern "C" fn fj_rt_map_insert_float(
    map_ptr: *mut u8,
    key_ptr: *const u8,
    key_len: i64,
    value: f64,
) {
    // SAFETY: caller guarantees valid map pointer and string slice
    unsafe {
        let map = &mut *(map_ptr as *mut HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        map.insert(key.to_string(), value.to_bits() as i64);
    }
}

/// Runtime: inserts a string value into a HashMap.
/// The string is copied (caller retains ownership of the original).
/// Value stored as pointer to heap-allocated (ptr, len) pair.
pub extern "C" fn fj_rt_map_insert_str(
    map_ptr: *mut u8,
    key_ptr: *const u8,
    key_len: i64,
    val_ptr: *const u8,
    val_len: i64,
) {
    // SAFETY: caller guarantees valid pointers and lengths
    unsafe {
        let map = &mut *(map_ptr as *mut HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        // Store the string value as a heap-allocated copy, packed as a pointer
        let val =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(val_ptr, val_len as usize));
        let boxed = Box::new(val.to_string());
        let raw_ptr = Box::into_raw(boxed) as i64;
        map.insert(key.to_string(), raw_ptr);
    }
}

/// Runtime: gets an integer value from a HashMap. Returns 0 if key not found.
pub extern "C" fn fj_rt_map_get_int(map_ptr: *mut u8, key_ptr: *const u8, key_len: i64) -> i64 {
    // SAFETY: caller guarantees valid map pointer and string slice
    unsafe {
        let map = &*(map_ptr as *const HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        map.get(key).copied().unwrap_or(0)
    }
}

/// Runtime: gets a string value from a HashMap via out-params.
///
/// The i64 value stored in the map is interpreted as a `*const String` (from `fj_rt_map_insert_str`).
/// Writes the string's data pointer and length to `out_ptr`/`out_len`.
/// If the key is not found, writes null ptr and len 0.
pub extern "C" fn fj_rt_map_get_str(
    map_ptr: *mut u8,
    key_ptr: *const u8,
    key_len: i64,
    out_ptr: *mut *const u8,
    out_len: *mut i64,
) {
    // SAFETY: caller guarantees valid map pointer, string slice, and out-param pointers
    unsafe {
        let map = &*(map_ptr as *const HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        if let Some(&raw) = map.get(key) {
            let s = &*(raw as *const String);
            *out_ptr = s.as_ptr();
            *out_len = s.len() as i64;
        } else {
            *out_ptr = std::ptr::null();
            *out_len = 0;
        }
    }
}

/// Runtime: checks if a key exists in the HashMap. Returns 1 if yes, 0 if no.
pub extern "C" fn fj_rt_map_contains(map_ptr: *mut u8, key_ptr: *const u8, key_len: i64) -> i64 {
    // SAFETY: caller guarantees valid map pointer and string slice
    unsafe {
        let map = &*(map_ptr as *const HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        if map.contains_key(key) {
            1
        } else {
            0
        }
    }
}

/// Runtime: returns the number of entries in the HashMap.
pub extern "C" fn fj_rt_map_len(map_ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid map pointer
    unsafe {
        let map = &*(map_ptr as *const HashMap<String, i64>);
        map.len() as i64
    }
}

/// Runtime: removes an entry from the HashMap. Returns 1 if removed, 0 if not found.
pub extern "C" fn fj_rt_map_remove(map_ptr: *mut u8, key_ptr: *const u8, key_len: i64) -> i64 {
    // SAFETY: caller guarantees valid map pointer and string slice
    unsafe {
        let map = &mut *(map_ptr as *mut HashMap<String, i64>);
        let key =
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(key_ptr, key_len as usize));
        if map.remove(key).is_some() {
            1
        } else {
            0
        }
    }
}

/// Runtime: removes all entries from the HashMap.
pub extern "C" fn fj_rt_map_clear(map_ptr: *mut u8) {
    // SAFETY: caller guarantees valid map pointer
    unsafe {
        let map = &mut *(map_ptr as *mut HashMap<String, i64>);
        map.clear();
    }
}

/// Runtime: frees a HashMap.
pub extern "C" fn fj_rt_map_free(map_ptr: *mut u8) {
    // SAFETY: caller guarantees this pointer was produced by fj_rt_map_new
    unsafe {
        let _ = Box::from_raw(map_ptr as *mut HashMap<String, i64>);
    }
}

/// Runtime: returns a heap-allocated array of all keys in the HashMap.
///
/// Returns a `Box<Vec<i64>>` with interleaved `[ptr_as_i64, len_as_i64, ...]` pairs,
/// compatible with `fj_rt_split_len` / `fj_rt_split_get` for iteration.
/// `count_out` receives the number of keys.
pub extern "C" fn fj_rt_map_keys(map_ptr: *mut u8, count_out: *mut i64) -> *mut u8 {
    // SAFETY: caller guarantees valid HashMap pointer
    let map = unsafe { &*(map_ptr as *const HashMap<String, i64>) };
    let count = map.len();
    // SAFETY: count_out is a valid stack slot pointer from the caller
    unsafe {
        *count_out = count as i64;
    }
    // Build a Vec<i64> with interleaved (ptr, len) pairs — same format as fj_rt_str_split
    let mut flat: Vec<i64> = Vec::with_capacity(count * 2);
    for key in map.keys() {
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len();
        // Allocate a heap copy of the key string
        let key_layout =
            std::alloc::Layout::from_size_align(key_len.max(1), 1).expect("valid key layout");
        // SAFETY: layout is valid (max(1) ensures non-zero size)
        let key_buf = unsafe { std::alloc::alloc(key_layout) };
        // SAFETY: key_buf is valid for key_len bytes
        unsafe {
            std::ptr::copy_nonoverlapping(key_bytes.as_ptr(), key_buf, key_len);
        }
        flat.push(key_buf as i64);
        flat.push(key_len as i64);
    }
    Box::into_raw(Box::new(flat)) as *mut u8
}

/// Runtime: returns a heap-allocated Vec of all values in the HashMap.
///
/// Returns an opaque pointer to a `Box<Vec<i64>>`, compatible with `fj_rt_array_get/len/free`.
pub extern "C" fn fj_rt_map_values(map_ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid HashMap pointer
    let map = unsafe { &*(map_ptr as *const HashMap<String, i64>) };
    let vec: Vec<i64> = map.values().copied().collect();
    Box::into_raw(Box::new(vec)) as *mut u8
}

// ═══════════════════════════════════════════════════════════════════════
// Thread primitives
// ═══════════════════════════════════════════════════════════════════════

/// Opaque thread handle containing the JoinHandle and result slot.
struct ThreadHandle {
    join_handle: Option<std::thread::JoinHandle<i64>>,
    result: Option<i64>,
}

/// Runtime: spawns a new thread that calls `fn_ptr(arg)` and returns a handle.
///
/// `fn_ptr` is a function pointer with signature `fn(i64) -> i64`.
/// `arg` is the single argument passed to the function.
///
/// # Safety
///
/// The caller must ensure `fn_ptr` is a valid function pointer.
pub extern "C" fn fj_rt_thread_spawn(fn_ptr: *const u8, arg: i64) -> *mut u8 {
    let fp = fn_ptr as usize; // Copy pointer value for Send
    let handle = std::thread::spawn(move || {
        // SAFETY: caller guarantees fn_ptr is a valid extern "C" fn(i64) -> i64
        let f: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(fp) };
        f(arg)
    });
    let th = Box::new(ThreadHandle {
        join_handle: Some(handle),
        result: None,
    });
    Box::into_raw(th) as *mut u8
}

/// Runtime: spawns a thread with a no-arg function `fn() -> i64`.
///
/// # Safety
///
/// The caller must ensure `fn_ptr` is a valid function pointer.
pub extern "C" fn fj_rt_thread_spawn_noarg(fn_ptr: *const u8) -> *mut u8 {
    let fp = fn_ptr as usize;
    let handle = std::thread::spawn(move || {
        // SAFETY: caller guarantees fn_ptr is a valid extern "C" fn() -> i64
        let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(fp) };
        f()
    });
    let th = Box::new(ThreadHandle {
        join_handle: Some(handle),
        result: None,
    });
    Box::into_raw(th) as *mut u8
}

/// Runtime: joins a thread and returns its result value.
///
/// Blocks until the thread completes. Returns the i64 result or 0 on error.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_thread_spawn`.
pub extern "C" fn fj_rt_thread_join(handle: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid thread handle pointer
    let th = unsafe { &mut *(handle as *mut ThreadHandle) };
    if let Some(jh) = th.join_handle.take() {
        match jh.join() {
            Ok(val) => {
                th.result = Some(val);
                val
            }
            Err(_) => 0,
        }
    } else {
        th.result.unwrap_or(0)
    }
}

/// Runtime: checks if a thread has finished (non-blocking).
///
/// Returns 1 if finished, 0 if still running.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_thread_spawn`.
pub extern "C" fn fj_rt_thread_is_finished(handle: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid thread handle pointer
    let th = unsafe { &*(handle as *const ThreadHandle) };
    if th.join_handle.is_none() || th.result.is_some() {
        1
    } else {
        i64::from(th.join_handle.as_ref().is_none_or(|h| h.is_finished()))
    }
}

/// Runtime: frees a thread handle.
///
/// # Safety
///
/// The caller must ensure `handle` was produced by `fj_rt_thread_spawn`.
pub extern "C" fn fj_rt_thread_free(handle: *mut u8) {
    // SAFETY: caller guarantees valid thread handle pointer
    unsafe {
        let _ = Box::from_raw(handle as *mut ThreadHandle);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Mutex primitives
// ═══════════════════════════════════════════════════════════════════════

/// Opaque mutex handle wrapping `std::sync::Mutex<i64>`.
struct MutexHandle {
    inner: std::sync::Mutex<i64>,
}

/// Runtime: creates a new Mutex with an initial i64 value.
pub extern "C" fn fj_rt_mutex_new(initial: i64) -> *mut u8 {
    let mh = Box::new(MutexHandle {
        inner: std::sync::Mutex::new(initial),
    });
    Box::into_raw(mh) as *mut u8
}

/// Runtime: locks a Mutex and returns the current value.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_mutex_new`.
pub extern "C" fn fj_rt_mutex_lock(handle: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid mutex handle pointer
    let mh = unsafe { &*(handle as *const MutexHandle) };
    let guard = mh.inner.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

/// Runtime: stores a value and unlocks a Mutex.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_mutex_new`.
pub extern "C" fn fj_rt_mutex_store(handle: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid mutex handle pointer
    let mh = unsafe { &*(handle as *const MutexHandle) };
    let mut guard = mh.inner.lock().unwrap_or_else(|e| e.into_inner());
    *guard = value;
}

/// Runtime: tries to lock a Mutex (non-blocking).
///
/// Returns 1 if lock acquired (value in out_val), 0 if already locked.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_mutex_new`.
pub extern "C" fn fj_rt_mutex_try_lock(handle: *mut u8, out_val: *mut i64) -> i64 {
    // SAFETY: caller guarantees valid mutex handle pointer
    let mh = unsafe { &*(handle as *const MutexHandle) };
    match mh.inner.try_lock() {
        Ok(guard) => {
            // SAFETY: out_val is a valid pointer
            unsafe {
                *out_val = *guard;
            }
            1
        }
        Err(_) => 0,
    }
}

/// Runtime: frees a Mutex handle.
///
/// # Safety
///
/// The caller must ensure `handle` was produced by `fj_rt_mutex_new`.
pub extern "C" fn fj_rt_mutex_free(handle: *mut u8) {
    // SAFETY: caller guarantees valid mutex handle pointer
    unsafe {
        let _ = Box::from_raw(handle as *mut MutexHandle);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// MutexGuard (RAII lock wrapper)
// ═══════════════════════════════════════════════════════════════════════

/// Opaque guard handle that holds a mutex lock.
///
/// SAFETY: The guard MUST be dropped (via `fj_rt_mutex_guard_free`)
/// before the underlying mutex is freed. Fajar Lang's scope-based cleanup
/// guarantees this: guards are block-scoped, mutexes are function-scoped.
struct MutexGuardHandle {
    /// Boxed MutexGuard, type-erased to avoid lifetime in struct.
    /// Actually stores `Box<std::sync::MutexGuard<'a, i64>>` (transmuted).
    guard_raw: *mut (),
    /// Pointer to the mutex handle (for diagnostics only).
    _mutex_ptr: *const MutexHandle,
}

/// Runtime: locks a mutex and returns an opaque guard handle.
///
/// The guard holds the lock until `fj_rt_mutex_guard_free` is called.
///
/// # Safety
///
/// The caller must ensure `mutex_handle` is a valid pointer from `fj_rt_mutex_new`.
pub extern "C" fn fj_rt_mutex_guard_lock(mutex_handle: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid mutex handle pointer
    let mh = unsafe { &*(mutex_handle as *const MutexHandle) };
    let guard = mh.inner.lock().unwrap_or_else(|e| e.into_inner());
    let boxed = Box::new(guard);
    let guard_raw = Box::into_raw(boxed) as *mut ();
    let gh = Box::new(MutexGuardHandle {
        guard_raw,
        _mutex_ptr: mutex_handle as *const MutexHandle,
    });
    Box::into_raw(gh) as *mut u8
}

/// Runtime: reads the current value through a guard (lock held).
///
/// # Safety
///
/// The caller must ensure `guard` is a valid pointer from `fj_rt_mutex_guard_lock`.
pub extern "C" fn fj_rt_mutex_guard_get(guard: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid guard handle pointer
    let gh = unsafe { &*(guard as *const MutexGuardHandle) };
    let guard_ref = unsafe { &*(gh.guard_raw as *const std::sync::MutexGuard<'_, i64>) };
    **guard_ref
}

/// Runtime: writes a value through a guard (lock held).
///
/// # Safety
///
/// The caller must ensure `guard` is a valid pointer from `fj_rt_mutex_guard_lock`.
pub extern "C" fn fj_rt_mutex_guard_set(guard: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid guard handle pointer
    let gh = unsafe { &*(guard as *const MutexGuardHandle) };
    let guard_ref = unsafe { &mut *(gh.guard_raw as *mut std::sync::MutexGuard<'_, i64>) };
    **guard_ref = value;
}

/// Runtime: drops a guard, releasing the mutex lock.
///
/// # Safety
///
/// The caller must ensure `guard` is a valid pointer from `fj_rt_mutex_guard_lock`.
/// After this call, the guard handle is invalid.
pub extern "C" fn fj_rt_mutex_guard_free(guard: *mut u8) {
    // SAFETY: caller guarantees valid guard handle pointer
    unsafe {
        let gh = Box::from_raw(guard as *mut MutexGuardHandle);
        // Drop the boxed MutexGuard to release the lock
        let _ = Box::from_raw(gh.guard_raw as *mut std::sync::MutexGuard<'_, i64>);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Channel primitives (MPSC — multi-producer, single-consumer)
// ═══════════════════════════════════════════════════════════════════════

/// Opaque channel handle wrapping Rust's `std::sync::mpsc` channel.
struct ChannelHandle {
    sender: std::sync::Mutex<Option<std::sync::mpsc::Sender<i64>>>,
    receiver: std::sync::Mutex<Option<std::sync::mpsc::Receiver<i64>>>,
}

/// Runtime: creates a new unbounded MPSC channel.
///
/// Returns a pointer to a `ChannelHandle` that contains both sender and receiver.
pub extern "C" fn fj_rt_channel_new() -> *mut u8 {
    let (sender, receiver) = std::sync::mpsc::channel();
    let ch = Box::new(ChannelHandle {
        sender: std::sync::Mutex::new(Some(sender)),
        receiver: std::sync::Mutex::new(Some(receiver)),
    });
    Box::into_raw(ch) as *mut u8
}

/// Runtime: sends an i64 value into the channel.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_channel_new`.
pub extern "C" fn fj_rt_channel_send(handle: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid channel handle pointer
    let ch = unsafe { &*(handle as *const ChannelHandle) };
    let guard = ch.sender.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref tx) = *guard {
        let _ = tx.send(value);
    }
}

/// Runtime: receives an i64 value from the channel (blocking).
///
/// Returns the received value, or 0 if the channel is disconnected.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_channel_new`.
pub extern "C" fn fj_rt_channel_recv(handle: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid channel handle pointer
    let ch = unsafe { &*(handle as *const ChannelHandle) };
    let guard = ch.receiver.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref rx) = *guard {
        rx.recv().unwrap_or(0)
    } else {
        0
    }
}

/// Runtime: tries to receive from the channel (non-blocking).
///
/// Returns 1 and stores value in `out_val` if successful, 0 if empty.
///
/// # Safety
///
/// The caller must ensure `handle` and `out_val` are valid pointers.
pub extern "C" fn fj_rt_channel_try_recv(handle: *mut u8, out_val: *mut i64) -> i64 {
    // SAFETY: caller guarantees valid channel handle pointer
    let ch = unsafe { &*(handle as *const ChannelHandle) };
    let guard = ch.receiver.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref rx) = *guard {
        match rx.try_recv() {
            Ok(val) => {
                // SAFETY: out_val is valid
                unsafe {
                    *out_val = val;
                }
                1
            }
            Err(_) => 0,
        }
    } else {
        0
    }
}

/// Runtime: clones the sender side of a channel (for multi-producer).
///
/// Returns a new handle that shares the same underlying channel.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_channel_new`.
pub extern "C" fn fj_rt_channel_clone_sender(handle: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid channel handle pointer
    let ch = unsafe { &*(handle as *const ChannelHandle) };
    let guard = ch.sender.lock().unwrap_or_else(|e| e.into_inner());
    let cloned_sender = guard.as_ref().cloned();
    let new_ch = Box::new(ChannelHandle {
        sender: std::sync::Mutex::new(cloned_sender),
        receiver: std::sync::Mutex::new(None), // Clone gets sender only
    });
    Box::into_raw(new_ch) as *mut u8
}

/// Runtime: closes the sender side of a channel.
///
/// After close, `recv()` returns 0 (disconnected) once the buffer is drained.
/// Further `send()` calls are no-ops.
///
/// # Safety
///
/// The caller must ensure `handle` was produced by `fj_rt_channel_new`.
pub extern "C" fn fj_rt_channel_close(handle: *mut u8) {
    // SAFETY: caller guarantees valid channel handle pointer
    let ch = unsafe { &*(handle as *const ChannelHandle) };
    let mut guard = ch.sender.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None; // Drop the sender, disconnecting the channel
}

/// Runtime: frees a channel handle.
///
/// # Safety
///
/// The caller must ensure `handle` was produced by `fj_rt_channel_new`.
pub extern "C" fn fj_rt_channel_free(handle: *mut u8) {
    // SAFETY: caller guarantees valid channel handle pointer
    unsafe {
        let _ = Box::from_raw(handle as *mut ChannelHandle);
    }
}

/// Runtime: select from two channels. Returns the value from whichever is ready first.
///
/// Returns a packed result: `channel_index * 1_000_000_000 + value`.
/// Channel index is 1 or 2. If both are closed/empty after polling, returns 0.
///
/// # Safety
///
/// Both handles must be valid channel pointers from `fj_rt_channel_new`.
pub extern "C" fn fj_rt_channel_select2(ch1: *mut u8, ch2: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid channel handles
    let handle1 = unsafe { &*(ch1 as *const ChannelHandle) };
    let handle2 = unsafe { &*(ch2 as *const ChannelHandle) };

    // Spin-poll both channels, first one with data wins
    for _ in 0..10_000 {
        // Try ch1
        {
            let guard = handle1.receiver.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref rx) = *guard {
                if let Ok(val) = rx.try_recv() {
                    return 1_000_000_000 + val;
                }
            }
        }
        // Try ch2
        {
            let guard = handle2.receiver.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref rx) = *guard {
                if let Ok(val) = rx.try_recv() {
                    return 2_000_000_000 + val;
                }
            }
        }
        std::hint::spin_loop();
    }
    // Timeout: block on ch1 as fallback
    let guard = handle1.receiver.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref rx) = *guard {
        if let Ok(val) = rx.recv() {
            return 1_000_000_000 + val;
        }
    }
    0
}

// ═══════════════════════════════════════════════════════════════════════
// Bounded Channel primitives
// ═══════════════════════════════════════════════════════════════════════

/// Opaque bounded channel handle wrapping `std::sync::mpsc::SyncSender/Receiver`.
struct BoundedChannelHandle {
    sender: std::sync::mpsc::SyncSender<i64>,
    receiver: std::sync::Mutex<Option<std::sync::mpsc::Receiver<i64>>>,
}

/// Runtime: creates a new bounded MPSC channel with the given capacity.
///
/// Returns a pointer to a `BoundedChannelHandle`.
pub extern "C" fn fj_rt_channel_bounded(capacity: i64) -> *mut u8 {
    let (sender, receiver) = std::sync::mpsc::sync_channel(capacity.max(1) as usize);
    let ch = Box::new(BoundedChannelHandle {
        sender,
        receiver: std::sync::Mutex::new(Some(receiver)),
    });
    Box::into_raw(ch) as *mut u8
}

/// Runtime: sends an i64 value into a bounded channel (blocks if full).
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_channel_bounded`.
pub extern "C" fn fj_rt_channel_bounded_send(handle: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid bounded channel handle pointer
    let ch = unsafe { &*(handle as *const BoundedChannelHandle) };
    let _ = ch.sender.send(value);
}

/// Runtime: receives from a bounded channel (blocking).
///
/// Returns the received value, or 0 if disconnected.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_channel_bounded`.
pub extern "C" fn fj_rt_channel_bounded_recv(handle: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid bounded channel handle pointer
    let ch = unsafe { &*(handle as *const BoundedChannelHandle) };
    let guard = ch.receiver.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref rx) = *guard {
        rx.recv().unwrap_or(0)
    } else {
        0
    }
}

/// Runtime: tries to send on a bounded channel (non-blocking).
///
/// Returns 1 if sent successfully, 0 if channel is full.
///
/// # Safety
///
/// The caller must ensure `handle` is a valid pointer from `fj_rt_channel_bounded`.
pub extern "C" fn fj_rt_channel_try_send(handle: *mut u8, value: i64) -> i64 {
    // SAFETY: caller guarantees valid bounded channel handle pointer
    let ch = unsafe { &*(handle as *const BoundedChannelHandle) };
    match ch.sender.try_send(value) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Runtime: frees a bounded channel handle.
///
/// # Safety
///
/// The caller must ensure `handle` was produced by `fj_rt_channel_bounded`.
pub extern "C" fn fj_rt_channel_bounded_free(handle: *mut u8) {
    // SAFETY: caller guarantees valid bounded channel handle pointer
    unsafe {
        let _ = Box::from_raw(handle as *mut BoundedChannelHandle);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RwLock primitives
// ═══════════════════════════════════════════════════════════════════════

/// Opaque RwLock handle wrapping `std::sync::RwLock<i64>`.
struct RwLockHandle {
    lock: std::sync::RwLock<i64>,
}

/// Runtime: creates a new RwLock with an initial value.
pub extern "C" fn fj_rt_rwlock_new(initial: i64) -> *mut u8 {
    let handle = Box::new(RwLockHandle {
        lock: std::sync::RwLock::new(initial),
    });
    Box::into_raw(handle) as *mut u8
}

/// Runtime: acquires a read lock and returns the current value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_rwlock_new`.
pub extern "C" fn fj_rt_rwlock_read(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid RwLock pointer
    let handle = unsafe { &*(ptr as *const RwLockHandle) };
    let guard = handle.lock.read().unwrap_or_else(|e| e.into_inner());
    *guard
}

/// Runtime: acquires a write lock and stores a new value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_rwlock_new`.
pub extern "C" fn fj_rt_rwlock_write(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid RwLock pointer
    let handle = unsafe { &*(ptr as *const RwLockHandle) };
    let mut guard = handle.lock.write().unwrap_or_else(|e| e.into_inner());
    *guard = value;
}

/// Runtime: frees a RwLock handle.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_rwlock_new`.
pub extern "C" fn fj_rt_rwlock_free(ptr: *mut u8) {
    // SAFETY: caller guarantees valid RwLock pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut RwLockHandle);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sleep utility
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: sleeps for `millis` milliseconds.
pub extern "C" fn fj_rt_sleep(millis: i64) {
    std::thread::sleep(std::time::Duration::from_millis(millis as u64));
}

// ═══════════════════════════════════════════════════════════════════════
// Barrier primitives
// ═══════════════════════════════════════════════════════════════════════

/// Opaque Barrier handle wrapping `std::sync::Barrier`.
struct BarrierHandle {
    barrier: std::sync::Barrier,
}

/// Runtime: creates a new Barrier for N threads.
pub extern "C" fn fj_rt_barrier_new(n: i64) -> *mut u8 {
    let handle = Box::new(BarrierHandle {
        barrier: std::sync::Barrier::new(n as usize),
    });
    Box::into_raw(handle) as *mut u8
}

/// Runtime: blocks until all N threads have called wait.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_barrier_new`.
pub extern "C" fn fj_rt_barrier_wait(ptr: *mut u8) {
    // SAFETY: caller guarantees valid Barrier pointer
    let handle = unsafe { &*(ptr as *const BarrierHandle) };
    handle.barrier.wait();
}

/// Runtime: frees a Barrier handle.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_barrier_new`.
pub extern "C" fn fj_rt_barrier_free(ptr: *mut u8) {
    // SAFETY: caller guarantees valid Barrier pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut BarrierHandle);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Condvar primitives
// ═══════════════════════════════════════════════════════════════════════

/// Opaque Condvar handle wrapping `std::sync::Condvar`.
struct CondvarHandle {
    condvar: std::sync::Condvar,
}

/// Runtime: creates a new Condvar.
pub extern "C" fn fj_rt_condvar_new() -> *mut u8 {
    let handle = Box::new(CondvarHandle {
        condvar: std::sync::Condvar::new(),
    });
    Box::into_raw(handle) as *mut u8
}

/// Runtime: waits on a Condvar with a Mutex.
///
/// Atomically releases the mutex, waits for notification, then reacquires.
/// Returns the current value in the mutex after reacquisition.
///
/// # Safety
///
/// The caller must ensure both `condvar_ptr` and `mutex_ptr` are valid.
pub extern "C" fn fj_rt_condvar_wait(condvar_ptr: *mut u8, mutex_ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid condvar and mutex pointers
    let cv = unsafe { &*(condvar_ptr as *const CondvarHandle) };
    let mh = unsafe { &*(mutex_ptr as *const MutexHandle) };
    let guard = mh.inner.lock().unwrap_or_else(|e| e.into_inner());
    let guard = cv.condvar.wait(guard).unwrap_or_else(|e| e.into_inner());
    *guard
}

/// Runtime: wakes one thread waiting on the Condvar.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_condvar_new`.
pub extern "C" fn fj_rt_condvar_notify_one(ptr: *mut u8) {
    // SAFETY: caller guarantees valid condvar pointer
    let cv = unsafe { &*(ptr as *const CondvarHandle) };
    cv.condvar.notify_one();
}

/// Runtime: wakes all threads waiting on the Condvar.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_condvar_new`.
pub extern "C" fn fj_rt_condvar_notify_all(ptr: *mut u8) {
    // SAFETY: caller guarantees valid condvar pointer
    let cv = unsafe { &*(ptr as *const CondvarHandle) };
    cv.condvar.notify_all();
}

/// Runtime: frees a Condvar handle.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_condvar_new`.
pub extern "C" fn fj_rt_condvar_free(ptr: *mut u8) {
    // SAFETY: caller guarantees valid condvar pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut CondvarHandle);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Atomic primitives
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: creates a new atomic i64 (thread-safe shared counter).
pub extern "C" fn fj_rt_atomic_new(initial: i64) -> *mut u8 {
    let atomic = Box::new(std::sync::atomic::AtomicI64::new(initial));
    Box::into_raw(atomic) as *mut u8
}

/// Runtime: atomically loads the value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_load(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.load(std::sync::atomic::Ordering::SeqCst)
}

/// Runtime: atomically stores a value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_store(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.store(value, std::sync::atomic::Ordering::SeqCst);
}

/// Runtime: atomically loads with Relaxed ordering.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_load_relaxed(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.load(std::sync::atomic::Ordering::Relaxed)
}

/// Runtime: atomically loads with Acquire ordering.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_load_acquire(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.load(std::sync::atomic::Ordering::Acquire)
}

/// Runtime: atomically stores with Relaxed ordering.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_store_relaxed(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.store(value, std::sync::atomic::Ordering::Relaxed);
}

/// Runtime: atomically stores with Release ordering.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_store_release(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.store(value, std::sync::atomic::Ordering::Release);
}

/// Runtime: atomically adds to the value and returns the previous value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_add(ptr: *mut u8, value: i64) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.fetch_add(value, std::sync::atomic::Ordering::SeqCst)
}

/// Runtime: atomically subtracts from the value and returns the previous value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_sub(ptr: *mut u8, value: i64) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.fetch_sub(value, std::sync::atomic::Ordering::SeqCst)
}

/// Runtime: compare-and-swap. If current == expected, stores desired and returns expected.
/// Otherwise returns the current value (swap failed).
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_cas(ptr: *mut u8, expected: i64, desired: i64) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    match atomic.compare_exchange(
        expected,
        desired,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
    ) {
        Ok(v) => v,
        Err(v) => v,
    }
}

/// Runtime: atomically ANDs the value and returns the previous value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_and(ptr: *mut u8, value: i64) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.fetch_and(value, std::sync::atomic::Ordering::SeqCst)
}

/// Runtime: atomically ORs the value and returns the previous value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_or(ptr: *mut u8, value: i64) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.fetch_or(value, std::sync::atomic::Ordering::SeqCst)
}

/// Runtime: atomically XORs the value and returns the previous value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_xor(ptr: *mut u8, value: i64) -> i64 {
    // SAFETY: caller guarantees valid atomic pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI64) };
    atomic.fetch_xor(value, std::sync::atomic::Ordering::SeqCst)
}

/// Runtime: frees an atomic value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_atomic_new`.
pub extern "C" fn fj_rt_atomic_free(ptr: *mut u8) {
    // SAFETY: caller guarantees valid atomic pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut std::sync::atomic::AtomicI64);
    }
}

// ── Typed Atomics (S8.1) ──

/// Creates an `AtomicI32` on the heap.
///
/// # Safety
///
/// Returns a valid pointer that must be freed with `fj_rt_atomic_i32_free`.
pub extern "C" fn fj_rt_atomic_i32_new(initial: i64) -> *mut u8 {
    let atomic = Box::new(std::sync::atomic::AtomicI32::new(initial as i32));
    Box::into_raw(atomic) as *mut u8
}

/// Loads from an `AtomicI32` (SeqCst).
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_atomic_i32_new`.
pub extern "C" fn fj_rt_atomic_i32_load(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid AtomicI32 pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI32) };
    atomic.load(std::sync::atomic::Ordering::SeqCst) as i64
}

/// Stores to an `AtomicI32` (SeqCst).
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_atomic_i32_new`.
pub extern "C" fn fj_rt_atomic_i32_store(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid AtomicI32 pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicI32) };
    atomic.store(value as i32, std::sync::atomic::Ordering::SeqCst);
}

/// Frees an `AtomicI32`.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_atomic_i32_new`.
pub extern "C" fn fj_rt_atomic_i32_free(ptr: *mut u8) {
    // SAFETY: caller guarantees valid AtomicI32 pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut std::sync::atomic::AtomicI32);
    }
}

/// Creates an `AtomicBool` on the heap. 0 = false, nonzero = true.
///
/// # Safety
///
/// Returns a valid pointer that must be freed with `fj_rt_atomic_bool_free`.
pub extern "C" fn fj_rt_atomic_bool_new(initial: i64) -> *mut u8 {
    let atomic = Box::new(std::sync::atomic::AtomicBool::new(initial != 0));
    Box::into_raw(atomic) as *mut u8
}

/// Loads from an `AtomicBool` (SeqCst). Returns 0 (false) or 1 (true).
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_atomic_bool_new`.
pub extern "C" fn fj_rt_atomic_bool_load(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid AtomicBool pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicBool) };
    if atomic.load(std::sync::atomic::Ordering::SeqCst) {
        1
    } else {
        0
    }
}

/// Stores to an `AtomicBool` (SeqCst). 0 = false, nonzero = true.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_atomic_bool_new`.
pub extern "C" fn fj_rt_atomic_bool_store(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid AtomicBool pointer
    let atomic = unsafe { &*(ptr as *const std::sync::atomic::AtomicBool) };
    atomic.store(value != 0, std::sync::atomic::Ordering::SeqCst);
}

/// Frees an `AtomicBool`.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_atomic_bool_new`.
pub extern "C" fn fj_rt_atomic_bool_free(ptr: *mut u8) {
    // SAFETY: caller guarantees valid AtomicBool pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut std::sync::atomic::AtomicBool);
    }
}

// ── Thread-Local Storage ──

std::thread_local! {
    static TLS_SLOTS: std::cell::RefCell<std::collections::HashMap<i64, i64>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Runtime: set a thread-local value by key.
pub extern "C" fn fj_rt_tls_set(key: i64, value: i64) {
    TLS_SLOTS.with(|slots| {
        slots.borrow_mut().insert(key, value);
    });
}

/// Runtime: get a thread-local value by key. Returns 0 if not set.
pub extern "C" fn fj_rt_tls_get(key: i64) -> i64 {
    TLS_SLOTS.with(|slots| slots.borrow().get(&key).copied().unwrap_or(0))
}

// ── Volatile Intrinsics ──

/// Volatile read: reads an i64 value from the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid, aligned i64 memory location.
pub extern "C" fn fj_rt_volatile_read(addr: *const i64) -> i64 {
    // SAFETY: caller guarantees valid aligned pointer
    unsafe { std::ptr::read_volatile(addr) }
}

/// Volatile write: writes an i64 value to the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid, aligned i64 memory location.
pub extern "C" fn fj_rt_volatile_write(addr: *mut i64, value: i64) {
    // SAFETY: caller guarantees valid aligned pointer
    unsafe { std::ptr::write_volatile(addr, value) }
}

/// Volatile read u8: reads a single byte from the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid u8 memory location.
pub extern "C" fn fj_rt_volatile_read_u8(addr: *const u8) -> i64 {
    // SAFETY: caller guarantees valid pointer
    unsafe { std::ptr::read_volatile(addr) as i64 }
}

/// Volatile read u16: reads a 16-bit value from the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid, aligned u16 memory location.
pub extern "C" fn fj_rt_volatile_read_u16(addr: *const u16) -> i64 {
    // SAFETY: caller guarantees valid aligned pointer
    unsafe { std::ptr::read_volatile(addr) as i64 }
}

/// Volatile read u32: reads a 32-bit value from the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid, aligned u32 memory location.
pub extern "C" fn fj_rt_volatile_read_u32(addr: *const u32) -> i64 {
    // SAFETY: caller guarantees valid aligned pointer
    unsafe { std::ptr::read_volatile(addr) as i64 }
}

/// Volatile write u8: writes a single byte to the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid u8 memory location.
pub extern "C" fn fj_rt_volatile_write_u8(addr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid pointer
    unsafe { std::ptr::write_volatile(addr, value as u8) }
}

/// Volatile write u16: writes a 16-bit value to the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid, aligned u16 memory location.
pub extern "C" fn fj_rt_volatile_write_u16(addr: *mut u16, value: i64) {
    // SAFETY: caller guarantees valid aligned pointer
    unsafe { std::ptr::write_volatile(addr, value as u16) }
}

/// Volatile write u32: writes a 32-bit value to the given address.
///
/// # Safety
///
/// The caller must ensure `addr` points to a valid, aligned u32 memory location.
pub extern "C" fn fj_rt_volatile_write_u32(addr: *mut u32, value: i64) {
    // SAFETY: caller guarantees valid aligned pointer
    unsafe { std::ptr::write_volatile(addr, value as u32) }
}

/// Compiler fence: prevents compiler reordering across this point.
pub extern "C" fn fj_rt_compiler_fence() {
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Memory fence: full hardware memory barrier.
pub extern "C" fn fj_rt_memory_fence() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

// ── Memory Access Primitives ──

/// Reads an i64 from an allocated memory region at byte offset.
///
/// # Safety
///
/// The caller must ensure `ptr + offset` points to a valid, aligned i64.
pub extern "C" fn fj_rt_mem_read(ptr: *const u8, offset: i64) -> i64 {
    // SAFETY: caller guarantees valid pointer + offset
    unsafe {
        let target = ptr.add(offset as usize) as *const i64;
        std::ptr::read(target)
    }
}

/// Writes an i64 to an allocated memory region at byte offset.
///
/// # Safety
///
/// The caller must ensure `ptr + offset` points to a valid, aligned i64.
pub extern "C" fn fj_rt_mem_write(ptr: *mut u8, offset: i64, value: i64) {
    // SAFETY: caller guarantees valid pointer + offset
    unsafe {
        let target = ptr.add(offset as usize) as *mut i64;
        std::ptr::write(target, value);
    }
}

// ── Tensor Runtime (ndarray-backed) ──

use ndarray::Array2;

/// Opaque tensor handle: Box<Array2<f64>> behind *mut u8.
type TensorHandle = Array2<f64>;

/// Creates a zeros tensor of shape (rows, cols).
pub extern "C" fn fj_rt_tensor_zeros(rows: i64, cols: i64) -> *mut u8 {
    let t = Box::new(Array2::<f64>::zeros((rows as usize, cols as usize)));
    Box::into_raw(t) as *mut u8
}

/// Creates a ones tensor of shape (rows, cols).
pub extern "C" fn fj_rt_tensor_ones(rows: i64, cols: i64) -> *mut u8 {
    let t = Box::new(Array2::<f64>::ones((rows as usize, cols as usize)));
    Box::into_raw(t) as *mut u8
}

/// Returns the number of rows in the tensor.
pub extern "C" fn fj_rt_tensor_rows(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    t.nrows() as i64
}

/// Returns the number of columns in the tensor.
pub extern "C" fn fj_rt_tensor_cols(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    t.ncols() as i64
}

/// Gets element at (row, col). Returns f64 bits as i64.
pub extern "C" fn fj_rt_tensor_get(ptr: *mut u8, row: i64, col: i64) -> i64 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let val = t[[row as usize, col as usize]];
    val.to_bits() as i64
}

/// Sets element at (row, col). Value is f64 bits as i64.
pub extern "C" fn fj_rt_tensor_set(ptr: *mut u8, row: i64, col: i64, val_bits: i64) {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &mut *(ptr as *mut TensorHandle) };
    t[[row as usize, col as usize]] = f64::from_bits(val_bits as u64);
}

/// Element-wise addition: a + b. Returns new tensor pointer.
pub extern "C" fn fj_rt_tensor_add(a: *mut u8, b: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointers
    let (ta, tb) = unsafe { (&*(a as *const TensorHandle), &*(b as *const TensorHandle)) };
    let result = Box::new(ta + tb);
    Box::into_raw(result) as *mut u8
}

/// Element-wise subtraction: a - b. Returns new tensor pointer.
pub extern "C" fn fj_rt_tensor_sub(a: *mut u8, b: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointers
    let (ta, tb) = unsafe { (&*(a as *const TensorHandle), &*(b as *const TensorHandle)) };
    let result = Box::new(ta - tb);
    Box::into_raw(result) as *mut u8
}

/// Element-wise multiplication: a * b. Returns new tensor pointer.
pub extern "C" fn fj_rt_tensor_mul(a: *mut u8, b: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointers
    let (ta, tb) = unsafe { (&*(a as *const TensorHandle), &*(b as *const TensorHandle)) };
    let result = Box::new(ta * tb);
    Box::into_raw(result) as *mut u8
}

/// Matrix multiplication: a @ b. Returns new tensor pointer.
pub extern "C" fn fj_rt_tensor_matmul(a: *mut u8, b: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointers
    let (ta, tb) = unsafe { (&*(a as *const TensorHandle), &*(b as *const TensorHandle)) };
    let result = Box::new(ta.dot(tb));
    Box::into_raw(result) as *mut u8
}

/// Reshape: returns new tensor with shape (rows, cols).
///
/// # Safety
///
/// `ptr` must be a valid TensorHandle pointer. `rows * cols` must equal the total element count.
pub extern "C" fn fj_rt_tensor_reshape(ptr: *mut u8, rows: i64, cols: i64) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let total = t.len();
    let (r, c) = (rows as usize, cols as usize);
    if r * c != total {
        eprintln!(
            "[reshape error] cannot reshape ({},{}) tensor to ({},{})",
            t.nrows(),
            t.ncols(),
            r,
            c
        );
        return std::ptr::null_mut();
    }
    let flat = t.as_slice().unwrap_or(&[]);
    let result = Box::new(
        Array2::from_shape_vec((r, c), flat.to_vec()).unwrap_or_else(|_| Array2::zeros((r, c))),
    );
    Box::into_raw(result) as *mut u8
}

/// Flatten: returns new tensor with shape (1, total_elements).
///
/// # Safety
///
/// `ptr` must be a valid TensorHandle pointer.
pub extern "C" fn fj_rt_tensor_flatten(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let total = t.len();
    let flat = t.as_slice().unwrap_or(&[]);
    let result = Box::new(
        Array2::from_shape_vec((1, total), flat.to_vec())
            .unwrap_or_else(|_| Array2::zeros((1, total))),
    );
    Box::into_raw(result) as *mut u8
}

/// Transpose: returns new transposed tensor.
pub extern "C" fn fj_rt_tensor_transpose(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = Box::new(t.t().to_owned());
    Box::into_raw(result) as *mut u8
}

/// ReLU activation: max(0, x) element-wise. Returns new tensor.
pub extern "C" fn fj_rt_tensor_relu(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = Box::new(t.mapv(|x| x.max(0.0)));
    Box::into_raw(result) as *mut u8
}

/// Softmax: exp(x_i) / sum(exp(x_j)) per row.
pub extern "C" fn fj_rt_tensor_softmax(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let rows = t.nrows();
    let cols = t.ncols();
    let mut result = ndarray::Array2::<f64>::zeros((rows, cols));
    for i in 0..rows {
        let row = t.row(i);
        let max_val = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = row.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f64 = exps.iter().sum();
        for j in 0..cols {
            result[[i, j]] = exps[j] / sum;
        }
    }
    Box::into_raw(Box::new(result)) as *mut u8
}

/// Sigmoid: 1 / (1 + exp(-x)) element-wise.
pub extern "C" fn fj_rt_tensor_sigmoid(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = Box::new(t.mapv(|x| 1.0 / (1.0 + (-x).exp())));
    Box::into_raw(result) as *mut u8
}

/// Sum all elements. Returns f64 bits as i64.
pub extern "C" fn fj_rt_tensor_sum(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    t.sum().to_bits() as i64
}

/// Frees a tensor.
pub extern "C" fn fj_rt_tensor_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid tensor pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut TensorHandle);
    }
}

// ── Autograd Runtime ──

/// Gradient-tracked tensor: data + optional gradient.
struct GradTensor {
    data: Array2<f64>,
    grad: Option<Array2<f64>>,
    requires_grad: bool,
}

/// Creates a gradient-tracked tensor from an existing tensor pointer.
/// Clones the data and sets requires_grad = true.
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid tensor pointer.
pub extern "C" fn fj_rt_tensor_requires_grad(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid tensor pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let gt = Box::new(GradTensor {
        data: t.clone(),
        grad: None,
        requires_grad: true,
    });
    Box::into_raw(gt) as *mut u8
}

/// Computes MSE loss: mean((pred - target)^2).
/// Returns loss value as f64 bits in i64, and stores the loss gradient
/// internally on the pred GradTensor (2 * (pred - target) / n).
///
/// # Safety
///
/// Both `pred_ptr` and `target_ptr` must be valid GradTensor / TensorHandle pointers.
pub extern "C" fn fj_rt_mse_loss(pred_ptr: *mut u8, target_ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid pointers
    let pred = unsafe { &mut *(pred_ptr as *mut GradTensor) };
    let target = unsafe { &*(target_ptr as *const TensorHandle) };
    let diff = &pred.data - target;
    let sq = &diff * &diff;
    let n = sq.len() as f64;
    let loss = sq.sum() / n;
    // Store gradient: d(MSE)/d(pred) = 2*(pred - target) / n
    if pred.requires_grad {
        pred.grad = Some(diff.mapv(|x| 2.0 * x / n));
    }
    loss.to_bits() as i64
}

/// Cross-entropy loss: -sum(target * log(pred)) / n.
///
/// # Safety
///
/// Both pointers must be valid GradTensor / TensorHandle pointers.
pub extern "C" fn fj_rt_cross_entropy_loss(pred_ptr: *mut u8, target_ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid pointers
    let pred = unsafe { &mut *(pred_ptr as *mut GradTensor) };
    let target = unsafe { &*(target_ptr as *const TensorHandle) };
    let eps = 1e-7;
    let log_pred = pred.data.mapv(|x| (x.max(eps)).ln());
    let n = target.len() as f64;
    let loss = -(target * &log_pred).sum() / n;
    // Gradient: -target / pred / n
    if pred.requires_grad {
        pred.grad = Some(target.mapv(|t| -t / n) / &pred.data.mapv(|p| p.max(eps)));
    }
    loss.to_bits() as i64
}

/// Returns the gradient tensor pointer (or null if no gradient).
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid GradTensor pointer.
pub extern "C" fn fj_rt_tensor_grad(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid GradTensor pointer
    let gt = unsafe { &*(ptr as *const GradTensor) };
    match &gt.grad {
        Some(g) => {
            let grad_copy = Box::new(g.clone());
            Box::into_raw(grad_copy) as *mut u8
        }
        None => std::ptr::null_mut(),
    }
}

/// Zeros out the gradient of a GradTensor.
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid GradTensor pointer.
pub extern "C" fn fj_rt_tensor_zero_grad(ptr: *mut u8) {
    // SAFETY: caller guarantees valid GradTensor pointer
    let gt = unsafe { &mut *(ptr as *mut GradTensor) };
    if let Some(ref mut g) = gt.grad {
        g.fill(0.0);
    }
}

/// Gets the data tensor pointer from a GradTensor (for reading values).
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid GradTensor pointer.
pub extern "C" fn fj_rt_grad_tensor_data(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid GradTensor pointer
    let gt = unsafe { &*(ptr as *const GradTensor) };
    let data_copy = Box::new(gt.data.clone());
    Box::into_raw(data_copy) as *mut u8
}

/// Frees a GradTensor.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_tensor_requires_grad`.
pub extern "C" fn fj_rt_grad_tensor_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid GradTensor pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut GradTensor);
    }
}

// =====================================================================
// S32.3 — Gradient through matmul, relu, sigmoid, softmax
// =====================================================================

/// Applies ReLU to a GradTensor and computes gradient: grad * (data > 0).
/// Returns a new GradTensor with the ReLU-ed data and chained gradient.
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid GradTensor pointer.
pub extern "C" fn fj_rt_grad_relu(ptr: *mut u8) -> *mut u8 {
    let gt = unsafe { &mut *(ptr as *mut GradTensor) };
    let output = gt.data.mapv(|x| if x > 0.0 { x } else { 0.0 });
    // Backward: d_relu/d_input = 1 if input > 0, else 0
    if gt.requires_grad {
        let local_grad = gt.data.mapv(|x| if x > 0.0 { 1.0 } else { 0.0 });
        let upstream = gt
            .grad
            .clone()
            .unwrap_or_else(|| Array2::ones(gt.data.raw_dim()));
        gt.grad = Some(&upstream * &local_grad);
    }
    let result = Box::new(GradTensor {
        data: output,
        grad: gt.grad.clone(),
        requires_grad: gt.requires_grad,
    });
    Box::into_raw(result) as *mut u8
}

/// Applies sigmoid to a GradTensor: σ(x) = 1/(1+exp(-x)).
/// Gradient: σ(x) * (1 - σ(x)).
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid GradTensor pointer.
pub extern "C" fn fj_rt_grad_sigmoid(ptr: *mut u8) -> *mut u8 {
    let gt = unsafe { &mut *(ptr as *mut GradTensor) };
    let sig = gt.data.mapv(|x| 1.0 / (1.0 + (-x).exp()));
    if gt.requires_grad {
        let local_grad = sig.mapv(|s| s * (1.0 - s));
        let upstream = gt
            .grad
            .clone()
            .unwrap_or_else(|| Array2::ones(gt.data.raw_dim()));
        gt.grad = Some(&upstream * &local_grad);
    }
    let result = Box::new(GradTensor {
        data: sig,
        grad: gt.grad.clone(),
        requires_grad: gt.requires_grad,
    });
    Box::into_raw(result) as *mut u8
}

/// Applies softmax to a GradTensor (per-row).
/// Gradient: diag(s) - s*s^T (simplified Jacobian-vector product).
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid GradTensor pointer.
pub extern "C" fn fj_rt_grad_softmax(ptr: *mut u8) -> *mut u8 {
    let gt = unsafe { &mut *(ptr as *mut GradTensor) };
    let (rows, cols) = gt.data.dim();
    let mut sm = Array2::<f64>::zeros((rows, cols));
    for i in 0..rows {
        let row = gt.data.row(i);
        let max_val = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_row: Vec<f64> = row.iter().map(|x| (x - max_val).exp()).collect();
        let sum: f64 = exp_row.iter().sum();
        for j in 0..cols {
            sm[[i, j]] = exp_row[j] / sum;
        }
    }
    if gt.requires_grad {
        let upstream = gt
            .grad
            .clone()
            .unwrap_or_else(|| Array2::ones(gt.data.raw_dim()));
        // Simplified softmax backward: s * (upstream - sum(upstream * s, axis=1))
        let mut grad = Array2::<f64>::zeros((rows, cols));
        for i in 0..rows {
            let dot: f64 = (0..cols).map(|j| upstream[[i, j]] * sm[[i, j]]).sum();
            for j in 0..cols {
                grad[[i, j]] = sm[[i, j]] * (upstream[[i, j]] - dot);
            }
        }
        gt.grad = Some(grad);
    }
    let result = Box::new(GradTensor {
        data: sm,
        grad: gt.grad.clone(),
        requires_grad: gt.requires_grad,
    });
    Box::into_raw(result) as *mut u8
}

/// Matmul with gradient tracking: C = A @ B.
/// Gradient: dL/dA = dL/dC @ B^T, dL/dB = A^T @ dL/dC.
/// Stores gradient on the first operand (A).
///
/// # Safety
///
/// Both pointers must be valid: `a_ptr` is a GradTensor, `b_ptr` is a TensorHandle.
pub extern "C" fn fj_rt_grad_matmul(a_ptr: *mut u8, b_ptr: *mut u8) -> *mut u8 {
    let a = unsafe { &mut *(a_ptr as *mut GradTensor) };
    let b = unsafe { &*(b_ptr as *const TensorHandle) };
    let output = a.data.dot(b);
    if a.requires_grad {
        // dL/dA = dL/dC @ B^T (upstream gradient is identity if at the end)
        let upstream = a
            .grad
            .clone()
            .unwrap_or_else(|| Array2::ones(output.raw_dim()));
        let b_t = b.t();
        a.grad = Some(upstream.dot(&b_t));
    }
    let result = Box::new(GradTensor {
        data: output,
        grad: a.grad.clone(),
        requires_grad: a.requires_grad,
    });
    Box::into_raw(result) as *mut u8
}

// =====================================================================
// S33 — Optimizer runtime functions
// =====================================================================

/// SGD optimizer state.
struct SgdOptimizer {
    lr: f64,
    momentum: f64,
    velocity: Option<Array2<f64>>,
}

/// Adam optimizer state.
struct AdamOptimizer {
    lr: f64,
    beta1: f64,
    beta2: f64,
    epsilon: f64,
    m: Option<Array2<f64>>,
    v: Option<Array2<f64>>,
    t: u64,
}

/// Creates a new SGD optimizer with given learning rate (as f64 bits).
///
/// # Safety
///
/// Returns a heap-allocated pointer. Must be freed with `fj_rt_optimizer_free`.
pub extern "C" fn fj_rt_sgd_new(lr_bits: i64) -> *mut u8 {
    let lr = f64::from_bits(lr_bits as u64);
    let opt = Box::new(SgdOptimizer {
        lr,
        momentum: 0.0,
        velocity: None,
    });
    Box::into_raw(opt) as *mut u8
}

/// Creates a new Adam optimizer with given learning rate (as f64 bits).
///
/// # Safety
///
/// Returns a heap-allocated pointer. Must be freed with `fj_rt_optimizer_free`.
pub extern "C" fn fj_rt_adam_new(lr_bits: i64) -> *mut u8 {
    let lr = f64::from_bits(lr_bits as u64);
    let opt = Box::new(AdamOptimizer {
        lr,
        beta1: 0.9,
        beta2: 0.999,
        epsilon: 1e-8,
        m: None,
        v: None,
        t: 0,
    });
    Box::into_raw(opt) as *mut u8
}

/// SGD step: updates GradTensor parameters using gradient.
/// param_ptr must be a GradTensor with computed gradient.
/// opt_ptr must be a SgdOptimizer.
///
/// # Safety
///
/// Both pointers must be valid and of the correct types.
pub extern "C" fn fj_rt_sgd_step(opt_ptr: *mut u8, param_ptr: *mut u8) {
    if opt_ptr.is_null() || param_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid pointers
    let opt = unsafe { &mut *(opt_ptr as *mut SgdOptimizer) };
    let param = unsafe { &mut *(param_ptr as *mut GradTensor) };

    if let Some(ref grad) = param.grad {
        if opt.momentum > 0.0 {
            let v = match &opt.velocity {
                Some(v) => v * opt.momentum + grad * (1.0 - opt.momentum),
                None => grad.clone(),
            };
            param.data = &param.data - &(&v * opt.lr);
            opt.velocity = Some(v);
        } else {
            param.data = &param.data - &(grad * opt.lr);
        }
    }
}

/// Adam step: updates GradTensor parameters using gradient.
///
/// # Safety
///
/// Both pointers must be valid and of the correct types.
pub extern "C" fn fj_rt_adam_step(opt_ptr: *mut u8, param_ptr: *mut u8) {
    if opt_ptr.is_null() || param_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid pointers
    let opt = unsafe { &mut *(opt_ptr as *mut AdamOptimizer) };
    let param = unsafe { &mut *(param_ptr as *mut GradTensor) };

    if let Some(ref grad) = param.grad {
        opt.t += 1;
        let m = match &opt.m {
            Some(m) => m * opt.beta1 + grad * (1.0 - opt.beta1),
            None => grad * (1.0 - opt.beta1),
        };
        let v = match &opt.v {
            Some(v) => v * opt.beta2 + &(grad * grad) * (1.0 - opt.beta2),
            None => &(grad * grad) * (1.0 - opt.beta2),
        };

        let bc1 = 1.0 - opt.beta1.powi(opt.t as i32);
        let bc2 = 1.0 - opt.beta2.powi(opt.t as i32);
        let m_hat = &m / bc1;
        let v_hat = &v / bc2;

        param.data = &param.data - &(&m_hat / &(v_hat.mapv(f64::sqrt) + opt.epsilon) * opt.lr);
        opt.m = Some(m);
        opt.v = Some(v);
    }
}

/// Frees an SGD or Adam optimizer.
/// Works for both types since we use Box::from_raw.
/// tag: 0 = SGD, 1 = Adam
///
/// # Safety
///
/// The caller must pass a valid optimizer pointer and correct tag.
pub extern "C" fn fj_rt_optimizer_free(ptr: *mut u8, tag: i64) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid pointer and matching tag
    unsafe {
        if tag == 0 {
            let _ = Box::from_raw(ptr as *mut SgdOptimizer);
        } else {
            let _ = Box::from_raw(ptr as *mut AdamOptimizer);
        }
    }
}

// =====================================================================
// S36 — Data Pipeline runtime functions
// =====================================================================

/// DataLoader holds a dataset as (data_tensor, label_tensor) + batch config.
struct DataLoaderHandle {
    data: Array2<f64>,
    labels: Array2<f64>,
    batch_size: usize,
    current_idx: usize,
    shuffle_indices: Vec<usize>,
}

/// Creates a DataLoader from data and label tensors with given batch size.
///
/// # Safety
///
/// data_ptr and label_ptr must be valid TensorHandle pointers.
pub extern "C" fn fj_rt_dataloader_new(
    data_ptr: *mut u8,
    label_ptr: *mut u8,
    batch_size: i64,
) -> *mut u8 {
    if data_ptr.is_null() || label_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle pointers
    let data = unsafe { &*(data_ptr as *const TensorHandle) };
    let labels = unsafe { &*(label_ptr as *const TensorHandle) };
    let n = data.nrows();
    let indices: Vec<usize> = (0..n).collect();
    let dl = Box::new(DataLoaderHandle {
        data: data.clone(),
        labels: labels.clone(),
        batch_size: batch_size.max(1) as usize,
        current_idx: 0,
        shuffle_indices: indices,
    });
    Box::into_raw(dl) as *mut u8
}

/// Returns the number of batches in the DataLoader.
pub extern "C" fn fj_rt_dataloader_len(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid DataLoaderHandle pointer
    let dl = unsafe { &*(ptr as *const DataLoaderHandle) };
    let n = dl.data.nrows();
    n.div_ceil(dl.batch_size) as i64
}

/// Resets the DataLoader to the beginning and optionally shuffles.
/// shuffle: 1 = shuffle, 0 = no shuffle.
pub extern "C" fn fj_rt_dataloader_reset(ptr: *mut u8, shuffle: i64) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid DataLoaderHandle pointer
    let dl = unsafe { &mut *(ptr as *mut DataLoaderHandle) };
    dl.current_idx = 0;
    if shuffle != 0 {
        use ndarray_rand::rand::seq::SliceRandom;
        use ndarray_rand::rand::thread_rng;
        dl.shuffle_indices.shuffle(&mut thread_rng());
    }
}

/// Returns the next batch of data as a tensor pointer (or null if exhausted).
pub extern "C" fn fj_rt_dataloader_next_data(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid DataLoaderHandle pointer
    let dl = unsafe { &*(ptr as *const DataLoaderHandle) };
    let n = dl.data.nrows();
    if dl.current_idx >= n {
        return std::ptr::null_mut();
    }
    let end = (dl.current_idx + dl.batch_size).min(n);
    let batch_rows: Vec<usize> = dl.shuffle_indices[dl.current_idx..end].to_vec();
    let cols = dl.data.ncols();
    let mut batch = Array2::<f64>::zeros((batch_rows.len(), cols));
    for (i, &row_idx) in batch_rows.iter().enumerate() {
        batch.row_mut(i).assign(&dl.data.row(row_idx));
    }
    let handle: Box<TensorHandle> = Box::new(batch);
    Box::into_raw(handle) as *mut u8
}

/// Returns the next batch of labels as a tensor pointer (or null if exhausted).
/// Must be called after next_data — they share the same current_idx.
pub extern "C" fn fj_rt_dataloader_next_labels(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid DataLoaderHandle pointer
    let dl = unsafe { &mut *(ptr as *mut DataLoaderHandle) };
    let n = dl.labels.nrows();
    if dl.current_idx >= n {
        return std::ptr::null_mut();
    }
    let end = (dl.current_idx + dl.batch_size).min(n);
    let batch_rows: Vec<usize> = dl.shuffle_indices[dl.current_idx..end].to_vec();
    let cols = dl.labels.ncols();
    let mut batch = Array2::<f64>::zeros((batch_rows.len(), cols));
    for (i, &row_idx) in batch_rows.iter().enumerate() {
        batch.row_mut(i).assign(&dl.labels.row(row_idx));
    }
    dl.current_idx = end; // advance index after getting both data+labels
    let handle: Box<TensorHandle> = Box::new(batch);
    Box::into_raw(handle) as *mut u8
}

/// Returns the total number of samples in the dataset.
pub extern "C" fn fj_rt_dataloader_num_samples(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid DataLoaderHandle pointer
    let dl = unsafe { &*(ptr as *const DataLoaderHandle) };
    dl.data.nrows() as i64
}

/// Frees a DataLoader.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_dataloader_new`.
pub extern "C" fn fj_rt_dataloader_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid DataLoaderHandle pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut DataLoaderHandle);
    }
}

/// Normalizes a tensor: (x - mean) / std per column.
///
/// # Safety
///
/// The caller must ensure `ptr` is a valid TensorHandle pointer.
pub extern "C" fn fj_rt_tensor_normalize(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let mut result = t.clone();
    for col_idx in 0..result.ncols() {
        let col = result.column(col_idx);
        let mean = col.mean().unwrap_or(0.0);
        let std_dev = col
            .mapv(|x| (x - mean).powi(2))
            .mean()
            .unwrap_or(1.0)
            .sqrt();
        let std_dev = if std_dev < 1e-8 { 1.0 } else { std_dev };
        for row_idx in 0..result.nrows() {
            result[[row_idx, col_idx]] = (result[[row_idx, col_idx]] - mean) / std_dev;
        }
    }
    let handle: Box<TensorHandle> = Box::new(result);
    Box::into_raw(handle) as *mut u8
}

// =====================================================================
// S37 — Model Serialization runtime functions
// =====================================================================

/// Saves a tensor to a binary file.
/// Format: [rows:u64][cols:u64][data:f64*rows*cols]
/// Returns 1 on success, 0 on failure.
///
/// # Safety
///
/// tensor_ptr must be a valid TensorHandle. path_ptr/path_len must point to valid UTF-8.
pub extern "C" fn fj_rt_tensor_save(
    tensor_ptr: *mut u8,
    path_ptr: *const u8,
    path_len: i64,
) -> i64 {
    if tensor_ptr.is_null() || path_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid pointers
    let t = unsafe { &*(tensor_ptr as *const TensorHandle) };
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) };
    let path = match std::str::from_utf8(path_bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let rows = t.nrows() as u64;
    let cols = t.ncols() as u64;
    let mut file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    use std::io::Write;
    if file.write_all(&rows.to_le_bytes()).is_err() {
        return 0;
    }
    if file.write_all(&cols.to_le_bytes()).is_err() {
        return 0;
    }
    for &val in t.iter() {
        if file.write_all(&val.to_le_bytes()).is_err() {
            return 0;
        }
    }
    1
}

/// Loads a tensor from a binary file.
/// Returns TensorHandle pointer or null on failure.
///
/// # Safety
///
/// path_ptr/path_len must point to valid UTF-8.
pub extern "C" fn fj_rt_tensor_load(path_ptr: *const u8, path_len: i64) -> *mut u8 {
    if path_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid pointer
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) };
    let path = match std::str::from_utf8(path_bytes) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return std::ptr::null_mut(),
    };
    if data.len() < 16 {
        return std::ptr::null_mut();
    }
    let rows = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0; 8])) as usize;
    let cols = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0; 8])) as usize;
    let expected = 16 + rows * cols * 8;
    if data.len() < expected {
        return std::ptr::null_mut();
    }
    let mut values = Vec::with_capacity(rows * cols);
    for i in 0..rows * cols {
        let offset = 16 + i * 8;
        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap_or([0; 8]));
        values.push(val);
    }
    let arr = match Array2::from_shape_vec((rows, cols), values) {
        Ok(a) => a,
        Err(_) => return std::ptr::null_mut(),
    };
    let handle: Box<TensorHandle> = Box::new(arr);
    Box::into_raw(handle) as *mut u8
}

/// Saves a checkpoint: tensor + epoch + loss (as f64 bits).
/// Format: [magic:u32][epoch:u64][loss_bits:u64][rows:u64][cols:u64][data:f64*rows*cols]
/// Returns 1 on success, 0 on failure.
///
/// # Safety
///
/// tensor_ptr must be a valid TensorHandle. path_ptr/path_len must point to valid UTF-8.
pub extern "C" fn fj_rt_checkpoint_save(
    tensor_ptr: *mut u8,
    path_ptr: *const u8,
    path_len: i64,
    epoch: i64,
    loss_bits: i64,
) -> i64 {
    if tensor_ptr.is_null() || path_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid pointers
    let t = unsafe { &*(tensor_ptr as *const TensorHandle) };
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) };
    let path = match std::str::from_utf8(path_bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    use std::io::Write;
    let magic: u32 = 0x464A_434B; // "FJCK"
    if file.write_all(&magic.to_le_bytes()).is_err() {
        return 0;
    }
    if file.write_all(&(epoch as u64).to_le_bytes()).is_err() {
        return 0;
    }
    if file.write_all(&(loss_bits as u64).to_le_bytes()).is_err() {
        return 0;
    }
    let rows = t.nrows() as u64;
    let cols = t.ncols() as u64;
    if file.write_all(&rows.to_le_bytes()).is_err() {
        return 0;
    }
    if file.write_all(&cols.to_le_bytes()).is_err() {
        return 0;
    }
    for &val in t.iter() {
        if file.write_all(&val.to_le_bytes()).is_err() {
            return 0;
        }
    }
    1
}

/// Helper to read and validate a checkpoint file.
fn read_checkpoint(path_ptr: *const u8, path_len: i64) -> Option<(u64, u64, Array2<f64>)> {
    if path_ptr.is_null() {
        return None;
    }
    // SAFETY: caller guarantees valid pointer
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) };
    let path = std::str::from_utf8(path_bytes).ok()?;
    let data = std::fs::read(path).ok()?;
    if data.len() < 36 {
        return None;
    }
    let magic = u32::from_le_bytes(data[0..4].try_into().ok()?);
    if magic != 0x464A_434B {
        return None;
    }
    let epoch = u64::from_le_bytes(data[4..12].try_into().ok()?);
    let loss_bits = u64::from_le_bytes(data[12..20].try_into().ok()?);
    let rows = u64::from_le_bytes(data[20..28].try_into().ok()?) as usize;
    let cols = u64::from_le_bytes(data[28..36].try_into().ok()?) as usize;
    let expected = 36 + rows * cols * 8;
    if data.len() < expected {
        return None;
    }
    let mut values = Vec::with_capacity(rows * cols);
    for i in 0..rows * cols {
        let offset = 36 + i * 8;
        let val = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        values.push(val);
    }
    let arr = Array2::from_shape_vec((rows, cols), values).ok()?;
    Some((epoch, loss_bits, arr))
}

/// Loads a checkpoint tensor from file.
/// Returns TensorHandle pointer or null on failure.
///
/// # Safety
///
/// path_ptr/path_len must point to valid UTF-8.
pub extern "C" fn fj_rt_checkpoint_load(path_ptr: *const u8, path_len: i64) -> *mut u8 {
    match read_checkpoint(path_ptr, path_len) {
        Some((_, _, arr)) => {
            let handle: Box<TensorHandle> = Box::new(arr);
            Box::into_raw(handle) as *mut u8
        }
        None => std::ptr::null_mut(),
    }
}

/// Reads the epoch from a checkpoint file. Returns -1 on failure.
///
/// # Safety
///
/// path_ptr/path_len must point to valid UTF-8.
pub extern "C" fn fj_rt_checkpoint_epoch(path_ptr: *const u8, path_len: i64) -> i64 {
    match read_checkpoint(path_ptr, path_len) {
        Some((epoch, _, _)) => epoch as i64,
        None => -1,
    }
}

/// Reads the loss bits from a checkpoint file. Returns 0 on failure.
///
/// # Safety
///
/// path_ptr/path_len must point to valid UTF-8.
pub extern "C" fn fj_rt_checkpoint_loss(path_ptr: *const u8, path_len: i64) -> i64 {
    match read_checkpoint(path_ptr, path_len) {
        Some((_, loss_bits, _)) => loss_bits as i64,
        None => 0,
    }
}

// =====================================================================
// Additional tensor & utility runtime functions
// =====================================================================

/// Returns the mean of all elements in a tensor as f64 bits.
pub extern "C" fn fj_rt_tensor_mean(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid TensorHandle pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let mean = t.mean().unwrap_or(0.0);
    mean.to_bits() as i64
}

/// Extracts a single row from a tensor, returning a new 1×cols tensor.
pub extern "C" fn fj_rt_tensor_row(ptr: *mut u8, row: i64) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let r = row as usize;
    if r >= t.nrows() {
        return std::ptr::null_mut();
    }
    let row_data = t.row(r).to_owned();
    let arr = row_data.insert_axis(ndarray::Axis(0));
    let handle: Box<TensorHandle> = Box::new(arr);
    Box::into_raw(handle) as *mut u8
}

/// Applies element-wise abs to a tensor.
pub extern "C" fn fj_rt_tensor_abs(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = t.mapv(f64::abs);
    let handle: Box<TensorHandle> = Box::new(result);
    Box::into_raw(handle) as *mut u8
}

/// Creates a tensor filled with a specific value (as f64 bits).
pub extern "C" fn fj_rt_tensor_fill(rows: i64, cols: i64, val_bits: i64) -> *mut u8 {
    let val = f64::from_bits(val_bits as u64);
    let arr = Array2::from_elem((rows.max(1) as usize, cols.max(1) as usize), val);
    let handle: Box<TensorHandle> = Box::new(arr);
    Box::into_raw(handle) as *mut u8
}

/// Creates a tensor with random values from [0, 1).
pub extern "C" fn fj_rt_tensor_rand(rows: i64, cols: i64) -> *mut u8 {
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    let arr = Array2::random(
        (rows.max(1) as usize, cols.max(1) as usize),
        Uniform::new(0.0, 1.0),
    );
    let handle: Box<TensorHandle> = Box::new(arr);
    Box::into_raw(handle) as *mut u8
}

/// Xavier initialization: random values in [-limit, limit] where limit = sqrt(6 / (rows+cols)).
pub extern "C" fn fj_rt_tensor_xavier(rows: i64, cols: i64) -> *mut u8 {
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    let r = rows.max(1) as usize;
    let c = cols.max(1) as usize;
    let limit = (6.0 / (r + c) as f64).sqrt();
    let arr = Array2::random((r, c), Uniform::new(-limit, limit));
    let handle: Box<TensorHandle> = Box::new(arr);
    Box::into_raw(handle) as *mut u8
}

/// Creates a tensor from a flat data buffer. data_ptr points to N f64-bit values.
/// Returns a (rows, cols) tensor. If rows*cols != n_elems, uses min(rows*cols, n_elems).
pub extern "C" fn fj_rt_tensor_from_data(
    data_ptr: *const i64,
    n_elems: i64,
    rows: i64,
    cols: i64,
) -> *mut u8 {
    if data_ptr.is_null() || n_elems <= 0 {
        return fj_rt_tensor_zeros(rows.max(1), cols.max(1));
    }
    let r = rows.max(1) as usize;
    let c = cols.max(1) as usize;
    let n = n_elems as usize;
    let total = r * c;
    // SAFETY: caller guarantees data_ptr is valid for n_elems i64 values
    let slice = unsafe { std::slice::from_raw_parts(data_ptr, n) };
    let mut values: Vec<f64> = slice
        .iter()
        .take(total)
        .map(|&v| f64::from_bits(v as u64))
        .collect();
    values.resize(total, 0.0);
    let arr = Array2::from_shape_vec((r, c), values).unwrap_or_else(|_| Array2::zeros((r, c)));
    let handle: Box<TensorHandle> = Box::new(arr);
    Box::into_raw(handle) as *mut u8
}

/// Returns the index of the maximum element in the tensor (flattened).
pub extern "C" fn fj_rt_tensor_argmax(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid TensorHandle pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let mut best_idx = 0usize;
    let mut best_val = f64::NEG_INFINITY;
    for (i, &v) in t.iter().enumerate() {
        if v > best_val {
            best_val = v;
            best_idx = i;
        }
    }
    best_idx as i64
}

/// Scales all elements in a tensor by a scalar (f64 bits). Returns new tensor.
pub extern "C" fn fj_rt_tensor_scale(ptr: *mut u8, scalar_bits: i64) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle pointer
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let scalar = f64::from_bits(scalar_bits as u64);
    let result = t * scalar;
    let handle: Box<TensorHandle> = Box::new(result);
    Box::into_raw(handle) as *mut u8
}

/// Returns a random integer in [0, max) range.
pub extern "C" fn fj_rt_random_int(max: i64) -> i64 {
    if max <= 0 {
        return 0;
    }
    use ndarray_rand::rand::Rng;
    let mut rng = ndarray_rand::rand::thread_rng();
    rng.gen_range(0..max)
}

// ═══════════════════════════════════════════════════════════════════════
// Saturating arithmetic
// ═══════════════════════════════════════════════════════════════════════

/// Saturating i64 addition: clamps to i64::MIN/MAX on overflow.
pub extern "C" fn fj_rt_saturating_add(a: i64, b: i64) -> i64 {
    a.saturating_add(b)
}

/// Saturating i64 subtraction: clamps to i64::MIN/MAX on overflow.
pub extern "C" fn fj_rt_saturating_sub(a: i64, b: i64) -> i64 {
    a.saturating_sub(b)
}

/// Saturating i64 multiplication: clamps to i64::MIN/MAX on overflow.
pub extern "C" fn fj_rt_saturating_mul(a: i64, b: i64) -> i64 {
    a.saturating_mul(b)
}

// ═══════════════════════════════════════════════════════════════════════
// Arc (atomic reference counting)
// ═══════════════════════════════════════════════════════════════════════

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// Opaque Arc handle: heap-allocated Arc<AtomicI64>.
struct ArcHandle {
    inner: Arc<AtomicI64>,
}

/// Runtime: creates a new Arc with initial value.
pub extern "C" fn fj_rt_arc_new(value: i64) -> *mut u8 {
    let handle = Box::new(ArcHandle {
        inner: Arc::new(AtomicI64::new(value)),
    });
    Box::into_raw(handle) as *mut u8
}

/// Runtime: clones an Arc (increments reference count).
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_arc_new` or `fj_rt_arc_clone`.
pub extern "C" fn fj_rt_arc_clone(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid ArcHandle pointer
    let handle = unsafe { &*(ptr as *const ArcHandle) };
    let cloned = Box::new(ArcHandle {
        inner: Arc::clone(&handle.inner),
    });
    Box::into_raw(cloned) as *mut u8
}

/// Runtime: loads the current value from Arc (atomic read).
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_arc_new` or `fj_rt_arc_clone`.
pub extern "C" fn fj_rt_arc_load(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid ArcHandle pointer
    let handle = unsafe { &*(ptr as *const ArcHandle) };
    handle.inner.load(Ordering::SeqCst)
}

/// Runtime: stores a new value into Arc (atomic write).
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_arc_new` or `fj_rt_arc_clone`.
pub extern "C" fn fj_rt_arc_store(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid ArcHandle pointer
    let handle = unsafe { &*(ptr as *const ArcHandle) };
    handle.inner.store(value, Ordering::SeqCst);
}

/// Runtime: drops an Arc handle (decrements reference count, frees if last).
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_arc_new` or `fj_rt_arc_clone`.
pub extern "C" fn fj_rt_arc_drop(ptr: *mut u8) {
    // SAFETY: caller guarantees valid ArcHandle pointer
    unsafe {
        let _ = Box::from_raw(ptr as *mut ArcHandle);
    }
}

/// Runtime: returns the current strong reference count.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_arc_new` or `fj_rt_arc_clone`.
pub extern "C" fn fj_rt_arc_strong_count(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid ArcHandle pointer
    let handle = unsafe { &*(ptr as *const ArcHandle) };
    Arc::strong_count(&handle.inner) as i64
}

// ── Built-in Allocators (S16.2) ─────────────────────────────────────

/// Internal state for a bump allocator.
struct BumpAlloc {
    buffer: *mut u8,
    capacity: usize,
    offset: usize,
}

/// Runtime: create a new bump allocator with given capacity.
pub extern "C" fn fj_rt_bump_new(capacity: i64) -> *mut u8 {
    let cap = capacity as usize;
    let layout = std::alloc::Layout::from_size_align(cap, 8).unwrap_or(
        // SAFETY: fallback to minimal layout if size is invalid
        std::alloc::Layout::from_size_align(8, 8).expect("valid layout"),
    );
    // SAFETY: we allocate a valid buffer with known layout
    let buffer = unsafe { std::alloc::alloc_zeroed(layout) };
    let state = Box::new(BumpAlloc {
        buffer,
        capacity: cap,
        offset: 0,
    });
    Box::into_raw(state) as *mut u8
}

/// Runtime: allocate `size` bytes from bump allocator. Returns 0 if exhausted.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_bump_new`.
pub extern "C" fn fj_rt_bump_alloc(ptr: *mut u8, size: i64) -> i64 {
    // SAFETY: caller guarantees valid BumpAlloc pointer
    let state = unsafe { &mut *(ptr as *mut BumpAlloc) };
    let sz = size as usize;
    // Align to 8 bytes
    let aligned_offset = (state.offset + 7) & !7;
    if aligned_offset + sz > state.capacity {
        return 0; // exhausted
    }
    // SAFETY: buffer is valid and offset is within bounds
    let result = unsafe { state.buffer.add(aligned_offset) };
    state.offset = aligned_offset + sz;
    result as i64
}

/// Runtime: reset bump allocator (all allocations invalidated).
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_bump_new`.
pub extern "C" fn fj_rt_bump_reset(ptr: *mut u8) {
    // SAFETY: caller guarantees valid BumpAlloc pointer
    let state = unsafe { &mut *(ptr as *mut BumpAlloc) };
    state.offset = 0;
}

/// Runtime: destroy bump allocator and free its buffer.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_bump_new`.
pub extern "C" fn fj_rt_bump_destroy(ptr: *mut u8) {
    // SAFETY: caller guarantees valid BumpAlloc pointer
    let state = unsafe { Box::from_raw(ptr as *mut BumpAlloc) };
    if !state.buffer.is_null() {
        let layout = std::alloc::Layout::from_size_align(state.capacity, 8)
            .unwrap_or(std::alloc::Layout::from_size_align(8, 8).expect("valid layout"));
        // SAFETY: buffer was allocated with this layout
        unsafe { std::alloc::dealloc(state.buffer, layout) };
    }
}

/// Internal node for free list allocator.
struct FreeNode {
    offset: usize,
    size: usize,
}

/// Internal state for a free list allocator.
struct FreeListAlloc {
    buffer: *mut u8,
    capacity: usize,
    free_list: Vec<FreeNode>,
}

/// Runtime: create a new free list allocator with given capacity.
pub extern "C" fn fj_rt_freelist_new(capacity: i64) -> *mut u8 {
    let cap = capacity as usize;
    let layout = std::alloc::Layout::from_size_align(cap, 8)
        .unwrap_or(std::alloc::Layout::from_size_align(8, 8).expect("valid layout"));
    // SAFETY: we allocate a valid buffer with known layout
    let buffer = unsafe { std::alloc::alloc_zeroed(layout) };
    let state = Box::new(FreeListAlloc {
        buffer,
        capacity: cap,
        free_list: vec![FreeNode {
            offset: 0,
            size: cap,
        }],
    });
    Box::into_raw(state) as *mut u8
}

/// Runtime: allocate `size` bytes from free list (first-fit). Returns 0 if no space.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_freelist_new`.
pub extern "C" fn fj_rt_freelist_alloc(ptr: *mut u8, size: i64) -> i64 {
    // SAFETY: caller guarantees valid FreeListAlloc pointer
    let state = unsafe { &mut *(ptr as *mut FreeListAlloc) };
    let sz = (size as usize + 7) & !7; // align to 8
                                       // First-fit search
    for i in 0..state.free_list.len() {
        if state.free_list[i].size >= sz {
            let offset = state.free_list[i].offset;
            if state.free_list[i].size == sz {
                state.free_list.remove(i);
            } else {
                state.free_list[i].offset += sz;
                state.free_list[i].size -= sz;
            }
            // SAFETY: buffer is valid and offset is within bounds
            let result = unsafe { state.buffer.add(offset) };
            return result as i64;
        }
    }
    0 // no space
}

/// Runtime: free a block back to the free list.
///
/// # Safety
///
/// The caller must ensure `alloc_ptr` was produced by `fj_rt_freelist_alloc`
/// and `ptr` was produced by `fj_rt_freelist_new`.
pub extern "C" fn fj_rt_freelist_free(ptr: *mut u8, alloc_ptr: i64, size: i64) {
    // SAFETY: caller guarantees valid FreeListAlloc pointer
    let state = unsafe { &mut *(ptr as *mut FreeListAlloc) };
    let offset = (alloc_ptr as usize).wrapping_sub(state.buffer as usize);
    let sz = (size as usize + 7) & !7;
    state.free_list.push(FreeNode { offset, size: sz });
    // Simple coalesce: sort by offset and merge adjacent
    state.free_list.sort_by_key(|n| n.offset);
    let mut i = 0;
    while i + 1 < state.free_list.len() {
        if state.free_list[i].offset + state.free_list[i].size == state.free_list[i + 1].offset {
            state.free_list[i].size += state.free_list[i + 1].size;
            state.free_list.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

/// Runtime: destroy free list allocator.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_freelist_new`.
pub extern "C" fn fj_rt_freelist_destroy(ptr: *mut u8) {
    // SAFETY: caller guarantees valid FreeListAlloc pointer
    let state = unsafe { Box::from_raw(ptr as *mut FreeListAlloc) };
    if !state.buffer.is_null() {
        let layout = std::alloc::Layout::from_size_align(state.capacity, 8)
            .unwrap_or(std::alloc::Layout::from_size_align(8, 8).expect("valid layout"));
        // SAFETY: buffer was allocated with this layout
        unsafe { std::alloc::dealloc(state.buffer, layout) };
    }
}

/// Internal state for a pool allocator.
struct PoolAlloc {
    buffer: *mut u8,
    block_size: usize,
    block_count: usize,
    free_indices: Vec<usize>,
}

/// Runtime: create a new pool allocator with `block_count` blocks of `block_size` bytes.
pub extern "C" fn fj_rt_pool_new(block_size: i64, block_count: i64) -> *mut u8 {
    let bs = ((block_size as usize) + 7) & !7; // align to 8
    let bc = block_count as usize;
    let total = bs * bc;
    let layout = std::alloc::Layout::from_size_align(total.max(8), 8)
        .unwrap_or(std::alloc::Layout::from_size_align(8, 8).expect("valid layout"));
    // SAFETY: we allocate a valid buffer
    let buffer = unsafe { std::alloc::alloc_zeroed(layout) };
    let free_indices = (0..bc).rev().collect();
    let state = Box::new(PoolAlloc {
        buffer,
        block_size: bs,
        block_count: bc,
        free_indices,
    });
    Box::into_raw(state) as *mut u8
}

/// Runtime: allocate one block from pool. Returns 0 if exhausted.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_pool_new`.
pub extern "C" fn fj_rt_pool_alloc(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid PoolAlloc pointer
    let state = unsafe { &mut *(ptr as *mut PoolAlloc) };
    match state.free_indices.pop() {
        Some(idx) => {
            // SAFETY: idx < block_count, buffer is valid
            let result = unsafe { state.buffer.add(idx * state.block_size) };
            result as i64
        }
        None => 0, // exhausted
    }
}

/// Runtime: free one block back to pool.
///
/// # Safety
///
/// The caller must ensure `alloc_ptr` was produced by `fj_rt_pool_alloc`
/// and `ptr` was produced by `fj_rt_pool_new`.
pub extern "C" fn fj_rt_pool_free(ptr: *mut u8, alloc_ptr: i64) {
    // SAFETY: caller guarantees valid PoolAlloc pointer
    let state = unsafe { &mut *(ptr as *mut PoolAlloc) };
    let offset = (alloc_ptr as usize).wrapping_sub(state.buffer as usize);
    let idx = offset / state.block_size;
    if idx < state.block_count {
        state.free_indices.push(idx);
    }
}

/// Runtime: destroy pool allocator.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_pool_new`.
pub extern "C" fn fj_rt_pool_destroy(ptr: *mut u8) {
    // SAFETY: caller guarantees valid PoolAlloc pointer
    let state = unsafe { Box::from_raw(ptr as *mut PoolAlloc) };
    if !state.buffer.is_null() {
        let total = state.block_size * state.block_count;
        let layout = std::alloc::Layout::from_size_align(total.max(8), 8)
            .unwrap_or(std::alloc::Layout::from_size_align(8, 8).expect("valid layout"));
        // SAFETY: buffer was allocated with this layout
        unsafe { std::alloc::dealloc(state.buffer, layout) };
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Async/Future Runtime
// ═══════════════════════════════════════════════════════════════════════

/// Heap-allocated future state for async function state machines.
struct FutureHandle {
    /// Current state machine state (0 = initial, -1 = completed).
    state: i64,
    /// Result value when future is complete.
    result: i64,
    /// Whether the future has resolved.
    is_ready: bool,
    /// Saved local variables across await points.
    locals: Vec<i64>,
}

/// Creates a new unresolved future handle.
///
/// Returns an opaque pointer (as `*mut u8`) to a heap-allocated `FutureHandle`.
pub extern "C" fn fj_rt_future_new() -> *mut u8 {
    let handle = Box::new(FutureHandle {
        state: 0,
        result: 0,
        is_ready: false,
        locals: Vec::new(),
    });
    Box::into_raw(handle) as *mut u8
}

/// Checks if a future is ready. Returns 1 if ready, 0 if pending.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_poll(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &*(ptr as *const FutureHandle) };
    if handle.is_ready {
        1
    } else {
        0
    }
}

/// Gets the result value of a completed future.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_get_result(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &*(ptr as *const FutureHandle) };
    handle.result
}

/// Sets the future as ready with a result value.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_set_result(ptr: *mut u8, value: i64) {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &mut *(ptr as *mut FutureHandle) };
    handle.result = value;
    handle.is_ready = true;
}

/// Gets the current state machine state.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_get_state(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &*(ptr as *const FutureHandle) };
    handle.state
}

/// Sets the state machine state.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_set_state(ptr: *mut u8, state: i64) {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &mut *(ptr as *mut FutureHandle) };
    handle.state = state;
}

/// Saves a local variable at the given index in the future's state.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_save_local(ptr: *mut u8, index: i64, value: i64) {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &mut *(ptr as *mut FutureHandle) };
    let idx = index as usize;
    if idx >= handle.locals.len() {
        handle.locals.resize(idx + 1, 0);
    }
    handle.locals[idx] = value;
}

/// Loads a saved local variable from the given index.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_load_local(ptr: *mut u8, index: i64) -> i64 {
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &*(ptr as *const FutureHandle) };
    let idx = index as usize;
    if idx < handle.locals.len() {
        handle.locals[idx]
    } else {
        0
    }
}

/// Frees a future handle.
///
/// # Safety
///
/// The caller must ensure `ptr` was produced by `fj_rt_future_new`.
pub extern "C" fn fj_rt_future_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid FutureHandle pointer
        let _ = unsafe { Box::from_raw(ptr as *mut FutureHandle) };
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Async executor
// ═══════════════════════════════════════════════════════════════════════

/// Executor: single-threaded task runner for async futures.
struct ExecutorHandle {
    /// Spawned tasks (FutureHandle pointers)
    tasks: Vec<*mut u8>,
}

/// Runtime: creates a new executor.
pub extern "C" fn fj_rt_executor_new() -> *mut u8 {
    let exec = ExecutorHandle { tasks: Vec::new() };
    Box::into_raw(Box::new(exec)) as *mut u8
}

/// Runtime: runs a single future to completion (blocking).
///
/// Polls the future until it is ready, then returns its result.
pub extern "C" fn fj_rt_executor_block_on(future_ptr: *mut u8) -> i64 {
    if future_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid FutureHandle pointer
    let handle = unsafe { &*(future_ptr as *const FutureHandle) };
    // In eager model, future is always ready when block_on is called.
    // For lazy futures (S10.2+), this would loop with waker-based polling.
    handle.result
}

/// Runtime: spawns a future as a task on the executor.
pub extern "C" fn fj_rt_executor_spawn(exec_ptr: *mut u8, future_ptr: *mut u8) {
    if exec_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid ExecutorHandle pointer
    let exec = unsafe { &mut *(exec_ptr as *mut ExecutorHandle) };
    exec.tasks.push(future_ptr);
}

/// Runtime: runs all spawned tasks to completion. Returns number of completed tasks.
pub extern "C" fn fj_rt_executor_run(exec_ptr: *mut u8) -> i64 {
    if exec_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ExecutorHandle pointer
    let exec = unsafe { &mut *(exec_ptr as *mut ExecutorHandle) };
    let mut completed = 0i64;
    for &task_ptr in &exec.tasks {
        if task_ptr.is_null() {
            continue;
        }
        // SAFETY: task_ptr is a valid FutureHandle
        let handle = unsafe { &*(task_ptr as *const FutureHandle) };
        if handle.is_ready {
            completed += 1;
        }
    }
    completed
}

/// Runtime: gets the result of a spawned task by index.
pub extern "C" fn fj_rt_executor_get_result(exec_ptr: *mut u8, index: i64) -> i64 {
    if exec_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ExecutorHandle pointer
    let exec = unsafe { &*(exec_ptr as *const ExecutorHandle) };
    let idx = index as usize;
    if idx >= exec.tasks.len() {
        return 0;
    }
    let task_ptr = exec.tasks[idx];
    if task_ptr.is_null() {
        return 0;
    }
    // SAFETY: task_ptr is a valid FutureHandle
    let handle = unsafe { &*(task_ptr as *const FutureHandle) };
    handle.result
}

/// Runtime: frees executor and all its spawned tasks.
pub extern "C" fn fj_rt_executor_free(exec_ptr: *mut u8) {
    if exec_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid ExecutorHandle pointer
    let exec = unsafe { Box::from_raw(exec_ptr as *mut ExecutorHandle) };
    for &task_ptr in &exec.tasks {
        if !task_ptr.is_null() {
            // Free each FutureHandle
            let _ = unsafe { Box::from_raw(task_ptr as *mut FutureHandle) };
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Waker
// ═══════════════════════════════════════════════════════════════════════

/// Waker: reference-counted wake flag for async tasks.
struct WakerHandle {
    /// Wake flag: set to true when waker.wake() is called.
    woken: bool,
    /// Reference count for clone/drop.
    ref_count: i64,
}

/// Runtime: creates a new waker.
pub extern "C" fn fj_rt_waker_new() -> *mut u8 {
    let waker = WakerHandle {
        woken: false,
        ref_count: 1,
    };
    Box::into_raw(Box::new(waker)) as *mut u8
}

/// Runtime: wakes the waker (sets the wake flag).
pub extern "C" fn fj_rt_waker_wake(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid WakerHandle pointer
    let waker = unsafe { &mut *(ptr as *mut WakerHandle) };
    waker.woken = true;
}

/// Runtime: checks if the waker has been woken.
pub extern "C" fn fj_rt_waker_is_woken(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid WakerHandle pointer
    let waker = unsafe { &*(ptr as *const WakerHandle) };
    if waker.woken {
        1
    } else {
        0
    }
}

/// Runtime: resets the waker's wake flag.
pub extern "C" fn fj_rt_waker_reset(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid WakerHandle pointer
    let waker = unsafe { &mut *(ptr as *mut WakerHandle) };
    waker.woken = false;
}

/// Runtime: clones a waker (increments reference count).
pub extern "C" fn fj_rt_waker_clone(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid WakerHandle pointer
    let waker = unsafe { &mut *(ptr as *mut WakerHandle) };
    waker.ref_count += 1;
    ptr
}

/// Runtime: drops a waker reference (decrements ref count, frees if zero).
pub extern "C" fn fj_rt_waker_drop(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid WakerHandle pointer
    let waker = unsafe { &mut *(ptr as *mut WakerHandle) };
    waker.ref_count -= 1;
    if waker.ref_count <= 0 {
        let _ = unsafe { Box::from_raw(ptr as *mut WakerHandle) };
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Timer wheel
// ═══════════════════════════════════════════════════════════════════════

/// A single scheduled timer entry.
struct TimerEntry {
    /// When this timer should fire.
    deadline: std::time::Instant,
    /// Waker pointer to wake when the timer fires (may be null).
    waker_ptr: *mut u8,
}

/// Timer wheel: tracks pending timers and fires them when due.
struct TimerWheelHandle {
    /// Scheduled timer entries.
    entries: Vec<TimerEntry>,
}

/// Runtime: creates a new timer wheel.
pub extern "C" fn fj_rt_timer_new() -> *mut u8 {
    let timer = TimerWheelHandle {
        entries: Vec::new(),
    };
    Box::into_raw(Box::new(timer)) as *mut u8
}

/// Runtime: schedules a timer to fire after `millis` milliseconds.
///
/// If `waker_ptr` is non-null, the waker will be woken when the timer fires.
/// Returns the timer entry index.
pub extern "C" fn fj_rt_timer_schedule(timer_ptr: *mut u8, millis: i64, waker_ptr: *mut u8) -> i64 {
    if timer_ptr.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees valid TimerWheelHandle pointer
    let timer = unsafe { &mut *(timer_ptr as *mut TimerWheelHandle) };
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(millis.max(0) as u64);
    let index = timer.entries.len() as i64;
    timer.entries.push(TimerEntry {
        deadline,
        waker_ptr,
    });
    index
}

/// Runtime: checks all timers and fires any that are past their deadline.
///
/// Wakes associated wakers for expired timers. Returns count of timers fired.
pub extern "C" fn fj_rt_timer_tick(timer_ptr: *mut u8) -> i64 {
    if timer_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid TimerWheelHandle pointer
    let timer = unsafe { &mut *(timer_ptr as *mut TimerWheelHandle) };
    let now = std::time::Instant::now();
    let mut fired = 0i64;
    for entry in &mut timer.entries {
        if now >= entry.deadline && !entry.waker_ptr.is_null() {
            // Wake the associated waker
            // SAFETY: waker_ptr is a valid WakerHandle
            let waker = unsafe { &mut *(entry.waker_ptr as *mut WakerHandle) };
            waker.woken = true;
            // Clear waker_ptr to mark as fired
            entry.waker_ptr = std::ptr::null_mut();
            fired += 1;
        }
    }
    fired
}

/// Runtime: returns the number of pending (unfired) timers.
pub extern "C" fn fj_rt_timer_pending(timer_ptr: *mut u8) -> i64 {
    if timer_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid TimerWheelHandle pointer
    let timer = unsafe { &*(timer_ptr as *const TimerWheelHandle) };
    timer
        .entries
        .iter()
        .filter(|e| !e.waker_ptr.is_null())
        .count() as i64
}

/// Runtime: frees the timer wheel.
pub extern "C" fn fj_rt_timer_free(timer_ptr: *mut u8) {
    if timer_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid TimerWheelHandle pointer
    let _ = unsafe { Box::from_raw(timer_ptr as *mut TimerWheelHandle) };
}

// ═══════════════════════════════════════════════════════════════════════
// Thread pool executor
// ═══════════════════════════════════════════════════════════════════════

/// Work-stealing deque: per-thread task queue shared via Arc<Mutex<>>.
type WorkDeque = std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<(usize, usize)>>>;

/// Thread pool: distributes tasks across N worker threads with work-stealing.
struct ThreadPoolHandle {
    /// Number of worker threads.
    thread_count: i64,
    /// Queued tasks (FutureHandle pointers).
    tasks: Vec<*mut u8>,
    /// Results from completed tasks.
    results: Vec<i64>,
    /// JoinHandle pointers (parallel to tasks, nullable).
    join_handles: Vec<*mut u8>,
}

// FutureHandle pointers are sent across threads in the pool.
// SAFETY: The opaque pointers reference heap-allocated FutureHandles
// that are only accessed by one thread at a time (dequeued, then processed).
unsafe impl Send for ThreadPoolHandle {}

/// Runtime: creates a new thread pool with `n` worker threads.
pub extern "C" fn fj_rt_threadpool_new(n: i64) -> *mut u8 {
    let pool = ThreadPoolHandle {
        thread_count: n.max(1),
        tasks: Vec::new(),
        results: Vec::new(),
        join_handles: Vec::new(),
    };
    Box::into_raw(Box::new(pool)) as *mut u8
}

/// Runtime: spawns a task (future) on the thread pool. Returns task index.
pub extern "C" fn fj_rt_threadpool_spawn(pool_ptr: *mut u8, future_ptr: *mut u8) -> i64 {
    if pool_ptr.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees valid ThreadPoolHandle pointer
    let pool = unsafe { &mut *(pool_ptr as *mut ThreadPoolHandle) };
    let index = pool.tasks.len() as i64;
    pool.tasks.push(future_ptr);
    index
}

/// Runtime: runs all queued tasks using work-stealing scheduling.
///
/// Each worker thread has a local deque. Tasks are initially distributed
/// round-robin. Workers pop from their own deque (LIFO for cache locality)
/// and steal from others (FIFO for fairness) when their local deque is empty.
pub extern "C" fn fj_rt_threadpool_run(pool_ptr: *mut u8) -> i64 {
    if pool_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ThreadPoolHandle pointer
    let pool = unsafe { &mut *(pool_ptr as *mut ThreadPoolHandle) };

    let task_count = pool.tasks.len();
    if task_count == 0 {
        return 0;
    }

    let n_threads = (pool.thread_count as usize).min(task_count);
    let results = std::sync::Arc::new(std::sync::Mutex::new(vec![0i64; task_count]));

    // Create per-thread work-stealing deques
    let queues: Vec<WorkDeque> = (0..n_threads)
        .map(|_| std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())))
        .collect();

    // Seed queues with round-robin distribution
    for (i, &task_ptr) in pool.tasks.iter().enumerate() {
        let thread_idx = i % n_threads;
        queues[thread_idx]
            .lock()
            .expect("queue lock")
            .push_back((i, task_ptr as usize));
    }

    let queues_arc = std::sync::Arc::new(queues);
    let mut handles = Vec::new();

    for t in 0..n_threads {
        let results_clone = std::sync::Arc::clone(&results);
        let queues_clone = std::sync::Arc::clone(&queues_arc);

        let handle = std::thread::spawn(move || {
            loop {
                // Try local pop (LIFO for cache locality)
                let task = {
                    let mut my_q = queues_clone[t].lock().expect("local queue lock");
                    my_q.pop_back()
                };

                if let Some((idx, task_addr)) = task {
                    if task_addr != 0 {
                        // SAFETY: FutureHandle pointer, read-only access to extract result
                        let future = unsafe { &*(task_addr as *const FutureHandle) };
                        let mut lock = results_clone.lock().expect("results lock");
                        lock[idx] = future.result;
                    }
                    continue;
                }

                // Local queue empty — steal from others (FIFO for fairness)
                let mut stolen = false;
                for other_t in 0..queues_clone.len() {
                    if other_t == t {
                        continue;
                    }
                    let task = {
                        let mut other_q = queues_clone[other_t].lock().expect("steal queue lock");
                        other_q.pop_front()
                    };
                    if let Some((idx, task_addr)) = task {
                        if task_addr != 0 {
                            // SAFETY: FutureHandle pointer, read-only access to extract result
                            let future = unsafe { &*(task_addr as *const FutureHandle) };
                            let mut lock = results_clone.lock().expect("results lock");
                            lock[idx] = future.result;
                        }
                        stolen = true;
                        break;
                    }
                }

                if !stolen {
                    break;
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all workers
    for h in handles {
        let _ = h.join();
    }

    // Store results and set JoinHandle results
    let lock = results.lock().expect("results lock");
    pool.results = lock.clone();

    // Notify JoinHandles if any
    for (i, &jh_ptr) in pool.join_handles.iter().enumerate() {
        if !jh_ptr.is_null() && i < pool.results.len() {
            fj_rt_joinhandle_set_result(jh_ptr, pool.results[i]);
        }
    }

    task_count as i64
}

/// Runtime: gets the result of a completed task by index.
pub extern "C" fn fj_rt_threadpool_get_result(pool_ptr: *mut u8, index: i64) -> i64 {
    if pool_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ThreadPoolHandle pointer
    let pool = unsafe { &*(pool_ptr as *const ThreadPoolHandle) };
    let idx = index as usize;
    if idx >= pool.results.len() {
        return 0;
    }
    pool.results[idx]
}

/// Runtime: returns the number of worker threads in the pool.
pub extern "C" fn fj_rt_threadpool_thread_count(pool_ptr: *mut u8) -> i64 {
    if pool_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ThreadPoolHandle pointer
    let pool = unsafe { &*(pool_ptr as *const ThreadPoolHandle) };
    pool.thread_count
}

/// Runtime: spawns a task and returns a JoinHandle for cross-thread result retrieval.
pub extern "C" fn fj_rt_threadpool_spawn_join(pool_ptr: *mut u8, future_ptr: *mut u8) -> *mut u8 {
    if pool_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid ThreadPoolHandle pointer
    let pool = unsafe { &mut *(pool_ptr as *mut ThreadPoolHandle) };
    let jh_ptr = fj_rt_joinhandle_new();
    pool.tasks.push(future_ptr);
    pool.join_handles.push(jh_ptr);
    jh_ptr
}

/// Runtime: frees the thread pool and all its tasks.
pub extern "C" fn fj_rt_threadpool_free(pool_ptr: *mut u8) {
    if pool_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid ThreadPoolHandle pointer
    let pool = unsafe { Box::from_raw(pool_ptr as *mut ThreadPoolHandle) };
    for &task_ptr in &pool.tasks {
        if !task_ptr.is_null() {
            // Free each FutureHandle
            let _ = unsafe { Box::from_raw(task_ptr as *mut FutureHandle) };
        }
    }
    // Note: JoinHandles are owned by the caller, not freed here
}

// ═══════════════════════════════════════════════════════════════════════
// S11.3 — JoinHandle (cross-thread result transfer)
// ═══════════════════════════════════════════════════════════════════════

/// Shared inner state for JoinHandle — thread-safe via atomics and mutex.
struct JoinHandleInner {
    /// The result value, set when the task completes.
    result: std::sync::Mutex<Option<i64>>,
    /// Atomic flag: true when result is available.
    ready: std::sync::atomic::AtomicBool,
    /// Atomic flag: true when task has been cancelled.
    cancelled: std::sync::atomic::AtomicBool,
}

/// Async join handle for cross-thread result transfer.
struct AsyncJoinHandle {
    inner: std::sync::Arc<JoinHandleInner>,
}

// SAFETY: JoinHandleInner uses only thread-safe primitives (Mutex, AtomicBool).
unsafe impl Send for AsyncJoinHandle {}
unsafe impl Sync for AsyncJoinHandle {}

/// Runtime: creates a new JoinHandle.
pub extern "C" fn fj_rt_joinhandle_new() -> *mut u8 {
    let handle = AsyncJoinHandle {
        inner: std::sync::Arc::new(JoinHandleInner {
            result: std::sync::Mutex::new(None),
            ready: std::sync::atomic::AtomicBool::new(false),
            cancelled: std::sync::atomic::AtomicBool::new(false),
        }),
    };
    Box::into_raw(Box::new(handle)) as *mut u8
}

/// Runtime: sets the result on a JoinHandle and marks it ready.
pub extern "C" fn fj_rt_joinhandle_set_result(ptr: *mut u8, value: i64) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncJoinHandle pointer
    let handle = unsafe { &*(ptr as *const AsyncJoinHandle) };
    *handle.inner.result.lock().expect("joinhandle lock") = Some(value);
    handle.inner.ready.store(true, {
        use std::sync::atomic::Ordering;
        Ordering::Release
    });
}

/// Runtime: checks if a JoinHandle result is ready (0=pending, 1=ready).
pub extern "C" fn fj_rt_joinhandle_is_ready(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncJoinHandle pointer
    let handle = unsafe { &*(ptr as *const AsyncJoinHandle) };
    i64::from(handle.inner.ready.load({
        use std::sync::atomic::Ordering;
        Ordering::Acquire
    }))
}

/// Runtime: gets the result from a JoinHandle, spinning until ready.
/// Returns -1 if cancelled before or without a result being set.
pub extern "C" fn fj_rt_joinhandle_get_result(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncJoinHandle pointer
    let handle = unsafe { &*(ptr as *const AsyncJoinHandle) };
    // Spin-wait with yield to avoid busy-waiting
    loop {
        use std::sync::atomic::Ordering;
        if handle.inner.cancelled.load(Ordering::Acquire) {
            // If cancelled but a real result was set (task completed before abort),
            // return the result; otherwise return -1
            let lock = handle.inner.result.lock().expect("joinhandle lock");
            return lock.unwrap_or(-1);
        }
        if handle.inner.ready.load(Ordering::Acquire) {
            return handle
                .inner
                .result
                .lock()
                .expect("joinhandle lock")
                .unwrap_or(0);
        }
        std::thread::yield_now();
    }
}

/// Runtime: frees a JoinHandle.
pub extern "C" fn fj_rt_joinhandle_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncJoinHandle pointer
    let _ = unsafe { Box::from_raw(ptr as *mut AsyncJoinHandle) };
}

// ═══════════════════════════════════════════════════════════════════════
// S11.4 — Cooperative cancellation
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: aborts a task via its JoinHandle (cooperative cancellation).
/// Sets the cancel flag and marks as ready so `get_result` unblocks.
pub extern "C" fn fj_rt_joinhandle_abort(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncJoinHandle pointer
    let handle = unsafe { &*(ptr as *const AsyncJoinHandle) };
    use std::sync::atomic::Ordering;
    handle.inner.cancelled.store(true, Ordering::Release);
    // Mark ready so get_result unblocks with -1
    handle.inner.ready.store(true, Ordering::Release);
}

/// Runtime: checks if a JoinHandle has been cancelled (0=no, 1=yes).
pub extern "C" fn fj_rt_joinhandle_is_cancelled(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncJoinHandle pointer
    let handle = unsafe { &*(ptr as *const AsyncJoinHandle) };
    i64::from(handle.inner.cancelled.load({
        use std::sync::atomic::Ordering;
        Ordering::Acquire
    }))
}

// ═══════════════════════════════════════════════════════════════════════
// S12.1 — Async channels
// ═══════════════════════════════════════════════════════════════════════

/// Async channel handle wrapping an unbounded MPSC channel.
///
/// Supports async send/recv semantics. In the current eager model,
/// send/recv complete immediately; the async interface is for API
/// consistency with async/await.
struct AsyncChannelHandle {
    sender: std::sync::Mutex<Option<std::sync::mpsc::Sender<i64>>>,
    receiver: std::sync::Mutex<Option<std::sync::mpsc::Receiver<i64>>>,
    closed: std::sync::atomic::AtomicBool,
}

/// Async bounded channel handle wrapping a bounded MPSC channel.
struct AsyncBoundedChannelHandle {
    sender: std::sync::Mutex<Option<std::sync::mpsc::SyncSender<i64>>>,
    receiver: std::sync::Mutex<Option<std::sync::mpsc::Receiver<i64>>>,
    closed: std::sync::atomic::AtomicBool,
}

/// Runtime: creates a new async unbounded channel.
pub extern "C" fn fj_rt_async_channel_new() -> *mut u8 {
    let (sender, receiver) = std::sync::mpsc::channel();
    let ch = Box::new(AsyncChannelHandle {
        sender: std::sync::Mutex::new(Some(sender)),
        receiver: std::sync::Mutex::new(Some(receiver)),
        closed: std::sync::atomic::AtomicBool::new(false),
    });
    Box::into_raw(ch) as *mut u8
}

/// Runtime: creates a new async bounded channel with given capacity.
pub extern "C" fn fj_rt_async_channel_bounded(capacity: i64) -> *mut u8 {
    let (sender, receiver) = std::sync::mpsc::sync_channel(capacity.max(1) as usize);
    let ch = Box::new(AsyncBoundedChannelHandle {
        sender: std::sync::Mutex::new(Some(sender)),
        receiver: std::sync::Mutex::new(Some(receiver)),
        closed: std::sync::atomic::AtomicBool::new(false),
    });
    Box::into_raw(ch) as *mut u8
}

/// Runtime: async send on unbounded channel. Returns 1 on success, 0 if closed.
pub extern "C" fn fj_rt_async_channel_send(ptr: *mut u8, value: i64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncChannelHandle pointer
    let ch = unsafe { &*(ptr as *const AsyncChannelHandle) };
    use std::sync::atomic::Ordering;
    if ch.closed.load(Ordering::Acquire) {
        return 0;
    }
    let lock = ch.sender.lock().expect("async channel sender lock");
    match lock.as_ref() {
        Some(sender) => i64::from(sender.send(value).is_ok()),
        None => 0,
    }
}

/// Runtime: async recv on unbounded channel. Returns the value, or 0 if closed/empty.
pub extern "C" fn fj_rt_async_channel_recv(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncChannelHandle pointer
    let ch = unsafe { &*(ptr as *const AsyncChannelHandle) };
    let lock = ch.receiver.lock().expect("async channel receiver lock");
    match lock.as_ref() {
        Some(receiver) => receiver.recv().unwrap_or(0),
        None => 0,
    }
}

/// Runtime: close async channel (drops sender).
pub extern "C" fn fj_rt_async_channel_close(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncChannelHandle pointer
    let ch = unsafe { &*(ptr as *const AsyncChannelHandle) };
    use std::sync::atomic::Ordering;
    ch.closed.store(true, Ordering::Release);
    let mut lock = ch.sender.lock().expect("async channel sender lock");
    *lock = None; // Drop sender → receiver gets Disconnected
}

/// Runtime: free async channel.
pub extern "C" fn fj_rt_async_channel_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncChannelHandle pointer
    let _ = unsafe { Box::from_raw(ptr as *mut AsyncChannelHandle) };
}

/// Runtime: async send on bounded channel. Returns 1 on success, 0 if full/closed.
pub extern "C" fn fj_rt_async_bchannel_send(ptr: *mut u8, value: i64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncBoundedChannelHandle pointer
    let ch = unsafe { &*(ptr as *const AsyncBoundedChannelHandle) };
    use std::sync::atomic::Ordering;
    if ch.closed.load(Ordering::Acquire) {
        return 0;
    }
    let lock = ch.sender.lock().expect("async bchannel sender lock");
    match lock.as_ref() {
        Some(sender) => i64::from(sender.send(value).is_ok()),
        None => 0,
    }
}

/// Runtime: async recv on bounded channel. Returns the value, or 0 if closed/empty.
pub extern "C" fn fj_rt_async_bchannel_recv(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid AsyncBoundedChannelHandle pointer
    let ch = unsafe { &*(ptr as *const AsyncBoundedChannelHandle) };
    let lock = ch.receiver.lock().expect("async bchannel receiver lock");
    match lock.as_ref() {
        Some(receiver) => receiver.recv().unwrap_or(0),
        None => 0,
    }
}

/// Runtime: close async bounded channel.
pub extern "C" fn fj_rt_async_bchannel_close(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncBoundedChannelHandle pointer
    let ch = unsafe { &*(ptr as *const AsyncBoundedChannelHandle) };
    use std::sync::atomic::Ordering;
    ch.closed.store(true, Ordering::Release);
    let mut lock = ch.sender.lock().expect("async bchannel sender lock");
    *lock = None;
}

/// Runtime: free async bounded channel.
pub extern "C" fn fj_rt_async_bchannel_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid AsyncBoundedChannelHandle pointer
    let _ = unsafe { Box::from_raw(ptr as *mut AsyncBoundedChannelHandle) };
}

// ═══════════════════════════════════════════════════════════════════════
// S36.3 — MNIST IDX format parser
// ═══════════════════════════════════════════════════════════════════════

/// Parses an IDX file buffer into image data.
///
/// IDX image format: magic(0x00000803), n_images, n_rows, n_cols, pixel_data
/// Returns a tensor of shape (n_images, n_rows * n_cols) with f64 pixel values [0.0, 255.0].
fn parse_idx_images(data: &[u8]) -> Option<Array2<f64>> {
    if data.len() < 16 {
        return None;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic != 0x00000803 {
        return None;
    }
    let n_images = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let n_rows = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let n_cols = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
    let pixel_count = n_rows * n_cols;
    let expected_len = 16 + n_images * pixel_count;
    if data.len() < expected_len {
        return None;
    }
    let mut tensor = Array2::<f64>::zeros((n_images, pixel_count));
    for i in 0..n_images {
        for j in 0..pixel_count {
            tensor[[i, j]] = data[16 + i * pixel_count + j] as f64;
        }
    }
    Some(tensor)
}

/// Parses an IDX file buffer into label data.
///
/// IDX label format: magic(0x00000801), n_labels, label_data
/// Returns a tensor of shape (n_labels, 1) with f64 label values [0.0, 9.0].
fn parse_idx_labels(data: &[u8]) -> Option<Array2<f64>> {
    if data.len() < 8 {
        return None;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic != 0x00000801 {
        return None;
    }
    let n_labels = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    if data.len() < 8 + n_labels {
        return None;
    }
    let mut tensor = Array2::<f64>::zeros((n_labels, 1));
    for i in 0..n_labels {
        tensor[[i, 0]] = data[8 + i] as f64;
    }
    Some(tensor)
}

/// Runtime: loads MNIST images from an IDX file. Returns tensor ptr or null.
pub extern "C" fn fj_rt_mnist_load_images(path_ptr: *const u8, path_len: i64) -> *mut u8 {
    if path_ptr.is_null() || path_len <= 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid string pointer
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return std::ptr::null_mut(),
    };
    match parse_idx_images(&data) {
        Some(tensor) => Box::into_raw(Box::new(tensor)) as *mut u8,
        None => std::ptr::null_mut(),
    }
}

/// Runtime: loads MNIST labels from an IDX file. Returns tensor ptr or null.
pub extern "C" fn fj_rt_mnist_load_labels(path_ptr: *const u8, path_len: i64) -> *mut u8 {
    if path_ptr.is_null() || path_len <= 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid string pointer
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return std::ptr::null_mut(),
    };
    match parse_idx_labels(&data) {
        Some(tensor) => Box::into_raw(Box::new(tensor)) as *mut u8,
        None => std::ptr::null_mut(),
    }
}

/// Runtime: parses IDX image data from a raw buffer. For testing without files.
pub extern "C" fn fj_rt_mnist_parse_images_buf(buf_ptr: *const u8, buf_len: i64) -> *mut u8 {
    if buf_ptr.is_null() || buf_len <= 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid buffer pointer
    let data = unsafe { std::slice::from_raw_parts(buf_ptr, buf_len as usize) };
    match parse_idx_images(data) {
        Some(tensor) => Box::into_raw(Box::new(tensor)) as *mut u8,
        None => std::ptr::null_mut(),
    }
}

/// Runtime: parses IDX label data from a raw buffer. For testing without files.
pub extern "C" fn fj_rt_mnist_parse_labels_buf(buf_ptr: *const u8, buf_len: i64) -> *mut u8 {
    if buf_ptr.is_null() || buf_len <= 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid buffer pointer
    let data = unsafe { std::slice::from_raw_parts(buf_ptr, buf_len as usize) };
    match parse_idx_labels(data) {
        Some(tensor) => Box::into_raw(Box::new(tensor)) as *mut u8,
        None => std::ptr::null_mut(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.2 — Stream (channel-backed iterator)
// ═══════════════════════════════════════════════════════════════════════

/// Stream handle: wraps a VecDeque as a pull-based iterator.
struct StreamHandle {
    /// Internal buffer of values.
    buffer: std::collections::VecDeque<i64>,
    /// True when no more items will be added.
    closed: bool,
}

/// Runtime: creates a new empty stream.
pub extern "C" fn fj_rt_stream_new() -> *mut u8 {
    let stream = Box::new(StreamHandle {
        buffer: std::collections::VecDeque::new(),
        closed: false,
    });
    Box::into_raw(stream) as *mut u8
}

/// Runtime: creates a stream from a range [start, end).
pub extern "C" fn fj_rt_stream_from_range(start: i64, end: i64) -> *mut u8 {
    let mut buffer = std::collections::VecDeque::new();
    for i in start..end {
        buffer.push_back(i);
    }
    let stream = Box::new(StreamHandle {
        buffer,
        closed: true, // Range is finite
    });
    Box::into_raw(stream) as *mut u8
}

/// Runtime: pushes a value into the stream buffer.
pub extern "C" fn fj_rt_stream_push(ptr: *mut u8, value: i64) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let stream = unsafe { &mut *(ptr as *mut StreamHandle) };
    stream.buffer.push_back(value);
}

/// Runtime: gets the next value from the stream. Returns the value, or i64::MIN if empty.
pub extern "C" fn fj_rt_stream_next(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return i64::MIN;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let stream = unsafe { &mut *(ptr as *mut StreamHandle) };
    stream.buffer.pop_front().unwrap_or(i64::MIN)
}

/// Runtime: checks if the stream has more items (1=yes, 0=no).
pub extern "C" fn fj_rt_stream_has_next(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let stream = unsafe { &*(ptr as *const StreamHandle) };
    i64::from(!stream.buffer.is_empty())
}

/// Runtime: closes the stream (marks as done, no more pushes).
pub extern "C" fn fj_rt_stream_close(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let stream = unsafe { &mut *(ptr as *mut StreamHandle) };
    stream.closed = true;
}

/// Runtime: frees the stream.
pub extern "C" fn fj_rt_stream_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let _ = unsafe { Box::from_raw(ptr as *mut StreamHandle) };
}

// ═══════════════════════════════════════════════════════════════════════
// S12.3 — Stream combinators (map, filter, take)
// ═══════════════════════════════════════════════════════════════════════

/// Runtime: creates a new stream by applying a function to each element.
///
/// The function pointer should have signature `extern "C" fn(i64) -> i64`.
pub extern "C" fn fj_rt_stream_map(ptr: *mut u8, fn_ptr: i64) -> *mut u8 {
    if ptr.is_null() || fn_ptr == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid StreamHandle and function pointer
    let source = unsafe { &mut *(ptr as *mut StreamHandle) };
    // SAFETY: fn_ptr is a valid function pointer with extern "C" fn(i64) -> i64 signature
    let map_fn: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(fn_ptr as usize) };

    let mut new_buffer = std::collections::VecDeque::new();
    while let Some(val) = source.buffer.pop_front() {
        new_buffer.push_back(map_fn(val));
    }
    let mapped = Box::new(StreamHandle {
        buffer: new_buffer,
        closed: source.closed,
    });
    Box::into_raw(mapped) as *mut u8
}

/// Runtime: creates a new stream with only elements passing the predicate.
///
/// The function pointer should have signature `extern "C" fn(i64) -> i64` (returns 0/1).
pub extern "C" fn fj_rt_stream_filter(ptr: *mut u8, fn_ptr: i64) -> *mut u8 {
    if ptr.is_null() || fn_ptr == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid StreamHandle and function pointer
    let source = unsafe { &mut *(ptr as *mut StreamHandle) };
    // SAFETY: fn_ptr is a valid function pointer
    let filter_fn: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(fn_ptr as usize) };

    let mut new_buffer = std::collections::VecDeque::new();
    while let Some(val) = source.buffer.pop_front() {
        if filter_fn(val) != 0 {
            new_buffer.push_back(val);
        }
    }
    let filtered = Box::new(StreamHandle {
        buffer: new_buffer,
        closed: source.closed,
    });
    Box::into_raw(filtered) as *mut u8
}

/// Runtime: creates a new stream with at most N items from the source.
pub extern "C" fn fj_rt_stream_take(ptr: *mut u8, n: i64) -> *mut u8 {
    if ptr.is_null() || n <= 0 {
        return fj_rt_stream_new(); // Empty stream
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let source = unsafe { &mut *(ptr as *mut StreamHandle) };
    let take_count = n as usize;
    let mut new_buffer = std::collections::VecDeque::new();
    for _ in 0..take_count {
        match source.buffer.pop_front() {
            Some(val) => new_buffer.push_back(val),
            None => break,
        }
    }
    let taken = Box::new(StreamHandle {
        buffer: new_buffer,
        closed: true,
    });
    Box::into_raw(taken) as *mut u8
}

/// Runtime: collects stream into a sum (convenience for testing).
pub extern "C" fn fj_rt_stream_sum(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let stream = unsafe { &mut *(ptr as *mut StreamHandle) };
    let mut sum = 0i64;
    while let Some(val) = stream.buffer.pop_front() {
        sum += val;
    }
    sum
}

/// Runtime: counts remaining items in the stream.
pub extern "C" fn fj_rt_stream_count(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid StreamHandle pointer
    let stream = unsafe { &*(ptr as *const StreamHandle) };
    stream.buffer.len() as i64
}

// ═══════════════════════════════════════════════════════════════════════
// S40 — SIMD vector types and operations
// ═══════════════════════════════════════════════════════════════════════

/// SIMD f32x4: 4 lanes of f32.
struct SimdF32x4 {
    lanes: [f32; 4],
}

/// SIMD f32x8: 8 lanes of f32.
struct SimdF32x8 {
    lanes: [f32; 8],
}

/// SIMD i32x4: 4 lanes of i32.
struct SimdI32x4 {
    lanes: [i32; 4],
}

/// SIMD i32x8: 8 lanes of i32.
struct SimdI32x8 {
    lanes: [i32; 8],
}

// ── f32x4 constructors ──────────────────────────────────────────────

/// Creates a new f32x4 from 4 f64 values (f64 bits as i64, truncated to f32).
pub extern "C" fn fj_rt_simd_f32x4_new(a: i64, b: i64, c: i64, d: i64) -> *mut u8 {
    let v = Box::new(SimdF32x4 {
        lanes: [
            f64::from_bits(a as u64) as f32,
            f64::from_bits(b as u64) as f32,
            f64::from_bits(c as u64) as f32,
            f64::from_bits(d as u64) as f32,
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// Creates a f32x4 with all lanes set to the same value (splat).
pub extern "C" fn fj_rt_simd_f32x4_splat(val: i64) -> *mut u8 {
    let f = f64::from_bits(val as u64) as f32;
    let v = Box::new(SimdF32x4 {
        lanes: [f, f, f, f],
    });
    Box::into_raw(v) as *mut u8
}

/// Creates a f32x4 with all zeros.
pub extern "C" fn fj_rt_simd_f32x4_zeros() -> *mut u8 {
    let v = Box::new(SimdF32x4 { lanes: [0.0; 4] });
    Box::into_raw(v) as *mut u8
}

/// Frees a f32x4 handle.
pub extern "C" fn fj_rt_simd_f32x4_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid SimdF32x4 pointer
        let _ = unsafe { Box::from_raw(ptr as *mut SimdF32x4) };
    }
}

/// Gets a lane value from f32x4 (returns as f64 bits in i64).
pub extern "C" fn fj_rt_simd_f32x4_get(ptr: *mut u8, idx: i64) -> i64 {
    if ptr.is_null() || !(0..4).contains(&idx) {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointer
    let v = unsafe { &*(ptr as *const SimdF32x4) };
    let f = v.lanes[idx as usize] as f64;
    f.to_bits() as i64
}

// ── f32x4 arithmetic (lane-wise) ────────────────────────────────────

/// f32x4 lane-wise addition.
pub extern "C" fn fj_rt_simd_f32x4_add(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointers
    let va = unsafe { &*(a as *const SimdF32x4) };
    let vb = unsafe { &*(b as *const SimdF32x4) };
    let v = Box::new(SimdF32x4 {
        lanes: [
            va.lanes[0] + vb.lanes[0],
            va.lanes[1] + vb.lanes[1],
            va.lanes[2] + vb.lanes[2],
            va.lanes[3] + vb.lanes[3],
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// f32x4 lane-wise subtraction.
pub extern "C" fn fj_rt_simd_f32x4_sub(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointers
    let va = unsafe { &*(a as *const SimdF32x4) };
    let vb = unsafe { &*(b as *const SimdF32x4) };
    let v = Box::new(SimdF32x4 {
        lanes: [
            va.lanes[0] - vb.lanes[0],
            va.lanes[1] - vb.lanes[1],
            va.lanes[2] - vb.lanes[2],
            va.lanes[3] - vb.lanes[3],
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// f32x4 lane-wise multiplication.
pub extern "C" fn fj_rt_simd_f32x4_mul(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointers
    let va = unsafe { &*(a as *const SimdF32x4) };
    let vb = unsafe { &*(b as *const SimdF32x4) };
    let v = Box::new(SimdF32x4 {
        lanes: [
            va.lanes[0] * vb.lanes[0],
            va.lanes[1] * vb.lanes[1],
            va.lanes[2] * vb.lanes[2],
            va.lanes[3] * vb.lanes[3],
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// f32x4 lane-wise division.
pub extern "C" fn fj_rt_simd_f32x4_div(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointers
    let va = unsafe { &*(a as *const SimdF32x4) };
    let vb = unsafe { &*(b as *const SimdF32x4) };
    let v = Box::new(SimdF32x4 {
        lanes: [
            va.lanes[0] / vb.lanes[0],
            va.lanes[1] / vb.lanes[1],
            va.lanes[2] / vb.lanes[2],
            va.lanes[3] / vb.lanes[3],
        ],
    });
    Box::into_raw(v) as *mut u8
}

// ── f32x4 horizontal ops ────────────────────────────────────────────

/// f32x4 horizontal sum: returns sum of all 4 lanes as f64 bits in i64.
pub extern "C" fn fj_rt_simd_f32x4_sum(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointer
    let v = unsafe { &*(ptr as *const SimdF32x4) };
    let s: f64 = v.lanes.iter().map(|&x| x as f64).sum();
    s.to_bits() as i64
}

/// f32x4 horizontal min: returns minimum lane as f64 bits in i64.
pub extern "C" fn fj_rt_simd_f32x4_min(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointer
    let v = unsafe { &*(ptr as *const SimdF32x4) };
    let m = v.lanes.iter().copied().fold(f32::INFINITY, f32::min) as f64;
    m.to_bits() as i64
}

/// f32x4 horizontal max: returns maximum lane as f64 bits in i64.
pub extern "C" fn fj_rt_simd_f32x4_max(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdF32x4 pointer
    let v = unsafe { &*(ptr as *const SimdF32x4) };
    let m = v.lanes.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64;
    m.to_bits() as i64
}

// ── i32x4 constructors ──────────────────────────────────────────────

/// Creates a new i32x4 from 4 i64 values (truncated to i32).
pub extern "C" fn fj_rt_simd_i32x4_new(a: i64, b: i64, c: i64, d: i64) -> *mut u8 {
    let v = Box::new(SimdI32x4 {
        lanes: [a as i32, b as i32, c as i32, d as i32],
    });
    Box::into_raw(v) as *mut u8
}

/// Creates an i32x4 with all lanes set to the same value (splat).
pub extern "C" fn fj_rt_simd_i32x4_splat(val: i64) -> *mut u8 {
    let i = val as i32;
    let v = Box::new(SimdI32x4 {
        lanes: [i, i, i, i],
    });
    Box::into_raw(v) as *mut u8
}

/// Frees an i32x4 handle.
pub extern "C" fn fj_rt_simd_i32x4_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid SimdI32x4 pointer
        let _ = unsafe { Box::from_raw(ptr as *mut SimdI32x4) };
    }
}

/// Gets a lane value from i32x4.
pub extern "C" fn fj_rt_simd_i32x4_get(ptr: *mut u8, idx: i64) -> i64 {
    if ptr.is_null() || !(0..4).contains(&idx) {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointer
    let v = unsafe { &*(ptr as *const SimdI32x4) };
    v.lanes[idx as usize] as i64
}

// ── i32x4 arithmetic (lane-wise) ────────────────────────────────────

/// i32x4 lane-wise addition.
pub extern "C" fn fj_rt_simd_i32x4_add(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointers
    let va = unsafe { &*(a as *const SimdI32x4) };
    let vb = unsafe { &*(b as *const SimdI32x4) };
    let v = Box::new(SimdI32x4 {
        lanes: [
            va.lanes[0].wrapping_add(vb.lanes[0]),
            va.lanes[1].wrapping_add(vb.lanes[1]),
            va.lanes[2].wrapping_add(vb.lanes[2]),
            va.lanes[3].wrapping_add(vb.lanes[3]),
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// i32x4 lane-wise subtraction.
pub extern "C" fn fj_rt_simd_i32x4_sub(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointers
    let va = unsafe { &*(a as *const SimdI32x4) };
    let vb = unsafe { &*(b as *const SimdI32x4) };
    let v = Box::new(SimdI32x4 {
        lanes: [
            va.lanes[0].wrapping_sub(vb.lanes[0]),
            va.lanes[1].wrapping_sub(vb.lanes[1]),
            va.lanes[2].wrapping_sub(vb.lanes[2]),
            va.lanes[3].wrapping_sub(vb.lanes[3]),
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// i32x4 lane-wise multiplication.
pub extern "C" fn fj_rt_simd_i32x4_mul(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointers
    let va = unsafe { &*(a as *const SimdI32x4) };
    let vb = unsafe { &*(b as *const SimdI32x4) };
    let v = Box::new(SimdI32x4 {
        lanes: [
            va.lanes[0].wrapping_mul(vb.lanes[0]),
            va.lanes[1].wrapping_mul(vb.lanes[1]),
            va.lanes[2].wrapping_mul(vb.lanes[2]),
            va.lanes[3].wrapping_mul(vb.lanes[3]),
        ],
    });
    Box::into_raw(v) as *mut u8
}

// ── i32x4 horizontal ops ────────────────────────────────────────────

/// i32x4 horizontal sum: returns sum of all 4 lanes.
pub extern "C" fn fj_rt_simd_i32x4_sum(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointer
    let v = unsafe { &*(ptr as *const SimdI32x4) };
    v.lanes.iter().map(|&x| x as i64).sum()
}

/// i32x4 horizontal min.
pub extern "C" fn fj_rt_simd_i32x4_min(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointer
    let v = unsafe { &*(ptr as *const SimdI32x4) };
    v.lanes.iter().copied().min().unwrap_or(0) as i64
}

/// i32x4 horizontal max.
pub extern "C" fn fj_rt_simd_i32x4_max(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdI32x4 pointer
    let v = unsafe { &*(ptr as *const SimdI32x4) };
    v.lanes.iter().copied().max().unwrap_or(0) as i64
}

// ── f32x8 constructors ──────────────────────────────────────────────

/// Creates a f32x8 with all lanes set to the same value (splat).
pub extern "C" fn fj_rt_simd_f32x8_splat(val: i64) -> *mut u8 {
    let f = f64::from_bits(val as u64) as f32;
    let v = Box::new(SimdF32x8 {
        lanes: [f, f, f, f, f, f, f, f],
    });
    Box::into_raw(v) as *mut u8
}

/// Frees a f32x8 handle.
pub extern "C" fn fj_rt_simd_f32x8_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid SimdF32x8 pointer
        let _ = unsafe { Box::from_raw(ptr as *mut SimdF32x8) };
    }
}

/// Gets a lane value from f32x8.
pub extern "C" fn fj_rt_simd_f32x8_get(ptr: *mut u8, idx: i64) -> i64 {
    if ptr.is_null() || !(0..8).contains(&idx) {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdF32x8 pointer
    let v = unsafe { &*(ptr as *const SimdF32x8) };
    let f = v.lanes[idx as usize] as f64;
    f.to_bits() as i64
}

/// f32x8 lane-wise addition.
pub extern "C" fn fj_rt_simd_f32x8_add(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdF32x8 pointers
    let va = unsafe { &*(a as *const SimdF32x8) };
    let vb = unsafe { &*(b as *const SimdF32x8) };
    let mut lanes = [0.0f32; 8];
    for (i, lane) in lanes.iter_mut().enumerate() {
        *lane = va.lanes[i] + vb.lanes[i];
    }
    Box::into_raw(Box::new(SimdF32x8 { lanes })) as *mut u8
}

/// f32x8 lane-wise multiplication.
pub extern "C" fn fj_rt_simd_f32x8_mul(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdF32x8 pointers
    let va = unsafe { &*(a as *const SimdF32x8) };
    let vb = unsafe { &*(b as *const SimdF32x8) };
    let mut lanes = [0.0f32; 8];
    for (i, lane) in lanes.iter_mut().enumerate() {
        *lane = va.lanes[i] * vb.lanes[i];
    }
    Box::into_raw(Box::new(SimdF32x8 { lanes })) as *mut u8
}

/// f32x8 horizontal sum.
pub extern "C" fn fj_rt_simd_f32x8_sum(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdF32x8 pointer
    let v = unsafe { &*(ptr as *const SimdF32x8) };
    let s: f64 = v.lanes.iter().map(|&x| x as f64).sum();
    s.to_bits() as i64
}

// ── i32x8 constructors ──────────────────────────────────────────────

/// Creates an i32x8 with all lanes set to the same value (splat).
pub extern "C" fn fj_rt_simd_i32x8_splat(val: i64) -> *mut u8 {
    let i = val as i32;
    let v = Box::new(SimdI32x8 {
        lanes: [i, i, i, i, i, i, i, i],
    });
    Box::into_raw(v) as *mut u8
}

/// Frees an i32x8 handle.
pub extern "C" fn fj_rt_simd_i32x8_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid SimdI32x8 pointer
        let _ = unsafe { Box::from_raw(ptr as *mut SimdI32x8) };
    }
}

/// Gets a lane value from i32x8.
pub extern "C" fn fj_rt_simd_i32x8_get(ptr: *mut u8, idx: i64) -> i64 {
    if ptr.is_null() || !(0..8).contains(&idx) {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdI32x8 pointer
    let v = unsafe { &*(ptr as *const SimdI32x8) };
    v.lanes[idx as usize] as i64
}

/// i32x8 lane-wise addition.
pub extern "C" fn fj_rt_simd_i32x8_add(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdI32x8 pointers
    let va = unsafe { &*(a as *const SimdI32x8) };
    let vb = unsafe { &*(b as *const SimdI32x8) };
    let mut lanes = [0i32; 8];
    for (i, lane) in lanes.iter_mut().enumerate() {
        *lane = va.lanes[i].wrapping_add(vb.lanes[i]);
    }
    Box::into_raw(Box::new(SimdI32x8 { lanes })) as *mut u8
}

/// i32x8 lane-wise multiplication.
pub extern "C" fn fj_rt_simd_i32x8_mul(a: *mut u8, b: *mut u8) -> *mut u8 {
    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid SimdI32x8 pointers
    let va = unsafe { &*(a as *const SimdI32x8) };
    let vb = unsafe { &*(b as *const SimdI32x8) };
    let mut lanes = [0i32; 8];
    for (i, lane) in lanes.iter_mut().enumerate() {
        *lane = va.lanes[i].wrapping_mul(vb.lanes[i]);
    }
    Box::into_raw(Box::new(SimdI32x8 { lanes })) as *mut u8
}

/// i32x8 horizontal sum.
pub extern "C" fn fj_rt_simd_i32x8_sum(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid SimdI32x8 pointer
    let v = unsafe { &*(ptr as *const SimdI32x8) };
    v.lanes.iter().map(|&x| x as i64).sum()
}

// ── SIMD load/store ─────────────────────────────────────────────────

/// Loads f32x4 from an array handle at offset (4 consecutive f64-as-i64 values).
pub extern "C" fn fj_rt_simd_f32x4_load(arr_ptr: *mut u8, offset: i64) -> *mut u8 {
    if arr_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: arr_ptr is a heap array (Vec<i64>), offset is the starting index
    let arr = unsafe { &*(arr_ptr as *const Vec<i64>) };
    let off = offset as usize;
    if off + 4 > arr.len() {
        return fj_rt_simd_f32x4_zeros();
    }
    let v = Box::new(SimdF32x4 {
        lanes: [
            f64::from_bits(arr[off] as u64) as f32,
            f64::from_bits(arr[off + 1] as u64) as f32,
            f64::from_bits(arr[off + 2] as u64) as f32,
            f64::from_bits(arr[off + 3] as u64) as f32,
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// Stores f32x4 into an array handle at offset (writes 4 values as f64-as-i64).
pub extern "C" fn fj_rt_simd_f32x4_store(vec_ptr: *mut u8, arr_ptr: *mut u8, offset: i64) {
    if vec_ptr.is_null() || arr_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid pointers
    let v = unsafe { &*(vec_ptr as *const SimdF32x4) };
    let arr = unsafe { &mut *(arr_ptr as *mut Vec<i64>) };
    let off = offset as usize;
    if off + 4 > arr.len() {
        return;
    }
    for i in 0..4 {
        arr[off + i] = (v.lanes[i] as f64).to_bits() as i64;
    }
}

/// Loads i32x4 from an array handle at offset (4 consecutive i64 values truncated to i32).
pub extern "C" fn fj_rt_simd_i32x4_load(arr_ptr: *mut u8, offset: i64) -> *mut u8 {
    if arr_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: arr_ptr is a heap array (Vec<i64>)
    let arr = unsafe { &*(arr_ptr as *const Vec<i64>) };
    let off = offset as usize;
    if off + 4 > arr.len() {
        return fj_rt_simd_i32x4_splat(0);
    }
    let v = Box::new(SimdI32x4 {
        lanes: [
            arr[off] as i32,
            arr[off + 1] as i32,
            arr[off + 2] as i32,
            arr[off + 3] as i32,
        ],
    });
    Box::into_raw(v) as *mut u8
}

/// Stores i32x4 into an array handle at offset (writes 4 values as i64).
pub extern "C" fn fj_rt_simd_i32x4_store(vec_ptr: *mut u8, arr_ptr: *mut u8, offset: i64) {
    if vec_ptr.is_null() || arr_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid pointers
    let v = unsafe { &*(vec_ptr as *const SimdI32x4) };
    let arr = unsafe { &mut *(arr_ptr as *mut Vec<i64>) };
    let off = offset as usize;
    if off + 4 > arr.len() {
        return;
    }
    for i in 0..4 {
        arr[off + i] = v.lanes[i] as i64;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S35 — ONNX Export (minimal protobuf writer)
// ═══════════════════════════════════════════════════════════════════════

/// Minimal protobuf wire format encoder for ONNX export.
mod onnx_proto {
    /// Encodes a varint (variable-length unsigned integer).
    pub fn encode_varint(buf: &mut Vec<u8>, mut val: u64) {
        loop {
            let byte = (val & 0x7F) as u8;
            val >>= 7;
            if val == 0 {
                buf.push(byte);
                break;
            }
            buf.push(byte | 0x80);
        }
    }

    /// Encodes a protobuf field tag (field_number << 3 | wire_type).
    pub fn encode_tag(buf: &mut Vec<u8>, field: u32, wire_type: u8) {
        encode_varint(buf, ((field as u64) << 3) | wire_type as u64);
    }

    /// Encodes a length-delimited bytes field.
    pub fn encode_bytes(buf: &mut Vec<u8>, field: u32, data: &[u8]) {
        encode_tag(buf, field, 2);
        encode_varint(buf, data.len() as u64);
        buf.extend_from_slice(data);
    }

    /// Encodes a string field.
    pub fn encode_string(buf: &mut Vec<u8>, field: u32, s: &str) {
        encode_bytes(buf, field, s.as_bytes());
    }

    /// Encodes a varint field.
    pub fn encode_varint_field(buf: &mut Vec<u8>, field: u32, val: u64) {
        encode_tag(buf, field, 0);
        encode_varint(buf, val);
    }

    /// Encodes a submessage field.
    pub fn encode_submessage(buf: &mut Vec<u8>, field: u32, inner: &[u8]) {
        encode_bytes(buf, field, inner);
    }

    /// ONNX TensorProto data types.
    pub const FLOAT: i32 = 1;

    /// Encodes a TensorProto (initializer weight tensor).
    pub fn encode_tensor_proto(name: &str, dims: &[i64], data: &[f32]) -> Vec<u8> {
        let mut buf = Vec::new();
        // dims: repeated int64, field 1
        for &d in dims {
            encode_varint_field(&mut buf, 1, d as u64);
        }
        // data_type: int32, field 2
        encode_varint_field(&mut buf, 2, FLOAT as u64);
        // float_data: packed repeated float, field 4
        {
            let mut packed = Vec::with_capacity(data.len() * 4);
            for &f in data {
                packed.extend_from_slice(&f.to_le_bytes());
            }
            encode_bytes(&mut buf, 4, &packed);
        }
        // name: string, field 8
        encode_string(&mut buf, 8, name);
        buf
    }

    /// Encodes a NodeProto (graph operation).
    pub fn encode_node_proto(
        inputs: &[&str],
        outputs: &[&str],
        op_type: &str,
        name: &str,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        // input: repeated string, field 1
        for inp in inputs {
            encode_string(&mut buf, 1, inp);
        }
        // output: repeated string, field 2
        for out in outputs {
            encode_string(&mut buf, 2, out);
        }
        // name: string, field 3
        encode_string(&mut buf, 3, name);
        // op_type: string, field 4
        encode_string(&mut buf, 4, op_type);
        buf
    }

    /// Encodes a ValueInfoProto (input/output spec with shape).
    pub fn encode_value_info(name: &str, dims: &[i64]) -> Vec<u8> {
        let mut buf = Vec::new();
        // name: string, field 1
        encode_string(&mut buf, 1, name);
        // type: TypeProto (field 2) — TensorType (field 1 in TypeProto)
        let mut tensor_type = Vec::new();
        // elem_type: int32, field 1 in TensorTypeProto
        encode_varint_field(&mut tensor_type, 1, FLOAT as u64);
        // shape: TensorShapeProto, field 2 in TensorTypeProto
        let mut shape = Vec::new();
        for &d in dims {
            // dim: repeated Dimension, field 1 in TensorShapeProto
            let mut dim_msg = Vec::new();
            // dim_value: int64, field 1 in Dimension
            encode_varint_field(&mut dim_msg, 1, d as u64);
            encode_submessage(&mut shape, 1, &dim_msg);
        }
        encode_submessage(&mut tensor_type, 2, &shape);
        // Wrap in TypeProto
        let mut type_proto = Vec::new();
        encode_submessage(&mut type_proto, 1, &tensor_type);
        encode_submessage(&mut buf, 2, &type_proto);
        buf
    }

    /// Encodes a GraphProto.
    pub fn encode_graph_proto(
        name: &str,
        nodes: &[Vec<u8>],
        initializers: &[Vec<u8>],
        inputs: &[Vec<u8>],
        outputs: &[Vec<u8>],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        // node: repeated NodeProto, field 1
        for node in nodes {
            encode_submessage(&mut buf, 1, node);
        }
        // name: string, field 2
        encode_string(&mut buf, 2, name);
        // initializer: repeated TensorProto, field 5
        for init in initializers {
            encode_submessage(&mut buf, 5, init);
        }
        // input: repeated ValueInfoProto, field 11
        for inp in inputs {
            encode_submessage(&mut buf, 11, inp);
        }
        // output: repeated ValueInfoProto, field 12
        for out in outputs {
            encode_submessage(&mut buf, 12, out);
        }
        buf
    }

    /// Encodes a full ModelProto.
    pub fn encode_model_proto(graph: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        // ir_version: int64, field 1
        encode_varint_field(&mut buf, 1, 7); // ONNX IR version 7
                                             // opset_import: repeated OperatorSetIdProto, field 8
        {
            let mut opset = Vec::new();
            // version: int64, field 2
            encode_varint_field(&mut opset, 2, 13); // opset 13
            encode_submessage(&mut buf, 8, &opset);
        }
        // graph: GraphProto, field 7
        encode_submessage(&mut buf, 7, graph);
        buf
    }
}

/// ONNX model builder handle.
struct OnnxModelHandle {
    /// Node protobuf encodings.
    nodes: Vec<Vec<u8>>,
    /// Initializer (weight) tensor protobuf encodings.
    initializers: Vec<Vec<u8>>,
    /// Input ValueInfoProto encodings.
    inputs: Vec<Vec<u8>>,
    /// Output ValueInfoProto encodings.
    outputs: Vec<Vec<u8>>,
    /// Graph name.
    graph_name: String,
}

/// Creates a new ONNX model builder.
pub extern "C" fn fj_rt_onnx_new() -> *mut u8 {
    let model = Box::new(OnnxModelHandle {
        nodes: Vec::new(),
        initializers: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        graph_name: "fajar_model".to_string(),
    });
    Box::into_raw(model) as *mut u8
}

/// Adds a Dense (MatMul + Add) layer to the ONNX model.
///
/// weight_ptr: TensorHandle for weight matrix (rows × cols f64 values).
/// bias_ptr: TensorHandle for bias vector.
/// layer_idx: unique layer index for naming.
pub extern "C" fn fj_rt_onnx_add_dense(
    model_ptr: *mut u8,
    weight_ptr: *mut u8,
    bias_ptr: *mut u8,
    layer_idx: i64,
) {
    if model_ptr.is_null() || weight_ptr.is_null() || bias_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid handles
    let model = unsafe { &mut *(model_ptr as *mut OnnxModelHandle) };
    let weight_tensor = unsafe { &*(weight_ptr as *const ndarray::Array2<f64>) };
    let bias_tensor = unsafe { &*(bias_ptr as *const ndarray::Array2<f64>) };

    let idx = layer_idx as usize;
    let input_name = if idx == 0 {
        "input".to_string()
    } else {
        format!("dense_{}_out", idx - 1)
    };
    let matmul_out = format!("dense_{}_matmul", idx);
    let output_name = format!("dense_{}_out", idx);
    let weight_name = format!("dense_{}_weight", idx);
    let bias_name = format!("dense_{}_bias", idx);

    let (rows, cols) = weight_tensor.dim();

    // Weight initializer
    let weight_data: Vec<f32> = weight_tensor.iter().map(|&v| v as f32).collect();
    let weight_proto =
        onnx_proto::encode_tensor_proto(&weight_name, &[rows as i64, cols as i64], &weight_data);
    model.initializers.push(weight_proto);

    // Bias initializer
    let bias_data: Vec<f32> = bias_tensor.iter().map(|&v| v as f32).collect();
    let bias_cols = bias_tensor.ncols();
    let bias_proto = onnx_proto::encode_tensor_proto(&bias_name, &[bias_cols as i64], &bias_data);
    model.initializers.push(bias_proto);

    // MatMul node
    let matmul_node = onnx_proto::encode_node_proto(
        &[&input_name, &weight_name],
        &[&matmul_out],
        "MatMul",
        &format!("matmul_{idx}"),
    );
    model.nodes.push(matmul_node);

    // Add node
    let add_node = onnx_proto::encode_node_proto(
        &[&matmul_out, &bias_name],
        &[&output_name],
        "Add",
        &format!("add_{idx}"),
    );
    model.nodes.push(add_node);
}

/// Adds a Relu activation node.
pub extern "C" fn fj_rt_onnx_add_relu(model_ptr: *mut u8, layer_idx: i64) {
    if model_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid handle
    let model = unsafe { &mut *(model_ptr as *mut OnnxModelHandle) };
    let idx = layer_idx as usize;
    let input_name = format!("dense_{}_out", idx);
    let output_name = format!("relu_{}_out", idx);

    let node = onnx_proto::encode_node_proto(
        &[&input_name],
        &[&output_name],
        "Relu",
        &format!("relu_{idx}"),
    );
    model.nodes.push(node);
}

/// Sets graph input shape (batch_size × features).
pub extern "C" fn fj_rt_onnx_set_input(model_ptr: *mut u8, batch: i64, features: i64) {
    if model_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid handle
    let model = unsafe { &mut *(model_ptr as *mut OnnxModelHandle) };
    let vi = onnx_proto::encode_value_info("input", &[batch, features]);
    model.inputs.push(vi);
}

/// Sets graph output shape.
pub extern "C" fn fj_rt_onnx_set_output(
    model_ptr: *mut u8,
    name_ptr: *const u8,
    name_len: i64,
    dim0: i64,
    dim1: i64,
) {
    if model_ptr.is_null() || name_ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid pointers
    let model = unsafe { &mut *(model_ptr as *mut OnnxModelHandle) };
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len as usize))
    };
    let vi = onnx_proto::encode_value_info(name, &[dim0, dim1]);
    model.outputs.push(vi);
}

/// Exports the ONNX model to a file. Returns 1 on success, 0 on error.
pub extern "C" fn fj_rt_onnx_export(model_ptr: *mut u8, path_ptr: *const u8, path_len: i64) -> i64 {
    if model_ptr.is_null() || path_ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid pointers
    let model = unsafe { &*(model_ptr as *const OnnxModelHandle) };
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    };

    let graph = onnx_proto::encode_graph_proto(
        &model.graph_name,
        &model.nodes,
        &model.initializers,
        &model.inputs,
        &model.outputs,
    );
    let model_proto = onnx_proto::encode_model_proto(&graph);

    match std::fs::write(path, &model_proto) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Frees an ONNX model builder handle.
pub extern "C" fn fj_rt_onnx_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid OnnxModelHandle pointer
        let _ = unsafe { Box::from_raw(ptr as *mut OnnxModelHandle) };
    }
}

/// Returns the number of nodes in the ONNX model (for testing).
pub extern "C" fn fj_rt_onnx_node_count(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid handle
    let model = unsafe { &*(ptr as *const OnnxModelHandle) };
    model.nodes.len() as i64
}

/// Returns the number of initializers (weight tensors) in the ONNX model.
pub extern "C" fn fj_rt_onnx_initializer_count(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid handle
    let model = unsafe { &*(ptr as *const OnnxModelHandle) };
    model.initializers.len() as i64
}

// ═══════════════════════════════════════════════════════════════════════
// Mixed Precision Runtime (S39)
// ═══════════════════════════════════════════════════════════════════════

/// Converts f32 to f16 (half precision) using IEEE 754 truncation.
///
/// Input: f32 bits as i64. Output: f16 bits as i64.
pub extern "C" fn fj_rt_f32_to_f16(bits: i64) -> i64 {
    let f = f32::from_bits(bits as u32);
    // Simple truncation: extract sign, exponent, mantissa
    let b = bits as u32;
    let sign = (b >> 31) & 1;
    let exp = ((b >> 23) & 0xFF) as i32;
    let mant = b & 0x7F_FFFF;

    let (h_exp, h_mant) = if exp == 0 {
        // Zero / subnormal → f16 zero
        (0u16, 0u16)
    } else if exp == 0xFF {
        // Inf/NaN
        if mant == 0 {
            (0x1F, 0) // Inf
        } else {
            (0x1F, 0x200) // NaN
        }
    } else {
        let new_exp = exp - 127 + 15;
        if new_exp <= 0 {
            (0, 0) // Underflow → zero
        } else if new_exp >= 31 {
            (0x1F, 0) // Overflow → Inf
        } else {
            (new_exp as u16, (mant >> 13) as u16)
        }
    };
    let _ = f; // suppress unused warning
    let h = ((sign as u16) << 15) | (h_exp << 10) | h_mant;
    h as i64
}

/// Converts f16 bits back to f32 bits.
///
/// Input: f16 bits as i64. Output: f32 bits as i64.
pub extern "C" fn fj_rt_f16_to_f32(bits: i64) -> i64 {
    let h = bits as u16;
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;

    let (f_exp, f_mant) = if exp == 0 {
        if mant == 0 {
            (0u32, 0u32) // Zero
        } else {
            // Subnormal → normalize
            let mut m = mant;
            let mut e = 0i32;
            while (m & 0x400) == 0 {
                m <<= 1;
                e -= 1;
            }
            let f_exp = (127 - 15 + 1 + e) as u32;
            let f_mant = (m & 0x3FF) << 13;
            (f_exp, f_mant)
        }
    } else if exp == 0x1F {
        if mant == 0 {
            (0xFF, 0) // Inf
        } else {
            (0xFF, mant << 13) // NaN
        }
    } else {
        let f_exp = exp - 15 + 127;
        let f_mant = mant << 13;
        (f_exp, f_mant)
    };

    let f_bits = (sign << 31) | (f_exp << 23) | f_mant;
    f_bits as i64
}

/// Converts a tensor from f64 to f16 (truncating each element).
///
/// Returns a new tensor where each f64 value is converted: f64 → f32 → f16 → stored as f64.
pub extern "C" fn fj_rt_tensor_to_f16(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = Box::new(t.mapv(|x| {
        // f64 → f32 → f16 bits → f32 → f64 (lossy round-trip simulating f16 precision)
        let f = x as f32;
        let bits = f.to_bits();
        let sign = (bits >> 31) & 1;
        let exp = ((bits >> 23) & 0xFF) as i32;
        let mant = bits & 0x7F_FFFF;
        let (h_exp, h_mant) = if exp == 0 || exp - 127 + 15 <= 0 {
            (0u16, 0u16)
        } else if exp == 0xFF || exp - 127 + 15 >= 31 {
            if mant == 0 {
                (0x1F, 0)
            } else {
                (0x1F, 0x200)
            }
        } else {
            ((exp - 127 + 15) as u16, (mant >> 13) as u16)
        };
        let h = ((sign as u16) << 15) | (h_exp << 10) | h_mant;
        // Convert back to f64 via f32
        let s32 = ((h >> 15) as u32) << 31;
        let e16 = ((h >> 10) & 0x1F) as u32;
        let m16 = (h & 0x3FF) as u32;
        let f32_bits = if e16 == 0 {
            s32
        } else if e16 == 0x1F {
            s32 | 0x7F80_0000 | (m16 << 13)
        } else {
            s32 | ((e16 - 15 + 127) << 23) | (m16 << 13)
        };
        f32::from_bits(f32_bits) as f64
    }));
    Box::into_raw(result) as *mut u8
}

// ═══════════════════════════════════════════════════════════════════════
// Loss Scaling & Post-Training Quantization (S39.3-S39.4)
// ═══════════════════════════════════════════════════════════════════════

/// Scales a tensor by a scalar factor (for loss scaling in mixed-precision training).
/// Returns a new tensor where every element is multiplied by `scale`.
pub extern "C" fn fj_rt_loss_scale(ptr: *mut u8, scale: f64) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = Box::new(t.mapv(|x| x * scale));
    Box::into_raw(result) as *mut u8
}

/// Unscales a tensor by dividing every element by `scale`.
/// Used after backward pass to restore gradient magnitudes.
pub extern "C" fn fj_rt_loss_unscale(ptr: *mut u8, scale: f64) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = if scale == 0.0 {
        Box::new(t.clone())
    } else {
        Box::new(t.mapv(|x| x / scale))
    };
    Box::into_raw(result) as *mut u8
}

/// Quantizes a f64 tensor to int8 using min/max affine quantization.
/// Returns a new tensor with values in [-128, 127] range (stored as f64).
/// Also stores the scale and zero_point as the first two elements of a
/// separate 1D metadata tensor, returned as a packed pair:
/// returns (quantized_tensor_ptr, meta_tensor_ptr) packed as i128 → use high/low i64.
/// For simplicity, returns the quantized tensor; scale/zero_point can be
/// retrieved via `fj_rt_tensor_quant_params`.
pub extern "C" fn fj_rt_tensor_quantize_int8(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let min_val = t.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = t.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let scale = if (max_val - min_val).abs() < 1e-12 {
        1.0
    } else {
        (max_val - min_val) / 255.0
    };
    let zero_point = (-128.0 - min_val / scale).round().clamp(-128.0, 127.0);
    let quantized = Box::new(t.mapv(|x| (x / scale + zero_point).round().clamp(-128.0, 127.0)));
    // Store scale and zero_point as globals for retrieval via fj_rt_tensor_quant_params
    LAST_QUANT_SCALE.store(scale.to_bits(), std::sync::atomic::Ordering::SeqCst);
    LAST_QUANT_ZERO.store(zero_point.to_bits(), std::sync::atomic::Ordering::SeqCst);
    Box::into_raw(quantized) as *mut u8
}

static LAST_QUANT_SCALE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LAST_QUANT_ZERO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Returns the scale factor from the last `fj_rt_tensor_quantize_int8` call.
pub extern "C" fn fj_rt_tensor_quant_scale() -> f64 {
    f64::from_bits(LAST_QUANT_SCALE.load(std::sync::atomic::Ordering::SeqCst))
}

/// Returns the zero point from the last `fj_rt_tensor_quantize_int8` call.
pub extern "C" fn fj_rt_tensor_quant_zero_point() -> f64 {
    f64::from_bits(LAST_QUANT_ZERO.load(std::sync::atomic::Ordering::SeqCst))
}

/// Dequantizes an int8 tensor back to f64 using `value = (q - zero_point) * scale`.
pub extern "C" fn fj_rt_tensor_dequantize_int8(
    ptr: *mut u8,
    scale: f64,
    zero_point: f64,
) -> *mut u8 {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid TensorHandle
    let t = unsafe { &*(ptr as *const TensorHandle) };
    let result = Box::new(t.mapv(|q| (q - zero_point) * scale));
    Box::into_raw(result) as *mut u8
}

// ═══════════════════════════════════════════════════════════════════════
// Closure Handles (S2.6) — Returning closures with captured variables
// ═══════════════════════════════════════════════════════════════════════

/// A closure handle bundles a function pointer with captured variable values.
/// The captured values are stored as an i64 array (same ABI as all Fajar Lang values).
struct ClosureHandle {
    /// Raw function pointer to the generated closure body.
    fn_ptr: i64,
    /// Snapshot of captured variable values at closure creation time.
    captures: Vec<i64>,
}

/// Allocates a closure handle with the given function pointer and capture count.
/// Caller must then store captured values via `fj_rt_closure_set_capture`.
///
/// # Safety
///
/// Returns a valid pointer that must be freed with `fj_rt_closure_free`.
pub extern "C" fn fj_rt_closure_new(fn_ptr: i64, capture_count: i64) -> *mut u8 {
    let handle = Box::new(ClosureHandle {
        fn_ptr,
        captures: vec![0i64; capture_count as usize],
    });
    Box::into_raw(handle) as *mut u8
}

/// Sets a captured value in the closure handle at the given index.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_set_capture(ptr: *mut u8, index: i64, value: i64) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &mut *(ptr as *mut ClosureHandle) };
    if (index as usize) < handle.captures.len() {
        handle.captures[index as usize] = value;
    }
}

/// Gets the function pointer from a closure handle.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_get_fn(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &*(ptr as *const ClosureHandle) };
    handle.fn_ptr
}

/// Gets a captured value from a closure handle at the given index.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_get_capture(ptr: *mut u8, index: i64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &*(ptr as *const ClosureHandle) };
    handle.captures.get(index as usize).copied().unwrap_or(0)
}

/// Returns the number of captures in a closure handle.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_capture_count(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &*(ptr as *const ClosureHandle) };
    handle.captures.len() as i64
}

/// Calls a closure handle with 1 user argument.
/// Extracts fn_ptr + captures, builds full args, calls via fn pointer.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
/// The underlying function must accept (captures..., arg) → i64.
pub extern "C" fn fj_rt_closure_call_1(ptr: *mut u8, arg: i64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &*(ptr as *const ClosureHandle) };
    let fn_ptr = handle.fn_ptr;
    let caps = &handle.captures;

    // Call the underlying function with captures prepended
    // We need to use a function pointer cast based on capture count
    match caps.len() {
        0 => {
            let f: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(arg)
        }
        1 => {
            let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0], arg)
        }
        2 => {
            let f: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0], caps[1], arg)
        }
        3 => {
            let f: extern "C" fn(i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0], caps[1], caps[2], arg)
        }
        _ => {
            eprintln!("[closure_call_1] unsupported capture count: {}", caps.len());
            0
        }
    }
}

/// Calls a closure handle with 0 user arguments.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_call_0(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &*(ptr as *const ClosureHandle) };
    let fn_ptr = handle.fn_ptr;
    let caps = &handle.captures;

    match caps.len() {
        0 => {
            let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f()
        }
        1 => {
            let f: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0])
        }
        2 => {
            let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0], caps[1])
        }
        _ => 0,
    }
}

/// Calls a closure handle with 2 user arguments.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_call_2(ptr: *mut u8, arg1: i64, arg2: i64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid ClosureHandle pointer
    let handle = unsafe { &*(ptr as *const ClosureHandle) };
    let fn_ptr = handle.fn_ptr;
    let caps = &handle.captures;

    match caps.len() {
        0 => {
            let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(arg1, arg2)
        }
        1 => {
            let f: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0], arg1, arg2)
        }
        2 => {
            let f: extern "C" fn(i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute(fn_ptr) };
            f(caps[0], caps[1], arg1, arg2)
        }
        _ => 0,
    }
}

/// Frees a closure handle.
///
/// # Safety
///
/// `ptr` must have been produced by `fj_rt_closure_new`.
pub extern "C" fn fj_rt_closure_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid ClosureHandle pointer
        unsafe {
            let _ = Box::from_raw(ptr as *mut ClosureHandle);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Distributed Training Runtime (S34)
// ═══════════════════════════════════════════════════════════════════════

/// Distributed context handle.
struct DistributedContext {
    world_size: i64,
    rank: i64,
}

/// Initializes a distributed context with given world_size and rank.
pub extern "C" fn fj_rt_dist_init(world_size: i64, rank: i64) -> *mut u8 {
    let ctx = Box::new(DistributedContext { world_size, rank });
    Box::into_raw(ctx) as *mut u8
}

/// Returns the world size from a distributed context.
pub extern "C" fn fj_rt_dist_world_size(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees valid handle
    let ctx = unsafe { &*(ptr as *const DistributedContext) };
    ctx.world_size
}

/// Returns the rank from a distributed context.
pub extern "C" fn fj_rt_dist_rank(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees valid handle
    let ctx = unsafe { &*(ptr as *const DistributedContext) };
    ctx.rank
}

/// All-reduce sum: simulates summing tensor across all ranks.
///
/// In single-process mode, this multiplies the tensor by world_size
/// (equivalent to all ranks contributing identical values).
pub extern "C" fn fj_rt_dist_all_reduce_sum(ctx_ptr: *mut u8, tensor_ptr: *mut u8) -> *mut u8 {
    if ctx_ptr.is_null() || tensor_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid handles
    let ctx = unsafe { &*(ctx_ptr as *const DistributedContext) };
    let t = unsafe { &*(tensor_ptr as *const TensorHandle) };
    let scale = ctx.world_size as f64;
    let result = Box::new(t.mapv(|x| x * scale));
    Box::into_raw(result) as *mut u8
}

/// Broadcast: simulates broadcasting tensor from root rank to all.
///
/// In single-process mode, returns a copy of the tensor.
pub extern "C" fn fj_rt_dist_broadcast(
    _ctx_ptr: *mut u8,
    tensor_ptr: *mut u8,
    _root: i64,
) -> *mut u8 {
    if tensor_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid handle
    let t = unsafe { &*(tensor_ptr as *const TensorHandle) };
    let result = Box::new(t.clone());
    Box::into_raw(result) as *mut u8
}

/// Splits a batch tensor across ranks for data parallelism.
/// Given a tensor with N rows, splits into `world_size` chunks and returns the
/// chunk for the given rank. Each chunk has `N / world_size` rows
/// (remainder rows go to the last rank).
///
/// # Safety
///
/// Both pointers must be valid.
pub extern "C" fn fj_rt_dist_split_batch(ctx_ptr: *mut u8, tensor_ptr: *mut u8) -> *mut u8 {
    if ctx_ptr.is_null() || tensor_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid handles
    let ctx = unsafe { &*(ctx_ptr as *const DistributedContext) };
    let t = unsafe { &*(tensor_ptr as *const TensorHandle) };
    let (total_rows, _cols) = t.dim();
    let ws = ctx.world_size.max(1) as usize;
    let rank = ctx.rank.max(0) as usize;
    let chunk_size = total_rows / ws;
    let start = rank * chunk_size;
    let end = if rank == ws - 1 {
        total_rows
    } else {
        start + chunk_size
    };
    let slice = t.slice(ndarray::s![start..end, ..]).to_owned();
    let result = Box::new(slice);
    Box::into_raw(result) as *mut u8
}

/// Frees a distributed context.
pub extern "C" fn fj_rt_dist_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid handle from fj_rt_dist_init
        unsafe {
            drop(Box::from_raw(ptr as *mut DistributedContext));
        }
    }
}

// =====================================================================
// S34.4 — TCP Backend for Gradient Exchange
// =====================================================================

/// TCP exchange handle: holds a bound TCP listener for gradient exchange.
struct TcpExchangeHandle {
    /// Port the listener is bound to.
    port: u16,
    /// Listener for incoming connections.
    listener: std::net::TcpListener,
}

/// Creates a TCP gradient exchange endpoint bound to `127.0.0.1:port`.
/// Returns an opaque handle, or null on failure.
pub extern "C" fn fj_rt_dist_tcp_bind(port: i64) -> *mut u8 {
    let addr = format!("127.0.0.1:{}", port as u16);
    match std::net::TcpListener::bind(&addr) {
        Ok(listener) => {
            // Get actual bound port (important when port=0 for ephemeral)
            let actual_port = listener
                .local_addr()
                .map(|a| a.port())
                .unwrap_or(port as u16);
            let _ = listener.set_nonblocking(false);
            let handle = Box::new(TcpExchangeHandle {
                port: actual_port,
                listener,
            });
            Box::into_raw(handle) as *mut u8
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns the port of a TCP exchange handle.
pub extern "C" fn fj_rt_dist_tcp_port(ptr: *mut u8) -> i64 {
    if ptr.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees valid handle
    let h = unsafe { &*(ptr as *const TcpExchangeHandle) };
    h.port as i64
}

/// Sends a tensor's raw data over TCP to `127.0.0.1:port`.
/// Serializes as: rows(i64) + cols(i64) + data(rows*cols*f64).
/// Returns number of bytes sent, or -1 on error.
pub extern "C" fn fj_rt_dist_tcp_send(port: i64, tensor_ptr: *mut u8) -> i64 {
    use std::io::Write;
    if tensor_ptr.is_null() {
        return -1;
    }
    // SAFETY: caller guarantees valid tensor
    let t = unsafe { &*(tensor_ptr as *const TensorHandle) };
    let (rows, cols) = t.dim();
    let addr = format!("127.0.0.1:{}", port as u16);
    match std::net::TcpStream::connect(&addr) {
        Ok(mut stream) => {
            let _ = stream.write_all(&(rows as i64).to_le_bytes());
            let _ = stream.write_all(&(cols as i64).to_le_bytes());
            for val in t.iter() {
                let _ = stream.write_all(&val.to_le_bytes());
            }
            (16 + rows * cols * 8) as i64
        }
        Err(_) => -1,
    }
}

/// Receives a tensor over TCP on the bound listener.
/// Reads: rows(i64) + cols(i64) + data(rows*cols*f64).
/// Returns a new tensor, or null on error.
pub extern "C" fn fj_rt_dist_tcp_recv(ptr: *mut u8) -> *mut u8 {
    use std::io::Read;
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees valid handle
    let h = unsafe { &*(ptr as *const TcpExchangeHandle) };
    match h.listener.accept() {
        Ok((mut stream, _)) => {
            let mut buf = [0u8; 8];
            if stream.read_exact(&mut buf).is_err() {
                return std::ptr::null_mut();
            }
            let rows = i64::from_le_bytes(buf) as usize;
            if stream.read_exact(&mut buf).is_err() {
                return std::ptr::null_mut();
            }
            let cols = i64::from_le_bytes(buf) as usize;
            let mut data = vec![0.0f64; rows * cols];
            for val in data.iter_mut() {
                if stream.read_exact(&mut buf).is_err() {
                    return std::ptr::null_mut();
                }
                *val = f64::from_le_bytes(buf);
            }
            let arr = Array2::from_shape_vec((rows, cols), data)
                .unwrap_or_else(|_| Array2::zeros((rows, cols)));
            Box::into_raw(Box::new(arr)) as *mut u8
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Frees a TCP exchange handle.
pub extern "C" fn fj_rt_dist_tcp_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees valid handle
        unsafe {
            drop(Box::from_raw(ptr as *mut TcpExchangeHandle));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Async I/O (S10.4): background file read/write returning future handles
// ═══════════════════════════════════════════════════════════════════════

/// Handle for an async I/O operation. Wraps a background thread that
/// produces a result (string pointer + length for reads, status for writes).
struct AsyncIoHandle {
    /// True when the operation has completed.
    is_ready: std::sync::atomic::AtomicBool,
    /// Result: 0 = success, 1 = error.
    status: std::sync::atomic::AtomicI64,
    /// For reads: heap-allocated result buffer pointer.
    result_ptr: std::sync::Mutex<*mut u8>,
    /// For reads: result length.
    result_len: std::sync::atomic::AtomicI64,
    /// Join handle for the background thread.
    _thread: Option<std::thread::JoinHandle<()>>,
}

// SAFETY: We only access result_ptr under a Mutex and AtomicBool guard.
unsafe impl Send for AsyncIoHandle {}
unsafe impl Sync for AsyncIoHandle {}

/// Spawns a background thread to read a file. Returns an opaque async handle.
///
/// # Safety
///
/// `path_ptr` must point to a valid UTF-8 string of `path_len` bytes.
pub extern "C" fn fj_rt_async_read_file(path_ptr: *const u8, path_len: i64) -> *mut u8 {
    // SAFETY: caller guarantees valid string slice
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    }
    .to_string();

    let handle = std::sync::Arc::new(AsyncIoHandle {
        is_ready: std::sync::atomic::AtomicBool::new(false),
        status: std::sync::atomic::AtomicI64::new(0),
        result_ptr: std::sync::Mutex::new(std::ptr::null_mut()),
        result_len: std::sync::atomic::AtomicI64::new(0),
        _thread: None,
    });

    let handle_clone = handle.clone();
    let thread = std::thread::spawn(move || {
        match std::fs::read(&path) {
            Ok(data) => {
                let len = data.len();
                let layout =
                    std::alloc::Layout::from_size_align(len.max(1), 8).expect("async read alloc");
                // SAFETY: layout is valid, freshly allocated
                let buf = unsafe { std::alloc::alloc(layout) };
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), buf, len);
                }
                *handle_clone.result_ptr.lock().expect("lock") = buf;
                handle_clone
                    .result_len
                    .store(len as i64, std::sync::atomic::Ordering::Release);
                handle_clone
                    .status
                    .store(0, std::sync::atomic::Ordering::Release);
            }
            Err(_) => {
                handle_clone
                    .status
                    .store(1, std::sync::atomic::Ordering::Release);
            }
        }
        handle_clone
            .is_ready
            .store(true, std::sync::atomic::Ordering::Release);
    });

    // Leak the thread handle (will be joined on free)
    let _ = thread;

    // Convert Arc to raw pointer for opaque handle
    std::sync::Arc::into_raw(handle) as *mut u8
}

/// Spawns a background thread to write a file. Returns an opaque async handle.
///
/// # Safety
///
/// Pointers must be valid UTF-8 strings of the specified lengths.
pub extern "C" fn fj_rt_async_write_file(
    path_ptr: *const u8,
    path_len: i64,
    content_ptr: *const u8,
    content_len: i64,
) -> *mut u8 {
    // SAFETY: caller guarantees valid string slices
    let path = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len as usize))
    }
    .to_string();
    let content = unsafe { std::slice::from_raw_parts(content_ptr, content_len as usize) }.to_vec();

    let handle = std::sync::Arc::new(AsyncIoHandle {
        is_ready: std::sync::atomic::AtomicBool::new(false),
        status: std::sync::atomic::AtomicI64::new(0),
        result_ptr: std::sync::Mutex::new(std::ptr::null_mut()),
        result_len: std::sync::atomic::AtomicI64::new(0),
        _thread: None,
    });

    let handle_clone = handle.clone();
    let _thread = std::thread::spawn(move || {
        match std::fs::write(&path, &content) {
            Ok(()) => {
                handle_clone
                    .status
                    .store(0, std::sync::atomic::Ordering::Release);
            }
            Err(_) => {
                handle_clone
                    .status
                    .store(1, std::sync::atomic::Ordering::Release);
            }
        }
        handle_clone
            .is_ready
            .store(true, std::sync::atomic::Ordering::Release);
    });

    std::sync::Arc::into_raw(handle) as *mut u8
}

/// Polls an async I/O handle. Returns 1 if ready, 0 if pending.
pub extern "C" fn fj_rt_async_io_poll(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid AsyncIoHandle Arc pointer
    let handle = unsafe { &*(ptr as *const AsyncIoHandle) };
    if handle.is_ready.load(std::sync::atomic::Ordering::Acquire) {
        1
    } else {
        0
    }
}

/// Gets the status of a completed async I/O op. 0 = success, 1 = error.
pub extern "C" fn fj_rt_async_io_status(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid AsyncIoHandle Arc pointer
    let handle = unsafe { &*(ptr as *const AsyncIoHandle) };
    handle.status.load(std::sync::atomic::Ordering::Acquire)
}

/// For async reads: gets the result string pointer.
pub extern "C" fn fj_rt_async_io_result_ptr(ptr: *mut u8) -> *mut u8 {
    // SAFETY: caller guarantees valid AsyncIoHandle Arc pointer
    let handle = unsafe { &*(ptr as *const AsyncIoHandle) };
    *handle.result_ptr.lock().expect("lock")
}

/// For async reads: gets the result string length.
pub extern "C" fn fj_rt_async_io_result_len(ptr: *mut u8) -> i64 {
    // SAFETY: caller guarantees valid AsyncIoHandle Arc pointer
    let handle = unsafe { &*(ptr as *const AsyncIoHandle) };
    handle.result_len.load(std::sync::atomic::Ordering::Acquire)
}

/// Frees an async I/O handle.
pub extern "C" fn fj_rt_async_io_free(ptr: *mut u8) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees this was created by async_read_file or async_write_file
        unsafe {
            let _ = std::sync::Arc::from_raw(ptr as *const AsyncIoHandle);
        }
    }
}

/// Lazy runtime symbol lookup for JIT.
/// Maps fj_rt_* symbol names to their function pointers on demand,
/// avoiding the need to pre-register all ~300 symbols at startup.
pub fn lookup_runtime_symbol(name: &str) -> Option<*const u8> {
    match name {
        "fj_rt_adam_new" => Some(fj_rt_adam_new as *const u8),
        "fj_rt_adam_step" => Some(fj_rt_adam_step as *const u8),
        "fj_rt_alloc" => Some(fj_rt_alloc as *const u8),
        "fj_rt_append_file" => Some(fj_rt_append_file as *const u8),
        "fj_rt_arc_clone" => Some(fj_rt_arc_clone as *const u8),
        "fj_rt_arc_drop" => Some(fj_rt_arc_drop as *const u8),
        "fj_rt_arc_load" => Some(fj_rt_arc_load as *const u8),
        "fj_rt_arc_new" => Some(fj_rt_arc_new as *const u8),
        "fj_rt_arc_store" => Some(fj_rt_arc_store as *const u8),
        "fj_rt_arc_strong_count" => Some(fj_rt_arc_strong_count as *const u8),
        "fj_rt_array_contains" => Some(fj_rt_array_contains as *const u8),
        "fj_rt_array_free" => Some(fj_rt_array_free as *const u8),
        "fj_rt_array_get" => Some(fj_rt_array_get as *const u8),
        "fj_rt_array_is_empty" => Some(fj_rt_array_is_empty as *const u8),
        "fj_rt_array_join" => Some(fj_rt_array_join as *const u8),
        "fj_rt_array_len" => Some(fj_rt_array_len as *const u8),
        "fj_rt_array_new" => Some(fj_rt_array_new as *const u8),
        "fj_rt_array_pop" => Some(fj_rt_array_pop as *const u8),
        "fj_rt_array_push" => Some(fj_rt_array_push as *const u8),
        "fj_rt_array_reverse" => Some(fj_rt_array_reverse as *const u8),
        "fj_rt_array_set" => Some(fj_rt_array_set as *const u8),
        "fj_rt_async_bchannel_close" => Some(fj_rt_async_bchannel_close as *const u8),
        "fj_rt_async_bchannel_free" => Some(fj_rt_async_bchannel_free as *const u8),
        "fj_rt_async_bchannel_recv" => Some(fj_rt_async_bchannel_recv as *const u8),
        "fj_rt_async_bchannel_send" => Some(fj_rt_async_bchannel_send as *const u8),
        "fj_rt_async_channel_bounded" => Some(fj_rt_async_channel_bounded as *const u8),
        "fj_rt_async_channel_close" => Some(fj_rt_async_channel_close as *const u8),
        "fj_rt_async_channel_free" => Some(fj_rt_async_channel_free as *const u8),
        "fj_rt_async_channel_new" => Some(fj_rt_async_channel_new as *const u8),
        "fj_rt_async_channel_recv" => Some(fj_rt_async_channel_recv as *const u8),
        "fj_rt_async_channel_send" => Some(fj_rt_async_channel_send as *const u8),
        "fj_rt_atomic_add" => Some(fj_rt_atomic_add as *const u8),
        "fj_rt_atomic_and" => Some(fj_rt_atomic_and as *const u8),
        "fj_rt_atomic_bool_free" => Some(fj_rt_atomic_bool_free as *const u8),
        "fj_rt_atomic_bool_load" => Some(fj_rt_atomic_bool_load as *const u8),
        "fj_rt_atomic_bool_new" => Some(fj_rt_atomic_bool_new as *const u8),
        "fj_rt_atomic_bool_store" => Some(fj_rt_atomic_bool_store as *const u8),
        "fj_rt_atomic_cas" => Some(fj_rt_atomic_cas as *const u8),
        "fj_rt_atomic_free" => Some(fj_rt_atomic_free as *const u8),
        "fj_rt_atomic_i32_free" => Some(fj_rt_atomic_i32_free as *const u8),
        "fj_rt_atomic_i32_load" => Some(fj_rt_atomic_i32_load as *const u8),
        "fj_rt_atomic_i32_new" => Some(fj_rt_atomic_i32_new as *const u8),
        "fj_rt_atomic_i32_store" => Some(fj_rt_atomic_i32_store as *const u8),
        "fj_rt_atomic_load" => Some(fj_rt_atomic_load as *const u8),
        "fj_rt_atomic_load_acquire" => Some(fj_rt_atomic_load_acquire as *const u8),
        "fj_rt_atomic_load_relaxed" => Some(fj_rt_atomic_load_relaxed as *const u8),
        "fj_rt_atomic_new" => Some(fj_rt_atomic_new as *const u8),
        "fj_rt_atomic_or" => Some(fj_rt_atomic_or as *const u8),
        "fj_rt_atomic_store" => Some(fj_rt_atomic_store as *const u8),
        "fj_rt_atomic_store_relaxed" => Some(fj_rt_atomic_store_relaxed as *const u8),
        "fj_rt_atomic_store_release" => Some(fj_rt_atomic_store_release as *const u8),
        "fj_rt_atomic_sub" => Some(fj_rt_atomic_sub as *const u8),
        "fj_rt_atomic_xor" => Some(fj_rt_atomic_xor as *const u8),
        "fj_rt_barrier_free" => Some(fj_rt_barrier_free as *const u8),
        "fj_rt_barrier_new" => Some(fj_rt_barrier_new as *const u8),
        "fj_rt_barrier_wait" => Some(fj_rt_barrier_wait as *const u8),
        "fj_rt_bump_alloc" => Some(fj_rt_bump_alloc as *const u8),
        "fj_rt_bump_destroy" => Some(fj_rt_bump_destroy as *const u8),
        "fj_rt_bump_new" => Some(fj_rt_bump_new as *const u8),
        "fj_rt_bump_reset" => Some(fj_rt_bump_reset as *const u8),
        "fj_rt_channel_bounded" => Some(fj_rt_channel_bounded as *const u8),
        "fj_rt_channel_bounded_free" => Some(fj_rt_channel_bounded_free as *const u8),
        "fj_rt_channel_bounded_recv" => Some(fj_rt_channel_bounded_recv as *const u8),
        "fj_rt_channel_bounded_send" => Some(fj_rt_channel_bounded_send as *const u8),
        "fj_rt_channel_clone_sender" => Some(fj_rt_channel_clone_sender as *const u8),
        "fj_rt_channel_close" => Some(fj_rt_channel_close as *const u8),
        "fj_rt_channel_free" => Some(fj_rt_channel_free as *const u8),
        "fj_rt_channel_new" => Some(fj_rt_channel_new as *const u8),
        "fj_rt_channel_recv" => Some(fj_rt_channel_recv as *const u8),
        "fj_rt_channel_select2" => Some(fj_rt_channel_select2 as *const u8),
        "fj_rt_channel_send" => Some(fj_rt_channel_send as *const u8),
        "fj_rt_channel_try_recv" => Some(fj_rt_channel_try_recv as *const u8),
        "fj_rt_channel_try_send" => Some(fj_rt_channel_try_send as *const u8),
        "fj_rt_closure_call_0" => Some(fj_rt_closure_call_0 as *const u8),
        "fj_rt_closure_call_1" => Some(fj_rt_closure_call_1 as *const u8),
        "fj_rt_closure_call_2" => Some(fj_rt_closure_call_2 as *const u8),
        "fj_rt_closure_capture_count" => Some(fj_rt_closure_capture_count as *const u8),
        "fj_rt_closure_free" => Some(fj_rt_closure_free as *const u8),
        "fj_rt_closure_get_capture" => Some(fj_rt_closure_get_capture as *const u8),
        "fj_rt_closure_get_fn" => Some(fj_rt_closure_get_fn as *const u8),
        "fj_rt_closure_new" => Some(fj_rt_closure_new as *const u8),
        "fj_rt_closure_set_capture" => Some(fj_rt_closure_set_capture as *const u8),
        "fj_rt_checkpoint_epoch" => Some(fj_rt_checkpoint_epoch as *const u8),
        "fj_rt_checkpoint_load" => Some(fj_rt_checkpoint_load as *const u8),
        "fj_rt_checkpoint_loss" => Some(fj_rt_checkpoint_loss as *const u8),
        "fj_rt_checkpoint_save" => Some(fj_rt_checkpoint_save as *const u8),
        "fj_rt_compiler_fence" => Some(fj_rt_compiler_fence as *const u8),
        "fj_rt_condvar_free" => Some(fj_rt_condvar_free as *const u8),
        "fj_rt_condvar_new" => Some(fj_rt_condvar_new as *const u8),
        "fj_rt_condvar_notify_all" => Some(fj_rt_condvar_notify_all as *const u8),
        "fj_rt_condvar_notify_one" => Some(fj_rt_condvar_notify_one as *const u8),
        "fj_rt_condvar_wait" => Some(fj_rt_condvar_wait as *const u8),
        "fj_rt_cross_entropy_loss" => Some(fj_rt_cross_entropy_loss as *const u8),
        "fj_rt_dataloader_free" => Some(fj_rt_dataloader_free as *const u8),
        "fj_rt_dataloader_len" => Some(fj_rt_dataloader_len as *const u8),
        "fj_rt_dataloader_new" => Some(fj_rt_dataloader_new as *const u8),
        "fj_rt_dataloader_next_data" => Some(fj_rt_dataloader_next_data as *const u8),
        "fj_rt_dataloader_next_labels" => Some(fj_rt_dataloader_next_labels as *const u8),
        "fj_rt_dataloader_num_samples" => Some(fj_rt_dataloader_num_samples as *const u8),
        "fj_rt_dataloader_reset" => Some(fj_rt_dataloader_reset as *const u8),
        "fj_rt_dbg_f64" => Some(fj_rt_dbg_f64 as *const u8),
        "fj_rt_dbg_i64" => Some(fj_rt_dbg_i64 as *const u8),
        "fj_rt_dbg_str" => Some(fj_rt_dbg_str as *const u8),
        "fj_rt_dist_all_reduce_sum" => Some(fj_rt_dist_all_reduce_sum as *const u8),
        "fj_rt_dist_broadcast" => Some(fj_rt_dist_broadcast as *const u8),
        "fj_rt_dist_free" => Some(fj_rt_dist_free as *const u8),
        "fj_rt_dist_init" => Some(fj_rt_dist_init as *const u8),
        "fj_rt_dist_split_batch" => Some(fj_rt_dist_split_batch as *const u8),
        "fj_rt_dist_rank" => Some(fj_rt_dist_rank as *const u8),
        "fj_rt_dist_tcp_bind" => Some(fj_rt_dist_tcp_bind as *const u8),
        "fj_rt_dist_tcp_free" => Some(fj_rt_dist_tcp_free as *const u8),
        "fj_rt_dist_tcp_port" => Some(fj_rt_dist_tcp_port as *const u8),
        "fj_rt_dist_tcp_recv" => Some(fj_rt_dist_tcp_recv as *const u8),
        "fj_rt_dist_tcp_send" => Some(fj_rt_dist_tcp_send as *const u8),
        "fj_rt_dist_world_size" => Some(fj_rt_dist_world_size as *const u8),
        "fj_rt_eprint_i64" => Some(fj_rt_eprint_i64 as *const u8),
        "fj_rt_eprintln_bool" => Some(fj_rt_eprintln_bool as *const u8),
        "fj_rt_eprintln_f64" => Some(fj_rt_eprintln_f64 as *const u8),
        "fj_rt_eprintln_i64" => Some(fj_rt_eprintln_i64 as *const u8),
        "fj_rt_eprintln_str" => Some(fj_rt_eprintln_str as *const u8),
        "fj_rt_eprint_str" => Some(fj_rt_eprint_str as *const u8),
        "fj_rt_executor_block_on" => Some(fj_rt_executor_block_on as *const u8),
        "fj_rt_executor_free" => Some(fj_rt_executor_free as *const u8),
        "fj_rt_executor_get_result" => Some(fj_rt_executor_get_result as *const u8),
        "fj_rt_executor_new" => Some(fj_rt_executor_new as *const u8),
        "fj_rt_executor_run" => Some(fj_rt_executor_run as *const u8),
        "fj_rt_executor_spawn" => Some(fj_rt_executor_spawn as *const u8),
        "fj_rt_f16_to_f32" => Some(fj_rt_f16_to_f32 as *const u8),
        "fj_rt_f32_to_f16" => Some(fj_rt_f32_to_f16 as *const u8),
        "fj_rt_file_exists" => Some(fj_rt_file_exists as *const u8),
        "fj_rt_float_to_string" => Some(fj_rt_float_to_string as *const u8),
        "fj_rt_format" => Some(fj_rt_format as *const u8),
        "fj_rt_free" => Some(fj_rt_free as *const u8),
        "fj_rt_freelist_alloc" => Some(fj_rt_freelist_alloc as *const u8),
        "fj_rt_freelist_destroy" => Some(fj_rt_freelist_destroy as *const u8),
        "fj_rt_freelist_free" => Some(fj_rt_freelist_free as *const u8),
        "fj_rt_freelist_new" => Some(fj_rt_freelist_new as *const u8),
        "fj_rt_future_free" => Some(fj_rt_future_free as *const u8),
        "fj_rt_future_get_result" => Some(fj_rt_future_get_result as *const u8),
        "fj_rt_future_get_state" => Some(fj_rt_future_get_state as *const u8),
        "fj_rt_future_load_local" => Some(fj_rt_future_load_local as *const u8),
        "fj_rt_future_new" => Some(fj_rt_future_new as *const u8),
        "fj_rt_future_poll" => Some(fj_rt_future_poll as *const u8),
        "fj_rt_future_save_local" => Some(fj_rt_future_save_local as *const u8),
        "fj_rt_future_set_result" => Some(fj_rt_future_set_result as *const u8),
        "fj_rt_future_set_state" => Some(fj_rt_future_set_state as *const u8),
        "fj_rt_grad_tensor_data" => Some(fj_rt_grad_tensor_data as *const u8),
        "fj_rt_grad_matmul" => Some(fj_rt_grad_matmul as *const u8),
        "fj_rt_grad_relu" => Some(fj_rt_grad_relu as *const u8),
        "fj_rt_grad_sigmoid" => Some(fj_rt_grad_sigmoid as *const u8),
        "fj_rt_grad_softmax" => Some(fj_rt_grad_softmax as *const u8),
        "fj_rt_grad_tensor_free" => Some(fj_rt_grad_tensor_free as *const u8),
        "fj_rt_int_to_string" => Some(fj_rt_int_to_string as *const u8),
        "fj_rt_joinhandle_abort" => Some(fj_rt_joinhandle_abort as *const u8),
        "fj_rt_joinhandle_free" => Some(fj_rt_joinhandle_free as *const u8),
        "fj_rt_joinhandle_get_result" => Some(fj_rt_joinhandle_get_result as *const u8),
        "fj_rt_joinhandle_is_cancelled" => Some(fj_rt_joinhandle_is_cancelled as *const u8),
        "fj_rt_joinhandle_is_ready" => Some(fj_rt_joinhandle_is_ready as *const u8),
        "fj_rt_joinhandle_new" => Some(fj_rt_joinhandle_new as *const u8),
        "fj_rt_joinhandle_set_result" => Some(fj_rt_joinhandle_set_result as *const u8),
        "fj_rt_map_clear" => Some(fj_rt_map_clear as *const u8),
        "fj_rt_map_contains" => Some(fj_rt_map_contains as *const u8),
        "fj_rt_map_free" => Some(fj_rt_map_free as *const u8),
        "fj_rt_map_get_int" => Some(fj_rt_map_get_int as *const u8),
        "fj_rt_map_get_str" => Some(fj_rt_map_get_str as *const u8),
        "fj_rt_map_insert_float" => Some(fj_rt_map_insert_float as *const u8),
        "fj_rt_map_insert_int" => Some(fj_rt_map_insert_int as *const u8),
        "fj_rt_map_insert_str" => Some(fj_rt_map_insert_str as *const u8),
        "fj_rt_map_keys" => Some(fj_rt_map_keys as *const u8),
        "fj_rt_map_len" => Some(fj_rt_map_len as *const u8),
        "fj_rt_map_new" => Some(fj_rt_map_new as *const u8),
        "fj_rt_map_remove" => Some(fj_rt_map_remove as *const u8),
        "fj_rt_map_values" => Some(fj_rt_map_values as *const u8),
        "fj_rt_math_cos" => Some(fj_rt_math_cos as *const u8),
        "fj_rt_math_log" => Some(fj_rt_math_log as *const u8),
        "fj_rt_math_log10" => Some(fj_rt_math_log10 as *const u8),
        "fj_rt_math_log2" => Some(fj_rt_math_log2 as *const u8),
        "fj_rt_math_pow" => Some(fj_rt_math_pow as *const u8),
        "fj_rt_math_sin" => Some(fj_rt_math_sin as *const u8),
        "fj_rt_math_tan" => Some(fj_rt_math_tan as *const u8),
        "fj_rt_memory_fence" => Some(fj_rt_memory_fence as *const u8),
        "fj_rt_mem_read" => Some(fj_rt_mem_read as *const u8),
        "fj_rt_mem_write" => Some(fj_rt_mem_write as *const u8),
        "fj_rt_mnist_load_images" => Some(fj_rt_mnist_load_images as *const u8),
        "fj_rt_mnist_load_labels" => Some(fj_rt_mnist_load_labels as *const u8),
        "fj_rt_mnist_parse_images_buf" => Some(fj_rt_mnist_parse_images_buf as *const u8),
        "fj_rt_mnist_parse_labels_buf" => Some(fj_rt_mnist_parse_labels_buf as *const u8),
        "fj_rt_mse_loss" => Some(fj_rt_mse_loss as *const u8),
        "fj_rt_mutex_free" => Some(fj_rt_mutex_free as *const u8),
        "fj_rt_mutex_lock" => Some(fj_rt_mutex_lock as *const u8),
        "fj_rt_mutex_new" => Some(fj_rt_mutex_new as *const u8),
        "fj_rt_mutex_store" => Some(fj_rt_mutex_store as *const u8),
        "fj_rt_mutex_try_lock" => Some(fj_rt_mutex_try_lock as *const u8),
        "fj_rt_mutex_guard_lock" => Some(fj_rt_mutex_guard_lock as *const u8),
        "fj_rt_mutex_guard_get" => Some(fj_rt_mutex_guard_get as *const u8),
        "fj_rt_mutex_guard_set" => Some(fj_rt_mutex_guard_set as *const u8),
        "fj_rt_mutex_guard_free" => Some(fj_rt_mutex_guard_free as *const u8),
        "fj_rt_onnx_add_dense" => Some(fj_rt_onnx_add_dense as *const u8),
        "fj_rt_onnx_add_relu" => Some(fj_rt_onnx_add_relu as *const u8),
        "fj_rt_onnx_export" => Some(fj_rt_onnx_export as *const u8),
        "fj_rt_onnx_free" => Some(fj_rt_onnx_free as *const u8),
        "fj_rt_onnx_initializer_count" => Some(fj_rt_onnx_initializer_count as *const u8),
        "fj_rt_onnx_new" => Some(fj_rt_onnx_new as *const u8),
        "fj_rt_onnx_node_count" => Some(fj_rt_onnx_node_count as *const u8),
        "fj_rt_onnx_set_input" => Some(fj_rt_onnx_set_input as *const u8),
        "fj_rt_onnx_set_output" => Some(fj_rt_onnx_set_output as *const u8),
        "fj_rt_optimizer_free" => Some(fj_rt_optimizer_free as *const u8),
        "fj_rt_parse_float" => Some(fj_rt_parse_float as *const u8),
        "fj_rt_parse_int" => Some(fj_rt_parse_int as *const u8),
        "fj_rt_pool_alloc" => Some(fj_rt_pool_alloc as *const u8),
        "fj_rt_pool_destroy" => Some(fj_rt_pool_destroy as *const u8),
        "fj_rt_pool_free" => Some(fj_rt_pool_free as *const u8),
        "fj_rt_pool_new" => Some(fj_rt_pool_new as *const u8),
        "fj_rt_print_bool" => Some(fj_rt_print_bool as *const u8),
        "fj_rt_print_f64_no_newline" => Some(fj_rt_print_f64_no_newline as *const u8),
        "fj_rt_print_i64" => Some(fj_rt_print_i64 as *const u8),
        "fj_rt_print_i64_no_newline" => Some(fj_rt_print_i64_no_newline as *const u8),
        "fj_rt_println_bool" => Some(fj_rt_println_bool as *const u8),
        "fj_rt_println_f64" => Some(fj_rt_println_f64 as *const u8),
        "fj_rt_println_str" => Some(fj_rt_println_str as *const u8),
        "fj_rt_print_str" => Some(fj_rt_print_str as *const u8),
        "fj_rt_random_int" => Some(fj_rt_random_int as *const u8),
        "fj_rt_read_file" => Some(fj_rt_read_file as *const u8),
        "fj_rt_reset_global_allocator" => Some(fj_rt_reset_global_allocator as *const u8),
        "fj_rt_rwlock_free" => Some(fj_rt_rwlock_free as *const u8),
        "fj_rt_rwlock_new" => Some(fj_rt_rwlock_new as *const u8),
        "fj_rt_rwlock_read" => Some(fj_rt_rwlock_read as *const u8),
        "fj_rt_rwlock_write" => Some(fj_rt_rwlock_write as *const u8),
        "fj_rt_saturating_add" => Some(fj_rt_saturating_add as *const u8),
        "fj_rt_saturating_mul" => Some(fj_rt_saturating_mul as *const u8),
        "fj_rt_saturating_sub" => Some(fj_rt_saturating_sub as *const u8),
        "fj_rt_set_global_allocator" => Some(fj_rt_set_global_allocator as *const u8),
        "fj_rt_sgd_new" => Some(fj_rt_sgd_new as *const u8),
        "fj_rt_sgd_step" => Some(fj_rt_sgd_step as *const u8),
        "fj_rt_simd_f32x4_add" => Some(fj_rt_simd_f32x4_add as *const u8),
        "fj_rt_simd_f32x4_div" => Some(fj_rt_simd_f32x4_div as *const u8),
        "fj_rt_simd_f32x4_free" => Some(fj_rt_simd_f32x4_free as *const u8),
        "fj_rt_simd_f32x4_get" => Some(fj_rt_simd_f32x4_get as *const u8),
        "fj_rt_simd_f32x4_load" => Some(fj_rt_simd_f32x4_load as *const u8),
        "fj_rt_simd_f32x4_max" => Some(fj_rt_simd_f32x4_max as *const u8),
        "fj_rt_simd_f32x4_min" => Some(fj_rt_simd_f32x4_min as *const u8),
        "fj_rt_simd_f32x4_mul" => Some(fj_rt_simd_f32x4_mul as *const u8),
        "fj_rt_simd_f32x4_new" => Some(fj_rt_simd_f32x4_new as *const u8),
        "fj_rt_simd_f32x4_splat" => Some(fj_rt_simd_f32x4_splat as *const u8),
        "fj_rt_simd_f32x4_store" => Some(fj_rt_simd_f32x4_store as *const u8),
        "fj_rt_simd_f32x4_sub" => Some(fj_rt_simd_f32x4_sub as *const u8),
        "fj_rt_simd_f32x4_sum" => Some(fj_rt_simd_f32x4_sum as *const u8),
        "fj_rt_simd_f32x4_zeros" => Some(fj_rt_simd_f32x4_zeros as *const u8),
        "fj_rt_simd_f32x8_add" => Some(fj_rt_simd_f32x8_add as *const u8),
        "fj_rt_simd_f32x8_free" => Some(fj_rt_simd_f32x8_free as *const u8),
        "fj_rt_simd_f32x8_get" => Some(fj_rt_simd_f32x8_get as *const u8),
        "fj_rt_simd_f32x8_mul" => Some(fj_rt_simd_f32x8_mul as *const u8),
        "fj_rt_simd_f32x8_splat" => Some(fj_rt_simd_f32x8_splat as *const u8),
        "fj_rt_simd_f32x8_sum" => Some(fj_rt_simd_f32x8_sum as *const u8),
        "fj_rt_simd_i32x4_add" => Some(fj_rt_simd_i32x4_add as *const u8),
        "fj_rt_simd_i32x4_free" => Some(fj_rt_simd_i32x4_free as *const u8),
        "fj_rt_simd_i32x4_get" => Some(fj_rt_simd_i32x4_get as *const u8),
        "fj_rt_simd_i32x4_load" => Some(fj_rt_simd_i32x4_load as *const u8),
        "fj_rt_simd_i32x4_max" => Some(fj_rt_simd_i32x4_max as *const u8),
        "fj_rt_simd_i32x4_min" => Some(fj_rt_simd_i32x4_min as *const u8),
        "fj_rt_simd_i32x4_mul" => Some(fj_rt_simd_i32x4_mul as *const u8),
        "fj_rt_simd_i32x4_new" => Some(fj_rt_simd_i32x4_new as *const u8),
        "fj_rt_simd_i32x4_splat" => Some(fj_rt_simd_i32x4_splat as *const u8),
        "fj_rt_simd_i32x4_store" => Some(fj_rt_simd_i32x4_store as *const u8),
        "fj_rt_simd_i32x4_sub" => Some(fj_rt_simd_i32x4_sub as *const u8),
        "fj_rt_simd_i32x4_sum" => Some(fj_rt_simd_i32x4_sum as *const u8),
        "fj_rt_simd_i32x8_add" => Some(fj_rt_simd_i32x8_add as *const u8),
        "fj_rt_simd_i32x8_free" => Some(fj_rt_simd_i32x8_free as *const u8),
        "fj_rt_simd_i32x8_get" => Some(fj_rt_simd_i32x8_get as *const u8),
        "fj_rt_simd_i32x8_mul" => Some(fj_rt_simd_i32x8_mul as *const u8),
        "fj_rt_simd_i32x8_splat" => Some(fj_rt_simd_i32x8_splat as *const u8),
        "fj_rt_simd_i32x8_sum" => Some(fj_rt_simd_i32x8_sum as *const u8),
        "fj_rt_sleep" => Some(fj_rt_sleep as *const u8),
        "fj_rt_split_get" => Some(fj_rt_split_get as *const u8),
        "fj_rt_split_len" => Some(fj_rt_split_len as *const u8),
        "fj_rt_str_bytes" => Some(fj_rt_str_bytes as *const u8),
        "fj_rt_str_chars" => Some(fj_rt_str_chars as *const u8),
        "fj_rt_str_concat" => Some(fj_rt_str_concat as *const u8),
        "fj_rt_str_contains" => Some(fj_rt_str_contains as *const u8),
        "fj_rt_str_eq" => Some(fj_rt_str_eq as *const u8),
        "fj_rt_stream_close" => Some(fj_rt_stream_close as *const u8),
        "fj_rt_stream_count" => Some(fj_rt_stream_count as *const u8),
        "fj_rt_stream_filter" => Some(fj_rt_stream_filter as *const u8),
        "fj_rt_stream_free" => Some(fj_rt_stream_free as *const u8),
        "fj_rt_stream_from_range" => Some(fj_rt_stream_from_range as *const u8),
        "fj_rt_stream_has_next" => Some(fj_rt_stream_has_next as *const u8),
        "fj_rt_stream_map" => Some(fj_rt_stream_map as *const u8),
        "fj_rt_stream_new" => Some(fj_rt_stream_new as *const u8),
        "fj_rt_stream_next" => Some(fj_rt_stream_next as *const u8),
        "fj_rt_stream_push" => Some(fj_rt_stream_push as *const u8),
        "fj_rt_stream_sum" => Some(fj_rt_stream_sum as *const u8),
        "fj_rt_stream_take" => Some(fj_rt_stream_take as *const u8),
        "fj_rt_str_ends_with" => Some(fj_rt_str_ends_with as *const u8),
        "fj_rt_str_index_of" => Some(fj_rt_str_index_of as *const u8),
        "fj_rt_str_repeat" => Some(fj_rt_str_repeat as *const u8),
        "fj_rt_str_replace" => Some(fj_rt_str_replace as *const u8),
        "fj_rt_str_rev" => Some(fj_rt_str_rev as *const u8),
        "fj_rt_str_split" => Some(fj_rt_str_split as *const u8),
        "fj_rt_str_starts_with" => Some(fj_rt_str_starts_with as *const u8),
        "fj_rt_str_substring" => Some(fj_rt_str_substring as *const u8),
        "fj_rt_str_to_lowercase" => Some(fj_rt_str_to_lowercase as *const u8),
        "fj_rt_str_to_uppercase" => Some(fj_rt_str_to_uppercase as *const u8),
        "fj_rt_str_trim" => Some(fj_rt_str_trim as *const u8),
        "fj_rt_str_trim_end" => Some(fj_rt_str_trim_end as *const u8),
        "fj_rt_str_trim_start" => Some(fj_rt_str_trim_start as *const u8),
        "fj_rt_tensor_abs" => Some(fj_rt_tensor_abs as *const u8),
        "fj_rt_tensor_add" => Some(fj_rt_tensor_add as *const u8),
        "fj_rt_tensor_argmax" => Some(fj_rt_tensor_argmax as *const u8),
        "fj_rt_tensor_cols" => Some(fj_rt_tensor_cols as *const u8),
        "fj_rt_tensor_fill" => Some(fj_rt_tensor_fill as *const u8),
        "fj_rt_tensor_free" => Some(fj_rt_tensor_free as *const u8),
        "fj_rt_tensor_from_data" => Some(fj_rt_tensor_from_data as *const u8),
        "fj_rt_tensor_get" => Some(fj_rt_tensor_get as *const u8),
        "fj_rt_tensor_grad" => Some(fj_rt_tensor_grad as *const u8),
        "fj_rt_tensor_load" => Some(fj_rt_tensor_load as *const u8),
        "fj_rt_tensor_matmul" => Some(fj_rt_tensor_matmul as *const u8),
        "fj_rt_tensor_mean" => Some(fj_rt_tensor_mean as *const u8),
        "fj_rt_tensor_mul" => Some(fj_rt_tensor_mul as *const u8),
        "fj_rt_tensor_normalize" => Some(fj_rt_tensor_normalize as *const u8),
        "fj_rt_tensor_ones" => Some(fj_rt_tensor_ones as *const u8),
        "fj_rt_tensor_rand" => Some(fj_rt_tensor_rand as *const u8),
        "fj_rt_tensor_relu" => Some(fj_rt_tensor_relu as *const u8),
        "fj_rt_tensor_requires_grad" => Some(fj_rt_tensor_requires_grad as *const u8),
        "fj_rt_tensor_row" => Some(fj_rt_tensor_row as *const u8),
        "fj_rt_tensor_rows" => Some(fj_rt_tensor_rows as *const u8),
        "fj_rt_tensor_save" => Some(fj_rt_tensor_save as *const u8),
        "fj_rt_tensor_scale" => Some(fj_rt_tensor_scale as *const u8),
        "fj_rt_tensor_set" => Some(fj_rt_tensor_set as *const u8),
        "fj_rt_tensor_sigmoid" => Some(fj_rt_tensor_sigmoid as *const u8),
        "fj_rt_tensor_softmax" => Some(fj_rt_tensor_softmax as *const u8),
        "fj_rt_tensor_sub" => Some(fj_rt_tensor_sub as *const u8),
        "fj_rt_tensor_sum" => Some(fj_rt_tensor_sum as *const u8),
        "fj_rt_tensor_to_f16" => Some(fj_rt_tensor_to_f16 as *const u8),
        "fj_rt_tensor_transpose" => Some(fj_rt_tensor_transpose as *const u8),
        "fj_rt_tensor_xavier" => Some(fj_rt_tensor_xavier as *const u8),
        "fj_rt_tensor_reshape" => Some(fj_rt_tensor_reshape as *const u8),
        "fj_rt_tensor_flatten" => Some(fj_rt_tensor_flatten as *const u8),
        // Loss scaling & quantization (S39.3-S39.4)
        "fj_rt_loss_scale" => Some(fj_rt_loss_scale as *const u8),
        "fj_rt_loss_unscale" => Some(fj_rt_loss_unscale as *const u8),
        "fj_rt_tensor_quantize_int8" => Some(fj_rt_tensor_quantize_int8 as *const u8),
        "fj_rt_tensor_quant_scale" => Some(fj_rt_tensor_quant_scale as *const u8),
        "fj_rt_tensor_quant_zero_point" => Some(fj_rt_tensor_quant_zero_point as *const u8),
        "fj_rt_tensor_dequantize_int8" => Some(fj_rt_tensor_dequantize_int8 as *const u8),
        "fj_rt_tensor_zero_grad" => Some(fj_rt_tensor_zero_grad as *const u8),
        "fj_rt_tensor_zeros" => Some(fj_rt_tensor_zeros as *const u8),
        "fj_rt_thread_free" => Some(fj_rt_thread_free as *const u8),
        "fj_rt_thread_is_finished" => Some(fj_rt_thread_is_finished as *const u8),
        "fj_rt_thread_join" => Some(fj_rt_thread_join as *const u8),
        "fj_rt_threadpool_free" => Some(fj_rt_threadpool_free as *const u8),
        "fj_rt_threadpool_get_result" => Some(fj_rt_threadpool_get_result as *const u8),
        "fj_rt_threadpool_new" => Some(fj_rt_threadpool_new as *const u8),
        "fj_rt_threadpool_run" => Some(fj_rt_threadpool_run as *const u8),
        "fj_rt_threadpool_spawn" => Some(fj_rt_threadpool_spawn as *const u8),
        "fj_rt_threadpool_spawn_join" => Some(fj_rt_threadpool_spawn_join as *const u8),
        "fj_rt_threadpool_thread_count" => Some(fj_rt_threadpool_thread_count as *const u8),
        "fj_rt_thread_spawn" => Some(fj_rt_thread_spawn as *const u8),
        "fj_rt_thread_spawn_noarg" => Some(fj_rt_thread_spawn_noarg as *const u8),
        "fj_rt_timer_free" => Some(fj_rt_timer_free as *const u8),
        "fj_rt_timer_new" => Some(fj_rt_timer_new as *const u8),
        "fj_rt_timer_pending" => Some(fj_rt_timer_pending as *const u8),
        "fj_rt_timer_schedule" => Some(fj_rt_timer_schedule as *const u8),
        "fj_rt_timer_tick" => Some(fj_rt_timer_tick as *const u8),
        "fj_rt_tls_get" => Some(fj_rt_tls_get as *const u8),
        "fj_rt_tls_set" => Some(fj_rt_tls_set as *const u8),
        "fj_rt_volatile_read" => Some(fj_rt_volatile_read as *const u8),
        "fj_rt_volatile_read_u8" => Some(fj_rt_volatile_read_u8 as *const u8),
        "fj_rt_volatile_read_u16" => Some(fj_rt_volatile_read_u16 as *const u8),
        "fj_rt_volatile_read_u32" => Some(fj_rt_volatile_read_u32 as *const u8),
        "fj_rt_volatile_write" => Some(fj_rt_volatile_write as *const u8),
        "fj_rt_volatile_write_u8" => Some(fj_rt_volatile_write_u8 as *const u8),
        "fj_rt_volatile_write_u16" => Some(fj_rt_volatile_write_u16 as *const u8),
        "fj_rt_volatile_write_u32" => Some(fj_rt_volatile_write_u32 as *const u8),
        "fj_rt_waker_clone" => Some(fj_rt_waker_clone as *const u8),
        "fj_rt_waker_drop" => Some(fj_rt_waker_drop as *const u8),
        "fj_rt_waker_is_woken" => Some(fj_rt_waker_is_woken as *const u8),
        "fj_rt_waker_new" => Some(fj_rt_waker_new as *const u8),
        "fj_rt_waker_reset" => Some(fj_rt_waker_reset as *const u8),
        "fj_rt_waker_wake" => Some(fj_rt_waker_wake as *const u8),
        "fj_rt_write_file" => Some(fj_rt_write_file as *const u8),
        // Async I/O (S10.4)
        "fj_rt_async_read_file" => Some(fj_rt_async_read_file as *const u8),
        "fj_rt_async_write_file" => Some(fj_rt_async_write_file as *const u8),
        "fj_rt_async_io_poll" => Some(fj_rt_async_io_poll as *const u8),
        "fj_rt_async_io_status" => Some(fj_rt_async_io_status as *const u8),
        "fj_rt_async_io_result_ptr" => Some(fj_rt_async_io_result_ptr as *const u8),
        "fj_rt_async_io_result_len" => Some(fj_rt_async_io_result_len as *const u8),
        "fj_rt_async_io_free" => Some(fj_rt_async_io_free as *const u8),
        // Phase 3 HAL bare-metal builtins (runtime_bare.rs simulation)
        "fj_rt_bare_gpio_config" => Some(runtime_bare::fj_rt_bare_gpio_config as *const u8),
        "fj_rt_bare_gpio_set_output" => Some(runtime_bare::fj_rt_bare_gpio_set_output as *const u8),
        "fj_rt_bare_gpio_set_input" => Some(runtime_bare::fj_rt_bare_gpio_set_input as *const u8),
        "fj_rt_bare_gpio_write" => Some(runtime_bare::fj_rt_bare_gpio_write as *const u8),
        "fj_rt_bare_gpio_read" => Some(runtime_bare::fj_rt_bare_gpio_read as *const u8),
        "fj_rt_bare_gpio_toggle" => Some(runtime_bare::fj_rt_bare_gpio_toggle as *const u8),
        "fj_rt_bare_gpio_set_pull" => Some(runtime_bare::fj_rt_bare_gpio_set_pull as *const u8),
        "fj_rt_bare_gpio_set_irq" => Some(runtime_bare::fj_rt_bare_gpio_set_irq as *const u8),
        "fj_rt_bare_uart_init" => Some(runtime_bare::fj_rt_bare_uart_init as *const u8),
        "fj_rt_bare_uart_write_byte" => Some(runtime_bare::fj_rt_bare_uart_write_byte as *const u8),
        "fj_rt_bare_uart_read_byte" => Some(runtime_bare::fj_rt_bare_uart_read_byte as *const u8),
        "fj_rt_bare_uart_available" => Some(runtime_bare::fj_rt_bare_uart_available as *const u8),
        "fj_rt_bare_spi_init" => Some(runtime_bare::fj_rt_bare_spi_init as *const u8),
        "fj_rt_bare_spi_transfer" => Some(runtime_bare::fj_rt_bare_spi_transfer as *const u8),
        "fj_rt_bare_spi_cs_set" => Some(runtime_bare::fj_rt_bare_spi_cs_set as *const u8),
        "fj_rt_bare_i2c_init" => Some(runtime_bare::fj_rt_bare_i2c_init as *const u8),
        "fj_rt_bare_timer_get_ticks" => Some(runtime_bare::fj_rt_bare_timer_get_ticks as *const u8),
        "fj_rt_bare_timer_get_freq" => Some(runtime_bare::fj_rt_bare_timer_get_freq as *const u8),
        "fj_rt_bare_time_since_boot" => Some(runtime_bare::fj_rt_bare_time_since_boot as *const u8),
        "fj_rt_bare_timer_set_deadline" => {
            Some(runtime_bare::fj_rt_bare_timer_set_deadline as *const u8)
        }
        "fj_rt_bare_sleep_ms" => Some(runtime_bare::fj_rt_bare_sleep_ms as *const u8),
        "fj_rt_bare_sleep_us" => Some(runtime_bare::fj_rt_bare_sleep_us as *const u8),
        "fj_rt_bare_timer_enable_virtual" => {
            Some(runtime_bare::fj_rt_bare_timer_enable_virtual as *const u8)
        }
        "fj_rt_bare_timer_disable_virtual" => {
            Some(runtime_bare::fj_rt_bare_timer_disable_virtual as *const u8)
        }
        "fj_rt_bare_timer_mark_boot" => Some(runtime_bare::fj_rt_bare_timer_mark_boot as *const u8),
        "fj_rt_bare_dma_alloc" => Some(runtime_bare::fj_rt_bare_dma_alloc as *const u8),
        "fj_rt_bare_dma_config" => Some(runtime_bare::fj_rt_bare_dma_config as *const u8),
        "fj_rt_bare_dma_start" => Some(runtime_bare::fj_rt_bare_dma_start as *const u8),
        "fj_rt_bare_dma_wait" => Some(runtime_bare::fj_rt_bare_dma_wait as *const u8),
        "fj_rt_bare_dma_status" => Some(runtime_bare::fj_rt_bare_dma_status as *const u8),
        "fj_rt_bare_dma_barrier" => Some(runtime_bare::fj_rt_bare_dma_barrier as *const u8),
        // Phase 4: Storage
        "fj_rt_bare_nvme_init" => Some(runtime_bare::fj_rt_bare_nvme_init as *const u8),
        "fj_rt_bare_nvme_read" => Some(runtime_bare::fj_rt_bare_nvme_read as *const u8),
        "fj_rt_bare_nvme_write" => Some(runtime_bare::fj_rt_bare_nvme_write as *const u8),
        "fj_rt_bare_sd_init" => Some(runtime_bare::fj_rt_bare_sd_init as *const u8),
        "fj_rt_bare_sd_read_block" => Some(runtime_bare::fj_rt_bare_sd_read_block as *const u8),
        "fj_rt_bare_sd_write_block" => Some(runtime_bare::fj_rt_bare_sd_write_block as *const u8),
        "fj_rt_bare_vfs_open" => Some(runtime_bare::fj_rt_bare_vfs_open as *const u8),
        "fj_rt_bare_vfs_read" => Some(runtime_bare::fj_rt_bare_vfs_read as *const u8),
        "fj_rt_bare_vfs_write" => Some(runtime_bare::fj_rt_bare_vfs_write as *const u8),
        "fj_rt_bare_vfs_close" => Some(runtime_bare::fj_rt_bare_vfs_close as *const u8),
        "fj_rt_bare_vfs_stat" => Some(runtime_bare::fj_rt_bare_vfs_stat as *const u8),
        "fj_rt_bare_vfs_mount" => Some(runtime_bare::fj_rt_bare_vfs_mount as *const u8),
        // Phase 5: Network
        "fj_rt_bare_eth_init" => Some(runtime_bare::fj_rt_bare_eth_init as *const u8),
        "fj_rt_bare_eth_send" => Some(runtime_bare::fj_rt_bare_eth_send as *const u8),
        "fj_rt_bare_eth_recv" => Some(runtime_bare::fj_rt_bare_eth_recv as *const u8),
        "fj_rt_bare_net_socket" => Some(runtime_bare::fj_rt_bare_net_socket as *const u8),
        "fj_rt_bare_net_bind" => Some(runtime_bare::fj_rt_bare_net_bind as *const u8),
        "fj_rt_bare_net_listen" => Some(runtime_bare::fj_rt_bare_net_listen as *const u8),
        "fj_rt_bare_net_accept" => Some(runtime_bare::fj_rt_bare_net_accept as *const u8),
        "fj_rt_bare_net_connect" => Some(runtime_bare::fj_rt_bare_net_connect as *const u8),
        "fj_rt_bare_net_send" => Some(runtime_bare::fj_rt_bare_net_send as *const u8),
        "fj_rt_bare_net_recv" => Some(runtime_bare::fj_rt_bare_net_recv as *const u8),
        "fj_rt_bare_net_close" => Some(runtime_bare::fj_rt_bare_net_close as *const u8),
        // Phase 6: Display & Input
        "fj_rt_bare_fb_init" => Some(runtime_bare::fj_rt_bare_fb_init as *const u8),
        "fj_rt_bare_fb_write_pixel" => Some(runtime_bare::fj_rt_bare_fb_write_pixel as *const u8),
        "fj_rt_bare_fb_fill_rect" => Some(runtime_bare::fj_rt_bare_fb_fill_rect as *const u8),
        "fj_rt_bare_fb_width" => Some(runtime_bare::fj_rt_bare_fb_width as *const u8),
        "fj_rt_bare_fb_height" => Some(runtime_bare::fj_rt_bare_fb_height as *const u8),
        "fj_rt_bare_kb_init" => Some(runtime_bare::fj_rt_bare_kb_init as *const u8),
        "fj_rt_bare_kb_read" => Some(runtime_bare::fj_rt_bare_kb_read as *const u8),
        "fj_rt_bare_kb_available" => Some(runtime_bare::fj_rt_bare_kb_available as *const u8),
        // Phase 8: OS Services
        "fj_rt_bare_proc_spawn" => Some(runtime_bare::fj_rt_bare_proc_spawn as *const u8),
        "fj_rt_bare_proc_wait" => Some(runtime_bare::fj_rt_bare_proc_wait as *const u8),
        "fj_rt_bare_proc_kill" => Some(runtime_bare::fj_rt_bare_proc_kill as *const u8),
        "fj_rt_bare_proc_self" => Some(runtime_bare::fj_rt_bare_proc_self as *const u8),
        "fj_rt_bare_proc_yield" => Some(runtime_bare::fj_rt_bare_proc_yield as *const u8),
        "fj_rt_bare_sys_poweroff" => Some(runtime_bare::fj_rt_bare_sys_poweroff as *const u8),
        "fj_rt_bare_sys_reboot" => Some(runtime_bare::fj_rt_bare_sys_reboot as *const u8),
        "fj_rt_bare_sys_cpu_temp" => Some(runtime_bare::fj_rt_bare_sys_cpu_temp as *const u8),
        "fj_rt_bare_sys_ram_total" => Some(runtime_bare::fj_rt_bare_sys_ram_total as *const u8),
        "fj_rt_bare_sys_ram_free" => Some(runtime_bare::fj_rt_bare_sys_ram_free as *const u8),
        _ => None,
    }
}
