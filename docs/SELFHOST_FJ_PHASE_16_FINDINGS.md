---
phase: 16 — fj-source helpers compile through chain (R14 fourth increment + var-type maturation)
status: CLOSED 2026-05-05
budget: ~3-4h realistic
actual: ~3h Claude time across 7 sub-tags (v34.5.0..v34.5.6)
variance: in budget
tags:
  - v34.5.0 — Pratt precedence + parens + first parser_ast helpers compile (~1.5h)
  - v34.5.1 — to_int smart dispatch + 3 more parser_ast helpers (~15min)
  - v34.5.2 — implicit return from expression body (~20min, -33%)
  - v34.5.3 — struct-typed fn signatures (reprioritized) (~50min, -67%)
  - v34.5.4 — [T] in struct/fn-ret + IDENT-rebind + free len() (~30min)
  - v34.5.5 — chained method calls in assignment (~50min)
  - v34.5.6 — string escape preservation (test driver fix) (~25min)
artifacts:
  - This findings doc
  - stdlib/codegen.fj +Pratt parser (6 levels), unary prefix, ARG_END markers
  - stdlib/codegen.fj +CodegenState.struct_names + .fn_ret_types fields, map_type_ctx helper
  - stdlib/codegen.fj +emit_fn implicit-return scan, [T] type accept in struct/fn-ret
  - stdlib/codegen.fj +chained .push().push() lookahead + nested BEGIN_METHOD_CALL emit
  - stdlib/parser_ast.fj +pratt parse_expr, parse_primary unary handling
  - tests/selfhost_stage1_full.rs: 53 → 70 (P54-P70, 17 NEW)
prereq: v34.4.0 (R12 closure + unary prefix); v34.3.1 (Phase 15 var-type tracking)
---

# fj-lang Self-Hosting — Phase 16 Findings

> **fj-source helpers from `stdlib/parser_ast.fj` now compile end-to-end
> through the chain.** Phase 16 is the bridge between "fj compiles
> small standalone test programs" and "fj compiles its own compiler
> source." It does NOT yet self-compile parser_ast.fj/codegen.fj as a
> whole — that is Phase 17. What Phase 16 proves is the **expression
> grammar + type-tracking is mature enough that helper-by-helper
> compile-through-chain works** for non-trivial fj functions like
> `is_digit_ast`, `skip_spaces`, `read_word`, `read_int_at`.

## 16.1 — v34.5.0: Pratt precedence (the unblocker)

### What broke

Phase 15 had right-associative parsing with a single precedence level.
`2 + 3 * 4` evaluated to `(2+3)*4 = 20` instead of `2 + (3*4) = 14`.
Once parser_ast helpers like `is_alnum_ast(c) = is_alpha_ast(c) ||
is_digit_ast(c)` came on, mixed-precedence expressions surfaced the
wrong-tree pervasively.

### The fix

Stack-based Pratt parser with 6 precedence levels (lowest→highest):

```
||
&&
== !=
< <= > >=
+ -
* / %
```

Implementation via `parse_expr_emit` walks atoms and uses a precedence
stack. Parenthesized expressions `(expr)` integrated as a primary atom.

### Side effect: ARG_END markers

Pratt walker can't tell where one expression ends and the next begins
inside `f(a, b, c)` or `[a, b, c]` purely from token shape — what
looks like a binop continuation might actually be a comma boundary.
Solution: parser inserts `ARG_END` markers between consecutive
expressions in:

- `BEGIN_CALL` arg list
- `MACRO_CALL` arg list
- `METHOD_CALL` arg list
- `ARRAY_LIT` element list
- `ARM` (pat → body)

Codegen consumes `ARG_END` to know when to emit `,` separator.

### String ordering lowering

`<`, `<=`, `>`, `>=` between strings now lower to `strcmp(a, b) OP 0`
instead of pointer comparison. Equality (`==`, `!=`) was already
handled in Phase 13; this closes ordering.

### Iteration record

Pratt landed → broke args → ARG_END inserted → match arms broke →
ARG_END for pat/body → green. 4 commits to land cleanly. Documented
under §6.8 R5 surprise-budget tracking (+50% est).

### Tests

P54 `(2+3)*4 == 20`. P55 `2+3*4 == 14`. P56 `(1<2) && (3>2)`.
**P57 headline: `is_digit_ast`/`is_alpha_ast`/`is_alnum_ast` from
stdlib/parser_ast.fj compile through chain** — first time fj-source
compiler-helper code goes through chain successfully.

## 16.2 — v34.5.1: to_int smart dispatch (silent miscompile fix)

### What broke

`to_int(x)` where `x` is `size_t` from `strlen(s)` emitted
`_fj_to_int(strlen(s))` which calls `atoll((const char*) <int>)` —
pointer-cast on an integer, undefined behavior.

### The fix

`to_int` arg is dispatched by atom shape:
- str arg (literal, IDENT-of-str-typed-var, str-returning method) →
  `_fj_to_int(s)` → `atoll`
- numeric arg (int, size_t, IDENT-of-int-typed-var) → `(int64_t)
  arg` cast

### Tests

P58: `skip_spaces` + `read_word` + `read_int_at` from parser_ast.fj
compile via chain. 3 helper fns added to the "compiles cleanly"
roster.

## 16.3 — v34.5.2: implicit return from expression body

### What broke

`fn add(a: i64, b: i64) -> i64 { a + b }` (no explicit `return`)
emitted `int64_t add(int64_t a, int64_t b) { a + b; }` — fell off the
end without returning. C99 says fall-off `int main` is implicit
`return 0`, but every other non-void function falling off is UB.

### The fix

`emit_fn` pre-scans the body in a single pass before emission. If
ret_type ≠ "void" AND last stmt is `BEGIN_EXPR_STMT`, emit `return
<expr>;` instead of bare `<expr>;`.

Helper: `emit_fn_implicit_return` (later renamed
`emit_if_implicit_return` in v35.0.0 when else-if recursion was added).

### Tests

P59 `fn id(x){x} → id(7) == 7`. P60 `fn add(a,b){a+b} → add(3,4) == 7`.
P61 nested fn calls.

## 16.4 — v34.5.3: struct-typed fn signatures (reprioritized)

### Pre-flight audit (CLAUDE.md §6.8 R1)

Original Phase 16 sub-task 2 was "match-with-payload extraction." A
runnable audit (`grep -nE "match.*=>" stdlib/parser_ast.fj`) showed
parser_ast.fj uses **if/else chains over enum tags, NOT match with
payload**. Reprioritized in real-time to "struct-typed fn signatures"
which IS a real blocker.

This is exactly the §6.8 R1 mechanism working — the original plan
assumed a feature parser_ast.fj didn't actually need.

### What broke

`fn make_point() -> Point { Point{x:1, y:2} }` emitted
`Point make_point() { ... }` — but `Point` was a typedef-name
forward-declared elsewhere; codegen didn't carry the struct/fn-ret
type registry through the walk.

### The fix

`CodegenState` (the runtime walker state) gains two fields:
- `struct_names: [str]` — populated in pre-pass over AST
- `fn_ret_types: [str]` — `name → ret_type` lookup

New helpers: `add_struct_name`, `is_struct_name`, `add_fn_ret_type`,
`lookup_fn_ret_type`, `map_type_ctx`.

`emit_function_typed` and struct field emission now use
`map_type_ctx` instead of bare `map_type` — the contextual variant
checks struct_names registry before defaulting to int64_t. `BEGIN_LET`
gains a `BEGIN_CALL` branch that calls `lookup_fn_ret_type` for
auto-typing assignments from fn calls.

### Tests

P62 `let p = make_point()` infers `Point*`. P63 struct param.
P64 struct-returning fn call inside expression.

### Variance

Estimated 2-3h, actual 50min (-67%). Recorded per §6.8 R5.

## 16.5 — v34.5.4: [T] in struct/fn-ret types

### What broke

`parse_struct_ast` and `parse_fn_ast` (in fj-source) accepted scalar
field/return types but not `[T]` array types, so structs with
`lines: [str]` field couldn't be declared in fj source compiled
through the chain.

### The fix

- `parse_struct_ast` accepts `[T]` field types (depth-tracking like
  `parse_params`)
- `parse_fn_ast` accepts `[T]` return types
- `BEGIN_LET` IDENT-type inference for rebind-via-alias: `let mut a =
  v` where `v` is `[T]`-typed inherits v's fj-type
- Free `len(arr)` on `[T]`-typed IDENT lowers to `_fj_arr_len(arr)`
  (not `strlen` which is for str)

### Tests

P65 struct with `[i64]` field. P66 fn returning `[str]`.

## 16.6 — v34.5.5: chained method calls in assignment

### What broke

`a = a.push("X").push("Y")` — assignment RHS with method-chain.
Existing parser handled method calls but not chained-in-assignment
because `count_method_chain_after` lookahead was missing.

### The fix

- `count_method_chain_after`: lookahead helper that counts how many
  `.method(...)` continuations follow a primary
- Nested `BEGIN_METHOD_CALL` emit
- Codegen subject via `parse_expr_emit` (recursive)
- Depth-aware `find_method_name` that matches `_fj_arr_push_str`
  pattern at the right depth

`a = a.push("X").push("Y")` lowers to:

```c
a = _fj_arr_push_str(_fj_arr_push_str(a, "X"), "Y");
```

### Tests

P67 2-deep chain. P68 3-deep chain.

## 16.7 — v34.5.6: string escape preservation (test driver fix)

### What broke

P57's str-output P-tests started failing with corrupt output bytes
when newlines/tabs were embedded in fj string literals.

### Pre-flight audit (§6.8 R1)

Audit asked: "is the chain itself losing escapes, or is the test
driver?" Targeted grep showed the chain emits string literals
verbatim and gcc handles escapes correctly. The bug was in the
**test harness** `compile_subset_program` helper which double-escaped
backslashes/control bytes when assembling the inline fj source.

Important lesson: not every test failure is a chain bug. The harness
is part of the audit surface.

### The fix

`compile_subset_program` properly encodes raw bytes when constructing
inline fj source. Chain itself unchanged.

### Tests

P69 newline in str literal. P70 tab in str literal.

**Phase 16 closed at v34.5.6.**

## 16.8 — Test suite expansion: 53 → 70

```
P54 (2+3)*4 == 20                                  | Pratt parens
P55 2+3*4   == 14                                  | Pratt precedence
P56 (1<2) && (3>2) == 1                            | && / parens
P57 is_digit_ast/is_alpha_ast/is_alnum_ast compile | parser_ast helpers (HEADLINE)
P58 skip_spaces / read_word / read_int_at compile  | scanning helpers
P59 fn id(x){x} → id(7) == 7                       | implicit return
P60 fn add(a,b){a+b} → add(3,4) == 7               | implicit return binop
P61 nested fn calls (implicit return)              | implicit return chained
P62 let p = make_point() → struct typing inferred  | struct fn-ret
P63 struct-typed fn parameter                      | struct sig param
P64 struct-returning fn call inside expr           | struct sig nested
P65 struct with [i64] field                        | [T] field
P66 fn returning [str]                             | [T] return
P67 chained .push().push() (2-deep)                | chained methods
P68 chained .push().push().push() (3-deep)         | chained methods
P69 str literal containing newline                 | escape preserve
P70 str literal containing tab                     | escape preserve
```

**70/70 PASS** in `tests/selfhost_stage1_full.rs` (165 fns total at
phase close — P1..P70 = full Stage-1 suite, plus helper / driver fns).

## 16.9 — Honest scope at Phase 16 close (CLAUDE.md §6.6 R3)

What works:
- ✅ Pratt parser handles standard arithmetic + boolean precedence
- ✅ Parens `(expr)` as primary
- ✅ Unary prefix `-x`, `!x` (closed in v34.4.0)
- ✅ String ordering via strcmp
- ✅ Implicit return from fn-with-expression-body
- ✅ Struct types in fn signatures + bodies
- ✅ `[T]` types in struct fields + fn return
- ✅ Chained method calls in assignment
- ✅ String escape pass-through
- ✅ `to_int` dispatched by arg kind (no silent pointer-cast)

What does NOT work yet at Phase 16 close (becomes Phase 17 scope):
- ❌ Full self-compile of parser_ast.fj — many fns work helper-by-helper
  but the whole file together has additional bugs (depth-counter STR
  values, pre-emission passes recursing into fn bodies, if-as-expr
  with else-if, field-then-index `s.lines[i]`)
- ❌ Self-compile of codegen.fj — substring byte-indexing on Unicode
  dividers (`═` in the source file headers), block-with-trailing-expr
  in if-expr
- ❌ Stage 2 binary triple-test — needs argv/read_file/write_file
  C-level glue + main() lowering with argc/argv
- ❌ R15 memory leak class — every malloc'd helper still leaks

What stays deferred (genuinely separate scope):
- ⏸️ Match expression with payload extraction (§6.8 R1 audit said
  parser_ast doesn't need it; if/else over enum tag works)
- ⏸️ Generics, closures with capture, async, lifetimes (Subset-excluded)
- ⏸️ Bounds checking on `_FjArr` access

## 16.10 — Effort recap

| Sub-tag | Phase 16 sub-item | Plan | Actual |
|---|---|---|---|
| v34.5.0 | Pratt + parens + ARG_END | 1-2h | ~1.5h |
| v34.5.1 | to_int smart dispatch | 30min | 15min |
| v34.5.2 | implicit return | 30min | 20min |
| v34.5.3 | struct-typed fn sigs (reprioritized) | 2-3h | 50min (-67%) |
| v34.5.4 | [T] in struct/fn-ret | 1h | 30min |
| v34.5.5 | chained method calls | 1h | 50min |
| v34.5.6 | string escape (driver fix) | 30min | 25min |
| Findings doc | This doc | 1h | written 2026-05-06 |
| **Total** | — | **~7-9h** | **~3h** |
| **Variance** | — | — | **-65%** |

## 16.11 — Cumulative state at v34.5.6

20 self-host phases (0-16) closed; cumulative ~15h Claude time across
v33.4.0..v34.5.6.

| Suite | Tests | Status |
|---|---|---|
| Stage-1 subset | 5 | ✅ |
| Stage-1 full | 70 (17 NEW from Phase 16) | ✅ |
| Stage 2 reproducibility | 6 | ✅ |
| **Total self-host** | **81** | ✅ |

## Decision gate (§6.8 R6)

Phase 16 closed → v34.5.6 release shipped. After Phase 16, the gating
work is **whole-file self-compile** of parser_ast.fj (Phase 17.A) and
codegen.fj (Phase 17.B), then **Stage 2 native triple-test** with the
fjc binary applied to its own combined source (Phase 17.C, the
fixed-point milestone).

---

*SELFHOST_FJ_PHASE_16_FINDINGS — written 2026-05-06 (audit-trail
catch-up; closes CLAUDE.md §6.8 R1 gap for v34.5.0..v34.5.6). Phase 16
closed across 7 sub-tags in ~3h actual / ~7-9h budget (-65%). 17 new
tests P54-P70 pass; first proof that fj-source compiler-helper code
compiles through the chain. Phase 17 then takes the next step — full
self-compile + Stage 2 fixed point.*
