---
phase: FAJAROS_100PCT_FJ — make Fajar Lang capable to build FajarOS Nova in 100% Fajar Lang (no .S, no .c, no inline-asm hacks where avoidable)
status: planned 2026-05-04
budget: 17-26 days base + 25% surprise = 21-32 days realistic
        (medium uncertainty: Phase 0 audit may reveal more inline-asm dependencies than the 4 known classes)
prereq: V33 PERFECTION_PLAN closed (engineering-side); HONEST_AUDIT_V33 = exit baseline.
        CI rehab chain `cfb82c88..6467fa07` landed (main CI green again).
        fajaros-x86 v3.9.0 (`ee13127`), fajarquant `3015545+`, fajar-lang `6467fa07`.
artifacts:
  - This plan doc
  - docs/FAJAROS_100PCT_FJ_PHASE_<N>_FINDINGS.md (one per phase)
  - docs/FAJAROS_100PCT_FJ_FINAL.md (synthesis when all phases close)
  - Code/test commits in fajar-lang AND fajaros-x86 (cross-repo per §6.8 R8)
  - Prevention layer per phase (§6.8 R3)
---

# Fajar Lang → FajarOS 100% Plan v1.0

> **User signal 2026-05-04:** *"Apakah Fajar Lang sekarang sudah capable
> 100% untuk membuat FajarOS tersebut atau perlu ada yang diperbaiki lagi
> ... jangan sampai nanti gagal karena ketidakmampuan Fajar Lang ...
> jangan pernah bilang kapan-kapan, kalau harus kita kerjakan agar Fajar
> Lang capable membuat FajarOS 100%, segera buat plan detail agar kita
> bisa kerjakan."*

## 1. What "100% Fajar Lang FajarOS" means (HONEST scope)

We CAN reach (this plan's exit criteria):

- ✅ **Kernel + drivers + apps + boot all `.fj` source** in `fajaros-x86/`. No `.S` files, no `.c` files, no `.cpp` files in the kernel build path.
- ✅ **Fajar Lang compiler has zero capability gap** that forces a developer to "drop down" to assembly via raw external file (inline `asm!()` blocks INSIDE `.fj` are allowed and considered Fajar Lang code, matching Rust's `unsafe { asm!(...) }` philosophy).
- ✅ **Hot-path performance preserved** (vecmat ≥ current C-bypass throughput within ±10%). No regression vs V30 P3.6 baseline.
- ✅ **SMP-correct synchronization** (current spinlock is TOCTOU race-prone — see Phase 1).
- ✅ **All existing FajarOS regression gates green** through every phase: `test-fs-roundtrip`, `test-security-triple-regression`, `test-gemma3-e2e`, `test-smap-regression`.

We CANNOT (acknowledged honestly):

- ❌ **Eliminate inline `asm!()` blocks entirely.** RDMSR/WRMSR/CPUID/port I/O fundamentally need machine instructions. Same applies to Linux (uses inline asm) and Rust kernels (use `core::arch::asm!`). Fajar Lang's `asm!()` is the equivalent — using it does NOT count as "non-Fajar-Lang code."
- ❌ **Eliminate vendored upstream code (F.11 BitNet TL2).** PERMANENT-DEFERRED per memory; vendoring upstream microsoft/BitNet is by-design FFI, not a fj-lang gap. Out of scope. If user later wants this in 100%, that's a separate Phase F.X work.
- ❌ **Eliminate Python host-side tooling** (`fajaros-x86/scripts/*.py`). Model export, disk build, kernel-trace parsing — these run on the host, not in the kernel. Out of scope. Could become its own future plan if host-side too should be `.fj`.
- ❌ **Fix LLVM upstream O2 vecmat miscompile.** A1 filing is pending founder action per V33; we can ship workarounds (`@no_vectorize`, AVX2 builtins) but not the compiler fix itself.

The exit criteria reflect this: **kernel build artifacts are 100% derived from `.fj` source**, with inline `asm!()` allowed. F.11 + Python scripts explicitly out of scope.

## 2. Hand-verified gap inventory (Phase 0 will re-verify)

Source: 2026-05-04 audit via `find` + `wc -l` + grep. See conversation transcript.

### Non-fj code currently in `fajaros-x86/` kernel build path

| File | LOC | Class | Replaceable today? |
|---|---|---|---|
| `boot/startup.S` | 515 | Asm boot trampoline | ✅ `fj build --no-std` auto-gens equivalent (`src/codegen/linker.rs:1418` `generate_x86_64_startup`) |
| `boot/runtime_stubs.S` | 912 | 15 asm symbols (VGA, str ops, buffer LE/BE, IDT/TSS/PIT init) | ✅ all replaceable via `global_asm!()` blocks in `.fj` (proven pattern: `kernel/hw/msr.fj`) |
| `kernel/compute/vecmat_v8.c` | 768 | C bypass for LLVM O2 vecmat miscompile | ✅ replace with dual-impl: `@no_vectorize` (slow path) + AVX2 builtins (fast path) + CPUID dispatch |
| **TOTAL** | **2,195** | (kernel-side only; F.11 wrapper.cpp + Python scripts are out of scope) | |

### Fajar Lang compiler gaps (block clean kernel-writing)

| ID | Gap | Severity | Workaround today | Fix |
|---|---|---|---|---|
| **G-A** | LLVM backend has no native atomics (Cranelift does) | **HIGH for SMP correctness** | inline `asm!("lock cmpxchg ...")` | Phase 5: `build_atomicrmw` + `build_cmpxchg` in `src/codegen/llvm/mod.rs` |
| **G-B** | No `@naked` attribute (Rust has `#[unsafe(naked)]`) | LOW | `global_asm!()` block per stub | Phase 6: `AtNaked` token + LLVM `naked` attr |
| **G-C** | No `@no_mangle` explicit attribute | LOW | `extern "C" fn foo()` | Phase 7: `AtNoMangle` token + suppress mangling |
| **G-D** | No `thread_local` keyword (TLS) | NEGLIGIBLE for kernel | FS_BASE/GS_BASE MSR direct access | Out-of-scope (kernel rarely needs userspace TLS) |
| **G-E** | LLVM O2 vecmat miscompile (upstream LLVM) | MEDIUM | `@no_vectorize` + AVX2 builtins | Out-of-fj-lang-scope; A1 upstream filing pending |

### Active correctness bug (independent of 100% migration)

| ID | Bug | Severity | File |
|---|---|---|---|
| **C-1** | Spinlock uses `volatile_read` + `volatile_write` (TOCTOU race-prone, NOT atomic) | **HIGH if SMP enabled** | `fajaros-x86/kernel/sched/spinlock.fj:9-17` |

> **C-1 is silently latent** — fajaros boots single-CPU in normal QEMU runs.
> Goes critical the moment `cmd_smp_boot` brings up APs and AP code touches
> the same lock. Memory says SMP work is V31 era.

## 3. Phase breakdown (8 phases, sequenced for safety)

### Phase 0 — Pre-flight audit (mandatory per §6.8 R1) — **0.5-1 day, +25%**

| # | Task | Verification (runnable command) | Effort |
|---|---|---|---|
| 0.1 | Inventory all non-fj files in `fajaros-x86/` kernel build path (re-verify §2 numbers) | `cd ~/Documents/fajaros-x86 && find . -type f \( -name "*.S" -o -name "*.c" -o -name "*.cpp" -o -name "*.asm" \) -not -path "*/target/*" -not -path "*/.git/*" -not -path "*/build/*" \| tee docs/non_fj_inventory.txt`; `wc -l` each; produce `FAJAROS_100PCT_FJ_PHASE_0_FINDINGS.md` | 2-3h |
| 0.2 | Audit fajar-lang inline-asm operand support depth (in/out/inout/clobber/options/abi) | grep `parse_inline_asm` body in `src/parser/expr.rs`; cross-check against `fajaros-x86/kernel/hw/msr.fj` real usage; document any operand class `parse_inline_asm` rejects | 1-2h |
| 0.3 | Verify `generate_x86_64_startup()` output is bit-equivalent to `boot/startup.S` (semantically) | `cargo run -- build examples/dummy_kernel.fj --no-std --target x86_64-unknown-none > /tmp/auto_startup.S; diff` against `fajaros-x86/boot/startup.S`; document semantic diff (NOT byte diff, since assembler emits differ) | 2h |
| 0.4 | Verify spinlock C-1 race actually trips under racing AP test | write 2-thread test in `tests/spinlock_race.fj` (or shell-driven QEMU SMP-2 test); show counter ≠ N×increments under volatile spinlock | 2-3h |
| 0.5 | Cross-repo state check (§6.8 R8): `git status -sb && git rev-list --count origin/main..main` for fajar-lang, fajaros-x86, fajarquant | All clean, all `0` ahead of origin | 5min |

**Prevention layer (§6.8 R3):** Phase 0 ships `scripts/audit_fajaros_non_fj.sh` that lists non-fj kernel-build files. Wired into `make audit-100pct-fj` Makefile target. Pre-commit hook in fajaros-x86 runs it on commit; downstream phases must show this count strictly decreasing.

**Decision gate (§6.8 R6):** `docs/FAJAROS_100PCT_FJ_PHASE_0_FINDINGS.md` committed. Pre-commit hook checks file exists before allowing Phase 1+ commits.

### Phase 1 — URGENT: fix spinlock C-1 race (~0.5 day, +30%)

> **Why first:** independent of 100% migration. Active correctness bug. Hidden by single-CPU testing. Ship the fix even if rest of plan slips.

| # | Task | Verification | Effort |
|---|---|---|---|
| 1.1 | Replace `kernel/sched/spinlock.fj` body with inline-asm `LOCK CMPXCHG` | Re-run Phase 0.4 spinlock-race test → counter == N×increments | 1-2h |
| 1.2 | Audit kernel for other "volatile-as-atomic" patterns (sequential search of all `.fj`) | `grep -rn "volatile_read\|volatile_write" kernel/` → manually classify each as RACE-SAFE / RACE-PRONE / ALREADY-ATOMIC; commit `docs/FAJAROS_100PCT_FJ_PHASE_1_FINDINGS.md` | 1-2h |
| 1.3 | Add `make test-spinlock-smp-regression` Makefile target (real QEMU 2-CPU race test) | `make test-spinlock-smp-regression` → exits 0 with "spinlock: 1000000/1000000 increments PASS" | 1h |
| 1.4 | Wire test-spinlock-smp-regression into pre-push hook | `cat .git/hooks/pre-push \| grep test-spinlock-smp-regression` succeeds | 15min |

**Prevention layer:** Pre-push hook + `make test-spinlock-smp-regression` Makefile gate.

**Surprise budget:** +30% (uncertain whether fjaros's QEMU SMP path actually exposes the race deterministically; may need 1000+ iterations to catch).

### Phase 2 — Replace `boot/startup.S` with auto-generated startup (~1-1.5 days, +25%)

| # | Task | Verification | Effort |
|---|---|---|---|
| 2.1 | Generate startup via `cargo run -- build kernel/main.fj --no-std --emit-startup` to `build/auto_startup.S`; semantic diff vs `boot/startup.S` | Both produce equivalent Multiboot2 header + 32→64 transition + BSS zero + serial init + call to entry | 2-3h |
| 2.2 | Modify `Makefile` to assemble auto-gen startup instead of `boot/startup.S` (keep the file but unused for one transition commit) | `make build/fajaros-llvm.elf` succeeds with auto-gen path | 2h |
| 2.3 | Boot test in QEMU; verify `Nova v3.7.0 ready` prompt reaches stdout | `make test-boot-qemu-llvm` exits 0 | 1h |
| 2.4 | Run all existing regression gates: `make test-fs-roundtrip test-security-triple-regression test-gemma3-e2e test-smap-regression` | All pass | 2h |
| 2.5 | Delete `boot/startup.S`; update Makefile to remove its build rules; update README | `git rm boot/startup.S && make all && all gates green` | 1h |
| 2.6 | Phase 2 findings doc | `git add docs/FAJAROS_100PCT_FJ_PHASE_2_FINDINGS.md` | 30min |

**Prevention layer:** Add CI job `boot-from-fj-only` that fails if any `.S` files exist in `boot/` (after this phase). Pre-commit hook in fajaros-x86: `find boot/ -name "*.S" -not -name "*.example" -exec false {} +`.

### Phase 3 — Replace `boot/runtime_stubs.S` with `.fj` `global_asm!()` blocks (~3-5 days, +25%)

15 symbols to migrate, grouped by hardware function:

| Group | Symbols | Target file | Sub-task |
|---|---|---|---|
| **VGA console** | `fj_rt_bare_console_putchar` | `kernel/runtime/vga_console.fj` | 3.1 |
| **String ops** | `fj_rt_bare_str_len`, `fj_rt_bare_str_byte_at` | `kernel/runtime/str_ops.fj` | 3.2 |
| **Buffer LE/BE** | `fj_rt_bare_buffer_read_u16/32/64_le`, `fj_rt_bare_buffer_read_u16/32_be`, `fj_rt_bare_buffer_write_u16/32_le`, `fj_rt_bare_buffer_write_u16/32_be` (10 symbols) | `kernel/runtime/buffer_endian.fj` | 3.3 |
| **Hardware init** | `fj_rt_bare_idt_init`, `fj_rt_bare_tss_init`, `fj_rt_bare_pit_init` | `kernel/runtime/hw_init.fj` | 3.4 |

| # | Task | Verification | Effort |
|---|---|---|---|
| 3.1 | Port VGA console putchar → `kernel/runtime/vga_console.fj` with `global_asm!()` block | `cargo run -- build kernel/runtime/vga_console.fj --no-std --emit-llvm-ir`; verify symbol present in IR; QEMU boot prints "Hello" via VGA | 2-3h |
| 3.2 | Port str ops (2 symbols) → `kernel/runtime/str_ops.fj` | nm `.elf` shows `fj_rt_bare_str_len` + `fj_rt_bare_str_byte_at` | 2-3h |
| 3.3 | Port buffer LE/BE ops (10 symbols) → `kernel/runtime/buffer_endian.fj` | nm `.elf` shows all 10 symbols; integration test reads/writes a known buffer and asserts correct LE/BE | 5-6h |
| 3.4 | Port hardware init (IDT/TSS/PIT, 3 symbols) → `kernel/runtime/hw_init.fj` | QEMU boot completes IDT install + TSS load + PIT timer fires | 5-7h |
| 3.5 | Run all gates again | `test-fs-roundtrip test-security-triple-regression test-gemma3-e2e test-smap-regression` all pass | 2h |
| 3.6 | Delete `boot/runtime_stubs.S`; update Makefile | `git rm boot/runtime_stubs.S && make all && all gates green` | 1h |
| 3.7 | Phase 3 findings doc | committed | 30min |

**Prevention layer:** `boot/` directory becomes empty (or contains only README explaining auto-gen). CI job `runtime-from-fj-only` fails if any `.S` re-introduced.

### Phase 4 — Replace `kernel/compute/vecmat_v8.c` with `.fj` dual-impl (~1.5-2 days, +25%)

| # | Task | Verification | Effort |
|---|---|---|---|
| 4.1 | Port vecmat_v8.c logic 1:1 → `kernel/compute/vecmat_v8.fj` with `@no_vectorize` (slow path) | `make test-vecmat-bit-exact` (compares fj output vs Python sim) → bit-exact | 4-5h |
| 4.2 | Port hot loop using AVX2 builtins → `kernel/compute/vecmat_v8_avx2.fj` | `cargo run -- build --features llvm --emit-llvm-ir`; verify `vpmaddubsw` + `vpdpbssd` in IR | 4-5h |
| 4.3 | CPUID dispatch wrapper: `if cpuid_has_avx2() { vecmat_avx2() } else { vecmat_safe() }` | LLM E2E test (Gemma 3 1B 12-phase audit) PASS on AVX2-capable host AND non-AVX2 host (or simulated via `-cpu pentium2`) | 2-3h |
| 4.4 | Performance regression check vs V30 P3.6 C baseline | `make bench-vecmat`; report tok/s; ≥ 90% of C baseline (memory: 50-100 tok/s on i7-13800H) | 1-2h |
| 4.5 | Delete `kernel/compute/vecmat_v8.c`; update Makefile (drop VECMAT_C, VECMAT_O, gcc rule) | `git rm vecmat_v8.c && make all && bit-exact still PASS` | 1h |
| 4.6 | Phase 4 findings doc | committed | 30min |

**Prevention layer:** `make test-vecmat-bit-exact` runs in CI; fails if vecmat output differs from Python sim by ANY bit. `pre-push` hook runs `bench-vecmat` if `kernel/compute/` changed; warns if perf drops >10%.

### Phase 5 — Add LLVM backend native atomics (~2-3 days, +30%) — closes Gap **G-A**

| # | Task | Verification | Effort |
|---|---|---|---|
| 5.1 | Audit Cranelift atomic implementation in `src/codegen/cranelift/compile/{call.rs,method.rs}` to map naming/semantics for parity | findings doc with name table | 2h |
| 5.2 | Add LLVM atomic emission: `build_atomicrmw` + `build_cmpxchg` + memory orderings (Relaxed/Acquire/Release/AcqRel/SeqCst) | `cargo test --features llvm --lib codegen::llvm::tests::atomic_*` (≥ 8 new tests, one per orderings × ops) | 6-8h |
| 5.3 | Wire `Atomic<T>::compare_and_swap`, `fetch_add`, `fetch_sub`, `load`, `store` into LLVM builtin lowering | example `.fj` exercising each → bit-exact behavior vs Cranelift | 4-5h |
| 5.4 | Replace `kernel/sched/spinlock.fj` Phase 1 inline-asm version with `AtomicI64` cmpxchg | spinlock-smp-regression still PASS; LOC drops; no `asm!()` block needed | 2h |
| 5.5 | Audit any other inline-asm sync primitives in fajaros kernel; replace with atomics where applicable | `grep -rn "asm!" kernel/sched/ kernel/mm/ kernel/ipc/` → classify each; replace where appropriate | 3-4h |
| 5.6 | Document atomic API in `docs/STDLIB_SPEC.md`; add error codes if needed | section added; `cargo doc` clean | 1h |
| 5.7 | Phase 5 findings doc | committed | 30min |

**Prevention layer:** New CI test `atomic_orderings_e2e_match_cranelift` ensures both backends produce semantically equivalent atomic ops for all 5 orderings. Pre-commit hook fails if `atomic_*` test count drops.

**Surprise budget:** +30% (LLVM atomic emission for tracked types in inkwell can have edge cases per memory ordering; may need to handle `monotonic` vs `unordered` mapping).

### Phase 6 — Add `@naked` attribute (~3-5 days, +25%) — closes Gap **G-B**

> **Optional but planned** — Phase 1-5 already gets fajaros to 100% fj.
> Phase 6 is "polish" that lets future kernel devs write naked stubs without
> `global_asm!()` boilerplate. Per user signal: NOT "kapan-kapan" — committed
> to ship.

| # | Task | Verification | Effort |
|---|---|---|---|
| 6.1 | Add `AtNaked` to `src/lexer/token.rs` (token + display + insert in keyword map) | `cargo test --lib lexer::tests::lex_at_naked_*` (3 tests: solo, with fn, with other attrs) | 1.5h |
| 6.2 | Parser: accept `@naked` on fn declarations; reject on non-fn items | `cargo test --lib parser::tests::parse_at_naked_*` (5 tests: fn ok, struct rejected, enum rejected, mixed-with-noinline-cold ok, error span correct) | 2h |
| 6.3 | Analyzer: `@naked` requires `@kernel` or `@unsafe` context AND function body is single `asm!()` block | error code KE006 (or next free) "naked function must contain only asm!() in @kernel/@unsafe context"; 4-6 tests | 3-4h |
| 6.4 | LLVM codegen: emit `naked` function attribute on the `FunctionValue` | LLVM IR contains `attributes #N = { naked ... }`; `objdump -d` shows ONLY the user's asm bytes (no Rust prologue/epilogue) | 4-5h |
| 6.5 | Cranelift backend: same support OR explicit "not supported in Cranelift" error | either parity or graceful error CE-XX | 3-4h |
| 6.6 | Migrate `kernel/runtime/hw_init.fj` (Phase 3.4) IDT/TSS/PIT init to use `@naked fn` instead of `global_asm!()` | functionality unchanged; LOC drops; readability improved | 4-6h |
| 6.7 | Document `@naked` in CLAUDE.md §5.3 + docs/SECURITY.md (caveat: skips bounds checks) | docs reviewed | 1h |
| 6.8 | Phase 6 findings doc | committed | 30min |

**Prevention layer:** Test `naked_function_no_implicit_prologue_epilogue` asserts emitted IR/asm has no compiler-inserted prelude. Pre-commit checks LLVM IR golden file unchanged when @naked example unchanged.

### Phase 7 — Add `@no_mangle` attribute (~0.5-1 day, +25%) — closes Gap **G-C**

| # | Task | Verification | Effort |
|---|---|---|---|
| 7.1 | Add `AtNoMangle` token + parser + analyzer | 3 tests: solo, with extern "C" rejected (redundant — error), with @kernel ok | 2h |
| 7.2 | LLVM/Cranelift codegen: suppress mangling when attribute present | `nm fajaros.elf \| grep "myExportFn"` (without mangled prefix) | 2h |
| 7.3 | Apply to fajaros symbols where `extern "C" fn` is currently used purely for non-mangling (if any) | LOC reduction; no behavior change | 1-2h |
| 7.4 | Phase 7 findings doc | committed | 30min |

**Prevention layer:** Lint warning if both `extern "C"` and `@no_mangle` on same fn (one is redundant).

### Phase 8 — Final validation + public-facing sync (~1-2 days, +25%)

| # | Task | Verification | Effort |
|---|---|---|---|
| 8.1 | Run `scripts/audit_fajaros_non_fj.sh` → expect 0 `.S`, 0 `.c`, 0 `.cpp` in kernel build path | exit 0 with "ALL FJ" | 30min |
| 8.2 | Full FajarOS regression suite | `make test-fs-roundtrip test-security-triple-regression test-gemma3-e2e test-smap-regression test-spinlock-smp-regression test-vecmat-bit-exact` ALL pass | 1h |
| 8.3 | Performance regression vs V30 P3.6 baseline (vecmat tok/s + boot time + IPC throughput) | within ±10% on AVX2 host; documented numbers | 1-2h |
| 8.4 | Update CLAUDE.md (§3 Current Status table: "FajarOS Nova: 100% Fajar Lang as of <date>") | `grep "100% Fajar Lang" CLAUDE.md` succeeds | 30min |
| 8.5 | Update fajaros-x86 README.md (kill "C bypass" / "boot asm" mentions; new build instructions) | rendered README clean | 30min |
| 8.6 | Multi-repo state check (§6.8 R8): all 3 repos clean, all `0` ahead of origin | output verified | 15min |
| 8.7 | Tag fajaros-x86 v3.10.0 "Pure Fajar Lang"; tag fajar-lang v34.0.0 if Phase 5+6+7 enhanced compiler | tags pushed; GitHub releases drafted | 1h |
| 8.8 | `docs/FAJAROS_100PCT_FJ_FINAL.md` synthesis | committed (mirroring HONEST_AUDIT_V33 style) | 1-2h |

**Prevention layer:** `make audit-100pct-fj` becomes a permanent CI gate in fajaros-x86. Adding any `.S` / `.c` file to kernel build path fails CI. Memory feedback note added.

## 4. Risk register

| ID | Risk | Probability | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Phase 0 audit reveals MORE inline-asm dependencies than the 4 known classes | Medium | Plan extends 1-3d | §6.8 R5 +25% surprise budget already accounts; Phase 0 ships before commits gate |
| R2 | Auto-gen startup (Phase 2) misses corner case in current `boot/startup.S` (e.g. CPUID feature detection ordering) | Medium | 1-2d delay | Bit-exact boot test in QEMU; keep `boot/startup.S` as `boot/startup.S.bak` for 2 commits |
| R3 | Phase 3 IDT/TSS/PIT init via `global_asm!()` hits assembler limitations (e.g. .align inside global_asm string escapes) | Low | 1d delay | Worst case: keep IDT init as `boot/idt_init.S` for one commit, port last |
| R4 | Phase 4 AVX2 vecmat under fj-lang LLVM produces different bytes than C baseline (because of @no_vectorize side effects, e.g. `no-implicit-float`) | Medium | 0.5-1d delay | Bit-exact gate is mandatory; if mismatch, fall back to scalar fj path while debugging |
| R5 | Phase 5 LLVM atomics: inkwell API doesn't expose `build_atomicrmw` cleanly | Low | 1-2d delay | LLVM-C inkwell API has it; if missing, use unsafe LLVMBuildAtomicRMW C-side via Module.context |
| R6 | Phase 6 `@naked` interacts poorly with `@interrupt` (existing) | Low | 0.5d delay | Document mutual exclusion in analyzer; test combination explicitly |
| R7 | Performance regression after Phase 4 (vecmat scalar path becomes default if AVX2 dispatch broken) | Medium | Halts release | Phase 4.4 perf gate enforces ≥90%; bench-vecmat in pre-push |
| R8 | Cross-repo dependency: fajar-lang v34 release blocks fajaros-x86 v3.10 release | Low | Sequencing | Ship fajar-lang v34 FIRST; fajaros bumps Cargo dep; both tagged in Phase 8 |
| R9 | LLVM upstream filing (A1) lands a fix while plan is in flight, conflicting with @no_vectorize | Very Low | Refactor needed | Revert @no_vectorize on affected sites once upstream fix released; current plan resilient either way |
| R10 | "100%" definition shifts (user later says F.11 + Python scripts must also be in fj) | Medium | Plan extension | Section 1 is HONEST scope; if user expands later, treat as Phase 9+ separate plan |

## 5. Effort summary

| Phase | Base | +Surprise | Realistic |
|---|---|---|---|
| 0 — Pre-flight audit | 0.5-1d | +25% | 0.6-1.3d |
| 1 — Spinlock C-1 fix | 0.5d | +30% | 0.65d |
| 2 — Auto-gen startup | 1-1.5d | +25% | 1.25-1.9d |
| 3 — Runtime stubs port | 3-5d | +25% | 3.75-6.25d |
| 4 — Vecmat dual-impl | 1.5-2d | +25% | 1.9-2.5d |
| 5 — LLVM atomics | 2-3d | +30% | 2.6-3.9d |
| 6 — `@naked` | 3-5d | +25% | 3.75-6.25d |
| 7 — `@no_mangle` | 0.5-1d | +25% | 0.6-1.3d |
| 8 — Final validation | 1-2d | +25% | 1.25-2.5d |
| **TOTAL** | **13-21d base** | **+25-30%** | **17-26.5d** |

> Single founder-day = 8 working hours; "day" estimates are Claude effort.
> Total is ~21-32 days realistic at +25-30% surprise.

## 6. Sequencing decision

**Phases 0-4 are mandatory + sequential** (each blocks the next).
**Phase 5 can run in parallel with Phase 6** (different code paths) once Phase 4 closes.
**Phase 6-7 are quality-of-life "polish"** — fajaros becomes 100% Fajar Lang at end of Phase 4 (with inline-asm spinlock from Phase 1 acceptable). Per user signal, NOT optional — committed to ship.
**Phase 8 always last.**

Recommended start: Phase 0 immediately (cheap, ~half day, surfaces hidden constraints).

## 7. Self-check (§6.8 R1-R8)

| Rule | Compliance |
|---|---|
| R1 — Pre-flight audit per Phase | ✅ Phase 0 mandatory; each subsequent phase ships `_FINDINGS.md` |
| R2 — Verification columns runnable commands | ✅ Every sub-task has explicit `cargo`/`make`/`grep`/`diff`/`nm` command |
| R3 — Prevention layer per phase | ✅ Each phase ends with hook/CI gate added |
| R4 — Multi-agent audit cross-check | ⚠️ Marked: cross-checking via Bash in real time during execution; no parallel agent runs needed for this plan |
| R5 — Surprise budget +25% min | ✅ +25-30% across all phases |
| R6 — Decision gates mechanical | ✅ Each phase produces a committed `_FINDINGS.md` whose presence is checked by the next phase's pre-commit hook |
| R7 — Public-facing artifact sync | ✅ Phase 8 explicitly syncs CLAUDE.md, README, GitHub releases, tags |
| R8 — Multi-repo state check | ✅ Phase 0.5 + Phase 8.6 |

**Self-check before commit:** All 8 YES → ship plan.

## 8. Out-of-scope (explicitly NOT in this plan)

These are honest limitations of "100% Fajar Lang FajarOS" as scoped here.
Each could become a future plan if user later requests:

- **F.11 BitNet TL2 vendoring** (135 LOC C++ wrapper.cpp, vendored microsoft/BitNet kernel). Currently OPTIONAL via Cargo path dep; PERMANENT-DEFERRED per memory. Could be replaced with custom AVX2 ternary kernel in fj inline asm (Strategic Option C, 1-2d) — separate Phase F.X plan.
- **Python host-side tooling** (`scripts/*.py`, 12 files, 3,492 LOC). Model export, disk build, kernel-trace parser. Run on host, not in kernel. Could be ported to `.fj` CLI tools (~1-2 weeks).
- **F.6.4 base/medium ablation execution** (fajarquant). Different track, GPU-dependent. Independent of this plan.
- **LLVM upstream O2 vecmat miscompile fix** (A1 filing). Founder external action. Plan resilient with `@no_vectorize` workaround.

---

*FAJAROS_100PCT_FJ_PLAN v1.0 — Created 2026-05-04 in response to user signal
"jangan pernah bilang kapan-kapan ... segera buat plan detail." Aligned with
§6.8 (Plan Hygiene) + §6.6 (Documentation Integrity). Per-phase findings docs
to follow as phases execute. Total realistic effort 21-32 days at +25-30%
surprise budget. Recommended start: Phase 0 audit (~0.6-1.3 day).*
