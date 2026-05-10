# @KERNEL MODE B0 — current state + Compass §4.4 default-safe gap

**Date:** 2026-05-10 (EOS-27, 4th lanjut)
**Status:** B0 CLOSED — recommendation table at §6
**Trigger:** Strategic next-pickup after v35.5.0 D-FULL ship
**Methodology:** Plan Hygiene §6.8 R1 (B0 is mandatory pre-flight)
**Author:** Claude session-end audit

---

## TL;DR

@kernel/@device/@safe **enforcement is substantially built and battle-tested**
— 4 enforcement layers (analyzer, Cranelift codegen, self-host analyzer_fj,
LSP), 3 KE error variants, ~50+ tracked builtin names, 148/148 passing
context-safety tests. The "next strategic chapter" framing
overestimated the open work.

**The one real gap surfaced by this B0 is Compass §4.4
"@safe sebagai default" at the *context* level (not affine — that
shipped in v35.5.0 D-FULL):**

```rust
// src/analyzer/type_check/check.rs:160
_ => crate::analyzer::scope::ScopeKind::Function,  // ← should be ::Safe
```

Unannotated functions get `ScopeKind::Function`, which is permissive —
they can call `port_outb`, `zeros()`, `irq_register`, etc. without
SE020/KE002 firing. The fix is conceptually one line; the cascade
(stdlib/: 180 unannotated vs 79 annotated; ~70% unannotated) is large
enough to make this a Phase-2-class migration, not a one-line ship.

---

## §1. What's already enforced (verified hands-on at v35.5.0)

### 1.1 Error codes wired end-to-end

| Code | Variant | Emission site | Coverage |
|------|---------|---------------|----------|
| **KE001** HeapAllocInKernel | `analyzer/type_check/mod.rs:779` | `check.rs:1872, 1884` | 11 heap_builtins (`push`, `pop`, `to_string`, `map_*`) + transitive via `heap_tainted_fns` |
| **KE002** TensorInKernel | `mod.rs:786` | `check.rs:1876, 1880` | 40+ tensor_builtins (`zeros`, `ones`, `relu`, `matmul`, …) + transitive via `tensor_tainted_fns` |
| **KE003** DeviceCallInKernel | `mod.rs:793` | `check.rs:1868` | call-site check against `device_fns` set |
| **DE002** KernelCallInDevice | (variant) | `check.rs:1916` | call-site check against `kernel_fns` set |

LSP (`server.rs:2670`), self-host analyzer_fj (`selfhost/analyzer_fj.rs:427`),
and Cranelift codegen (`codegen/cranelift/mod.rs:349`) all map / re-emit
these codes. Multi-layer redundancy.

### 1.2 Context-tracking pipeline

```
parser/mod.rs:746   parse @kernel/@device/@safe/@unsafe/@gpu/@npu/@ffi
       │
       ▼
parser/ast.rs:231   Annotation field on FnDef
       │
       ▼
analyzer/type_check/check.rs:148-161
       fn_def.annotation → ScopeKind::{Kernel, Device, Safe, Unsafe, Gpu, Npu}
       (NO annotation → ScopeKind::Function)  ◀── §4.4 GAP
       │
       ▼
analyzer/scope.rs:208-237
       is_inside_kernel() / is_inside_device() / is_inside_safe()
       │
       ▼
analyzer/type_check/check.rs:1865-1980
       Per call-site lookup against {heap, tensor, os}_builtins +
       {kernel, device}_fns + transitive {heap, tensor}_tainted_fns
```

### 1.3 Builtin coverage sets (verified by source read at `mod.rs:1721`)

| Set | Count | Examples | Notes |
|-----|-------|----------|-------|
| `heap_builtins` | **11** | push, pop, to_string, map_insert/get/get_or/remove/contains/keys/values/len | Likely under-tagged (no string-concat, no `format!`-equiv); audit recommended |
| `tensor_builtins` | **40+** | tensor_zeros..tensor_xavier; short aliases (zeros, ones, randn, relu, sigmoid, softmax, mse_loss, quantize_int8, ...) | Looks complete |
| `os_builtins` | **45+** | mem_*, page_*, irq_*, port_*, syscall_*, x86 cpuid/cr0/cr4, idt_init, pic_*, pit_*, kb_*, proc_*, str_byte_at | Comprehensive |

The Cranelift-codegen list (`codegen/cranelift/mod.rs:349-400`) is a
**smaller, partially-overlapping subset** (`String_new`, `Vec_new`,
`read_file`, `write_file`, `append_file` for HEAP_OPS; ~17 tensor names
for TENSOR_OPS). Analyzer is the canonical authority; Cranelift's list
is best-effort defense-in-depth.

### 1.4 Test coverage (verified by `cargo test --release --test context_safety_tests`)

| Test file | Tests | KE / context refs |
|-----------|-------|-------------------|
| `tests/context_safety_tests.rs` | **148** PASS @ 0.07s | 40 KE-related |
| `tests/safety_tests.rs` | 96 | 3 KE refs |
| `tests/error_code_coverage.rs` | 106 | 11 KE refs |
| `tests/eval_tests.rs` | 957 | 1 KE ref |

`context_safety_tests.rs` is the canonical test surface for context
isolation. It tests SE020 (hw access in @safe), KE001/KE002/KE003
(kernel violations), DE001/DE002 (device violations), and cross-context
call propagation. Zero failures at v35.5.0.

### 1.5 Smoke probes (run at v35.5.0 HEAD)

| Probe | Expected | Got | Verdict |
|-------|----------|-----|---------|
| `@kernel fn { let mut a:[i32]=[]; a.push(1) }` | KE001 | KE001 | ✅ |
| `@kernel fn { let _y = to_string(42) }` | KE001 | KE001 | ✅ |
| `@kernel fn { let _t = zeros(3,3) }` | KE002 | KE002 | ✅ |
| `@kernel fn { let _y = "h".to_string() }` (with `_y`) | KE001 | KE001 | ✅ |
| `@safe fn f() { port_outb(0x3F8, 65) }` | SE020 | SE020 (in test suite) | ✅ |
| `fn no_ann() { let _t = zeros(3,3) }` | (per Compass §4.4 should fire) | **OK no errors** | ❌ §4.4 gap |
| `fn no_ann() { port_outb(0x3F8, 65) }` | (per Compass §4.4 should fire) | **OK no errors** | ❌ §4.4 gap |

---

## §2. The §4.4 default-safe gap (the actual finding)

### 2.1 Stated intent

| Source | Says |
|--------|------|
| `STRATEGIC_COMPASS.md:298–304` | "Semua function tanpa annotation otomatis `@safe`. Domain checking aktif tanpa annotation eksplisit. Naik ke `@kernel`/`@device` adalah opt-in." |
| `CLAUDE.md §5.3` table | `@safe` row blocks `String::new()`, hardware ops, raw pointer deref, etc. |
| `analyzer/scope.rs:235` (comment) | "Functions annotated with @safe **or implicitly safe (no annotation)**." |
| `analyzer/effects.rs:838` (comment) | "`@safe` — **default**, most restrictive." |

### 2.2 Actual code

```rust
// src/analyzer/type_check/check.rs:148
let scope_kind = match &fndef.annotation {
    Some(ann) if ann.name == "kernel" => ScopeKind::Kernel,
    Some(ann) if ann.name == "device" => ScopeKind::Device,
    Some(ann) if ann.name == "npu" => ScopeKind::Npu,
    Some(ann) if ann.name == "gpu" => ScopeKind::Gpu,
    Some(ann) if ann.name == "unsafe" => ScopeKind::Unsafe,
    Some(ann) if ann.name == "safe" => ScopeKind::Safe,
    _ if fndef.is_async => ScopeKind::AsyncFn,
    _ => ScopeKind::Function,    // ◀── unannotated fn = permissive
};
```

`ScopeKind::Function` does not satisfy `is_inside_safe()`, so the
SE020 / KE002 enforcement that reads `is_inside_safe()` never fires
for unannotated fns. **The comment lies; the code is permissive.**

### 2.3 Closure-cost gauge

| Metric | Count |
|--------|-------|
| Rust src lines to change | **1** (the `_ => Function` arm) |
| stdlib/ unannotated fn | **180** (vs 79 annotated; ~70% would gain enforcement) |
| examples/ files using annotations | **23 of 242** (most examples are unannotated by convention; many would break if flipped) |
| Likely test breakage | medium-high (148 context_safety_tests is by-design narrow; many of the 957 eval_tests use unannotated fns calling builtins) |

**Verdict: not a one-line fix.** The change is structurally simple but
the migration cascade is comparable in scope to v35.5.0 D-FULL
(D-FULL touched ~37 stdlib sites + 19 Q6A examples + 12 integration
tests). A §4.4-context closure would touch every unannotated fn that
legitimately uses domain builtins (push, port_*, tensor ops, mem_*).

---

## §3. Adjacent micro-gaps (lower priority)

These surfaced during the survey but are not critical-path:

1. **`heap_builtins` set is small (11 names)** vs `tensor_builtins`
   (40+) and `os_builtins` (45+). Possibly under-tags string concat,
   f-string interpolation, format-style, or other heap-allocating
   primitives. Audit recommended; closure would extend the set.
2. **Cranelift codegen builtin list disagrees with analyzer**
   (`String_new`, `Vec_new` are codegen-only names, not in analyzer
   sets). Probably benign defense-in-depth; could be reconciled.
3. **f-string heap-allocation** — does `f"{x}"` route through a
   heap-allocating runtime fn? If yes, is the fn-name in
   `heap_builtins`? Quick probe needed.
4. **Top-level expressions** — let-bindings at module scope are not
   inside a `ScopeKind::*` for any of the context kinds. Behavior
   undocumented here; probably non-issue (top-level usually @safe-ish
   by virtue of being initialization).

---

## §4. Reproduction (run by Claude EOS-27)

```bash
# context_safety_tests already passes
cargo test --release --test context_safety_tests
# → 148 passed; 0 failed

# Smoke probe — confirms §4.4 gap
cat > /tmp/p.fj <<'EOF'
fn no_ann() -> void { let _t = zeros(3, 3) }
EOF
cargo run --release -- check /tmp/p.fj
# → OK: /tmp/p.fj — no errors found
# (would fire KE002 if default-safe were enforced)

# By comparison, the explicit-@safe variant DOES fire
cat > /tmp/p2.fj <<'EOF'
@safe fn f() -> void { port_outb(0x3F8, 65) }
EOF
cargo run --release -- check /tmp/p2.fj
# → fires SE020
```

---

## §5. Comparison vs predecessor B0s

| B0 | Predicted scope | Actual finding | Variance |
|----|-----------------|----------------|----------|
| `D_FULL_CASCADE_B0_FINDINGS` | ~14h Strategy D | ~7h D-LITE done (D-FULL deferred to v35.5.0) | -50% |
| `PHASE17_PERF_B0_FINDINGS` | ~1-2h profile + optimize | ~25min: NO regression to chase | -75% |
| **`KERNEL_MODE_B0_FINDINGS`** (this) | ~2-4h B0; multi-session impl | B0 ~1h; impl is Phase-2-class | tbd |

Recurring lesson: B0s on "next strategic chapter" tend to discover the
chapter is **already substantially built**. The honest deliverable is
re-scoping, not greenfield design.

---

## §6. Recommendation table for next phase

| Option | Closes | Cost | Risk | When to pick |
|--------|--------|------|------|--------------|
| **A. §4.4 default-safe ship** (`Function` → `Safe`) | Compass §4.4 at the context level (the actual stated promise) | High — Phase 2-class migration; ~30-100 fn need explicit annotation | High — many tests + examples affected | **If user wants the natural pair to v35.5.0 D-FULL.** Closes the language-design promise that `@safe` is the default at every level (affine + context). |
| **B. Audit + extend `heap_builtins`** | KE001 enforcement completeness | Low — likely <1h | Low | If user prefers a small win that hardens existing enforcement. |
| **C. Reconcile analyzer vs Cranelift builtin lists** | Layer-consistency | Low | Low | Same as B; cosmetic but principled. |
| **D. Defer @kernel work; pivot to other strategic items** | n/a | 0h | 0 | If strategic priority is elsewhere (Compass §5 backlog, Phase E/F roadmap, etc.). |

**My recommendation:** Option A (§4.4 default-safe ship). It is the
real strategic chapter the original "@kernel mode" framing pointed at.
v35.5.0 closed §4.4 at the *type-system* level (affine); A closes it at
the *context* level. The scope is comparable to D-FULL and is best
done while the D-FULL recipe is fresh in memory.

If A is picked, the next step is a Phase-A B0 (sub-B0) that:
1. Flips line 160 locally (no commit yet)
2. Runs full test suite + records every failure
3. Categorizes each failure as (i) legitimate gap (annotate), (ii) false
   positive (loosen enforcement), or (iii) over-strict (relax compass)
4. Produces a migration playbook before any cascade fixes

---

## §7. Self-check (Plan Hygiene §6.8)

```
[x] Pre-flight audit (B0) hands-on verifies baseline?      (R1)
[x] Verification commands runnable and quoted in doc?       (R2)
[x] Prevention layer in §3 + §6 (audit recommendations)?   (R3)
[x] Numbers cross-checked with Bash (line counts, sets)?   (R4)
[x] Effort variance tagged in commit message?               (R5)
[x] Decision file gate at §6 (4-option table)?              (R6)
[ ] Public-artifact drift?                                   (R7) — N/A (no public claim to correct yet)
[ ] Multi-repo state check?                                  (R8) — N/A (single repo)
```

6/6 applicable YES. R7 N/A because no false public claim was made
about @kernel mode (unlike the phase17 perf B0).

---

## §8. Disposition

- ✅ Findings doc committed locally (this file).
- ⏸️ Phase plan: **awaits user decision** at §6 table.
- ✅ B0 task list cleaned. Pending tasks 5-9 closed.
- 🟢 No code changes; v35.5.0 working tree unaffected.
