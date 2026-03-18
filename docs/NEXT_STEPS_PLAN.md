# Fajar Lang + FajarOS — Next Steps Implementation Plan

> **Date:** 2026-03-18
> **Context:** All major features working. Q6A verified. Session wrap-up tasks.

---

## Step 1: Real MNIST Inference on Q6A (Quick Win)

**Priority:** HIGH — impressive demo, 2 hours
**Repo:** fajar-os + Q6A device

### Background
Current MNIST test uses blank image (all zeros → uniform output). Need real
handwritten digit images to prove the model actually classifies correctly.

### Tasks
| # | Task | Status |
|---|------|--------|
| 1.1 | Upload MNIST test samples (.raw files) from host models/ directory | [ ] |
| 1.2 | Run qnn-net-run on 10 different digit images (0-9) | [ ] |
| 1.3 | Parse outputs, verify correct classification for each | [ ] |
| 1.4 | Write Fajar Lang program that calls qnn inference and prints result | [ ] |
| 1.5 | Benchmark: time per inference on CPU backend | [ ] |
| 1.6 | Try GPU backend (libQnnGpu.so) for comparison | [ ] |
| 1.7 | Document results in Q6A_VERIFICATION_LOG.md | [ ] |

### Success Criteria
- 8/10+ correct digit predictions from MNIST model
- Inference time measured (CPU vs GPU)
- Fajar Lang program demonstrates NPU integration

---

## Step 2: EL0 User Space (Architecture Milestone)

**Priority:** HIGH — transforms FajarOS architecture, ~8 hours
**Repo:** fajar-os + fajar-lang (minor builtins)

### Background
Currently all processes run at EL1 (kernel privilege). Real OS runs user
processes at EL0 (unprivileged). Syscalls via SVC from EL0 trap to EL1.

ARM64 Exception Levels:
- EL0: User space (unprivileged, no hardware access)
- EL1: Kernel (privileged, MMU/IRQ/MMIO control)

### Architecture
```
EL0 (User)          EL1 (Kernel)
┌────────────┐      ┌──────────────────┐
│ Process A  │─SVC─→│ Syscall dispatch │
│ Process B  │─SVC─→│ Scheduler        │
│ Process C  │─SVC─→│ IRQ handler      │
└────────────┘←eret─└──────────────────┘
     ↑                      │
     └── Timer IRQ (EL0→EL1)│
```

### Tasks
| # | Task | Status |
|---|------|--------|
| 2.1 | **Create EL0 process entry stub** | `eret` with SPSR.M[3:0]=0000 (EL0t). Set ELR to process entry. Stack at user address. | [ ] |
| 2.2 | **Separate kernel/user page tables** | TTBR0=user (0x0-0x3F), TTBR1=kernel (0xFFFF...). Or: keep TTBR0 with split VA. | [ ] |
| 2.3 | **Handle SVC from EL0** | `__exc_sync_lower` already exists in vector table. Verify ESR.EC=0x15 from EL0 works. | [ ] |
| 2.4 | **Handle IRQ from EL0** | `__exc_irq_lower` already exists. Verify timer preemption from EL0 works. | [ ] |
| 2.5 | **Kernel stack per process** | Each process needs separate kernel stack (for exception handling while in EL0). Store in process table. | [ ] |
| 2.6 | **SP_EL0 save/restore** | Save user SP to SP_EL0 on exception entry. Restore on eret. | [ ] |
| 2.7 | **Block direct MMIO from EL0** | User process accessing UART/GIC should fault (page table AP bits). | [ ] |
| 2.8 | **Test: EL0 process prints via SVC** | Process at EL0 calls svc(1, 'U', 0) → kernel prints 'U' → eret to EL0. | [ ] |
| 2.9 | **Test: Timer preempts EL0 process** | Process runs at EL0, timer fires, scheduler switches, process resumes. | [ ] |
| 2.10 | **Test: EL0 cannot access kernel memory** | Process at EL0 tries to read 0x47000000 → data abort → kernel handles. | [ ] |

### Technical Notes
- **SPSR for EL0:** Set M[3:0]=0b0000 (EL0t), DAIF=0 (IRQs enabled)
- **SP_EL0:** ARM64 has separate SP for EL0. On exception entry to EL1, SP automatically switches to SP_EL1. Need to save/restore SP_EL0 in process table.
- **Vector table:** `__exc_sync_lower` and `__exc_irq_lower` handle exceptions from EL0. These already exist in linker.rs but call the same handlers.
- **Page table AP bits:** AP[2:1]=01 means RW at EL1, no access at EL0. AP[2:1]=00 means RW at both EL0 and EL1.

### Success Criteria
- User process runs at EL0 (verified by reading CurrentEL)
- SVC from EL0 works (print via syscall)
- Timer preempts EL0 processes
- EL0 process cannot access kernel memory

---

## Step 3: Compiler Sprint 4 — Labeled Break/Continue

**Priority:** MEDIUM — code clarity, ~4 hours
**Repo:** fajar-lang

### Tasks
| # | Task | Status |
|---|------|--------|
| 3.1 | Add `label: Option<String>` to Break/Continue AST nodes | [x] |
| 3.2 | Parse `'name: while/loop/for` syntax (labeled loops) | [x] |
| 3.3 | Parse `break 'name` and `continue 'name` | [x] |
| 3.4 | Codegen: track label→Block mapping in loop stack | [x] |
| 3.5 | Codegen: `break 'outer` → jump to outer loop's exit block | [x] |
| 3.6 | Codegen: `continue 'outer` → jump to outer loop's header block | [x] |
| 3.7 | Test: nested loop with labeled break | [x] |
| 3.8 | Test: labeled continue | [x] |
| 3.9 | Verify all 5,947 tests pass | [x] |

### Success Criteria
- `'outer: while a { while b { break 'outer } }` works
- `'scan: while ... { continue 'scan }` works
- FajarOS scheduler can use labeled break

---

## Step 4: Fajar Lang v3.1 Release

**Priority:** LOW — ship it, ~1 hour
**Repo:** fajar-lang

### Tasks
| # | Task | Status |
|---|------|--------|
| 4.1 | Version bump: Cargo.toml → 3.1.0 | [x] |
| 4.2 | Update CHANGELOG.md with all session achievements | [x] |
| 4.3 | Update CLAUDE.md with FajarOS status | [x] |
| 4.4 | Git tag: v3.1.1 | [x] |
| 4.5 | Build release binaries (x86_64: 6.5MB, aarch64: 5.7MB) | [x] |
| 4.6 | GitHub release: github.com/fajarkraton/fajar-lang/releases/tag/v3.1.1 | [x] |

### Release Notes Highlights
- 90+ bare-metal runtime functions
- String literals in @kernel (`println("text")`)
- Return fix (value + void in bare-metal)
- SPSR save/restore in exception frames
- Sequential SVC fix (direct ELR advance)
- Per-process page tables (TTBR0 switch)
- 12 module splits (27K LOC refactored)
- FajarOS verified on Radxa Dragon Q6A
- Labeled break/continue (`'outer: while ... { break 'outer }`)
- Const folding (compile-time evaluation of constant expressions)
- @kernel codegen enforcement (compiler rejects heap/tensor ops in @kernel context)

---

## Execution Order

```
Step 1: Real MNIST on Q6A          ← NOW (quick win, impressive)
Step 2: EL0 User Space             ← next session (big architecture)
Step 3: Labeled break/continue     ← compiler polish
Step 4: v3.1 Release               ← ship everything
```

---

*Plan created 2026-03-18 by Claude Opus 4.6*
