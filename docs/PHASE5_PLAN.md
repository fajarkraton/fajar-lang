# PHASE 5 IMPLEMENTATION PLAN — Fajar Lang

> Comprehensive plan: Pre-Phase 5 Gap Fixes + Phase 5 Tooling & Compiler Backend.
> Created: 2026-03-05 | Status: PLANNING

---

## Status Overview

```
Current State:    Phase 4 COMPLETE, 660 tests passing
Gap Fixes Needed: 4 sprints (G.1 - G.4) before Phase 5
Phase 5 Sprints:  6 sprints (5.1 - 5.6)
Total Effort:     ~20-24 weeks estimated
```

---

## PART 1: PRE-PHASE 5 GAP FIXES

These are features documented in FAJAR_LANG_SPEC but NOT yet implemented.
The AST already parses them — they need interpreter, analyzer, and test coverage.

---

### Sprint G.1 — impl Blocks & Method Dispatch
**Goal:** `impl Type { fn method() {} }` works, methods callable via `obj.method()`
**Duration:** 2-3 sessions | **Priority:** CRITICAL

**Current State:**
- Parser: `ImplBlock` AST node exists (name, generic_params, trait_name, target_type, methods)
- Interpreter: `Item::ImplBlock(_) => Ok(Value::Null)` — completely ignored
- `eval_method_call()` only handles hardcoded String/Array methods
- Type checker: does NOT register impl methods

**Tasks:**

#### G.1.1 — Interpreter: Register impl methods
```
File: src/interpreter/eval.rs
```
- [ ] When evaluating `Item::ImplBlock`, register each method in environment
  - Key format: `"{TypeName}::{method_name}"` (e.g., `"Point::distance"`)
  - Value: `Value::Function(FnValue)` with `self` as implicit first param
- [ ] Store impl method registry in `Interpreter` struct: `impl_methods: HashMap<(String, String), FnValue>`
  - Key: `(type_name, method_name)`

#### G.1.2 — Interpreter: Method dispatch
```
File: src/interpreter/eval.rs → eval_method_call()
```
- [ ] Before the hardcoded String/Array match, check impl_methods registry
- [ ] Look up `(receiver.type_name(), method_name)` in impl_methods
- [ ] If found, call with receiver as first argument (self)
- [ ] Fall through to hardcoded methods if not found

#### G.1.3 — Type checker: Register impl methods
```
File: src/analyzer/type_check.rs
```
- [ ] In two-pass analysis, first pass registers impl methods in SymbolTable
- [ ] Second pass verifies method call signatures

#### G.1.4 — Tests
- [ ] Unit test: define struct + impl, call method → correct result
- [ ] Unit test: impl with multiple methods
- [ ] Unit test: method accessing struct fields via self
- [ ] Unit test: method not found → RuntimeError
- [ ] Integration test in tests/eval_tests.rs

**Example that must work after G.1:**
```fajar
struct Point { x: f64, y: f64 }
impl Point {
    fn new(x: f64, y: f64) -> Point {
        Point { x: x, y: y }
    }
    fn distance(self) -> f64 {
        (self.x * self.x + self.y * self.y)
    }
}
let p = Point { x: 3.0, y: 4.0 }
println(p.distance())  // 25.0
```

**Exit Criteria:** impl methods work for all user-defined structs. cargo test passes.

---

### Sprint G.2 — Option/Result Types & ? Operator
**Goal:** `Option<T>` and `Result<T,E>` as first-class types, `?` propagation works
**Duration:** 2-3 sessions | **Priority:** HIGH

**Current State:**
- Parser: `Expr::Try { expr }` exists in AST (the `?` operator)
- Interpreter: `Expr::Try { .. } => Err(Unsupported("cast and try expressions not yet supported"))`
- No `Option` or `Result` types in Value enum
- Enum system exists and works (variants with data)

**Approach:** Implement Option/Result as regular enums, not special Value variants.
The existing enum system supports `Enum { variant, data }` — we register Option and Result
as built-in enums during interpreter initialization.

**Tasks:**

#### G.2.1 — Register built-in Option/Result enums
```
File: src/interpreter/eval.rs → register_builtins()
```
- [ ] Register `Some(value)` → `Value::Enum { variant: "Some", data: Some(Box::new(value)) }`
- [ ] Register `None` → `Value::Enum { variant: "None", data: None }`
- [ ] Register `Ok(value)` → `Value::Enum { variant: "Ok", data: Some(Box::new(value)) }`
- [ ] Register `Err(value)` → `Value::Enum { variant: "Err", data: Some(Box::new(value)) }`
- [ ] Register `Some`, `None`, `Ok`, `Err` as builtin constructors

#### G.2.2 — Implement ? operator
```
File: src/interpreter/eval.rs → eval_expr() Expr::Try branch
```
- [ ] Evaluate inner expression
- [ ] If result is `Enum { variant: "Ok", data }` → unwrap to data value
- [ ] If result is `Enum { variant: "Err", .. }` → early return with the Err
- [ ] If result is `Enum { variant: "Some", data }` → unwrap to data value
- [ ] If result is `Enum { variant: "None", .. }` → early return with None
- [ ] Otherwise → RuntimeError (? used on non-Option/Result)

#### G.2.3 — Utility methods
- [ ] `.unwrap()` method on Option/Result enums
- [ ] `.unwrap_or(default)` method
- [ ] `.is_some()` / `.is_none()` / `.is_ok()` / `.is_err()` methods

#### G.2.4 — Type checker support
```
File: src/analyzer/type_check.rs
```
- [ ] Register Option/Result as known types
- [ ] `?` operator type inference: `Option<T>?` → T, `Result<T,E>?` → T

#### G.2.5 — Tests
- [ ] `Some(42)` creates Option with value
- [ ] `None` creates empty Option
- [ ] `?` unwraps Ok → value
- [ ] `?` short-circuits Err → early return
- [ ] `.unwrap()` on Some → value
- [ ] `.unwrap()` on None → panic
- [ ] Match on Option/Result variants
- [ ] Integration test: function returning Result with ? propagation chain

**Example that must work after G.2:**
```fajar
fn divide(a: f64, b: f64) -> Result<f64, str> {
    if b == 0.0 { Err("division by zero") }
    else { Ok(a / b) }
}
fn compute() -> Result<f64, str> {
    let x = divide(10.0, 2.0)?
    let y = divide(x, 3.0)?
    Ok(x + y)
}
println(compute())  // Ok(6.666...)
```

**Exit Criteria:** Option/Result types work, ? operator propagates correctly. cargo test passes.

---

### Sprint G.3 — Module System (use/mod)
**Goal:** `mod` creates namespaces, `use` imports symbols, multi-file support
**Duration:** 3-4 sessions | **Priority:** CRITICAL (blocks Sprint 5.4)

**Current State:**
- Parser: `ModDecl { name, body }` and `UseDecl { path, kind }` AST nodes exist
- Interpreter: both return `Ok(Value::Null)` — completely ignored
- UseKind: Simple, Glob, Group — all parsed but not evaluated
- All code is single-file only

**Approach:** Two-phase implementation:
- Phase A: Inline modules (`mod math { ... }`) with use imports
- Phase B: File-based modules (`mod math;` loads `math.fj`)

**Tasks:**

#### G.3.1 — Inline module evaluation
```
File: src/interpreter/eval.rs
```
- [ ] `Item::ModDecl` with `body: Some(items)`:
  - Create child environment/scope with module name prefix
  - Evaluate all items inside the module scope
  - Store module symbols: `module_name::symbol_name`
- [ ] Module nesting: `mod a { mod b { fn c() {} } }` → `a::b::c`

#### G.3.2 — Use statement evaluation
```
File: src/interpreter/eval.rs
```
- [ ] `UseKind::Simple` (`use math::square`):
  - Look up `math::square` in module registry → alias as `square` in current scope
- [ ] `UseKind::Glob` (`use math::*`):
  - Import all pub symbols from `math` module into current scope
- [ ] `UseKind::Group` (`use math::{square, cube}`):
  - Import specified symbols from module

#### G.3.3 — Visibility (pub)
```
Files: src/parser/ast.rs, src/interpreter/eval.rs
```
- [ ] FnDef already has `pub` support? Check AST
- [ ] Non-pub items in modules should not be importable
- [ ] `pub fn` → visible outside module; `fn` → module-private

#### G.3.4 — File-based modules
```
File: src/interpreter/mod.rs or eval.rs
```
- [ ] `mod math;` (no body) → look for `math.fj` in same directory
- [ ] Read file, tokenize, parse, evaluate in module scope
- [ ] Support `stdlib/` directory: `use os::memory` → load from stdlib path
- [ ] Module path resolution: relative to current file

#### G.3.5 — Type checker updates
```
File: src/analyzer/type_check.rs
```
- [ ] Register module symbols in SymbolTable with qualified names
- [ ] Resolve `use` imports in type checking pass
- [ ] Verify visibility constraints (pub/private)

#### G.3.6 — Tests
- [ ] Inline module: `mod math { pub fn square(x) { x * x } }; use math::square; square(5)`
- [ ] Module nesting
- [ ] Glob import `use math::*`
- [ ] Group import `use math::{a, b}`
- [ ] Private function not importable → error
- [ ] File-based module: `mod helper;` loads `helper.fj`
- [ ] stdlib import: `use os::*` works
- [ ] Integration tests

**Example that must work after G.3:**
```fajar
mod math {
    pub fn square(x: f64) -> f64 { x * x }
    pub fn cube(x: f64) -> f64 { x * x * x }
    fn internal() { }  // private, not importable
}
use math::{square, cube}
println(square(5.0))  // 25.0
println(cube(3.0))    // 27.0
```

**Exit Criteria:** Inline + file-based modules work. use/mod evaluates correctly. cargo test passes.

---

### Sprint G.4 — Cast Expression & Minor Gaps
**Goal:** Fix remaining small gaps: `as` cast, @device parameters, named arguments
**Duration:** 1-2 sessions | **Priority:** MEDIUM

**Tasks:**

#### G.4.1 — Cast expression (`as`)
```
File: src/interpreter/eval.rs → Expr::Cast branch
```
- [ ] `42 as f64` → Value::Float(42.0)
- [ ] `3.14 as i32` → Value::Int(3)
- [ ] `42i32 as i64` → widening cast
- [ ] Invalid casts → RuntimeError

#### G.4.2 — @device parameter parsing
```
File: src/parser/ (annotation parsing), src/parser/ast.rs
```
- [ ] Extend Annotation struct: `pub args: Option<Vec<String>>`
- [ ] Parse `@device(cpu)`, `@device(gpu)`, `@device(auto)`
- [ ] Currently `@device` has no args — add optional arg parsing
- [ ] Type checker: validate device args (cpu|gpu|auto only)

#### G.4.3 — Named arguments (basic)
```
File: src/interpreter/eval.rs
```
- [ ] CallArg already has `name: Option<String>` in AST
- [ ] When calling function with named args, match by name instead of position
- [ ] Error if named arg doesn't match any param name

#### G.4.4 — Tests
- [ ] Cast: int to float, float to int, widening, narrowing
- [ ] @device(cpu) annotation parsed correctly
- [ ] Named arguments: `add(b: 2, a: 1)` → 3

**Exit Criteria:** All minor gaps resolved. cargo test passes.

---

### Sprint G.5 — Missing Global Builtins & Math Functions
**Goal:** Implement builtins documented in STDLIB_SPEC but not yet available
**Duration:** 2-3 sessions | **Priority:** HIGH

**Current State:**
- 11 builtins exist: print, println, len, type_of, push, pop, to_string, to_int, to_float, assert, assert_eq
- Missing: panic!, todo!, dbg!, eprint, eprintln, read_line
- Missing math: abs, sqrt, pow, log, log2, log10, sin, cos, tan, floor, ceil, round, clamp
- Missing constants: PI, E (math)

**Tasks:**

#### G.5.1 — Error/Debug builtins
```
File: src/interpreter/eval.rs
```
- [ ] `panic(msg)` — print message and terminate with RuntimeError
- [ ] `todo(msg?)` — like panic but "not yet implemented" message
- [ ] `dbg(value)` — print `[debug] value = <repr>` to stderr, return value
- [ ] `eprint(args...)` — print to stderr (no newline)
- [ ] `eprintln(args...)` — print to stderr (with newline)

#### G.5.2 — Basic I/O builtins
- [ ] `read_line()` — read a line from stdin, return String

#### G.5.3 — Math functions
```
File: src/interpreter/eval.rs
```
- [ ] `abs(x)`, `sqrt(x)`, `pow(base, exp)` — basic math
- [ ] `log(x)`, `log2(x)`, `log10(x)` — logarithms
- [ ] `sin(x)`, `cos(x)`, `tan(x)` — trigonometry
- [ ] `floor(x)`, `ceil(x)`, `round(x)` — rounding
- [ ] `clamp(x, min, max)` — clamping
- [ ] `PI`, `E` as pre-defined constants in global scope

#### G.5.4 — Type checker signatures
- [ ] Register all new builtins in type_check.rs
- [ ] Math functions: accept Float/IntLiteral, return Float

#### G.5.5 — Tests (~20 tests)
- [ ] panic! terminates with error message
- [ ] todo! terminates with "not yet implemented"
- [ ] dbg! prints to stderr and returns value
- [ ] eprint/eprintln output to stderr
- [ ] All math functions: known values (sin(0)=0, cos(0)=1, sqrt(4)=2, etc.)
- [ ] PI and E constants accessible
- [ ] clamp(5, 0, 3) == 3

---

### Sprint G.6 — NN Runtime Builtin Exposure
**Goal:** Make optimizer, layer, and autograd operations callable from Fajar Lang
**Duration:** 2-3 sessions | **Priority:** HIGH

**Current State:**
- SGD, Adam optimizers exist in `src/runtime/ml/optim.rs` — NOT exposed as builtins
- Dense, Dropout, BatchNorm layers exist in `src/runtime/ml/layers.rs` — NOT exposed
- Autograd ops (backward, grad, requires_grad) exist in `src/runtime/ml/autograd.rs` — NOT exposed
- Only tensor creation/ops/activations/losses are callable from .fj

**Tasks:**

#### G.6.1 — Autograd builtins
```
File: src/interpreter/eval.rs
```
- [ ] `tensor_backward(tensor)` — run backward pass from scalar loss
- [ ] `tensor_grad(tensor)` — get gradient of tensor
- [ ] `tensor_requires_grad(tensor, bool)` — set requires_grad flag
- [ ] `tensor_no_grad_start()` / `tensor_no_grad_end()` — disable/enable gradient recording

#### G.6.2 — Optimizer builtins
- [ ] `optimizer_sgd(lr, momentum?)` — create SGD optimizer (return as opaque value)
- [ ] `optimizer_adam(lr?, beta1?, beta2?, epsilon?)` — create Adam optimizer
- [ ] `optimizer_step(optimizer, params)` — update parameters
- [ ] `optimizer_zero_grad(params)` — reset gradients

#### G.6.3 — Layer builtins
- [ ] `layer_dense(in_features, out_features)` — create Dense layer
- [ ] `layer_forward(layer, input)` — forward pass through layer
- [ ] `layer_params(layer)` — get layer parameters (for optimizer)
- [ ] `layer_dropout(rate)` — create Dropout layer
- [ ] `layer_batchnorm(features)` — create BatchNorm layer

#### G.6.4 — New Value variants
```
File: src/interpreter/value.rs
```
- [ ] Consider `Value::Optimizer(OptimizerValue)` or use opaque handle
- [ ] Consider `Value::Layer(LayerValue)` or use opaque handle

#### G.6.5 — Type checker + KE002
- [ ] Register all new builtins in type_check.rs
- [ ] Add to tensor_builtins set for KE002 enforcement

#### G.6.6 — Tests (~15 tests)
- [ ] backward computes gradients correctly
- [ ] SGD optimizer updates parameters
- [ ] Adam optimizer with bias correction
- [ ] Dense layer forward pass shape check
- [ ] Dropout applies during training
- [ ] Full training loop: forward → loss → backward → step
- [ ] Integration test: XOR training convergence

---

### Sprint G.7 — Parser & Analyzer Cleanup
**Goal:** Fix dead code, missing parser features, and code quality issues
**Duration:** 1-2 sessions | **Priority:** MEDIUM

**Tasks:**

#### G.7.1 — `loop` expression
```
File: src/parser/mod.rs
```
- [ ] Add `loop { body }` parsing (documented in GRAMMAR_REFERENCE, not implemented)
- [ ] AST representation: reuse `While { condition: true, body }` or add `Loop` variant
- [ ] Interpreter: evaluate like `while true { ... }`
- [ ] Tests: loop with break, infinite loop protection

#### G.7.2 — Dead code cleanup in analyzer
```
File: src/analyzer/type_check.rs
```
- [ ] Remove/fix references to non-existent Expr variants
- [ ] Clean up unreachable match arms
- [ ] Verify all Expr/Stmt/Item variants are handled

#### G.7.3 — Missing error codes
- [ ] KE004 (documented but unused) — assess if needed or remove from ERROR_CODES.md
- [ ] DE003 (documented but unused) — assess if needed or remove
- [ ] ME001-ME008 (ownership errors) — mark as "Phase 7" in ERROR_CODES.md since borrow_lite.rs is empty
- [ ] Update ERROR_CODES.md to reflect actual implementation status

#### G.7.4 — stdlib/core.fj
```
New file: stdlib/core.fj
```
- [ ] Create core standard library with basic utilities
- [ ] Functions: min, max, swap, range helpers
- [ ] Type aliases or common patterns
- [ ] Update src/stdlib/ with core module

#### G.7.5 — Tests (~10 tests)
- [ ] loop { break } terminates
- [ ] loop { if cond { break } } works
- [ ] Error codes document matches implementation

---

### Gap Fix Summary

| Sprint | Focus | Sessions | Tests Added |
|--------|-------|----------|-------------|
| G.1 | impl blocks & method dispatch | 2-3 | ~15 |
| G.2 | Option/Result & ? operator | 2-3 | ~15 |
| G.3 | Module system (use/mod) | 3-4 | ~20 |
| G.4 | Cast, @device params, named args | 1-2 | ~10 |
| G.5 | Missing builtins & math | 2-3 | ~20 |
| G.6 | NN runtime exposure | 2-3 | ~15 |
| G.7 | Parser & analyzer cleanup | 1-2 | ~10 |
| **Total** | | **13-21 sessions** | **~105 tests** |

**Expected test count after gaps:** ~765 tests

---

## PART 2: PHASE 5 — TOOLING & COMPILER BACKEND

After all gaps are fixed, Phase 5 builds developer tooling and performance improvements.

---

### Sprint 5.1 — Code Formatter (`fj fmt`)
**Goal:** Idempotent code formatter for Fajar Lang source files
**Duration:** 2-3 sessions | **Priority:** HIGH (quick DX win)

**Architecture:**
```
Source (.fj) → Lexer → Parser → AST → Formatter → Formatted Source
```
The formatter reads AST and emits properly formatted source code.
Key: must preserve comments and doc comments.

**Tasks:**

#### 5.1.1 — Formatter module
```
New file: src/formatter/mod.rs
New file: src/formatter/pretty.rs
```
- [ ] `pub fn format(source: &str) -> Result<String, FjError>`
- [ ] `Formatter` struct that walks AST and emits formatted text
- [ ] Formatting rules:
  - Indentation: 4 spaces (no tabs)
  - Braces: opening brace on same line, closing on new line
  - Operators: spaces around binary ops (`a + b`, not `a+b`)
  - Commas: space after comma, not before
  - Empty lines: max 1 consecutive blank line
  - Trailing newline at end of file
  - Line length: soft limit 100 chars

#### 5.1.2 — Comment preservation
```
File: src/lexer/ (may need token changes)
```
- [ ] Lexer currently discards comments — need to capture them
- [ ] Option A: Lexer emits comment tokens (with flag to skip in parser)
- [ ] Option B: Collect comments with positions in separate vec, reattach during formatting
- [ ] Preserve `///` doc comments, `//!` module comments, `//` line comments, `/* */` block comments

#### 5.1.3 — Item formatting
- [ ] Functions: annotation on separate line, params aligned
- [ ] Structs: fields on separate lines, aligned
- [ ] Enums: variants on separate lines
- [ ] Impl blocks: methods indented inside
- [ ] Const: single line

#### 5.1.4 — Expression formatting
- [ ] Binary: spaces around operators
- [ ] If/else: braces required, else on same line as `}`
- [ ] Match: arms indented, `=>` aligned
- [ ] Pipeline: `|>` on new line if chained
- [ ] Block: last expression without semicolon
- [ ] Array/tuple: compact if short, multi-line if long

#### 5.1.5 — CLI integration
```
File: src/main.rs
```
- [ ] Add `Fmt` subcommand:
  ```rust
  Fmt {
      file: PathBuf,
      #[arg(long)]
      check: bool,  // exit 1 if not formatted (CI mode)
  }
  ```
- [ ] `fj fmt file.fj` → rewrite file in-place
- [ ] `fj fmt --check file.fj` → exit 0 if formatted, 1 if not

#### 5.1.6 — Tests
- [ ] Idempotency: format(format(src)) == format(src)
- [ ] Comment preservation: comments remain in output
- [ ] All example files: format and verify they still run correctly
- [ ] Edge cases: empty file, single expression, deeply nested

**Exit Criteria:** `fj fmt` produces consistent, readable output. All examples pass after formatting.

---

### Sprint 5.2 — Bytecode VM
**Goal:** 10-100x performance improvement over tree-walking interpreter
**Duration:** 6-8 sessions | **Priority:** VERY HIGH (core value)

**Architecture:**
```
AST → Compiler → Vec<Instruction> (bytecode) → VM → Value
                  + ConstantPool
                  + FunctionTable
```

**Tasks:**

#### 5.2.1 — Instruction set design
```
New file: src/vm/instruction.rs
```
- [ ] Stack-based VM (simpler to implement than register-based)
- [ ] Instruction enum (~50 opcodes):

```rust
pub enum Op {
    // Stack manipulation
    Const(u32),        // push constant from pool
    Pop,               // discard TOS
    Dup,               // duplicate TOS

    // Arithmetic (operate on TOS)
    Add, Sub, Mul, Div, Mod, Neg, Pow,

    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,

    // Logical
    Not, And, Or,

    // Bitwise
    BitAnd, BitOr, BitXor, BitNot, Shl, Shr,

    // Variables
    LoadLocal(u32),    // push local variable
    StoreLocal(u32),   // pop into local variable
    LoadGlobal(u32),   // push global variable
    StoreGlobal(u32),  // pop into global variable

    // Control flow
    Jump(i32),         // unconditional jump (relative)
    JumpIfFalse(i32),  // conditional jump
    JumpIfTrue(i32),

    // Functions
    Call(u32),         // call function (arg count)
    Return,            // return from function
    CallBuiltin(u32),  // call built-in function

    // Data structures
    NewArray(u32),     // create array with N items from stack
    NewTuple(u32),     // create tuple
    NewStruct(u32),    // create struct (field count)
    GetField(u32),     // get struct field by name index
    SetField(u32),     // set struct field
    GetIndex,          // array[index]
    SetIndex,          // array[index] = value

    // Tensor (delegate to runtime)
    TensorOp(TensorOpKind),

    // OS Runtime (delegate to runtime)
    OsOp(OsOpKind),

    // Misc
    Print, Println,
    Halt,
}
```

#### 5.2.2 — Constant pool & function table
```
New file: src/vm/chunk.rs
```
- [ ] `ConstantPool`: Vec<Value> for literals (ints, floats, strings)
- [ ] `FunctionEntry`: { name, arity, local_count, code_offset }
- [ ] `Chunk`: bytecode + constants + functions + debug info (line numbers)

```rust
pub struct Chunk {
    pub code: Vec<Op>,
    pub constants: Vec<Value>,
    pub functions: Vec<FunctionEntry>,
    pub lines: Vec<u32>,  // source line for each instruction
}
```

#### 5.2.3 — Compiler: AST → Bytecode
```
New file: src/vm/compiler.rs
```
- [ ] `Compiler` struct: walks AST, emits instructions
- [ ] Expression compilation:
  - Literals → Const(pool_index)
  - Binary → compile left, compile right, emit op
  - If/else → JumpIfFalse, compile then, Jump, compile else
  - Block → compile each stmt, last expr is result
  - Function call → compile args, emit Call(arity)
  - Pipeline → rewrite as Call
- [ ] Statement compilation:
  - Let → compile init, StoreLocal
  - While → loop with JumpIfFalse
  - For → desugar to while
  - Return → emit Return
- [ ] Function compilation:
  - Each function gets its own Chunk section
  - Parameters are local variables 0..arity-1
  - Local variable resolution: scope-aware indexing
- [ ] Closure compilation:
  - Capture upvalues (variables from enclosing scope)
  - Store captures in closure object

#### 5.2.4 — VM execution engine
```
New file: src/vm/vm.rs
```
- [ ] `VM` struct: stack, call frames, globals

```rust
pub struct VM {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    chunk: Chunk,
    ip: usize,
    os_rt: OsRuntime,
    ml_tape: Option<Tape>,
}

struct CallFrame {
    function_index: usize,
    ip: usize,           // saved instruction pointer
    stack_base: usize,   // base of this frame's stack
}
```

- [ ] Main execution loop: fetch-decode-execute
- [ ] Stack operations: push, pop, peek
- [ ] Function calls: push CallFrame, set ip to function code
- [ ] Return: pop CallFrame, restore ip
- [ ] Error handling: RuntimeError with source location from debug info

#### 5.2.5 — Built-in function dispatch
- [ ] All 38 ML builtins → TensorOp variants
- [ ] All 16 OS builtins → OsOp variants
- [ ] Standard builtins (print, println, len, type_of, etc.)
- [ ] Reuse existing `runtime::ml` and `runtime::os` implementations

#### 5.2.6 — CLI integration
```
File: src/main.rs
```
- [ ] New flag: `fj run --vm file.fj` (use bytecode VM)
- [ ] Default: keep tree-walking interpreter for now
- [ ] Later: make VM the default once stable

#### 5.2.7 — Tests & benchmarks
- [ ] All existing eval_tests must pass with VM backend
- [ ] All ml_tests must pass with VM backend
- [ ] All os_tests must pass with VM backend
- [ ] Benchmark: fibonacci(30) tree-walk vs bytecode
- [ ] Benchmark: MNIST forward pass comparison
- [ ] Target: 10x+ speedup

**Exit Criteria:** VM executes all existing programs correctly. Performance significantly better than tree-walking.

---

### Sprint 5.2.1 — VM Gap Fixes (CRITICAL)
**Goal:** Fix correctness bugs and missing features before VM can be trusted
**Duration:** 2-3 sessions | **Priority:** CRITICAL (blocks VM usability)
**Recommendation:** FIX BEFORE SPRINT 5.3

**Rationale: Why fix now, not later?**
```
1. CORRECTNESS > FEATURES — CLAUDE.md principle #1. VM has semantically WRONG behavior
   for &&/||. This isn't "missing feature", it's "silently produces wrong results."
2. COMPOUND DEBT — Every sprint built on broken VM foundation makes bugs harder to find.
   LSP testing will use VM output → wrong VM = wrong LSP tests.
3. SMALL SCOPE — Critical fixes (#1-#5) are 2-3 sessions. Deferring saves nothing.
4. PARITY REQUIREMENT — Original Sprint 5.2 exit criteria: "all existing tests pass on VM".
   Currently only 15/65 eval_tests have VM equivalents. Sprint 5.2 is not truly complete.
```

**Gap Classification:**

```
CRITICAL (semantically wrong — fix immediately):
  C1. Logical AND/OR short-circuiting
  C2. SetField (struct mutation)
  C3. SetIndex (array mutation)
  C4. NewEnum (enum construction)
  C5. Dispatch deduplication (run() vs execute_one())

HIGH (missing features — fix in this sprint):
  H1. Pipe operator stack order
  H2. Break/Continue locals cleanup
  H3. Closure environment capture

MEDIUM (defer to Phase 7 — not needed for core correctness):
  M1. Advanced match patterns (struct/enum/tuple/range)
  M2. Try operator (?) unwrapping
  M3. Method dispatch via impl
  M4. Module/import system in VM
  M5. Tensor/OS runtime opcodes
  M6. MatMul operator
```

**Tasks:**

#### 5.2.1a — Fix logical short-circuiting [CRITICAL]
```
File: src/vm/compiler.rs — compile_expr() Binary branch
File: src/vm/engine.rs — remove Op::And/Op::Or if present
```
- [ ] `&&` must NOT evaluate RHS if LHS is false
  - Compile LHS → JumpIfFalse(end) → compile RHS → end:
  - Do NOT use BitAnd — that evaluates both sides
- [ ] `||` must NOT evaluate RHS if LHS is true
  - Compile LHS → JumpIfTrue(end) → compile RHS → end:
- [ ] Tests: `false && panic("boom")` must NOT panic
- [ ] Tests: `true || panic("boom")` must NOT panic

#### 5.2.1b — Implement SetField, SetIndex, NewEnum [CRITICAL]
```
File: src/vm/engine.rs — Op::SetField, Op::SetIndex, Op::NewEnum handlers
```
- [ ] SetField: pop value, pop struct, set field, push struct back
- [ ] SetIndex: pop value, pop index, pop array, set element, push array back
- [ ] NewEnum: construct Value::Enum with variant name and optional data
- [ ] Tests: struct field mutation, array element mutation, enum construction

#### 5.2.1c — Deduplicate dispatch logic [TECH DEBT]
```
File: src/vm/engine.rs
```
- [ ] Remove execute_one() method entirely
- [ ] Refactor run_until_return() to reuse run() with a depth-based exit condition
  - Option A: run() takes optional `target_depth` param
  - Option B: run_until_return() just calls run() after setting up frame
- [ ] Verify no behavior change (all tests still pass)

#### 5.2.1d — Fix pipe operator [HIGH]
```
File: src/vm/compiler.rs — compile_expr() Pipe branch
```
- [ ] `x |> f` must compile to: compile x, push as arg, compile f, Call(1)
- [ ] Currently: compiles both but wrong stack order for Call
- [ ] Tests: `5 |> double` where double(x) = x * 2

#### 5.2.1e — Fix break/continue locals cleanup [HIGH]
```
File: src/vm/compiler.rs — compile_break/compile_continue
```
- [ ] Track local count at loop start
- [ ] On break/continue: emit Pop for each local declared inside loop body
- [ ] Tests: `for i in 0..5 { let x = i * 2; if x > 4 { break } }; println("ok")`

#### 5.2.1f — Fix closure environment capture [HIGH]
```
File: src/vm/compiler.rs — compile_closure()
File: src/vm/engine.rs — closure dispatch
```
- [ ] Identify free variables referenced inside closure body
- [ ] Capture current values at closure creation time
- [ ] Store captured values in closure object (Value::Function with captured env)
- [ ] Tests: `let x = 10; let f = |y| x + y; println(f(5))` → 15

#### 5.2.1g — VM parity tests [HIGH]
- [ ] Run all 50 existing tree-walker eval_tests through VM
- [ ] Mark which ones pass, which fail
- [ ] Target: ≥40/50 pass (allow failures for modules, advanced match, try)
- [ ] Add VM parity test runner: for each eval_output() test, also run vm_output()

**Exit Criteria:**
- [ ] `false && panic("boom")` does NOT panic on VM
- [ ] Struct field mutation works on VM
- [ ] Array index mutation works on VM
- [ ] Enum construction works on VM
- [ ] No duplicated dispatch logic (execute_one removed)
- [ ] Pipe operator produces correct results
- [ ] ≥40/50 eval_tests pass on VM backend
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo test` all pass

---

### Sprint 5.3 — LSP Server
**Goal:** IDE support: diagnostics, go-to-definition, hover, completions
**Duration:** 4-5 sessions | **Priority:** HIGH (DX)

**New dependencies:**
```toml
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }
```

**Tasks:**

#### 5.3.1 — LSP server skeleton
```
New file: src/lsp/mod.rs
New file: src/lsp/server.rs
```
- [ ] Implement `tower_lsp::LanguageServer` trait
- [ ] Initialize with capabilities: diagnostics, hover, completion, goto-definition
- [ ] Document sync: full text on change

#### 5.3.2 — Diagnostics (on-save / on-change)
- [ ] On text change: run lexer + parser + analyzer
- [ ] Convert FjDiagnostic → LSP Diagnostic
- [ ] Publish diagnostics to client
- [ ] Severity mapping: Error → DiagnosticSeverity::Error, Warning → Warning

#### 5.3.3 — Hover (show types)
- [ ] On hover over identifier: look up in SymbolTable
- [ ] Show type information + doc comment if available
- [ ] Functions: show full signature
- [ ] Variables: show inferred type

#### 5.3.4 — Go-to-definition
- [ ] Build symbol location index during analysis
- [ ] On goto-definition: look up symbol → return definition span
- [ ] Works for: variables, functions, structs, enums

#### 5.3.5 — Completions
- [ ] Keyword completions (all Fajar Lang keywords)
- [ ] Built-in function completions (all builtins with signatures)
- [ ] Variable completions (from current scope)
- [ ] Struct field completions after `.`
- [ ] Module member completions after `::`

#### 5.3.6 — CLI integration
```
File: src/main.rs
```
- [ ] Add `Lsp` subcommand: `fj lsp` (start LSP server on stdin/stdout)

#### 5.3.7 — VS Code extension
```
New directory: editors/vscode/
```
- [ ] `package.json`: extension manifest
- [ ] TextMate grammar (`fajar-lang.tmLanguage.json`): syntax highlighting
  - Keywords, types, operators, strings, comments, annotations
- [ ] Language configuration: bracket pairs, comment toggling, auto-indent
- [ ] LSP client: connect to `fj lsp`

#### 5.3.8 — Tests
- [ ] Diagnostics: send source with errors → receive diagnostics
- [ ] Hover: hover over variable → correct type shown
- [ ] Completions: trigger completion → keywords + builtins present
- [ ] VS Code: extension loads and highlights correctly (manual test)

**Exit Criteria:** LSP server provides useful diagnostics, hover, and completions. VS Code extension works.

---

### Sprint 5.4 — Package Manager (`fj.toml`)
**Goal:** Project management: create, build, manage dependencies
**Duration:** 3-4 sessions | **Priority:** MEDIUM
**Prerequisite:** Sprint G.3 (Module System) MUST be complete

**Tasks:**

#### 5.4.1 — Project manifest
```
New file: src/package/mod.rs
New file: src/package/manifest.rs
```
- [ ] `fj.toml` format:
```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2026"
entry = "src/main.fj"

[dependencies]
# future: registry support
```
- [ ] Parse fj.toml using `toml` crate (new dependency)
- [ ] `ProjectConfig` struct

#### 5.4.2 — CLI commands
```
File: src/main.rs
```
- [ ] `fj new <name>` → create project directory with fj.toml + src/main.fj
- [ ] `fj build` → compile project (resolve modules, check types)
- [ ] `fj run` (no file arg) → build + run entry point from fj.toml

#### 5.4.3 — Module resolution for projects
- [ ] Read fj.toml to find entry point
- [ ] Resolve `mod` declarations relative to project root
- [ ] Search path: `src/` directory, then `stdlib/`
- [ ] Error: module file not found → helpful message

#### 5.4.4 — stdlib bundling
- [ ] Ship `stdlib/*.fj` with binary (embed or locate at runtime)
- [ ] `use std::math::sqrt` → resolve from bundled stdlib
- [ ] `use os::memory` → resolve from os.fj
- [ ] `use nn::tensor` → resolve from nn.fj

#### 5.4.5 — Tests
- [ ] `fj new test-project` creates correct structure
- [ ] `fj build` compiles multi-file project
- [ ] Module imports across files resolve correctly
- [ ] Missing module → clear error message

**Exit Criteria:** Can create and build multi-file Fajar Lang projects. stdlib accessible via `use`.

---

### Sprint 5.5 — LLVM Backend (Assessment & Foundation)
**Goal:** Evaluate feasibility, implement minimal prototype if viable
**Duration:** 2-3 sessions (assessment) + 6-8 sessions (implementation, if proceeding)
**Priority:** LOW — defer full implementation to Phase 7

**Assessment criteria:**
- [ ] Can `inkwell` (LLVM Rust bindings) be integrated without breaking build?
- [ ] What LLVM version is available on target platforms?
- [ ] What is compile time impact? (inkwell adds ~50MB+ to build)
- [ ] Can we compile a simple fibonacci program to native binary?

**If proceeding (minimal prototype):**

#### 5.5.1 — LLVM IR codegen for expressions
```
New file: src/codegen/mod.rs
New file: src/codegen/llvm.rs
New dependency: inkwell = "0.4" (LLVM 17+)
```
- [ ] Integer arithmetic → LLVM IR (add, sub, mul, div)
- [ ] Function definitions → LLVM functions
- [ ] Function calls → LLVM call instruction
- [ ] Conditionals → LLVM br instruction
- [ ] Compile `fibonacci(30)` to native binary

#### 5.5.2 — Runtime library (libfj)
- [ ] Minimal C runtime: print, memory allocation
- [ ] Link with generated LLVM IR

**Decision point:** After assessment, decide if full LLVM backend is worth pursuing in Phase 5
or should be deferred to Phase 7.

**Recommendation:** Do assessment only. Full LLVM backend is Phase 7 material.

---

### Sprint 5.6 — GPU Backend (Assessment Only)
**Goal:** Research and prototype GPU tensor acceleration
**Duration:** 1-2 sessions (research only)
**Priority:** LOW — defer implementation to Phase 7

**Research questions:**
- [ ] wgpu vs vulkano vs opencl-rs for compute shaders?
- [ ] Can we write a matmul compute shader in WGSL?
- [ ] What is the data transfer overhead (CPU ↔ GPU)?
- [ ] Does ndarray have GPU backend options?

**If prototyping:**
- [ ] Single operation: matmul on GPU via wgpu compute shader
- [ ] Benchmark: GPU matmul vs CPU matmul for 1000x1000

**Recommendation:** Research only. Full GPU backend is Phase 7 material.

---

## FULL SCHEDULE

```
┌──────────────────────────────────────────────────────────────────┐
│  PRE-PHASE 5 GAP FIXES                                          │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌───────┐ ┌───────┐          │
│  │ G.1    │ │ G.2    │ │ G.3    │ │ G.4   │ │ G.5   │          │
│  │ impl   │→│ Option │→│ Module │→│ Cast  │→│ Math  │          │
│  │ 2-3 ses│ │ 2-3 ses│ │ 3-4 ses│ │ 1-2 s │ │ 2-3 s │          │
│  └────────┘ └────────┘ └────────┘ └───────┘ └───────┘          │
│                                                                  │
│  ┌────────┐ ┌────────┐                                          │
│  │ G.6    │ │ G.7    │                                          │
│  │ NN exp │ │ Cleanup│                                          │
│  │ 2-3 ses│ │ 1-2 ses│                                          │
│  └────────┘ └────────┘                                          │
│  ~13-21 sessions total → ~765 tests                              │
├──────────────────────────────────────────────────────────────────┤
│  PHASE 5 PROPER                                                  │
│  ┌─────────┐  ┌──────────────┐  ┌─────────┐                     │
│  │ 5.1     │  │ 5.2          │  │ 5.3     │                     │
│  │ Code    │→ │ Bytecode VM  │→ │ LSP     │                     │
│  │ Fmt     │  │ (biggest)    │  │ Server  │                     │
│  │ 2-3 ses │  │ 6-8 ses      │  │ 4-5 ses │                     │
│  └─────────┘  └──────────────┘  └─────────┘                     │
│  ┌─────────┐  ┌──────────────┐  ┌───────────┐                   │
│  │ 5.4     │  │ 5.5          │  │ 5.6       │                   │
│  │ Package │  │ LLVM         │  │ GPU       │                   │
│  │ Manager │  │ (assess only)│  │ (research)│                   │
│  │ 3-4 ses │  │ 2-3 ses      │  │ 1-2 ses   │                   │
│  └─────────┘  └──────────────┘  └───────────┘                   │
│  ~18-25 sessions total → ~900+ tests                             │
├──────────────────────────────────────────────────────────────────┤
│  PHASE 6 — STANDARD LIBRARY                                     │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │ 6.1     │  │ 6.2     │  │ 6.3     │  │ 6.4     │            │
│  │ String  │→ │ Collect │→ │ IO/File │→ │ OS+NN   │            │
│  │ 2-3 ses │  │ 3-4 ses │  │ 2-3 ses │  │ 2-3 ses │            │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │
│  ~9-13 sessions total → ~1050+ tests                             │
├──────────────────────────────────────────────────────────────────┤
│  PHASE 7 — PRODUCTION HARDENING                                  │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │ 7.1     │  │ 7.2     │  │ 7.3     │  │ 7.4     │            │
│  │ Fuzzing │→ │ Bench   │→ │ Audit   │→ │ Docs    │            │
│  │ 2-3 ses │  │ 2-3 ses │  │ 3-4 ses │  │ 3-4 ses │            │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │
│  ┌─────────┐  ┌─────────┐                                       │
│  │ 7.5     │  │ 7.6     │                                       │
│  │ Example │  │ LLVM/GPU│                                       │
│  │ OS+ML   │  │ Full    │                                       │
│  │ 3-4 ses │  │ 8-12 ses│                                       │
│  └─────────┘  └─────────┘                                       │
│  ~21-30 sessions total → ~1200+ tests                            │
└──────────────────────────────────────────────────────────────────┘
```

## DEPENDENCY GRAPH

```
G.1 (impl) ────┬──────────────────────→ 5.1 (fmt) ──→ 6.x
     │          │                              │
     ▼          ▼                              ▼
G.2 (Option) G.5 (math/builtins)        5.2 (bytecode VM)
     │          │                              │
     ▼          ▼                              ▼
G.3 (modules) G.6 (NN exposure)         5.3 (LSP)
     │          │
     ▼          ▼
G.4 (minor) G.7 (cleanup)
     │
     ▼
5.4 (package) ──→ 5.5 (LLVM assess) ──→ 7.6 (LLVM/GPU full)
                        │
                        ▼
                  5.6 (GPU research)
```

**Critical path:** G.1 → G.2 → G.3 → G.4 → 5.1 → 5.2 → 5.3

## EXECUTION ORDER (Recommended)

| # | Sprint | Focus | Prereq | Sessions |
|---|--------|-------|--------|----------|
| 1 | G.1 | impl blocks & method dispatch | — | 2-3 |
| 2 | G.2 | Option/Result & ? operator | G.1 | 2-3 |
| 3 | G.3 | Module system (use/mod) | G.1 | 3-4 |
| 4 | G.4 | Cast, @device params, named args | — | 1-2 |
| 5 | G.5 | Missing builtins & math functions | — | 2-3 |
| 6 | G.6 | NN runtime builtin exposure | G.1 | 2-3 |
| 7 | G.7 | Parser & analyzer cleanup | — | 1-2 |
| 8 | 5.1 | Code Formatter (`fj fmt`) | G.1-G.7 | 2-3 |
| 9 | 5.2 | Bytecode VM | G.1-G.7 | 6-8 |
| 10 | 5.3 | LSP Server + VS Code | 5.2 | 4-5 |
| 11 | 5.4 | Package Manager | G.3 | 3-4 |
| 12 | 5.5 | LLVM Backend (assess) | 5.2 | 2-3 |
| 13 | 5.6 | GPU Backend (research) | — | 1-2 |

**Note:** G.4, G.5, G.7 have no dependencies — can run in parallel with G.2/G.3.
**Note:** G.6 requires G.1 (impl blocks for layer/optimizer methods).

**Total gap fixes: ~13-21 sessions | Phase 5: ~18-25 sessions**

---

## PART 3: PHASE 6 — STANDARD LIBRARY

After Phase 5, build out the complete standard library documented in STDLIB_SPEC.md.

---

### Sprint 6.1 — std::string & std::convert
**Goal:** String manipulation and type conversion utilities
**Duration:** 2-3 sessions

**Tasks:**
- [ ] String methods: split, trim, trim_start, trim_end, starts_with, ends_with
- [ ] String methods: contains, replace, to_uppercase, to_lowercase, repeat
- [ ] String methods: chars (iterate), substring/slice, format
- [ ] Type conversions: From/Into/TryFrom trait pattern (or builtin conversions)
- [ ] Numeric parsing: parse_int, parse_float
- [ ] Tests: ~25 tests

### Sprint 6.2 — std::collections
**Goal:** Generic collection types beyond Array
**Duration:** 3-4 sessions

**Tasks:**
- [ ] HashMap<K,V>: new, insert, get, remove, contains_key, keys, values, len, iter
- [ ] HashSet<T>: new, insert, remove, contains, union, intersection, len
- [ ] Requires generics support (from G.1 or simplified approach)
- [ ] Option: may use existing enum-based approach from G.2
- [ ] Tests: ~30 tests

### Sprint 6.3 — std::io & File I/O
**Goal:** File system operations and I/O streams
**Duration:** 2-3 sessions

**Tasks:**
- [ ] read_file(path) → Result<String, Error>
- [ ] write_file(path, content) → Result<(), Error>
- [ ] append_file(path, content) → Result<(), Error>
- [ ] file_exists(path) → Bool
- [ ] Stdin: read_line() (basic, from G.5)
- [ ] Stdout/Stderr: print/println/eprint/eprintln (from G.5)
- [ ] Tests: ~15 tests

### Sprint 6.4 — OS & NN Stdlib Completion
**Goal:** Complete the os:: and nn:: standard libraries
**Duration:** 2-3 sessions

**Tasks:**
- [ ] os::memory completion: memory_copy, memory_set, memory_compare
- [ ] os::process: basic process abstraction (if relevant for embedded)
- [ ] nn::data: load_csv, DataLoader concept, batch iteration
- [ ] nn::layer: Conv2d, Attention, LayerNorm (complex layers)
- [ ] nn::metrics: accuracy, precision, recall, f1_score
- [ ] stdlib/core.fj completion (from G.7.4)
- [ ] Tests: ~25 tests

### Phase 6 Exit Criteria
- [ ] All STDLIB_SPEC.md functions implemented (>90% coverage)
- [ ] HashMap and HashSet usable from Fajar Lang
- [ ] File I/O works for reading/writing text files
- [ ] nn::data can load CSV and batch data
- [ ] ~1050+ tests, clippy clean, fmt clean

---

## PART 4: PHASE 7 — PRODUCTION HARDENING

Final phase: make Fajar Lang production-ready.

---

### Sprint 7.1 — Fuzzing & Property Testing
**Duration:** 2-3 sessions

- [ ] cargo-fuzz setup for lexer (random input → no panic)
- [ ] cargo-fuzz for parser (random token streams → no panic)
- [ ] cargo-fuzz for interpreter (random AST → no panic, only RuntimeError)
- [ ] proptest: lexer round-trip (tokenize → reconstruct source ≈ original)
- [ ] proptest: parser invariants (every AST node has valid Span)
- [ ] Tests: ~20 property tests

### Sprint 7.2 — Performance Benchmarks
**Duration:** 2-3 sessions

- [ ] criterion benchmarks for all pipeline stages
- [ ] Benchmark suite: fibonacci(30), prime_sieve(10000), matmul(100x100), MNIST forward
- [ ] Compare tree-walking vs bytecode VM
- [ ] Memory profiling: peak memory usage per benchmark
- [ ] Identify and fix performance bottlenecks
- [ ] Document performance characteristics

### Sprint 7.3 — Security & Safety Audit
**Duration:** 3-4 sessions

- [ ] Review all `unsafe` blocks (runtime/os/) — verify SAFETY comments
- [ ] Audit OS runtime: memory bounds checking, no buffer overflows
- [ ] Audit ML runtime: no panics on malformed tensor data
- [ ] borrow_lite.rs: implement basic ownership/move semantics (ME001-ME008)
- [ ] Ownership: detect use-after-move in type checker
- [ ] Review context isolation: verify @kernel/@device fully enforced
- [ ] Tests: ~20 security-focused tests

### Sprint 7.4 — Documentation Site
**Duration:** 3-4 sessions

- [ ] mdBook or similar static site generator
- [ ] Tutorial: Getting Started with Fajar Lang
- [ ] Tutorial: OS Development with Fajar Lang
- [ ] Tutorial: ML/AI with Fajar Lang
- [ ] API reference (auto-generated from doc comments)
- [ ] Language reference (from FAJAR_LANG_SPEC.md)
- [ ] Deploy to GitHub Pages or similar

### Sprint 7.5 — Example Projects
**Duration:** 3-4 sessions

- [ ] Example: Minimal OS kernel (memory allocator + IRQ handler + syscall dispatcher)
- [ ] Example: MNIST classifier (full training loop, not just forward pass)
- [ ] Example: Simple transformer (attention mechanism)
- [ ] Example: Cross-domain project (sensor data → ML inference → actuator control)
- [ ] Each example has README, runs with `fj run`

### Sprint 7.6 — LLVM & GPU Full Implementation (if decided in 5.5/5.6)
**Duration:** 8-12 sessions (conditional)

- [ ] LLVM codegen for all expression/statement types
- [ ] LLVM runtime library (libfj): print, memory, tensor dispatch
- [ ] `fj build --native` produces standalone binary
- [ ] GPU compute shader backend (wgpu/WGSL)
- [ ] @device(gpu) dispatches tensor ops to GPU
- [ ] CPU↔GPU data transfer optimization
- [ ] Benchmark: GPU vs CPU for large matmul/MNIST

### Phase 7 Exit Criteria
- [ ] No panics found by fuzzer after 1M+ iterations
- [ ] All benchmarks meet Phase 5 performance targets
- [ ] Security audit complete, no critical issues
- [ ] Documentation site live and comprehensive
- [ ] Example OS kernel runs and handles interrupts
- [ ] Example MNIST trains to >90% accuracy
- [ ] 1200+ tests, all passing
- [ ] Ready for v1.0 release

---

## EXIT CRITERIA (Phase 5 Complete)

- [ ] All gap fixes done: impl blocks, Option/Result, modules, cast, math, NN exposure, cleanup
- [ ] `fj fmt` produces idempotent, readable formatting
- [ ] Bytecode VM runs all programs correctly, 10x+ faster than tree-walking
- [ ] LSP server provides diagnostics + hover + completions
- [ ] VS Code extension with syntax highlighting + LSP
- [ ] `fj new` / `fj build` creates and compiles multi-file projects
- [ ] All existing tests pass on both interpreter and VM backends
- [ ] 900+ tests, clippy clean, fmt clean
- [ ] LLVM feasibility assessed (build or defer decision made)
- [ ] GPU feasibility assessed (research complete)

---

## NEW DEPENDENCIES (Phase 5)

| Crate | Sprint | Purpose | Size Impact |
|-------|--------|---------|-------------|
| `toml` | 5.4 | Parse fj.toml | Small |
| `tower-lsp` | 5.3 | LSP protocol | Medium |
| `tokio` | 5.3 | Async runtime for LSP | Medium |
| `inkwell` | 5.5 (if proceed) | LLVM bindings | Large (~50MB) |
| `wgpu` | 5.6 (if proceed) | GPU compute | Large |

**Conservative approach:** Only add `toml`, `tower-lsp`, `tokio` in Phase 5.
`inkwell` and `wgpu` deferred to Phase 7 unless assessment shows easy wins.

---

## RISK REGISTER

| Risk | Impact | Mitigation |
|------|--------|------------|
| Module system complexity | HIGH | Start with inline modules only, add file-based later |
| VM bugs (subtle execution differences) | HIGH | Run ALL existing tests on both backends |
| LLVM dependency bloat | MEDIUM | Assessment-only in Phase 5; defer full impl |
| LSP async complexity | MEDIUM | Use tower-lsp framework (handles protocol) |
| Comment preservation in formatter | MEDIUM | Modify lexer to capture comments early |
| Closure compilation in VM | HIGH | Study Lua/Crafting Interpreters for upvalue design |
| Generics required for collections | HIGH | Phase 6 may need simplified generics; assess in G.1 |
| NN builtin exposure complexity | MEDIUM | Use opaque handles for optimizer/layer values |
| Mojo competition (open-source 2026) | LOW | Our dual-domain @kernel/@device is unique differentiator |

## TECHNOLOGY DECISIONS (from online research, March 2026)

| Topic | Decision | Rationale |
|-------|----------|-----------|
| VM architecture | Stack-based | Simpler to implement; register-based better for JIT but JIT is not in scope |
| LSP framework | tower-lsp | Standard Rust LSP framework, handles protocol complexity |
| Formatter approach | AST-based + comment reattachment | Simpler than rowan/CST approach; good enough for v1 |
| LLVM bindings | inkwell | Mature, supports LLVM 11-21, Rust-safe API |
| GPU compute | wgpu + WGSL shaders | Cross-platform, Rust-native, reference: burn framework |
| Collections in Phase 6 | Builtin approach (not generic) | Avoid need for full monomorphization; HashMap<String,Value> first |

---

## FULL PROJECT TIMELINE SUMMARY

| Phase | Focus | Sessions | Cumulative Tests |
|-------|-------|----------|-----------------|
| Gap Fixes (G.1-G.7) | Language feature completeness | 13-21 | ~765 |
| Phase 5 (5.1-5.6) | Tooling & compiler backend | 18-25 | ~900+ |
| Phase 6 (6.1-6.4) | Standard library | 9-13 | ~1050+ |
| Phase 7 (7.1-7.6) | Production hardening | 21-30 | ~1200+ |
| **Total remaining** | | **61-89 sessions** | **1200+ tests** |

---

*Phase 5 Implementation Plan v2.0 | Updated: 2026-03-05*
*Includes: Gap fixes (G.1-G.7) + Phase 5 + Phase 6 + Phase 7*
*Total effort: ~61-89 sessions | Expected final test count: 1200+*
