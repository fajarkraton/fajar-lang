# Fajar Lang v1.0 — Safety Audit Report

> Generated from source analysis. All claims verified against codebase.

---

## 1. Unsafe Code Inventory

| Location | Usage | SAFETY Comment |
|----------|-------|----------------|
| `src/main.rs:445` | `std::mem::transmute` — JIT function pointer cast | Yes — `main()` compiled with `() -> i64` |
| `src/codegen/cranelift.rs:47` | `std::slice::from_raw_parts` — string data from static section | Yes — data lives in static data section |
| `src/codegen/cranelift.rs:59` | `std::slice::from_raw_parts` — string data from static section | Yes — data lives in static data section |
| `src/codegen/cranelift.rs:1757` | `std::mem::transmute` — JIT function pointer cast (test) | Yes — `main()` compiled with `() -> i64` |
| `src/interpreter/ffi.rs:66` | `libloading::Library::new` — dynamic library loading | Yes — user-specified path, inherently unsafe |
| `src/interpreter/ffi.rs:87` | FFI raw symbol call | Yes — covered by SAFETY comment at line 64 |
| `src/interpreter/ffi.rs:133` | `call_raw` — FFI function invocation | Yes — marshaled through FfiManager |

**Total: 7 unsafe blocks, all with `// SAFETY:` documentation.**

### Justification

- **JIT transmute (2 occurrences):** Required to call JIT-compiled machine code. The function signature is controlled by Cranelift codegen — always `() -> i64` for `main()`.
- **Static string slices (2 occurrences):** String literals are embedded in the data section during AOT/JIT compilation. Pointers and lengths are set at compile time.
- **FFI (3 occurrences):** Dynamic library loading and foreign function calls are inherently unsafe. Guarded by SE013 type checking (only FFI-safe types allowed).

---

## 2. .unwrap() Usage

- **In `src/` production code:** 1 instance (`src/analyzer/scope.rs:166`)
  - `self.scopes.pop().unwrap()` — guarded by `if self.scopes.len() <= 1 { return }` on line 163
  - **Risk: None** — pop is only called when stack has 2+ elements
- **In test code (`#[cfg(test)]`, `tests/`):** Extensively used (expected)
- **No `.unwrap()` in library-facing code paths**

---

## 3. Panic Safety

- **Zero `panic!()` in library code** — all errors return `Result` or `Option`
- `todo!()` macros: none in production paths
- All runtime errors produce `FjError` variants with error codes

---

## 4. Context Isolation Enforcement

| Check | Mechanism | Error Code |
|-------|-----------|------------|
| No heap in `@kernel` | Analyzer `check_context_violation()` | KE001 |
| No tensor in `@kernel` | Analyzer `check_context_violation()` | KE002 |
| No cross-context calls | Analyzer `check_fn_call_context()` | KE003 |
| No raw pointer in `@device` | Analyzer `check_context_violation()` | DE001 |
| No OS primitives in `@device` | Analyzer `check_context_violation()` | DE002 |

**57 safety integration tests** in `tests/safety_tests.rs` cover all context isolation paths.

---

## 5. Type Safety

- **No implicit type conversions** — `as` cast required
- **Exhaustive match** — SE011 for non-exhaustive patterns
- **Option/Result** — must be matched or propagated with `?`
- **FFI boundary** — SE013 rejects non-primitive types (String, Array, Tensor)

---

## 6. Memory Safety

| Feature | Status | Coverage |
|---------|--------|----------|
| Move semantics | ME001 UseAfterMove | 5 safety tests |
| Borrow checking | ME003-ME005 | 3 eval tests |
| Integer overflow | RE009 checked | 12 safety tests |
| Array bounds | RE010 IndexOutOfBounds | 5 safety tests |
| Stack overflow | RE003 depth limit | 4 safety tests |
| Division by zero | RE004 | 3 safety tests |
| Null safety | Option<T> required | 7 safety tests |

---

## 7. Test Coverage Summary

| Category | Tests |
|----------|-------|
| Unit tests | 1024 |
| Integration (eval) | 148 |
| ML tests | 39 |
| OS tests | 16 |
| Property/fuzz | 33 |
| Autograd | 13 |
| Safety | 57 |
| Doc tests | 3 |
| **Total** | **1333** |

---

## 8. Dependencies Audit

All dependencies are well-known, maintained crates:

| Dependency | Purpose | Risk |
|------------|---------|------|
| ndarray | Tensor backend | Low — mature, widely used |
| thiserror | Error derive | Low — zero-cost |
| miette | Error display | Low — display only |
| clap | CLI parsing | Low — standard |
| rustyline | REPL | Low — terminal only |
| libloading | FFI | Medium — loads native code (gated behind FFI) |
| serde/serde_json/toml | Config parsing | Low |
| tokio/tower-lsp | LSP server | Low — network-local only |

**No known CVEs** in pinned dependency versions (Cargo.lock).

---

## 9. Conclusion

Fajar Lang v1.0 has a minimal unsafe footprint (7 blocks, all documented), comprehensive safety testing (57 dedicated safety tests), and enforces memory/type/context safety through the analyzer pipeline. The primary risk surface is FFI (inherently unsafe) which is guarded by SE013 type restrictions.

**Audit status: PASS**
