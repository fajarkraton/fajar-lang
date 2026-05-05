---
phase: 13 — String values + method calls (Stage 2 prerequisite)
status: CLOSED 2026-05-05
budget: ~1d realistic per Track A R14 plan
actual: ~1h Claude time
variance: -88%
artifacts:
  - This findings doc
  - parser_ast.fj +50 LOC: BEGIN_METHOD_CALL AST shape via field-chain extension
  - codegen_driver.fj +60 LOC: BEGIN_METHOD_CALL atom handler, map_method registry, find_method_name helper, type-inference for str-returning methods, string `==`/`!=` lowering to strcmp
  - codegen.fj +12 LOC: C runtime helpers `_fj_substring`, `_fj_streq`, `_fj_concat2` in emit_preamble
  - tests/selfhost_stage1_full.rs: 31 → 35 (4 NEW: P32-P35)
prereq: v34.0.0 (Phase 12 Stage 2 Lite)
trigger: Track A — codegen enrichment toward full Stage 2 triple-test
---

# fj-lang Self-Hosting — Phase 13 Findings

> **First R14 increment shipped.** fj-source compiler now handles
> string-typed values, method calls (`s.substring(a, b)`),
> string equality (`s == "x"` → strcmp). Real string-processing
> programs like count_vowels compile end-to-end through the chain.
> Foundation for full Stage 2 self-compile narrative.

## 13.1 — Phase 13 fits Track A R14 plan

R14 ("codegen enrichment for full Stage 2 triple-test") is genuinely
3-7d work. Phase 13 ships the **foundational increment**: string
values + method calls + comparison. Subsequent phases will add
dynamic arrays, `concat!` macro, `len()` polymorphic builtin, etc.

The narrative arc:
- **Phase 13** (this): scalar-string features → simple programs
  like count_vowels compile
- **Phase 14**: dynamic arrays of strings (`[str]`) + `arr.push(x)`
  + `len(arr)` polymorphic
- **Phase 15**: `concat!` variadic macro lowering, integer-to-string
  / string-to-integer conversion (`to_int`, `to_string`)
- **Phase 16**: enough features to compile a small NON-trivial
  fj-source utility (e.g., simplified lexer that tokenizes ints +
  identifiers)
- **Phase 17**: full triple-test — Stage 1 binary compiles its own
  source → Stage 2 binary; verify Stage 1 == Stage 2 byte-identical

Each phase ~30min-2h Claude time per established pattern. Total R14
~6-12h realistic.

## 13.2 — What Phase 13 added

### Method call AST shape

```
BEGIN_METHOD_CALL IDENT <obj> [FIELD <f>]* METHOD <method_name>
                  <arg_expr>* END_METHOD_CALL
```

parse_primary_ast extended: after IDENT + field-chain, detect `(`
to determine if last segment is a method call vs field access.
Composes correctly with binop chain detection.

### C runtime helpers (added to emit_preamble)

```c
static const char* _fj_substring(const char* s, int64_t start, int64_t end);
static int _fj_streq(const char* a, const char* b);
static const char* _fj_concat2(const char* a, const char* b);
```

`_fj_substring` allocates a new string (no GC; binaries are
short-lived). `_fj_streq` wraps `strcmp(...) == 0`. `_fj_concat2`
allocates the concatenation.

### Method → C helper mapping

`map_method` registry in codegen_driver:
- `.substring(a, b)` → `_fj_substring(s, a, b)`
- `.push(x)` → `_fj_arr_push(arr, x)` (Phase 14 will wire helper)
- `.len()` → `_fj_arr_len(arr)` (Phase 14)

### emit_let type inference for method returns

When first atom of let-rhs is `BEGIN_METHOD_CALL`, find the METHOD
name and infer C type:
- `substring`, `concat` → `const char*`
- otherwise → `int64_t`

This makes `let h = s.substring(0, 5)` correctly emit `const char* h
= _fj_substring(s, 0, 5);` instead of `int64_t h = ...;` (which
would be a type error in C).

### String comparison → strcmp

In `parse_expr_emit`, when LHS or RHS atom is `STR`, lower binop:
- `==` → `_fj_streq(a, b)`
- `!=` → `(!_fj_streq(a, b))`

Other binops on strings would be ill-typed; we leave them as-is
(gcc will catch the mistake).

## 13.3 — count_vowels headline test

```fj
fn count_vowels(s: str) -> i64 {
    let mut count = 0
    let mut i = 0
    let n = strlen(s)
    while i < n {
        let c = s.substring(i, i + 1)
        if c == "a" { count = count + 1 }
        if c == "e" { count = count + 1 }
        if c == "i" { count = count + 1 }
        if c == "o" { count = count + 1 }
        if c == "u" { count = count + 1 }
        i = i + 1
    }
    return count
}

fn main() -> i64 { return count_vowels("hello world") }
```

Compiles via the chain to:

```c
int64_t count_vowels(const char* s) {
    int64_t count = 0;
    int64_t i = 0;
    int64_t n = strlen(s);
    while ((i < n)) {
        const char* c = _fj_substring(s, i, (i + 1));
        if (_fj_streq(c, "a")) { count = (count + 1); }
        if (_fj_streq(c, "e")) { count = (count + 1); }
        if (_fj_streq(c, "i")) { count = (count + 1); }
        if (_fj_streq(c, "o")) { count = (count + 1); }
        if (_fj_streq(c, "u")) { count = (count + 1); }
        i = (i + 1);
    }
    return count;
}

int64_t main(void) { return count_vowels("hello world"); }
```

gcc produces a binary that returns 3 (e + o + o in "hello world").

## 13.4 — Test suite expansion: 31 → 35

```
test full_p32_string_param_and_strlen ... ok        # NEW
test full_p33_string_eq_via_strcmp ... ok           # NEW
test full_p34_method_call_substring ... ok          # NEW
test full_p35_count_vowels_composability ... ok     # NEW (headline)

[+ 31 prior P1-P31 still PASS]

test result: ok. 35 passed; 0 failed; finished in 0.18s
```

P35 is the composability headline — combines string param, strlen,
substring method call, string comparison, while loop, conditional
counting. Real fj idiom.

## 13.5 — Stage 2 progress checklist

| R14 increment | Phase | Status |
|---|---|---|
| String values (let, param, return) | Phase 13 | ✅ |
| String comparison (==, !=) → strcmp | Phase 13 | ✅ |
| Method call `obj.fn(args)` syntax | Phase 13 | ✅ |
| `s.substring(a, b)` | Phase 13 | ✅ |
| `strlen(s)` (already worked via direct call) | Phase 13 | ✅ |
| `concat!` variadic macro | Phase 14+ | ⏳ |
| Dynamic array `[str]` + `.push` + `len` | Phase 14+ | ⏳ |
| `to_int(s)` / `to_string(n)` builtins | Phase 15 | ⏳ |
| Full self-compile of stdlib/parser_ast.fj | Phase 16 | ⏳ |
| Triple-test Stage 1 == Stage 2 | Phase 17 | ⏳ |

## 13.6 — Effort recap

| Sub-item | Plan | Actual |
|---|---|---|
| 13.A method call AST shape | 30min | 15min |
| 13.B map_method + parse_atom dispatch | 30min | 10min |
| 13.C C runtime helpers in preamble | 15min | 5min |
| 13.D type inference for method returns | 20min | 10min |
| 13.E string == → strcmp lowering | 15min | 5min |
| 13.F 4 new tests | 15min | 10min |
| 13.G findings doc | 30min | 15min |
| **Total** | **~3-4h** | **~1h** |
| **Variance** | — | **-67% to -75%** |

## 13.7 — Risk register at v34.1.0

| ID | Risk | Status |
|---|---|---|
| R12 | String pattern equality in match | RESOLVED via Phase 13 strcmp lowering |
| R14 | Codegen enrichment for self-compile | **PARTIAL** — Phase 13 covers string scalars; Phase 14+ for arrays/macros |
| **NEW R15** | **Memory leaks** | _fj_substring/_fj_concat2 malloc without free. Test programs are short-lived so OK; production-grade would need arena allocator or refcounting. Honest gap. |

## 13.8 — Cumulative state at v34.1.0

15 self-host phases (0-13) closed; cumulative ~10h Claude time
across v33.4.0..v34.1.0.

| Suite | Tests | Status |
|---|---|---|
| Stage-1 subset | 5 | ✅ |
| Stage-1 full | 35 (4 NEW) | ✅ |
| Stage 2 reproducibility | 6 | ✅ |
| **Total self-host** | **46** | ✅ |

## Decision gate (§6.8 R6)

This file committed → v34.1.0 release commit ready (minor bump:
adds string-handling capability, no breaking change).

---

*SELFHOST_FJ_PHASE_13_FINDINGS — 2026-05-05. Phase 13 closed in
~1h vs ~3-4h budget (-75%). String values + method calls + strcmp
lowering shipped. count_vowels program compiles end-to-end through
the chain. R14 PARTIAL closure — Phase 14+ for arrays + macros.
v34.1.0 release ready.*
