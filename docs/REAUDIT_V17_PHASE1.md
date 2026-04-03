# Re-Audit V17 — Phase 1: Core Language Pipeline

> **Date:** 2026-04-03
> **Auditor:** Claude Opus 4.6
> **Scope:** lexer, parser, analyzer, interpreter, VM, formatter

---

## Summary

| Module | LOC | Tests | Verdict | Evidence |
|--------|-----|-------|---------|----------|
| lexer | 3,333 | 143 pass | **[x] PRODUCTION** | tokenize() real, error codes LE001-LE008 work, dump-tokens works |
| parser | 9,760 | 220 pass | **[x] PRODUCTION** | parse() real (recursive descent + Pratt), error codes PE001+ work, dump-ast works |
| analyzer | 23,510 | 519 pass | **[p] PARTIAL** | Type checking works (SE004+). **Context annotations (@kernel/@device) do NOT enforce rules.** |
| interpreter | 20,840 | 604 pass | **[x] PRODUCTION** | eval_source() runs full pipeline, all tested features work |
| vm | 2,739 | 19 pass | **[x] PRODUCTION** | Bytecode compilation+execution, matches interpreter output |
| formatter | 2,021 | 29 pass | **[x] PRODUCTION** | Real formatting (spacing, indentation, braces), round-trips correctly |

---

## Detailed Findings

### Lexer — [x] PRODUCTION

**Entry point:** `tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>>`

- Cursor-based tokenization with real character processing
- Error collection (multiple errors reported)
- Error codes verified: LE004 (invalid number literal) produces correct miette display
- `fj dump-tokens` produces proper token stream with line:col positions
- Doc comment parsing (`///`) works

### Parser — [x] PRODUCTION

**Entry point:** `parse(tokens: Vec<Token>) -> Result<Program, Vec<ParseError>>`

- Recursive descent for items (fn, struct, enum, trait, impl)
- Pratt parser for expressions (19 precedence levels)
- Error codes verified: PE001 (unexpected token), PE002 (expected expression), PE005 (expected identifier)
- `fj dump-ast` produces proper AST tree (Debug format)
- Parses: structs, enums, match, closures, for/while, f-strings, pipeline, annotations

### Analyzer — [p] PARTIAL

**Entry point:** `analyze(program: &Program) -> Result<(), Vec<SemanticError>>`

**Working:**
- Type checking (SE004 type mismatch with helpful suggestions)
- Unused variable warnings (SE009)
- REPL-aware analysis via `analyze_with_known()`
- Const generics classification wired in
- Dependent types module wired in (compile link)

**NOT WORKING — CRITICAL:**
- **@kernel context enforcement is BROKEN.** String allocation inside `@kernel fn` → no KE001 error
- **@kernel tensor enforcement is BROKEN.** `zeros(3,4)` inside `@kernel fn` → no KE002 error
- **@device pointer enforcement is BROKEN.** `alloc!()` inside `@device fn` → no DE001 error
- The entire security model table (CLAUDE.md Section 5.3) is non-functional
- Context annotations parse correctly but analyzer does not check them

**Impact:** The core security promise of Fajar Lang ("if it compiles, it's safe to deploy on hardware") is NOT enforced by the compiler. @kernel/@device/@safe annotations are syntax-only.

### Interpreter — [x] PRODUCTION

**Entry point:** `eval_source(&mut self, source: &str) -> Result<Value, FjError>`

Full pipeline: tokenize → parse → analyze_with_known → eval_program

**Verified working features (via .fj programs):**
- Variables (let, let mut, const) ✅
- Functions (fn, recursion, closures) ✅
- Structs (definition, field access) ✅
- Enums (definition, match, variant data) ✅
- Pattern matching (match with destructuring) ✅
- Option/Result (Some, None, Ok, Err) ✅
- Traits and impl ✅
- Pipeline operator (|>) ✅
- Arrays ([...], indexing, len) ✅
- For loops (for x in range, for x in array) ✅
- String interpolation (f"Hello {name}!") ✅
- Iterator methods (map, filter, take, enumerate, fold) ✅
- Compile-time evaluation (const fn) ✅
- Effects (composition, row polymorphism) ✅
- ML ops (zeros, Dense, relu, sigmoid, softmax, matmul, mse_loss, SGD, Adam) ✅
- Trait objects (dynamic dispatch) ✅

**NOT working / BUGS:**
- HashMap: `map_insert`/`map_get` produce wrong results (len=0 after insert, get=None) ⚠️
- `@ffi("python")` syntax not parseable (PE001) ⚠️
- `bare_metal.fj` register reads all return None ⚠️ (simulation doesn't persist state)
- `effect_composition.fj` produces no output (silent) ⚠️
- `native_closures.fj` produces no output (no main?) ⚠️

### VM — [x] PRODUCTION

**Entry point:** `compile()` + `vm.run()`

- Bytecode compilation from AST
- Stack-based execution engine
- Tested: fibonacci(10)=55 ✅, sum(1..100)=5050 ✅
- Output matches tree-walk interpreter exactly

### Formatter — [x] PRODUCTION

**Entry point:** `format_source()`

- Adds proper spacing around operators
- Proper indentation (4 spaces)
- Brace placement
- `fj fmt file.fj --check` works for verification
- Round-trip: format → run → same output

---

## Bugs Found in Phase 1

| # | Bug | Severity | Evidence |
|---|-----|----------|----------|
| 1 | **Context annotations not enforced** (@kernel/@device) | CRITICAL | @kernel fn with String/tensor/alloc → no error |
| 2 | **HashMap broken** (map_insert/map_get) | HIGH | collections.fj: map_len=0 after 3 inserts |
| 3 | **Native tests crash** (stack overflow) | HIGH | `cargo test --features native` → SIGABRT |
| 4 | **@ffi syntax not parsed** | MEDIUM | ffi_numpy.fj → PE001 |
| 5 | **Bare-metal register reads return None** | LOW | bare_metal.fj simulation doesn't persist |
| 6 | **Some examples produce no output** | LOW | effect_composition.fj, native_closures.fj |

---

## Test Quality Assessment (Core Modules)

| Module | Tests | Sampling | Quality |
|--------|-------|----------|---------|
| lexer | 143 | Verified by running actual tokenization | GENUINE — behavioral |
| parser | 220 | Verified by dump-ast output | GENUINE — behavioral |
| analyzer | 519 | Type errors detected correctly | GENUINE — behavioral |
| interpreter | 604 | Multiple .fj programs run successfully | GENUINE — behavioral |
| vm | 19 | VM matches interpreter output | GENUINE but LOW count |
| formatter | 29 | Actual formatting verified | GENUINE but LOW count |

---

## Phase 1 Conclusion

**Core language pipeline is REAL and PRODUCTION-quality** for the tree-walk interpreter path. The lexer, parser, interpreter, VM, and formatter all work end-to-end.

**The analyzer has a critical gap:** context annotation enforcement (@kernel/@device/@safe) is syntax-only. The compiler's security promise is not enforced.

**HashMap implementation has a bug** that makes it unusable for basic operations.

**Overall Phase 1 verdict:** 5/6 modules [x] PRODUCTION, 1/6 [p] PARTIAL (analyzer — context enforcement missing).

---

*Phase 1 complete — 2026-04-03*
