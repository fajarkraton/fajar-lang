# Skills — Fajar Lang v1.0 Implementation Patterns

> Reusable patterns, algorithms, and techniques for implementing Fajar Lang.

---

## 1. Cranelift Native Code Generation

### 1.1 Architecture
```
Program (AST)
    │
    ▼
┌──────────────────┐
│  IR Lowering     │  AST → Cranelift IR (CLIF)
│  (src/codegen/)  │
├──────────────────┤
│  Type:           │  FunctionBuilder + InstBuilder
│  Input:          │  &Program (analyzed AST)
│  Output:         │  ObjectModule with compiled functions
│  Pattern:        │  One pass per function definition
└──────────────────┘
    │
    ▼
┌──────────────────┐
│  JIT / AOT       │
│  Execution       │
├──────────────────┤
│  JIT:  cranelift-jit    → fn() pointer
│  AOT:  cranelift-object → .o file → link → binary
└──────────────────┘
```

### 1.2 Value Representation in Native Code
```
i64     → cranelift I64
f64     → cranelift F64
bool    → cranelift I8 (0 or 1)
char    → cranelift I32 (Unicode scalar)
*T      → cranelift I64 (raw pointer)

String  → struct { ptr: *u8, len: usize, cap: usize }
Array   → struct { ptr: *T, len: usize, cap: usize }
Tensor  → struct { data: *f64, shape: *usize, ndim: usize }
Struct  → flat layout (field1, field2, ...)
Enum    → tagged union { tag: u8, data: [u8; MAX_VARIANT_SIZE] }
```

### 1.3 Function Compilation Pattern
```rust
fn compile_function(&mut self, fndef: &FnDef) -> Result<(), CodegenError> {
    // 1. Create function signature
    let mut sig = self.module.make_signature();
    for param in &fndef.params {
        sig.params.push(AbiParam::new(self.lower_type(&param.ty)));
    }
    sig.returns.push(AbiParam::new(self.lower_type(&fndef.return_type)));

    // 2. Declare function
    let func_id = self.module.declare_function(
        &fndef.name, Linkage::Export, &sig
    )?;

    // 3. Build function body
    let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);

    // 4. Compile each expression
    let result = self.compile_expr(&mut builder, &fndef.body)?;

    // 5. Return
    builder.ins().return_(&[result]);
    builder.seal_all_blocks();
    builder.finalize();

    // 6. Define in module
    self.module.define_function(func_id, &mut self.ctx)?;
    self.module.clear_context(&mut self.ctx);
    Ok(())
}
```

---

## 2. Generics — Monomorphization

### 2.1 Strategy
```
Source:     fn max<T: Ord>(a: T, b: T) -> T { if a > b { a } else { b } }
Call site:  max(1, 2)        → generates max_i64(a: i64, b: i64) -> i64
Call site:  max(1.0, 2.0)    → generates max_f64(a: f64, b: f64) -> f64
```

### 2.2 Algorithm
```
1. Parse generic function → store as template (not compiled)
2. At each call site:
   a. Infer type arguments from actual arguments
   b. Check trait bounds (T: Ord → does i64 implement Ord?)
   c. Substitute type params in function body
   d. Compile specialized version
   e. Cache: (fn_name, [concrete_types]) → compiled_fn_id
3. If same specialization exists → reuse cached version
```

### 2.3 Type Inference at Call Site
```rust
fn infer_type_args(
    generic_params: &[GenericParam],
    formal_params: &[Param],
    actual_args: &[Value],
) -> Result<HashMap<String, Type>, TypeError> {
    let mut bindings = HashMap::new();
    for (formal, actual) in formal_params.iter().zip(actual_args) {
        unify(&formal.ty, &actual.type_of(), &mut bindings)?;
    }
    // Verify all generic params are bound
    for gp in generic_params {
        if !bindings.contains_key(&gp.name) {
            return Err(TypeError::CannotInfer(gp.name.clone()));
        }
    }
    Ok(bindings)
}
```

---

## 3. Trait System

### 3.1 Design
```fajar
trait Sensor {
    fn read(&self) -> [f32; 4]
    fn calibrate(&mut self) -> void
}

impl Sensor for Accelerometer {
    fn read(&self) -> [f32; 4] { ... }
    fn calibrate(&mut self) -> void { ... }
}

// Static dispatch (monomorphized) — default for embedded
fn process<S: Sensor>(sensor: &S) -> Tensor {
    let data = sensor.read()
    Tensor::from_slice(data)
}
```

### 3.2 Trait Resolution Algorithm
```
1. For method call `obj.method()`:
   a. Look up obj's concrete type
   b. Search impl blocks for that type
   c. If not found, search trait impls
   d. If multiple matches, error (ambiguous)
   e. If no match, error (undefined method)

2. For trait bound `T: Sensor`:
   a. At monomorphization, verify concrete type has impl Sensor
   b. If not, error with helpful message
```

### 3.3 Vtable Layout (for dynamic dispatch — future)
```
// Not needed for v1.0 (static dispatch only)
// But design it now for forward compatibility
struct VTable {
    drop_fn: fn(*mut ()),
    size: usize,
    align: usize,
    methods: [fn(*const ()); N],  // method pointers
}
```

---

## 4. Ownership & Borrow Checking

### 4.1 Move Semantics
```
Rule: Every value has exactly ONE owner.
      When ownership transfers (move), the original is invalidated.

let x = Tensor::zeros([3, 3])
let y = x                        // x is MOVED to y
println(x)                       // ERROR ME001: use after move
```

### 4.2 Borrow Rules
```
Rule 1: Many immutable borrows (&T) OR one mutable borrow (&mut T)
Rule 2: Borrows cannot outlive the owner
Rule 3: No borrowing across @kernel/@device boundary

let x = [1, 2, 3]
let r1 = &x          // OK: immutable borrow
let r2 = &x          // OK: multiple immutable borrows
let r3 = &mut x      // ERROR ME003: cannot borrow mutably while immutably borrowed
```

### 4.3 Borrow Checker Algorithm (Simplified for v1.0)
```
For each function:
  1. Build control flow graph (CFG)
  2. For each variable, track:
     - State: Owned | Moved | Borrowed(count) | MutBorrowed
     - Last use position (for drop insertion)
  3. At each statement:
     - Assignment `let y = x`: if x is non-Copy, mark x as Moved
     - Borrow `&x`: increment borrow count, check no MutBorrow active
     - MutBorrow `&mut x`: check no borrows active, mark MutBorrowed
     - Use of Moved variable: emit ME001
     - MutBorrow of Borrowed: emit ME003
  4. At scope exit: drop all Owned variables (insert drop calls)
```

### 4.4 Copy Types (No Move)
```
Copy types (always copied, never moved):
  i8, i16, i32, i64, i128
  u8, u16, u32, u64, u128
  f32, f64
  bool, char
  ()  (unit type)
  VirtAddr, PhysAddr (OS address types)

Non-Copy types (moved):
  String, Array, Tensor, HashMap
  Struct (unless all fields are Copy)
  Enum (unless all variants are Copy)
  Function, Closure
```

---

## 5. FFI — C Interop

### 5.1 Calling C from Fajar Lang
```fajar
@ffi("C")
extern fn printf(fmt: *const u8, ...) -> i32

@ffi("C")
extern fn malloc(size: usize) -> *mut u8

// Usage in @unsafe context only
@unsafe fn allocate(n: usize) -> *mut u8 {
    let ptr = malloc(n)
    if ptr == null {
        panic("allocation failed")
    }
    ptr
}
```

### 5.2 Implementation Pattern
```rust
// Using libffi for dynamic C calls
fn call_ffi(
    &mut self,
    lib_name: &str,
    fn_name: &str,
    args: &[Value],
    ret_type: &Type,
) -> Result<Value, RuntimeError> {
    let lib = unsafe { Library::new(lib_name)? };
    let func: Symbol<unsafe extern "C" fn() -> i64> =
        unsafe { lib.get(fn_name.as_bytes())? };

    // Marshal args from Value → C types
    let c_args = self.marshal_args(args)?;

    // Call
    let result = unsafe { func(/* marshaled args */) };

    // Unmarshal result
    self.unmarshal_result(result, ret_type)
}
```

---

## 6. Cross-Compilation for Embedded

### 6.1 Target Triples
```
Target:                   Use case:
aarch64-unknown-none      ARM64 bare-metal (Raspberry Pi, drones)
thumbv7em-none-eabihf     ARM Cortex-M4/M7 (STM32, sensor boards)
riscv64gc-unknown-none    RISC-V 64-bit (emerging IoT)
x86_64-unknown-none       x86-64 bare-metal (custom OS)
wasm32-unknown-unknown    WebAssembly (edge inference)
```

### 6.2 no_std Support
```rust
// Generated code for embedded targets:
#![no_std]
#![no_main]

// No heap allocator by default in @kernel context
// Only stack allocation + static buffers

// Tensor ops use fixed-size stack arrays:
// let t: Tensor<f32, [3, 3]> = ...  // 9 * 4 = 36 bytes on stack
```

### 6.3 Memory Budget
```
For embedded targets, compile with memory budget:
  --max-stack=8192       # 8KB stack
  --max-static=16384     # 16KB static data
  --no-heap              # Enforce @kernel everywhere

Compiler checks at compile time:
  - Stack frame size for each function
  - Total static data size
  - No heap allocation calls
```

---

## 7. Autograd — Tape-Based Reverse Mode

### 7.1 Computation Graph
```
Forward pass records operations:
  let a = tensor([1.0, 2.0])       // leaf (requires_grad=true)
  let b = tensor([3.0, 4.0])       // leaf
  let c = a * b                     // recorded: Mul(a_id, b_id) → c_id
  let d = c.sum()                   // recorded: Sum(c_id) → d_id
  d.backward()                      // triggers reverse pass

Tape: [
  Op::Mul { inputs: [a_id, b_id], output: c_id },
  Op::Sum { input: c_id, output: d_id },
]
```

### 7.2 Backward Pass Algorithm
```
1. Set grad[output] = 1.0 (seed gradient)
2. Walk tape in REVERSE order:
   For each Op:
     Mul(a, b) → c:
       grad[a] += grad[c] * b.data
       grad[b] += grad[c] * a.data
     Sum(x) → y:
       grad[x] += broadcast(grad[y], x.shape)
     MatMul(a, b) → c:
       grad[a] += grad[c] @ b.T
       grad[b] += a.T @ grad[c]
     ReLU(x) → y:
       grad[x] += grad[y] * (x.data > 0)
3. Return grad map: TensorId → gradient array
```

---

## 8. Embedded ML Inference Pattern

### 8.1 Quantized Inference (INT8)
```fajar
@device fn quantize(t: Tensor<f32>) -> Tensor<i8> {
    let scale = t.abs().max() / 127.0
    (t / scale).round().cast::<i8>()
}

@device fn dequantize(t: Tensor<i8>, scale: f32) -> Tensor<f32> {
    t.cast::<f32>() * scale
}

// INT8 matmul for embedded (no FPU needed)
@device fn int8_matmul(a: Tensor<i8>, b: Tensor<i8>) -> Tensor<i32> {
    // Uses integer-only arithmetic
    tensor_matmul(a.cast::<i32>(), b.cast::<i32>())
}
```

### 8.2 Model Export for Embedded
```fajar
// Train on host (f32)
@device fn train(model: &mut Model, data: DataLoader) {
    for batch in data {
        let pred = model.forward(batch.x)
        let loss = cross_entropy(pred, batch.y)
        loss.backward()
        optimizer.step()
    }
}

// Export quantized model for target
fn export(model: &Model, path: str) {
    let quantized = model.quantize()  // f32 → i8
    write_file(path, quantized.serialize())
}

// Deploy on embedded target (no FPU, no heap)
@kernel fn predict(input: [i8; 784]) -> i8 {
    let model = include_model!("model.bin")  // compiled into binary
    model.forward_int8(input)
}
```

---

## 9. Error Recovery in Parser

### 9.1 Synchronization Points
```
When parser encounters error:
  1. Record the error with span
  2. Skip tokens until a "synchronization point":
     - Semicollon / newline (statement boundary)
     - `fn` / `struct` / `enum` / `impl` (item boundary)
     - `}` at matching depth (block boundary)
  3. Resume parsing from synchronization point
  4. Continue collecting more errors
```

### 9.2 Pattern
```rust
fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
    match self.parse_statement_inner() {
        Ok(stmt) => Ok(stmt),
        Err(e) => {
            self.errors.push(e);
            self.synchronize();  // skip to next statement
            Err(e)
        }
    }
}

fn synchronize(&mut self) {
    while !self.is_at_end() {
        if self.previous().kind == TokenKind::Newline {
            return;
        }
        match self.peek().kind {
            TokenKind::Fn | TokenKind::Struct | TokenKind::Enum
            | TokenKind::Let | TokenKind::Const | TokenKind::If
            | TokenKind::While | TokenKind::For | TokenKind::Return => return,
            _ => { self.advance(); }
        }
    }
}
```

---

## 10. Debug Information (DWARF)

### 10.1 Source Mapping
```
Every compiled instruction maps back to source:
  Instruction address → (file, line, column)

Generated via Cranelift's SourceLoc:
  builder.set_srcloc(SourceLoc::new(span.start as u32));
```

### 10.2 Variable Location
```
For debugger support (GDB/LLDB):
  Each local variable → register or stack slot
  Each parameter → argument register

DWARF DIE entries:
  DW_TAG_subprogram → function
  DW_TAG_variable → local variable
  DW_TAG_formal_parameter → function parameter
```

---

*V1_SKILLS.md v1.0 — Created 2026-03-05*
