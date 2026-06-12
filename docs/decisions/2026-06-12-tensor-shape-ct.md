# Decision: Tensor Shape Compile-Time — D1/D2/D3 (Compass §6.3 P3 gate)

**Status:** ACCEPTED
**Date:** 2026-06-12
**Decider:** Fajar (approved via session directive "Lanjutkan dari P3" after
recommendations were presented twice: at plan §0 and at P2 closure report)
**Plan:** `docs/TENSOR_SHAPE_CT_PLAN.md` §0 — this file unblocks Phase P3.

## D1 — Runtime checks after static pass: **(a) KEEP**

Runtime shape checks (RE002/TE001-TE009 in runtime/ml) stay exactly as
they are even when the analyzer statically proves a shape. Elision is a
future perf phase; correctness > performance (CLAUDE.md §6.1). The static
layer only ADDS earlier detection, never removes a runtime guard.

## D2 — Dynamic annotation form: **(a) NO new syntax**

Dynamic shapes keep the two existing forms: omitted dims (`tensor`,
`Tensor<f64>` → dims=[], unknown rank) and per-dim wildcard
(`Tensor<f64>[*, 10]`). No `Tensor<f64>[?]` form is added — the wildcard
already expresses per-dim unknowns and B0 P5 proved it parses.

## D3 — Symbolic-dim surface syntax (P3): **(a) uppercase ident in dim slot**

`fn dense(x: Tensor<f64>[B, I], w: Tensor<f64>[I, O]) -> Tensor<f64>[B, O]`

- An identifier starting with an uppercase letter in a dim slot is a
  symbolic dimension, scoped to the enclosing fn signature; no separate
  declaration list (`fn f<[M, K]>` form rejected — needless grammar
  surgery; precedent is the `*` wildcard living in the same slot).
- Same symbol → same size, unified per call site from argument shapes;
  conflict → **TE010 SymbolicDimMismatch** naming the symbol + both sizes.
- Bound symbols substitute into the return shape, propagating concrete
  dims to the caller.
- Dynamic/unknown argument dims bind nothing (gradual — no false errors).
- Inside the fn body, symbolic dims are treated as dynamic in v1
  (intra-body symbol equality is future work, noted in plan §3).
