# S2.6 — Closure-with-Capture as Function Argument: Closure Findings

> **Date:** 2026-06-12 (same session as Compass §6.3 P1-P4 closure)
> **Origin:** `#[ignore = "...requires trampoline (deferred to S2.6)"]` at
> `src/codegen/cranelift/tests.rs` + `examples/aspirational/README.md` row.
> **Status:** CLOSED — the ignored test passes; 7,791 native lib tests green.

## The gap

Cranelift lambda-lifts closures (captures prepended as params). A closure
WITH captures stored in a variable becomes a runtime `ClosureHandle`
(fn_ptr + capture snapshot) — but a fn-ptr-typed parameter call site
(`f(x)` inside `fn apply(f: fn(i64) -> i64, ...)`) compiled a bare
`call_indirect`, so passing a capturing closure executed the HANDLE as
code → SIGSEGV (reproduced as the RED phase of TDD).

## Design chosen: tagged handle + dynamic dispatch (not a trampoline)

Per-instance trampolines need runtime code emission — impossible on the
AOT path. Uniform "everything is a handle" conversion was rejected after
B0 showed 6 more raw-address consumers (map/filter/fold, method-position
indirect calls, thread::spawn) — unacceptable blast radius for one work
item. Instead:

1. `fj_rt_closure_new` returns a **tagged** pointer (`CLOSURE_TAG`);
   every `fj_rt_closure_*` helper untags on entry, so the tagged value is
   the canonical handle representation everywhere.
2. New `fj_rt_closure_call_dyn_{0,1,2}(target, args...)`: tag bit set →
   unpack handle and call body with captures prepended; clear → call
   `target` as a raw function address. Declared in BOTH JIT and AOT
   module paths.
3. `compile_fn_ptr_call` dispatches through `__closure_call_dyn_N` when
   the signature is all-int/pointer with ≤2 args; other signatures keep
   the old direct `call_indirect` (pre-S2.6 parity).

So ONE call site accepts both `apply(double, 10)` (plain fn) and
`apply(add_offset, 20)` (capturing closure) — verified by the new
`native_closure_as_arg_mixed_plain_and_capture` test.

## Assumption disproven by testing (recorded per §6.8 R4 spirit)

First implementation tagged **bit 0**, assuming code addresses are
≥2-aligned. The no-capture regression test immediately disproved this:
**Cranelift JIT does not align function entries on x86-64** (an odd
function address was observed), so a raw fn address collided with the
tag. Fix: tag **bit 62** (never set in canonical userspace VAs on
x86-64/aarch64, heap and code alike; bit 30 on 32-bit targets) +
`debug_assert` at handle creation. Lesson: alignment guarantees are ISA-
and-allocator folklore until a test proves them.

## Honest scope line (unchanged semantics elsewhere)

- Capturing closures into `map/filter/fold`, method-position fn args, and
  `thread::spawn` still take the raw-address path (pre-existing behavior,
  out of S2.6 scope).
- `call_dyn` covers 0-2 user args (parity with the existing handle-call
  max); >2-arg or float-signature fn-ptr params keep direct
  `call_indirect`.
- Handle allocations still lean on process teardown when not explicitly
  freed (pre-existing parity).

## Gates at close

```
cargo test --features native --lib                 7,791 PASS / 0 FAIL / 0 ignored
                                                   (the S2.6 #[ignore] is gone; +2 new tests)
cargo test --features native --lib --test-threads=64   PASS
cargo test --lib (default)                         6,616 PASS (cranelift gated — untouched)
stress ×5 (default)                                5/5
cargo clippy --features native --all-targets       exit 0
cargo fmt -- --check                               exit 0
```

TDD trail: RED = SIGSEGV on un-ignored test → bit-0 tag → RED #2 =
misaligned-deref panic (assumption disproven) → bit-62 tag → GREEN, full
native suite clean on first complete run.
