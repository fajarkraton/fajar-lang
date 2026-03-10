# Tasks — Fajar Lang

> Update setiap task segera setelah selesai. Format: [x] = done, [-] = in progress, [ ] = todo.
> Setiap Claude Code session dimulai dari task pertama yang belum done.

## Legend

```
[ ] → Not started
[-] → In progress (current session)
[x] → Complete
[!] → Blocked (reason in notes)
[~] → Deferred to later phase
```

---

## Phase 0 — Project Scaffolding

> See `docs/IMPLEMENTATION_PLAN.md` Steps 0.1-0.5 for detailed scaffolding instructions.

#### T0.1 — Scaffold Project ✅
- [x] Create Cargo.toml with all Phase 1 dependencies (NO logos — hand-written lexer)
- [x] Create src/lib.rs with module declarations + FjError enum
- [x] Create src/main.rs with CLI skeleton
- [x] Create directory structure (all src/ subdirs)
- [x] Create placeholder .rs files for all modules (28 files)
- [x] Create examples/ with hello.fj, fibonacci.fj
- [x] Verify: `cargo build` succeeds
- [x] Verify: `cargo test` runs (0 tests, 0 failures)
- [x] Verify: `cargo clippy -- -D warnings` clean
- Notes: Completed 2026-03-05

---

## Phase 1 — Core Language Foundation

### Sprint 1.1 — Lexer

#### T1.1.1 — Test Helper Macros ✅
- [x] Create `assert_tokens!` macro (filters EOF)
- [~] Create `assert_eval!` macro — deferred to Sprint 1.5 (needs interpreter)
- [~] Create `assert_compile_error!` macro — deferred to Sprint 1.5
- Notes: `assert_tokens!` defined in `src/lexer/mod.rs` tests. Other macros need interpreter. Completed 2026-03-05.

#### T1.1.2 — Token Types ✅
- [x] Create `src/lexer/token.rs`
- [x] Define `Span { start: usize, end: usize }`
- [x] Define `Token { kind, span, line, col }`
- [x] Define `TokenKind` enum:
  - [x] Keywords: if, else, match, while, for, in, return, break, continue
  - [x] Keywords: let, mut, fn, struct, enum, impl, trait, type, const
  - [x] Keywords: use, mod, pub, extern, as
  - [x] Keywords: true, false, null
  - [x] Types: bool, i8-i128, u8-u128, isize, usize, f32, f64, str, char, void, never
  - [x] ML keywords: tensor, grad, loss, layer, model
  - [x] OS keywords: ptr, addr, page, region, irq, syscall
  - [x] Annotations: @kernel, @device, @safe, @unsafe, @ffi
  - [x] Operators: all arithmetic, comparison, logical, bitwise, assignment
  - [x] Delimiters: (, ), {, }, [, ], ;, :, ,, ., ->, =>, ::, |>
  - [x] Literals: IntLit, FloatLit, StringLit, RawStringLit, CharLit
  - [x] Ident(String)
  - [x] Eof
- [x] Completion: `cargo test lexer` → ALL PASSING (15 token tests)
- Notes: KEYWORDS + ANNOTATIONS static lookup tables via LazyLock. Display impl for all variants. Completed 2026-03-05.

#### T1.1.3 — Lexer Implementation ✅
- [x] Create `src/lexer/cursor.rs` with Cursor struct
- [x] Implement: peek, peek_second, advance, eat, eat_while, is_eof, slice_from
- [x] Implement: skip_whitespace, skip_comments (single-line, multi-line with nesting, doc)
- [x] Implement: scan_identifier_or_keyword (with raw string r"..." handling)
- [x] Implement: scan_number (decimal, hex, binary, octal, float, scientific, underscore separators)
- [x] Implement: scan_string (regular with escape processing + raw strings)
- [x] Implement: scan_char (with escape sequences)
- [x] Implement: scan_operator (all operators + multi-char operators)
- [x] Implement: scan_annotation (@kernel, @device, @safe, @unsafe, @ffi)
- [x] Create `src/lexer/mod.rs` with `pub fn tokenize()` + LexError enum (7 error variants: LE001-LE007)
- [x] Implement: error collection (Vec<LexError>)
- [x] Comprehensive test suite: 82 tests total (cursor: 11, token: 15, tokenize: 56)
- Notes: All tests pass, clippy clean, fmt clean. Completed 2026-03-05.

### Sprint 1.2 — AST Definition

#### T1.2.1 — Core AST Nodes ✅
- [x] Create `src/parser/ast.rs`
- [x] Span re-used from `crate::lexer::token::Span`
- [x] Implement: Expr enum (24 variants)
  - [x] Literal { kind: LiteralKind, span }
  - [x] Ident { name, span }
  - [x] Binary { left, op: BinOp, right, span }
  - [x] Unary { op: UnaryOp, operand, span }
  - [x] Call { callee, args: Vec<CallArg>, span }
  - [x] MethodCall { receiver, method, args, span }
  - [x] Field { object, field, span }
  - [x] Index { object, index, span }
  - [x] Block { stmts, expr, span }
  - [x] If { condition, then_branch, else_branch, span }
  - [x] Match { subject, arms: Vec<MatchArm>, span }
  - [x] While { condition, body, span }
  - [x] For { variable, iterable, body, span }
  - [x] Assign { target, op: AssignOp, value, span }
  - [x] Pipe { left, right, span }
  - [x] Array { elements, span }
  - [x] Tuple { elements, span }
  - [x] Range { start, end, inclusive, span }
  - [x] Cast { expr, ty, span }
  - [x] Try { expr, span }
  - [x] Closure { params, return_type, body, span }
  - [x] StructInit { name, fields: Vec<FieldInit>, span }
  - [x] Grouped { expr, span }
  - [x] Path { segments, span }
- [x] Implement: Stmt enum (7 variants: Let, Const, Expr, Return, Break, Continue, Item)
- [x] Implement: Item enum (9 variants: FnDef, StructDef, EnumDef, ImplBlock, TraitDef, ConstDef, UseDecl, ModDecl, Stmt)
- [x] Implement: TypeExpr enum (10 variants: Simple, Generic, Tensor, Pointer, Reference, Tuple, Array, Slice, Fn, Path)
- [x] Implement: Pattern enum (7 variants: Literal, Ident, Wildcard, Tuple, Struct, Enum, Range)
- [x] Implement: Annotation struct, GenericParam, TraitBound, Param, Field, Variant, ClosureParam, FieldInit, FieldPattern, CallArg, MatchArm
- [x] Implement: BinOp (20 variants), UnaryOp (6 variants), AssignOp (11 variants), LiteralKind (7 variants), UseKind (3 variants)
- [x] Display impl: LiteralKind, BinOp, UnaryOp, AssignOp, TypeExpr, Pattern
- [x] Expr::span(), TypeExpr::span(), Pattern::span() helper methods
- [x] 33 tests, all passing. Clippy clean, fmt clean.
- Notes: Completed 2026-03-05. TensorLiteral merged into Array (parsed same way, distinguished later by type checker).

### Sprint 1.3 — Parser ✅
- [x] T1.3.1: Token cursor (peek, peek_at, advance, at, eat, expect, expect_ident, synchronize, eat_semi, prev_span)
- [x] T1.3.2: Pratt expression parser with 19-level binding power table (src/parser/pratt.rs)
- [x] T1.3.3: Parse literal expressions (int, float, string, raw string, char, bool, null)
- [x] T1.3.4: Parse binary expressions (20 operators) + unary expressions (6 operators including &mut)
- [x] T1.3.5: Parse function calls (positional + named args) + method calls + field access + indexing + try (?)
- [x] T1.3.6: Parse let/const statements + assignment (simple + compound, 11 AssignOp variants)
- [x] T1.3.7: Parse if/else/else-if expressions
- [x] T1.3.8: Parse while/for loops
- [x] T1.3.9: Parse match expressions (with guard conditions, 7 pattern types)
- [x] T1.3.10: Parse function definitions (with annotations, generics, params, return type)
- [x] T1.3.11: Parse struct/enum definitions (with generics, fields, tuple variants)
- [x] T1.3.12: Parse impl blocks (inherent + trait impl) + trait definitions
- [x] T1.3.13: Parse use declarations (simple, glob, group) + mod declarations (inline + external)
- [x] T1.3.14: ParseError (6 variants: PE001-PE006) + error recovery (synchronize with guaranteed progress)
- [x] T1.3.15: 82 parser tests in mod.rs (inline), 12 pratt tests — all passing
- [x] Additional: pipeline |>, range ../..=, type cast (as), closures, struct init, path expressions, grouped/tuple, block expressions, array literals
- Notes: Completed 2026-03-05. Total project tests: 209. Clippy clean, fmt clean.

### Sprint 1.4 — Environment & Values ✅
- [x] T1.4.1: Value enum (all runtime value types)
- [x] T1.4.2: Environment struct (scope chain)
- [x] T1.4.3: Variable binding, lookup, assignment
- [x] T1.4.4: Nested scope push/pop
- [x] T1.4.5: Function values (closure capture)
- Notes: Value enum (12 variants), FnValue struct, Environment with Rc<RefCell<>> scope chain. 33 tests (18 value + 15 env). Completed 2026-03-05.

### Sprint 1.5 — Interpreter ✅
- [x] T1.5.1: Interpreter struct + initialization
- [x] T1.5.2: eval_expr dispatch (24 expression variants)
- [x] T1.5.3: Arithmetic + comparison + logical (with short-circuit)
- [x] T1.5.4: Variable operations (let, const, assign, compound assign)
- [x] T1.5.5: Block expressions (scoped)
- [x] T1.5.6: If/else evaluation
- [x] T1.5.7: While/for loops (with break/continue)
- [x] T1.5.8: Function definition + call (with closures)
- [x] T1.5.9: Return/break/continue (via ControlFlow signals)
- [x] T1.5.10: Match evaluation (6 pattern types, guards)
- [x] T1.5.11: Struct/enum instantiation + field access
- [x] T1.5.12: Built-in functions (print, println, len, type_of, push, pop, to_string, to_int, to_float, assert, assert_eq)
- [x] T1.5.13: Integration test: hello.fj (completed in Sprint 1.6)
- [x] T1.5.14: Integration test: fibonacci.fj (completed in Sprint 1.6)
- [x] T1.5.15: Integration test: factorial.fj (completed in Sprint 1.6)
- Notes: 69 eval tests + RuntimeError (8 variants) + ControlFlow signals. Recursion limit 256. Pipeline, range, index, method call, closures with capture. Completed 2026-03-05.

### Sprint 1.6 — CLI & REPL ✅
- [x] T1.6.1: clap CLI (run, repl, check, dump-tokens, dump-ast)
- [x] T1.6.2: REPL with rustyline (history, exit/quit, Ctrl-D)
- [x] T1.6.3: Error display (lex/parse/runtime errors to stderr)
- [x] T1.6.4: Exit codes (0 = success, 1 = error)
- [x] T1.6.5: examples/hello.fj, fibonacci.fj, factorial.fj — all running
- [x] T1.5.13: Integration test: hello.fj ✅ (prints "Hello from Fajar Lang!")
- [x] T1.5.14: Integration test: fibonacci.fj ✅ (prints 0,1,1,2,3,5,8,13,21,34)
- [x] T1.5.15: Integration test: factorial.fj ✅ (prints 1,1,2,6,...,362880)
- Notes: call_main() auto-invokes main() if defined. REPL persists state across lines. Completed 2026-03-05.

### Sprint 1.7 — Phase 1 Gap Fixes ✅

#### T1.7.1 — Error Code Alignment ✅
- [x] Renumbered LexError to match ERROR_CODES.md: LE003=UnterminatedBlockComment, LE006=NumberOverflow, LE007=EmptyCharLiteral, LE008=MultiCharLiteral
- [x] Added LE008 MultiCharLiteral — `'ab'` → error with test
- [x] Added LE006 NumberOverflow — `99999999999999999999` → error with test
- [x] Added PE007 InvalidPattern, PE008 DuplicateField, PE009 TrailingSeparator, PE010 InvalidAnnotation
- [x] PE008 DuplicateField detection wired into parse_struct_init
- Notes: LE codes now match ERROR_CODES.md exactly. PE007-PE010 defined (PE008 actively detected, others available for future use). Completed 2026-03-05.

#### T1.7.2 — Span::merge ✅
- [x] Added `Span::merge(self, other) -> Span` — combines two spans
- Notes: Completed 2026-03-05.

#### T1.7.3 — Exit Codes ✅
- [x] Exit code 0 = success, 1 = runtime error, 2 = compile error, 3 = usage error
- [x] All lex/parse/semantic errors → ExitCode::from(2)
- [x] Runtime errors → ExitCode::from(1)
- [x] File not found → ExitCode::from(3)
- Notes: Completed 2026-03-05.

#### T1.7.4 — Integration Tests ✅
- [x] Created `tests/eval_tests.rs` with 12 E2E tests
- [x] Tests: hello.fj, fibonacci.fj, factorial.fj, expressions, recursion, structs, closures, match, for loop, pipeline, eval_source, call_fn
- Notes: Completed 2026-03-05. tests/lexer_tests.rs and tests/parser_tests.rs deferred (inline tests sufficient for now).

#### T1.7.5 — Interpreter API ✅
- [x] `Interpreter::eval_source(&mut self, source: &str) -> Result<Value, FjError>` — lex+parse+eval convenience
- [x] `Interpreter::call_fn(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError>` — call named function
- Notes: Completed 2026-03-05.

#### T1.7.6 — FjError Typed Variants ✅
- [x] `FjError::Lex(Vec<LexError>)` with From impl
- [x] `FjError::Parse(Vec<ParseError>)` with From impl
- [x] `FjError::Semantic(Vec<SemanticError>)` with From impl
- [x] `FjError::Runtime(RuntimeError)` with From impl
- Notes: Completed 2026-03-05.

#### T1.7.7 — Dependency Cleanup ✅
- [x] Removed `serde`, `serde_json`, `indexmap` (unused)
- [x] Removed `criterion` dev-dependency (benches/ empty)
- [x] Kept `ndarray` (Phase 4 tensor backend)
- [x] Kept `pretty_assertions` (for integration tests)
- Notes: Completed 2026-03-05.

#### T1.7.8 — CHANGELOG Updated ✅
- [x] Updated [Unreleased] section with all Phase 1 + Phase 2 completions
- [x] Detailed per-sprint breakdown with feature lists
- Notes: Completed 2026-03-05.

---

## Phase 2 — Type System

### Sprint 2.1 — Type Representation & Errors ✅
- [x] T2.1.1: Type enum (14 variants: Void, Never, I64, F64, Bool, Char, Str, Array, Tuple, Struct, Enum, Function, Unknown, Named)
- [x] T2.1.2: SemanticError enum (9 variants: SE001-SE008, SE012)
- [x] T2.1.3: TypeEnv via SymbolTable (scoped type lookups)
- Notes: Type::is_compatible() handles Unknown (error recovery) and Never (diverging). Completed 2026-03-05.

### Sprint 2.2 — Symbol Table & Scope ✅
- [x] T2.2.1: Symbol struct (name, ty: Type, mutable, span)
- [x] T2.2.2: SymbolTable with scoped push/pop (Vec<Vec<Symbol>> stack)
- [x] T2.2.3: Function signatures in symbol table (two-pass: register then check)
- [x] T2.2.4: Struct/enum type registration (first pass registers struct fields + enum names)
- Notes: 8 scope tests. Innermost-first lookup for lexical scoping. Completed 2026-03-05.

### Sprint 2.3 — Type Checker (Expressions) ✅
- [x] T2.3.1: TypeChecker struct + analyze() entry point (two-pass)
- [x] T2.3.2: Check literals, identifiers, binary/unary ops
- [x] T2.3.3: Check function calls (arity + argument types, variadic builtins)
- [x] T2.3.4: Check field access, index, method calls
- [x] T2.3.5: Check if/match/block type consistency
- Notes: 11 builtin functions registered. check_expr dispatches all 24 expression types. Completed 2026-03-05.

### Sprint 2.4 — Type Checker (Statements & Items) ✅
- [x] T2.4.1: Check let/const bindings (type annotation vs inferred)
- [x] T2.4.2: Check function definitions (params, return type, body)
- [x] T2.4.3: Check struct/enum definitions
- [x] T2.4.4: Check assignment (mutability + type match)
- [x] T2.4.5: Check for/while loops
- Notes: Completed 2026-03-05.

### Sprint 2.5 — Integration & Pipeline ✅
- [x] T2.5.1: Wire analyzer into CLI `check` and `run` commands
- [x] T2.5.2: Comprehensive test suite (28 analyzer tests + 8 scope tests = 36 total; 350 project-wide)
- [x] T2.5.3: Update PLANNING.md — Phase 2 complete
- Notes: CLI catches SE004 (type mismatch), SE005 (arity), SE007 (immutable assign). All examples pass. Completed 2026-03-05.

### Sprint 2.6 — Distinct Integer/Float Types ✅
- [x] T2.6.1: Expand Type enum with distinct integer/float types (I8-I128, U8-U128, ISize, USize, F32, F64)
- [x] T2.6.2: Update resolve_type() to map each TypeExpr to its own Type variant (i32 ≠ i64)
- [x] T2.6.3: Add IntLiteral/FloatLiteral types for unsuffixed literals (compatible with any int/float)
- [x] T2.6.4: No implicit type promotion — `i32 + i64` → SE004 error, `i32 + 42` → OK (literal inference)
- [x] T2.6.5: Updated check_binary (arithmetic + bitwise), check_unary (BitNot preserves type), conditions (accept any integer)
- [x] T2.6.6: Updated builtins: len() → USize, method .len() → USize, index accepts any integer
- [x] T2.6.7: Range accepts any integer type, not just i64
- [x] T2.6.8: 12 new type checker tests: i32≠i64, f32≠f64, mixed arithmetic error, same-type OK, bitnot on float, bitnot preserves type, bitwise mixed types, all integer types resolve, both float types resolve
- [x] T2.6.9: `cargo test` — 376 tests passing, `cargo clippy -- -D warnings` clean, `cargo fmt` clean
- Notes: Interpreter keeps i64/f64 runtime values; type distinction is compile-time only. Completed 2026-03-05.

### Sprint 2.7 — Missing Semantic Error Codes ✅
- [x] T2.7.1: SE009 UnusedVariable — detect variables declared but never read (warning)
- [x] T2.7.2: SE010 UnreachableCode — detect code after return/break/continue in blocks (warning)
- [x] T2.7.3: SE011 NonExhaustiveMatch — detect match without wildcard/catch-all pattern (error)
- [x] T2.7.4: Warning vs error distinction — `is_warning()` method, warnings don't fail `analyze()`
- [x] T2.7.5: Symbol.used tracking — `mark_used()`, `pop_scope_unused()`, `_` prefix suppresses warnings
- [x] T2.7.6: 14 new tests (4 unused var, 3 unreachable, 4 non-exhaustive match, 3 scope tracking)
- Notes: All 12 SE codes (SE001-SE012) now implemented. 390 tests passing. Completed 2026-03-05.

### Sprint 2.8 — ScopeKind & Context Tracking ✅
- [x] T2.8.1: ScopeKind enum (Module, Function, Block, Loop, Kernel, Device, Unsafe)
- [x] T2.8.2: Refactored SymbolTable scopes from `Vec<Vec<Symbol>>` to `Vec<Scope>` where `Scope { symbols, kind }`
- [x] T2.8.3: `push_scope_kind()` used for Function, Loop, Closure scopes; `is_inside_loop()`, `is_inside_function()` queries
- [x] T2.8.4: Validate break/continue only inside Loop scope → BreakOutsideLoop error
- [x] T2.8.5: Validate return only inside Function scope → ReturnOutsideFunction error
- [x] T2.8.6: 15 new tests (7 break/continue/return validation, 8 scope kind tracking)
- Notes: ScopeKind::Kernel/Device/Unsafe defined but not yet enforced (Phase 3/4). 404 tests passing. Completed 2026-03-05.

### Sprint 2.9 — miette Error Display ✅
- [x] T2.9.1: `FjDiagnostic` struct implementing `miette::Diagnostic` — generic wrapper for all error types
- [x] T2.9.2: `from_lex_error()` — converts LexError with code, span, help text
- [x] T2.9.3: `from_parse_error()` — converts ParseError with code and span
- [x] T2.9.4: `from_semantic_error()` — converts SemanticError with code, span, severity (error/warning)
- [x] T2.9.5: `from_runtime_error()` — converts RuntimeError (no span)
- [x] T2.9.6: CLI `cmd_run`, `cmd_check`, `cmd_dump_tokens`, `cmd_dump_ast` all use `FjDiagnostic::eprint()`
- [x] T2.9.7: `NamedSource` stores source code + filename for span highlighting
- [x] T2.9.8: Help text for lex errors (e.g., "add closing `\"` to terminate the string")
- Notes: miette `fancy` feature enabled. Error output shows source code, highlighted spans, error codes, and help text. 404 tests passing. Completed 2026-03-05.

### Sprint 2.10 — Phase 2 Completion ✅
- [x] T2.10.1: Verify all 12 SE codes (SE001-SE012) are implemented and tested
- [x] T2.10.2: Verify `fj check` shows beautiful miette error output
- [x] T2.10.3: Verify `let x: i32 = 42; let y: i64 = x` → SE004 compile error
- [x] T2.10.4: Update PLANNING.md — Phase 2 truly complete
- [x] T2.10.5: `cargo test` all passing, `cargo clippy -- -D warnings` clean
- Notes: Phase 2 exit gate. All gaps from gap analysis resolved.

### Deferred to Later Phases
- [~] Type Inference (Hindley-Milner Lite) — constraint generation + unification → Phase 5 (not needed for tree-walking interpreter)
- [~] Generic Type Parameters — monomorphization at call sites → Phase 5
- [~] Tensor Type & Shape Checking — TE001-TE008 → Phase 4
- [~] Context Annotation Enforcement — @kernel KE001-KE004, @device DE001-DE003 → Phase 3/4
- [~] borrow_lite.rs — ownership/move semantics ME001-ME008 → Phase 3
- [~] TypedProgram output — analyzer returns typed AST → Phase 5 (compiler backend needs it)

## Phase 3 — OS Runtime

### Sprint 3.1 — Memory Manager ✅
- [x] T3.1.1: MemoryManager struct with Vec<u8> backing store
- [x] T3.1.2: VirtAddr, PhysAddr newtype structs (distinct types, not aliases)
- [x] T3.1.3: MemoryRegion struct (start, size, allocated flag)
- [x] T3.1.4: alloc(size, align) → Result<VirtAddr, MemoryError>
- [x] T3.1.5: free(addr) → Result<(), MemoryError>
- [x] T3.1.6: read/write methods (read_u8, read_u32, write_u8, write_u32, etc.)
- [x] T3.1.7: Bounds checking + overlap detection
- [x] T3.1.8: Unit tests (alloc, free, double-free, out-of-bounds, overlap)

### Sprint 3.2 — Page Tables & Virtual Memory ✅
- [x] T3.2.1: PageFlags bitflags (READ, WRITE, EXEC, USER)
- [x] T3.2.2: PageTable struct (HashMap<VirtAddr, (PhysAddr, PageFlags)>)
- [x] T3.2.3: map_page(va, pa, flags) → Result
- [x] T3.2.4: unmap_page(va) → Result
- [x] T3.2.5: translate(va) → Result<(PhysAddr, PageFlags)>
- [x] T3.2.6: Protection violation checks (already-mapped, page-fault)
- [x] T3.2.7: Unit tests (map, unmap, translate, protection violations)

### Sprint 3.3 — IRQ & Interrupt Handling ✅
- [x] T3.3.1: IrqTable struct with handler slots
- [x] T3.3.2: irq_register(num, handler_name) → Result
- [x] T3.3.3: irq_unregister(num) → Result
- [x] T3.3.4: irq_enable() / irq_disable() global flag
- [x] T3.3.5: dispatch(irq_num) → trigger registered handler
- [x] T3.3.6: Standard IRQ numbers (TIMER=0x20, KEYBOARD=0x21, etc.)
- [x] T3.3.7: Unit tests (13 tests)

### Sprint 3.4 — System Calls ✅
- [x] T3.4.1: SyscallTable struct
- [x] T3.4.2: syscall_define(num, handler_name) → Result
- [x] T3.4.3: syscall_dispatch(num, args) → Result<SyscallHandler>
- [x] T3.4.4: Standard syscall numbers (READ=0, WRITE=1, EXIT=60)
- [x] T3.4.5: Unit tests (10 tests)

### Sprint 3.5 — Port I/O Simulation ✅
- [x] T3.5.1: PortIO struct with port registry
- [x] T3.5.2: port_write(port, value) / port_read(port) → value
- [x] T3.5.3: Simulated devices (serial port COM1, keyboard status)
- [x] T3.5.4: Unit tests (4 tests)

### Sprint 3.6 — Wire into Interpreter ✅
- [x] T3.6.1: Add Pointer(u64) variant to Value enum
- [x] T3.6.2: Add OsRuntime field to Interpreter
- [x] T3.6.3: Register OS builtins (16 functions: mem_*, page_*, irq_*, port_*)
- [x] T3.6.4: Implement call_builtin dispatch for OS functions
- [x] T3.6.5: Integration tests (10 tests in tests/os_tests.rs)

### Sprint 3.7 — @kernel Context Enforcement ✅
- [x] T3.7.1: Add KE001-KE003, DE001-DE002 to SemanticError
- [x] T3.7.2: TypeChecker: detect @kernel/@device annotation, push ScopeKind::Kernel/Device
- [x] T3.7.3: Track kernel_fns, device_fns, os_builtins sets
- [x] T3.7.4: Enforce: @device cannot call OS builtins (DE001)
- [x] T3.7.5: Enforce: @device cannot call @kernel (DE002), @kernel cannot call @device (KE003)
- [x] T3.7.6: Unit tests (7 tests: context violations + safe bridge)

### Sprint 3.8 — OS Stdlib & Integration Tests ✅
- [x] T3.8.1: examples/memory_map.fj — OS memory mapping demo (runs correctly)
- [x] T3.8.2: tests/os_tests.rs — integration tests (10 tests)
- [x] T3.8.3: miette error display for context violations (KE/DE codes)
- [x] T3.8.4: Test: alloc + free cycle from .fj code
- [x] T3.8.5: Test: context violation → compile error (DE001 via `fj check`)

### Sprint 3.9 — Phase 3 Exit Gate ✅
- [x] T3.9.1: Verify examples/memory_map.fj runs correctly
- [x] T3.9.2: Verify @kernel functions can use OS primitives
- [x] T3.9.3: Verify @device functions CANNOT use OS primitives (DE001)
- [x] T3.9.4: cargo test (484 total), clippy clean, fmt clean
- [x] T3.9.5: Update PLANNING.md, CHANGELOG.md

### Sprint 3.10 — Phase 3 Gap Fixes
> Gaps found via post-completion documentation audit.

#### T3.10.1 — KE001/KE002 Enforcement (HIGH) ✅
- [x] Enforce KE001 (HeapAllocInKernel): detect heap-allocating builtins (push, pop, to_string) called inside @kernel scope → emit HeapAllocInKernel error
- [x] Enforce KE002 (TensorInKernel): detect tensor/ML builtins called inside @kernel scope → emit TensorInKernel error (placeholder until Phase 4 ML builtins exist)
- [x] Add tests: 6 new tests (push→KE001, to_string→KE001, pop→KE001, safe fn OK, non-heap builtins OK, KE001+KE003 combo)
- [x] Verify existing KE003/DE001/DE002 still pass
- Notes: Added heap_builtins + tensor_builtins HashSets to TypeChecker. 490 tests passing. Completed 2026-03-05.

#### T3.10.2 — Syscall Builtins in Interpreter (MEDIUM) ✅
- [x] Register `syscall_define` and `syscall_dispatch` as interpreter builtins
- [x] Implement `builtin_syscall_define(num, handler_name, arg_count)` — wraps SyscallTable::define()
- [x] Implement `builtin_syscall_dispatch(num, ...args)` — wraps SyscallTable::dispatch(), returns handler name
- [x] Register builtin type signatures in TypeChecker + added to os_builtins set
- [x] Add 3 integration tests in tests/os_tests.rs (define+dispatch, wrong arg count, no handler)
- Notes: 493 tests passing. Completed 2026-03-05.

#### T3.10.3 — OS Stdlib Files (MEDIUM) ✅
- [x] Create `stdlib/os.fj` — Fajar Lang OS standard library (wrapper functions, constants, doc comments)
- [x] Create `src/stdlib/os.rs` — OS_BUILTINS const listing all 18 OS builtin names
- [x] Create `src/stdlib/mod.rs` — stdlib module with os submodule
- [x] Wire `pub mod stdlib` into `src/lib.rs`
- Notes: stdlib/os.fj includes memory, page table, IRQ, syscall, port I/O wrappers + standard constants. Completed 2026-03-05.

#### T3.10.4 — Missing Integration Tests (LOW) ✅
- [x] Test: kernel init sequence (alloc → write → read → page map → unmap → free)
- [x] Test: IRQ register + enable + disable + unregister lifecycle from .fj code
- [x] Test: syscall define + dispatch from .fj code (two syscalls, verify handler names)
- Notes: 3 new integration tests in tests/os_tests.rs. 16 OS tests total. Completed 2026-03-05.

#### T3.10.5 — Phase 3 Final Exit Gate ✅
- [x] cargo test — 496 tests, all passing
- [x] cargo clippy -- -D warnings — clean
- [x] cargo fmt -- --check — clean
- [x] All KE/DE error codes enforced and tested (KE001, KE002 placeholder, KE003, DE001, DE002)
- [x] Update PLANNING.md, CHANGELOG.md with gap fix entries
- [x] Phase 3 status: ✅ COMPLETE
- Notes: Completed 2026-03-05.

## Phase 4 — ML/AI Runtime

### Sprint 4.1 — TensorValue Struct

#### T4.1.1 — TensorValue Core ✅
- [x] Define `TensorValue` struct: `data: ArrayD<f64>`, `requires_grad: bool`, `grad: Option<ArrayD<f64>>`
- [x] Tensor creation: `zeros`, `ones`, `from_data`, `randn`, `eye`, `full`
- [x] Shape query: `shape()`, `ndim()`, `numel()`, `to_scalar()`, `to_vec()`
- [x] Gradient ops: `set_grad`, `accumulate_grad`, `zero_grad`, `grad()`
- [x] TensorError enum (TE001-TE008) defined
- [x] Display/Debug/PartialEq impl for TensorValue
- [x] 19 unit tests, all passing
- Notes: Added ndarray-rand + rand deps. Completed 2026-03-05.

#### T4.1.2 — Wire Tensor into Value Enum ✅
- [x] Add `Tensor(TensorValue)` variant to `Value` enum
- [x] PartialEq, Display, type_name for Tensor variant
- [x] 9 tensor creation builtins: tensor_zeros, tensor_ones, tensor_randn, tensor_eye, tensor_full, tensor_from_data, tensor_shape, tensor_reshape, tensor_numel
- [x] Registered in interpreter (register_builtins + call_builtin dispatch)
- [x] Registered type signatures in TypeChecker (ml_fns)
- [x] Populated tensor_builtins set (9 entries) for KE002 enforcement
- [x] Box<ControlFlow> in EvalError to fix clippy large_enum_variant
- [x] MAX_RECURSION_DEPTH reduced to 128 for debug stack safety
- [x] 9 ML integration tests (tensor creation, reshape, KE002 enforcement)
- Notes: 524 tests, clippy clean, fmt clean. Completed 2026-03-05.

### Sprint 4.2 — Basic Tensor Operations ✅

#### T4.2.1 — Element-wise Ops ✅
- [x] Add, Sub, Mul, Div (element-wise, with broadcasting)
- [x] Negation (unary minus)
- [x] Broadcasting rules: compatible shapes expand
- [x] Shape validation: mismatched non-broadcastable shapes → TensorError
- [x] Unit tests: element-wise ops, broadcasting, shape errors
- Notes: `check_broadcast()` validates NumPy-style broadcasting before ops. 19 unit tests in ops.rs. Completed 2026-03-05.

#### T4.2.2 — Matrix Operations ✅
- [x] matmul (@ operator): 2D matrix multiply via ndarray `.dot()`
- [x] transpose: swap last two dimensions
- [x] reshape: change shape preserving element count
- [x] flatten: collapse to 1D
- [x] Register as interpreter builtins (matmul, transpose, reshape, flatten)
- [x] Unit tests: matmul shape validation (TE002), transpose, reshape
- Notes: 18 tensor builtins wired into interpreter and type checker. sum/mean reductions added. 543 total tests passing. Completed 2026-03-05.

### Sprint 4.3 — Activation Functions ✅

#### T4.3.1 — Activations in ops.rs ✅
- [x] relu(x): max(0, x) element-wise
- [x] sigmoid(x): 1/(1+exp(-x))
- [x] tanh(x): element-wise tanh
- [x] softmax(x): exp(x)/sum(exp(x)) with log-sum-exp trick for stability
- [x] gelu(x): x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715*x^3)))
- [x] leaky_relu(x, alpha=0.01): max(alpha*x, x)
- [x] Register as interpreter builtins (6 new builtins)
- [x] Unit tests: each activation, numerical stability, known values (18 new tests)
- Notes: 6 activations in ops.rs, wired into interpreter + type checker. 561 total tests passing. Completed 2026-03-05.

### Sprint 4.4 — Computation Graph (Autograd) ✅

#### T4.4.1 — Tape-Based Autograd ✅
- [x] `GradFn` type: `Box<dyn Fn(&ArrayD<f64>) -> Vec<ArrayD<f64>>>`
- [x] `TapeEntry` struct: output_id, input_ids, grad_fn
- [x] `Tape` struct: records operations, fresh_id(), backward(), clear()
- [x] `TensorId` on TensorValue: unique identifier for autograd tracking
- [x] `requires_grad` propagation: result requires_grad if any input does
- [x] `no_grad` context: set_recording(false) disables recording
- [x] Unit tests: 11 autograd tests (tape recording, backward, chain rule, accumulation)
- Notes: Tape-based reverse-mode autograd in autograd.rs. TensorId added to TensorValue. Completed 2026-03-05.

### Sprint 4.5 — Backward Pass ✅

#### T4.5.1 — Backward for All Ops ✅
- [x] backward for add/sub/mul/div (tracked variants with grad_fn)
- [x] backward for matmul (grad_a = g @ B^T, grad_b = A^T @ g)
- [x] backward for relu/sigmoid/tanh (with numerical verification)
- [x] backward for sum/mean (tracked variants)
- [x] `reduce_broadcast()` — reduce gradients along broadcast dimensions
- [x] Numerical gradient check utility (`numerical_gradient()`, epsilon=1e-5)
- [x] Unit tests: 14 gradient tests (analytical vs numerical for all ops, chain rule)
- Notes: All tracked ops in ops.rs. Gradient correctness verified against numerical gradients. 586 total tests. Completed 2026-03-05.

### Sprint 4.6 — Loss Functions ✅

#### T4.6.1 — Loss Functions in ops.rs ✅
- [x] mse_loss(pred, target): mean squared error
- [x] cross_entropy(pred, target): -sum(target * log(pred)) with log-sum-exp stability
- [x] bce_loss(pred, target): binary cross-entropy with clamping
- [x] All with autograd support (tracked variants with backward)
- [x] Register as interpreter builtins (3 new builtins)
- [x] Unit tests: 9 new tests (loss values, shape mismatch, gradient vs numerical)
- Notes: All losses return scalar tensors. Tracked variants for autograd. 595 total tests. Completed 2026-03-05.

### Sprint 4.7 — Optimizers ✅

#### T4.7.1 — SGD and Adam ✅
- [x] `SGD` struct: lr, momentum, velocity buffers
- [x] `Adam` struct: lr, beta1=0.9, beta2=0.999, epsilon=1e-8, m/v moments, timestep
- [x] `optimizer.step(params)` — update parameters using gradients
- [x] `zero_grad(params)` — reset all parameter gradients
- [x] Unit tests: 8 tests (SGD basic/momentum/no-grad, Adam basic/multi-step/custom, zero_grad)
- Notes: In optim.rs. Bias-corrected Adam with proper moment estimation. 603 total tests. Completed 2026-03-05.

### Sprint 4.8 — Layer Abstractions ✅

#### T4.8.1 — Dense Layer ✅
- [x] Dense/Linear layer: weight + bias tensors, forward(x) = x @ W + b, Xavier init
- [x] Dropout: random mask during training, pass-through during eval, inverted scaling
- [x] BatchNorm: normalize + scale + shift, per-feature mean/variance
- [x] Unit tests: 14 tests (shape, param count, requires_grad, forward pass, normalization)
- Notes: In layers.rs. Dense with Xavier init. Dropout with inverted scaling. BatchNorm along batch axis. 617 total tests. Completed 2026-03-05.

### Sprint 4.9 — ML Stdlib & Wire into Interpreter ✅

#### T4.9.1 — ML Builtins & Stdlib ✅
- [x] Wire all ML builtins into interpreter call_builtin dispatch (27 builtins)
- [x] Wire all ML type signatures into TypeChecker (27 entries)
- [x] Populate tensor_builtins set (27 entries, KE002 enforcement)
- [x] @device annotation dispatch: tensor ops only in @device/@unsafe
- [x] Create stdlib/nn.fj — Fajar Lang ML standard library (creation, ops, activations, losses, reductions)
- [x] Create src/stdlib/nn.rs — ML_BUILTINS const listing all builtin names
- [x] Unit tests: @device can use tensor ops, @kernel cannot (existing KE002 tests)
- Notes: Complete ML builtin wiring across interpreter, type checker, and KE002. 617 total tests. Completed 2026-03-05.

### Sprint 4.10 — ML Integration Tests ✅

#### T4.10.1 — Integration Tests ✅
- [x] tests/ml_tests.rs: 23 comprehensive ML integration tests
- [x] Test: MNIST forward pass (784→128→10, relu + softmax, output sums to 1)
- [x] Test: XOR gradient flow (relu backward, gradient propagation)
- [x] Test: Gradient correctness (numerical vs analytical for mul chain)
- [x] Test: All tensor builtins through interpreter (creation, arithmetic, activations, losses, reductions)
- [x] Test: KE002 enforcement for activations and loss functions
- Notes: 631 total tests (577 unit + 12 eval + 23 ml + 16 os + 3 doc), all passing, clippy clean, fmt clean. Completed 2026-03-05.
- [x] Test: examples/mnist_forward.fj runs correctly
- [x] Phase 4 exit gate: all tests pass, clippy clean, fmt clean

### Sprint 4.11 — Gap Fixes ✅

#### T4.11.1 — Missing builtins from STDLIB_SPEC ✅
- [x] ops.rs: squeeze, unsqueeze, max, min, argmax, arange, linspace, xavier, l1_loss
- [x] eval.rs: 11 new builtin registrations + dispatch + handler methods
- [x] type_check.rs: tensor_builtins set + ml_fns signatures for all new builtins
- [x] Unit tests: 21 new tests for gap-fix ops (81 total ops unit tests)
- [x] Integration tests: 8 new ML integration tests (31 total ml tests)
- [x] stdlib/nn.fj: wrapper functions for all new builtins
- [x] src/stdlib/nn.rs: ML_BUILTINS updated with 11 new entries (38 total)
- [x] examples/mnist_forward.fj: working MNIST forward pass example
- [x] 660 total tests (598 unit + 12 eval + 31 ml + 16 os + 3 doc), all passing
- [x] clippy clean, fmt clean
- [~] Conv2d, Attention, LayerNorm → Phase 5
- [~] Data utilities (load_csv, etc.) → Phase 6

---

## Pre-Phase 5 — Gap Fixes

> Features documented in FAJAR_LANG_SPEC but not yet implemented.
> See `docs/PHASE5_PLAN.md` for full details.

### Sprint G.1 — impl Blocks & Method Dispatch ✅

#### T.G.1.1 — Interpreter: Register impl methods ✅
- [x] `Item::ImplBlock`: register methods in impl_methods HashMap
- [x] Key: `(TypeName, method_name)` → `FnValue`
- [x] Self parameter: inject receiver as first argument
- [x] Static methods: register as `TypeName::method` in global env for Path access

#### T.G.1.2 — Method dispatch via impl lookup ✅
- [x] `eval_method_call()`: check impl_methods before hardcoded match
- [x] Look up `(receiver struct name, method)` in registry
- [x] Call with receiver as first arg (self)
- [x] Enum variant method dispatch supported

#### T.G.1.3 — Type checker updates ✅
- [x] Register impl methods in SymbolTable during first pass (qualified names)
- [x] Check impl method bodies during second pass
- [x] `self` param resolves to target struct type

#### T.G.1.4 — Parser: bare `self` parameter ✅
- [x] `parse_params()` handles bare `self` (no `: Type` annotation)
- [x] Assigns `TypeExpr::Simple { name: "Self" }` as placeholder type

#### T.G.1.5 — Tests (12 tests) ✅
- [x] Struct + impl + basic method call (magnitude_sq)
- [x] Multiple methods in one impl block (area + perimeter)
- [x] Method with additional arguments (add)
- [x] Static method via Path (Point::origin)
- [x] Static method with args (Point::new)
- [x] Method not found → RuntimeError
- [x] Method returns struct (scale)
- [x] Method with string output (greet)
- [x] Two different structs with same method name (Dog/Cat speak)
- [x] Self field access (diameter)
- [x] Integration: impl block with static + instance methods
- [x] Integration: multiple structs with area methods
- Notes: 672 tests (608 unit + 14 eval + 31 ml + 16 os + 3 doc). Completed 2026-03-05.

### Sprint G.2 — Option/Result Types & ? Operator ✅

#### T.G.2.1 — Register built-in Option/Result ✅
- [x] Some(v), None, Ok(v), Err(v) as builtin constructors
- [x] Using existing Value::Enum representation
- [x] None registered as unit variant in global env

#### T.G.2.2 — Implement ? operator ✅
- [x] `Expr::Try`: unwrap Ok/Some → value, Err/None → early return
- [x] Return Err/None through ControlFlow::Return signal

#### T.G.2.3 — Utility methods ✅
- [x] .unwrap() on Some/Ok → value, None/Err → RuntimeError
- [x] .unwrap_or(default) on Some/Ok → value, None/Err → default
- [x] .is_some(), .is_none(), .is_ok(), .is_err() → Bool
- [x] Added to eval_method_call() for Enum values

#### T.G.2.4 — Parser: direct variant patterns ✅
- [x] `Some(x)`, `Ok(v)`, `Err(e)` as match patterns (without EnumName::)
- [x] `None` as unit variant pattern (special case in match_pattern)

#### T.G.2.5 — Tests (21 tests) ✅
- [x] Some(42), None, Ok(10), Err("bad") creation (4 tests)
- [x] ? unwraps Ok → value, ? unwraps Some → value (2 tests)
- [x] ? short-circuits Err → early return, ? short-circuits None (2 tests)
- [x] ? propagation chain (1 test)
- [x] .unwrap() on Some/Ok → value (2 tests)
- [x] .unwrap() on None/Err → error (2 tests)
- [x] .unwrap_or() on Some/None/Err (3 tests)
- [x] .is_some()/.is_none() (1 test)
- [x] .is_ok()/.is_err() (1 test)
- [x] Match on Option with Some(x)/None patterns (1 test)
- [x] Integration: divide with ? propagation chain (1 test)
- [x] Integration: Option methods + match (1 test)
- Notes: 693 tests (627 unit + 16 eval + 31 ml + 16 os + 3 doc). Completed 2026-03-05.

### Sprint G.3 — Module System (use/mod) ✅

#### T.G.3.1 — Inline module evaluation ✅
- [x] `Item::ModDecl` with body: eval items, register with qualified names in env + modules map
- [x] Module nesting support (outer::inner::symbol)
- [x] Struct, const, enum definitions inside modules

#### T.G.3.2 — Use statement evaluation ✅
- [x] UseKind::Simple: `use math::square` → alias in current scope
- [x] UseKind::Glob: `use math::*` → import all symbols from module
- [x] UseKind::Group: `use math::{a, b}` → import specified symbols

#### T.G.3.3 — Visibility (pub/private)
- [~] Deferred: all items public by default (like Go). pub/private tracking added later.

#### T.G.3.4 — File-based modules
- [~] Deferred to Phase 6 (stdlib file loading): `mod helper;` loads helper.fj

#### T.G.3.5 — Type checker updates ✅
- [x] register_mod_decl: qualified name registration for functions/consts in modules
- [x] register_use_decl: Simple and Group import resolution in symbol table
- [x] check_item handles Item::ModDecl (recursive check of module body)

#### T.G.3.6 — Tests ✅ (7 integration tests)
- [x] e2e_inline_module_qualified_access — mod math { fn square, fn cube }
- [x] e2e_use_simple_import — use math::double
- [x] e2e_use_glob_import — use utils::*
- [x] e2e_use_group_import — use ops::{inc, dec}
- [x] e2e_module_with_struct — struct inside module + glob import
- [x] e2e_module_const — const inside module with qualified access
- [x] e2e_nested_modules — outer::inner::secret()

### Sprint G.4 — Cast Expression & Minor Gaps ✅

#### T.G.4.1 — Cast expression (as) ✅
- [x] `42 as f64` → Float, `3.14 as i64` → Int (truncate)
- [x] Int widening/narrowing (i8/i16/i32/i64/u8/u16/u32/u64)
- [x] Float widening/narrowing (f32/f64)
- [x] Bool → Int (`true as i64` → 1), Int → Bool
- [x] Invalid cast → RuntimeError::TypeError

#### T.G.4.2 — @device parameter parsing
- [~] Deferred: annotation args require parser changes; low priority

#### T.G.4.3 — Named arguments ✅
- [x] Reorder call args by name to match parameter positions
- [x] Mixed positional + named args supported
- [x] Unknown parameter name → RuntimeError::TypeError

#### T.G.4.4 — Tests ✅ (7 integration tests)
- [x] e2e_cast_int_to_float — `42 as f64`
- [x] e2e_cast_float_to_int — `3.7 as i64` → 3
- [x] e2e_cast_int_widening — `x as i64`
- [x] e2e_cast_float_narrowing — `x as f32`
- [x] e2e_cast_bool_to_int — `true as i64` → 1
- [x] e2e_cast_in_expression — `(10 as f64) / 3.0`
- [x] e2e_named_arguments — `greet(times: 2, name: "hello")`

### Sprint G.5 — Missing Global Builtins & Math Functions ✅

#### T.G.5.1 — Error/Debug builtins ✅
- [x] panic(msg) — terminates with RuntimeError
- [x] todo(msg?) — terminates with "not yet implemented"
- [x] dbg(value) — prints "[dbg] value" and returns value
- [x] eprint(args...) — print to stderr (captured in test mode)
- [x] eprintln(args...) — println to stderr (captured in test mode)

#### T.G.5.2 — Basic I/O
- [~] read_line() — deferred (requires stdin handling, interactive only)

#### T.G.5.3 — Math functions ✅
- [x] abs (int + float), sqrt, pow, log, log2, log10
- [x] sin, cos, tan
- [x] floor, ceil, round, clamp (3 args)
- [x] min, max (int + float)
- [x] PI, E constants (registered as global Float values)
- [x] Helper methods: math_f64_unary, math_f64_binary (auto-coerce int→f64)

#### T.G.5.4 — Type checker signatures
- [~] Deferred: builtins work at runtime; type signatures added when needed

#### T.G.5.5 — Tests ✅ (12 integration tests)
- [x] e2e_panic_terminates
- [x] e2e_dbg_prints_and_returns
- [x] e2e_eprint_and_eprintln
- [x] e2e_math_abs, e2e_math_sqrt, e2e_math_pow
- [x] e2e_math_floor_ceil_round, e2e_math_clamp
- [x] e2e_math_log, e2e_math_trig
- [x] e2e_math_constants (PI, E)
- [x] e2e_math_min_max

### Sprint G.6 — NN Runtime Builtin Exposure ✅

#### T.G.6.1 — Autograd builtins ✅
- [x] tensor_backward(tensor) — runs backward pass via tape or seed gradient
- [x] tensor_grad(tensor) — retrieves gradient from last backward
- [x] tensor_requires_grad(tensor) → bool
- [x] tensor_set_requires_grad(tensor, bool) → tensor with grad tracking

#### T.G.6.2 — Optimizer builtins ✅
- [x] optimizer_sgd(lr, momentum) → Optimizer value
- [x] optimizer_adam(lr) → Optimizer value
- [x] optimizer_step(optimizer, tensor) → updated tensor
- [x] optimizer_zero_grad(tensor) → tensor with cleared gradient

#### T.G.6.3 — Layer builtins ✅
- [x] layer_dense(in_features, out_features) → Layer value
- [x] layer_forward(layer, input) → output tensor
- [x] layer_params(layer) → array of parameter tensors
- [~] layer_dropout, layer_batchnorm — deferred (less commonly used from .fj)

#### T.G.6.4 — Value variants ✅
- [x] Value::Optimizer(OptimizerValue) — SGD | Adam enum
- [x] Value::Layer(Box<LayerValue>) — Dense (boxed to avoid large enum variant)
- [x] type_name: "optimizer", "layer"
- [x] Display: "<optimizer SGD>", "<layer Dense>"

#### T.G.6.5 — Tests ✅ (8 integration tests)
- [x] ml_tensor_requires_grad — set + check requires_grad
- [x] ml_tensor_backward_and_grad — backward + grad retrieval
- [x] ml_optimizer_sgd_create — type_of returns "optimizer"
- [x] ml_optimizer_adam_create — type_of returns "optimizer"
- [x] ml_optimizer_step_and_zero_grad — step + zero_grad round-trip
- [x] ml_layer_dense_create — type_of returns "layer"
- [x] ml_layer_forward — shape [1,3] → [1,2] via Dense
- [x] ml_layer_params — Dense returns 2 params (weight + bias)

### Sprint G.7 — Parser & Analyzer Cleanup ✅

#### T.G.7.1 — loop expression ✅
- [x] Added `loop` keyword to lexer (TokenKind::Loop)
- [x] Added `Expr::Loop { body, span }` to AST
- [x] Parser: `parse_loop_expr()` — `loop { body }`
- [x] Interpreter: `eval_loop()` — infinite loop with break/continue support
- [x] Type checker: Loop scope with break/continue validation

#### T.G.7.2 — Dead code cleanup ✅
- [x] No dead code warnings (cargo clippy clean, cargo build clean)
- [x] No unused imports or unreachable code detected

#### T.G.7.3 — stdlib/core.fj ✅
- [x] Created stdlib/core.fj with utility functions (clamp_int, clamp_float, not, is_even, is_odd, sign, range_sum)

#### T.G.7.4 — Tests ✅ (3 integration tests)
- [x] e2e_loop_with_break — loop + conditional break
- [x] e2e_loop_with_continue — loop + continue + break
- [x] e2e_loop_break_value — loop as expression returning value via break

---

## Phase 5 — Tooling & Compiler Backend

> See `docs/PHASE5_PLAN.md` for comprehensive plan.

### Sprint 5.1 — Code Formatter ✅

#### T5.1.1 — Formatter module ✅
- [x] Created src/formatter/mod.rs — `pub fn format(source: &str) -> Result<String, FjError>`
- [x] Created src/formatter/pretty.rs — `Formatter` struct walks AST and emits formatted source
- [x] Registered `pub mod formatter` in src/lib.rs

#### T5.1.2 — Comment preservation ✅
- [x] Added `Comment` struct to lexer (pos, text, is_doc, is_block)
- [x] Added `tokenize_with_comments()` to lexer — returns `(Vec<Token>, Vec<Comment>)`
- [x] Added `collect_whitespace_and_comments()` — captures comments during tokenization
- [x] Added `Cursor::slice()` method for extracting comment text
- [x] Formatter emits comments at correct positions relative to AST nodes

#### T5.1.3 — Item formatting ✅
- [x] Functions: annotation on separate line, params with proper spacing
- [x] Structs: fields on separate lines with trailing commas
- [x] Enums: variants on separate lines with trailing commas
- [x] Impl blocks: methods indented inside
- [x] Trait definitions
- [x] Const definitions
- [x] Use declarations (Simple, Glob, Group)
- [x] Module declarations (mod name { items })

#### T5.1.4 — Expression formatting ✅
- [x] Binary operators with spaces: `a + b`
- [x] If/else with braces, else on same line as `}`
- [x] Match arms indented with `=>`
- [x] While, For, Loop blocks
- [x] Pipeline `|>` with spaces
- [x] Array, Tuple, Range literals
- [x] Cast expressions, Try `?`, Closures
- [x] Struct init, Field access, Method calls
- [x] Preserves hex/binary/octal literal formats

#### T5.1.5 — CLI integration ✅
- [x] Added `Fmt` subcommand: `fj fmt <file.fj>`
- [x] `fj fmt file.fj` — rewrites file in-place
- [x] `fj fmt --check file.fj` — exit 0 if formatted, 1 if not

#### T5.1.6 — Tests ✅ (19 unit + 5 integration = 24 tests)
- [x] 19 unit tests in formatter/pretty.rs (all expression types, items, idempotency)
- [x] e2e_formatter_idempotent_hello — format(format(hello.fj)) == format(hello.fj)
- [x] e2e_formatter_preserves_comments — top-level and inline comments preserved
- [x] e2e_formatter_normalizes_spacing — inconsistent spacing fixed
- [x] e2e_formatter_formatted_code_still_runs — formatted code produces same output
- [x] e2e_formatter_check_mode — already-formatted code returns unchanged

### Sprint 5.2 — Bytecode VM ✅
- [x] T5.2.1: Instruction set design (~45 opcodes in `vm/instruction.rs`)
  - Arithmetic: Add, Sub, Mul, Div, Rem, Pow, Neg
  - Comparison: Eq, Ne, Lt, Le, Gt, Ge
  - Logical: Not, BitAnd, BitOr, BitXor, BitNot, Shl, Shr
  - Variables: GetLocal, SetLocal, GetGlobal, SetGlobal, DefineGlobal
  - Control: Jump, JumpIfFalse, JumpIfTrue, Call, Return
  - Data: NewArray, NewTuple, NewStruct, GetField, SetField, GetIndex, SetIndex, NewEnum
  - I/O: Print, Println
  - Other: Const, Pop, Dup, Halt
- [x] T5.2.2: Constant pool & function table (`vm/chunk.rs`)
  - Chunk: code, constants (deduped), names (deduped), functions (FunctionEntry), lines
  - FunctionEntry: name, arity, local_count, code_start, code_end
- [x] T5.2.3: Compiler: AST → bytecode (`vm/compiler.rs`)
  - Two-pass: first registers functions/structs, then compiles bodies
  - Pratt-style expression compilation
  - For-range optimization (counter-based loops, not array materialization)
  - Match compilation (subject duplication + pattern comparison)
  - Closure compilation (anonymous function entries)
- [x] T5.2.4: VM execution engine (`vm/engine.rs`)
  - Stack-based fetch-decode-execute loop
  - CallFrame stack for function calls
  - Function registration as globals at startup
  - run_until_return() for call_main() support
- [x] T5.2.5: Built-in function dispatch (29 builtins + PI, E constants)
  - len, type_of, push, pop, to_string, to_int, to_float
  - Math: abs, sqrt, pow, log, log2, log10, sin, cos, tan, floor, ceil, round, clamp, min, max
  - Debug: assert, assert_eq, panic, todo, dbg, eprint, eprintln
- [x] T5.2.6: CLI integration (`fj run --vm`)
- [x] T5.2.7: Tests (15 VM integration tests, all pass)
  - hello.fj, fibonacci.fj, factorial.fj on VM
  - Arithmetic, boolean, string, variables, if/else, while, for-range
  - Recursive functions, builtins (len, type_of)
  - VM-vs-interpreter parity tests
- Notes: 769 total tests passing. clippy clean.

### Sprint 5.2.1 — VM Gap Fixes (CRITICAL) ✅
- [x] T5.2.1a: Fix logical AND/OR short-circuiting (CRITICAL)
  - Compiled `&&`/`||` with Dup + JumpIfFalse/JumpIfTrue pattern
- [x] T5.2.1b: Implement SetField, SetIndex, NewEnum (CRITICAL)
  - SetField, SetIndex, NewEnum fully implemented in engine dispatch
- [x] T5.2.1c: Deduplicate dispatch logic (TECH DEBT)
  - Extracted `dispatch_op()` with `DispatchResult` enum, deleted `execute_one()`
- [x] T5.2.1d: Fix pipe operator stack order (HIGH)
  - `x |> f` now compiles callee first, then arg, then Call(1)
- [x] T5.2.1e: Fix break/continue locals cleanup (HIGH)
  - Emit Pop for locals declared inside loop body before jumping
- [x] T5.2.1f: Fix closure environment capture (HIGH)
  - Bug: `collect_free_vars` didn't recurse into Block tail expression
  - Fix: Added `if let Some(tail) = expr` branch in Block handler
  - Capture via GetLocal + DefineGlobal; closure body accesses via GetGlobal
- [x] T5.2.1g: VM parity tests (HIGH)
  - 25 VM tests passing: hello_world, fibonacci, factorial, arithmetic,
    boolean_logic, string_concat, variable_binding, if_else, while_loop,
    for_range, recursive_function, builtins, short-circuit, structs,
    arrays, enums, pipe, closures (with and without capture)
- Notes: 779 total tests passing. clippy clean. Completed 2026-03-05.

### Sprint 5.3 — LSP Server ✅
- [x] T5.3.1: LSP server skeleton (tower-lsp)
  - FajarLspBackend with tower_lsp::LanguageServer trait
  - Capabilities: text sync (full), hover, completion, go-to-definition
  - Async-safe: Mutex guard dropped before await points
- [x] T5.3.2: Diagnostics (on-change)
  - Full pipeline: lex → parse → analyze on every text change
  - Error codes mapped: LE001-LE008, PE001-PE010, SE001-SE012, KE/DE
  - Severity: errors vs warnings (SE009 UnusedVariable = warning)
- [x] T5.3.3: Hover (show types)
  - Keywords: 27 keywords with descriptions + code examples
  - Builtins: 29 functions with signatures
  - Types: 19 primitive types with descriptions
  - Annotations: @kernel, @device, @safe, @unsafe, @ffi
- [x] T5.3.4: Go-to-definition
  - Text-based search for fn/let/struct/enum/const/trait definitions
  - Returns location of name in definition
- [x] T5.3.5: Completions
  - Keywords (27), builtins (29 with signatures), types (19), annotations (5)
  - Trigger chars: `.` and `:`
- [x] T5.3.6: CLI integration (`fj lsp`)
  - Lsp subcommand starts server on stdin/stdout via tokio runtime
- [x] T5.3.7: VS Code extension (TextMate grammar + LSP client)
  - editors/vscode/: package.json, extension.js, language-configuration.json
  - TextMate grammar: keywords, types, operators, strings, comments, annotations, functions
  - LSP client connects to `fj lsp` via stdio
- [x] T5.3.8: Tests (14 unit tests)
  - DocumentState: offset_to_position, span_to_range
  - Diagnostics: clean source, lex error, parse error, semantic error
  - Hover: word_at_position, keyword/builtin/type info lookups
- Notes: 793 total tests passing. clippy clean. Completed 2026-03-05.

### Sprint 5.4 — Package Manager ✅
- [x] T5.4.1: Project manifest (fj.toml)
  - ProjectConfig + PackageInfo structs with serde Deserialize
  - Parse fj.toml with defaults (version="0.1.0", entry="src/main.fj")
  - find_project_root: walk up directories to find fj.toml
- [x] T5.4.2: CLI commands (`fj new`, `fj build`)
  - `fj new <name>` — scaffolds project dir with fj.toml + src/main.fj
  - `fj build` — resolves entry from fj.toml, runs check (lex+parse+analyze)
  - `fj run` (no file) — resolves entry from fj.toml, executes it
- [x] T5.4.3: Module resolution for projects
  - resolve_project_entry: find fj.toml → read entry → resolve path
  - Error messages: "no fj.toml found", "entry point not found"
- [~] T5.4.4: stdlib bundling — deferred (requires file-based module system, Phase 6)
- [x] T5.4.5: Tests (8 unit tests)
  - Manifest: parse minimal, full, invalid, missing name
  - Scaffolding: create project, already exists error
  - Project root: find from subdir, not found
- Notes: 801 total tests passing. clippy clean. Completed 2026-03-05.

### Sprint 5.5 — LLVM Backend (Assessment) ✅
- [x] T5.5.1: Feasibility assessment
  - LLVM 18/19/20 runtime libs on Ubuntu 24.04, llvm-dev not installed (~329MB)
  - inkwell v0.5 supports LLVM 14-20 via feature flags
  - Build impact: ~50MB artifacts, ~10-15s compile
  - Main complexity: strings (heap), closures (upvalues), tensor ops, OS primitives
- [x] Decision: **DEFER to Phase 7** — tree-walker + bytecode VM sufficient for now
- Notes: Assessment documented in src/codegen/mod.rs

### Sprint 5.6 — GPU Backend (Research) ✅
- [x] T5.6.1: Research
  - wgpu: best cross-platform option (Vulkan/Metal/DX12/WebGPU)
  - WGSL compute shaders for matmul, element-wise ops
  - Data transfer overhead: break-even at ~10K+ elements
  - ndarray interop: copy to/from wgpu buffers as raw f32 slices
- [x] Decision: **DEFER to Phase 7** — CPU ndarray sufficient, GPU adds complexity
- Notes: Assessment documented in src/codegen/mod.rs

---

## Phase 6 — Standard Library

> See `docs/PHASE5_PLAN.md` Part 3 for full details.

### Sprint 6.1 — std::string & std::convert ✅
- [x] T6.1.1: String methods (split, trim, trim_start, trim_end, starts_with, ends_with, contains, replace)
- [x] T6.1.2: String methods (to_uppercase, to_lowercase, repeat, chars, substring, is_empty, len)
- [x] T6.1.3: Type conversions (parse_int → Result, parse_float → Result, as cast i64↔f64↔bool, to_string builtin)
- [x] T6.1.4: Array methods (join, reverse, contains, is_empty)
- [x] T6.1.5: Tests (22 integration tests: 14 string + 4 conversion + 4 array)
- Notes: parse_int/parse_float return Result enum (Ok/Err), not runtime errors. From/Into traits deferred (generics). 823 total tests.

### Sprint 6.2 — std::collections ✅
- [x] T6.2.1: HashMap builtins (map_new, map_insert, map_get, map_remove, map_contains_key, map_keys, map_values, map_len)
- [x] T6.2.2: HashMap method-style (.insert, .get, .remove, .contains_key, .keys, .values, .len, .is_empty)
- [x] T6.2.3: Value::Map variant + Display + PartialEq + type_name + VM format_value
- [x] T6.2.4: Collection iteration (for-in on Map yields (key, value) tuples)
- [x] T6.2.5: len() builtin handles Map
- [x] T6.2.6: Tests (7 integration tests: create/insert/get, contains_key, remove, keys/values, method-style, iteration)
- [~] T6.2.7: HashSet — deferred (HashMap covers primary use case)
- Notes: String-keyed maps only (per PLANNING.md decision). 830 total tests.

### Sprint 6.3 — std::io & File I/O ✅
- [x] T6.3.1: read_file, write_file, append_file, file_exists builtins
- [x] T6.3.2: I/O error handling (all return Result enum: Ok/Err)
- [x] T6.3.3: Tests (4 integration tests: write+read, append, exists, read-nonexistent)
- Notes: 834 total tests. File I/O only in @safe context (enforced by existing context rules).

### Sprint 6.4 — OS & NN Stdlib Completion ✅
- [x] T6.4.1: os::memory completion (memory_copy, memory_set, memory_compare on MemoryManager)
- [x] T6.4.4: nn::metrics module (accuracy, precision, recall, f1_score) + 8 unit tests
- [x] T6.4.5: Metrics builtins wired (metric_accuracy, metric_precision, metric_recall, metric_f1_score) + 3 integration tests
- [~] T6.4.2: nn::data (load_csv, DataLoader) — deferred to Phase 7 (requires async I/O + batch logic)
- [~] T6.4.3: nn::layer advanced (Conv2d, Attention, LayerNorm) — deferred to Phase 7 (large scope)
- Notes: 845 total tests. Core stdlib complete. Advanced NN layers planned for Phase 7.

---

## Phase 7 — Production Hardening

> See `docs/PHASE5_PLAN.md` Part 4 for full details.

### Sprint 7.1 — Fuzzing & Property Testing ✅
- [x] T7.1.1: proptest dependency added
- [x] T7.1.2: proptest invariants — 15 property tests across lexer, parser, interpreter, value
  - Lexer: never panics, EOF last, spans in bounds, int roundtrip, string roundtrip
  - Parser: never panics, arithmetic parses, let parses
  - Interpreter: addition commutative, multiplication commutative, string len, double negation
  - Value: int display, bool display, equality reflexive
- [~] T7.1.3: cargo-fuzz — deferred (requires nightly toolchain)
- Notes: 860 total tests.

### Sprint 7.2 — Performance Benchmarks ✅
- [x] T7.2.1: criterion benchmark suite (5 benchmarks)
  - lex_3000_tokens: ~120µs
  - parse_300_stmts: ~190µs
  - fibonacci_20_treewalk: ~26ms
  - loop_1000_iterations: ~293µs
  - string_concat_100: ~73µs
- [~] T7.2.2: Tree-walking vs bytecode comparison — deferred (VM not all features)
- [~] T7.2.3: Memory profiling — deferred (requires valgrind/heaptrack)

### Sprint 7.3 — Security & Safety Audit ✅
- [x] T7.3.1: unsafe block audit — zero `unsafe {}` blocks in codebase (verified via grep)
- [x] T7.3.3: Context isolation verification — 6 security tests (kernel heap, device pointer, immutable, div0, stack overflow, array bounds)
- [x] T7.3.4: Tests — 6 security integration tests added to eval_tests.rs
- [~] T7.3.2: borrow_lite.rs (ME001-ME008) — deferred (ownership analysis is a major feature)
- Notes: 866 total tests. No unsafe code anywhere. All context isolation rules enforced.

### Sprint 7.4 — Documentation Site
- [~] T7.4.1: mdBook setup — deferred (docs/ already comprehensive, 24 documents)
- [~] T7.4.2-T7.4.4: Tutorials — deferred (existing docs/EXAMPLES.md covers use cases)

### Sprint 7.5 — Example Projects ✅
- [x] T7.5.1: examples/collections.fj — HashMap, Array, String demo
- [x] T7.5.2: examples/file_io.fj — File read/write/append with error handling
- [x] T7.5.3: examples/ml_metrics.fj — Classification metrics demo
- [x] T7.5.4: Type checker updated — all new builtins (map, file I/O, metrics, math, debug, autograd, optimizer, layer) registered
- Notes: 8 working examples total. All run through full pipeline (lex → parse → analyze → interpret).

### Sprint 7.6 — LLVM & GPU Full Implementation
- [~] T7.6.1: LLVM codegen — deferred to future version (assessment in Phase 5.5)
- [~] T7.6.2: GPU compute — deferred to future version (assessment in Phase 5.6)
- [~] T7.6.3: @device(gpu) dispatch — deferred

---

## Completed Tasks Archive

*(Move completed sprints here for reference)*

## Daily Log

```
[DATE] Session 1: Started T1.1.1 scaffolding
[DATE] Session 2: ...
```

---

*Last Updated: 2026-03-05 — Sprint 5.2 COMPLETE. 769 tests. VM has critical gaps (short-circuit, mutation, enum). Next: Sprint 5.2.1 (VM Gap Fixes) before LSP.*
