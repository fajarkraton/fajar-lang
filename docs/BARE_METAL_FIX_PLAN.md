# Bare-Metal Readiness Fix Plan

> **Goal:** Make Fajar Lang capable of building a real bare-metal OS on aarch64 (Radxa Dragon Q6A / QCS6490).
> **Scope:** Fix all Blocker (B1-B4), High (H1-H4), and Medium (M1-M3) issues identified in the pre-FajarOS audit.
> **Approach:** Minimal, surgical changes — no rewrites, no new subsystems. Extend what exists.

---

## Status Legend

- `[ ]` Not started
- `[~]` In progress
- `[x]` Complete

---

## Phase 1: BLOCKERS (B1-B4) — Must fix before any OS code

### B1: ARM64 Real Inline Assembly

**Problem:** `compile_inline_asm()` translates asm mnemonics to Cranelift IR (e.g., `asm!("add")` → `builder.ins().iadd()`). This means ARM64-specific instructions like `mrs`, `msr`, `ldr`, `str`, `eret`, `isb`, `dsb`, `tlbi`, `wfi`, `svc` cannot be emitted. These are essential for OS development.

**Root Cause:** Cranelift has no "emit raw bytes" API for inline asm. It only generates IR-level operations.

**Solution:** Emit ARM64 instructions as raw `.word` data using Cranelift's `DataDescription` mechanism for AOT, and keep existing IR-based asm as a fast path for generic ops. For JIT, use runtime function trampolines.

**Files to modify:**
- `src/codegen/cranelift/compile/mod.rs` — extend `compile_inline_asm()`
- `src/codegen/cranelift/mod.rs` — add raw data section emission for AOT
- `src/codegen/cranelift/runtime_fns.rs` — add ARM64 trampoline fns for JIT
- `src/parser/ast.rs` — add `RawAsm` variant (optional, can reuse InlineAsm)

**Implementation:**

#### B1.1: ARM64 Instruction Encoder Module
- [ ] Create `src/codegen/aarch64_asm.rs` — ARM64 instruction encoder
- [ ] Encode system register access: `mrs Xd, <sysreg>` → 0xD5300000 | (op0<<19) | (op1<<16) | (CRn<<12) | (CRm<<8) | (op2<<5) | Rd
- [ ] Encode system register write: `msr <sysreg>, Xt` → 0xD5100000 | ...
- [ ] Encode barriers: `isb` → 0xD5033FDF, `dsb sy` → 0xD503309F, `dmb sy` → 0xD50330BF
- [ ] Encode exception: `eret` → 0xD69F03E0, `svc #imm` → 0xD4000001 | (imm<<5), `wfi` → 0xD503207F
- [ ] Encode TLB ops: `tlbi alle1` → 0xD508871F, `tlbi vae1, Xt` → 0xD5088760 | Rt
- [ ] Encode load/store: `ldr Xt, [Xn, #imm]` → 0xF9400000 | (imm12<<10) | (Rn<<5) | Rt
- [ ] Encode `str Xt, [Xn, #imm]` → 0xF9000000 | (imm12<<10) | (Rn<<5) | Rt
- [ ] Encode `mov Xd, #imm16` → 0xD2800000 | (imm16<<5) | Rd (MOVZ)
- [ ] Encode `movk Xd, #imm16, lsl #shift` → 0xF2800000 | (hw<<21) | (imm16<<5) | Rd
- [ ] Encode `ret` → 0xD65F03C0, `br Xn` → 0xD61F0000 | (Rn<<5)
- [ ] System register constants: SCTLR_EL1, TCR_EL1, MAIR_EL1, TTBR0_EL1, TTBR1_EL1, VBAR_EL1, ESR_EL1, FAR_EL1, ELR_EL1, SPSR_EL1, SP_EL0, DAIF, CurrentEL, ICC_SRE_EL1, ICC_PMR_EL1, ICC_IAR1_EL1, ICC_EOIR1_EL1, ICC_CTRL_EL1, CNTFRQ_EL0, CNTP_TVAL_EL0, CNTP_CTL_EL0
- [ ] Register name → number mapping: x0-x30, sp (31), xzr (31)
- [ ] Tests: encode each instruction category, verify against known encodings

#### B1.2: AOT Raw Section Emission
- [ ] In `ObjectCompiler::compile()`, after function codegen, emit raw asm data sections
- [ ] For each `asm!` with ARM64-only instructions, encode to bytes using `aarch64_asm.rs`
- [ ] Emit as `.text.asm_N` data sections with executable flag
- [ ] Link via linker script `.text.asm_*` placement in `.text` section
- [ ] Alternative: use Cranelift's `raw_binary_emit()` if available; else emit as data + symbol

#### B1.3: Compile-time Asm Routing
- [ ] In `compile_inline_asm()`: check target arch from `CodegenCtx`
- [ ] If target is aarch64 AND mnemonic is ARM64-specific: route to `aarch64_asm` encoder
- [ ] If mnemonic is generic (add, sub, and, or, nop): keep existing Cranelift IR path
- [ ] ARM64-specific mnemonics: `mrs`, `msr`, `ldr`, `str`, `stp`, `ldp`, `eret`, `svc`, `wfi`, `wfe`, `isb`, `dsb`, `dmb`, `tlbi`, `at`, `dc`, `ic`, `mov` (when used with system regs), `movz`, `movk`, `ret`, `br`, `blr`, `b`, `cbz`, `cbnz`, `adr`, `adrp`
- [ ] Parse template: extract mnemonic + register operands from template string
- [ ] Map operand `{0}`, `{1}` to physical register numbers from constraint
- [ ] For `in(reg)`: Cranelift places value in a register; we need the physical reg → use `raw_word` emit at function epilog
- [ ] Error on unsupported ARM64 instructions with clear message

#### B1.4: Register Constraint Enhancement
- [ ] Add ARM64 register names to constraint validation: `x0`-`x30`, `w0`-`w30`, `sp`
- [ ] Constraint `"reg"` on aarch64 = general purpose X register
- [ ] Constraint `"x0"`, `"x1"`, etc. = specific register
- [ ] Constraint `"w0"` etc. = 32-bit register (uses I32 type)

#### B1.5: JIT Trampoline Functions (for host testing)
- [ ] Add `fj_rt_aarch64_mrs(sysreg_id: i64) -> i64` — reads system register on real ARM64 hardware
- [ ] Add `fj_rt_aarch64_msr(sysreg_id: i64, value: i64)` — writes system register
- [ ] Add `fj_rt_aarch64_barrier(kind: i64)` — issues isb/dsb/dmb
- [ ] On x86_64 host: these functions return 0 / no-op (simulation mode)
- [ ] On aarch64 host: these emit actual instructions via inline asm in Rust
- [ ] Register in JIT symbol resolver

#### B1.6: Tests
- [ ] Test ARM64 encoding for each instruction category (unit tests in aarch64_asm.rs)
- [ ] Test compile_inline_asm routes to ARM64 encoder when target is aarch64
- [ ] Test generic mnemonics still use Cranelift IR path
- [ ] Test AOT object file contains correct ARM64 bytes (objdump verification)
- [ ] Test error messages for unsupported instructions
- [ ] Test register constraint validation for ARM64 register names

**Test count: ~30 tests**

---

### B2: Multi-Width Volatile I/O

**Problem:** `fj_rt_volatile_read/write` only accept `*const i64` / `*mut i64`. ARM64 MMIO registers are typically 32-bit. Reading a 32-bit MMIO register as i64 corrupts adjacent registers.

**Solution:** Add u8/u16/u32 variants of volatile read/write runtime functions.

**Files to modify:**
- `src/codegen/cranelift/runtime_fns.rs` — add new functions
- `src/codegen/cranelift/mod.rs` — register new symbols in JIT/AOT
- `src/codegen/cranelift/compile/mod.rs` — add builtin routing for new functions

**Implementation:**

#### B2.1: Runtime Functions
- [ ] Add `fj_rt_volatile_read_u8(addr: *const u8) -> i64`
- [ ] Add `fj_rt_volatile_read_u16(addr: *const u16) -> i64`
- [ ] Add `fj_rt_volatile_read_u32(addr: *const u32) -> i64`
- [ ] Add `fj_rt_volatile_write_u8(addr: *mut u8, value: i64)`
- [ ] Add `fj_rt_volatile_write_u16(addr: *mut u16, value: i64)`
- [ ] Add `fj_rt_volatile_write_u32(addr: *mut u32, value: i64)`
- [ ] All use `std::ptr::read_volatile` / `std::ptr::write_volatile` with proper casts

#### B2.2: Symbol Registration
- [ ] Register all 6 new functions in JIT symbol resolver (`lookup_symbol`)
- [ ] Declare all 6 in AOT function imports (`declare_runtime_functions`)
- [ ] Add Fajar Lang builtins: `volatile_read_u8`, `volatile_read_u16`, `volatile_read_u32`, `volatile_write_u8`, `volatile_write_u16`, `volatile_write_u32`

#### B2.3: Tests
- [ ] Test each volatile function reads/writes correct width
- [ ] Test that u32 volatile write doesn't corrupt adjacent memory
- [ ] Test builtin routing compiles correctly

**Test count: ~10 tests**

---

### B3: Integer Type Width Enforcement

**Problem:** `as` cast between integer types is a no-op in native codegen — `256 as u8` stays 256. Integer literals always produce I64 regardless of declared type.

**Solution:** Add proper `ireduce` / `uextend` / `sextend` instructions in cast codegen, and honor declared types in variable initialization.

**Files to modify:**
- `src/codegen/cranelift/compile/expr.rs` — fix `compile_cast()`
- `src/codegen/cranelift/compile/mod.rs` — honor declared type in let bindings
- `src/codegen/types.rs` — already correct (i32→I32), no changes needed

**Implementation:**

#### B3.1: Fix `compile_cast()` — Integer-to-Integer
- [ ] When casting to `i8`/`u8`: `builder.ins().ireduce(I8, val)` (truncate to 8 bits)
- [ ] When casting to `i16`/`u16`: `builder.ins().ireduce(I16, val)`
- [ ] When casting to `i32`/`u32`: `builder.ins().ireduce(I32, val)`
- [ ] When casting from smaller to larger (e.g., u8→i64): `builder.ins().uextend(I64, val)` or `sextend` for signed
- [ ] When same size: pass through (no-op)
- [ ] Track source type in `cx.last_expr_type` so we know the current width
- [ ] Update `last_expr_type` after cast to reflect new type

#### B3.2: Let Binding Type Honoring
- [ ] When `let x: u32 = expr`, after evaluating expr (I64), insert `ireduce(I32, val)`
- [ ] Store actual Cranelift type in `cx.var_types` for later use
- [ ] This already partially works — `lower_simple_type("u32")` returns `I32`
- [ ] Ensure `builder.declare_variable()` uses correct Cranelift type, not always I64

#### B3.3: Arithmetic Type Propagation
- [ ] When both operands are I32, result should be I32 (not promote to I64)
- [ ] When mixing I32 and I64, extend I32 to I64 before operation
- [ ] This prevents silent overflow bugs in 32-bit register math

#### B3.4: Tests
- [ ] `256 as u8` == 0
- [ ] `0xFFFF as u16` == 65535
- [ ] `-1i8 as i64` == -1 (sign extend)
- [ ] `255u8 as i64` == 255 (zero extend)
- [ ] `let x: u32 = 0xFFFF_FFFF` wraps correctly
- [ ] u32 arithmetic stays u32

**Test count: ~15 tests**

---

### B4: Proper `_start` for Bare-Metal

**Problem:** `_start` is just a wrapper that calls `@entry fn`. No BSS zeroing, no stack pointer setup, no CPU init. On bare-metal, these are essential.

**Solution:** Generate a proper `_start` that: (1) loads stack pointer from linker symbol, (2) zeros BSS, (3) calls @entry, (4) loops forever on return.

**Files to modify:**
- `src/codegen/cranelift/mod.rs` — rewrite _start generation for bare-metal targets
- `src/codegen/linker.rs` — ensure linker symbols are correct

**Implementation:**

#### B4.1: Enhanced _start for Bare-Metal AOT
- [ ] Check if target is bare-metal (`target.is_bare_metal`)
- [ ] If bare-metal, generate _start that:
  1. Sets up stack: load `__stack_top` symbol, emit as initial SP (via assembly or Cranelift global)
  2. Zeros BSS: loop from `__bss_start` to `__bss_end`, writing zeros
  3. Copies .data from LMA to VMA (if flash→RAM layout)
  4. Calls @entry function
  5. On return: infinite loop (`loop { wfi }` on ARM64)
- [ ] For non-bare-metal: keep existing simple wrapper

#### B4.2: Linker Script Additions
- [ ] Add `__data_load_start = LOADADDR(.data)` symbol for data copy
- [ ] Ensure `__bss_start`, `__bss_end`, `__data_start`, `__data_end` are defined (already present)
- [ ] Add `.text.start` section for _start placement at beginning of FLASH

#### B4.3: Exception Vector Stub
- [ ] For aarch64 bare-metal: generate minimal exception vector table
- [ ] 16 entries × 128 bytes = 2048 bytes at VBAR_EL1
- [ ] All vectors jump to infinite loop (default handler)
- [ ] Place in `.text.vectors` section, aligned to 2048 bytes
- [ ] User can override with custom handlers later

#### B4.4: Tests
- [ ] Test _start contains BSS zeroing code
- [ ] Test _start ends with infinite loop, not return
- [ ] Test linker script has all required symbols
- [ ] Test exception vector table is correct size and alignment
- [ ] Test non-bare-metal _start is unchanged

**Test count: ~10 tests**

---

## Phase 2: HIGH PRIORITY (H1-H4) — Before Phase 3-4 of FajarOS

### H1: no_std Hard Enforcement in Codegen

**Problem:** `nostd.rs` returns `Vec<NoStdViolation>` but the compiler ignores violations. Forbidden builtins still compile and link.

**Solution:** Wire violation check into the compilation pipeline; fail compilation if violations found for bare-metal targets.

**Files to modify:**
- `src/codegen/cranelift/mod.rs` — call `check_nostd_compliance()` before codegen
- `src/codegen/nostd.rs` — no changes needed (already returns violations)
- `src/main.rs` — integrate into build pipeline

**Implementation:**

#### H1.1: Pre-Codegen Validation
- [ ] In `ObjectCompiler::compile()`, before function codegen:
  ```
  if self.target.is_bare_metal || self.no_std_mode {
      let violations = check_nostd_compliance(&program, &config);
      if !violations.is_empty() {
          return Err(violations.into_codegen_errors());
      }
  }
  ```
- [ ] Convert `NoStdViolation` to `CodegenError` — new variant `CodegenError::NoStdViolation(String)`
- [ ] Error code: NS001

#### H1.2: Context-Aware Config Selection
- [ ] `@kernel` functions → `NoStdConfig::kernel()` (no heap, no float, no string)
- [ ] Bare-metal target → `NoStdConfig::bare_metal()` (no heap, float OK)
- [ ] Normal mode → no checking

#### H1.3: Tests
- [ ] Bare-metal compile with `tensor_zeros` → compilation error NS001
- [ ] Bare-metal compile with pure arithmetic → success
- [ ] @kernel function with string literal → compilation error
- [ ] Normal mode with tensor_zeros → success (no restriction)

**Test count: ~8 tests**

---

### H2: Pointer Arithmetic in Codegen

**Problem:** `ptr + offset` is generic `iadd` — no pointer-specific semantics. Pointer arithmetic needs to work for MMIO base + register offset patterns.

**Solution:** Pointer arithmetic already works via `iadd` since pointers are I64. The real issue is the interpreter not supporting it. In codegen, it already works. Document this and add interpreter support.

**Files to modify:**
- `src/interpreter/eval.rs` — add pointer arithmetic for `Value::Pointer + Value::Int`
- `src/codegen/cranelift/compile/expr.rs` — verify iadd works for pointer offsets (it does)

**Implementation:**

#### H2.1: Interpreter Pointer Arithmetic
- [ ] In `eval_binary()`, add case for `BinOp::Add` with `Value::Pointer`:
  ```
  (Value::Pointer(addr), Value::Int(offset)) => Value::Pointer(addr + offset as u64)
  (Value::Int(offset), Value::Pointer(addr)) => Value::Pointer(addr + offset as u64)
  ```
- [ ] Add `BinOp::Sub` for pointer - int:
  ```
  (Value::Pointer(addr), Value::Int(offset)) => Value::Pointer(addr - offset as u64)
  ```

#### H2.2: Tests
- [ ] `let p = mem_alloc(16, 8); let q = p + 8` — valid pointer
- [ ] `mem_write_u32(p + 4, 42)` — write at offset
- [ ] Pointer subtraction: `p - 4` → valid pointer

**Test count: ~6 tests**

---

### H3: Real MMIO Runtime Functions

**Problem:** All `src/runtime/os/` modules use in-memory simulation (HashMap, Vec). For real hardware, volatile_read/write builtins cover this, but the interpreter's OS builtins (`mem_alloc`, `irq_register`, etc.) are misleading.

**Solution:** This is NOT a code fix — it's an architecture decision. The existing simulation layer is correct for interpreter/testing. Real hardware access will use volatile_read/volatile_write builtins in compiled code. Document the split clearly.

**Files to modify:**
- None for code. Documentation only.

**Implementation:**

#### H3.1: Documentation
- [ ] Add comment header to `src/runtime/os/mod.rs` explaining simulation vs real hardware
- [ ] Document in CLAUDE.md: interpreter OS builtins = simulation; real MMIO = volatile builtins in native code
- [ ] Add "OS Runtime Architecture" section to V30_SKILLS.md

#### H3.2: Interpreter MMIO Passthrough (optional, for host testing with `/dev/mem`)
- [ ] This is DEFERRED — not needed for cross-compiled bare-metal code
- [ ] Real hardware interaction only happens in AOT-compiled binaries running on target

**Test count: 0 (documentation only)**

---

### H4: Context Enforcement in Native Codegen

**Problem:** @kernel/@device context annotations are only checked by the semantic analyzer. Native codegen doesn't enforce restrictions — a @kernel function can call heap-allocating builtins.

**Solution:** Add codegen-level enforcement by checking function annotations before emitting builtin calls.

**Files to modify:**
- `src/codegen/cranelift/compile/mod.rs` — check context before emitting forbidden builtins
- `src/codegen/cranelift/mod.rs` — track current function's context annotation

**Implementation:**

#### H4.1: Context Tracking in Codegen
- [ ] Add `current_context: Option<ContextAnnotation>` to `CodegenCtx`
- [ ] Set it when entering a function with @kernel/@device/@safe/@unsafe annotation
- [ ] Clear it on function exit

#### H4.2: Builtin Call Gating
- [ ] Before emitting call to tensor builtins: check `cx.current_context != Some(Kernel)`
- [ ] Before emitting call to heap builtins in @kernel: error KE001
- [ ] Before emitting raw pointer ops in @device: error DE001
- [ ] Error type: `CodegenError::ContextViolation { context, operation, code }`

#### H4.3: Tests
- [ ] @kernel fn calling tensor_zeros → CE error KE002
- [ ] @kernel fn calling mem_alloc → CE error KE001
- [ ] @device fn with raw pointer → CE error DE001
- [ ] @safe fn with normal code → success
- [ ] @unsafe fn with everything → success

**Test count: ~10 tests**

---

## Phase 3: MEDIUM (M1-M3) — Can be fixed incrementally

### M1: Pointer Dereference Syntax

**Problem:** `*ptr` doesn't work in expressions. Must use `mem_read_u32(ptr)` instead.

**Solution:** Add `UnaryOp::Deref` handling in both interpreter and codegen.

**Files to modify:**
- `src/interpreter/eval.rs` — handle `UnaryOp::Deref` for `Value::Pointer`
- `src/codegen/cranelift/compile/expr.rs` — emit `load` instruction for `UnaryOp::Deref`

**Implementation:**

#### M1.1: Interpreter Deref
- [ ] In `eval_unary()`, add:
  ```
  (UnaryOp::Deref, Value::Pointer(addr)) => {
      self.os.memory.read_u64(addr)
  }
  ```
- [ ] Support `*ptr = value` assignment in `eval_assign()`

#### M1.2: Codegen Deref
- [ ] In `compile_unary()`, add:
  ```
  UnaryOp::Deref => {
      let addr = compile_expr(builder, cx, operand)?;
      Ok(builder.ins().load(I64, MemFlags::new(), addr, 0))
  }
  ```
- [ ] For assignment: `builder.ins().store(MemFlags::new(), value, addr, 0)`

#### M1.3: Tests
- [ ] `let p = mem_alloc(8, 8); *p = 42; assert(*p == 42)`
- [ ] Deref in expressions: `let x = *p + 1`
- [ ] Deref assignment: `*p = *p + 1`

**Test count: ~6 tests**

---

### M2: Cast Truncation (Interpreter)

**Problem:** Interpreter's `eval_cast()` returns same i64 value for all integer casts. `256 as u8` should be 0.

**Solution:** Add truncation/extension logic to interpreter's cast handling.

**Files to modify:**
- `src/interpreter/eval.rs` — fix `eval_cast()` for integer types

**Implementation:**

#### M2.1: Interpreter Cast Fix
- [ ] `val as u8` → `val & 0xFF`
- [ ] `val as u16` → `val & 0xFFFF`
- [ ] `val as u32` → `val & 0xFFFF_FFFF`
- [ ] `val as i8` → sign-extend from 8 bits: `((val & 0xFF) as i8) as i64`
- [ ] `val as i16` → sign-extend from 16 bits
- [ ] `val as i32` → sign-extend from 32 bits
- [ ] `val as u64` / `val as i64` → no-op (already i64)

#### M2.2: Tests
- [ ] `256 as u8` == 0
- [ ] `65536 as u16` == 0
- [ ] `-1 as u8` == 255
- [ ] `128 as i8` == -128
- [ ] `0xFFFF_FFFF as u32` == 4294967295
- [ ] `0x1_0000_0000 as u32` == 0

**Test count: ~8 tests**

---

### M3: Compile-Time Const Evaluation

**Problem:** `const PAGE_SIZE = 4096` is evaluated at runtime in the interpreter. Not blocking but semantically incorrect.

**Solution:** This is a DEFERRED fix. The interpreter's behavior is correct for now — consts are immutable let bindings. True compile-time evaluation requires a const-eval pass, which is a larger feature.

**Implementation:**

#### M3.1: Codegen Const Folding (Already Partial)
- [ ] Verify that Cranelift already folds constants during optimization
- [ ] `const X = 4096; let y = X * 2` → should optimize to `let y = 8192`
- [ ] Add test to verify constant propagation in native codegen
- [ ] Document: interpreter evaluates const at first encounter; native codegen folds at compile time

#### M3.2: Tests
- [ ] Const value propagates in native codegen
- [ ] Const arithmetic optimized away in native output

**Test count: ~4 tests**

---

## Summary

| Issue | Phase | Tasks | Tests | Files Modified |
|-------|-------|-------|-------|----------------|
| **B1: ARM64 Inline Asm** | 1 | 6 sub-tasks | ~30 | 4 files + 1 new |
| **B2: Multi-Width Volatile** | 1 | 3 sub-tasks | ~10 | 3 files |
| **B3: Integer Type Width** | 1 | 4 sub-tasks | ~15 | 2 files |
| **B4: Proper _start** | 1 | 4 sub-tasks | ~10 | 2 files |
| **H1: no_std Enforcement** | 2 | 3 sub-tasks | ~8 | 3 files |
| **H2: Pointer Arithmetic** | 2 | 2 sub-tasks | ~6 | 2 files |
| **H3: MMIO Documentation** | 2 | 2 sub-tasks | 0 | docs only |
| **H4: Context in Codegen** | 2 | 3 sub-tasks | ~10 | 2 files |
| **M1: *ptr Dereference** | 3 | 3 sub-tasks | ~6 | 2 files |
| **M2: Cast Truncation** | 3 | 2 sub-tasks | ~8 | 1 file |
| **M3: Const Evaluation** | 3 | 2 sub-tasks | ~4 | docs + verify |
| **TOTAL** | 3 phases | 34 sub-tasks | ~107 tests | ~12 files |

---

## Execution Order

```
Phase 1 (Blockers) — Sequential, each builds on previous:
  B2 (volatile widths)     ← Simplest, no dependencies, warm-up
  B3 (integer widths)      ← Needed for B1 (ARM64 uses u32 registers)
  B1 (ARM64 asm)           ← Largest task, uses B2+B3
  B4 (bare-metal _start)   ← Uses B1 for ARM64 exception vectors

Phase 2 (High) — Can be parallelized:
  H1 (no_std enforcement)  ← Independent
  H2 (pointer arithmetic)  ← Independent
  H3 (documentation)       ← Independent
  H4 (context in codegen)  ← Independent

Phase 3 (Medium) — Can be done incrementally:
  M2 (cast truncation)     ← Quick fix
  M1 (*ptr dereference)    ← Depends on H2
  M3 (const eval)          ← Verify + document
```

---

## Quality Gates

### Per-Task Gate
- [ ] All new tests pass
- [ ] `cargo test` — zero failures (existing tests unbroken)
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo fmt -- --check` — clean
- [ ] No `.unwrap()` in `src/`
- [ ] All `unsafe` blocks have `// SAFETY:` comment

### Per-Phase Gate
- [ ] Full test suite passes: `cargo test && cargo test --features native`
- [ ] No regressions in existing 5,236 tests
- [ ] New tests added to count
- [ ] All examples still run: `cargo run -- run examples/*.fj`

### Final Gate (All Phases Complete)
- [ ] Can compile a minimal aarch64 bare-metal program:
  ```fajar
  @entry
  @kernel fn boot() -> ! {
      let uart_base: u64 = 0x0984_0000
      volatile_write_u32(uart_base as *mut u32, 0x48)  // 'H'
      loop { asm!("wfi") }
  }
  ```
- [ ] Produces valid ELF binary for `aarch64-unknown-none`
- [ ] `aarch64-linux-gnu-objdump -d output.o` shows correct ARM64 instructions
- [ ] Binary < 16KB for minimal program

---

## Risk Analysis

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Cranelift can't emit raw bytes in .text | Medium | B1 blocked | Use data section + linker script to place in .text; or use Cranelift's `trap` instruction slots |
| ARM64 encoding bugs | Medium | Wrong instructions | Verify every encoding against ARM ARM (Architecture Reference Manual) |
| Integer width changes break existing tests | High | Regression | Run full test suite after each change; may need to update tests that depend on i64 behavior |
| _start changes break Linux user-mode tests | Low | QEMU tests fail | Only modify _start for bare-metal targets; keep Linux target unchanged |

---

*Plan Version: 1.0 | Created: 2026-03-12 | Target: Pre-FajarOS V3.0 readiness*
