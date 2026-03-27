# Keywords

All reserved keywords in Fajar Lang, grouped by category.

## Control Flow

| Keyword | Description | Example |
|---------|-------------|---------|
| `if` | Conditional branch | `if x > 0 { println("positive") }` |
| `else` | Alternative branch | `if cond { a } else { b }` |
| `match` | Pattern matching | `match x { 0 => "zero", _ => "other" }` |
| `while` | Conditional loop | `while i < 10 { i = i + 1 }` |
| `for` | Iterator loop | `for item in list { println(item) }` |
| `loop` | Infinite loop | `loop { if done { break } }` |
| `in` | Iteration source | `for x in 0..10 { ... }` |
| `return` | Return from function | `return 42` |
| `break` | Exit loop | `break` |
| `continue` | Skip to next iteration | `continue` |
| `async` | Async function | `async fn fetch() -> Response { ... }` |
| `await` | Await future | `let data = fetch().await` |

## Declarations

| Keyword | Description | Example |
|---------|-------------|---------|
| `let` | Variable binding | `let x = 42` |
| `mut` | Mutable modifier | `let mut count = 0` |
| `fn` | Function definition | `fn add(a: i32, b: i32) -> i32 { a + b }` |
| `struct` | Struct definition | `struct Point { x: f64, y: f64 }` |
| `enum` | Enum definition | `enum Color { Red, Green, Blue }` |
| `impl` | Implementation block | `impl Point { fn origin() -> Point { ... } }` |
| `trait` | Trait definition | `trait Display { fn show(&self) -> str }` |
| `type` | Type alias | `type Pair = (i64, i64)` |
| `const` | Compile-time constant | `const MAX: i64 = 100` |
| `static` | Static variable | `static mut COUNTER: i64 = 0` |
| `dyn` | Dynamic dispatch | `let obj: dyn Display = value` |

## Type Keywords

| Keyword | Description | Example |
|---------|-------------|---------|
| `bool` | Boolean type | `let flag: bool = true` |
| `i8` ... `i128` | Signed integers | `let x: i32 = -42` |
| `u8` ... `u128` | Unsigned integers | `let y: u8 = 255` |
| `isize` | Pointer-sized signed | `let idx: isize = -1` |
| `usize` | Pointer-sized unsigned | `let len: usize = 10` |
| `f32` | 32-bit float | `let t: f32 = 3.14` |
| `f64` | 64-bit float | `let pi: f64 = 3.14159` |
| `str` | String type | `let name: str = "Fajar"` |
| `char` | Character type | `let c: char = 'A'` |
| `void` | Unit/no value | `fn log(msg: str) -> void { ... }` |
| `never` | Bottom type | `fn abort() -> never { panic("!") }` |

## ML Keywords

| Keyword | Description | Example |
|---------|-------------|---------|
| `tensor` | Tensor type | `let t: tensor = zeros(3, 4)` |
| `grad` | Gradient access | `let g = grad(weights)` |
| `loss` | Loss keyword | `let l = loss(pred, target)` |
| `layer` | Layer type | `let fc: layer = Dense(784, 10)` |
| `model` | Model type | `let m: model = Sequential(layers)` |

## OS Keywords

| Keyword | Description | Example |
|---------|-------------|---------|
| `ptr` | Pointer type | `let p: ptr = mem_alloc(64)` |
| `addr` | Address type | `let a: addr = 0xDEAD_BEEF` |
| `page` | Page type | `page_map(virt, phys, flags)` |
| `region` | Memory region | `let r: region = stack_region()` |
| `irq` | Interrupt | `irq_register(0, handler)` |
| `syscall` | System call | `syscall_define(1, sys_write)` |

## Module Keywords

| Keyword | Description | Example |
|---------|-------------|---------|
| `use` | Import | `use std::math` |
| `mod` | Module declaration | `mod utils` |
| `pub` | Public visibility | `pub fn api_call() { ... }` |
| `extern` | External linkage | `extern fn c_function()` |
| `as` | Rename / type cast | `use math::sqrt as root` |
| `where` | Trait bounds | `fn f<T>(x: T) where T: Display` |

## Literal Keywords

| Keyword | Description | Example |
|---------|-------------|---------|
| `true` | Boolean true | `let active = true` |
| `false` | Boolean false | `let done = false` |
| `null` | Null value | `let ptr = null` |

## Annotations

| Keyword | Description | Example |
|---------|-------------|---------|
| `@kernel` | Kernel context | `@kernel fn irq_handler() { ... }` |
| `@device` | Device/ML context | `@device fn forward(x: Tensor) { ... }` |
| `@safe` | Safe context (default) | `@safe fn process() { ... }` |
| `@unsafe` | Unrestricted context | `@unsafe fn raw_access() { ... }` |
| `@ffi` | Foreign function | `@ffi fn c_malloc(size: usize) -> *mut u8` |
| `@test` | Test function | `@test fn test_add() { assert_eq(1+1, 2) }` |
| `@should_panic` | Expect panic in test | `@test @should_panic fn test_div_zero() { ... }` |
| `@ignore` | Skip test | `@test @ignore fn slow_test() { ... }` |
| `@entry` | Entry point | `@entry fn main() { ... }` |

## Contextual Keywords

The OS and ML keywords (`tensor`, `grad`, `ptr`, `addr`, `page`, `region`, `irq`, `syscall`, `loss`, `layer`, `model`) are contextual -- they can be used as parameter names and in expressions where the context is unambiguous.

```fajar
fn set_layer(layer: i64) {   // "layer" as parameter name is OK
    println(layer)
}
```
