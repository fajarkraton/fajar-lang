# Fajar Lang + FajarOS — Implementation Plan V5

> **Date:** 2026-03-25
> **Status:** Post Plan V3 (240/268) + V4 (all options done except B)
> **Current:** Fajar Lang v6.1.0, FajarOS Nova v2.0.0, fajaros-x86 v2.0.0
> **Total completed this mega-session:** ~500 tasks

---

## What's Been Achieved

| Version | Milestone |
|---------|-----------|
| v5.5.0 "Illumination" | async/await, patterns, traits, macros |
| v6.0.0 "Absolute" | 22 array methods, Nova v1.0 (50 syscalls), fuzz 2.3M/0 crash |
| v6.1.0 | 21 integration tests, 923 total eval_tests |
| fajaros-x86 v2.0.0 | 139 modules, 37K LOC, SMP v2, demand paging, POSIX |

---

## Next Phase Options

| # | Option | Sprints | Tasks | Effort | Description |
|---|--------|---------|-------|--------|-------------|
| 1 | **Self-Hosting Compiler v2** | 10 | 100 | ~20 hrs | Write Fajar Lang compiler in Fajar Lang |
| 2 | **GPU Compute Backend** | 6 | 60 | ~12 hrs | wgpu/Vulkan backend for tensor ops |
| 3 | **Package Registry** | 4 | 40 | ~8 hrs | Online registry, `fj publish`, dependency resolution |
| 4 | **Fajar Lang v0.9** | 8 | 80 | ~16 hrs | GATs, effect system, comptime, SIMD intrinsics |
| 5 | **Q6A Full Deploy** | 3 | 28 | ~6 hrs | Deploy all v6.1.0 features to Dragon Q6A |
| 6 | **Nova v2.0 "Phoenix"** | 14 | 140 | ~28 hrs | GUI, audio, real persistence, POSIX compliance |
| 7 | **Education Platform** | 4 | 40 | ~8 hrs | Interactive tutorial, playground, course material |
| 8 | **Benchmarks Suite** | 3 | 30 | ~6 hrs | Formal benchmarks vs Rust/C/Python/Zig |
| **Total** | | **52** | **518** | **~104 hrs** | |

---

## Option 1: Self-Hosting Compiler v2 (10 sprints, 100 tasks)

**Goal:** Write core Fajar Lang compiler (lexer + parser + codegen) in Fajar Lang itself
**Milestone:** `fj` binary compiled by `fj` — full bootstrap

### Phases
- Phase S1: Lexer in .fj (2 sprints) — tokenize() reimplemented
- Phase S2: Parser in .fj (3 sprints) — recursive descent + Pratt
- Phase S3: IR Generation (2 sprints) — simple bytecode or C output
- Phase S4: Bootstrap (2 sprints) — compile fj with fj, verify identical output
- Phase S5: Optimization (1 sprint) — constant folding, dead code elimination

---

## Option 2: GPU Compute Backend (6 sprints, 60 tasks)

**Goal:** Native GPU execution for tensor operations via wgpu/Vulkan
**Milestone:** `tensor_matmul()` runs on GPU with 10-100x speedup

### Phases
- Phase G1: wgpu abstraction (2 sprints) — device init, buffer, compute pipeline
- Phase G2: WGSL kernels (2 sprints) — matmul, conv2d, attention, elementwise
- Phase G3: Auto-dispatch (1 sprint) — CPU vs GPU based on tensor size
- Phase G4: Benchmarks (1 sprint) — compare CPU vs GPU vs NPU (Q6A)

---

## Option 3: Package Registry (4 sprints, 40 tasks)

**Goal:** `fj publish` → online registry, `fj add pkg` → dependency resolution
**Milestone:** Working registry with 10+ community packages

### Phases
- Phase P1: Registry server (1 sprint) — REST API, S3 storage, SQLite index
- Phase P2: CLI integration (1 sprint) — `fj publish`, `fj add`, `fj update`
- Phase P3: Dependency resolution (1 sprint) — PubGrub solver, lockfile
- Phase P4: Security (1 sprint) — signing, checksums, yanking

---

## Option 4: Fajar Lang v0.9 (8 sprints, 80 tasks)

**Goal:** Advanced type system + performance features

### Phases
- Phase T1: Generic Associated Types (2 sprints) — `type Item<'a>`
- Phase T2: Effect System (2 sprints) — `fn foo() -> T ! E` (checked effects)
- Phase T3: Comptime (2 sprints) — compile-time evaluation, const generics
- Phase T4: SIMD Intrinsics (2 sprints) — `@simd fn add4(a: f32x4, b: f32x4)`

---

## Option 5: Q6A Full Deploy (3 sprints, 28 tasks)

**Status:** BLOCKED until Q6A board online
**Goal:** Deploy v6.1.0 with all new features to Radxa Dragon Q6A

---

## Option 6: Nova v2.0 "Phoenix" (14 sprints, 140 tasks)

**Goal:** Next-generation FajarOS with GUI, audio, real persistence

### Phases
- Phase N1: GUI Framework (4 sprints) — window manager, widgets, mouse input
- Phase N2: Audio Driver (2 sprints) — Intel HDA, PCM playback
- Phase N3: Real Persistence (3 sprints) — ext2 with full journaling, boot from disk
- Phase N4: POSIX v2 (3 sprints) — mmap file-backed, select/poll, pipes v3
- Phase N5: Networking v4 (2 sprints) — DHCP v2, NTP, multicast

---

## Recommended Order

```
Quick wins:
  5 → Q6A Deploy (when board available)

Language evolution:
  4 → v0.9 (GATs, effects, comptime, SIMD)
  1 → Self-hosting (ultimate validation)

Ecosystem:
  3 → Package registry (community growth)
  7 → Education (adoption)

Performance:
  2 → GPU backend (tensor acceleration)
  8 → Benchmarks (competitive positioning)

OS:
  6 → Nova v2.0 (GUI, audio, persistence)
```

---

*Plan V5 — 52 sprints, 518 tasks, ~104 hours*
*Fajar Lang v6.1.0 + FajarOS Nova v2.0.0*
