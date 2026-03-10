# Gap Analysis — Fajar Lang Documentation

> Comprehensive cross-document analysis of all 18+ documentation files.
> Generated: 2026-03-05 | Analyzer: Claude Opus 4.6

---

## Table of Contents

1. [Cross-Document Conflicts](#1-cross-document-conflicts)
2. [Spec vs Implementation Plan Gaps](#2-spec-vs-implementation-plan-gaps)
3. [Architecture vs API Reference Discrepancies](#3-architecture-vs-api-reference-discrepancies)
4. [Testing Coverage Gaps](#4-testing-coverage-gaps)
5. [Missing Features Analysis](#5-missing-features-analysis)
6. [Language Design Ambiguities](#6-language-design-ambiguities)
7. [Error Code Gaps](#7-error-code-gaps)
8. [Critical Priority Items](#8-critical-priority-items)

---

## 1. Cross-Document Conflicts

### Conflict 1.1 — Interpreter Input: `&TypedProgram` vs `&Program`

**Documents:** CLAUDE.md (Section 4.2) vs IMPLEMENTATION_PLAN.md (Sprint 1.5)

- **CLAUDE.md** (Section 4.2, Module Contracts table):
  > `Interpreter | eval_program(&mut self, prog) | &TypedProgram -> Result<Value, RuntimeError>`

- **ARCHITECTURE.md** (Section 2.4):
  > `pub fn eval_program(&mut self, program: &TypedProgram) -> Result<Value, RuntimeError>;`

- **IMPLEMENTATION_PLAN.md** (Step 1.5.1):
  > `pub fn eval_program(&mut self, program: &Program) -> Result<Value, RuntimeError>;`

The IMPLEMENTATION_PLAN uses `&Program` (untyped AST), which makes sense for Phase 1 where the semantic analyzer does not yet exist. But CLAUDE.md and ARCHITECTURE.md say `&TypedProgram`, implying the analyzer is always in the pipeline.

**Resolution:** IMPLEMENTATION_PLAN.md is correct for Phase 1. CLAUDE.md and ARCHITECTURE.md should note that Phase 1 skips the analyzer and interprets untyped `Program` directly, then upgrades to `TypedProgram` in Phase 2. Add an explicit note in ARCHITECTURE.md: "Phase 1: Interpreter accepts `&Program` directly. Phase 2+: Interpreter accepts `&TypedProgram` after analysis."

---

### Conflict 1.2 — Interpreter Phase Range: "Phase 1-3" vs "Phase 1-4"

**Documents:** FAJAR_LANG_SPEC.md vs CLAUDE.md

- **FAJAR_LANG_SPEC.md** (Section 14.1, compilation pipeline):
  > `INTERPRETER (Phase 1-3) Tree-walking`

- **CLAUDE.md** (Section 4.1):
  > `INTERPRETER (Phase 1-4) Tree-walking`

**Resolution:** CLAUDE.md is authoritative. The tree-walking interpreter is used through Phase 4 (ML Runtime), with the bytecode VM coming in Phase 5. FAJAR_LANG_SPEC.md should be updated from "Phase 1-3" to "Phase 1-4".

---

### Conflict 1.3 — `@safe` Context: Tensor Ops Allowed or Forbidden?

**Documents:** CLAUDE.md (Section 5.3) vs STDLIB_SPEC.md (Section 4) vs TESTING.md (Section 5)

- **CLAUDE.md** (Section 5.3, Context Annotations table):
  > `zeros(3,4) / relu() | @safe: ERROR`

- **STDLIB_SPEC.md** (Section 4):
  > `Semua fungsi nn:: optimal di @device context. Bisa dipanggil di @safe tapi tanpa hardware acceleration.`

- **TESTING.md** (Section 5, Context Annotation Test Matrix):
  > `tensor: zeros(3,4) | @safe: ERROR`
  > `relu() | @safe: ERROR`

The STDLIB_SPEC explicitly says nn:: functions CAN be called in `@safe` (just without hardware acceleration), while CLAUDE.md and TESTING.md say they produce errors. This is a fundamental semantic conflict.

**Resolution:** This needs an explicit design decision. Two options:
1. **Tensor ops restricted to @device/@unsafe only** (CLAUDE.md + TESTING.md position) -- tighter safety, consistent with context isolation philosophy.
2. **Tensor ops available everywhere, accelerated in @device** (STDLIB_SPEC position) -- more flexible, easier for beginners.

**Recommendation:** Option 1 is more consistent with the "dual-context safety" design principle. STDLIB_SPEC.md should be corrected. If the design intent is that `@safe` code cannot call ML functions directly, the bridge pattern shown in EXAMPLES.md Section 5.1 must use `@device` for inference calls rather than `@safe`.

---

### Conflict 1.4 — Tensor Error Code TE002 vs TE003 for Matmul Shape Mismatch

**Documents:** CLAUDE.md vs ERROR_CODES.md vs FAJAR_LANG_SPEC.md

- **CLAUDE.md** (Section 7):
  > `TE002 MatmulShapeMismatch -- inner dims don't match for @`

- **ERROR_CODES.md** (Section 6):
  > `TE001 ShapeMismatch -- Dimensi tensor tidak kompatibel untuk operasi`
  > `TE002 MatmulShapeMismatch -- Inner dimensions tidak cocok untuk matrix multiply`

- **FAJAR_LANG_SPEC.md** (Section 13.4):
  > `error[TE003]: matrix multiply shape mismatch`

FAJAR_LANG_SPEC.md uses TE003 for matmul shape mismatch, but ERROR_CODES.md and CLAUDE.md define it as TE002.

**Resolution:** ERROR_CODES.md should be authoritative for error codes. FAJAR_LANG_SPEC.md Section 13.4 should be corrected from `TE003` to `TE002`.

---

### Conflict 1.5 — Phase 1 Current Status Discrepancy

**Documents:** IMPLEMENTATION_PLAN.md vs CLAUDE.md vs PLANNING.md

- **IMPLEMENTATION_PLAN.md** (Status Overview):
  > `Current Step: Phase 0 -- Project belum di-scaffold`

- **CLAUDE.md** (Section 3):
  > `Current Phase: 1 -- Core Language Foundation, Current Sprint: 1.1 -- Lexer`

- **PLANNING.md**:
  > `Current Phase: 1 -- Core Language Foundation, Current Sprint: 1.1 -- Lexer, Status: IN PROGRESS`

IMPLEMENTATION_PLAN.md says Phase 0 has not started, while CLAUDE.md and PLANNING.md say Sprint 1.1 is in progress.

**Resolution:** All documents should be synchronized. Since CLAUDE.md is the auto-loaded master reference, IMPLEMENTATION_PLAN.md must be updated. The status should consistently read "Phase 1, Sprint 1.1 -- Lexer" or "Phase 0 -- Scaffolding" depending on what has actually been done.

---

### Conflict 1.6 — IMPLEMENTATION_PLAN.md Has a "Phase 0" Not in Other Documents

**Documents:** IMPLEMENTATION_PLAN.md vs PLANNING.md vs TASKS.md

- **IMPLEMENTATION_PLAN.md** includes a full "Phase 0 -- Project Scaffolding" with Steps 0.1-0.5.
- **PLANNING.md** starts directly at "Phase 1 -- Core Language Foundation".
- **TASKS.md** starts at "Sprint 1.1 -- Lexer" with T1.1.1 being "Project Scaffolding".

The IMPLEMENTATION_PLAN treats scaffolding as a separate Phase 0, while TASKS.md embeds it into Sprint 1.1 Task T1.1.1.

**Resolution:** Align on one approach. IMPLEMENTATION_PLAN's Phase 0 is more granular (5 steps for scaffolding), which is better for execution. TASKS.md T1.1.1 should reference IMPLEMENTATION_PLAN Steps 0.1-0.5, or PLANNING.md should add a Phase 0 entry.

---

### Conflict 1.7 — `logos` Dependency: Used or Not?

**Documents:** CLAUDE.md (Section 15) vs IMPLEMENTATION_PLAN.md vs SKILLS.md

- **CLAUDE.md** (Section 15, Cargo.toml):
  > `logos = "0.14" # Lexer (optional, can use hand-written)`

- **ARCHITECTURE.md** (Section 5, Cargo.toml):
  > `logos = "0.14"`

- **IMPLEMENTATION_PLAN.md** (Step 0.1, Cargo.toml):
  Does NOT include `logos` in the dependency list.

- **SKILLS.md** (Section 1.1-1.3): Shows hand-written Cursor-based lexer pattern, no logos usage.

**Resolution:** The IMPLEMENTATION_PLAN and SKILLS.md clearly design a hand-written lexer (Cursor pattern). The `logos` dependency should be removed from CLAUDE.md and ARCHITECTURE.md Cargo.toml listings, OR a note should be added: "logos is listed but NOT used in Phase 1. The lexer is hand-written for educational clarity and maximum control."

---

### Conflict 1.8 — Value Enum: `BuiltinFn(String)` Present or Absent

**Documents:** CLAUDE.md (Section 4.4) vs ARCHITECTURE.md (Section 2.4) vs API_REFERENCE.md (Section 5)

- **CLAUDE.md** (Section 4.4):
  > Includes `BuiltinFn(String)` in Value enum

- **ARCHITECTURE.md** (Section 2.4):
  > Does NOT include `BuiltinFn(String)` in Value enum

- **API_REFERENCE.md** (Section 5):
  > Includes `BuiltinFn(String)` in Value enum

- **IMPLEMENTATION_PLAN.md** (Step 1.2.1):
  > Does NOT include `BuiltinFn` in the Value enum (it defines Value in interpreter, not parser)

**Resolution:** `BuiltinFn(String)` is needed to represent built-in functions like `print`, `println`, `len` etc. ARCHITECTURE.md should be updated to include it.

---

### Conflict 1.9 — Binding Power Numbers Do Not Map to Spec Precedence Levels

**Documents:** CLAUDE.md (Section 5.2) vs SKILLS.md (Section 2.2) vs GRAMMAR_REFERENCE.md (Section 5)

- **CLAUDE.md** operator precedence table lists 11 levels (1=lowest, 11=highest).
- **SKILLS.md** binding power table uses numeric pairs: Pipe=(2,3), Or=(4,5), etc.
- **GRAMMAR_REFERENCE.md** expresses precedence implicitly through grammar rules (assignment > pipeline > logic_or > ...) but adds a `range` level between `comparison` and `addition` that is NOT present in CLAUDE.md or FAJAR_LANG_SPEC.md.

**GRAMMAR_REFERENCE.md** (Section 5):
```
comparison  = range { ('<' | '>' | '<=' | '>=') range } ;
range       = addition [ '..' [ '=' ] addition ] ;
addition    = multiply { ('+' | '-') multiply } ;
```

**CLAUDE.md** (Section 5.2): No `range` precedence level exists between comparison and addition. Range is not listed at all.

**Resolution:** The GRAMMAR_REFERENCE.md adds a `range` precedence level that is absent from the CLAUDE.md and FAJAR_LANG_SPEC.md precedence tables. The precedence table needs to be updated to include `range` between comparison and addition (or the grammar must be restructured). Additionally, the binding power numbers in SKILLS.md need to add a `Range` entry. **The GRAMMAR_REFERENCE.md is more complete and should be authoritative for the grammar.**

---

### Conflict 1.10 — `assert_tokens!` Macro: Filters EOF or Not

**Documents:** TESTING.md vs RULES.md

- **TESTING.md** (Section 4.1):
  > Macro filters out EOF: `.filter(|t| t.kind != TokenKind::Eof)`

- **RULES.md** (Section 3.3):
  > Macro does NOT filter EOF: `.map(|t| t.kind.clone()).collect()`

These two macros produce different results. The TESTING.md version excludes EOF, while the RULES.md version includes it.

**Resolution:** The TESTING.md version is correct. EOF should be filtered out for cleaner test assertions. RULES.md should be updated to match.

---

### Conflict 1.11 — `@safe` Context Allows Tensor Calls in Cross-Domain Bridge Examples

**Documents:** FAJAR_LANG_SPEC.md (Section 6.7) vs CLAUDE.md (Section 5.3)

- **FAJAR_LANG_SPEC.md** (Section 6.7, Cross-Context Bridge):
  > Shows `@safe fn ai_kernel_monitor(...)` calling `inference_forward(input)` which is a tensor/ML function.

- **CLAUDE.md** (Section 5.3):
  > `@safe` context shows `zeros(3,4) / relu()` as `ERROR`.

If `@safe` prohibits tensor operations, the cross-domain bridge example in FAJAR_LANG_SPEC.md is invalid because `inference_forward` (which calls `relu`, `softmax`, matmul) runs under `@device` but is called FROM `@safe`. The question is whether `@safe` can CALL a `@device` function, even if it cannot directly use tensor ops.

**Resolution:** A "context calling convention" must be defined. Can `@safe` code call `@device` functions? Can `@safe` code call `@kernel` functions? The spec implies yes (the bridge example is the core value prop), but the rules are not explicitly stated. This is a critical design gap -- see Section 6 for details.

---

## 2. Spec vs Implementation Plan Gaps

### 2.1 Features in Spec but Missing from Implementation Tasks

| Feature | Source Document | Missing From |
|---------|----------------|-------------|
| `async fn` / `await` | FAJAR_LANG_SPEC.md Section 5.3 | TASKS.md, IMPLEMENTATION_PLAN.md (Phase 1-4) -- only mentioned in PLANNING.md Blockers as "add in Phase 4" |
| `loop` with `break` value | FAJAR_LANG_SPEC.md Section 5.2 | GRAMMAR_REFERENCE.md lists `loop_expr` but TASKS.md has no `loop` parsing task |
| `@ffi("C") extern` block | FAJAR_LANG_SPEC.md Section 6.6 | Not in any TASKS.md or IMPLEMENTATION_PLAN.md phase |
| Named function arguments | FAJAR_LANG_SPEC.md Section 4.3: `add(a: 1, b: 2)` | Not in GRAMMAR_REFERENCE.md, not in TASKS.md, not in IMPLEMENTATION_PLAN.md |
| Lambda / closure syntax | EXAMPLES.md Section 1.4: `\|x\| x * 2` | Not explicitly in TASKS.md parser tasks; GRAMMAR_REFERENCE.md has no closure/lambda production rule |
| `const` statement at file scope | FAJAR_LANG_SPEC.md Section 5.1 | GRAMMAR_REFERENCE.md lists `const_stmt` but `item` rule does not include `const_def` |
| `trait_def` in item rule | GRAMMAR_REFERENCE.md Section 2 | Present in grammar, but TASKS.md has no explicit trait parsing task (only impl blocks at T1.3.12) |
| Type aliases (`type Name = ...`) | FAJAR_LANG_SPEC.md Section 2.1 lists `type` keyword | No grammar rule, no TASKS.md entry, no IMPLEMENTATION_PLAN.md step |
| Bitwise operators in expressions | FAJAR_LANG_SPEC.md Section 2.2 | GRAMMAR_REFERENCE.md Section 5 does NOT include bitwise operators in its expression grammar. The `multiply` rule jumps to `unary` without a bitwise level |
| `@device(cpu)` / `@device(gpu)` / `@device(auto)` | FAJAR_LANG_SPEC.md Section 6.3 | GRAMMAR_REFERENCE.md annotation rule supports `annotation_args` but no specific validation. TASKS.md has no device target parsing task |
| Capability-based unsafe | SECURITY.md Section 7.2 | Labeled as "Future Enhancement" -- not in any implementation phase |
| `@fuzz` annotation | SECURITY.md Section 8.2 | Not in any implementation plan |
| `fj audit` command | SECURITY.md Section 8.1 | Not in CLI tasks (Sprint 1.6) or any later phase |

### 2.2 Features in Implementation Plan but Not in Spec

| Feature | Source | Missing From Spec |
|---------|--------|-------------------|
| `@kernel fn` annotation parsing | IMPLEMENTATION_PLAN.md Step 1.3.9 | Parsing is covered but runtime behavior in Phase 1 (before analyzer) is undefined |
| `eval_source(&mut self, source: &str)` | IMPLEMENTATION_PLAN.md Step 1.5.1 | Not in ARCHITECTURE.md module contracts, not in API_REFERENCE.md (it IS in API_REFERENCE.md Section 5 Interpreter but not in ARCHITECTURE.md Section 2.4) |
| Property-based tests with `proptest` | TESTING.md Section 2.4 | `proptest` not in Cargo.toml dev-dependencies |
| `fj fmt` CLI subcommand | IMPLEMENTATION_PLAN.md Phase 5 | TESTING.md Section 3 references "fmt" in CLI coverage at 90%, but CLAUDE.md CLI commands list does not include `fj fmt` for Phase 1 |

### 2.3 Order/Priority Mismatches

| Mismatch | Details |
|----------|---------|
| Phase 3 and 4 parallelism | IMPLEMENTATION_PLAN.md dependency graph shows Phase 3 and Phase 4 as parallelizable (both depend on Phase 2). But PLANNING.md lists them sequentially (Phase 3 then Phase 4). TASKS.md also lists them sequentially. |
| Trait parsing in Phase 1 | IMPLEMENTATION_PLAN.md Step 1.3.11 includes trait definitions. TASKS.md T1.3.12 says "Parse impl blocks" but does not mention traits. GRAMMAR_REFERENCE.md includes `trait_def` in the `item` rule. |
| `const_def` as item vs statement | GRAMMAR_REFERENCE.md Section 2 `item` rule does NOT include `const_def`. Section 4 has `const_stmt`. IMPLEMENTATION_PLAN.md Step 1.2.3 `Item` enum does not include `ConstDef`. But FAJAR_LANG_SPEC.md Section 5.1 shows `const MAX: usize = 1024` at file scope. |

---

## 3. Architecture vs API Reference Discrepancies

### 3.1 Struct/Enum Mismatches

| Structure | ARCHITECTURE.md | API_REFERENCE.md | Discrepancy |
|-----------|----------------|-------------------|-------------|
| `Value` enum | Missing `BuiltinFn(String)` | Includes `BuiltinFn(String)` | ARCHITECTURE.md is incomplete |
| `Interpreter` fields | `env`, `os_rt`, `ml_rt` | `env`, `os_rt`, `ml_rt` | Match (OK) |
| `Interpreter` methods | `new()`, `eval_program()`, `eval_expr()`, `call_fn()` | `new()`, `eval_program()`, `eval_source()` | `eval_expr()` and `call_fn()` are in ARCHITECTURE.md but NOT in API_REFERENCE.md. `eval_source()` is in API_REFERENCE.md but NOT in ARCHITECTURE.md |
| `LexError` fields | Not specified in detail | Detailed: `UnexpectedChar { ch, line, col, span }` | API_REFERENCE.md is more complete |
| `Item` enum | Not defined | `FnDef`, `StructDef`, `EnumDef`, `ImplBlock`, `UseDecl`, `ModDecl` | API_REFERENCE.md is more complete but is MISSING `TraitDef` (which is in GRAMMAR_REFERENCE.md) |
| `ScopeKind` enum | Listed in ARCHITECTURE.md: `Function`, `Block`, `Module`, `Kernel`, `Device` | Listed in API_REFERENCE.md: `Module`, `Function`, `Block`, `Kernel`, `Device`, `Unsafe` | ARCHITECTURE.md is missing `Unsafe` variant |

### 3.2 Missing Function Signatures

| Function | Document That Defines It | Missing From |
|----------|--------------------------|-------------|
| `FjPipeline::run()` | API_REFERENCE.md | ARCHITECTURE.md (pipeline is shown as data flow but `FjPipeline` struct is not defined) |
| `FjPipeline::check()` | API_REFERENCE.md | ARCHITECTURE.md |
| `FjPipeline::tokenize()` | API_REFERENCE.md | ARCHITECTURE.md (only `tokenize()` as free function) |
| `Interpreter::eval_source()` | API_REFERENCE.md | ARCHITECTURE.md |
| `Interpreter::eval_expr()` | ARCHITECTURE.md | API_REFERENCE.md |
| `Interpreter::call_fn()` | ARCHITECTURE.md | API_REFERENCE.md |

### 3.3 Module Organization Differences

- **API_REFERENCE.md** defines `FjPipeline` as a struct in `src/lib.rs` with methods `run()`, `check()`, `tokenize()`, `parse()`.
- **ARCHITECTURE.md** does NOT mention `FjPipeline` at all -- it describes the pipeline as a sequence of free function calls.
- **IMPLEMENTATION_PLAN.md** does NOT define `FjPipeline` in any step.

**Resolution:** Decide whether `FjPipeline` is a real struct or just conceptual. If real, add it to ARCHITECTURE.md and IMPLEMENTATION_PLAN.md. If conceptual, remove it from API_REFERENCE.md and show the pipeline as free function calls.

---

## 4. Testing Coverage Gaps

### 4.1 Test Categories in TESTING.md Missing from TASKS.md

| Test Category | TESTING.md Reference | TASKS.md Status |
|---------------|---------------------|-----------------|
| Property-based tests (proptest) | Section 2.4 | No tasks mention proptest at all |
| End-to-end tests (`tests/e2e_tests.rs`) | Section 2.3 | TASKS.md mentions integration tests in T1.5.13-15 but not a separate e2e file |
| Context annotation test matrix (27 cells) | Section 5 | No Phase 2 task explicitly covers all 27 combinations |
| Benchmark tests (criterion) | Section 6 | TASKS.md has no benchmark-related tasks in any phase |
| CI/CD pipeline setup | Section 7 | No task for creating `.github/workflows/ci.yml` |

### 4.2 Coverage Requirements Not Reflected in Tasks

| Component | TESTING.md Target | TASKS.md Coverage |
|-----------|------------------|-------------------|
| Lexer | 100% every TokenKind | T1.1.3 says "Comprehensive test suite" but does not enumerate specific TokenKind coverage |
| Parser | 100% every AST node | T1.3.15 says "Parser test suite" but does not list specific node coverage |
| CLI | 90%+ every subcommand | T1.6.1-T1.6.4 cover subcommands but no explicit CLI test tasks |
| Analyzer | 100% every error kind | Phase 2 T2.10 is a single "Test suite" task -- too coarse |
| ML Runtime | Numerical gradient check | Phase 4 T4.12 mentions "MNIST + XOR integration tests" but not specifically gradient correctness tests |

### 4.3 Test Infrastructure Not in Tasks

| Missing | Details |
|---------|---------|
| Test helper macros | `assert_tokens!`, `assert_eval!`, `assert_compile_error!` are defined in TESTING.md and RULES.md but no TASKS.md item creates them |
| `proptest` dev-dependency | Not listed in any Cargo.toml |
| `e2e_tests.rs` file creation | Not in TASKS.md |
| `run_fj_file()` test helper | Referenced in TESTING.md Section 2.3 but not created by any task |

---

## 5. Missing Features Analysis

### 5.1 Features in EXAMPLES.md Without Implementation Tasks

| Feature Used in Example | Example File | Implementation Task? |
|------------------------|--------------|---------------------|
| `use std::io::println` (module import) | hello.fj (Section 1.1) | Module resolution is NOT implemented in Phase 1. No task addresses how `use` imports are resolved at runtime. |
| `.to_string()` method | variables.fj (Section 1.2) | No built-in `to_string()` method defined in STDLIB_SPEC.md or TASKS.md |
| `.sqrt()` method on floats | structs.fj (Section 1.5) | `sqrt` is in `std::math` (STDLIB_SPEC) but is a free function, not a method. Example uses method syntax. |
| Trait bounds `<T: Printable>` | generics.fj (Section 2.3) | Phase 2, but no specific task for trait bound checking |
| `model.parameters()` returning Vec<Tensor> | xor_nn.fj (Section 4.1) | No task defines how `parameters()` method auto-collects learnable params |
| `score.item()` (scalar extraction from tensor) | ai_kernel_monitor.fj (Section 5.1) | Not in STDLIB_SPEC.md nn::tensor API |
| `Tensor::from_data(&[f32], &[usize])` with int data | xor_nn.fj: `&[0,0, 0,1, 1,0, 1,1]` | The `from_data` signature takes `&[f32]` but the example passes integer literals |
| Pipeline with method calls | xor_nn.fj: `x \|> self.hidden.forward \|> relu` | Pipeline with method references is not in the grammar. Pipeline rule is: `pipeline = logic_or { '\|>' logic_or }` which expects expressions, not method references |
| `String` concatenation with `+` | Multiple examples | No operator overloading defined for String + in STDLIB_SPEC.md |

### 5.2 Features in DIFFERENTIATION.md Without Tasks

| Differentiator Feature | Referenced In | Task Exists? |
|------------------------|--------------|--------------|
| Compile-time tensor shape checking | DIFFERENTIATION.md Section 5.3 | Phase 2 Sprint 2.6 (exists but coarse) |
| Cross-domain bridge pattern | DIFFERENTIATION.md Section 5.2 | No specific integration task tests this pattern end-to-end |
| `fj audit` security command | SECURITY.md Section 8.1 | Not in any phase |

### 5.3 Features in SECURITY.md Without Tasks

| Security Feature | SECURITY.md Section | Implementation Task? |
|-----------------|---------------------|---------------------|
| Capability-based unsafe `@unsafe(capability: [raw_pointer, port_io])` | Section 7.2 | None (labeled "Future Enhancement" with no phase target) |
| `@fuzz` annotation | Section 8.2 | "Phase 6+" but no task exists |
| `fj audit --report` command | Section 8.1 | Not in any TASKS.md entry |
| Compliance assessment (MISRA, DO-178C, IEC 62304, ISO 26262) | Section 10 | Phase 7 in PLANNING.md but no specific tasks in TASKS.md |
| Fuzzing support (AFL++, libFuzzer) | Section 8.2 | Phase 7 in PLANNING.md, no specific tasks |

---

## 6. Language Design Ambiguities

### 6.1 Grammar Rules Missing from GRAMMAR_REFERENCE.md

| Missing Rule | Where It's Used | Impact |
|-------------|----------------|--------|
| Lambda / closure expressions | EXAMPLES.md Section 1.4: `\|x\| x * 2` | Parser cannot handle closures without a grammar rule |
| `loop` expression | GRAMMAR_REFERENCE.md Section 8 lists `loop_expr = 'loop' block_expr` but this is not reachable -- `loop` is not a keyword in the `primary` expression alternatives | Parser will reject `loop { ... }` |
| Named arguments `foo(a: 1, b: 2)` | FAJAR_LANG_SPEC.md Section 4.3 | No grammar rule distinguishes named vs positional args; `call = '(' [ expr { ',' expr } ] ')'` does not support `name: value` pairs |
| String interpolation | Not mentioned anywhere | Is this supported? Not specified. |
| `as` cast operator | FAJAR_LANG_SPEC.md Section 13.3: `x as i64` | Not in GRAMMAR_REFERENCE.md expression rules. No precedence level defined. |
| Bitwise operators | FAJAR_LANG_SPEC.md Section 2.2 lists `& \| ^ ~ << >>` | GRAMMAR_REFERENCE.md expression rules have no bitwise level between any precedence levels |
| `const_def` as top-level item | FAJAR_LANG_SPEC.md Section 5.1: `const MAX: usize = 1024` | `item` rule in GRAMMAR_REFERENCE.md does not include `const_def` |
| `?` error propagation operator | FAJAR_LANG_SPEC.md Section 11 | Not in grammar, no precedence, not in any expression rule |
| `with no_grad { ... }` block | FAJAR_LANG_SPEC.md Section 8.6 | Not in grammar -- `with` is not a keyword |
| Type-level `Tensor<f32>[784, 128]` syntax | FAJAR_LANG_SPEC.md Section 3.3 | GRAMMAR_REFERENCE.md has `tensor_type` rule but uses `tensor` keyword, while the spec uses `Tensor` as a generic type name |

### 6.2 Type System Edge Cases

| Edge Case | Status |
|-----------|--------|
| What is the default integer type for literals? | EXAMPLES.md Section 1.2: `let inferred = 100 // i64 (default integer)`. But FAJAR_LANG_SPEC.md does not explicitly state this. Is it `i32` (like Rust) or `i64`? |
| Can you have `Option<Option<T>>`? | Not addressed |
| What happens with recursive types? (e.g., linked list) | Not addressed |
| How are generic type constraints resolved? | `T: Comparable` is shown but `Comparable` trait is never defined |
| What is `str` vs `String`? | FAJAR_LANG_SPEC.md lists both `str` (type keyword) and `String` (in std::string). The relationship (slice vs owned) is not defined. |
| Tensor with mixed static/dynamic dims | `Tensor<f32>[*, 784]` -- how does matmul shape checking work when one dim is dynamic? |
| Move semantics for tensors | Are tensors moved or cloned? Tensor operations in examples seem to use them multiple times without explicit clone. |
| Semicolon rules | Grammar says `';'?` for let_stmt and expr_stmt. Are semicolons optional everywhere? What delineates statements in a block? |
| Method syntax `self` vs `&self` vs `&mut self` | FAJAR_LANG_SPEC.md examples mix `self`, `&self`, `&mut self`. Ownership semantics of method receivers are not specified. |

### 6.3 Semantic Rules Not Fully Specified

| Rule | Gap |
|------|-----|
| **Context calling convention** | Can `@safe` code call `@kernel` functions? Can `@safe` code call `@device` functions? The cross-domain bridge example implies yes, but no rule states this. |
| **Context inheritance** | If a `@kernel` function calls a non-annotated function, what context does the callee execute in? |
| **Module resolution** | `use std::io::println` -- how are modules found? File-based? Built-in? No resolution algorithm defined. |
| **Operator overloading** | Is `+` on String valid? Is `@` only for tensors? Can user types implement operators? |
| **Pattern exhaustiveness** | How is exhaustiveness checked for ranges? For nested patterns? |
| **Tensor broadcasting rules** | "Element-wise add (broadcast)" is mentioned but broadcasting rules (NumPy-style?) are not specified. |
| **Scope of `mut`** | Does `let mut x = struct_instance` make all fields mutable? Or only reassignment of `x`? |
| **Type coercion of number literals** | Is `let x: f32 = 3.14` valid? The literal `3.14` defaults to f64 -- is implicit narrowing allowed for literals? |

---

## 7. Error Code Gaps

### 7.1 Error Codes Referenced in Documents but Missing from ERROR_CODES.md

| Code | Referenced In | Description | In ERROR_CODES.md? |
|------|-------------|-------------|---------------------|
| TE003 | FAJAR_LANG_SPEC.md Section 13.4 | Matrix multiply shape mismatch (used instead of TE002) | TE003 exists as `BroadcastError` -- NOT matmul mismatch. Conflict. |

### 7.2 Error Scenarios in Docs Without Assigned Codes

| Scenario | Document | Suggested Code |
|----------|----------|---------------|
| `@safe` calling tensor ops | CLAUDE.md Section 5.3 (shows "ERROR" without code) | Needs a code -- perhaps SE013 "ContextRestrictedOperation"? |
| `@safe` calling raw pointer ops | CLAUDE.md Section 5.3 (shows "ERROR" without code) | Same -- no assigned error code |
| Type alias undefined | Possible error when using `type` keyword | No code assigned |
| Duplicate variant in enum | PE008 covers duplicate struct fields but not enum variants | Need separate code or extend PE008 |
| Import not found (`use foo::bar` where `bar` doesn't exist) | Common scenario | No error code -- possibly SE013? |
| Circular dependency in modules | Common scenario | No error code |
| Generic type constraint not satisfied | `T: Comparable` where T doesn't implement Comparable | No error code |

### 7.3 Error Code Count Discrepancy

- **CLAUDE.md** (Section 7): States `54 error codes across 8 categories`
- **ERROR_CODES.md** title: States `54 error codes across 8 categories`
- **Actual count in ERROR_CODES.md**: LE(8) + PE(10) + SE(12) + KE(4) + DE(3) + TE(8) + RE(8) + ME(8) = **61 total**

The stated count of 54 is incorrect. The actual count is 61.

**Resolution:** Update the error code count in both CLAUDE.md and ERROR_CODES.md to 61.

---

## 8. Critical Priority Items — Resolution Status

All 10 critical items have been resolved on 2026-03-05.

### Priority 1 (Blocking) — ALL RESOLVED

**1. Define context calling convention** ✅ RESOLVED
- Added Section 6.7 "Context Calling Convention" to FAJAR_LANG_SPEC.md
- Added Section 5.4 "Context Calling Convention" to CLAUDE.md
- Added cross-context call rows to TESTING.md test matrix
- **Decision:** `@safe` CAN call `@kernel`/`@device` functions (bridge pattern), but CANNOT directly use their primitives.

**2. Resolve `@safe` tensor ops conflict** ✅ RESOLVED
- STDLIB_SPEC.md corrected: nn:: functions require `@device` or `@unsafe` context for direct use
- CLAUDE.md, TESTING.md already correct (tensor ops = ERROR in @safe)
- `@safe` can call `@device` functions that internally use tensor ops
- **Decision:** Tensor primitives restricted to `@device`/`@unsafe` only.

**3. Add missing grammar rules** ✅ RESOLVED
- GRAMMAR_REFERENCE.md v0.2: added closure_expr, `as` cast (level 15), `?` try (level 17), bitwise operators (levels 5-7, 11), named arguments (call_arg rule), `const_def` in item, `no_grad` block, generic trait bounds, ref types
- Precedence expanded from 11 to 19 levels

### Priority 2 (Important) — ALL RESOLVED

**4. Synchronize Phase 0** ✅ RESOLVED
- PLANNING.md: Phase 0 added as first phase
- TASKS.md: Phase 0 section added with T0.1
- Current status synchronized across all docs: "Phase 0 — Scaffolding"

**5. Clarify Interpreter input type** ✅ RESOLVED
- CLAUDE.md Section 4.2: noted Phase 1 uses `&Program`, Phase 2+ uses `&TypedProgram`
- ARCHITECTURE.md: interpreter signature updated to `&Program` with phase note
- API_REFERENCE.md: updated with phase note

**6. Remove `logos` dependency** ✅ RESOLVED
- Removed from CLAUDE.md Cargo.toml section
- Removed from ARCHITECTURE.md Cargo.toml section
- IMPLEMENTATION_PLAN.md already didn't include it

### Priority 3 (Important) — ALL RESOLVED

**7. Add bitwise operators to grammar** ✅ RESOLVED
- Added as levels 5 (|), 6 (^), 7 (&), 11 (<<, >>) in GRAMMAR_REFERENCE.md
- Added `~` as unary bitwise NOT

**8. Add `range` to precedence table** ✅ RESOLVED
- Added as level 10 (non-associative) in both GRAMMAR_REFERENCE.md and CLAUDE.md

**9. Define semicolon rules** ✅ RESOLVED
- Added Section 4.1 "Semicolon Rules" to GRAMMAR_REFERENCE.md
- Added Section 5.0 "Semicolon Rules" to FAJAR_LANG_SPEC.md

### Priority 4 (Should Fix) — ALL RESOLVED

**10. Fix error code count and TE002/TE003** ✅ RESOLVED
- FAJAR_LANG_SPEC.md: corrected TE003→TE002 for matmul shape mismatch
- ERROR_CODES.md: count updated from 54 to 61
- CLAUDE.md: error count updated throughout

---

## Appendix A: Document Cross-Reference Matrix

| Concern | Primary Authority | Secondary | Conflicts With |
|---------|-------------------|-----------|----------------|
| Language syntax & semantics | FAJAR_LANG_SPEC.md | GRAMMAR_REFERENCE.md | EXAMPLES.md (uses features not in grammar) |
| Formal grammar | GRAMMAR_REFERENCE.md | FAJAR_LANG_SPEC.md Section 15 | Missing rules (see 6.1) |
| Rust API contracts | ARCHITECTURE.md | API_REFERENCE.md | Value enum, Interpreter methods |
| Implementation order | IMPLEMENTATION_PLAN.md | TASKS.md | Phase 0 existence, task granularity |
| Current project status | PLANNING.md | CLAUDE.md | IMPLEMENTATION_PLAN.md (Phase 0 vs Phase 1) |
| Error codes | ERROR_CODES.md | CLAUDE.md Section 7 | FAJAR_LANG_SPEC.md (TE003 vs TE002) |
| Testing strategy | TESTING.md | RULES.md | assert_tokens! macro (EOF filter) |
| Context annotation rules | CLAUDE.md Section 5.3 | SECURITY.md | STDLIB_SPEC.md, FAJAR_LANG_SPEC.md Section 6.7 |
| Coding conventions | RULES.md | CLAUDE.md Section 6 | Generally consistent |
| Git workflow | CONTRIBUTING.md | CLAUDE.md Section 10 | Generally consistent |

## Appendix B: Summary Statistics

| Metric | Count |
|--------|-------|
| **Total conflicts found** | 11 |
| **Spec features missing from implementation** | 13 |
| **Grammar rules missing** | 10 |
| **Type system ambiguities** | 9 |
| **Semantic rules unspecified** | 8 |
| **Error code issues** | 7+ |
| **Test coverage gaps** | 5 categories |
| **Critical items (must-fix)** | 10 |

---

## 9. Remaining Open Items (Non-Blocking)

The following items from the gap analysis are acknowledged but **deferred** — they do not block Phase 0 or Phase 1 development:

| Item | Category | Deferred To |
|------|----------|-------------|
| `async fn` / `await` | Missing from tasks | Phase 4 |
| `@ffi("C") extern` block | Missing from tasks | Phase 5 |
| Capability-based unsafe `@unsafe(capability: [...])` | Future enhancement | Phase 7+ |
| `@fuzz` annotation | Future enhancement | Phase 7+ |
| `fj audit` command | Missing from CLI tasks | Phase 5 |
| Compliance targets (MISRA, DO-178C, etc.) | Future | Phase 7 |
| `FjPipeline` struct vs free functions | Architecture decision | Phase 1 Sprint 1.6 |
| `proptest` dev-dependency | Not in Cargo.toml | Add when needed in Phase 2 |
| CI/CD pipeline setup (.github/workflows/) | Missing task | After git repo is set up |
| `str` vs `String` relationship | Type system design | Phase 2 |
| Tensor broadcasting rules | Unspecified | Phase 4 |
| `self` / `&self` method receiver semantics | Unspecified | Phase 2 |

---

*Gap Analysis Version: 1.1 | Updated: 2026-03-05 | All 10 critical items RESOLVED*
*Analyst: Claude Opus 4.6 via Claude Code*
