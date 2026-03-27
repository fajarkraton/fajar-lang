# Troubleshooting

Common errors and their solutions when working with Fajar Lang.

## Build Errors

### 1. "linker not found" during cargo build

**Cause:** Missing C toolchain on the system.

**Fix:**

```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# macOS
xcode-select --install

# Fedora
sudo dnf groupinstall "Development Tools"
```

### 2. "cargo build fails with Cranelift errors"

**Cause:** Rust version too old. Fajar Lang requires Rust 1.87+.

**Fix:**

```bash
rustup update stable
rustc --version   # Should be 1.87.0 or newer
```

### 3. "feature native not found"

**Cause:** Running native codegen tests without the feature flag.

**Fix:**

```bash
cargo test --features native
```

## Compilation Errors

### 4. SE004 TypeMismatch: expected i64, found i32

**Cause:** Fajar Lang does not perform implicit integer widening.

**Fix:** Use explicit cast:

```fajar
let x: i32 = 42
let y: i64 = x as i64   // Explicit cast required
```

### 5. KE001 HeapAllocInKernel

**Cause:** Using heap-allocating types (String, Vec) inside `@kernel` context.

**Fix:** Use stack-allocated arrays or fixed-size buffers instead:

```fajar
@kernel
fn good() {
    let buffer: [u8; 256] = [0; 256]   // Stack allocation -- OK
}
```

### 6. KE002 TensorInKernel

**Cause:** Using tensor operations inside `@kernel` context.

**Fix:** Move tensor code to `@device` context and use a `@safe` bridge:

```fajar
@device fn infer(data: Tensor) -> i32 { argmax(model.forward(data)) }
@kernel fn read_hw() -> [f32; 4] { /* hardware read */ }
@safe fn bridge() { let raw = read_hw(); infer(Tensor::from_slice(raw)); }
```

### 7. DE001 RawPointerInDevice

**Cause:** Using raw pointers (`*mut T`) inside `@device` context.

**Fix:** Raw pointers belong in `@kernel` or `@unsafe`. Restructure so
the `@device` function receives data by value or reference.

### 8. ME001 UseAfterMove

**Cause:** Accessing a variable after it was moved to another binding.

**Fix:** Clone the value before moving, or restructure to avoid the move:

```fajar
let data = create_data()
let copy = data.clone()    // Clone before moving
process(data)              // data is moved
use_copy(copy)             // copy is still valid
```

### 9. SE009 UnusedVariable (warning)

**Cause:** A variable is declared but never used.

**Fix:** Prefix with underscore to suppress, or remove the variable:

```fajar
let _unused = compute()   // Underscore prefix silences warning
```

## Runtime Errors

### 10. RE003 StackOverflow

**Cause:** Infinite recursion or very deep recursion.

**Fix:** The default recursion limit is 64 (debug) / 1024 (release).
Rewrite recursive algorithms iteratively, or ensure a proper base case:

```fajar
fn fib(n: i32) -> i32 {
    if n <= 1 { return n }   // Base case -- must exist
    fib(n - 1) + fib(n - 2)
}
```

### 11. RE005 IndexOutOfBounds

**Cause:** Accessing an array with an index beyond its length.

**Fix:** Check length before access, or use `.get()` which returns `Option`:

```fajar
let arr = [1, 2, 3]
if idx < len(arr) {
    println(arr[idx])
}
// Or:
match arr.get(idx) {
    Some(val) => println(val),
    None => println("Index out of range"),
}
```

### 12. RE001 IntegerOverflow

**Cause:** Arithmetic exceeds the type's range (checked in debug mode).

**Fix:** Use a larger type or check before arithmetic:

```fajar
let x: i64 = large_value as i64   // Use wider type
let safe = if x < 2147483647 { x + 1 } else { x }
```

## REPL Issues

### 13. "Variable not found" in REPL

**Cause:** The REPL uses `analyze_with_known()` to track cross-line state.
This can fail if a previous line had an error.

**Fix:** Restart the REPL with `fj repl` or re-declare the variable.

### 14. Multi-line input not working

**Cause:** The REPL expects the opening brace `{` on the first line.

**Fix:** Start the block on the same line as the declaration:

```
fj> fn double(x: i32) -> i32 {
...     x * 2
... }
```

## Test Issues

### 15. "Random test failures"

**Cause:** Tests sharing state through a global interpreter instance.

**Fix:** Each test should create a fresh `Interpreter::new()`:

```fajar
@test
fn test_something() {
    // Each test gets its own state
    let result = eval("1 + 2")
    assert_eq(result, 3)
}
```

### 16. Gradient mismatch in ML tests

**Cause:** Floating-point precision differences.

**Fix:** Use approximate comparison with epsilon tolerance:

```fajar
assert(abs(actual - expected) < 1e-4)
```

## Performance Issues

### 17. Program runs slowly

**Cause:** Running in interpreter mode instead of native compilation.

**Fix:**

```bash
fj build --release       # Use Cranelift native compilation
```

The native-compiled binary is typically 10-50x faster than the tree-walker.

### 18. Slow compilation

**Cause:** Full pipeline with code generation.

**Fix:** Use `fj check` (or `cargo check`) for quick type-checking
without generating code.

## Platform Issues

### 19. "Permission denied" when running fj

**Cause:** Binary not marked executable.

**Fix:**

```bash
chmod +x /usr/local/bin/fj
```

### 20. Cross-compilation target not found

**Cause:** Missing cross-compilation toolchain.

**Fix:**

```bash
# For ARM64
sudo apt-get install gcc-aarch64-linux-gnu

# For RISC-V
sudo apt-get install gcc-riscv64-linux-gnu
```

Then set the linker in `fj.toml`:

```toml
[build.cross]
linker = "aarch64-linux-gnu-gcc"
```
