---
phase: 15 — Conversions + concat! macro + [str] arrays (R14 third increment)
status: CLOSED 2026-05-05
budget: ~2-3h realistic
actual: ~1h Claude time
variance: -67%
artifacts:
  - This findings doc
  - codegen.fj +30 LOC: unified _FjArr (void** layout), conversions (_fj_to_int, _fj_to_string), str push/get helpers
  - parser_ast.fj +35 LOC: BEGIN_MACRO_CALL AST shape (`name!(args)` syntax), TYPE marker preserved in BEGIN_LET when annotation present
  - codegen_driver.fj +50 LOC: BEGIN_MACRO_CALL atom (concat! → nested _fj_concat2), TYPE-aware emit_let, .push dispatch by arg atom (_fj_arr_push_str vs _fj_arr_push_i64), to_int/to_string name mapping
  - tests/selfhost_stage1_full.rs: 40 → 45 (5 NEW: P41-P45)
prereq: v34.2.0 (Phase 14 [i64] arrays)
---

# fj-lang Self-Hosting — Phase 15 Findings

> **Third R14 increment shipped.** fj-source compiler now handles
> `concat!` macro, `to_int`/`to_string` conversions, and `[str]`
> dynamic arrays via unified `_FjArr` C type. Programs combining
> string manipulation, conversions, and string arrays compile
> end-to-end through the chain.

## 15.1 — Architectural refactor: unified `_FjArr` (void**)

Phase 14 had separate `_FjArr` (int64*) for `[i64]`. Phase 15 needed
also `[str]` arrays. Two paths:
- **A**: separate `_FjArrStr*` C type — clean but doubles helpers,
  requires var-type tracking for `arr.len()` / `arr[i]` dispatch
- **B**: unified `_FjArr*` (void**) C type — one struct, element-type
  picked at push/get site

**Phase 15 chose B.** All arrays are `_FjArr*` at C level; element
type is determined by which push/get helper is called:

```c
typedef struct _FjArr { void** data; size_t len; size_t cap; } _FjArr;

static _FjArr* _fj_arr_push_i64(_FjArr* a, int64_t v) { /* cast to void* */ }
static _FjArr* _fj_arr_push_str(_FjArr* a, const char* v) { /* store ptr */ }
static int64_t _fj_arr_get_i64(_FjArr* a, int64_t i) { /* cast back */ }
static const char* _fj_arr_get_str(_FjArr* a, int64_t i) { /* cast back */ }
static int64_t _fj_arr_len(_FjArr* a) { /* uniform */ }
```

**Benefit**: `arr.len()` works uniformly. Phase 14 regression: 0
test failures after refactor (verified 51/51 pre-existing tests still
PASS).

**Cost**: indirect element access via void* casts. C strict-aliasing
warnings could appear under `-Wstrict-aliasing=2` but default gcc is
quiet.

## 15.2 — concat! macro

Parser detects `IDENT!(args)` after IDENT word + `!` + `(`:

AST: `BEGIN_MACRO_CALL <name> <args>* END_MACRO_CALL`

Codegen for `concat!`:
- 0 args → `""`
- 1 arg → arg as-is
- 2 args → `_fj_concat2(a, b)`
- 3+ args → right-associative nesting:
  `_fj_concat2(a, _fj_concat2(b, _fj_concat2(c, d)))`

Other macro names emit `/* unknown macro X */` comment.

P43: `concat!("hi ", "world") == "hi world"` → 1 ✅
P44: `concat!("a", "b", "c").len() == 3` → 3 ✅

## 15.3 — to_int / to_string conversions

In `BEGIN_CALL` handler, name remap:
- `to_int(s)` → `_fj_to_int(s)` → `atoll(s)` cast to int64
- `to_string(n)` → `_fj_to_string(n)` → snprintf to malloc'd buffer

P41: `to_int("42")` → 42 ✅
P42: `strlen(to_string(12345))` → 5 ✅

## 15.4 — `[str]` array push dispatch

`.push(arg)` dispatch in BEGIN_METHOD_CALL handler:
- arg atom is `STR` → `_fj_arr_push_str`
- arg atom is `BEGIN_MACRO_CALL` (concat! returns string) → `_fj_arr_push_str`
- arg atom is `BEGIN_METHOD_CALL` with str-returning method
  (substring, concat) → `_fj_arr_push_str`
- otherwise → `_fj_arr_push_i64`

**Honest gap**: arr[i] indexing always emits `_fj_arr_get_i64`. To
read string elements, fj source must call `_fj_arr_get_str(arr, i)`
explicitly (the underscore-prefixed helper is exposed as a callable
fn). P45 demonstrates this pattern.

True element-type-aware `arr[i]` would require var-type tracking
across the codegen walk. Phase 16 work — that's the prerequisite for
self-compiling stdlib/parser_ast.fj which uses many implicit `[str]`
indexings.

## 15.5 — TYPE marker in BEGIN_LET

parse_let now preserves the type annotation:
- `let x: i64 = 5` → `BEGIN_LET <name> TYPE <"i64"> <expr> END_LET`
- `let arr: [str] = []` → `BEGIN_LET <name> TYPE <"[str]"> <expr> END_LET`
- `let y = 7` (no annotation) → `BEGIN_LET <name> <expr> END_LET` (unchanged)

emit_let prefers declared annotation over atom-based inference:
1. If declared type known via map_type → use it
2. Else fall back to first-atom inference (FLOAT/STR/STRUCT_LIT/ARRAY_LIT/METHOD_CALL/MACRO_CALL)
3. Default int64_t

This makes `let arr: [str] = []` correctly emit `_FjArr* arr =
_fj_arr_new();` (declared type wins) instead of `_FjArr*` defaulting
to `[i64]` semantics.

## 15.6 — Test suite expansion: 40 → 45

```
P41 to_int("42")                          → 42
P42 strlen(to_string(12345))              → 5
P43 concat!("hi ", "world") == "hi world" → 1 (str eq)
P44 strlen(concat!("a","b","c"))          → 3
P45 [str] push + _fj_arr_get_str          → 2 (arr.len after 2 pushes)
```

**45/45 PASS in 0.24s.**

## 15.7 — R14 progress

| Increment | Phase | Status |
|---|---|---|
| String scalars + .substring + ==/!= → strcmp | 13 | ✅ |
| Dynamic [i64] arrays + push + len + index | 14 | ✅ |
| concat! macro + to_int/to_string + [str] basics | 15 | ✅ |
| **PARTIAL [str]**: arr[i] always emits _i64 helper; needs Phase 16 type tracking |  | ⏳ |
| Self-compile of stdlib/parser_ast.fj | 16 | ⏳ |
| Stage 1 == Stage 2 byte-equal | 17 | ⏳ |

## 15.8 — Effort recap

| Sub-item | Plan | Actual |
|---|---|---|
| 15.A concat! macro AST + codegen | 30min | 15min |
| 15.B to_int/to_string runtime + name map | 15min | 5min |
| 15.C unified _FjArr refactor | 30min | 15min |
| 15.D TYPE marker in BEGIN_LET | 30min | 10min |
| 15.E .push dispatch by arg atom | 20min | 10min |
| 15.F 5 new tests | 15min | 10min |
| 15.G findings doc | 30min | 10min |
| **Total** | **~3h** | **~1h** |
| **Variance** | — | **-67%** |

## 15.9 — Honest scope (CLAUDE.md §6.6 R3)

- ✅ `concat!`, `to_int`, `to_string` work end-to-end
- ✅ `[str]` arrays construct correctly with `.push("...")`
- ❌ **`arr[i]` for `[str]` arrays NOT auto-dispatched** — fj source
  must explicitly call `_fj_arr_get_str(arr, i)` to read strings.
  Element-type-aware indexing needs var-type tracking (Phase 16).
- ❌ `arr.push(some_var)` where var is a str-typed binding — push
  arg atom is IDENT, dispatch defaults to _i64 helper. Needs var-type
  tracking too.
- ❌ Memory leaks persist (R15) — _fj_to_string, _fj_concat2, push_str
  all malloc without free. OK for short-lived test programs.
- ❌ `concat!` only handles string args — int-args would emit nested
  _fj_concat2 with int values, generating C type errors. Real fj
  `concat!` handles all-types via Display trait; ours is string-only.
- ❌ Strict aliasing warnings under `-Wstrict-aliasing=2` (gcc default
  doesn't enable). Production code would need union-based casts.

## 15.10 — Cumulative state at v34.3.0

17 self-host phases (0-15) closed; cumulative ~12h Claude time
across v33.4.0..v34.3.0.

| Suite | Tests | Status |
|---|---|---|
| Stage-1 subset | 5 | ✅ |
| Stage-1 full | 45 (5 NEW) | ✅ |
| Stage 2 reproducibility | 6 | ✅ |
| **Total self-host** | **56** | ✅ |

## Decision gate (§6.8 R6)

This file committed → v34.3.0 release commit ready. After v34.3.0,
remaining R14 work requires var-type tracking (Phase 16) before
self-compile of stdlib/parser_ast.fj becomes feasible.

---

*SELFHOST_FJ_PHASE_15_FINDINGS — 2026-05-05. Phase 15 closed in
~1h vs ~3h budget (-67%). concat! macro + to_int/to_string +
unified _FjArr refactor + [str] partial support shipped. R14
progress: 3 of 5 increments closed (Phases 13-15). Phase 16 needs
var-type tracking; arr[i] for [str] is the gating gap.*
