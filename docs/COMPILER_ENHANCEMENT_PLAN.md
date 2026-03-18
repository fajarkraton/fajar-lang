# Fajar Lang Compiler Enhancement Plan — "Surya Enablers"

> **Context:** FajarOS v3.0 development revealed practical compiler limitations.
> These enhancements will reduce FajarOS code by 30-40% and unblock critical features.
> **Date:** 2026-03-18
> **Estimated total:** 5 sprints, 48 tasks

---

## Sprint 1: String Literals in @kernel + print_str() (HIGH IMPACT)

**Goal:** Eliminate 200+ `putc()` calls → `print("hello")`
**Estimated impact:** 30-40% code reduction in FajarOS
**Files:** `nostd.rs`, `compile/mod.rs`, `runtime_bare.rs`, `linker.rs`

### Background

Currently `nostd.rs:163-170` rejects ALL string literals in no-std mode with
"string literal requires heap allocation". But read-only string data in `.rodata`
section does NOT require heap — it's compile-time constant data embedded in the ELF.

The existing `fj_rt_bare_print(ptr, len)` already outputs byte buffers to UART.
We just need to allow string literals to compile as static data + pointer/length pair.

### Tasks

| # | Task | Detail | File(s) | Status |
|---|------|--------|---------|--------|
| 1.1 | **Allow string literals in no-std** | Change `nostd.rs:163-170`: allow `LiteralKind::String` when target is bare-metal. String data goes to `.rodata` section instead of heap. | `nostd.rs` | [ ] |
| 1.2 | **Compile string literal to static data** | In `compile/mod.rs` or `compile/expr.rs`: when encountering string literal in no-std, emit data to `.rodata` section via `module.declare_data()`. Return pointer+length pair. | `compile/mod.rs` | [ ] |
| 1.3 | **String type in no-std = (ptr, len)** | In no-std mode, `"hello"` compiles to `(rodata_ptr, 5)` tuple. The existing `__println_str` builtin already takes `(ptr, len)`. | `compile/mod.rs` | [ ] |
| 1.4 | **print() in @kernel** | Map `print("text")` → call `fj_rt_bare_print(ptr, len)` with static rodata pointer. Already linked in bare-metal runtime. | `compile/call.rs` | [ ] |
| 1.5 | **println() in @kernel** | Same as print() but append `\n`. Either emit extra `putc(10)` call or modify `fj_rt_bare_print` to optionally add newline. | `compile/call.rs` | [ ] |
| 1.6 | **String concatenation at compile time** | For `"Hello" + " " + "World"` → concatenate at compile time into single `.rodata` entry. Only for literal + literal (not runtime). | `compile/expr.rs` | [ ] |
| 1.7 | **Test: bare-metal string print** | AOT test: `fn main() { println("Hello FajarOS") }` → compile + link + verify `.rodata` contains "Hello FajarOS". | `tests.rs` | [ ] |
| 1.8 | **Test: FajarOS cmd_help with strings** | Rewrite `cmd_help()` from 89 putc calls to 10 print() calls. Verify same QEMU output. | FajarOS | [ ] |

### Technical Design

```
Current no-std compilation of "hello":
  → REJECTED (nostd.rs:163)

New no-std compilation of "hello":
  1. Cranelift: module.declare_data("str_0", Linkage::Local, false, false)
  2. Write "hello\0" bytes to data section
  3. Create pointer: module.declare_data_in_func(data_id, builder.func)
  4. Create length: builder.ins().iconst(I64, 5)
  5. When passed to print(): call fj_rt_bare_print(ptr, len)

Cranelift ObjectModule data declaration:
  let data_id = module.declare_data(&name, Linkage::Local, false, false)?;
  let mut data_ctx = DataDescription::new();
  data_ctx.define(bytes.to_vec().into_boxed_slice());
  module.define_data(data_id, &data_ctx)?;
```

### Success Criteria
- `println("Hello")` compiles in no-std AOT
- String data appears in `.rodata` section
- FajarOS `cmd_help()` reduced from 89 putc calls to ~10 print calls
- All 5,947 existing tests pass (no regression)

---

## Sprint 2: Fix `return` in Bare-Metal Functions (BLOCKER)

**Goal:** Allow `return expr` in @kernel functions without Cranelift verifier errors
**Files:** `compile/stmt.rs`, `compile/control.rs`, `cranelift/mod.rs`

### Background

`stmt.rs:487-529` compiles `return` by emitting `builder.ins().return_(&[val])`
then switching to a new unreachable block. In bare-metal AOT, the Cranelift verifier
rejects this in certain control flow patterns (e.g., return inside if/else branches).

The fix: use a dedicated exit block with block parameters instead of multiple
`return_` instructions scattered through the function body.

### Tasks

| # | Task | Detail | File(s) | Status |
|---|------|--------|---------|--------|
| 2.1 | **Reproduce minimal failing case** | Create AOT test: `@kernel fn foo(x: i64) -> i64 { if x > 0 { return 1 } 0 }`. Capture verifier error. | `tests.rs` | [ ] |
| 2.2 | **Create function exit block** | At function start, create `exit_block` with return type parameter. All `return` statements jump to this block instead of emitting `return_`. | `compile/stmt.rs` | [ ] |
| 2.3 | **Redirect return to exit block** | Replace `builder.ins().return_(&[val])` with `builder.ins().jump(exit_block, &[val])`. | `compile/stmt.rs` | [ ] |
| 2.4 | **Emit return at exit block** | After compiling function body, switch to `exit_block` and emit single `builder.ins().return_(&[param])`. | `cranelift/mod.rs` | [ ] |
| 2.5 | **Handle void return** | Functions returning void: `return` → jump to exit block with no parameters. | `compile/stmt.rs` | [ ] |
| 2.6 | **Test: return in if/else** | `@kernel fn f(x:i64)->i64 { if x>0 { return 1 } if x<0 { return -1 } 0 }` → compiles + runs correctly. | `tests.rs` | [ ] |
| 2.7 | **Test: early return in loop** | `@kernel fn find(arr:i64,n:i64)->i64 { let mut i=0; while i<n { if cond { return i } i=i+1 } -1 }` | `tests.rs` | [ ] |
| 2.8 | **Verify existing tests** | All 5,947 tests pass. No regressions in JIT or AOT. | CI | [ ] |

### Technical Design

```
Current (broken in some cases):
  bb_entry:
    brif cond, bb_then, bb_else
  bb_then:
    v1 = iconst 1
    return v1          ← verifier error: unreachable code after return
  bb_else:
    v2 = iconst 0
    return v2

New (single exit block):
  bb_exit(v_ret: i64):
    return v_ret

  bb_entry:
    brif cond, bb_then, bb_else
  bb_then:
    v1 = iconst 1
    jump bb_exit(v1)
  bb_else:
    v2 = iconst 0
    jump bb_exit(v2)
```

### Success Criteria
- `return` works in bare-metal functions (no verifier errors)
- FajarOS can use early return in `find_free_pid()`, `ipc_recv()`, etc.
- Code clarity improved: no more manual flag variables

---

## Sprint 3: Fix Volatile/ASM Ordering in AOT (BLOCKER)

**Goal:** volatile_read/write + asm! out() work correctly in all contexts
**Files:** `compile/call.rs`, `compile/asm.rs`, `cranelift/mod.rs`

### Background

Two related issues:
1. `volatile_read()` return values sometimes lost (register clobber after function calls)
2. `asm!("mrs x0, REG", out("x0") var)` doesn't capture x0 in AOT

Root cause analysis from `asm.rs:68-77`:
- `out()` pushes dummy zero to `input_vals` (line 75)
- `write_output()` at line 126-133 does `builder.def_var(var, val)` AFTER the asm
- But the `val` might be wrong — it's the asm instruction result, not the register value

### Tasks

| # | Task | Detail | File(s) | Status |
|---|------|--------|---------|--------|
| 3.1 | **Reproduce asm! out() failure** | AOT test: `let mut v=0; asm!("mov x0, #42", out("x0") v); assert(v == 42)`. | `tests.rs` | [ ] |
| 3.2 | **Trace Cranelift IR for asm! out()** | Dump IR before/after asm compilation. Check if output variable gets correct SSA value. | debug | [ ] |
| 3.3 | **Fix asm output value capture** | After inline asm block, read the output register via Cranelift's register interface. Map physical register → SSA value. | `compile/asm.rs` | [ ] |
| 3.4 | **Reproduce volatile_read ordering** | AOT test: `let a=volatile_read(addr); let b=volatile_read(addr); assert(a != b)` with timer counter. | `tests.rs` | [ ] |
| 3.5 | **Mark volatile calls as side-effecting** | Ensure Cranelift doesn't CSE/reorder volatile operations. Use `SideEffects::All` or equivalent. | `compile/call.rs` | [ ] |
| 3.6 | **Test: timer_count in AOT** | `timer_count()` returns monotonically increasing values. Verify in AOT binary. | `tests.rs` | [ ] |
| 3.7 | **Test: volatile_read in IRQ handler** | Simulate IRQ context, verify volatile_read returns correct values after function calls. | `tests.rs` | [ ] |

### Technical Design

For asm! out() — the issue is mapping Cranelift's inline assembly result to an SSA variable:
```rust
// Current (asm.rs:75): pushes dummy zero
input_vals.push(builder.ins().iconst(ty, 0));

// Fix: after asm block, use the instruction result directly
let asm_inst = builder.ins().call(asm_stub_fn, &input_vals);
let result = builder.inst_results(asm_inst)[0];
builder.def_var(out_var, result);  // capture actual return value
```

### Success Criteria
- `asm!("mrs x0, CNTPCT_EL0", out("x0") t)` returns counter value in AOT
- `volatile_read(addr)` returns correct value after any number of function calls
- FajarOS timer, peek, and IPC work without assembly stub workarounds

---

## Sprint 4: Labeled Break/Continue + Const Expressions

**Goal:** Cleaner control flow and initialization
**Files:** `parser/ast.rs`, `parser/expr.rs`, `parser/items.rs`, `compile/control.rs`

### Background

`ast.rs:474-485` shows Break/Continue nodes have NO label field.
Labeled break/continue requires parser + AST + codegen changes.

### Tasks

| # | Task | Detail | File(s) | Status |
|---|------|--------|---------|--------|
| 4.1 | **Add label to Break/Continue AST** | `Break { label: Option<String>, value: Option<Box<Expr>>, span }` and `Continue { label: Option<String>, span }`. | `ast.rs` | [ ] |
| 4.2 | **Parse labeled loops** | `'name: while ...` or `'name: loop ...` → store label name. | `parser/expr.rs` | [ ] |
| 4.3 | **Parse labeled break** | `break 'name` or `break 'name value` → store label. | `parser/stmt.rs` | [ ] |
| 4.4 | **Parse labeled continue** | `continue 'name` → store label. | `parser/stmt.rs` | [ ] |
| 4.5 | **Codegen: labeled break** | Track loop labels → Cranelift blocks. `break 'outer` → jump to outer loop's merge block. | `compile/control.rs` | [ ] |
| 4.6 | **Codegen: labeled continue** | `continue 'outer` → jump to outer loop's header block. | `compile/control.rs` | [ ] |
| 4.7 | **Const expression evaluation** | Allow `const X = 100 / 2 + 3` → evaluate at compile time. Support arithmetic + bitwise ops. | `compile/stmt.rs` | [ ] |
| 4.8 | **Test: nested loop break** | `'outer: while a { while b { break 'outer } }` → exits both loops. | `tests.rs` | [ ] |
| 4.9 | **Test: const expression** | `const SIZE = 4096 * 16; let arr = alloc(SIZE)` → SIZE = 65536 at compile time. | `tests.rs` | [ ] |

### Success Criteria
- `break 'outer` exits nested loops
- `continue 'outer` continues outer loop
- `const X = 1024 * 64` evaluates at compile time
- FajarOS loop patterns simplified

---

## Sprint 5: @kernel Codegen Enforcement + @interrupt Attribute

**Goal:** Compile-time safety guarantees for OS code
**Files:** `cranelift/mod.rs`, `compile/call.rs`, `compile/stmt.rs`

### Background

Currently `@kernel` context is only checked by the semantic analyzer (`type_check/check.rs`).
The native codegen allows @kernel functions to call heap-allocating builtins if the analyzer
is bypassed. Full safety requires codegen-level enforcement.

### Tasks

| # | Task | Detail | File(s) | Status |
|---|------|--------|---------|--------|
| 5.1 | **Pass context annotation to codegen** | Thread `@kernel`/`@device`/`@safe` from FnDef through to CraneliftCompiler/ObjectCompiler. Store in `CodegenCtx`. | `cranelift/mod.rs` | [ ] |
| 5.2 | **Block heap builtins in @kernel codegen** | When compiling @kernel function, reject calls to `alloc`, `String::new`, `Vec::new`, etc. at codegen time. | `compile/call.rs` | [ ] |
| 5.3 | **Block tensor ops in @kernel codegen** | Reject tensor builtins (zeros, matmul, etc.) in @kernel context at codegen time. | `compile/call.rs` | [ ] |
| 5.4 | **@interrupt function attribute** | New attribute: `@interrupt fn handler() { ... }` → auto-generate register save/restore prologue/epilogue. | `cranelift/mod.rs` | [ ] |
| 5.5 | **@interrupt: save all GP registers** | Generate `stp x0,x1,[sp,#-16]!; stp x2,x3,...` at function entry. Reverse at exit. | `cranelift/mod.rs` | [ ] |
| 5.6 | **@interrupt: eret instead of ret** | @interrupt functions end with `eret` (exception return) instead of `ret`. | `cranelift/mod.rs` | [ ] |
| 5.7 | **Test: @kernel blocks heap** | `@kernel fn f() { let s = String::new() }` → codegen error CE011. | `tests.rs` | [ ] |
| 5.8 | **Test: @interrupt saves registers** | `@interrupt fn irq() { putc(46) }` → objdump shows STP/LDP pairs around function body. | `tests.rs` | [ ] |

### Technical Design

@interrupt function compilation:
```
@interrupt fn fj_exception_irq() {
    let irq = gic_ack()
    ...
}

Generates:
fj_exception_irq:
    stp x29, x30, [sp, #-16]!
    stp x0, x1, [sp, #-16]!
    ... (save all regs)
    ; function body
    ... (restore all regs)
    ldp x0, x1, [sp], #16
    ldp x29, x30, [sp], #16
    eret
```

### Success Criteria
- @kernel functions cannot call heap builtins (codegen enforced)
- @interrupt functions auto-save/restore registers
- No changes needed to FajarOS exception handler code

---

## Execution Timeline

```
Sprint 1: String literals     [████████░░] ~6h  — 30% code reduction
Sprint 2: Fix return          [██████░░░░] ~4h  — unblock early returns
Sprint 3: Fix volatile/asm    [████████░░] ~6h  — unblock IPC + timer
Sprint 4: Labels + const      [██████░░░░] ~4h  — code clarity
Sprint 5: @kernel + @interrupt[██████░░░░] ~4h  — safety guarantees
                              Total: ~24h, 48 tasks
```

## Impact Summary

| Enhancement | FajarOS LOC Saved | Code Clarity | Safety |
|-------------|-------------------|-------------|--------|
| String literals | **1,000+ lines** (putc→print) | +++ | — |
| Fix return | ~100 lines (flag removal) | +++ | — |
| Fix volatile/asm | ~50 lines (workaround removal) | ++ | +++ |
| Labeled break | ~80 lines (flag removal) | ++ | — |
| @kernel enforce | — | — | +++ |
| @interrupt | ~50 lines (manual save removal) | ++ | +++ |
| **Total** | **~1,300 lines saved** | **Major** | **Major** |

---

## Research Sources

- Cranelift DataDescription API: `module.declare_data()` + `data_ctx.define()`
- Cranelift function exit block pattern: single return point via `jump bb_exit(val)`
- ARM64 calling convention: callee-saved x19-x28, caller-saved x0-x18
- Cranelift SideEffects enum: `All`, `Reads`, `Writes` for memory ops

---

*Plan created 2026-03-18 by Claude Opus 4.6*
*Total: 5 sprints, 48 tasks, ~24 hours estimated*
