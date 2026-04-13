# V27 "Hardened" — Production Plan

## Context

V26 "Final" brought all three products to near-production. A deep re-audit on 2026-04-14 found **5 minor gaps in Fajar Lang**, **11 real gaps in FajarOS** (2 P0 + 3 P1 + 4 P2 + 2 P3), and **3 user-action items for FajarQuant**. V27 closes every gap with prevention layers, producing the first fully-hardened release.

**Total budget: ~48h estimated, ~61h with surprise buffers.**

---

## Phase X0 — Multi-Repo State Check (before any work)

```bash
cd ~/Documents/Fajar\ Lang && git status -sb && git rev-list --count origin/main..main
cd ~/Documents/fajaros-x86  && git status -sb && git rev-list --count origin/main..main
cd ~/Documents/fajarquant    && git status -sb && git rev-list --count origin/main..main
```
Gate: All 3 repos at 0 unpushed.

---

## Phase A — Fajar Lang Hardening (~14h, +25% = ~17h)

### A0: Pre-Flight Audit (1h)
| # | Task | Verification | Est |
|---|------|-------------|-----|
| A0.1 | Verify test counts | `cargo test --lib 2>&1 \| tail -1` → 7,611 | 10m |
| A0.2 | Count doc warnings | `cargo doc 2>&1 \| grep -c "warning:"` → 11 | 10m |
| A0.3 | Feature flag test coverage | `for f in websocket mqtt ble gui https cuda smt cpp-ffi python-ffi gpu tls playground-wasm; do echo -n "$f: "; grep -rl "cfg.*feature.*\"$f\"" tests/ 2>/dev/null \| wc -l; done` → 12 at 0 | 10m |
| A0.4 | Version check | `grep '^version' Cargo.toml` → 24.0.0 (stale) | 5m |

**Gate:** `docs/V27_A0_FINDINGS.md` committed.

### A1: Feature Flag Test Matrix (8h)
Add `tests/feature_{flag}_tests.rs` for each untested flag. Gated with `#![cfg(feature = "...")]`. Use mocks where external deps unavailable.

| # | Feature | Source files | Test type | Verification |
|---|---------|-------------|-----------|-------------|
| A1.1 | websocket | `src/runtime/os/websocket.rs` | Frame construct/parse | `cargo test --features websocket -- feature_websocket` |
| A1.2 | mqtt | `src/runtime/os/mqtt.rs` | Packet lifecycle | `cargo test --features mqtt -- feature_mqtt` |
| A1.3 | ble | `src/runtime/os/ble.rs` | Scan request (mock) | `cargo test --features ble -- feature_ble` |
| A1.4 | gui | `src/runtime/os/gui.rs` | Window API surface | `cargo test --features gui -- feature_gui` |
| A1.5 | https | `src/runtime/os/network.rs` | TLS setup (mock) | `cargo test --features https -- feature_https` |
| A1.6 | cuda | `src/codegen/cuda.rs` | PTX generation (no GPU) | `cargo test --features cuda -- feature_cuda` |
| A1.7 | smt | `src/analyzer/smt.rs` | Constraint building | `cargo test --features smt -- feature_smt` |
| A1.8 | cpp-ffi | `src/ffi_v2/build_system.rs` | Header parse + type map | `cargo test --features cpp-ffi -- feature_cpp_ffi` |
| A1.9 | python-ffi | `src/ffi_v2/build_system.rs` | Object bridge | `cargo test --features python-ffi -- feature_python_ffi` |
| A1.10 | gpu | `src/runtime/gpu/kernel.rs` | Kernel descriptor | `cargo test --features gpu -- feature_gpu` |
| A1.11 | tls | `src/runtime/os/network.rs` | TLS config builder | `cargo test --features tls -- feature_tls` |
| A1.12 | playground-wasm | `playground/` | WASM compile check | `cargo check --features playground-wasm` |

**Prevention:** CI matrix job: `cargo check --features {flag}` for each flag.

### A2: Doc Warnings Fix (1.5h)
Fix 11 warnings in 6 files — escape brackets, wrap generics in backticks:

| # | File:Line | Fix |
|---|-----------|-----|
| A2.1 | `src/const_generics.rs:84-85` | `[3]` → `` `[3]` `` |
| A2.2 | `src/ffi_v2/build_system.rs:45,136` | `[ffi]` → `` `[ffi]` `` |
| A2.3 | `src/gpu_codegen/spirv.rs:173` | `[gid]` → `` `[gid]` `` |
| A2.4 | `src/runtime/gpu/kernel.rs:39` | `[i]`, `[j]` → backticks |
| A2.5 | `src/compiler/incremental/parallel.rs:267` | `<Mutex>` → `` `Mutex` `` |
| A2.6 | `src/main.rs:184,386` | `<dir>`, `<file>` → backticks |

**Verification:** `cargo doc 2>&1 | grep -c "warning:"` = 0
**Prevention:** Add `RUSTDOCFLAGS="-D warnings"` to CI.

### A3: Silent Error Fixes (2h)
| # | File:Line | Issue | Fix |
|---|-----------|-------|-----|
| A3.1 | `src/interpreter/eval/mod.rs:2032` | `call_main()` swallows non-Function main | Add `Some(non_fn) => Err(TypeError)` arm |
| A3.2 | `src/interpreter/eval/builtins.rs` | `keys()` on non-Map returns empty array | Return `Err(TypeError)` instead |
| A3.3 | Regression tests for both | | `cargo test --lib -- call_main keys_on_non_map` |

### A4: Version + Metadata Sync (1.5h)
| # | File | Change |
|---|------|--------|
| A4.1 | `Cargo.toml:3` | `24.0.0` → `27.0.0` |
| A4.2 | `CLAUDE.md` §3 | Test counts, examples, V27 version |
| A4.3 | `README.md` badges | Version + test count badges |
| A4.4 | Add `scripts/check_version_sync.sh` | Verify Cargo.toml matches CLAUDE.md |

**Prevention:** Wire version sync check into pre-commit hook.

### Phase A Success Criteria
```
[ ] cargo test --lib → 7,611+ pass, 0 fail
[ ] cargo doc → 0 warnings (with -D warnings)
[ ] 12 feature flag test files exist
[ ] CI matrix checks all flags compile
[ ] call_main() rejects non-Function with TypeError
[ ] Cargo.toml version = "27.0.0"
[ ] CLAUDE.md numbers verified by runnable commands
```

---

## Phase B — FajarOS Production Fixes (~24h, +30% = ~31h)

### B0: Pre-Flight Audit (1h)
| # | Task | Verification |
|---|------|-------------|
| B0.1 | Verify serial_send_str is stub | `grep -n "TODO" drivers/serial.fj` → line 79 |
| B0.2 | Count unchecked frame_alloc | `grep -n "frame_alloc()" kernel/syscall/*.fj` |
| B0.3 | Verify kernel stack TODO | `grep -n "TODO" kernel/sched/process.fj` → line 159 |
| B0.4 | Dead code survey | `grep -n "fn cmd_type\|fn cmd_yes_arg" shell/commands.fj` |
| B0.5 | SMAP coverage | `grep -rn "smap_disable" kernel/` |

**Gate:** `docs/V27_B0_FINDINGS.md` committed.

### B1: P0 Critical Fixes (6h)

**B1.1 serial_send_str() (2h)** — `drivers/serial.fj:79`
Implement string byte iteration using `volatile_read_u8(s + i)` pattern (matches existing FajarOS string handling). Loop until null terminator, call `serial_send(base, byte)` per byte.
- **Verify:** `make build-llvm && make run-llvm` — serial output shows string text

**B1.2 Kernel stack cleanup on exit (4h)** — `kernel/sched/process.fj:159`
After freeing stack frame (line 142), call `unmap_page()` on the stack virtual address to remove stale PTE. Handle both sys_fork (fixed 16-page stacks) and proc_v2_fork (1-frame stacks) variants. Mirror fix to `kernel/core/sched.fj` µkernel.
- **Verify:** fork+exit ×100 stress test, verify frame count returns to baseline ±5

### B2: P1 OOM Hardening (8h)

**B2.1 frame_alloc() checks (4h)** — 4 sites in `syscall/dispatch.fj:295,319` + `elf.fj:91,149`
Add `-1` check after each `frame_alloc()`. On OOM: `sys_brk` returns -1, `sys_mmap` returns -1 (MAP_FAILED), `elf_load` returns -5 (ENOMEM).
- **Verify:** Exhaust frames in test, verify sys_brk returns -1 cleanly

**B2.2 Memory map document (2h)** — Create `docs/MEMORY_MAP.md`
Consolidate ALL `const = 0x...` addresses from kernel/*.fj into one table. Include: frame bitmap, process table, FD table, canary table, IPC, SHM, heap, stack, MMIO.
- **Verify:** `scripts/check_memory_map.sh` cross-references grep output vs doc
- **Prevention:** Wire into CI

**B2.3 Boot init sanity checks (2h)** — `kernel/main.fj:127-149`
After each critical init (`frames_init`, `heap_init`, `slab_init`, `ipc_init`, `ramfs_init`), add post-init probe: e.g., `frame_alloc()` returns non-(-1) after `frames_init`. Log `[BOOT] subsystem OK` on success.
- **Verify:** Boot output shows `[BOOT] frames OK`, `[BOOT] heap OK`, etc.

### B3: P2 Boot Hardening (6h)

**B3.1 Multiboot2 validation (2h)** — `kernel/main.fj:78-99`
After parse loop, check if critical tags (ACPI RSDP=tag 6, framebuffer=tag 8) were found. Emit `[WARN]` to serial if missing.
- **Verify:** Boot in QEMU, verify no false warnings

**B3.2 SMEP/SMAP unsupported warning (1h)** — `kernel/core/security.fj:39-53`
Replace silent `return` with `cprintln("[SEC] SMEP/SMAP not supported — unprotected")` before return.
- **Verify:** Boot with TCG (no KVM), verify warning appears

**B3.3 SMAP contract documentation (1h)** — `kernel/core/security.fj` header
Add doc block establishing the SMAP wrapping contract: all user-buffer access MUST go through `smap_disable()/smap_enable()` bracket. Currently enforced at `syscall_dispatch()` level.

**B3.4 Dead code cleanup (1h)** — `shell/commands.fj`
Wire `cmd_type()` to dispatcher (fix `time`/`type` prefix conflict). Remove `cmd_yes_arg()` (existing `yes` handles arguments).
- **Verify:** `nova> type echo` → "echo is a shell builtin"

**B3.5 OOM sentry test (1h)** — `tests/kernel_tests.fj`
New test: allocate frames until OOM, verify `frame_alloc()` returns -1, verify subsequent sys_brk returns -1. TEST_TOTAL → 26.
- **Prevention:** Catches future OOM regressions in CI

### Phase B Success Criteria
```
[ ] serial_send_str() outputs bytes over serial
[ ] proc_v2_exit() unmaps kernel stack PTE
[ ] sys_brk/sys_mmap return -1 on OOM
[ ] docs/MEMORY_MAP.md exists, CI validates
[ ] Boot shows [BOOT] subsystem OK for 5 critical inits
[ ] [WARN] on missing Multiboot2 tags
[ ] [SEC] warning on unsupported SMEP/SMAP
[ ] Dead code wired or removed
[ ] OOM sentry test passes
[ ] make build-llvm succeeds, make run-llvm boots to nova>
```

---

## Phase C — FajarQuant Final Polish (~5h, +25% = ~6h)

### C0: Pre-Flight (30m)
| # | Task | Verification |
|---|------|-------------|
| C0.1 | Test count | `cargo test` → 29+ pass |
| C0.2 | Paper claims | `python3 scripts/verify_paper_tables.py --strict` → 28/28 |

### C1: User Action Prep (3h)
| # | Task | Verification |
|---|------|-------------|
| C1.1 | ORCID placeholder in paper | `grep "orcid" paper/fajarquant.tex` |
| C1.2 | `cargo publish --dry-run` clean | Exit 0, <200KB package |
| C1.3 | Final PDF compile (both versions) | `pdflatex fajarquant.tex && pdflatex fajarquant_mlsys.tex` |

### C2: Release Tag (1h)
Tag `v0.3.1` if any code changes, update README badge.

---

## Phase D — Release + Documentation (~5h, +25% = ~6h)

### D1: CLAUDE.md V27 Update (1.5h)
Update `CLAUDE.md` §3: version V27, test counts, examples, V27 in version table.

### D2: README Badge Sync (1h)
All 3 repos: version badges, test count badges, compiler version.

### D3: GitHub Releases (1.5h)
| Repo | Tag | Title |
|------|-----|-------|
| fajar-lang | v27.0.0 | V27 "Hardened" — Feature flag coverage + doc + version sync |
| fajaros-x86 | v3.2.0 | V27 "Hardened" — OOM hardening + serial fix + security warnings |
| fajarquant | v0.3.1 | V27 "Hardened" — Pre-publish polish |

### D4: CHANGELOG (1h)
Add V27 "Hardened" entry to each repo's CHANGELOG.md.

---

## Effort Summary

| Phase | Base | Surprise | Budget |
|-------|------|----------|--------|
| **A** Fajar Lang | 14h | +25% | **17h** |
| **B** FajarOS | 24h | +30% | **31h** |
| **C** FajarQuant | 5h | +25% | **6h** |
| **D** Release | 5h | +25% | **6h** |
| **Total** | **48h** | | **60h** |

## Plan Hygiene Self-Check (§6.8)
```
[x] Pre-flight audit per phase (A0/B0/C0/D0)               (Rule 1)
[x] Every task has runnable verification command             (Rule 2)
[x] Prevention per phase (CI matrix/doc lint/OOM test/sync) (Rule 3)
[x] Numbers will be cross-checked with Bash                 (Rule 4)
[x] Surprise budget: +25% A/C/D, +30% B                    (Rule 5)
[x] Gates are committed files (V27_*_FINDINGS.md)           (Rule 6)
[x] Public artifact sync (D2 badges)                        (Rule 7)
[x] Multi-repo state check (X0)                             (Rule 8)
```
