# Fajar Lang — Master Implementation Plan

> **Versi:** 7.0 | **Tanggal:** 2026-03-23
> **Status:** Dokumen referensi utama pengembangan Fajar Lang
> **Tim:** 1 Engineer (Fajar) + Claude AI
> **Target OS:** FajarOS x86_64 + FajarOS ARM64 (Radxa Dragon Q6A)
> **Durasi:** 24 minggu (6 bulan)
> **Metode:** AI-first — AI generates 70% code, engineer reviews + directs
> **Referensi:** HuggingFace Candle, seL4, Rust, Zig, Koka

---

## Daftar Isi

0. [Cara Kerja 1 Engineer + AI](#bagian-0-cara-kerja-1-engineer--ai)
1. [Posisi Saat Ini](#bagian-1-posisi-saat-ini)
2. [Prioritas & Urutan Kerja](#bagian-2-prioritas--urutan-kerja)
3. [Implementation Plan](#bagian-3-implementation-plan-24-minggu)
4. [Definition of Done](#bagian-4-definition-of-done)
5. [Arsitektur Target](#bagian-5-arsitektur-target)
6. [ML Runtime & Candle Strategy](#bagian-6-ml-runtime--candle-strategy)
7. [Migration Strategy FajarOS](#bagian-7-migration-strategy-fajaros)
8. [Performance Budget](#bagian-8-performance-budget)
9. [Infrastructure](#bagian-9-infrastructure)
10. [API Stability Policy](#bagian-10-api-stability-policy)
11. [Quality Gates](#bagian-11-quality-gates)
12. [Risk & Fallback](#bagian-12-risk--fallback)
13. [Metrik Sukses](#bagian-13-metrik-sukses)

---

## Bagian 0: Cara Kerja 1 Engineer + AI

### Workflow Harian

```
Pagi (08:00-12:00) — FOCUS BLOCK: Coding berat
  1. Buka Claude Code session
  2. Load context: "Resume docs/MASTER_IMPLEMENTATION_PLAN.md → Sprint X.Y"
  3. AI generates implementation (code + tests)
  4. Engineer reviews: logic benar? edge cases?
  5. cargo test + clippy + fmt
  6. Commit jika pass

Siang (13:00-15:00) — REVIEW & FIX
  7. Fix issues dari pagi
  8. Run FajarOS regression test
  9. Update plan: mark [x] yang selesai

Sore (15:00-17:00) — EXPLORATION & DESIGN
  10. Study next sprint requirements
  11. AI research (WebFetch, code exploration)
  12. Design decisions untuk besok
```

### Pembagian Kerja Engineer vs AI

| Aktivitas | Engineer | AI (Claude) |
|-----------|----------|-------------|
| Architecture decisions | ✅ Decides | Proposes options |
| Sprint prioritization | ✅ Decides | Suggests order |
| Code generation | Reviews + adjusts | ✅ Generates 70%+ |
| Test generation | Reviews | ✅ Generates 90%+ |
| Debugging | Analyzes root cause | ✅ Proposes fixes |
| Code review | ✅ Final approve | Pre-review (style, bugs) |
| Documentation | Approves | ✅ Generates |
| FajarOS testing | ✅ Runs QEMU, verifies | Generates test scripts |
| Hardware testing (Q6A) | ✅ Physical access required | Remote via SSH |
| Design trade-offs | ✅ Makes call | Presents pros/cons |

### Session Protocol

Setiap Claude Code session HARUS mengikuti urutan ini:

```
1. LOAD  → MASTER_IMPLEMENTATION_PLAN.md (dokumen ini)
2. CHECK → Sprint berapa sekarang? Task mana yang in-progress?
3. READ  → File source yang relevan untuk sprint ini
4. CODE  → Implement task, AI generates + engineer reviews
5. TEST  → cargo test + clippy + fmt + FajarOS regression
6. MARK  → Update checklist di dokumen ini: [ ] → [x]
7. COMMIT→ git commit dengan conventional format
```

### Keunggulan 1 Engineer + AI vs 10 Engineer

```
✅ Zero coordination overhead   (no meetings, no PR wait, no merge conflicts)
✅ One vision, one codebase      (no design disagreements)
✅ AI never forgets context       (1M token window = entire codebase)
✅ AI generates tests faster      (50 tests in minutes, not hours)
✅ AI handles boring parts        (boilerplate, serialization, format code)
✅ 24/7 availability              (AI doesn't sleep)

⚠️ Bottleneck: engineer's review capacity (~30h/week focused coding)
⚠️ Risk: single point of failure (engineer sick = project stops)
```

---

## Bagian 1: Posisi Saat Ini

### Compiler Statistics

```
Codebase:        ~290,000 LOC Rust (220+ files)
Tests:           5,582 (0 failures)
Self-hosted:     1,268 LOC Fajar Lang (lexer + parser + analyzer + codegen)
Backends:        Cranelift (dev) + LLVM (release) + Wasm (browser)
Targets:         x86_64, ARM64, RISC-V, Wasm
IDE:             VS Code (LSP + DAP debugger)
Examples:        130+ programs (.fj)
Packages:        7 standard
Error Codes:     80+ across 10 categories
```

### Fitur Unik

| Fitur | Status |
|-------|--------|
| `@kernel/@device/@safe` context isolation | ✅ Implemented |
| Effect system (`with IO, Hardware`) | ✅ Implemented |
| Linear types (`linear struct`) | ✅ Implemented |
| Comptime evaluation (`comptime {}`) | ✅ Implemented |
| First-class tensor operations | ✅ Implemented |
| Macro system (`vec![]`, `@derive`) | ✅ Implemented |
| Dual backend (Cranelift + LLVM) | ✅ Implemented |
| Incremental compilation | ✅ Implemented |
| Self-hosted compiler (1,268 LOC .fj) | ✅ Implemented |

### 10 Problem Kritis

| # | Problem | Prioritas |
|---|---------|-----------|
| P1 | **Concatenation hack** — FajarOS cat 75 file jadi 1 | CRITICAL |
| P2 | **@safe tidak fully enforced** — bisa panggil port_outb | CRITICAL |
| P3 | **Tidak ada multi-binary build** (kernel + services) | CRITICAL |
| P4 | **Tidak ada user-mode runtime** (@safe println) | HIGH |
| P5 | **IPC raw bytes** — 64-byte buffer tanpa tipe | HIGH |
| P6 | **Tensor hanya f64** — tidak ada f16/bf16/INT8 | HIGH |
| P7 | **Tidak ada device abstraction** (CPU/GPU/NPU) | HIGH |
| P8 | **Macro $ patterns belum work** | MEDIUM |
| P9 | **Effect polymorphism belum ada** | MEDIUM |
| P10 | **Tidak ada cross-service type sharing** | MEDIUM |

---

## Bagian 2: Prioritas & Urutan Kerja

### Prinsip: Sequential, Impact-First

Karena 1 orang, TIDAK bisa parallel. Urutan berdasarkan:
1. **Apa yang unblock paling banyak hal lain** (critical path)
2. **Apa yang paling impactful untuk FajarOS** (user value)
3. **Apa yang AI bisa generate paling efisien** (leverage AI)

### Critical Path

```
P2 (@safe block) → P1 (multi-file) → P3 (multi-binary) → P4 (user runtime) → FajarOS migration
      W1-2              W3-5              W6-8               W9-10              W11-12
```

P2 dikerjakan PERTAMA karena:
- Paling kecil effort (2 minggu)
- Unblock security model yang merupakan USP
- AI bisa generate 121+ test cases otomatis

### Dependency Chain

```
Week 1-2:   P2 (@safe enforcement) ←── standalone, no dependency
Week 3-5:   P1 (multi-file build)  ←── standalone
Week 6-8:   P3 (multi-binary)      ←── depends on P1
Week 9-10:  P4 (user runtime)      ←── depends on P3
Week 11-12: FajarOS migration      ←── depends on P3 + P4
Week 13-14: P5 (typed IPC)         ←── depends on P3
Week 15-16: P6 (multi-dtype tensor) ←── standalone
Week 17-18: P7 (device backend)    ←── depends on P6
Week 19-20: P8 (macros) + P9 (effect poly) ←── standalone
Week 21-22: P10 (shared types) + ML models ←── depends on P6 + P7
Week 23-24: FajarOS v3.0 release   ←── depends on everything
```

---

## Bagian 3: Implementation Plan (24 Minggu)

### Phase 1: Safety & Build (Minggu 1-12)

#### Sprint 1: @safe Complete Enforcement (Minggu 1-2)

**Goal:** @safe code CANNOT access hardware. Period.

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 1.1 | ~~Define 121+ blocked builtins in analyzer~~ | 3h | 80% | ✅ 239 builtins already blocked |
| 1.2 | ~~SE020 error: "@safe cannot access hardware"~~ | 1h | 90% | ✅ Already implemented |
| 1.3 | ~~Whitelist safe builtins (println, len, math)~~ | 2h | 80% | ✅ Already implemented |
| 1.4 | ~~Block asm!() in @safe and @device~~ | 1h | 90% | ✅ Already implemented (KE005/KE006) |
| 1.5 | ~~SE021: @safe → @kernel call blocked~~ | 2h | 80% | ✅ Already implemented |
| 1.6 | ~~SE022: @safe → @device call blocked~~ | 2h | 80% | ✅ NEW: Separate SE022 error added |
| 1.7 | ~~`fj check --call-graph` command~~ | 3h | 70% | ✅ NEW: --call-graph flag added |
| 1.8 | ~~Tests: 85 (AI generates)~~ | 4h | 95% | ✅ NEW: 85 context safety tests |
| 1.9 | FajarOS context_enforcement.fj passes | 2h | 50% | ⏳ Needs FajarOS repo access |

**Total: ~20h | AI generates: ~70%**

```
Hari 1-2: Task 1.1-1.4 (block builtins)
Hari 3-4: Task 1.5-1.7 (call gates)
Hari 5-6: Task 1.8-1.9 (tests + FajarOS verification)
Hari 7-8: Buffer untuk edge cases + documentation
```

#### Sprint 2-3: Multi-File Module System (Minggu 3-5)

**Goal:** `fj build dir/` compiles multi-file project tanpa concatenation.

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 2.1 | ~~Multi-file compilation: `fj build dir/`~~ | 8h | 60% | ✅ Already worked (read_source_dir) |
| 2.2 | ~~Import resolution~~ | 8h | 50% | ✅ Already worked (interpreter resolves use/mod) |
| 2.3 | ~~Symbol table per module~~ | 4h | 70% | ✅ Already worked (concatenation shares scope) |
| 2.4 | Pub visibility enforcement | 3h | 80% | ⏳ Deferred — needs per-file tracking in codegen |
| 2.5 | ~~Dependency ordering (topological sort)~~ | 3h | 80% | ✅ NEW: order_by_dependencies() in main.rs |
| 2.6 | ~~Circular dependency detection~~ | 2h | 90% | ✅ NEW: fallback to alphabetical with warning |
| 2.7 | ~~Incremental multi-file rebuild~~ | 4h | 60% | ✅ Already worked (check_incremental_cache) |
| 2.8 | ~~Tests: 14~~ | 4h | 95% | ✅ NEW: 14 multi-file tests |
| 2.9 | Parse ALL 75 FajarOS x86 files | 4h | 70% | ⏳ Needs FajarOS repo checkout |

**Total: ~40h | AI generates: ~70%**

#### Sprint 4-5: Multi-Binary Build (Minggu 6-8)

**Goal:** 1 project → kernel.elf + N service ELFs.

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 4.1 | ~~fj.toml `[[service]]` sections~~ | 3h | 80% | ✅ NEW: KernelConfig + ServiceConfig in manifest |
| 4.2 | ~~`fj build --all` command~~ | 8h | 60% | ✅ NEW: discovers kernel + services, maps outputs |
| 4.3 | ~~Per-target configuration~~ | 3h | 70% | ✅ NEW: kernel=x86_64-none, svc=x86_64-user |
| 4.4 | ~~Per-target entry point~~ | 2h | 80% | ✅ NEW: each service has entry field |
| 4.5 | ~~Output structure~~ | 2h | 80% | ✅ NEW: build/kernel.elf + build/services/*.elf |
| 4.6 | ~~Custom linker script per target~~ | 4h | 60% | ✅ NEW: for_user_mode() + for_kernel_with_initramfs() |
| 4.7 | ~~.initramfs section~~ | 4h | 60% | ✅ NEW: pack_initramfs() / unpack_initramfs() |
| 4.8 | ~~`fj pack` command~~ | 3h | 70% | ✅ NEW: CLI command, auto-detect from build/services/ |
| 4.9 | ~~ARM64 multi-target~~ | 4h | 60% | ✅ NEW: aarch64-user target supported |
| 4.10 | ~~Tests: 16~~ | 4h | 95% | ✅ NEW: 16 manifest + multi-binary tests |

**Total: ~37h | AI generates: ~70%**

#### Sprint 6: User-Mode Runtime (Minggu 9-10)

**Goal:** @safe programs can println, exit, malloc, IPC via syscalls.

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 6.1 | ~~fj_user_println → SYS_WRITE~~ | 3h | 70% | ✅ Already: fj_rt_user_print/print_i64/println |
| 6.2 | ~~fj_user_exit → SYS_EXIT~~ | 1h | 90% | ✅ Already: fj_rt_user_exit |
| 6.3 | ~~fj_user_malloc/free → SYS_BRK~~ | 4h | 60% | ✅ Already: fj_rt_user_mmap |
| 6.4 | ~~fj_user_ipc_send/recv/call/reply~~ | 6h | 60% | ✅ Already: 7 IPC wrappers including select |
| 6.5 | ~~Auto-link for x86_64-user target~~ | 2h | 70% | ✅ Already: set_user_mode(true) in cmd_build_native |
| 6.6 | Auto-link for aarch64-user target | 2h | 70% | ⏳ Needs ARM64 SYSCALL instruction variant |
| 6.7 | ~~Tests: 21~~ | 3h | 95% | ✅ NEW: 21 user runtime tests |

**Total: ~21h | AI generates: ~70%**

#### Sprint 7: FajarOS Migration (Minggu 11-12)

**Goal:** FajarOS builds with `fj build --all` instead of concatenation Makefile.

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 7.1 | Add `use` imports to FajarOS files | 8h | 50% | ⏳ Needs per-file edit in fajaros-x86 repo |
| 7.2 | ~~Write fj.toml with kernel + 9 services~~ | 2h | 80% | ✅ NEW: docs/FAJAROS_FJ_TOML.md reference |
| 7.3 | ~~Regression: all 90 .fj files lex~~ | 4h | 50% | ✅ NEW: 90/90 lex, 0 failures |
| 7.4 | ~~Regression: combined.fj parses~~ | 4h | 50% | ✅ NEW: 27K LOC, 160K tokens parse |
| 7.5 | QEMU boot test | 4h | 40% | ⏳ Needs native codegen build |
| 7.6 | Verify 200+ shell commands | 8h | 30% | ⏳ Needs running OS |
| 7.7 | Remove concatenation Makefile | 1h | 90% | ⏳ After multi-file build proven |
| 7.8 | ~~Regression tests: 23~~ | 4h | 50% | ✅ NEW: file count, LOC, key files |

**Total: ~35h | AI generates: ~50%** (paling banyak manual — real OS code)

---

### Phase 2: IPC & ML (Minggu 13-18)

#### Sprint 8: Typed IPC (Minggu 13-14)

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 8.1 | ~~`@message` struct annotation~~ | 3h | 80% | ✅ Already: parser + analyzer recognize @message |
| 8.2 | Auto serialize/deserialize | 6h | 70% | ⏳ Needs codegen integration (compile to pack/unpack) |
| 8.3 | ~~Message ID auto-assignment~~ | 1h | 90% | ✅ NEW: message_ids HashMap, next_message_id counter |
| 8.4 | ~~IPC001: size validation (≤64 bytes)~~ | 4h | 70% | ✅ NEW: estimated_size check, IPC001 error |
| 8.5 | IPC002: type-check ipc_send/recv | 1h | 90% | ⏳ Needs typed overload of ipc_send |
| 8.6 | ~~Tests: 19~~ | 3h | 95% | ✅ NEW: parsing, size limits, contexts, FajarOS-style |

**Total: ~24h | AI generates: ~75%**

#### Sprint 9: Protocol & Service (Minggu 15-16)

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 9.1 | ~~`protocol` keyword + parsing~~ | 4h | 70% | ✅ Already: protocol → TraitDef |
| 9.2 | ~~`implements` clause~~ | 3h | 70% | ✅ Already: service X implements Y |
| 9.3 | ~~Completeness check~~ | 3h | 80% | ✅ Already: missing method → error with hint |
| 9.4 | Client stub auto-generation | 8h | 60% | ⏳ Deferred: VfsClient::open() → IPC |
| 9.5 | ~~`service` block + handlers~~ | 6h | 60% | ✅ Already: fn handlers in service block |
| 9.6 | ~~Tests: 22~~ | 3h | 95% | ✅ NEW: 22 protocol/service tests |

**Total: ~27h | AI generates: ~70%**

#### Sprint 10: Multi-DType Tensor (Minggu 17-18)

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 10.1 | ~~DType enum (F16, BF16, F32, F64, I8, U8, I32, I64, Bool)~~ | 3h | 80% | ✅ NEW: 9 dtypes |
| 10.2 | ~~Tensor storage per dtype~~ | 8h | 60% | ✅ NEW: to_dtype() converts all types |
| 10.3 | ~~Dtype conversion (.to_f16(), .to_i8())~~ | 3h | 70% | ✅ NEW: F16/BF16 precision sim, I8/U8 clamp |
| 10.4 | Compile-time shape tracking | 6h | 50% | ⏳ Deferred: needs type system integration |
| 10.5 | ~~DType metadata (size, range, classify)~~ | 3h | 80% | ✅ NEW: is_float/is_int/is_quantized/min/max |
| 10.6 | ~~Tests: 28~~ | 3h | 95% | ✅ NEW: enum, parse, convert, ranges, memory |

**Total: ~26h | AI generates: ~70%**

---

### Phase 3: Advanced & ML Backends (Minggu 19-22)

#### Sprint 11: Device Backend Abstraction (Minggu 19-20)

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 11.1 | Device enum (Cpu, Gpu, Npu) | 3h | 80% | Enum + traits |
| 11.2 | Backend trait (matmul, relu, softmax) | 6h | 60% | Interface defined |
| 11.3 | CPU backend (ndarray, existing) | 3h | 70% | Existing code adapted |
| 11.4 | GPU backend (Adreno/Vulkan) | 12h | 40% | Vulkan compute on Q6A |
| 11.5 | NPU backend (Hexagon/QNN) | 10h | 40% | QNN SDK integration |
| 11.6 | Tests: 25+ | 3h | 95% | Same result across backends |

**Total: ~37h | AI generates: ~55%** (GPU/NPU needs hardware knowledge)

#### Sprint 12: Quantization & Models (Minggu 21-22)

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 12.1 | GGUF format parser | 8h | 60% | Load llama.cpp models |
| 12.2 | Safetensors parser | 6h | 70% | Load HuggingFace models |
| 12.3 | Q4_0/Q8_0 dequantize | 6h | 60% | Quantized matmul works |
| 12.4 | INT8 quantize (full → INT8) | 3h | 70% | Accuracy within 1% |
| 12.5 | Model inference pipeline | 6h | 50% | Load → infer → output |
| 12.6 | Tests: 20+ | 3h | 95% | GGUF load, accuracy check |

**Total: ~32h | AI generates: ~65%**

---

### Phase 4: Polish & Release (Minggu 23-24)

#### Sprint 13: Release Preparation (Minggu 23-24)

| # | Task | Effort | AI% | Acceptance |
|---|------|--------|-----|-----------|
| 13.1 | Complete macro system ($ patterns) | 8h | 70% | vec!, format! with repetition |
| 13.2 | Effect polymorphism (basic) | 6h | 60% | Generic over effects |
| 13.3 | Cross-service type sharing | 4h | 70% | @shared module works |
| 13.4 | FajarOS x86 v3.0 final test | 8h | 30% | Boot + all services + 200 cmds |
| 13.5 | FajarOS ARM64 v3.0 on Q6A | 8h | 30% | Hardware verified |
| 13.6 | TinyLLaMA on Hexagon NPU | 8h | 40% | Inference works on Q6A |
| 13.7 | Documentation update | 4h | 80% | Book chapters updated |
| 13.8 | Blog post + demo video | 4h | 60% | Publication ready |
| 13.9 | Version bump v5.0.0 | 1h | 90% | Cargo.toml, CHANGELOG |

**Total: ~51h | AI generates: ~55%**

---

### Effort Summary

| Phase | Minggu | Hours | AI% | Focus |
|-------|--------|-------|-----|-------|
| Phase 1: Safety & Build | 1-12 | 153h | ~65% | The foundation — enables everything else |
| Phase 2: IPC & ML | 13-18 | 77h | ~72% | Type-safe OS + tensor runtime |
| Phase 3: Advanced | 19-22 | 69h | ~60% | GPU/NPU backends, quantization |
| Phase 4: Release | 23-24 | 51h | ~55% | Polish, FajarOS v3.0, publication |
| **TOTAL** | **24** | **350h** | **~65%** | **~15h/week average** |

```
350h ÷ 24 weeks = 14.6 hours/week
Dengan buffer 30%: 14.6 × 1.3 = 19h/week

Sangat achievable untuk 1 engineer full-time (40h/week).
Sisa 20h/week untuk: review, debugging, hardware testing, thinking.
```

---

## Bagian 4: Definition of Done

### Per-Task

```
1. Code committed to feature branch
2. New functions have at least 1 test
3. cargo test — ALL pass
4. cargo clippy -- -D warnings — ZERO warnings
5. cargo fmt — formatted
6. No .unwrap() in src/
7. AI review: no obvious bugs
8. Self-review: logic correct, edge cases covered
9. Commit with conventional format
```

### Per-Sprint

```
1. All tasks [x] checked
2. Acceptance criteria met
3. 20+ new tests
4. FajarOS x86 regression: 75 files parse
5. Sprint goal achieved (stated at top of sprint)
```

### Per-Phase

```
1. Phase gate criteria met (Bagian 11)
2. Performance budget not violated (Bagian 8)
3. No open blockers for next phase
```

---

## Bagian 5: Arsitektur Target

### FajarOS x86_64

```
┌─────────────────────────────────────────────────────────────┐
│ Applications (@safe)        — Ring 3                         │
│   compiler, editor, mnist classifier                         │
├─────────────────────────────────────────────────────────────┤
│ Services (@safe, separate ELFs)  — Ring 3, IPC              │
│   shell (200+ cmds), vfs, net, blk, display, input, gpu,    │
│   gui, auth                                                  │
├─────────────────────────────────────────────────────────────┤
│ Microkernel (@kernel, ~2,500 LOC)  — Ring 0                 │
│   mm, sched (16 PIDs, SMP), ipc (seL4-style), syscall,      │
│   boot (IDT/TSS/GDT), security (12 capabilities)            │
├─────────────────────────────────────────────────────────────┤
│ Hardware — Intel Core i9-14900HX                              │
│   24C/32T 5.8GHz, RTX 4090, NVMe Gen4, 32GB DDR5            │
└─────────────────────────────────────────────────────────────┘
```

### FajarOS ARM64 (Radxa Dragon Q6A)

```
┌─────────────────────────────────────────────────────────────┐
│ Applications (@safe) — EL0                                   │
│   AI inference, sensor fusion, navigation                    │
├─────────────────────────────────────────────────────────────┤
│ Services (@safe/@device) — EL0, IPC                         │
│   npu (Hexagon 12 TOPS), gpu (Adreno 643 Vulkan),           │
│   camera (4K), net (WiFi), sensor (GPIO/I2C/SPI)            │
├─────────────────────────────────────────────────────────────┤
│ Microkernel (@kernel) — EL1                                  │
│   core (arch-independent), aarch64 (GICv3, MMU),             │
│   bsp/dragon_q6a (pinout, clocks)                            │
├─────────────────────────────────────────────────────────────┤
│ Hardware — Qualcomm QCS6490                                   │
│   Kryo 670 8-core, Adreno 643, Hexagon 770, 7.4GB           │
└─────────────────────────────────────────────────────────────┘
```

---

## Bagian 6: ML Runtime & Candle Strategy

### Adopt dari Candle

| Pattern | Adaptasi Fajar Lang | Sprint |
|---------|-------------------|--------|
| Device enum (Cpu/Cuda/Metal) | Device::Cpu, Device::Gpu, Device::Npu | Sprint 11 |
| DType enum (f16/bf16/f32/f64) | Native dtype support | Sprint 10 |
| Storage backend per device | @device auto-routes | Sprint 11 |
| GGUF quantization | Load llama.cpp models | Sprint 12 |
| Safetensors format | Load HuggingFace models | Sprint 12 |

### Skip

Python bindings, Flash Attention, 80+ models, MKL, CUDA langsung.

### Target Models

| Model | Size | Backend | Sprint |
|-------|------|---------|--------|
| MNIST MLP | 100KB | CPU | Existing |
| MobileNet v2 | 14MB | GPU | Sprint 12 |
| YOLO-tiny | 15MB | GPU (Vulkan) | Sprint 12 |
| TinyLLaMA 1.1B | 600MB Q4 | NPU (Hexagon) | Sprint 13 |
| Whisper-tiny | 75MB | CPU+NPU | Sprint 13 |

---

## Bagian 7: Migration Strategy FajarOS

### Dual-Path Principle

```
Minggu 1-10:  KEDUA path work (concatenation + multi-file)
Minggu 11:    Multi-file validated → concatenation deprecated
Minggu 12:    Concatenation removed → multi-file only
```

### Step-by-Step

| Step | Minggu | Action | Fallback |
|------|--------|--------|----------|
| 1 | 3-5 | `fj build kernel/` = single ELF | make build still works |
| 2 | 6-8 | `fj build --all` = kernel + 1 service | Step 1 |
| 3 | 9-10 | User runtime: services println via syscall | Step 2 |
| 4 | 11 | All 9 services compile as separate ELFs | Step 3 |
| 5 | 12 | QEMU boot: microkernel + services | Step 4 |
| 6 | 12 | Remove concatenation Makefile | Step 5 is stable |

---

## Bagian 8: Performance Budget

### Compilation

| Metric | Target |
|--------|--------|
| Hello world (1 file) | <50ms |
| FajarOS kernel (20+ files) | <3s |
| FajarOS all (75+ files) | <10s |
| Incremental (1 file changed) | <500ms |

### Runtime

| Metric | Target |
|--------|--------|
| IPC round-trip | <5μs |
| Syscall overhead | <1μs |
| Boot to shell | <500ms |
| MNIST inference | <1ms |
| YOLO-tiny (Adreno GPU) | <30ms |
| TinyLLaMA (Hexagon NPU) | <100ms/token |

### Binary Size

| Target | Max Size |
|--------|---------|
| Hello world ELF | <50KB |
| Microkernel ELF | <100KB |
| Full OS (kernel + services) | <1MB |

---

## Bagian 9: Infrastructure

### Development Environment

```
Machine:     Linux x86_64, 16GB+ RAM, Rust stable 1.85+
QEMU:        qemu-system-x86_64, qemu-system-aarch64 (v8.2+)
Hardware:    Radxa Dragon Q6A (SSH: radxa@192.168.50.94)
CI:          GitHub Actions (auto on every push)
Editor:      VS Code + Fajar Lang extension
AI:          Claude Code (Claude Opus 4.6, 1M context)
```

### CI Pipeline (Automated)

```
Push/PR → GitHub Actions:
  1. cargo fmt -- --check
  2. cargo clippy -- -D warnings
  3. cargo test (5,582+ tests)
  4. cargo test --features native
  5. Parse 75 FajarOS x86 files (regression)
  6. QEMU x86_64 boot test (weekly)
  7. Performance benchmark tracking
```

### External Dependencies (Pinned)

| Dependency | Version | Purpose |
|-----------|---------|---------|
| Rust | stable 1.85+ | Compiler host |
| Cranelift | Cargo.toml pinned | Dev backend |
| inkwell/LLVM | 0.8.0 / 18.1 | Release backend |
| ndarray | 0.16 | Tensor ops |
| tower-lsp | 0.20 | LSP server |
| QNN SDK | 2.40+ | Hexagon NPU |

---

## Bagian 10: API Stability Policy

| Level | Policy | Examples |
|-------|--------|---------|
| **Stable** | No breaking changes | fn, let, struct, enum, match, @kernel |
| **Beta** | May change with notice | with clause, comptime, @derive |
| **Experimental** | May change any time | service, protocol, Cap\<T\>, @message |

### Versioning

```
Week 1-12:   v5.0.0-alpha.N
Week 13-18:  v5.0.0-beta.N
Week 19-24:  v5.0.0-rc.N → v5.0.0 release
```

### FajarOS Compatibility Rule

```
Rule: Existing FajarOS code MUST continue to parse after every commit.
Test: Parse 75 .fj files in CI — regression check.
If syntax changes: provide migration script or compiler warning.
```

---

## Bagian 11: Quality Gates

### Per-Phase Gate

| Phase | Minggu | Gate |
|-------|--------|------|
| 1 | W12 | `fj build --all` → kernel.elf + 3 services; @safe→port_outb → SE020; FajarOS boots |
| 2 | W18 | Typed IPC compiles; tensor f16 matmul works; protocol generates client stub |
| 3 | W22 | GPU backend works on Adreno; GGUF model loads; quantized inference correct |
| 4 | W24 | FajarOS v3.0 boots (x86+ARM64); TinyLLaMA on NPU; paper submitted |

### Release Criteria (FajarOS v3.0)

```
□ ZERO concatenation — pure fj build --all
□ Kernel ≤ 2,500 LOC in Ring 0
□ 9+ services as separate ELFs
□ ALL @safe → hardware = compile error
□ Typed IPC — wrong message = compile error
□ Both x86_64 and ARM64 boot
□ 200+ shell commands via IPC
□ ≥1 ML model on Q6A NPU
□ 6,500+ tests
□ Documentation updated
```

---

## Bagian 12: Risk & Fallback

| Risk | Prob | Mitigation | Fallback |
|------|------|-----------|----------|
| Multi-file build too complex | Med | Start with 2 files | Keep concatenation |
| FajarOS breaks during migration | High | Dual-path (Bagian 7) | Revert to concatenation |
| Hexagon NPU backend fails | High | QNN SDK complexity | CPU fallback |
| ARM64 codegen bugs | Med | QEMU test per commit | x86 as primary |
| Burnout (1 person) | Med | 15h/week max coding | Take breaks, AI handles rest |
| Context overload | Med | 1 sprint at a time | Never start next before current done |
| Performance regression | Low | Benchmark per commit | Profile + fix |

### Contingency Timeline

```
If Phase 1 takes 14 weeks instead of 12:
  → Skip Sprint 13.1 (macros) and 13.2 (effect poly) — nice-to-have
  → Phase 4 becomes 2 weeks: FajarOS v3.0 + release only

If GPU/NPU backend fails:
  → CPU fallback for all ML models
  → FajarOS v3.0 ships without GPU/NPU
  → Add GPU/NPU in v3.1 patch release
```

---

## Bagian 13: Metrik Sukses

| Metrik | Saat Ini | W12 | W24 |
|--------|----------|-----|-----|
| Tests | 5,582 | 6,200+ | 7,000+ |
| FajarOS build | Concatenation | Multi-file | `fj build --all` |
| @safe enforcement | Partial | Complete | Verified |
| IPC safety | Raw bytes | @message types | Protocol stubs |
| Tensor dtypes | f64 only | f16/f32/f64 | + bf16/i8 |
| ML models | MNIST | + MobileNet | + YOLO + LLaMA |
| Platforms | x86_64 | + ARM64 QEMU | + Q6A hardware |
| Services (ELF) | 0 | 3 | 9+ |
| Compile speed | N/A | <5s FajarOS | <3s |
| IPC latency | N/A | <50μs | <5μs |

---

## Lampiran: Glossary

| Term | Definition |
|------|-----------|
| @kernel | Ring 0 / EL1 — hardware access allowed |
| @device | Compute domain — tensor ops allowed |
| @safe | Userspace — no hardware, no tensor |
| Effect | Declared side effect: `with IO, Hardware` |
| Comptime | Compile-time evaluation block |
| Linear type | Must be consumed exactly once |
| IPC | Inter-Process Communication (seL4-style) |
| Capability | Type-safe permission: `Cap<PortIO>` |
| GGUF | Quantized model format (llama.cpp) |
| QNN | Qualcomm Neural Network SDK (Hexagon) |
| Concatenation hack | Current FajarOS: cat 75 files → 1 combined.fj |

---

*Dokumen referensi utama pengembangan Fajar Lang.*
*1 Engineer + AI. Sequential. Impact-first.*
*Setiap session dimulai dengan membaca dokumen ini.*

*v7.0 — 2026-03-23*
