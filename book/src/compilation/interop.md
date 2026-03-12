# Cross-Language Interop

Fajar Lang generates bindings for C/C++, Python, and WebAssembly, enabling integration with existing codebases.

## C/C++ Bindings

Generate C headers from Fajar Lang types and functions:

```bash
fj bindgen --target c program.fj -o fajar_api.h
```

Generated header:
```c
#ifndef FAJAR_API_H
#define FAJAR_API_H

#include <stdint.h>

typedef struct {
    double x;
    double y;
} Point;

extern int64_t fibonacci(int64_t n);
extern Point make_point(double x, double y);

#endif
```

### Calling C from Fajar

```fajar
@ffi("C")
extern fn printf(fmt: *const u8, ...) -> i32

fn main() {
    printf("Hello from C: %d\n", 42)
}
```

### Struct Layout

`#[repr(C)]` ensures C-compatible struct layout:

```fajar
#[repr(C)]
struct SensorData {
    timestamp: u64,
    values: [f32; 4],
}
```

## Python Bindings

Generate Python package with type stubs:

```bash
fj bindgen --target python program.fj -o fajar_py/
```

Generates:
- `__init__.py` — Python module with ctypes bindings
- `fajar_py.pyi` — `.pyi` type stubs for IDE support

```python
import fajar_py

result = fajar_py.fibonacci(30)
point = fajar_py.make_point(1.0, 2.0)
```

### NumPy Bridge

Tensors are automatically bridged to NumPy ndarrays:

```python
import numpy as np
import fajar_py

data = np.random.randn(28, 28).astype(np.float32)
result = fajar_py.infer(data)  # Fajar tensor ↔ NumPy ndarray
```

## WebAssembly Component Model

Generate WIT (WebAssembly Interface Types):

```bash
fj bindgen --target wasm program.fj -o api.wit
```

```wit
package fajar:api

interface math {
    fibonacci: func(n: s64) -> s64
    make-point: func(x: f64, y: f64) -> point

    record point {
        x: f64,
        y: f64,
    }
}

world fajar-math {
    export math
}
```

## Type Mapping

| Fajar Type | C | Python | Wasm |
|-----------|---|--------|------|
| `i8`-`i64` | `int8_t`-`int64_t` | `int` | `s8`-`s64` |
| `u8`-`u64` | `uint8_t`-`uint64_t` | `int` | `u8`-`u64` |
| `f32`/`f64` | `float`/`double` | `float` | `f32`/`f64` |
| `bool` | `bool` | `bool` | `bool` |
| `str` | `const char*` | `str` | `string` |
| `Tensor` | `float*` + dims | `np.ndarray` | `list<f32>` |
| `struct` | `struct` | `dataclass` | `record` |
| `enum` | tagged union | `enum.Enum` | `variant` |
