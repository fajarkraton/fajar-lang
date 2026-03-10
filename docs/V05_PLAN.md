# Fajar Lang v0.5 "Ascendancy" — Implementation Plan

> **Focus:** Developer experience, language completeness, real-world usability
> **Timeline:** 8 sprints, ~80 tasks, 2-3 months
> **Prerequisite:** v0.4 "Sovereignty" RELEASED (2026-03-10)
> **Theme:** *"Make the language usable for real projects"*

---

## Motivation

v0.3-v0.4 built powerful infrastructure (concurrency, ML, bare metal, generic enums, RAII). But critical developer-facing features are missing:

- **No test runner** — users cannot write tests in `.fj` files
- **No doc generation** — no `fj doc` command, no doc comments extraction
- **No trait objects** — `dyn Trait` / dynamic dispatch not supported in codegen
- **No iterator protocol** — `for x in collection` works only for ranges and arrays
- **No string interpolation** — concatenation requires explicit `+` and `to_string()`
- **No `fj watch`** — no auto-rebuild on file change
- **Limited error recovery** — parser stops at first error in many cases

v0.5 targets these gaps to make Fajar Lang a language people can actually build projects with.

---

## Sprint Plan

### Sprint 1: Test Framework `P0` `CRITICAL` ✅

**Goal:** `@test` annotation + `fj test` CLI command

- [x] S1.1 — Lexer: `AtTest`, `AtShouldPanic`, `AtIgnore` tokens
- [x] S1.2 — Parser: `@test` on `fn` items → `FnDef { is_test: true }`
- [x] S1.3 — CLI: `fj test` subcommand (discover + run all `@test` functions)
- [x] S1.4 — Test runner: collect test fns, run each in isolation, report pass/fail/panic
- [x] S1.5 — `assert_eq(a, b)` detects failures (existing builtin)
- [x] S1.6 — `@should_panic` — expect a panic/assertion failure
- [x] S1.7 — Test filtering: `fj test --filter name_pattern`
- [x] S1.8 — Test output: summary table (passed/failed/ignored), colored output
- [x] S1.9 — `@ignore` attribute — skip test unless `--include-ignored`
- [x] S1.10 — 10 tests: test discovery, pass/fail, should_panic, filter, ignore, lexer tokens

### Sprint 2: Doc Comments & Generation `P1` ✅

**Goal:** `/// doc comments` + `fj doc` command

- [x] S2.1 — Lexer: `///` doc comment tokens (preserve content, attach to next item)
- [x] S2.2 — Parser: store doc comments on FnDef, StructDef, EnumDef, TraitDef, ImplBlock
- [x] S2.3 — CLI: `fj doc` subcommand — generate HTML from doc comments
- [x] S2.4 — Doc renderer: Markdown-in-doc-comments → HTML (headings, code blocks, lists)
- [x] S2.5 — Module index page: list all public functions, structs, enums, traits
- [x] S2.6 — Function signatures in output: `fn name(params) -> ReturnType`
- [x] S2.7 — Cross-references: `[`OtherType`]` links within docs
- [x] S2.8 — `fj doc --open` — generate and open in browser
- [x] S2.9 — Doc tests: `/// ``` ... ```` code blocks are extracted and run as tests
- [x] S2.10 — 11 tests: doc comment parsing, HTML generation, doc test extraction, module index

### Sprint 3: Trait Objects & Dynamic Dispatch `P1`

**Goal:** `dyn Trait` with vtable-based dispatch in native codegen

- [ ] S3.1 — Parser: `dyn Trait` in type position → `TypeExpr::DynTrait(name)`
- [ ] S3.2 — Analyzer: validate `dyn Trait` usage — trait must exist, object-safe check
- [ ] S3.3 — Object safety rules: no generic methods, no `Self` in return position, no associated types
- [ ] S3.4 — Vtable layout: `[fn_ptr_method1, fn_ptr_method2, ..., drop_fn, size, align]`
- [ ] S3.5 — Fat pointer: `dyn Trait` = `(data_ptr, vtable_ptr)` — two i64 values
- [ ] S3.6 — Codegen: trait object creation — build vtable, pack fat pointer
- [ ] S3.7 — Codegen: method call on `dyn Trait` — vtable lookup + indirect call
- [ ] S3.8 — Interpreter: dynamic dispatch via trait method resolution
- [ ] S3.9 — Coercion: `impl Trait` concrete type → `dyn Trait` (auto-boxing)
- [ ] S3.10 — 10 tests: vtable creation, method dispatch, multiple traits, object safety errors

### Sprint 4: Iterator Protocol `P1`

**Goal:** User-defined iterators with `for x in iterable { }` support

- [ ] S4.1 — `Iterator` trait definition: `trait Iterator { type Item; fn next(&mut self) -> Option<Item>; }`
- [ ] S4.2 — Parser: `for x in expr` desugars to Iterator protocol (call `.next()` until `None`)
- [ ] S4.3 — Built-in iterators: `Range`, `ArrayIter`, `MapIter`, `StringChars`
- [ ] S4.4 — Iterator combinators: `.map(f)`, `.filter(f)`, `.take(n)`, `.enumerate()`
- [ ] S4.5 — `.collect()` — consume iterator into array
- [ ] S4.6 — `.fold(init, f)` and `.sum()`, `.count()`
- [ ] S4.7 — Codegen: iterator protocol in Cranelift (next() call loop, Option check)
- [ ] S4.8 — User-defined iterators: implement Iterator trait on custom structs
- [ ] S4.9 — Lazy evaluation: combinators don't allocate intermediate arrays
- [ ] S4.10 — 10 tests: range iter, array iter, map/filter, collect, user-defined, codegen

### Sprint 5: String Interpolation `P1`

**Goal:** `f"Hello {name}, you are {age} years old"` syntax

- [ ] S5.1 — Lexer: `f"..."` string literal with `{expr}` interpolation holes
- [ ] S5.2 — Parser: `Expr::FString { parts: Vec<FStringPart> }` where parts are literal or expr
- [ ] S5.3 — Analyzer: type-check each interpolated expression (must be Display-able)
- [ ] S5.4 — Interpreter: evaluate f-string by formatting each part and concatenating
- [ ] S5.5 — Codegen: compile f-string to a series of `to_string()` + string concat calls
- [ ] S5.6 — Format specifiers (optional): `{x:.2f}` for float precision, `{x:04d}` for padding
- [ ] S5.7 — Escape handling: `{{` and `}}` for literal braces inside f-strings
- [ ] S5.8 — 8 tests: basic interpolation, nested expr, format specifiers, escapes, codegen

### Sprint 6: Error Recovery & Diagnostics `P2`

**Goal:** Parser continues after errors, show multiple diagnostics at once

- [ ] S6.1 — Parser error recovery: synchronize on `;`, `}`, `fn`, `struct`, `enum`
- [ ] S6.2 — Collect multiple parse errors (currently stops at first)
- [ ] S6.3 — Suggestion engine: "did you mean X?" for misspelled identifiers
- [ ] S6.4 — Type mismatch hints: show expected type, got type, and possible fix
- [ ] S6.5 — Unused import warnings (SE013)
- [ ] S6.6 — Unreachable pattern warnings in match
- [ ] S6.7 — Missing return type inference: suggest `-> Type` based on body
- [ ] S6.8 — 8 tests: multi-error recovery, suggestions, unused imports, pattern warnings

### Sprint 7: Developer Tools `P2`

**Goal:** `fj watch`, improved REPL, LSP completions

- [ ] S7.1 — `fj watch` command: watch .fj files, re-run on change (notify crate)
- [ ] S7.2 — `fj watch --test` — auto-run tests on file change
- [ ] S7.3 — REPL improvements: multi-line input, syntax highlighting, history search
- [ ] S7.4 — REPL: `:type expr` command to show type without evaluating
- [ ] S7.5 — LSP: auto-completion for identifiers, struct fields, methods
- [ ] S7.6 — LSP: go-to-definition for functions, structs, traits
- [ ] S7.7 — LSP: hover type information
- [ ] S7.8 — LSP: rename symbol across files
- [ ] S7.9 — `fj bench` command: built-in micro-benchmark framework
- [ ] S7.10 — 8 tests: watch file trigger, REPL multiline, LSP completion, bench runner

### Sprint 8: Polish & Release `P2`

**Goal:** Integration tests, examples, documentation, release

- [ ] S8.1 — Example: `examples/test_framework.fj` — showcase #[test] + assert_eq
- [ ] S8.2 — Example: `examples/iterator_demo.fj` — custom iterator + combinators
- [ ] S8.3 — Example: `examples/trait_objects.fj` — dynamic dispatch patterns
- [ ] S8.4 — Example: `examples/fstring_demo.fj` — string interpolation
- [ ] S8.5 — Update mdBook: test framework chapter, iterator chapter, trait objects chapter
- [ ] S8.6 — Update CHANGELOG.md with v0.5.0 entry
- [ ] S8.7 — Update CLAUDE.md with v0.5 status
- [ ] S8.8 — Integration tests: full pipeline tests for all new features
- [ ] S8.9 — Benchmark: test runner performance, iterator overhead vs manual loops
- [ ] S8.10 — Release: tag v0.5.0, GitHub release, update README

---

## Dependencies

```
S1 (test framework) ─────────────────────────────→ S8 (polish)
S2 (doc comments) ───────────────────────────────→ S8
S3 (trait objects) ──→ S4 (iterators, uses traits) → S8
S5 (f-strings) ──────────────────────────────────→ S8
S6 (error recovery) ─────────────────────────────→ S8
S7 (dev tools) ──────────────────────────────────→ S8
```

**Critical path:** S1 (test framework) is the highest priority — unblocks writing tests in .fj files.

**Parallel tracks:**
- Track A: S1 → S2 (testing + docs)
- Track B: S3 → S4 (type system + iterators)
- Track C: S5 + S6 (syntax + diagnostics)
- Track D: S7 (tooling)

---

## Success Criteria

- [ ] Users can write and run tests in `.fj` files with `fj test`
- [ ] `fj doc` generates browsable HTML documentation from doc comments
- [ ] `dyn Trait` works in both interpreter and native codegen
- [ ] `for x in collection` works for user-defined iterators
- [ ] `f"Hello {name}"` string interpolation works in all backends
- [ ] Parser recovers from errors and shows multiple diagnostics
- [ ] `fj watch` auto-rebuilds on file change
- [ ] All existing tests still pass (2,650+ baseline, zero regression)

---

## Stats Targets

| Metric | v0.4 (current) | v0.5 (target) |
|--------|----------------|---------------|
| Tests | 2,650 | 3,300+ |
| LOC | ~98,000 | ~115,000 |
| Examples | 24 | 28+ |
| Error codes | 71 | 75+ |
| Token kinds | 82+ | 90+ |

---

## Non-Goals (Deferred to v0.6+)

- LLVM backend (Cranelift is sufficient for current targets)
- Board support packages / BSP (need real hardware CI first)
- Full lifetime annotations (NLL borrow checker is sufficient)
- Package registry hosting server (local packages work)
- Debugger / DAP protocol (needs DWARF debug info first)
- RTOS integration (needs BSP first)

---

*V05_PLAN.md v1.0 | Created 2026-03-10*
