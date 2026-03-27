# Lesson 13: Foreign Function Interface (FFI)

## Objectives

By the end of this lesson, you will be able to:

- Declare external C functions in Fajar Lang
- Call C library functions from Fajar Lang code
- Pass data between Fajar Lang and C
- Understand safety considerations for FFI

## What is FFI?

FFI (Foreign Function Interface) lets Fajar Lang call functions written in other languages -- primarily C. This is essential for:

- Using existing C libraries (OpenSSL, SQLite, zlib)
- Interfacing with OS APIs (POSIX, Win32)
- Performance-critical code written in C/assembly

## Declaring External Functions

Use `extern "C"` to declare functions that exist in a C library:

```fajar
extern "C" {
    fn abs(x: i32) -> i32
    fn sqrt(x: f64) -> f64
    fn puts(s: *const u8) -> i32
}

fn main() {
    let result = abs(-42)
    println(result)   // 42

    let root = sqrt(144.0)
    println(root)     // 12.0
}
```

The `extern "C"` block tells the compiler these functions use the C calling convention and are linked from an external library.

## C Type Mapping

Fajar Lang types map to C types as follows:

| Fajar Lang | C Type | Size |
|------------|--------|------|
| `i8` | `int8_t` / `char` | 1 byte |
| `i16` | `int16_t` / `short` | 2 bytes |
| `i32` | `int32_t` / `int` | 4 bytes |
| `i64` | `int64_t` / `long long` | 8 bytes |
| `u8` | `uint8_t` | 1 byte |
| `u16` | `uint16_t` | 2 bytes |
| `u32` | `uint32_t` | 4 bytes |
| `u64` | `uint64_t` | 8 bytes |
| `f32` | `float` | 4 bytes |
| `f64` | `double` | 8 bytes |
| `bool` | `_Bool` | 1 byte |
| `ptr` | `void*` | pointer size |
| `*const T` | `const T*` | pointer size |
| `*mut T` | `T*` | pointer size |

## Calling libc Functions

```fajar
extern "C" {
    fn strlen(s: *const u8) -> u64
    fn strcmp(a: *const u8, b: *const u8) -> i32
    fn malloc(size: u64) -> ptr
    fn free(p: ptr)
    fn memcpy(dst: ptr, src: ptr, n: u64) -> ptr
}

@unsafe
fn main() {
    let s = "hello"
    let length = strlen(s.as_ptr())
    println(f"Length: {length}")   // Length: 5
}
```

## Wrapping C Libraries

Good practice: wrap unsafe FFI calls in a safe API.

```fajar
// Raw C bindings
extern "C" {
    fn c_compress(src: *const u8, src_len: u64, dst: *mut u8, dst_len: *mut u64) -> i32
    fn c_decompress(src: *const u8, src_len: u64, dst: *mut u8, dst_len: *mut u64) -> i32
}

// Safe wrapper
mod compress {
    pub fn compress(data: [u8]) -> Result<[u8], str> {
        let mut out_len: u64 = len(data) * 2
        let mut output = [0u8; out_len]

        @unsafe {
            let result = c_compress(
                data.as_ptr(),
                len(data) as u64,
                output.as_mut_ptr(),
                &mut out_len
            )
            if result != 0 {
                return Err("compression failed")
            }
        }

        Ok(output[0..out_len])
    }
}

fn main() {
    let data = [1, 2, 3, 4, 5]
    match compress::compress(data) {
        Ok(compressed) => println(f"Compressed to {len(compressed)} bytes"),
        Err(e) => println(f"Error: {e}")
    }
}
```

## Callbacks: Passing Fajar Functions to C

Some C libraries accept function pointers as callbacks:

```fajar
extern "C" {
    fn qsort(base: ptr, count: u64, size: u64, cmp: fn(*const ptr, *const ptr) -> i32)
}

@unsafe
fn compare_ints(a: *const ptr, b: *const ptr) -> i32 {
    let va = *(a as *const i32)
    let vb = *(b as *const i32)
    va - vb
}

@unsafe
fn main() {
    let mut data = [5i32, 3, 8, 1, 9, 2]
    qsort(
        data.as_mut_ptr() as ptr,
        len(data) as u64,
        4,   // sizeof(i32)
        compare_ints
    )
    println(data)   // [1, 2, 3, 5, 8, 9]
}
```

## Linking Libraries

Specify libraries to link in `fj.toml`:

```toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies.ffi]
link = ["sqlite3", "z", "ssl"]
include = ["/usr/include"]
```

## Safety Guidelines

FFI crosses the safety boundary. Follow these rules:

1. **Always wrap** raw FFI in a safe public API
2. **Validate** all data before passing to C (null checks, bounds)
3. **Document** which C library version you target
4. **Free** memory allocated by C using the C allocator (not Fajar's)
5. **Never** expose raw pointers in your public API

```fajar
// BAD: exposing raw FFI
pub fn get_data() -> ptr { ... }

// GOOD: safe wrapper
pub fn get_data() -> Result<[u8], str> { ... }
```

## Exercises

### Exercise 13.1: Math FFI (*)

Declare `extern "C"` bindings for `sin`, `cos`, and `pow` from the C math library. Write a function `hypotenuse(a: f64, b: f64) -> f64` that computes `sqrt(a*a + b*b)` using the C `sqrt`. Test with `a=3.0, b=4.0`.

**Expected output:**

```
5.0
```

### Exercise 13.2: Safe String Length (**)

Write an `extern "C"` binding for `strlen`. Wrap it in a safe function `safe_strlen(s: str) -> i64` that handles the pointer conversion internally. The caller should never see a raw pointer. Test with "hello world".

**Expected output:**

```
Length: 11
```
