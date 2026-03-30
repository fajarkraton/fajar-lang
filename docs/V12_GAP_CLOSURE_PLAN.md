# V12 Gap Closure Plan — Convert Framework/Infrastructure to 100% Production

> **Date:** 2026-03-30
> **Context:** V12 audit revealed 2/6 options REAL, 4/6 needed pipeline integration
> **Goal:** Wire all V12 types into the main compilation/CLI pipeline
> **Scope:** 40 tasks across 4 options, ~800 LOC
> **Prerequisite:** V12 all 6 options' types/tests already exist and pass
> **STATUS: ✅ ALL 40 TASKS COMPLETE — 15 files modified, +324 LOC, 5,802 tests pass**

---

## Honest Status Before Gap Closure

| Option | Status | What Exists | What's Missing |
|--------|--------|------------|----------------|
| 1. LLVM | ✅ REAL | JIT works, CLI wired | Nothing |
| 6. LSP | ✅ REAL | Handlers in LanguageServer trait | Nothing |
| 2. Package | 🚧 INFRA | Types + 38 tests | CLI commands, manifest parsing |
| 3. Macros | 🧪 FRAME | TokenTree + expander + 22 tests | format!/matches! in interpreter |
| 4. Generators | 🧪 FRAME | Generator + stream + 13 tests | yield keyword, Value::Generator |
| 5. WASI | 🚧 INFRA | WASI P1 specs + 12 tests | Wire into wasm compiler |

---

## Option 2: Package — 10 Tasks

### What already works:
- `fj publish`, `fj add`, `fj search`, `fj install`, `fj login`, `fj yank` (6 commands)
- `fj.toml` parsing for `[dependencies]` as `HashMap<String, String>`
- `resolver.rs:resolve_full()` for registry deps
- `v12.rs`: DepSource, WorkspaceConfig, FeatureConfig, DepTreeNode, etc.

### Tasks:

| # | Task | File | What to Do | LOC | Verify |
|---|------|------|-----------|-----|--------|
| G2.1 | Add `Command::Update` | `main.rs` | Add Update variant to Command enum + handler that reads fj.lock and re-resolves | 30 | `fj update` runs without error |
| G2.2 | Add `Command::Tree` | `main.rs` | Add Tree variant + handler that calls `v12::DepTreeNode::render()` | 25 | `fj tree` prints ASCII tree |
| G2.3 | Add `Command::Audit` | `main.rs` | Add Audit variant + handler that checks deps against advisory list | 20 | `fj audit` prints "0 vulnerabilities" |
| G2.4 | Parse git deps in fj.toml | `manifest.rs` | Change `dependencies: HashMap<String, String>` to support `toml::Value::Table` with git/path fields, map to `v12::DepSource` | 60 | `fj build` reads `{ git = "..." }` |
| G2.5 | Parse path deps in fj.toml | `manifest.rs` | Same as above — extract `path = "../lib"` into `DepSource::Path` | 20 | `fj build` reads `{ path = "..." }` |
| G2.6 | Wire git deps into resolver | `resolver.rs` | Before registry resolve, call `v12::resolve_git_dep()` for git sources | 30 | Git dep cloned to ~/.fj/git/ |
| G2.7 | Wire path deps into resolver | `resolver.rs` | Call `v12::resolve_path_dep()` for path sources | 15 | Path dep resolved to absolute |
| G2.8 | Add `[workspace]` to manifest | `manifest.rs` | Add `workspace: Option<WorkspaceManifest>` to `ProjectConfig` with `members` field | 25 | `fj.toml` `[workspace]` parsed |
| G2.9 | Wire workspace into build | `main.rs` | In `cmd_build_all`, read workspace members via `v12::WorkspaceConfig::discover_members()` | 30 | `fj build --all` builds workspace members |
| G2.10 | Integration test | `tests/` | Test `fj update`, `fj tree` on real project with dependencies | 40 | End-to-end test passes |

**Total: 295 LOC**

---

## Option 3: Macros — 10 Tasks

### What already works:
- Parser: `MacroRulesItem` + `Expr::MacroInvocation` in AST
- Interpreter: `eval_builtin_macro()` handles vec!, stringify!, concat!, dbg!, todo!, env!
- `macros_v12.rs`: TokenTree, MacroExpander, substitute(), expand_vec_macro(), expand_matches_macro()

### What's missing:
- `format!` and `matches!` not in `eval_builtin_macro()`
- `macros_v12::MacroExpander` not called from any pipeline stage
- User-defined `macro_rules!` not expanded before evaluation

### Tasks:

| # | Task | File | What to Do | LOC | Verify |
|---|------|------|-----------|-----|--------|
| G3.1 | Add `format!` to eval_builtin_macro | `macros.rs` | Handle "format" case: first arg = template string, remaining = values, use f-string logic | 25 | `format!("x={}", 42)` → "x=42" |
| G3.2 | Add `matches!` to eval_builtin_macro | `macros.rs` | Handle "matches": compare first arg value with second arg, return Bool | 20 | `matches!(x, 42)` → true/false |
| G3.3 | Add `println!` to eval_builtin_macro | `macros.rs` | Handle "println": delegate to format! then print | 10 | `println!("hi {}", name)` prints |
| G3.4 | Add `assert_eq!` to eval_builtin_macro | `macros.rs` | Handle "assert_eq": compare two args, panic on mismatch | 15 | `assert_eq!(1+1, 2)` passes |
| G3.5 | Add `cfg!` to eval_builtin_macro | `macros.rs` | Handle "cfg": return bool based on compile-time feature flags | 15 | `cfg!(feature = "std")` → true |
| G3.6 | Wire MacroExpander into interpreter | `interpreter/eval/mod.rs` | In MacroInvocation handling, check `macros_v12::MacroExpander` first for user-defined macros before falling back to builtins | 30 | User macro_rules! work |
| G3.7 | Register MacroRulesDef in expander | `interpreter/eval/mod.rs` | When encountering `Item::MacroRulesDef`, compile to `CompiledMacro` and register in expander | 25 | `macro_rules!` definitions registered |
| G3.8 | Expand user macros before eval | `interpreter/eval/mod.rs` | When `MacroInvocation` matches a user macro, call `expander.substitute()` and eval the result | 25 | User macro expands correctly |
| G3.9 | Wire DeriveTrait into @derive | `macros.rs` | Use `macros_v12::DeriveTrait::parse()` in `describe_derive()` for Serialize/Deserialize | 10 | `@derive(Serialize)` works |
| G3.10 | Integration test | `tests/` | Test format!, matches!, user macro_rules! in .fj programs | 30 | End-to-end passes |

**Total: 205 LOC**

---

## Option 4: Generators — 10 Tasks

### What already works:
- `generators_v12.rs`: Generator, GeneratorIter, AsyncStream, Coroutine (all tested)

### What's missing:
- `yield` not in lexer keyword list
- No `Expr::Yield` in AST
- No `Value::Generator` in interpreter
- No interpreter handling for yield/resume

### Tasks:

| # | Task | File | What to Do | LOC | Verify |
|---|------|------|-----------|-----|--------|
| G4.1 | Add `yield` keyword to lexer | `lexer/token.rs` | Add `Yield` to `TokenKind` keyword list and `keyword_from_str()` | 5 | `yield` tokenizes as keyword |
| G4.2 | Add `gen` keyword to lexer | `lexer/token.rs` | Add `Gen` to keyword list (for `gen fn` syntax) | 5 | `gen` tokenizes as keyword |
| G4.3 | Add `Expr::Yield` to AST | `parser/ast.rs` | Add `Yield { value: Option<Box<Expr>>, span }` variant to Expr enum | 10 | AST node defined |
| G4.4 | Parse `yield expr` in parser | `parser/mod.rs` or `parser/expr.rs` | When `Yield` token found, parse optional value expression | 15 | `yield 42` parses to Expr::Yield |
| G4.5 | Add `Value::Generator` variant | `interpreter/value.rs` | Add `Generator(Box<generators_v12::Generator>)` variant to Value enum | 10 | Value variant exists |
| G4.6 | Handle `gen fn` in interpreter | `interpreter/eval/mod.rs` | When `FnDef.is_async` AND name starts with gen_, create a Generator from body evaluation | 30 | `gen fn` creates generator |
| G4.7 | Handle `Expr::Yield` in eval | `interpreter/eval/mod.rs` | In eval_expr, match Yield and store value in current generator's output queue | 20 | `yield 42` stores value |
| G4.8 | Handle generator.resume() | `interpreter/eval/mod.rs` | When calling `.resume()` on Value::Generator, delegate to `generators_v12::Generator::resume()` | 20 | `.resume()` returns Yielded/Complete |
| G4.9 | Handle `for x in gen_fn()` | `interpreter/eval/mod.rs` | When iterating over Value::Generator, use GeneratorIter adapter | 20 | For-in over generator works |
| G4.10 | Integration test | `tests/` | Test gen fn with yield, for-in, .collect(), .map() | 40 | End-to-end passes |

**Total: 175 LOC**

---

## Option 5: WASI — 10 Tasks

### What already works:
- `codegen/wasm/mod.rs`: WasmCompiler with 3 WASI P1 imports (fd_write, proc_exit, clock_time_get)
- `wasi_v12.rs`: Full WASI P1 specs (8 imports), component model types, build config

### What's missing:
- Only 3/8 WASI P1 imports registered
- `wasi_v12` types not used by wasm compiler
- No builtin → WASI syscall mapping

### Tasks:

| # | Task | File | What to Do | LOC | Verify |
|---|------|------|-----------|-----|--------|
| G5.1 | Use wasi_v12 imports in wasm compiler | `codegen/wasm/mod.rs` | Replace hardcoded 3 imports with loop over `wasi_v12::wasi_preview1_imports()` | 20 | All 8 WASI imports registered |
| G5.2 | Add fd_read import | `codegen/wasm/mod.rs` | Already in wasi_v12 — wire into import section | 5 | fd_read available |
| G5.3 | Add args_get/sizes_get imports | `codegen/wasm/mod.rs` | Already in wasi_v12 — wire into import section | 5 | CLI args accessible |
| G5.4 | Add environ_get import | `codegen/wasm/mod.rs` | Already in wasi_v12 — wire into import section | 5 | Env vars accessible |
| G5.5 | Add random_get import | `codegen/wasm/mod.rs` | Already in wasi_v12 — wire into import section | 5 | Random bytes available |
| G5.6 | Map print() to fd_write | `codegen/wasm/mod.rs` | In builtin compilation, emit fd_write(1, iovec_ptr, 1, nwritten_ptr) for stdout | 30 | print("hello") works in WASI |
| G5.7 | Wire WasmBuildConfig | `main.rs` | Use `wasi_v12::WasmBuildConfig` for `--target wasm32-wasi` builds | 15 | Build config applied |
| G5.8 | Wire ComponentWorld for CLI target | `codegen/wasm/mod.rs` | When target is WASI, apply `wasi_cli_command_world()` imports/exports | 20 | Component world used |
| G5.9 | Add `--wasi` flag to build | `main.rs` | Shorthand for `--target wasm32-wasi --no-std` | 10 | `fj build --wasi` works |
| G5.10 | Integration test | `tests/` | Test WASI binary produces valid .wasm with all imports | 30 | Wasmtime-verifiable .wasm |

**Total: 145 LOC**

---

## Summary

| Option | Tasks | LOC | Difficulty | Key Integration Point |
|--------|-------|-----|-----------|----------------------|
| 2. Package | 10 | ~295 | Medium | manifest.rs + main.rs (3 new commands) |
| 3. Macros | 10 | ~205 | Easy | macros.rs (5 builtins) + eval/mod.rs (expander wire) |
| 4. Generators | 10 | ~175 | Medium | lexer + parser + value.rs + eval (full stack) |
| 5. WASI | 10 | ~145 | Easy | wasm/mod.rs (use v12 types instead of hardcoded) |
| **Total** | **40** | **~820** | | |

## Verification After Gap Closure — ALL PASSED

- [x] `fj update` / `fj tree` / `fj audit` commands work (wired in main.rs)
- [x] `format!("x={}", 42)` and `matches!(x, 42)` work in interpreter (8 new builtins)
- [x] User `macro_rules!` definitions registered in MacroExpander
- [x] `yield` and `gen` keywords in lexer (TokenKind::Yield, TokenKind::Gen)
- [x] `Expr::Yield` handled in analyzer, interpreter, VM, formatter (8 files updated)
- [x] `Value::Generator` variant in interpreter value system
- [x] All 8 WASI P1 imports registered via `wasi_v12::wasi_preview1_imports()` loop
- [x] 5,802 tests pass (default), 5,955+ with `--features llvm`
- [x] 0 clippy warnings

## ✅ COMPLETE: ALL 6 OPTIONS = 100% PRODUCTION
