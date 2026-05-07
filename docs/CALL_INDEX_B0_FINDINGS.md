---
phase: f()[i] / obj.m()[i] indexing — B0 pre-flight audit
plan: docs/CALL_INDEX_PLAN.md §0
status: B0 CLOSED 2026-05-07
artifacts: this doc
purpose: empirical baseline before §1 decisions (D1 AST shape / D2 ret-type lookup / D3 method registry)
prereq: docs/CALL_INDEX_PLAN.md committed (commit 77bda3f0)
---

# CALL_INDEX — B0 Pre-Flight Audit Findings

> Per CLAUDE.md §6.8 R1, every Phase opens with a pre-flight audit
> via runnable commands. This doc records B0.1–B0.4 from
> `docs/CALL_INDEX_PLAN.md` §0 plus a HEADLINE finding that revises
> the plan's assumed failure mode. **No implementation work begins
> until §1 decisions (D1/D2/D3) are committed.**

## ⚠ Headline finding (revises plan §0)

The plan stated `f()[i]` would produce a parser error like
`ERR_PRIMARY` or "expected operator" at the `[`. **That is wrong.**
The actual failure mode is **silent miscompile** — the self-host
chain accepts the source, emits broken C, and produces a binary
that prints a heap-pointer-cast-to-int instead of the expected
array element. The bug is therefore MORE serious than the plan
suggested, and the prevention layer (§3 of the plan) becomes more
important: any future user-extension of self-host source that
indexes a call result will silently corrupt output, not loudly
break.

Specifically: the parser_ast.fj treats `make_arr()[1]` as TWO
adjacent expression statements:

1. `make_arr()` — the call. Parsed correctly.
2. `[1]` — interpreted as an **array literal** (singleton array
   containing the integer 1), emitted as
   `_fj_arr_push_i64(_fj_arr_new(), 1);`. The array is constructed
   then immediately leaked.

The user's `let v = ...` binds `v` to the call result (an `_FjArr*`).
The downstream `println(v)` then passes `_FjArr*` to
`fj_println_int(int64_t)`, gcc warns `[-Wint-conversion]` but
compiles, and the binary prints `(int64_t)heap_pointer`.

This finding is recorded in §B0.2 below with the exact emitted C
and the binary's actual output.

## B0.1 — Build + sanity baseline ✅ CONFIRMED

**Command:**
```
cd "/home/primecore/Documents/Fajar Lang"
ls -la target/release/fj
md5sum target/release/fj
```

**Output:**
```
-rwxrwxr-x 2 primecore primecore 17996392 May  6 19:43 target/release/fj
c24501c8ede673b7bc5001b9afd423c1  target/release/fj
```

**Conclusion:** Stage 0 binary present; md5 captured for any
post-fix regression.

## B0.2 — `f()[i]` (`[i64]` ret) failure mode ⚠ SILENT MISCOMPILE

**Plan-predicted observation:** parser error at `[`.

**Actual observation:** silent miscompile.

### Reproducer

```fj
fn make_arr() -> [i64] { let mut a: [i64] = []; a = a.push(10); a = a.push(20); a = a.push(30); a }
fn main() { let v = make_arr()[1]; println(v) }
```

Fed through the chain (parse_to_ast → emit_program of concatenated
`stdlib/{codegen,parser_ast,codegen_driver}.fj`).

### Emitted C (relevant excerpt)

```c
int main(int argc, char** argv) {
    g_fj_argc = argc;
    g_fj_argv = argv;
    atexit(_fj_arena_free_all);
    _FjArr* v = make_arr();              // call OK; v is the WHOLE array
    _fj_arr_push_i64(_fj_arr_new(), 1);  // [1] parsed as a STANDALONE array literal!
    fj_println_int(v);                   // printing _FjArr* through int64_t signature
}
```

### gcc warning (real, but non-fatal)

```
chain_out.c:139:20: warning: passing argument 1 of 'fj_println_int' makes integer
from pointer without a cast [-Wint-conversion]
  139 |     fj_println_int(v);
      |                    ^
      |                    |
      |                    _FjArr *
```

### Binary output

```
$ /tmp/chain_bin
103218759844512        ← heap pointer cast to int64_t
$ echo $?
0
```

### Compare to interpreter (NOT chain)

```
$ ./target/release/fj run /tmp/b0_call_index_v2.fj
20                     ← correct
```

**Conclusion:** the **interpreter's Rust parser** at `src/parser/`
already handles `f()[i]` correctly via Pratt-style postfix chaining.
The **self-host parser** at `stdlib/parser_ast.fj` does NOT, but
fails by silently splitting the expression rather than erroring out.

## B0.3 — Method registry: any [str]-returning today? ✅ NONE (matches plan)

**Command:** `grep -n 'fn map_method\|method == "' stdlib/codegen_driver.fj`

**Output:**
```
179:        if method == "push" {
493:fn map_method(method: str) -> str {
494:    if method == "substring" { return "_fj_substring" }
495:    if method == "push" { return "_fj_arr_push_i64" }
496:    if method == "len" { return "_fj_arr_len" }
497:    if method == "join" { return "_fj_arr_join_str" }
```

| Method | Lowering | Ret-type |
|---|---|---|
| substring | `_fj_substring` | `const char*` (str) |
| push | `_fj_arr_push_*` | `_FjArr*` |
| len | `_fj_arr_len` | `int64_t` (i64) |
| join | `_fj_arr_join_str` | `const char*` (str) |

**Conclusion:** **0 methods return `[str]`.** D3 (method ret-type
registry) is **infrastructure for FUTURE methods**, not for
fixing a current user-visible bug. The user-visible
`f()[i]` (call form) is what surfaces today; `obj.m()[i]` becomes
relevant only when a `[str]`-returning method is added.

## B0.4 — Codegen dispatch trace on synthesized AST ✅ CONFIRMED

For the AST shape the plan posits (`BEGIN_INDEX <BEGIN_CALL make_arr
END_CALL> <INT 1> END_INDEX`), walk codegen_driver.fj L139–L154:

1. `let arr_name = ast[pos + 1]` → would be the literal string
   `"BEGIN_CALL"` (the AST tag), not a fn name.
2. `lookup_var_type_in_table(vars, "BEGIN_CALL")` → returns `""`
   (no var named "BEGIN_CALL").
3. `let helper = if arr_type == "[str]" { ... } else { "_fj_arr_get_i64" }`
   → defaults to `_fj_arr_get_i64`.
4. `subj` ends up as the literal string `"BEGIN_CALL"`, emitted as
   the C identifier `BEGIN_CALL` — undeclared.
5. emitted C: `_fj_arr_get_i64(BEGIN_CALL, ...)` → would fail
   compile.

**Conclusion:** even if the parser produced a `BEGIN_INDEX` with
complex subject, the codegen dispatch would fail to compile. **Both
parser AND codegen MUST be touched** to close the gap. Confirms
plan §2 phasing (P1 parser surgery + P2 codegen subject
generalization).

## Summary table

| ID | Check | Status | Notes |
|---|---|---|---|
| B0.1 | Build sanity | ✅ confirmed | md5 c24501c8... |
| B0.2 | Parser failure mode | ⚠ revised | silent miscompile, NOT parser error (see headline) |
| B0.3 | Method [str]-returners | ✅ confirmed | 0 today; D3 is future-wiring |
| B0.4 | Codegen dispatch trace | ✅ confirmed | both layers need touch |

## Decision-gate inputs (for §1)

The B0 audit confirms:

1. **Bug exists** but is **silent miscompile**, not parser error
   (B0.2 headline). Severity recalibrated UP.
2. **Both layers (parser + codegen) need work** (B0.4). Plan §2
   phasing P1 + P2 is correct.
3. **D3 (method registry) is infrastructure**, not blocker (B0.3).
   Could defer — but trivial wire-up if A or B is implemented.
4. **Interpreter path correct already** (B0.2 compare). Means user
   programs run via `fj run` are unaffected; only self-host
   extensions are at risk. This narrows the user-impact surface
   (good) but does not lower the importance of the fix (since
   self-host is a stated goal of the project).

## Plan amendments suggested

Based on B0.2 revising the failure mode, when the user authors
the §1 decision file, they should consider:

- **D1 AST shape** preference is unchanged (D1.A reuse-BEGIN_INDEX
  remains the recommendation).
- **D2 ret-type lookup** preference is unchanged (D2.A peek-subject
  for immediate fix; D2.B typed parse_expr_emit on Phase 19+
  roadmap).
- **D3 method registry**: given B0.3 (no [str] methods today),
  D3 wiring CAN be deferred to a follow-on phase. Or kept in scope
  as a forward investment. **User to decide.**
- **§3 prevention layer**: regression tests P81+ should explicitly
  assert binary output value (not just exit code), so the silent
  miscompile case is caught even when gcc only warns.
- **§5 budget**: unchanged. The fix scope is the same; only the
  failure-mode description in the plan needs updating to match
  reality.

## Next step

Per §6.8 R6: commit `docs/decisions/2026-05-07-call-index-shape.md`
with sections for D1, D2, D3 choices. Until that file exists, no
§2 implementation work starts.

---

*CALL_INDEX_B0_FINDINGS — 2026-05-07. B0 closed; failure mode
revised to silent miscompile. D1/D2/D3 unblocked.*
