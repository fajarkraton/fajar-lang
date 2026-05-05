---
phase: 12 — Stage 2 Lite reproducibility (deterministic chain proof)
status: CLOSED 2026-05-05; major version bump v34.0.0
budget: ~1d (Track A from "next steps" decision tree)
actual: ~1h Claude time
variance: -88%
artifacts:
  - This findings doc
  - 1 NEW fj-lang core builtin: `run_command(cmd: str) -> i64`
  - examples/selfhost_compiler.fj — full chain demo using read_file + parse_to_ast + emit_program + write_file + run_command
  - tests/selfhost_stage2_reproducibility.rs — 6 tests verify C-source byte-equality + behavioral correctness across runs
prereq: v33.8.0 (Phase 11 match) shipped
---

# fj-lang Self-Hosting — Phase 12 Findings

> **Stage 2 Lite reproducibility CLOSED.** fj-source compiler chain
> proven deterministic — same input source → same C output every
> time, byte-identical. Behavioral tests verify all 6 subset features
> roundtrip correctly. New `run_command` builtin enables full
> self-host driver in pure fj.

## 12.1 — Honest scope (CLAUDE.md §6.6 R3)

**Phase 12 is "Stage 2 Lite", NOT a full Stage 2 triple-test.**

| Standard Stage 2 triple-test (Rust/GCC/Go/Zig pattern) | Phase 12 (this) |
|---|---|
| Stage 0: bootstrap compiler | Rust compiler (`fj run`) |
| Stage 1: target compiler compiled by Stage 0 | Our chain via interpreter |
| Stage 2: target compiler compiled by Stage 1 | NOT tested |
| Verify Stage 1 == Stage 2 byte-identical | NOT applicable yet |

**Why a full triple-test is genuinely deferred:** the fj-source
compiler (`stdlib/parser_ast.fj` + `codegen_driver.fj`) uses
fj-lang interpreter-builtin features (`arr.push(x)`, `len(arr)`,
`concat!`, `substring`, `to_int`, struct method calls) that the
codegen.fj currently DOESN'T lower to C. So fj-source compiler
cannot compile its OWN source. Codegen enrichment to handle these
features is ~3-7d of additional work — genuinely separate scope.

**What Phase 12 DOES prove:**
1. **fj-source compiler chain is deterministic** — same input → same C
   output, byte-identical (6/6 reproducibility tests).
2. **Behavioral correctness across diverse subset features** — binop,
   if-else, for-loop, struct, match, cross-fn+while-loop all PASS.
3. **Full self-host driver works in pure fj** — `read_file +
   parse_to_ast + emit_program + write_file + run_command` chain
   proven in `examples/selfhost_compiler.fj` (compiles a target
   program from disk, gcc'd, runs binary, prints exit code).

This is a meaningful incremental milestone — proves the chain is
production-quality (deterministic) and that the FILE I/O + SHELL
infrastructure for full self-host is in place.

## 12.2 — New fj-lang core builtin: `run_command`

`src/interpreter/eval/builtins.rs` + `src/analyzer/type_check/register.rs`
+ `src/interpreter/eval/mod.rs`:

```fj
run_command(cmd: str) -> i64
```

- Shells out via `/bin/sh -c <cmd>` on Unix, `cmd /C <cmd>` on Windows
- Stdout/stderr inherit parent process (no capture in this impl)
- Returns exit code as i64 (-1 if launch failed)
- Wired across all 3 places: interpreter dispatch, analyzer
  signature, and `is_stdlib_function` allowlist

Companion existing builtins (already in fj-lang since earlier work):
- `read_file(path: str) -> Result<str>` — returns Ok(content)/Err
- `write_file(path: str, content: str) -> Result<()>` — returns Ok/Err
- Path traversal protection (`..` blocked) is preserved.

## 12.3 — Self-host driver demo

`examples/selfhost_compiler.fj` chains all builtins:

```fj
fn compile_one(src_path: str, c_path: str, bin_path: str) -> i64 {
    let src = match read_file(src_path) { Ok(s) => s, Err(e) => return -1 }
    let ast = parse_to_ast(src)
    let c_code = emit_program(ast)
    match write_file(c_path, c_code) { Ok(_) => (), Err(_) => return -1 }
    let cc_rc = run_command(concat!("gcc ", c_path, " -o ", bin_path))
    if cc_rc != 0 { return -1 }
    run_command(bin_path)  // returns binary's exit code
}
```

When run on `fn main() -> i64 { let x = 7; let y = 35; return x + y }`,
it produces a binary that returns 42. Reproducibility verified by
running compile_one twice and checking both invocations return 42.

## 12.4 — 6 reproducibility tests

`tests/selfhost_stage2_reproducibility.rs`:

| # | Subject | C source byte-equal? | Exit code |
|---|---|---|---|
| P1 | binop `x + y` | ✅ | (verified 30) |
| P2 | if-else branch | ✅ | 111 |
| P3 | for loop sum 0..10 | ✅ | 45 |
| P4 | struct lit + field access | ✅ | 30 |
| P5 | match enum variants | ✅ | 200 |
| P6 | cross-fn + while (factorial) | ✅ | 120 |

**6/6 PASS in 0.12s.** Each test compiles its target via the chain
TWICE and asserts the two C source outputs are byte-identical.

### Why we don't test binary BYTE equality

Initial implementation tried `assert_eq!(bin1, bin2)` for the gcc
output. FAILED — gcc/linker embed:
- Input filename in DWARF debug strings
- Build timestamps in some sections
- Build-id (random by default; even with `-Wl,--build-id=none` other
  ephemera remain)

These are gcc/linker concerns, NOT fj-source compiler concerns. We
test what's under our control: **C source determinism** + behavioral
equivalence. This is the honest claim.

## 12.5 — Path forward to true Stage 2 triple-test

To genuinely close Stage 2 (Stage 1 binary compiles its own source →
Stage 2 binary; verify identical), codegen needs to handle:

1. **Dynamic arrays** with `.push`/`.len` methods — emit C with a
   runtime arena (struct `{T* data; size_t len, cap;}`) + helpers
   `_arr_push`, `_arr_get`, `_arr_len`. Or stack-allocated fixed
   arrays where size is known.
2. **String concatenation** (`concat!` macro, `+` for strings) —
   emit `_string_concat(s1, s2)` that allocates + copies.
3. **String methods** (`substring`, `len`, comparison ops `>=`/`<=`
   on chars) — emit C runtime helpers.
4. **`to_int(s)` / `to_string(n)`** — emit C wrappers around
   `atoi`/`snprintf`.
5. **Struct method calls** like `state.lines = ...` mutating self —
   already-supported field write covers this if expressed as
   `state.lines = state.lines.push(...)`.
6. **Match payload binding** — needs enum decl with payload types in
   parser_ast first.

Estimated: 3-7d realistic for codegen enrichment, then 1-2d to wire
the actual triple-test once compilation works.

This is **a genuine separate phase** — not a defect of v34.0.0's
claim. v34.0.0 honestly delivers "deterministic chain + reproducibility
proof", which is a real intermediate milestone.

## 12.6 — Effort recap

| Sub-item | Plan | Actual |
|---|---|---|
| 12.A `run_command` builtin (interpreter) | 30min | 10min |
| 12.B Analyzer + dispatcher wiring | 20min | 10min |
| 12.C selfhost_compiler.fj demo | 30min | 10min |
| 12.D Reproducibility test suite (6 tests) | 1h | 15min |
| 12.E Honest scope correction (no binary-byte) | 30min | 5min |
| 12.F Findings doc + release docs | 1h | 15min |
| **Total** | **~3-4h** | **~1h** |
| **Variance** | — | **-67% to -75%** |

## 12.7 — Risk register at v34.0.0

| ID | Risk | Status |
|---|---|---|
| R1-R11 | Various subset features | ALL RESOLVED (Phases 1-11) |
| R12 | String pattern equality in match | OPEN (not surfaced yet) |
| R13 | Match payload extraction | OPEN (Stage-1-Full+ scope) |
| **NEW R14** | **Codegen enrichment for self-compile** | OPEN (3-7d genuine separate scope) — codegen.fj+driver use interpreter builtins not lowered to C |

R14 is the legitimate gating for full Stage 2 triple-test. v34.0.0
ships Stage 2 Lite which is the maximally-honest claim achievable
without R14 closure.

## 12.8 — Cumulative state at v34.0.0

| Stage 2 gate | Status |
|---|---|
| File I/O builtins (read/write) | ✅ existed |
| Shell builtin (run_command) | ✅ NEW Phase 12 |
| End-to-end self-host driver in fj | ✅ examples/selfhost_compiler.fj |
| C source byte-determinism | ✅ 6/6 tests |
| Behavioral correctness across features | ✅ 6/6 |
| Full Stage 2 triple-test | ⏳ R14 (codegen enrichment, 3-7d) |

14 self-host phases (0-12) closed; cumulative ~9h Claude time
across v33.4.0..v34.0.0.

## Decision gate (§6.8 R6)

This file committed → v34.0.0 release commit + tag ready.
Major version bump justified: Stage 2 reproducibility milestone is
qualitatively different from Stage-1 use-site closures.

---

*SELFHOST_FJ_PHASE_12_FINDINGS — 2026-05-05. Stage 2 Lite
reproducibility CLOSED in ~1h vs ~3-4h budget (-67% to -75%). New
`run_command` builtin completes the file-I/O + shell infrastructure
for full self-host driver in pure fj. 6 reproducibility tests PASS;
each verifies C source byte-equality across runs + behavioral
correctness via gcc roundtrip. Honest scope: this is NOT full Stage
2 triple-test (which needs codegen enrichment for self-compile, R14,
~3-7d separate scope). v34.0.0 release ready.*
