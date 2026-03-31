# V14 "Infinity" — Post-Beyond Production Plan

> **Previous:** V13 "Beyond" (8 options, 710 tasks — ALL COMPLETE)
> **Version:** Fajar Lang v11.0.0 → v12.0.0 "Infinity"
> **Goal:** Ship production release, harden, validate in real-world, then add world-first features
> **Scale:** 5 options, 50 sprints, 500 tasks, ~1,200 hours
> **Prerequisite:** V13 complete (verification, distributed, self-hosting, FFI v2, WASI P2)
> **Date:** 2026-04-01
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **STATUS: PENDING**

---

## Motivation

V13 "Beyond" proved Fajar Lang has **world-class toolchain parity** — formal verification, distributed runtime, self-hosting compiler, full FFI, and WASI P2 component model are all production-ready. But **shipping** requires more:

1. **Release & Polish** — Version bump, documentation sync, release artifacts, website update
2. **Production Hardening** — Integration tests, performance regression checks, CI green, security audit
3. **FajarOS Nova v2.0** — Port V13 features to the OS kernel (verification for @kernel, distributed scheduling)
4. **Real-World Validation** — Deploy actual projects: OpenCV FFI, WASI HTTP on Fermyon, distributed MNIST
5. **"Infinity" Features** — Effect system, dependent types, GPU shaders, LSP v4, package registry server

**Production-grade** means: every feature has been tested end-to-end in a real deployment scenario, not just unit tests. A user can `fj new`, write code, `fj build`, `fj test`, `fj deploy` and everything works.

---

## Codebase Audit Summary (Pre-V14)

| Area | LOC | Tests | Files | Status |
|------|-----|-------|-------|--------|
| Core Compiler | ~45,000 | 2,267 | 50+ | 100% production |
| WASI P2 | ~8,000 | 242 | 11 | 100% production |
| FFI v2 | ~12,000 | 284 | 10 | 100% production |
| SMT Verification | ~10,000 | 282 | 10 | 100% production |
| Distributed | ~12,000 | 237 | 10 | 100% production |
| Self-Hosting | ~10,000 | 207 | 10 | 100% production |
| Other (ML, OS, etc.) | ~300,000 | 3,883 | 317 | 100% production |
| **Total** | **~398,000** | **7,402** | **418** | **100%** |

---

## Execution Order & Dependencies

```
PHASE 1 — SHIP (must complete first, validates everything)
  Option 1: Release & Polish ............ 5 sprints,  50 tasks  (NO dependency)
  Option 2: Production Hardening ........ 5 sprints,  50 tasks  (depends on 1)

PHASE 2 — VALIDATE (proves it works in the real world)
  Option 3: FajarOS Nova v2.0 .......... 10 sprints, 100 tasks (depends on 2)
  Option 4: Real-World Validation ....... 10 sprints, 100 tasks (depends on 2)

PHASE 3 — INNOVATE (world-first features, unique differentiation)
  Option 5: "Infinity" Features ......... 20 sprints, 200 tasks (depends on 2)

TOTAL: 50 sprints, 500 tasks, ~60,000 LOC, ~1,000 tests
```

---

## Option 1: Release & Polish (5 sprints, 50 tasks)

### Context

V13 added 710 tasks worth of features, but the release metadata (version, changelog, README, website) hasn't been updated. This option brings all documentation and release artifacts into sync with the actual codebase state.

### Sprint R1: Version Bump & Metadata (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| R1.1 | Bump Cargo.toml to v11.0.0 | Version, description, keywords | 5 | `cargo build` succeeds |
| R1.2 | Update CLAUDE.md to v11.0.0 | Status, test counts, LOC, module list | 100 | Accurate stats |
| R1.3 | Update README.md | Feature list, badges, examples, installation | 200 | Renders correctly |
| R1.4 | Update CHANGELOG.md | v11.0.0 entry with all V13 features | 300 | Complete changelog |
| R1.5 | Update GAP_ANALYSIS_V2.md | All V13 modules marked 100% production | 200 | Honest audit |
| R1.6 | Update docs/FAJAR_LANG_SPEC.md | New syntax (const generics, WIT, FFI) | 150 | Spec matches implementation |
| R1.7 | Update docs/ARCHITECTURE.md | New modules (wasi_p2, ffi_v2, verify, distributed, selfhost) | 100 | Architecture diagram accurate |
| R1.8 | Update docs/STDLIB_SPEC.md | New stdlib additions from V13 | 100 | All builtins documented |
| R1.9 | Update docs/ERROR_CODES.md | New error codes from V13 modules | 80 | Error catalog complete |
| R1.10 | Tag v11.0.0 in git | Create annotated tag with release notes | 0 | Tag exists |

### Sprint R2: Website & Landing Page (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| R2.1 | Update index.html | v11.0.0 features, stats, hero section | 100 | Page loads correctly |
| R2.2 | Feature showcase page | Interactive demos of V13 features | 200 | All demos work |
| R2.3 | Getting started guide | Updated installation + first project | 150 | Guide works end-to-end |
| R2.4 | API documentation | cargo doc + hosted on GitHub Pages | 50 | Docs build and deploy |
| R2.5 | Benchmark page | Performance comparisons (vs Rust, Go, Zig) | 100 | Charts render |
| R2.6 | Blog post: V13 recap | "What's new in Fajar Lang v11.0.0" | 300 | Published |
| R2.7 | Blog post: WASI P2 | "Building WASI Components with Fajar Lang" | 200 | Published |
| R2.8 | Blog post: FFI v2 | "Call C++/Python/Rust from Fajar Lang" | 200 | Published |
| R2.9 | Blog post: Verification | "Formal Verification for Embedded ML" | 200 | Published |
| R2.10 | SEO + social sharing | Open Graph tags, Twitter card, meta description | 30 | Preview correct |

### Sprint R3: VS Code Extension (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| R3.1 | Bump extension to v11.0.0 | package.json version + changelog | 10 | Extension loads |
| R3.2 | Update syntax highlighting | New keywords (const, gen, yield, comptime) | 50 | Highlighting correct |
| R3.3 | Update snippets | Snippets for V13 features (WASI, FFI, verify) | 100 | Snippets trigger |
| R3.4 | LSP feature verification | All 18 LSP features work in VS Code | 0 | Manual test |
| R3.5 | Extension marketplace publish | `npx vsce publish` | 0 | Published |
| R3.6 | JetBrains plugin update | IntelliJ/CLion syntax + LSP | 100 | Plugin works |
| R3.7 | Neovim plugin update | Tree-sitter grammar + LSP config | 80 | Neovim works |
| R3.8 | Zed editor support | Extension for Zed editor | 50 | Zed highlights .fj |
| R3.9 | Helix editor support | Language config for Helix | 30 | Helix works |
| R3.10 | Editor test matrix | Test all 5 editors with sample project | 0 | All editors green |

### Sprint R4: Release Artifacts (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| R4.1 | Linux x86_64 binary | Static release binary | 0 | `./fj --version` = 11.0.0 |
| R4.2 | Linux aarch64 binary | Cross-compiled ARM64 | 0 | Runs on ARM64 |
| R4.3 | macOS x86_64 binary | macOS Intel build | 0 | Runs on macOS Intel |
| R4.4 | macOS aarch64 binary | macOS Apple Silicon | 0 | Runs on M1/M2/M3 |
| R4.5 | Windows x86_64 binary | Windows MSVC build | 0 | Runs on Windows |
| R4.6 | Docker image | `ghcr.io/fajarkraton/fajar-lang:11.0.0` | 30 | Docker run works |
| R4.7 | Homebrew formula | `brew install fajar-lang` | 30 | brew install works |
| R4.8 | GitHub Release | Upload all binaries + release notes | 0 | Download + run works |
| R4.9 | Install script | `curl -fsSL install.fajarlang.dev | sh` | 50 | One-line install works |
| R4.10 | Checksum + signatures | SHA256 + GPG signatures for all artifacts | 0 | Verified |

### Sprint R5: Community & Ecosystem (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| R5.1 | Contributing guide update | CONTRIBUTING.md with V13 workflow | 100 | Clear for newcomers |
| R5.2 | Issue templates | Bug report, feature request, RFC templates | 30 | Templates work |
| R5.3 | Discussion forums | GitHub Discussions enabled + categories | 0 | Forums active |
| R5.4 | Discord/Telegram setup | Community chat channels | 0 | Channels created |
| R5.5 | Example repository | `fajar-lang-examples` repo with 20+ examples | 200 | All examples compile |
| R5.6 | Playground web app | Online REPL (WASI-based) | 300 | Playground works |
| R5.7 | Package registry spec | `registry.fajarlang.dev` API design | 100 | Spec complete |
| R5.8 | First 10 community packages | Math, HTTP, JSON, CSV, CLI, testing, etc. | 0 | Published |
| R5.9 | Roadmap public page | `fajarlang.dev/roadmap` with milestones | 50 | Renders correctly |
| R5.10 | Launch announcement | Blog + Twitter + HN + Reddit | 0 | Posted |

---

## Option 2: Production Hardening (5 sprints, 50 tasks)

### Context

7,402 unit tests pass, but production hardening requires integration tests, fuzz testing, performance benchmarks, security audit, and CI/CD pipeline validation across all platforms.

### Sprint H1: Integration Test Suite (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| H1.1 | End-to-end pipeline tests | Source -> lex -> parse -> analyze -> eval for 50 programs | 500 | All 50 pass |
| H1.2 | Cranelift codegen integration | Compile + execute 30 programs via native | 300 | All 30 pass |
| H1.3 | LLVM codegen integration | Compile + execute 20 programs via LLVM O2 | 200 | All 20 pass |
| H1.4 | VM bytecode integration | Compile + execute 20 programs via VM | 200 | All 20 pass |
| H1.5 | WASI P2 integration | Build + run 10 component programs on wasmtime | 300 | All 10 pass |
| H1.6 | FFI integration | Call C/Python/Rust from 10 .fj programs | 300 | All 10 pass |
| H1.7 | LSP integration | Start server, send 20 requests, verify responses | 200 | All 20 pass |
| H1.8 | CLI integration | Test all CLI commands (run, build, check, test, fmt, lsp, new) | 200 | All commands work |
| H1.9 | Cross-compilation integration | Compile for ARM64, RISC-V, WASM targets | 100 | All targets compile |
| H1.10 | Package system integration | Create, build, publish, install package end-to-end | 200 | Full workflow works |

### Sprint H2: Fuzz Testing (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| H2.1 | Lexer fuzzer | cargo-fuzz on tokenize() | 50 | 10M iterations, no crash |
| H2.2 | Parser fuzzer | cargo-fuzz on parse() | 50 | 10M iterations, no crash |
| H2.3 | Analyzer fuzzer | Fuzz type checker with random ASTs | 50 | No panic in analyzer |
| H2.4 | Interpreter fuzzer | Fuzz eval_source() | 50 | No UB in interpreter |
| H2.5 | WIT parser fuzzer | Fuzz WIT tokenizer + parser | 50 | No crash on random .wit |
| H2.6 | FFI boundary fuzzer | Random types at FFI boundary | 50 | All rejected cleanly |
| H2.7 | Format string fuzzer | Fuzz f-string interpolation | 30 | No injection |
| H2.8 | REPL fuzzer | Random input sequences to REPL | 30 | No hang or crash |
| H2.9 | Macro expansion fuzzer | Random macro inputs | 30 | No infinite loop |
| H2.10 | Fuzz CI integration | Run fuzzers in CI (1 hour budget) | 30 | CI job passes |

### Sprint H3: Performance Benchmarks (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| H3.1 | Compilation speed | Measure LOC/sec for 1K/10K/50K projects | 100 | Baseline recorded |
| H3.2 | Runtime performance | fibonacci, sort, matrix multiply benchmarks | 100 | vs Rust/Go/Zig |
| H3.3 | Memory usage | Peak RSS during compilation | 50 | < 2GB for 50K LOC |
| H3.4 | Binary size | Release binary sizes per target | 30 | Documented |
| H3.5 | Startup time | Cold start + warm start measurements | 30 | < 50ms cold start |
| H3.6 | LSP response time | Completion, hover, diagnostics latency | 50 | < 200ms each |
| H3.7 | Incremental rebuild | Time for single-file change in 50K project | 30 | < 500ms |
| H3.8 | Tensor operations | ML benchmark (MNIST training loop) | 100 | vs PyTorch CPU |
| H3.9 | WASI component size | Hello/HTTP/filesystem component sizes | 30 | < 100KB hello |
| H3.10 | Benchmark dashboard | Criterion HTML + GitHub Pages | 50 | Dashboard accessible |

### Sprint H4: Security Audit (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| H4.1 | Dependency audit | `cargo audit` — no known vulnerabilities | 0 | 0 advisories |
| H4.2 | Unsafe code audit | Every `unsafe` block has SAFETY comment | 0 | All documented |
| H4.3 | Input validation | All user inputs (source, CLI args, config) validated | 100 | No injection |
| H4.4 | Path traversal | File operations can't escape project dir | 50 | Traversal blocked |
| H4.5 | WASI sandbox | Component can't access host beyond grants | 50 | Sandbox enforced |
| H4.6 | FFI boundary safety | No UB at language boundaries | 50 | ASAN clean |
| H4.7 | Denial of service | Compile-time limits (recursion, macro expansion) | 30 | Limits enforced |
| H4.8 | Supply chain | Lock file, checksum verification | 20 | Cargo.lock committed |
| H4.9 | SBOM generation | CycloneDX SBOM for release | 30 | SBOM valid |
| H4.10 | Security policy | SECURITY.md with disclosure process | 30 | Policy published |

### Sprint H5: CI/CD Pipeline (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| H5.1 | Linux CI (x86_64) | Ubuntu 22.04 + 24.04, stable + nightly | 50 | Green |
| H5.2 | macOS CI | macOS 13 + 14, x86_64 + aarch64 | 30 | Green |
| H5.3 | Windows CI | Windows Server 2022, MSVC + GNU | 30 | Green |
| H5.4 | Cross-compilation CI | ARM64 + RISC-V + WASM32 | 30 | Green |
| H5.5 | Feature flag matrix | All 8 feature flags independently | 20 | All compile |
| H5.6 | Coverage reporting | tarpaulin + Codecov upload | 20 | Coverage > 80% |
| H5.7 | Release automation | Tag -> build -> publish workflow | 50 | Automated release |
| H5.8 | Nightly builds | Daily dev builds with full test suite | 20 | Nightly green |
| H5.9 | Benchmark CI | Performance regression detection | 30 | Alert on 10%+ regression |
| H5.10 | Status page | `status.fajarlang.dev` with CI/CD health | 30 | Page shows green |

---

## Option 3: FajarOS Nova v2.0 (10 sprints, 100 tasks)

### Context

FajarOS Nova v1.4.0 "Zenith" is a fully working x86_64 bare-metal OS (21,396 lines, 240+ commands, preemptive multitasking, TCP/IP, GPU, ELF loading). V2.0 integrates V13's formal verification and distributed capabilities into the kernel.

### Sprint N1: Verified @kernel Functions (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| N1.1 | Port SMT verify to kernel | Run `fj verify` on kernel .fj files | 100 | Verifier runs on kernel |
| N1.2 | Verify memory allocator | Prove no double-free, no use-after-free | 150 | Proof passes |
| N1.3 | Verify scheduler | Prove no deadlock, fair scheduling | 150 | Proof passes |
| N1.4 | Verify syscall dispatch | Prove all syscalls return safely | 100 | Proof passes |
| N1.5 | Verify page table ops | Prove no unmapped access | 100 | Proof passes |
| N1.6 | Verify interrupt handlers | Prove no nested lock, bounded time | 100 | Proof passes |
| N1.7 | Verify IPC channels | Prove message ordering, no data loss | 100 | Proof passes |
| N1.8 | Verify CoW fork | Prove refcount correctness | 100 | Proof passes |
| N1.9 | Verify filesystem | Prove journaling consistency | 100 | Proof passes |
| N1.10 | Verification report | Generate DO-178C evidence for kernel | 50 | Report generated |

### Sprint N2: Kernel Optimization (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| N2.1 | LLVM O2 for kernel | Compile kernel with LLVM backend O2 | 100 | Boot succeeds |
| N2.2 | LTO for kernel | Link-time optimization across modules | 50 | Binary smaller |
| N2.3 | Const eval in kernel | Compile-time page tables, GDT | 100 | Const data in .rodata |
| N2.4 | Dead code elimination | Remove unused kernel paths | 30 | Binary 20%+ smaller |
| N2.5 | Inline critical paths | Inline IRQ handlers, syscall dispatch | 50 | Latency reduced |
| N2.6 | Stack usage audit | Verify all kernel stacks < 8KB | 30 | No overflow possible |
| N2.7 | Cache-friendly layout | Struct packing, hot/cold splitting | 80 | Cache misses reduced |
| N2.8 | Zero-copy IPC | Shared memory pages for large messages | 100 | No copy for > 4KB |
| N2.9 | Lock-free data structures | Lock-free queue for IPC, scheduler | 100 | No lock contention |
| N2.10 | Kernel benchmark suite | Boot time, context switch, syscall latency | 100 | Benchmarks documented |

### Sprint N3: Distributed Kernel Services (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| N3.1 | Kernel RPC server | Expose kernel services over network | 150 | Remote syscall works |
| N3.2 | Distributed process spawn | Fork process on remote node | 150 | Process runs remotely |
| N3.3 | Distributed shared memory | Cross-node shared pages | 150 | Reads/writes consistent |
| N3.4 | Cluster-wide PID space | Global PID allocation | 80 | PIDs unique across cluster |
| N3.5 | Network filesystem | NFS-like mount from remote nodes | 150 | File access works |
| N3.6 | Distributed scheduler | Schedule processes across nodes | 100 | Load balanced |
| N3.7 | Cluster health monitor | Heartbeat + failure detection | 80 | Failed node detected |
| N3.8 | Node discovery | mDNS for kernel-to-kernel discovery | 80 | Nodes find each other |
| N3.9 | Secure inter-node comm | TLS for all kernel-to-kernel traffic | 100 | Encrypted |
| N3.10 | Cluster integration test | 4-node QEMU cluster test | 100 | Cluster boots and works |

### Sprint N4: AI-Integrated Kernel (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| N4.1 | In-kernel tensor ops | Basic matmul/add in @kernel context | 100 | Tensor ops work in kernel |
| N4.2 | ML-based scheduler | Neural scheduler predicting task duration | 150 | Better than round-robin |
| N4.3 | Anomaly detection | Detect unusual syscall patterns | 100 | Anomalies flagged |
| N4.4 | Predictive memory | Pre-allocate based on usage patterns | 100 | Fewer page faults |
| N4.5 | QNN integration | Qualcomm QNN inference in kernel (Dragon Q6A) | 150 | Inference works |
| N4.6 | Power management ML | Predict idle periods, optimize power | 100 | Power reduced |
| N4.7 | Network traffic ML | Predict congestion, adjust buffers | 100 | Throughput improved |
| N4.8 | Storage prefetch ML | Predict file access, prefetch blocks | 100 | Latency reduced |
| N4.9 | Security ML | Detect intrusion via syscall analysis | 100 | Intrusion detected |
| N4.10 | AI kernel benchmark | Compare ML-enhanced vs baseline | 100 | Improvements measured |

### Sprint N5: Hardware Abstraction v2 (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| N5.1 | USB 3.0 driver | xHCI controller support | 200 | USB devices enumerated |
| N5.2 | PCIe enumeration v2 | Full BAR mapping, MSI-X | 150 | PCIe devices working |
| N5.3 | AHCI/SATA driver | Native SATA for real hardware | 150 | Disk read/write works |
| N5.4 | Audio driver | HD Audio basic playback | 150 | Sound plays |
| N5.5 | ACPI support | Power management, shutdown, sleep | 150 | `shutdown` command works |
| N5.6 | Multi-monitor | VirtIO-GPU multi-head | 100 | Two displays working |
| N5.7 | Keyboard layout | International keyboard layouts | 50 | Non-US layout works |
| N5.8 | Mouse driver | PS/2 + USB mouse support | 80 | Cursor moves |
| N5.9 | RTC clock | Real-time clock for wall time | 50 | `date` shows correct time |
| N5.10 | Hardware test suite | Test all drivers in QEMU + real HW | 100 | All drivers pass |

### Sprint N6-N10: Network Stack, Userland, GUI, Package Manager, Release

*(10 tasks each — TCP/IP v2, user-space libraries, GUI framework, pkg manager, ISO image release)*

**Sprints N6-N10 follow the same pattern with 10 tasks each:**
- N6: Network stack v2 (WiFi, IPv6, DNS resolver, DHCP client, firewall)
- N7: Userland libraries (libc, libm, libfj, dynamic linking, shared objects)
- N8: GUI framework (window manager, widget toolkit, themes, compositor)
- N9: Package manager (apt-like for FajarOS, repository, dependencies)
- N10: Release (ISO image, installer, boot media, documentation, benchmarks)

---

## Option 4: Real-World Validation (10 sprints, 100 tasks)

### Context

The most important validation is deploying real projects that users would actually build. Each sprint creates a complete, working project that exercises V13 features end-to-end.

### Sprint W1: OpenCV Face Detection (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W1.1 | FFI bindgen for OpenCV | `fj bindgen opencv2/core.hpp` | 100 | Bindings generated |
| W1.2 | Mat type bridge | OpenCV Mat <-> Fajar Tensor | 100 | Data shared |
| W1.3 | Image loading | `cv_imread("photo.jpg")` from Fajar | 50 | Image loaded |
| W1.4 | Haar cascade loader | Load face detection model | 50 | Model loaded |
| W1.5 | Face detection | `detect_faces(img) -> Array<Rect>` | 80 | Faces found |
| W1.6 | Draw rectangles | Annotate detected faces on image | 50 | Rectangles drawn |
| W1.7 | Save result | `cv_imwrite("result.jpg", img)` | 30 | File saved |
| W1.8 | Webcam stream | Live face detection from camera | 100 | Real-time works |
| W1.9 | Performance benchmark | FPS comparison vs Python OpenCV | 30 | Competitive |
| W1.10 | Tutorial + blog post | Step-by-step guide | 200 | Published |

### Sprint W2: WASI HTTP Server on Fermyon (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W2.1 | HTTP router in .fj | REST API with 5 endpoints | 100 | Routes work locally |
| W2.2 | JSON handling | Parse/generate JSON bodies | 80 | Round-trip correct |
| W2.3 | Component build | `fj build --target wasm32-wasi-p2` | 0 | .wasm produced |
| W2.4 | Spin manifest | spin.toml configuration | 20 | `spin up` starts |
| W2.5 | Local testing | Test with `spin up` locally | 0 | All endpoints work |
| W2.6 | Fermyon Cloud deploy | `spin deploy` to production | 0 | Deployed |
| W2.7 | Load testing | 1000 req/sec benchmark | 30 | < 10ms p99 |
| W2.8 | Database integration | Key-value store via WASI | 50 | CRUD works |
| W2.9 | Authentication | JWT-based auth middleware | 80 | Auth works |
| W2.10 | Production monitoring | Logs, metrics, health check | 30 | Observable |

### Sprint W3: Distributed MNIST Training (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| W3.1 | MNIST data loader | Load dataset, split into batches | 100 | 60K train + 10K test |
| W3.2 | CNN model definition | Conv2d + Dense layers in .fj | 80 | Model defined |
| W3.3 | Single-node training | Train to 95%+ accuracy | 50 | Accuracy achieved |
| W3.4 | Data-parallel setup | 4-worker distributed training | 80 | Workers sync |
| W3.5 | Gradient synchronization | AllReduce after each batch | 30 | Gradients averaged |
| W3.6 | Distributed training run | Train on 4 nodes | 0 | Accuracy matches single |
| W3.7 | Checkpoint saving | Save/load distributed checkpoint | 30 | Training resumes |
| W3.8 | Scaling efficiency | Measure 1/2/4 node speedup | 30 | > 3.2x at 4 nodes |
| W3.9 | Mixed precision | FP16 communication, FP32 compute | 30 | 2x comm speedup |
| W3.10 | Tutorial + benchmark | Step-by-step guide + results | 200 | Published |

### Sprint W4-W10: More Real-World Projects

*(10 tasks each — real deployable projects)*
- W4: PyTorch model inference via FFI (load .pt, run inference, compare accuracy)
- W5: Embedded ML on Radxa Dragon Q6A (QNN inference, GPIO, real hardware)
- W6: Rust serde_json interop (parse/generate JSON via Rust FFI)
- W7: WebSocket chat server (WASI component, real-time messaging)
- W8: CLI tool with Fajar (arg parsing, file processing, colored output)
- W9: Database client (PostgreSQL via FFI, connection pooling, migrations)
- W10: Full-stack web app (WASI HTTP backend + static frontend + database)

---

## Option 5: "Infinity" Features (20 sprints, 200 tasks)

### Context

These are features that would make Fajar Lang the **first language** to combine them all in one coherent system. No existing language has algebraic effects + dependent types + GPU shaders + embedded ML in a single unified type system.

### Sub-Option 5A: Effect System (4 sprints, 40 tasks)

**Goal:** Algebraic effects and handlers — controlled side effects without monads.

- Sprint EF1: Effect definition & syntax (`effect IO { fn read() -> str; fn write(s: str) }`)
- Sprint EF2: Effect handlers (`handle io_action { IO.read() => "mock data" }`)
- Sprint EF3: Effect inference (automatically infer which effects a function uses)
- Sprint EF4: Effect polymorphism (`fn map<E>(f: fn(A) -> B / E) -> Array<B> / E`)

### Sub-Option 5B: Dependent Types (4 sprints, 40 tasks)

**Goal:** Types that depend on values — compile-time proof of array bounds, matrix dimensions.

- Sprint DT1: Pi types (`fn zeros(n: Nat) -> Vector<f64, n>`)
- Sprint DT2: Sigma types (dependent pairs: `(n: Nat, Vector<f64, n>)`)
- Sprint DT3: Propositional equality (`proof: n + m == m + n`)
- Sprint DT4: Refinement types (`type Positive = { x: i32 | x > 0 }`)

### Sub-Option 5C: GPU Compute Shaders (4 sprints, 40 tasks)

**Goal:** Write GPU compute kernels in Fajar Lang, compile to SPIR-V/Metal/CUDA.

- Sprint GS1: Shader syntax (`@gpu fn matmul(a: Tensor, b: Tensor) -> Tensor`)
- Sprint GS2: SPIR-V backend (Vulkan compute shaders)
- Sprint GS3: Metal backend (Apple GPU compute)
- Sprint GS4: Auto-dispatch (CPU fallback, GPU when available, multi-GPU)

### Sub-Option 5D: LSP v4 (4 sprints, 40 tasks)

**Goal:** State-of-the-art IDE support rivaling rust-analyzer.

- Sprint LS1: Semantic tokens (full syntax-aware highlighting)
- Sprint LS2: Inlay hints (type annotations, parameter names, lifetime scopes)
- Sprint LS3: Inline values (show const eval results inline in editor)
- Sprint LS4: AI-powered completions (ML-based code completion using local model)

### Sub-Option 5E: Package Registry Server (4 sprints, 40 tasks)

**Goal:** `registry.fajarlang.dev` — a crates.io-like package registry.

- Sprint PR1: Registry API (publish, search, download, versions)
- Sprint PR2: Web frontend (package pages, docs, stats)
- Sprint PR3: CLI integration (`fj publish`, `fj install`, `fj search`)
- Sprint PR4: Security (signing, audit, vulnerability scanning, SBOM)

---

## Summary

| Phase | Options | Sprints | Tasks | LOC |
|-------|---------|---------|-------|-----|
| Phase 1: Ship | 1 (Release) + 2 (Hardening) | 10 | 100 | ~6,000 |
| Phase 2: Validate | 3 (FajarOS) + 4 (Real-World) | 20 | 200 | ~20,000 |
| Phase 3: Innovate | 5 (Infinity Features) | 20 | 200 | ~30,000 |
| **Total** | **5 options** | **50** | **500** | **~56,000** |

**Target:** V14 brings Fajar Lang to v12.0.0 "Infinity" — shipped, hardened, validated, and featuring world-first language innovations.

---

*V14 "Infinity" Plan — Version 1.0 | 2026-04-01 | Fajar (PrimeCore.id) + Claude Opus 4.6*
