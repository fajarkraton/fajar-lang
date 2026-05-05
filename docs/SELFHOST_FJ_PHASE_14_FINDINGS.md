---
phase: 14 — Dynamic [i64] arrays (R14 second increment)
status: CLOSED 2026-05-05
budget: ~1.5-2h realistic
actual: ~1h Claude time
variance: -50% to -67%
artifacts:
  - This findings doc
  - codegen.fj +20 LOC: `_FjArr` struct + 4 helpers in emit_preamble
  - codegen.fj +3 LOC: map_type for `[i64]` → `_FjArr*`
  - parser_ast.fj +30 LOC: `[type]` annotation parsing in let + fn params; `[a,b,c]` array literal parsing; postfix `arr[i]` indexing
  - codegen_driver.fj +30 LOC: BEGIN_ARRAY_LIT atom (chained _fj_arr_push_i64), BEGIN_INDEX atom (_fj_arr_get_i64), map_method push/len wired
  - tests/selfhost_stage1_full.rs: 35 → 40 (5 NEW: P36-P40)
prereq: v34.1.0 (Phase 13 string scalars)
---

# fj-lang Self-Hosting — Phase 14 Findings

> **Second R14 increment shipped.** fj-source compiler now handles
> `[i64]` dynamic arrays with `[]`/`[a,b,c]` literals, `.push(x)`,
> `.len()`, `arr[i]` indexing, plus typed array params. Real
> array-processing programs like `sum_first_n` compile end-to-end.

## 14.1 — What Phase 14 added

### C runtime helpers (added to emit_preamble)

```c
typedef struct _FjArr { int64_t* data; size_t len; size_t cap; } _FjArr;

static _FjArr* _fj_arr_new(void) { ... }
static _FjArr* _fj_arr_push_i64(_FjArr* a, int64_t v) { ... }
static int64_t _fj_arr_get_i64(_FjArr* a, int64_t i) { ... }
static int64_t _fj_arr_len(_FjArr* a) { ... }
```

Reference semantics — array is a pointer; mutations via push are
visible to all aliases. No bounds checking (Stage 2 prerequisite,
not production code). Realloc-doubling growth strategy starting at
8 elements.

### Type annotation parsing for `[T]`

Both `let` declarations and fn parameters now accept `[type]` form:

```fj
let mut arr: [i64] = []      // ← was failing before, now parses
fn sum_array(arr: [i64]) -> i64 { ... }
```

parse_let + parse_params extended to scan `[..]` (with depth
tracking for nested brackets, though only flat `[T]` used in subset).

### Array literal `[]` and `[a, b, c]`

New AST: `BEGIN_ARRAY_LIT <expr>* END_ARRAY_LIT`. Parsed by
parse_primary_ast at top of dispatch chain (before keyword detection).
Codegen lowers to chained `_fj_arr_push_i64`:

```c
[1, 2, 3]
// →
_fj_arr_push_i64(_fj_arr_push_i64(_fj_arr_push_i64(_fj_arr_new(), 1), 2), 3)
```

Empty array `[]` → just `_fj_arr_new()`.

### Array indexing `arr[i]`

New AST: `BEGIN_INDEX <name> <idx_expr> END_INDEX`. Parsed in
parse_primary_ast IDENT branch when no field-chain present and
next char is `[`. Codegen lowers to `_fj_arr_get_i64(arr, i)`.

### Method dispatch updated

- `arr.push(x)` → `_fj_arr_push_i64(arr, x)` — returns same array
  pointer (chainable like fj's let-rebind pattern: `arr = arr.push(x)`)
- `arr.len()` → `_fj_arr_len(arr)`
- emit_let infers `_FjArr*` for both `BEGIN_ARRAY_LIT` first-atom AND
  `BEGIN_METHOD_CALL` with method == "push" (for chained push results)

## 14.2 — sum_first_n headline test

```fj
fn sum_first_n(n: i64) -> i64 {
    let mut arr: [i64] = []
    let mut i = 0
    while i < n {
        arr = arr.push(i)
        i = i + 1
    }
    let mut total = 0
    let mut k = 0
    while k < arr.len() {
        total = total + arr[k]
        k = k + 1
    }
    return total
}

fn main() -> i64 { return sum_first_n(5) }
```

Compiles to:

```c
int64_t sum_first_n(int64_t n) {
    _FjArr* arr = _fj_arr_new();
    int64_t i = 0;
    while ((i < n)) {
        arr = _fj_arr_push_i64(arr, i);
        i = (i + 1);
    }
    int64_t total = 0;
    int64_t k = 0;
    while ((k < _fj_arr_len(arr))) {
        total = (total + _fj_arr_get_i64(arr, k));
        k = (k + 1);
    }
    return total;
}
```

`sum_first_n(5)` → 0+1+2+3+4 = 10. RC=10 ✅.

## 14.3 — Test suite expansion: 35 → 40

```
test full_p36_empty_array_lit_and_len ... ok            # arr.len() on []
test full_p37_array_lit_with_elems ... ok               # [1,2,3,4,5].len()
test full_p38_array_push_and_index ... ok               # arr.push() + arr[i]
test full_p39_sum_first_n_via_array ... ok              # headline composability
test full_p40_array_passed_to_fn ... ok                 # fn param: [i64]

[+ 35 prior P1-P35 still PASS]

test result: ok. 40 passed; 0 failed; finished in 0.22s
```

## 14.4 — R14 progress

| R14 increment | Phase | Status |
|---|---|---|
| String values + ==/!= → strcmp | Phase 13 | ✅ |
| Method call `obj.fn(args)` | Phase 13 | ✅ |
| `s.substring(a, b)`, strlen | Phase 13 | ✅ |
| `[i64]` dynamic arrays + push + len + index | Phase 14 | ✅ |
| Typed array params `fn f(arr: [i64])` | Phase 14 | ✅ |
| `[str]` dynamic arrays | Phase 15 | ⏳ |
| `concat!` variadic macro | Phase 15 | ⏳ |
| `to_int(s)` / `to_string(n)` | Phase 15 | ⏳ |
| Self-compile of stdlib/parser_ast.fj | Phase 16 | ⏳ |
| Stage 1 == Stage 2 byte-equal | Phase 17 | ⏳ |

## 14.5 — Effort recap

| Sub-item | Plan | Actual |
|---|---|---|
| 14.A C runtime _FjArr + 4 helpers | 30min | 5min |
| 14.B map_type [i64] → _FjArr* | 5min | 2min |
| 14.C parser type-annotation extension (let + params) | 20min | 10min |
| 14.D array literal AST + codegen | 30min | 10min |
| 14.E array indexing AST + codegen | 20min | 10min |
| 14.F push/len method dispatch | 15min | 5min |
| 14.G emit_let array-type inference | 10min | 5min |
| 14.H 5 new tests | 15min | 10min |
| 14.I findings doc | 30min | 15min |
| **Total** | **~3h** | **~1h** |
| **Variance** | — | **-67%** |

## 14.6 — Honest scope (CLAUDE.md §6.6 R3)

- ✅ `[i64]` arrays work end-to-end
- ❌ `[str]` arrays NOT supported yet (Phase 15) — current
  `_fj_arr_push_i64` is hardcoded for int64 elements; string arrays
  would need parallel `_fj_arr_push_str` variant + element-type
  tracking
- ❌ Multi-dimensional arrays (`[[i64]]`) — type parser handles via
  bracket-depth tracking but codegen doesn't lower nested types
- ❌ Array bounds checking — production code would add panic on
  out-of-bounds
- ❌ Memory free — same R15 leak class as Phase 13 substring helpers
- ❌ Generic `len(x)` (polymorphic over any sequence) — only
  `arr.len()` method form works for arrays. For strings, use
  explicit `strlen(s)`. Polymorphic dispatch needs type tracking.

## 14.7 — Cumulative state at v34.2.0

16 self-host phases (0-14) closed; cumulative ~11h Claude time
across v33.4.0..v34.2.0.

| Suite | Tests | Status |
|---|---|---|
| Stage-1 subset | 5 | ✅ |
| Stage-1 full | 40 (5 NEW) | ✅ |
| Stage 2 reproducibility | 6 | ✅ |
| **Total self-host** | **51** | ✅ |

## Decision gate (§6.8 R6)

This file committed → v34.2.0 release commit ready (minor bump).

---

*SELFHOST_FJ_PHASE_14_FINDINGS — 2026-05-05. Phase 14 closed in
~1h vs ~3h budget (-67%). [i64] dynamic arrays + push + len + index
+ typed params shipped. sum_first_n compiles end-to-end. R14 progress:
2 of 5 increments closed (Phases 13-14). String arrays + concat!
macro + to_int/to_string queued for Phase 15.*
