# Keywords

## Control Flow

| Keyword | Description |
|---------|-------------|
| `if` | Conditional branch |
| `else` | Alternative branch |
| `match` | Pattern matching |
| `while` | Conditional loop |
| `for` | Iterator loop |
| `in` | Iterator binding |
| `loop` | Infinite loop |
| `break` | Exit loop |
| `continue` | Skip to next iteration |
| `return` | Return from function |

## Declarations

| Keyword | Description |
|---------|-------------|
| `let` | Variable binding |
| `mut` | Mutable binding |
| `fn` | Function definition |
| `struct` | Struct type |
| `enum` | Enum type |
| `impl` | Implementation block |
| `trait` | Trait definition |
| `type` | Type alias |
| `const` | Constant |
| `union` | Union type |

## Types

| Keyword | Description |
|---------|-------------|
| `bool` | Boolean (true/false) |
| `i8`-`i128` | Signed integers |
| `u8`-`u128` | Unsigned integers |
| `isize`, `usize` | Pointer-sized integers |
| `f32`, `f64` | Floating point |
| `str` | String |
| `char` | Character |
| `void` | No value |
| `never` | Never returns |

## ML Keywords

| Keyword | Description |
|---------|-------------|
| `tensor` | Tensor type |
| `grad` | Gradient access |
| `loss` | Loss value |
| `layer` | Neural network layer |
| `model` | ML model |

## OS Keywords

| Keyword | Description |
|---------|-------------|
| `ptr` | Raw pointer |
| `addr` | Memory address |
| `page` | Memory page |
| `region` | Memory region |
| `irq` | Interrupt request |
| `syscall` | System call |

## Module Keywords

| Keyword | Description |
|---------|-------------|
| `use` | Import |
| `mod` | Module declaration |
| `pub` | Public visibility |
| `extern` | External linkage |
| `as` | Type cast / rename |

## Literals

| Keyword | Description |
|---------|-------------|
| `true` | Boolean true |
| `false` | Boolean false |
| `null` | Null value |

## Context Annotations

| Annotation | Description |
|------------|-------------|
| `@kernel` | OS kernel context (no heap, no tensor) |
| `@device` | ML device context (no raw pointer, no IRQ) |
| `@safe` | Default safe context |
| `@unsafe` | Full access (all features) |
| `@ffi` | Foreign function interface |

## Concurrency

| Keyword | Description |
|---------|-------------|
| `async` | Asynchronous function |
| `await` | Wait for async result |
