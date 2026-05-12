# V35.6 Layer Reconciliation — B0 Findings (Option A pre-flight)

> **Phase:** v35.6.x Option A — Layer reconciliation: analyzer vs codegen builtin lists
> **Audit date:** 2026-05-12
> **Plan Hygiene §6.8 R1:** This B0 sub-phase precedes any code work.

## §1. Scope

The EOS-28 resume protocol (`memory/project_resume_lanjut_protocol.md` §2.A)
flagged a layer-reconciliation question:

> Per `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §3 micro-gap #2:
> - Analyzer canonical: `heap_builtins` (11 names) + `tensor_builtins` (40+) + `os_builtins` (~150)
> - Cranelift codegen at `src/codegen/cranelift/mod.rs:349-400`: smaller non-overlapping list
>   (`String_new`, `Vec_new`, `read_file`, `write_file`, `append_file` for HEAP_OPS; ~17 names for TENSOR_OPS)
>
> Two paths:
> - **B-α**: Cranelift list mirrors analyzer (defense-in-depth, redundancy)
> - **B-β**: Cranelift list dropped entirely (analyzer is single source of truth)
>
> Recommended: **B-β** because analyzer runs first; if it passes, codegen need not re-check.

This B0 hands-on verifies the premise.

## §2. Verified facts (commit refs + line numbers, all on main @ HEAD)

### 2.1 Cranelift `check_call_name` hardcoded lists (`src/codegen/cranelift/mod.rs:346-423`)

| Category | Count | Names |
|---|---|---|
| `TENSOR_OPS` (KE002) | 19 | tensor_zeros, tensor_ones, **tensor_rand**, tensor_xavier, tensor_matmul, tensor_relu, tensor_sigmoid, tensor_softmax, zeros, ones, randn, xavier, matmul, relu, sigmoid, softmax, backward, **tensor_grad**, **cross_entropy_loss** |
| `HEAP_OPS` (KE001) | 5 | **String_new**, **Vec_new**, read_file, write_file, append_file |
| `PTR_OPS` (DE001) | 10 | mem_alloc, mem_free, **mem_read**, **mem_write**, mem_read_u8, **mem_read_u16**, mem_read_u32, mem_write_u8, **mem_write_u16**, mem_write_u32 |
| `IRQ_OPS` (DE002) | 6 | irq_register, irq_unregister, irq_enable, irq_disable, port_read, port_write |
| **Total** | **40** | |

**Bold = name NOT present in analyzer canonical lists.** Stale/drifted.

### 2.2 Analyzer canonical lists (`src/analyzer/type_check/mod.rs`)

| List | Lines | Count | Sample |
|---|---|---|---|
| `heap_builtins` | L1733-1748 | 11 | push, pop, to_string, map_insert/get/get_or/remove/contains/keys/values/len |
| `tensor_builtins` | L1750-1837 | ~80 | tensor_zeros..tensor_scale + 30+ short aliases (Dense, Conv2d, SGD, Adam, …) |
| `os_builtins` | L1439-1655 | ~150 | mem_alloc, page_map, irq_*, port_*, x86_*, cpuid_*, proc_*, kb_*, pci_*, volatile_*, buffer_*, … |
| `safe_blocked_builtins` | L1681-1730 | os_builtins minus 6 carve-outs | str_byte_at, str_len, tensor_workload_hint, cap_new, cap_unwrap, cap_is_valid carved out (v35.6.0) |

Analyzer enforcement (`src/analyzer/type_check/check.rs:1845-1924`):
- in_kernel: heap_builtins → KE001; tensor_builtins → KE002; + transitive taint
- in_device: kernel_fns → DeviceCallInKernel; os_builtins → DE001 (RawPointerInDevice)
- in_gpu: os_builtins → DE001; heap_builtins → KE001
- in_safe: safe_blocked_builtins → SE020 (default for unannotated `fn` since v35.6.0)

### 2.3 List divergence (Cranelift ∩ Analyzer)

| Category | Cranelift only | Both | Analyzer only |
|---|---|---|---|
| HEAP | `String_new`, `Vec_new` (don't exist as current builtins; obsolete?), `read_file`, `write_file`, `append_file` (analyzer treats these as gpu-block but not kernel-block) | **0 overlap with analyzer's `heap_builtins`** | 11 (push, pop, to_string, map_*) |
| TENSOR | `tensor_rand` (≠analyzer's `tensor_randn`), `tensor_grad` (≠analyzer's `grad`), `cross_entropy_loss` (≠analyzer's `cross_entropy`) | ~16 | ~64 (full tensor_builtins minus subset) |
| PTR | `mem_read`, `mem_write`, `mem_read_u16`, `mem_write_u16` | 6 (mem_alloc/free, mem_read_u8/u32, mem_write_u8/u32) | ~144 (full os_builtins minus mem_*) |
| IRQ | (none) | 6 (irq_*, port_*) | analyzer's os_builtins is superset of IRQ_OPS |

**Conclusion:** Cranelift's list is **partial, drifted, and contains stale names** (`tensor_rand`, `String_new`, `Vec_new`, `mem_read_u16`). It is NOT a faithful mirror of the analyzer.

## §3. **Premise correction — B-β as stated is unsafe**

The protocol's recommendation assumed Cranelift's check is pure defense-in-depth
(analyzer runs first, codegen need not re-check). **This is false** for two production paths.

### 3.1 Path 1: `fj run --native` (Cranelift)

`src/main.rs:1920-1968` `cmd_run_native`:
```
lex → parse → CraneliftCompiler::compile_program  (NO analyzer pass)
```
The H4 hook at `mod.rs:5494-5507` (and identical hook at `mod.rs:12810-12823`) is
the **only** context-violation check on this path.

### 3.2 Path 2: `fj run --llvm` (LLVM)

`src/main.rs:1996-2056` `cmd_run_llvm`:
```
lex → parse → LlvmCompiler::compile_program  (NO analyzer pass)
```
And: **LLVM codegen has no context-violation check at all** (verified via grep —
no `check_context`, no `ContextViolation`, no equivalent hook in `src/codegen/llvm/*.rs`).
So `fj run --llvm` silently compiles `@kernel fn { tensor_zeros(2,3) }` to native code today.

### 3.3 Production paths that DO run analyzer first

`cmd_run` (L1108, default interpreter), `cmd_run_vm` (L1874, bytecode), `cmd_run_jit`
(L1699, Cranelift JIT — separate from `--native`), `cmd_check` (L1534), `cmd_run_strict`
(L1621) all call `analyze(&program)` before backend dispatch.

So the analyzer-bypassed paths are precisely the two AOT-style backends (`--native`,
`--llvm`), which is also where end-users most need safety guarantees.

### 3.4 Test artifacts that depend on H4 hook firing

Three tests in `src/codegen/cranelift/tests.rs:15724-15791` exercise the H4 hook
**directly by bypassing the analyzer**:

- `context_kernel_rejects_tensor` (@kernel + tensor_zeros)
- `context_kernel_rejects_read_file` (@kernel + read_file)
- `context_device_rejects_raw_pointer` (@device + mem_alloc)

If the H4 hook is dropped, these tests' assertions fail unless rewritten to run
the analyzer first.

## §4. Refined decision matrix

Original protocol gave two paths (B-α mirror, B-β drop). The hands-on audit
surfaces **four** options with different scope/risk trade-offs:

### B-α — Cranelift mirrors analyzer (+ propagate to LLVM)

- Extract analyzer's `tensor_builtins` / `heap_builtins` / `os_builtins` from
  `mod.rs:1439-1837` into pub statics.
- Cranelift `check_call_name` consumes the analyzer's lists (instead of hardcoded).
- Add equivalent hook to LLVM codegen (mirrors Cranelift's).
- **Effort:** ~2-3h · **Risk:** Low (no semantic change; remove duplication)
- **Maintenance:** Single source of truth for builtin classification; new builtins
  automatically picked up by both codegens.
- **Deliverable:** 1 refactor commit + 1 LLVM hook commit + closure findings.

### B-β — Drop Cranelift list (PRECONDITION: plug analyzer-bypass paths first)

- **Precondition (must ship first):** Add `analyze(&program)` call to `cmd_run_native`
  and `cmd_run_llvm`.
- Drop H4 hook + `check_context_violations` + `check_call_name` from Cranelift.
- Rewrite 3 tests at `tests.rs:15724-15791` to run analyzer first OR delete them
  (analyzer's own context_safety suite covers same invariants via 149 tests).
- **Effort:** ~1-2h · **Risk:** Medium (changes API contract — non-analyzed
  `compile_program` calls lose safety check; any external caller is affected)
- **Maintenance:** Smaller — Cranelift becomes pure codegen.
- **Deliverable:** 1 main.rs commit (add analyze) + 1 cranelift cleanup commit
  + test migration + closure findings.

### B-γ — Shared `context_safety` module (Cranelift + LLVM both consume)

- Extract canonical lists to `src/analyzer/context_safety/mod.rs` (or similar).
- Both analyzer's `check.rs` and Cranelift's `check_call_name` consume from this
  module.
- Add equivalent hook to LLVM codegen (consumes same module).
- **Effort:** ~3-5h · **Risk:** Low-Medium (refactor crosses analyzer/codegen
  boundary; needs care with module deps)
- **Maintenance:** Cleanest — single source of truth + genuine defense-in-depth
  at every backend.
- **Deliverable:** 1 module-extract commit + 1 analyzer-use commit + 1
  Cranelift-use commit + 1 LLVM-add commit + closure findings.

### B-δ — Plug-the-hole minimal-scope (PRAGMATIC)

- Add `analyze(&program)` call to `cmd_run_native` and `cmd_run_llvm` (~10 LOC).
- Leave Cranelift H4 hook untouched (now genuine belt-and-suspenders).
- Leave LLVM untouched (analyzer pre-pass catches anything LLVM would miss).
- Add 1 regression test confirming `fj run --native` rejects `@kernel fn { tensor_zeros(...) }`
  via analyzer (not just Cranelift).
- Acknowledge Cranelift's hardcoded list is a partial belt; defer cleanup to
  v36.x as scope-limited refactor.
- **Effort:** ~30-45min · **Risk:** Lowest (additive only)
- **Maintenance:** Drift remains but is no longer load-bearing; the analyzer is
  the authoritative gate.
- **Deliverable:** 1 main.rs commit + 1 regression test + decision doc deferring
  full cleanup to v36.x.

## §5. Recommendation

**B-δ first (this session), B-γ later (v36.x or as part of v35.7.x).**

Rationale:
1. **Immediate safety win**: closing the `fj run --native` / `fj run --llvm`
   analyzer-bypass holes is the most impactful single change. Currently those
   commands accept programs the analyzer would reject.
2. **Smallest scope per §6.8 R5**: B-δ is ≤45min; protocol budget was 1-2h
   total. Headroom to ship cleanly.
3. **B-γ is the right end state** but extracting & refactoring three backends'
   list-consumption is a v36 refactor with its own B0. Don't bundle.
4. **No file deletes, no test rewrites**: the H4 hook + 3 cranelift tests stay
   put. Drift is documented and time-bounded.

## §6. Stage 2 byte-equality risk

- B-δ touches only `src/main.rs` (Rust). No fj-source change. **Phase17
  byte-equality NOT at risk.**
- B-α, B-β, B-γ all touch codegen but no stdlib `.fj` files. Phase17 risk
  also LOW for all four — but B-δ has zero touchpoints in stdlib so is the
  safest by definition.

## §7. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase           (Rule 1)
[x] Every task has runnable verification command          (Rule 2 — see §8)
[ ] Prevention mechanism added (hook/CI/rule)             (Rule 3 — B-δ adds regression test)
[x] Agent-produced numbers cross-checked with Bash        (Rule 4 — all counts verified live)
[ ] Effort variance tagged in commit message              (Rule 5 — at commit time)
[ ] Decisions are committed files                         (Rule 6 — decision doc still TBD)
[ ] Public-artifact drift swept                            (Rule 7 — done in R4)
[x] Multi-repo state checked                              (Rule 8 — R1 done)
```

## §8. Verification commands for chosen path

### For B-δ (recommended):
```bash
# Before:
cd "/home/primecore/Documents/Fajar Lang"
cat > /tmp/kernel_bypass.fj <<'EOF'
@kernel fn boot() -> i64 {
    let t = tensor_zeros(2, 3)
    0
}
fn main() -> i64 { boot() }
EOF
cargo run --features native -- run --native /tmp/kernel_bypass.fj 2>&1 | grep -E "(KE002|tensor)"

# After B-δ ship: same command should print analyzer's KE002 error
# (currently prints Cranelift's H4 KE002 — they look similar but origin differs)

# Coverage check:
cargo test --lib test_cmd_run_native_runs_analyzer  # new regression test
cargo test --lib --features native context_           # cranelift H4 tests still green
```

## §9. Source artifacts (audit trail)

- This file: `docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md`
- Decision file (to write after user picks): `docs/decisions/2026-05-12-cranelift-builtin-list-shape.md`
- Pred-context: `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §3 micro-gap #2
- Resume protocol: `memory/project_resume_lanjut_protocol.md` §2.A

---

*B0 written 2026-05-12 EOS-29 session. ~30min actual. All counts verified live
via grep on HEAD `bd11e8e3` (post-R4 ship).*
