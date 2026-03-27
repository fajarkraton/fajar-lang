# FFI Guide

Fajar Lang supports calling C functions, exposing functions to C callers,
and bridging with Python. This guide covers foreign function interface patterns.

## Calling C Functions

Use `extern` blocks to declare C functions:

```fajar
extern "C" {
    fn printf(fmt: *const u8, ...) -> i32
    fn malloc(size: usize) -> *mut u8
    fn free(ptr: *mut u8)
    fn strlen(s: *const u8) -> usize
}

@unsafe
fn call_c() {
    let msg = "Hello from Fajar Lang\n\0"
    printf(msg.as_ptr())
}
```

## Linking C Libraries

In `fj.toml`:

```toml
[build.ffi]
link = ["m", "ssl", "crypto"]       # -lm -lssl -lcrypto
include = ["/usr/include/openssl"]
```

## Exporting Functions to C

Mark functions with `extern "C"` to make them callable from C:

```fajar
@ffi
extern "C" fn fj_add(a: i32, b: i32) -> i32 {
    a + b
}

@ffi
extern "C" fn fj_process(data: *const f32, len: usize) -> f64 {
    let mut sum = 0.0
    for i in 0..len {
        sum += unsafe { *data.offset(i as i32) } as f64
    }
    sum
}
```

Compile as a shared library:

```bash
fj build --lib --output libfajar.so
```

Use from C:

```c
// Link with -lfajar
extern int fj_add(int a, int b);
int result = fj_add(3, 4);  // 7
```

## Type Mapping

| Fajar Lang | C | Size |
|------------|---|------|
| `i8` | `int8_t` | 1 byte |
| `i32` | `int32_t` | 4 bytes |
| `i64` | `int64_t` | 8 bytes |
| `f32` | `float` | 4 bytes |
| `f64` | `double` | 8 bytes |
| `bool` | `_Bool` | 1 byte |
| `*const T` | `const T*` | pointer |
| `*mut T` | `T*` | pointer |
| `str` | `const char*` | pointer |

## Python Bridge

Use the NumPy bridge for zero-copy tensor sharing:

```fajar
use fj_python::numpy

@ffi
fn process_numpy(data: NumpyBuffer) -> Tensor {
    let tensor = Tensor::from_numpy(data)
    let result = matmul(tensor, weights)
    result
}
```

From Python:

```python
import numpy as np
import fajar_lang as fj

data = np.array([[1.0, 2.0], [3.0, 4.0]], dtype=np.float32)
result = fj.process_numpy(data)
print(result)  # NumPy array back
```

## Callback Registration

Pass Fajar Lang functions to C as callbacks:

```fajar
extern "C" {
    fn register_callback(cb: fn(i32) -> i32)
}

fn my_handler(x: i32) -> i32 {
    x * 2
}

@unsafe
fn setup() {
    register_callback(my_handler)
}
```

## Safety Rules

- All FFI calls require `@unsafe` or `@ffi` context
- Raw pointers from C must be validated before use
- String data from C must be null-terminated
- Memory allocated by C must be freed by C (and vice versa)
