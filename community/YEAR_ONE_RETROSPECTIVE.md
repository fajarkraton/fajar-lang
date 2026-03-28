# Fajar Lang: Year One Retrospective

> A look back at the first year of building a systems programming language for embedded ML and OS integration.

---

## Timeline

Fajar Lang's first year was a sprint from concept to a working compiler, runtime, and two operating systems. Here is how it unfolded:

### 2025 Q3-Q4: Foundation (v0.1 -- v1.0)

The project started with a clear thesis: embedded AI systems need a language that enforces domain safety at compile time, not by convention. The first prototype was a tree-walking interpreter written in Rust.

- **v0.1:** Lexer, parser, interpreter. Basic types (i32, f64, bool, str), control flow (if/else, while, for), functions, structs. Enough to run fibonacci and hello world.
- **v1.0 (506 tasks across 6 months / 26 sprints):**
  - Month 1: Semantic analyzer, Cranelift JIT/AOT native compilation, module system, CI.
  - Month 2: Generics with monomorphization, trait system, C FFI interop.
  - Month 3: Move semantics, NLL borrow checker, integer overflow checking, null safety.
  - Month 4: Autograd engine, Conv2d/Attention layers, MNIST training, INT8 quantization.
  - Month 5: ARM64 and RISC-V cross-compilation, no_std bare-metal, HAL traits.
  - Month 6: mdBook documentation (40 pages), 7 standard packages, VS Code extension, release workflows.

By the end of v1.0, the compiler had 1,563 tests and ~45,000 lines of Rust.

### 2026 Q1: Rapid Expansion (v0.2 -- v0.5)

With the core compiler solid, development accelerated into advanced features:

- **v0.2 (49 tasks):** Codegen type system completeness, advanced types, parity and correctness fixes.
- **v0.3 "Dominion" (739 tasks, 52 sprints):** The largest release. Concurrency (threads, channels, mutexes, async/await), inline assembly, volatile I/O, allocators, GPU backends (CUDA + Vulkan), native ML operations, self-hosting (lexer + parser in .fj), and dead code optimization.
- **v0.4 "Sovereignty" (40 tasks, 6 sprints):** Generic enums (Option<T>, Result<T,E>), RAII/Drop trait, Future/Poll, lazy async with state machines and round-robin executor.
- **v0.5 "Ascendancy" (80 tasks, 8 sprints):** Test framework (@test, @should_panic, @ignore), doc generation, trait objects with vtable dispatch, iterator protocol (.map/.filter/.collect), f-string interpolation, multi-error parser recovery.

### 2026 Q1: Operating Systems

Two operating systems were built entirely in Fajar Lang to prove the language's capability:

- **FajarOS Surya (ARM64):** Running on the Radxa Dragon Q6A (QCS6490). MMU, EL0 user space, 10 syscalls, IPC, preemptive scheduler, 65+ shell commands. Verified on real hardware.
- **FajarOS Nova (x86_64):** 20,176 lines of Fajar Lang. 240+ shell commands, preemptive multitasking with timer-driven context switch, Copy-on-Write fork, NVMe + FAT32 + ext2 storage, TCP/IP networking, VirtIO-GPU, multi-user system, ELF loader, GDB remote stub. Verified on QEMU + KVM.

### 2026-03-25: v6.1.0 "Illumination"

The current release. 292,000 lines of Rust, 5,000+ tests, 126 examples, 7 packages. The gap analysis (GAP_ANALYSIS_V2.md) was completed to provide an honest assessment of what is production-ready vs. framework-only.

---

## Key Milestones

| Milestone | Date | Significance |
|-----------|------|-------------|
| First `.fj` program executed | 2025-Q3 | Proof of concept: lexer + parser + interpreter working |
| Cranelift native compilation | 2025-Q4 (S2) | Native code generation; no longer interpreter-only |
| Borrow checker passing | 2025-Q4 (S10) | Memory safety without lifetime annotations |
| MNIST 90%+ accuracy | 2025-Q4 (S16) | ML runtime is real, not just types |
| ARM64 cross-compilation | 2025-Q4 (S18) | Embedded target support |
| Self-hosting lexer+parser | 2026-Q1 (v0.3) | Language can parse itself (bootstrap milestone) |
| FajarOS Surya on Q6A hardware | 2026-03-18 | First real hardware deployment |
| FajarOS Nova booting x86_64 | 2026-Q1 | Full OS in Fajar Lang with 240+ commands |
| GAP_ANALYSIS_V2 audit | 2026-03-26 | Honest reckoning: what's real vs. framework |
| 5,000+ tests, 0 failures | 2026-03-28 | Robust test suite |

---

## What Went Well

### 1. The TDD Discipline Paid Off

Every feature was developed test-first. This sounds obvious but the consistency mattered: 5,000+ tests meant we could refactor aggressively without fear. When the borrow checker was rewritten from scratch (v0.3), the existing test suite caught 14 regressions in the first run. Without TDD, those would have shipped as bugs.

### 2. Context Annotations Were the Right Bet

The `@kernel` / `@device` / `@safe` / `@unsafe` system turned out to be the language's most compelling feature. It solves a real problem that no existing language addresses at compile time. Every conference conversation and every embedded engineer who saw the demo had the same reaction: "Why doesn't my language do this?" The four-context model is simple enough to explain in 2 minutes but powerful enough to prevent entire classes of embedded bugs.

### 3. Cranelift Was the Right Choice for First Backend

Choosing Cranelift over LLVM for the initial native backend was a good decision. Cranelift compiles faster, has a simpler API, and is easier to debug. For an embedded-focused language where compilation speed matters (you don't want to wait 30 seconds to flash a drone), Cranelift's trade-off of slightly less optimized output for much faster compilation was correct. LLVM was added later for cases where maximum optimization matters.

### 4. FajarOS Proved the Language

Building two operating systems in Fajar Lang was the strongest possible validation. It forced the language to handle real-world concerns: interrupt handlers, page tables, DMA buffers, context switching. Every gap in the language was exposed by OS development, and every fix made the language better for all users. FajarOS Nova's 20,000+ lines of .fj code is the best integration test suite the language has.

---

## What Didn't Go Well

### 1. Documentation Inflated Ahead of Implementation

The most significant lesson from year one was the gap between documented features and real implementation. V06 and V07 plans documented ~1,200 tasks, but the gap analysis revealed that roughly 730 of those were framework-only (type definitions with tests, but no actual integration with external systems). This wasn't intentional deception -- it was a gradual drift where "define the types and write unit tests" was counted as "done" when the feature wasn't usable end-to-end.

The fix was GAP_ANALYSIS_V2.md and the introduction of the `[f]` (framework) marker. Going forward, a task is only `[x]` if a user can actually use the feature, not just if a test passes.

### 2. Networking Stack Was Deferred Too Long

The distributed computing module has type definitions for consensus, replication, and service discovery, but no actual TCP/UDP socket implementation. This means features that depend on networking (HTTP server, package registry, distributed training) are framework-only. In hindsight, a basic socket layer should have been prioritized in v0.3 alongside the concurrency primitives.

### 3. Single-Maintainer Risk

The entire project was built by one developer (with AI assistance). This means:
- No code review from a second human perspective.
- Bus factor of 1.
- Community processes (issue triage, PR review, release management) are untested with real external contributors.

The Ambassador program and contributor onboarding documentation are attempts to address this, but the risk remains until there are active external contributors.

---

## Lessons Learned

### 1. Honest Auditing Is Non-Negotiable

The gap analysis was uncomfortable but essential. It would have been easier to keep marking tasks as complete and publishing impressive numbers. Instead, the project now has an honest public audit of every module. This builds trust with potential contributors and users, even though the numbers look less impressive. Transparency is a feature.

### 2. OS Development Is the Best Language Test

No benchmark suite, no fuzzer, no formal verification tool tests a language as thoroughly as building an operating system in it. Every OS subsystem -- scheduler, filesystem, network stack, memory manager -- exercises different language capabilities in ways that synthetic tests cannot replicate. If the goal is a systems language, build a system in it early.

### 3. Scope Management Requires Active Discipline

The project went from "embedded ML language" to "language with two operating systems, GPU backends, formal verification, and a package ecosystem" in one year. Some of that expansion was valuable (FajarOS proved the language). Some of it was premature (formal verification without Z3 integration). The V8 plan addresses this by making "Gap Closure" the mandatory first step before any new features.

### 4. AI-Assisted Development Changes the Calculus

This project would not exist at its current scale without AI assistance. 292,000 lines of Rust, 5,000+ tests, two operating systems, and 44 documentation files in one year is not typical for a single developer. The AI-assisted workflow (Claude Code with CLAUDE.md as persistent context) enabled a development velocity that would normally require a small team. The trade-off is that the code reflects one architect's vision without the friction and diversity of a team. That's a strength for consistency and a weakness for catching blind spots.

---

## Goals for Year Two

### Near-Term (Q2 2026)

1. **Close the gaps.** V8 Option 0 (Gap Closure) addresses the 100 most critical framework-only modules. Real networking, real FFI v2, real crypto with audited implementations.
2. **First external contributors.** Launch the Ambassador program, resolve all `good-first-issue` items, and onboard at least 3 external contributors.
3. **Publish to crates.io.** Make `fj` installable via `cargo install fj-lang`.

### Mid-Term (Q3-Q4 2026)

4. **Production deployment.** At least one external team using Fajar Lang for a real embedded ML project (drone, robot, IoT device).
5. **Formal verification.** Z3 integration for `fj verify` -- real SMT solving, not just type definitions.
6. **Language server maturity.** LSP v3 with full diagnostics, go-to-definition, and refactoring support.

### Long-Term (2027)

7. **Self-hosting compiler.** The Fajar Lang compiler compiles itself. The lexer and parser already self-host; the analyzer and codegen are the remaining challenge.
8. **Industry adoption.** Fajar Lang used in at least one safety-critical domain (automotive, aerospace, medical).
9. **Governance transition.** From single-maintainer to a multi-person core team with defined roles and decision processes.

---

## Acknowledgments

Fajar Lang was built by Muhamad Fajar Putranto (TaxPrime / PrimeCore.id) with AI assistance from Claude (Anthropic). The Radxa Dragon Q6A hardware deployment was made possible by the Radxa team's excellent documentation and community support.

The Rust ecosystem deserves recognition: Cranelift, ndarray, miette, clap, and the dozens of other crates that Fajar Lang depends on are maintained by dedicated open-source contributors. This project stands on their work.

---

*Year One Retrospective v1.0 -- Fajar Lang Project, March 2026*
