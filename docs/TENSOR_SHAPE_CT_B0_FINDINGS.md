# Tensor Shape Compile-Time (Compass §6.3) — B0 Pre-Flight Findings

> **Date:** 2026-06-12 (session post-HONEST_AUDIT_V36)
> **Rule:** CLAUDE.md §6.8 R1 — hands-on baseline verification before any plan.
> **HEAD:** `770d2b10` (v36.0.0+8). Probes run against `target/release/fj`.
> **Companion plan:** `docs/TENSOR_SHAPE_CT_PLAN.md`.

## §1. Why this B0 exists

Compass §6.3 names compile-time tensor shape checking as a core deepening
target ("shape error di runtime adalah bug fatal" for embedded ML), and
CLAUDE.md §1 design principle 4 already CLAIMS "shape checked at compile
time". This B0 establishes what is actually true at HEAD.

## §2. Probes (all runnable; exact files inline)

### P1 — un-annotated matmul mismatch: `fj check` MISSES it ❌

```fajar
@device fn bad_matmul() {
    let a = zeros(2, 3)
    let b = zeros(4, 5)
    let c = matmul(a, b)
}
fn main() { bad_matmul() }
```
```
fj check  → "OK — no errors found" (exit 0)        ← THE GAP
fj run    → RE002: type error: TE002: matmul shape mismatch:
            [2, 3] @ [4, 5] — inner dims 3 != 4    ← runtime only
```

### P2 — `Tensor<f32, [2, 3]>` (dims-inside-generics form): NOT the syntax

`PE003 expected type at the '['`. **Not a gap** — the language already has
a different, working form (P3/P5). Any future docs must use the existing
form `Tensor<elem>[dims]`.

### P3 — element type at param boundary: ENFORCED ✓

`fn take(t: Tensor<f32>)` called with `zeros(2,2)` (f64 default) →
`SE004: expected Tensor, found Tensor<f64>[]` at `fj check`.

### P4 — shape at param boundary vs dynamic arg: NOT enforced ❌

`fn take(t: Tensor<f64>[3, 3])` called with `zeros(2, 2)` →
`fj check` OK. Root cause is P-root below (the arg types as dims=[] which
unifies with everything), not the boundary check itself — see P6.

### P5 — annotated `@` matmul mismatch: ENFORCED ✓

```fajar
let a: Tensor<f64>[2, 3] = zeros(2, 3)
let b: Tensor<f64>[4, 5] = zeros(4, 5)
let c = a @ b
```
→ `TE001: tensor shape mismatch: matmul: [2, 3] × [4, 5]` at `fj check`,
with span pointing at `a @ b`. (TE001 here is a **SemanticError**, distinct
from the runtime TE001/TE002 in `runtime/ml`.)

### P6 — known-vs-known at assignment: ENFORCED ✓

`let b: Tensor<f64>[9, 9] = a` where `a: Tensor<f64>[2, 3]` →
`SE004: expected Tensor<f64>[9, 9], found Tensor<f64>[2, 3]`.

## §3. Existing machinery inventory (all live at HEAD)

| Component | Location | State |
|---|---|---|
| `Type::Tensor { element, dims: Vec<Option<_>> }` | `src/analyzer/type_check/mod.rs:104` region | per-dim known/dynamic; `dims=[]` = unknown rank |
| `matmul_shape()` `[M,K]×[K,N]→[M,N]` + `elementwise_shape()` | same file ~330-380 | working, used by P5 path |
| `verify::tensor_verify::{SymbolicShape, verify_matmul, ShapeCheckStatus}` | `src/verify/tensor_verify.rs` | **survived the EOS-37 verify/SMT freeze**; wired into `@` path for richer messages |
| `TypeExpr::Tensor` syntax `Tensor<f32>[3, 4]` + wildcard `Tensor<f64>[*, 10]` | `src/parser/ast.rs:1423`, parsed at `src/parser/expr.rs:1743` | parses in type position (let/param/return) |
| `@` (BinOp::MatMul) static check | `src/analyzer/type_check/check.rs:1402-1455` | fires ONLY when both sides have non-empty dims |
| Analyzer `SemanticError::TensorShapeMismatch` (TE001-class compile error) | `type_check/mod.rs:996` | exists + tested (B.1/B.4 test sections ~3996-4360) |
| `src/dependent/{mod,nat}.rs` | type-level naturals remnant post-freeze | available; `tensor_shapes.rs` was REMOVED at EOS-37, not frozen in place |

## §4. Root cause (single load-bearing gap)

**All tensor builtins are registered with `dynamic_tensor()` (dims=[]).**
`src/analyzer/type_check/register.rs:945`:
`("matmul", vec![dyn_t, dyn_t], dyn_t)` — and `zeros/ones/randn/...`
likewise return dyn. Because `dims=[]` unifies with every shape, every
un-annotated value is invisible to the (otherwise working) checker, and
P1/P4 sail through. The inference engine is sound but starved at the
front door.

Secondary gaps:
- **G2:** `matmul(a, b)` builtin-CALL form bypasses the `@`-operator
  static path entirely (name-table dyn signature) — P1 used the call form.
- **G3:** no symbolic dims at fn boundaries — `Tensor<f64>[*, 10]`
  wildcard exists per-dim, but two `*`s are unrelated; there is no way to
  express "inner dims must MATCH" (`[M,K] × [K,N]`) across params.
  `SymbolicShape` in tensor_verify already models symbols — unused surface.
- **G4:** doc drift — CLAUDE.md §1 claims compile-time shape checking
  unqualified; true only for the annotated-`@` slice (P5/P6).

## §5. Verdict

The enhancement is **wiring, not architecture**: fold constructor literal
args into `dims`, shape-type the ~10 tensor builtins, enforce
known-vs-known at call boundaries (falls out of P1 fix + existing P6
machinery), then add symbolic dims as the differentiator stage. No new
type-system machinery needed until the symbolic stage, which reuses
`SymbolicShape`.

## §6. §6.8 self-check

R1 hands-on probes ✓ (6 probes, verbatim) · R2 runnable commands ✓ ·
R4 no agent numbers (all first-hand) ✓ · R6 decisions deferred to plan's
decision gates ✓.
