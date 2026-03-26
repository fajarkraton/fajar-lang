# Plan V6 "Dominance" — Next Implementation Plan

> **Date:** 2026-03-26
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Prerequisite:** Plan V5 complete (490/518 tasks, 94.6%)
> **Scope:** 8 options, 560 tasks, 56 sprints
> **Estimated Effort:** ~112 hours total (~14 hrs per option average)

---

## Current State (Post-V5)

```
Fajar Lang:    v5.5.0 "Illumination" — 6,286 tests, ~290K LOC, 161 examples
FajarOS Nova:  v2.0 "Phoenix" — GUI, audio, ext2 journal, POSIX v2, networking v4
FajarOS ARM:   v3.0 "Surya" — 112/420 tasks (microkernel on Dragon Q6A)
Hardware:      RTX 4090, Dragon Q6A (QCS6490), QEMU x86_64/aarch64/riscv64
Ecosystem:     7 packages, VS Code extension, LSP, mdBook docs, 10 tutorials
Compiler:      Cranelift JIT/AOT + LLVM backend + Wasm target
```

## Option Summary

| # | Option | Sprints | Tasks | Effort | Focus |
|---|--------|---------|-------|--------|-------|
| 1 | Language Playground & Web IDE | 6 | 60 | ~12 hrs | Developer experience |
| 2 | Profiler & Time-Travel Debugger | 8 | 80 | ~16 hrs | Observability |
| 3 | FajarOS Nova v3.0 "Aurora" | 12 | 120 | ~24 hrs | OS evolution |
| 4 | Real-Time ML Pipeline | 6 | 60 | ~12 hrs | Core differentiator |
| 5 | FFI v2 — C++/Python/Rust Interop | 5 | 50 | ~10 hrs | Ecosystem integration |
| 6 | Standard Library v3 | 8 | 80 | ~16 hrs | Language completeness |
| 7 | Language Server v3 | 5 | 50 | ~10 hrs | IDE experience |
| 8 | Formal Verification v2 | 6 | 60 | ~12 hrs | Safety guarantee |
| **Total** | | **56** | **560** | **~112 hrs** | |

**Recommended order:** 1 → 7 → 6 → 4 → 3 → 2 → 5 → 8

---

## Option 1: Language Playground & Web IDE (6 sprints, 60 tasks) ✅ COMPLETE

**Goal:** Browser-based Fajar Lang playground — write, run, share code instantly
**Impact:** Lowest barrier to entry; anyone can try Fajar Lang without installing anything

### Phase P1: Wasm Runtime (2 sprints, 20 tasks)

#### Sprint P1.1: Interpreter-in-Browser (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P1.1.1 | Compile interpreter to Wasm | `wasm-pack build` with `wasm-bindgen` | [x] |
| P1.1.2 | JavaScript bridge | `run_fajar(source: &str) -> String` exported | [x] |
| P1.1.3 | stdout/stderr capture | Redirect `println` to JS callback | [x] |
| P1.1.4 | Error formatting | miette-style errors as HTML/ANSI | [x] |
| P1.1.5 | Execution timeout | Web Worker with 5-second kill | [x] |
| P1.1.6 | Memory limit | Wasm linear memory cap (64MB) | [x] |
| P1.1.7 | AST dump mode | `dump-ast` as JSON for visualization | [x] |
| P1.1.8 | Token dump mode | `dump-tokens` as JSON | [x] |
| P1.1.9 | Type check mode | `check` returns diagnostics as JSON | [x] |
| P1.1.10 | Wasm size optimization | `wasm-opt -Oz`, strip debug, < 2MB | [x] |

#### Sprint P1.2: Web Worker Sandbox (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P1.2.1 | Web Worker executor | Run Wasm in dedicated worker thread | [x] |
| P1.2.2 | Message protocol | `{type: "run", source: "..."}` → `{type: "output", text: "..."}` | [x] |
| P1.2.3 | Cancellation | `worker.terminate()` on timeout or user cancel | [x] |
| P1.2.4 | Progress events | Stream output line-by-line | [x] |
| P1.2.5 | File system mock | In-memory VFS for `read_file`/`write_file` | [x] |
| P1.2.6 | Import resolution | `use std::math` → bundled stdlib | [x] |
| P1.2.7 | Tensor operations | ndarray subset compiled to Wasm | [x] |
| P1.2.8 | Random seed control | Deterministic `randn` for reproducibility | [x] |
| P1.2.9 | Performance metrics | Execution time, memory used, tokens parsed | [x] |
| P1.2.10 | Error recovery | Worker crash → graceful restart | [x] |

### Phase P2: Editor UI (2 sprints, 20 tasks)

#### Sprint P2.1: Monaco Editor Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P2.1.1 | Monaco editor setup | React + Monaco with Fajar Lang mode | [x] |
| P2.1.2 | Syntax highlighting | TextMate grammar for `.fj` | [x] |
| P2.1.3 | Auto-completion | Keywords, builtins, std library | [x] |
| P2.1.4 | Error markers | Red squiggles from type checker | [x] |
| P2.1.5 | Output panel | Scrollable output with ANSI colors | [x] |
| P2.1.6 | Run button | Execute with keyboard shortcut (Ctrl+Enter) | [x] |
| P2.1.7 | Theme toggle | Light/dark theme matching FajarOS colors | [x] |
| P2.1.8 | Font configuration | Monospace, size 14px default | [x] |
| P2.1.9 | Mobile responsive | Usable on tablet (stack layout) | [x] |
| P2.1.10 | Keyboard shortcuts | Ctrl+S save, Ctrl+Enter run, Ctrl+/ comment | [x] |

#### Sprint P2.2: Sharing & Examples (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P2.2.1 | URL hash encoding | Source code in URL fragment (LZ-compressed) | [x] |
| P2.2.2 | Share button | Copy shareable link to clipboard | [x] |
| P2.2.3 | Example gallery | 20 curated examples from tutorials | [x] |
| P2.2.4 | Example categories | Basics, ML, OS, Algorithms, Embedded | [x] |
| P2.2.5 | GitHub Gist export | One-click save to Gist | [x] |
| P2.2.6 | Embed mode | `<iframe>` snippet for blogs/docs | [x] |
| P2.2.7 | QR code | Mobile-friendly sharing | [x] |
| P2.2.8 | Version selector | Switch between Fajar Lang versions | [x] |
| P2.2.9 | Permalink API | `playground.fajarlang.dev/p/{id}` | [x] |
| P2.2.10 | Social meta tags | OpenGraph preview when sharing links | [x] |

### Phase P3: Deployment (2 sprints, 20 tasks)

#### Sprint P3.1: Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P3.1.1 | Static site build | Vite/Next.js SSG → Cloudflare Pages | [x] |
| P3.1.2 | CDN deployment | Global edge with <50ms TTFB | [x] |
| P3.1.3 | Service worker cache | Offline playground support | [x] |
| P3.1.4 | Analytics | Plausible (privacy-friendly) page views | [x] |
| P3.1.5 | Error reporting | Sentry for JS/Wasm crashes | [x] |
| P3.1.6 | CI/CD pipeline | Auto-deploy on main push | [x] |
| P3.1.7 | Custom domain | playground.fajarlang.dev | [x] |
| P3.1.8 | SSL certificate | Cloudflare auto-SSL | [x] |
| P3.1.9 | Rate limiting | 100 executions/minute per IP | [x] |
| P3.1.10 | Health check | `/api/health` endpoint | [x] |

#### Sprint P3.2: Polish & Launch (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P3.2.1 | Landing page | Hero section with live code demo | [x] |
| P3.2.2 | Documentation link | "Learn Fajar Lang" → tutorials | [x] |
| P3.2.3 | GitHub link | Star button + repo link | [x] |
| P3.2.4 | Logo & favicon | FajarOS "sunrise" branding | [x] |
| P3.2.5 | SEO optimization | Title, description, structured data | [x] |
| P3.2.6 | Accessibility | ARIA labels, keyboard navigation, screen reader | [x] |
| P3.2.7 | Browser compatibility | Chrome, Firefox, Safari, Edge | [x] |
| P3.2.8 | Loading spinner | Wasm download progress indicator | [x] |
| P3.2.9 | Changelog modal | "What's new" on version update | [x] |
| P3.2.10 | Blog: "Try Fajar Lang" | Launch announcement with playground link | [x] |

---

## Option 2: Profiler & Time-Travel Debugger (8 sprints, 80 tasks) ✅ COMPLETE

**Goal:** Production-grade profiling + reverse debugging for embedded ML code
**Impact:** Debug tensor shape mismatches, memory leaks, and performance bottlenecks

### Phase D1: Profiler (3 sprints, 30 tasks)

#### Sprint D1.1: Instrumentation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1.1 | Function entry/exit hooks | Timestamp + call depth tracking | [x] |
| D1.1.2 | Call graph builder | Parent→child edges with call counts | [x] |
| D1.1.3 | Execution time per function | Wall clock + CPU time | [x] |
| D1.1.4 | Memory allocation tracking | alloc/free pairs with sizes | [x] |
| D1.1.5 | Tensor operation profiling | Shape, dtype, compute time per op | [x] |
| D1.1.6 | Hot path detection | Top 10 functions by cumulative time | [x] |
| D1.1.7 | Loop iteration counting | Iterations per loop with avg time | [x] |
| D1.1.8 | Branch prediction stats | if/else taken ratio | [x] |
| D1.1.9 | Sampling profiler | Statistical sampling at 1kHz | [x] |
| D1.1.10 | Profile data format | Chrome Trace Event JSON | [x] |

#### Sprint D1.2: Flamegraph Generation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.2.1 | Stack frame capture | Collapsed stack format | [x] |
| D1.2.2 | SVG flamegraph renderer | Interactive zoom/filter | [x] |
| D1.2.3 | Differential flamegraph | Compare two runs (red/blue) | [x] |
| D1.2.4 | Reverse flamegraph | Callee-first (icicle chart) | [x] |
| D1.2.5 | Memory flamegraph | Allocation-weighted stacks | [x] |
| D1.2.6 | `fj profile` CLI command | `fj profile run examples/mnist.fj` | [x] |
| D1.2.7 | HTML report generation | Self-contained single-file report | [x] |
| D1.2.8 | Threshold filtering | Hide functions < 1% total time | [x] |
| D1.2.9 | Source annotation | Click flamegraph → jump to source | [x] |
| D1.2.10 | JSON export | For CI/CD regression tracking | [x] |

#### Sprint D1.3: Memory Profiler (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.3.1 | Allocation timeline | Heap size over time | [x] |
| D1.3.2 | Leak detection | Unreachable allocations at exit | [x] |
| D1.3.3 | Allocation site tracking | File:line for each alloc | [x] |
| D1.3.4 | Peak memory analysis | High-water mark + contributing allocs | [x] |
| D1.3.5 | Tensor memory tracking | Tensor count, total bytes, peak | [x] |
| D1.3.6 | Fragmentation analysis | Free block distribution | [x] |
| D1.3.7 | Object graph dump | Reference chains for leak diagnosis | [x] |
| D1.3.8 | `fj memprof` CLI | `fj memprof run program.fj` | [x] |
| D1.3.9 | GC pressure metrics | Allocation rate, collection frequency | [x] |
| D1.3.10 | Valgrind-style report | "Definitely lost", "possibly lost" | [x] |

### Phase D2: Time-Travel Debugger (3 sprints, 30 tasks)

#### Sprint D2.1: Execution Recording (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1.1 | State snapshot on each statement | Variable values + PC | [x] |
| D2.1.2 | Snapshot compression | Delta encoding (only changed vars) | [x] |
| D2.1.3 | Circular buffer (1M snapshots) | Ring buffer with configurable depth | [x] |
| D2.1.4 | Checkpoint system | Full snapshot every N statements | [x] |
| D2.1.5 | Replay engine | Forward replay from checkpoint | [x] |
| D2.1.6 | `step-back` command | Reverse single statement | [x] |
| D2.1.7 | `reverse-continue` | Run backwards to previous breakpoint | [x] |
| D2.1.8 | `reverse-next` | Step back over function calls | [x] |
| D2.1.9 | Watchpoint trigger history | "When did x change to 42?" | [x] |
| D2.1.10 | Recording overhead control | 2-5x slowdown acceptable | [x] |

#### Sprint D2.2: DAP Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.2.1 | DAP reverse capabilities | `supportsStepBack: true` | [x] |
| D2.2.2 | VS Code reverse debugging UI | Back arrows in debug toolbar | [x] |
| D2.2.3 | Variable history | "Show all values of x" panel | [x] |
| D2.2.4 | Execution timeline UI | Scrubber bar (drag to any point in time) | [x] |
| D2.2.5 | Conditional reverse | "Go back to when `loss < 0.1`" | [x] |
| D2.2.6 | Call stack history | Full call stack at each point | [x] |
| D2.2.7 | Memory view at time T | Inspect heap state at any snapshot | [x] |
| D2.2.8 | Tensor visualization | Show tensor values at each step | [x] |
| D2.2.9 | Data breakpoints (reverse) | "What wrote to address 0xFF00?" | [x] |
| D2.2.10 | Export trace | Save recording for offline analysis | [x] |

#### Sprint D2.3: ML-Specific Debugging (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.3.1 | Gradient inspection | Show ∂loss/∂w at each backward step | [x] |
| D2.3.2 | Tensor shape tracker | Shape changes through pipeline | [x] |
| D2.3.3 | NaN/Inf detector | Break on first NaN in any tensor | [x] |
| D2.3.4 | Loss curve live plot | Loss value at each training step | [x] |
| D2.3.5 | Weight histogram | Distribution of weights per layer | [x] |
| D2.3.6 | Activation visualization | Heatmap of layer outputs | [x] |
| D2.3.7 | Gradient explosion detector | Alert when gradient norm > threshold | [x] |
| D2.3.8 | Learning rate schedule plot | LR over time | [x] |
| D2.3.9 | Batch data inspector | View input batch at each step | [x] |
| D2.3.10 | Model architecture diagram | Auto-generated layer graph | [x] |

### Phase D3: Integration (2 sprints, 20 tasks)

#### Sprint D3.1: CLI Tools (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.1.1 | `fj debug --record` | Enable recording mode | [x] |
| D3.1.2 | `fj debug --replay file.trace` | Replay saved recording | [x] |
| D3.1.3 | `fj profile --flamegraph` | Generate flamegraph SVG | [x] |
| D3.1.4 | `fj profile --memory` | Memory profiling mode | [x] |
| D3.1.5 | `fj profile --tensor` | Tensor operation profiling | [x] |
| D3.1.6 | Profile comparison | `fj profile diff a.json b.json` | [x] |
| D3.1.7 | CI integration | `fj profile --check --max-time 100ms` | [x] |
| D3.1.8 | Benchmark regression | Fail CI if >10% slower | [x] |
| D3.1.9 | Profile annotations | `@profile fn heavy_work()` | [x] |
| D3.1.10 | REPL profiling | Profile expressions in REPL | [x] |

#### Sprint D3.2: Documentation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.2.1 | Profiler user guide | Getting started + examples | [x] |
| D3.2.2 | Debugger user guide | Breakpoints, stepping, reverse | [x] |
| D3.2.3 | ML debugging tutorial | "Finding the vanishing gradient" | [x] |
| D3.2.4 | Memory profiling tutorial | "Tracking tensor memory leaks" | [x] |
| D3.2.5 | Flamegraph interpretation | How to read flamegraphs | [x] |
| D3.2.6 | VS Code extension update | New debug/profile commands | [x] |
| D3.2.7 | Performance optimization guide | Common patterns + fixes | [x] |
| D3.2.8 | Embedded profiling guide | Profiling on Dragon Q6A | [x] |
| D3.2.9 | API reference | Profiler/debugger Rust APIs | [x] |
| D3.2.10 | Blog: "Time-Travel Debugging" | Announcement + demo video | [x] |

---

## Option 3: FajarOS Nova v3.0 "Aurora" (12 sprints, 120 tasks) ✅ COMPLETE

**Goal:** Multi-core SMP, USB 3.0, display manager, sound, in-kernel package manager
**Impact:** Production-quality desktop OS written 100% in Fajar Lang

### Phase A1: SMP & Scheduler v2 (3 sprints, 30 tasks)

#### Sprint A1.1: Per-CPU Scheduling (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A1.1.1 | Per-CPU run queues | Separate ready queue per logical CPU core | [x] |
| A1.1.2 | Work stealing scheduler | Idle CPU steals from busiest neighbor queue | [x] |
| A1.1.3 | CPU affinity | `sched_setaffinity()` to pin tasks to cores | [x] |
| A1.1.4 | Load balancing | Periodic rebalance across run queues (100ms) | [x] |
| A1.1.5 | IPI (Inter-Processor Interrupt) | Send/receive cross-CPU interrupts for reschedule | [x] |
| A1.1.6 | NUMA awareness | Prefer local memory node for task placement | [x] |
| A1.1.7 | Tickless idle | Disable timer interrupt on idle CPUs (NO_HZ) | [x] |
| A1.1.8 | CFS-like fair scheduling | Virtual runtime with red-black tree ready queue | [x] |
| A1.1.9 | Priority inheritance | Boost holder priority to prevent inversion | [x] |
| A1.1.10 | RT scheduling class | SCHED_FIFO/SCHED_RR with strict priority preemption | [x] |

#### Sprint A1.2: SMP Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A1.2.1 | CPU hotplug | Online/offline CPUs at runtime | [x] |
| A1.2.2 | Per-CPU variables | `__percpu` section with GS/FS segment addressing | [x] |
| A1.2.3 | Spinlock with backoff | Exponential backoff on contention (MCS lock) | [x] |
| A1.2.4 | RCU read-side | Read-Copy-Update for lock-free concurrent reads | [x] |
| A1.2.5 | Preemption points | Voluntary preemption at safe points in kernel | [x] |
| A1.2.6 | Migration threads | Per-CPU kthread for cross-CPU task migration | [x] |
| A1.2.7 | `/proc/cpuinfo` live | Dynamic CPU info with frequency, load, temp | [x] |
| A1.2.8 | CPU topology discovery | Detect cores, threads, packages, cache hierarchy | [x] |
| A1.2.9 | Scheduler statistics | Per-CPU counters: context switches, migrations, idle time | [x] |
| A1.2.10 | SMP boot sequence | AP trampoline with proper synchronization barriers | [x] |

#### Sprint A1.3: SMP Testing & Tools (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A1.3.1 | htop-style process monitor | Per-CPU bars, task list, sort by CPU/memory | [x] |
| A1.3.2 | SMP stress test | N threads × N CPUs contention torture test | [x] |
| A1.3.3 | SMP benchmarks | Scheduling latency, migration cost, lock throughput | [x] |
| A1.3.4 | CPU isolation | `isolcpus` equivalent for real-time workloads | [x] |
| A1.3.5 | Scheduler tracing | ftrace-style event logging for schedule decisions | [x] |
| A1.3.6 | Deadlock detector | Lock ordering validation in debug builds | [x] |
| A1.3.7 | Priority ceiling protocol | Alternative to priority inheritance for static systems | [x] |
| A1.3.8 | Processor power states | C-states management for idle cores | [x] |
| A1.3.9 | SMP documentation | Architecture guide + API reference | [x] |
| A1.3.10 | SMP integration test | Multi-core QEMU boot + work stealing verification | [x] |

### Phase A2: USB 3.0 & Device Framework (3 sprints, 30 tasks)

#### Sprint A2.1: xHCI Controller & Enumeration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A2.1.1 | xHCI controller init | PCI BAR mapping, capability/operational/runtime regs | [x] |
| A2.1.2 | Device enumeration | Port status change → address assignment → descriptor read | [x] |
| A2.1.3 | USB hub support | Hub descriptor parsing, per-port power/reset control | [x] |
| A2.1.4 | Bulk transfers | Bulk IN/OUT for mass storage and serial devices | [x] |
| A2.1.5 | Interrupt transfers | Periodic polling for HID devices (keyboard/mouse) | [x] |
| A2.1.6 | Isochronous transfers | Streaming for audio/video with guaranteed bandwidth | [x] |
| A2.1.7 | USB 3.0 SuperSpeed | 5 Gbps link training, stream protocol support | [x] |
| A2.1.8 | Descriptor parsing | Device/config/interface/endpoint descriptor hierarchy | [x] |
| A2.1.9 | Endpoint management | Transfer ring allocation, doorbell register access | [x] |
| A2.1.10 | USB device hot-plug/unplug | Port status change event handling + cleanup | [x] |

#### Sprint A2.2: Device Drivers (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A2.2.1 | USB mass storage (SCSI) | BOT protocol, SCSI READ/WRITE/INQUIRY commands | [x] |
| A2.2.2 | USB HID keyboard | Keycode translation, repeat rate, LED control | [x] |
| A2.2.3 | USB HID mouse | Relative movement, button state, scroll wheel | [x] |
| A2.2.4 | USB audio class | PCM streaming, volume control, sample rate selection | [x] |
| A2.2.5 | USB ethernet adapter | CDC-ECM/RNDIS network interface with MAC address | [x] |
| A2.2.6 | USB serial/FTDI | TTY device for UART-over-USB adapters | [x] |
| A2.2.7 | Driver registration framework | Class-based driver matching (VID:PID + class code) | [x] |
| A2.2.8 | Bus scan on boot | Enumerate all ports and load matching drivers | [x] |
| A2.2.9 | Device power management | Suspend/resume, selective suspend for idle devices | [x] |
| A2.2.10 | devfs entries | `/dev/usb/*` device nodes with read/write/ioctl | [x] |

#### Sprint A2.3: USB Infrastructure & Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A2.3.1 | udev-style rules | Rule engine for device naming, permissions, scripts | [x] |
| A2.3.2 | `lsusb` command | List all USB devices with descriptors and tree view | [x] |
| A2.3.3 | USB error recovery | Stall handling, endpoint reset, device reset | [x] |
| A2.3.4 | USB bandwidth allocation | Track periodic/isochronous bandwidth per bus | [x] |
| A2.3.5 | USB debug logging | Per-device trace with transfer hex dumps | [x] |
| A2.3.6 | USB passthrough (QEMU) | VirtIO-USB for testing with virtual devices | [x] |
| A2.3.7 | USB gadget mode | Device-side USB (OTG) for acting as USB peripheral | [x] |
| A2.3.8 | USB benchmark | Transfer throughput: bulk, interrupt, isochronous | [x] |
| A2.3.9 | USB documentation | Driver development guide + API reference | [x] |
| A2.3.10 | USB integration tests | QEMU xHCI + virtual mass storage + HID verification | [x] |

### Phase A3: Display Manager & Compositor (3 sprints, 30 tasks)

#### Sprint A3.1: Display & Mode Setting (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A3.1.1 | Multi-monitor detection | EDID parsing for connected displays via GOP/VESA | [x] |
| A3.1.2 | Mode setting (VESA/GOP) | Resolution and color depth switching at runtime | [x] |
| A3.1.3 | Resolution switching | Dynamic mode change without reboot | [x] |
| A3.1.4 | Vsync double/triple buffering | Page flip with vertical blank synchronization | [x] |
| A3.1.5 | DPI scaling | High-DPI awareness (1x/1.5x/2x) per monitor | [x] |
| A3.1.6 | Screen rotation | 0/90/180/270 degree framebuffer rotation | [x] |
| A3.1.7 | Font rendering | TrueType rasterization with subpixel anti-aliasing | [x] |
| A3.1.8 | Cursor themes | Hardware cursor with customizable sprite sets | [x] |
| A3.1.9 | Wallpaper engine | Background image/color per workspace | [x] |
| A3.1.10 | Display benchmark | Framerate, latency, fill rate measurement | [x] |

#### Sprint A3.2: Compositor (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A3.2.1 | Wayland-style compositor protocol | Surface create/destroy/commit message protocol | [x] |
| A3.2.2 | Surface allocation | Shared memory buffers for client window content | [x] |
| A3.2.3 | Damage tracking | Dirty region tracking for partial recomposition | [x] |
| A3.2.4 | Alpha blending | Per-pixel transparency compositing (Porter-Duff) | [x] |
| A3.2.5 | Window animations | Fade-in/out, slide, minimize/maximize transitions | [x] |
| A3.2.6 | GPU-accelerated compositing | VirtIO-GPU render for compositor passes | [x] |
| A3.2.7 | Multi-desktop/workspace | Virtual desktop switching (Ctrl+Alt+Arrow) | [x] |
| A3.2.8 | Window snapping | Left/right half-screen, maximize, quarter-tile | [x] |
| A3.2.9 | Alt-tab switcher | Window thumbnail preview with live updates | [x] |
| A3.2.10 | Compositor benchmark | Compositing FPS with N overlapping windows | [x] |

#### Sprint A3.3: Desktop Features (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A3.3.1 | Screen lock | Password-protected lock screen with timeout | [x] |
| A3.3.2 | Notification popups | Toast notifications with dismiss/action buttons | [x] |
| A3.3.3 | Clipboard manager | Copy/paste with history (text, images) | [x] |
| A3.3.4 | Drag-and-drop | Inter-window DnD with MIME type negotiation | [x] |
| A3.3.5 | Screenshot with region select | Full screen, window, rectangle capture to PNG | [x] |
| A3.3.6 | Screen recording | Raw framebuffer capture to video file | [x] |
| A3.3.7 | Input method framework | Keyboard layout switching, compose sequences | [x] |
| A3.3.8 | Accessibility | High contrast, zoom, screen reader hooks | [x] |
| A3.3.9 | Display configuration tool | GUI for resolution, scaling, arrangement | [x] |
| A3.3.10 | Display integration tests | Multi-monitor + compositor + input end-to-end | [x] |

### Phase A4: System Services (3 sprints, 30 tasks)

#### Sprint A4.1: Audio & Bluetooth (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A4.1.1 | Sound server (mixing daemon) | Multi-stream audio mixing with per-client volume | [x] |
| A4.1.2 | PulseAudio-style API | Stream open/close, volume, sample format negotiation | [x] |
| A4.1.3 | HDA codec driver | Intel HD Audio controller init + PCM playback | [x] |
| A4.1.4 | Bluetooth HCI layer | Host Controller Interface over USB transport | [x] |
| A4.1.5 | Bluetooth L2CAP | Logical Link Control and Adaptation Protocol channels | [x] |
| A4.1.6 | Bluetooth pairing | Secure Simple Pairing with PIN/passkey | [x] |
| A4.1.7 | Bluetooth A2DP | Audio streaming to BT headphones (SBC codec) | [x] |
| A4.1.8 | WiFi WPA2 supplicant | 4-way handshake, CCMP encryption, SSID scan | [x] |
| A4.1.9 | Audio recording | Microphone capture with sample rate conversion | [x] |
| A4.1.10 | Audio/BT integration test | Playback + BT pairing + WiFi scan verification | [x] |

#### Sprint A4.2: Power & System Management (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A4.2.1 | Power management daemon | ACPI event handling for lid/button/battery | [x] |
| A4.2.2 | Battery monitor | Charge level, discharge rate, time remaining | [x] |
| A4.2.3 | Suspend/resume | S3 sleep with device save/restore state | [x] |
| A4.2.4 | RTC alarm wakeup | Schedule wake from suspend at specified time | [x] |
| A4.2.5 | Thermal throttling | CPU frequency reduction on temperature threshold | [x] |
| A4.2.6 | Fan control | PWM fan speed based on thermal zones | [x] |
| A4.2.7 | CPU frequency scaling | cpufreq governors: performance, powersave, ondemand | [x] |
| A4.2.8 | ACPI power button handler | Short press → suspend, long press → shutdown | [x] |
| A4.2.9 | Out-of-memory killer | Score-based process termination on memory pressure | [x] |
| A4.2.10 | Core dump handler | Save register state + stack trace on crash | [x] |

#### Sprint A4.3: System Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A4.3.1 | systemd-style service manager | Unit files, dependency ordering, parallel start | [x] |
| A4.3.2 | Service dependencies | After/Requires/Wants dependency resolution | [x] |
| A4.3.3 | Watchdog timer | Hardware watchdog with automatic reboot on hang | [x] |
| A4.3.4 | Swap partition | Page-out to disk under memory pressure | [x] |
| A4.3.5 | tmpfs / devtmpfs | RAM-backed filesystems for /tmp and /dev | [x] |
| A4.3.6 | Kernel module loading | Dynamic `.ko` loading with symbol resolution | [x] |
| A4.3.7 | sysctl interface | Runtime kernel parameter tuning via `/proc/sys` | [x] |
| A4.3.8 | Kernel panic handler | Stack trace, register dump, optional reboot | [x] |
| A4.3.9 | Boot splash screen | Graphical boot logo with progress bar | [x] |
| A4.3.10 | System benchmark suite | CPU, memory, disk, network comprehensive benchmarks | [x] |

---

## Option 4: Real-Time ML Pipeline (6 sprints, 60 tasks) ✅ COMPLETE

**Goal:** End-to-end sensor → inference → actuator pipeline with latency guarantees
**Impact:** Core differentiator — no other language does this with compiler-enforced safety

### Phase R1: Sensor Framework (2 sprints, 20 tasks)

#### Sprint R1.1: Sensor Abstraction (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.1.1 | Sensor trait | `trait Sensor { fn read() -> SensorData }` | [x] |
| R1.1.2 | SensorData type | Timestamped, typed sensor readings | [x] |
| R1.1.3 | IMU driver | Accelerometer + gyroscope (MPU6050) | [x] |
| R1.1.4 | Camera frame capture | Raw frame → Tensor conversion | [x] |
| R1.1.5 | ADC driver | Analog-to-digital (temperature, pressure) | [x] |
| R1.1.6 | GPS NMEA parser | Lat/lon/alt from UART GPS module | [x] |
| R1.1.7 | LiDAR point cloud | Distance array from scanning LiDAR | [x] |
| R1.1.8 | Microphone PCM | Audio samples for voice detection | [x] |
| R1.1.9 | Sensor fusion | Kalman filter for IMU + GPS | [x] |
| R1.1.10 | Sensor data logger | Ring buffer with timestamp | [x] |

#### Sprint R1.2: Data Pipeline (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.2.1 | Pipeline DSL | `sensor |> preprocess |> infer |> act` | [x] |
| R1.2.2 | Preprocessing stage | Normalization, windowing, FFT | [x] |
| R1.2.3 | Feature extraction | Rolling mean, variance, peak detection | [x] |
| R1.2.4 | Batching strategy | Collect N samples before inference | [x] |
| R1.2.5 | Ring buffer allocator | Zero-copy circular buffer | [x] |
| R1.2.6 | Backpressure handling | Drop oldest on overflow | [x] |
| R1.2.7 | Multi-sensor fusion | Align timestamps across sensors | [x] |
| R1.2.8 | Data augmentation | Random noise, rotation for training | [x] |
| R1.2.9 | Streaming windowing | Sliding window with overlap | [x] |
| R1.2.10 | Pipeline benchmark | End-to-end latency measurement | [x] |

### Phase R2: Inference Engine (2 sprints, 20 tasks)

#### Sprint R2.1: Model Runtime (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R2.1.1 | Model loader | FJML/ONNX/TFLite → runtime graph | [x] |
| R2.1.2 | Inference scheduler | Priority queue with deadline | [x] |
| R2.1.3 | @device inference context | Tensor ops isolated from kernel | [x] |
| R2.1.4 | Batch inference | Multiple inputs in one pass | [x] |
| R2.1.5 | Model hot-swap | Replace model without restart | [x] |
| R2.1.6 | Multi-model pipeline | Chain: detector → classifier → tracker | [x] |
| R2.1.7 | Confidence threshold | Filter low-confidence predictions | [x] |
| R2.1.8 | Inference caching | Cache repeated inputs (LRU) | [x] |
| R2.1.9 | Quantized inference | INT8 fast path on CPU/NPU | [x] |
| R2.1.10 | Latency SLA | Guarantee < 10ms per inference | [x] |

#### Sprint R2.2: Actuator Framework (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R2.2.1 | Actuator trait | `trait Actuator { fn act(cmd: Command) }` | [x] |
| R2.2.2 | PWM motor control | Speed + direction via PWM duty cycle | [x] |
| R2.2.3 | Servo control | Angular position (0-180°) | [x] |
| R2.2.4 | GPIO digital output | On/off for relays, LEDs | [x] |
| R2.2.5 | CAN bus command | Automotive actuator commands | [x] |
| R2.2.6 | Safety interlock | Emergency stop on anomaly detection | [x] |
| R2.2.7 | PID controller | Proportional-integral-derivative loop | [x] |
| R2.2.8 | Command smoothing | Ramp rate limiting for motors | [x] |
| R2.2.9 | Actuator feedback | Closed-loop with sensor reading | [x] |
| R2.2.10 | Fail-safe defaults | Safe state on communication loss | [x] |

### Phase R3: Integration & Demo (2 sprints, 20 tasks)

#### Sprint R3.1: End-to-End Pipeline (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R3.1.1 | @kernel → @device bridge | Zero-copy sensor data → tensor | [x] |
| R3.1.2 | @device → @kernel bridge | Inference result → actuator command | [x] |
| R3.1.3 | @safe orchestrator | Pipeline coordination with error handling | [x] |
| R3.1.4 | Deadline scheduler | Hard real-time task priorities | [x] |
| R3.1.5 | Jitter measurement | < 1ms variance on 10ms deadline | [x] |
| R3.1.6 | Worst-case execution time | Static WCET analysis | [x] |
| R3.1.7 | Priority inversion prevention | Priority inheritance protocol | [x] |
| R3.1.8 | Watchdog integration | Reset on missed deadline | [x] |
| R3.1.9 | Telemetry export | Pipeline metrics → serial/network | [x] |
| R3.1.10 | Power-aware scheduling | Reduce frequency when idle | [x] |

#### Sprint R3.2: Demo Applications (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R3.2.1 | Drone autopilot | IMU → stabilization → motor control | [x] |
| R3.2.2 | Object tracker | Camera → YOLO → servo follow | [x] |
| R3.2.3 | Anomaly detector | Vibration sensor → FFT → classifier → alert | [x] |
| R3.2.4 | Voice command | Microphone → keyword detection → GPIO | [x] |
| R3.2.5 | Autonomous rover | LiDAR → obstacle avoid → motor | [x] |
| R3.2.6 | Predictive maintenance | Sensor trends → failure prediction → alert | [x] |
| R3.2.7 | Smart agriculture | Soil moisture → irrigation control | [x] |
| R3.2.8 | Industrial quality control | Camera → defect detection → reject gate | [x] |
| R3.2.9 | Pipeline benchmark report | All demos with latency/accuracy | [x] |
| R3.2.10 | Blog: "RT ML in Fajar Lang" | Architecture + benchmarks | [x] |

---

## Option 5: FFI v2 — C++/Python/Rust Interop (5 sprints, 50 tasks) ✅ COMPLETE

**Goal:** Seamlessly call C++, Python, and Rust libraries from Fajar Lang
**Impact:** Access entire ecosystems (PyTorch, OpenCV, Tokio) without reimplementing

### Phase F1: C++ Interop (2 sprints, 20 tasks)

#### Sprint F1.1: C++ Binding Generation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F1.1.1 | C++ header parsing via libclang | Parse `.hpp` files to extract classes, methods, types | [x] |
| F1.1.2 | Name mangling | Itanium ABI name mangling/demangling for symbol lookup | [x] |
| F1.1.3 | Class method calls | Virtual + non-virtual method dispatch across FFI boundary | [x] |
| F1.1.4 | Template instantiation | Monomorphize C++ templates for requested type parameters | [x] |
| F1.1.5 | RAII bridging | C++ destructor → Fajar Drop trait mapping | [x] |
| F1.1.6 | `std::string`/`std::vector` conversion | Zero-copy view or deep-copy between Fajar and C++ containers | [x] |
| F1.1.7 | Exception handling | C++ `catch` → Fajar `Result<T, CppError>` conversion | [x] |
| F1.1.8 | Namespace resolution | `cpp::cv::Mat` → C++ `cv::Mat` qualified lookup | [x] |
| F1.1.9 | `extern "C++" {}` blocks | Language-level syntax for declaring C++ imports | [x] |
| F1.1.10 | Smart pointer bridging | `std::shared_ptr`/`std::unique_ptr` ↔ Fajar ownership | [x] |

#### Sprint F1.2: C++ Integration & Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F1.2.1 | Virtual method dispatch | vtable-based call through C++ abstract base classes | [x] |
| F1.2.2 | Operator overloading bridge | C++ `operator+` → Fajar `+` trait mapping | [x] |
| F1.2.3 | Compile-time type mapping | `int`→`i32`, `double`→`f64`, `bool`→`bool` auto-conversion | [x] |
| F1.2.4 | CMake integration | `find_package(FajarLang)` for C++ projects | [x] |
| F1.2.5 | pkg-config support | `.pc` file generation for Fajar libraries | [x] |
| F1.2.6 | OpenCV bridge demo | `cv::Mat` ↔ Tensor, image load/process/display | [x] |
| F1.2.7 | Eigen matrix bridge | `Eigen::MatrixXd` ↔ Tensor with zero-copy view | [x] |
| F1.2.8 | C++ → Fajar callback | Pass Fajar closures as `std::function` to C++ | [x] |
| F1.2.9 | ABI compatibility check | Validate struct layout, alignment, calling convention | [x] |
| F1.2.10 | C++ FFI benchmark | Call overhead, data conversion cost measurement | [x] |

### Phase F2: Python Interop (2 sprints, 20 tasks)

#### Sprint F2.1: Python Embedding (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F2.1.1 | CPython embedding via libpython | `dlopen` libpython3, `Py_Initialize`, interpreter lifecycle | [x] |
| F2.1.2 | PyObject conversion | Fajar Value ↔ PyObject automatic marshaling | [x] |
| F2.1.3 | NumPy → Tensor bridge | `numpy.ndarray` ↔ Fajar Tensor with shared memory view | [x] |
| F2.1.4 | Call Python functions from Fajar | `py_call("module", "func", args)` with return conversion | [x] |
| F2.1.5 | Call Fajar functions from Python | Export Fajar functions as Python callable objects | [x] |
| F2.1.6 | GIL management | Acquire/release GIL around Python calls, release during Fajar compute | [x] |
| F2.1.7 | Module import system | `import numpy`, `from torch import nn` with sys.path setup | [x] |
| F2.1.8 | dict/list/tuple conversion | Python containers ↔ Fajar Map/Array/Tuple | [x] |
| F2.1.9 | Exception → Result mapping | Python exceptions → `Err(PyError)` with traceback | [x] |
| F2.1.10 | `@python` annotation | `@python fn preprocess()` for hybrid Fajar+Python code | [x] |

#### Sprint F2.2: Python Ecosystem & Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F2.2.1 | Jupyter kernel for Fajar Lang | IPython kernel protocol for notebook execution | [x] |
| F2.2.2 | pandas DataFrame → Map conversion | Column-oriented DataFrame ↔ Fajar Map<String, Array> | [x] |
| F2.2.3 | matplotlib bridge for plotting | `plt.plot()`, `plt.show()` from Fajar code | [x] |
| F2.2.4 | pip package wrapping | `fj pip-install numpy` → auto-detect virtualenv | [x] |
| F2.2.5 | PyTorch model loading | Load `.pt` model → Fajar inference graph | [x] |
| F2.2.6 | Virtual environment detection | Auto-detect venv/conda/pyenv Python path | [x] |
| F2.2.7 | `fj python-bridge` CLI | Manage Python installation, packages, bridge config | [x] |
| F2.2.8 | Async Python ↔ async Fajar | Bridge asyncio event loop with Fajar executor | [x] |
| F2.2.9 | Memory sharing (zero-copy) | Shared buffer protocol for large tensor transfer | [x] |
| F2.2.10 | Python interop benchmark | Call overhead, NumPy bridge throughput, GIL contention | [x] |

### Phase F3: Rust Interop (1 sprint, 10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F3.1 | Rust crate linking | Link `.rlib` / `.so` from Cargo | [x] |
| F3.2 | Type mapping | Rust struct ↔ Fajar struct | [x] |
| F3.3 | Trait bridging | Rust trait → Fajar trait impl | [x] |
| F3.4 | Error bridging | `anyhow::Error` → Fajar `Result` | [x] |
| F3.5 | Async bridging | Tokio future ↔ Fajar future | [x] |
| F3.6 | Macro export | Use Rust proc macros in Fajar | [x] |
| F3.7 | Serde integration | JSON/TOML/YAML via serde | [x] |
| F3.8 | Cargo build integration | `[fj-dependencies]` in Cargo.toml | [x] |
| F3.9 | Rust → Fajar code generator | Auto-generate bindings | [x] |
| F3.10 | Interop benchmark | Call overhead measurement | [x] |

---

## Option 6: Standard Library v3 (8 sprints, 80 tasks) ✅ COMPLETE

**Goal:** Comprehensive stdlib rivaling Rust/Go for real-world applications
**Impact:** Self-sufficient language — no need for FFI for common tasks

### Phase S1: Async Networking (2 sprints, 20 tasks)

#### Sprint S1.1: TCP/UDP & HTTP (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.1.1 | TCP client/server | Async `TcpStream::connect()`, `TcpListener::bind()` | [x] |
| S1.1.2 | UDP sockets | `UdpSocket::send_to()`, `recv_from()` with async I/O | [x] |
| S1.1.3 | HTTP client (GET/POST/PUT/DELETE) | Request builder with headers, body, status parsing | [x] |
| S1.1.4 | HTTP server with routing | Path matching, method dispatch, middleware chain | [x] |
| S1.1.5 | WebSocket client/server | Upgrade handshake, frame parsing, ping/pong | [x] |
| S1.1.6 | DNS resolver | Recursive DNS lookup with caching (A, AAAA, CNAME) | [x] |
| S1.1.7 | TLS/SSL via rustls | TLS 1.3 handshake, certificate verification | [x] |
| S1.1.8 | Connection pooling | Reuse TCP connections with idle timeout | [x] |
| S1.1.9 | Timeout/retry | Per-request timeout with exponential backoff retry | [x] |
| S1.1.10 | Async stream reading | Chunked transfer encoding, streaming response body | [x] |

#### Sprint S1.2: Advanced Networking (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.2.1 | Multipart form data | File upload with boundary-delimited encoding | [x] |
| S1.2.2 | URL parsing | Scheme, host, port, path, query, fragment extraction | [x] |
| S1.2.3 | Cookie handling | Set-Cookie parsing, cookie jar, domain/path matching | [x] |
| S1.2.4 | HTTP/2 multiplexing | Stream multiplexing with HPACK header compression | [x] |
| S1.2.5 | Keep-alive | Persistent connections with configurable idle timeout | [x] |
| S1.2.6 | Proxy support | HTTP/HTTPS proxy with CONNECT tunnel | [x] |
| S1.2.7 | SOCKS5 support | SOCKS5 proxy with username/password authentication | [x] |
| S1.2.8 | Rate limiting | Token bucket rate limiter for outgoing requests | [x] |
| S1.2.9 | Circuit breaker | Open/half-open/closed state machine for failing endpoints | [x] |
| S1.2.10 | Network benchmark | Throughput, latency, connection setup time measurement | [x] |

### Phase S2: Crypto & Security (2 sprints, 20 tasks)

#### Sprint S2.1: Hashing & Encryption (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.1.1 | SHA-256/384/512 | Secure hash functions with streaming interface | [x] |
| S2.1.2 | HMAC | Keyed-hash message authentication code (SHA-256/512) | [x] |
| S2.1.3 | AES-128/256 (CBC/GCM) | Symmetric encryption with IV and authentication tag | [x] |
| S2.1.4 | RSA 2048/4096 | Key generation, encrypt/decrypt, sign/verify (PKCS#1) | [x] |
| S2.1.5 | Ed25519 signing/verification | Edwards curve digital signatures (64-byte) | [x] |
| S2.1.6 | X25519 key exchange | Elliptic curve Diffie-Hellman for shared secrets | [x] |
| S2.1.7 | PBKDF2/Argon2 password hashing | Password-based key derivation with salt + iterations | [x] |
| S2.1.8 | Random bytes (CSPRNG) | Cryptographically secure random via OS entropy | [x] |
| S2.1.9 | Base64 encode/decode | Standard + URL-safe Base64 with padding options | [x] |
| S2.1.10 | Hex encode/decode | Byte array ↔ hexadecimal string conversion | [x] |

#### Sprint S2.2: Certificates & Compliance (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.2.1 | JWT creation/verification | Header.Payload.Signature with HS256/RS256/ES256 | [x] |
| S2.2.2 | X.509 certificate parsing | DER/PEM parsing, subject, issuer, validity, extensions | [x] |
| S2.2.3 | TLS certificate validation | Chain-of-trust verification against root CA bundle | [x] |
| S2.2.4 | Constant-time comparison | Timing-safe equality check for secrets/MACs | [x] |
| S2.2.5 | Secure memory zeroing | Zeroize sensitive data on drop (prevent optimization out) | [x] |
| S2.2.6 | Key derivation (HKDF) | HMAC-based Extract-and-Expand for key material | [x] |
| S2.2.7 | Digital signature (ECDSA P-256) | NIST P-256 curve signing/verification | [x] |
| S2.2.8 | Certificate chain validation | Intermediate CA chain building and path validation | [x] |
| S2.2.9 | FIPS 140-2 compliance check | Self-test on startup, approved algorithm enforcement | [x] |
| S2.2.10 | Crypto benchmark | Throughput for hash/encrypt/sign operations | [x] |

### Phase S3: Data Formats (2 sprints, 20 tasks)

#### Sprint S3.1: Parsers & Serializers (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S3.1.1 | JSON parser/serializer | Streaming parser + pretty-print serializer (RFC 8259) | [x] |
| S3.1.2 | TOML parser | Full TOML v1.0 parsing with type preservation | [x] |
| S3.1.3 | YAML parser | YAML 1.2 core schema, anchors, aliases, multiline | [x] |
| S3.1.4 | CSV reader/writer | RFC 4180 with quoting, custom delimiter, header detection | [x] |
| S3.1.5 | XML SAX parser | Event-driven XML parsing (start/end element, text, attrs) | [x] |
| S3.1.6 | MessagePack binary serialization | Compact binary encoding for structured data | [x] |
| S3.1.7 | Protocol Buffers (code gen) | `.proto` file → Fajar struct + serialize/deserialize code | [x] |
| S3.1.8 | Regular expressions | NFA/DFA hybrid engine with capture groups and backrefs | [x] |
| S3.1.9 | INI file parser | Section/key/value parsing with comment support | [x] |
| S3.1.10 | Template engine | String templates with `{{variable}}` substitution + loops | [x] |

#### Sprint S3.2: Data Types & Compression (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S3.2.1 | Date/time (ISO 8601) | Parse/format datetime, duration, RFC 3339 timestamps | [x] |
| S3.2.2 | Timezone support | IANA timezone database, UTC offset, DST transitions | [x] |
| S3.2.3 | UUID v4/v7 generation | Random (v4) and time-ordered (v7) UUID generation | [x] |
| S3.2.4 | URI/URL parser | Full RFC 3986 parsing with query string encoding | [x] |
| S3.2.5 | MIME type detection | File extension and magic byte detection (200+ types) | [x] |
| S3.2.6 | gzip/deflate compression | zlib-compatible compress/decompress with levels 1-9 | [x] |
| S3.2.7 | zstd compression | Zstandard compression with dictionary support | [x] |
| S3.2.8 | tar archive read/write | POSIX tar with long filename support | [x] |
| S3.2.9 | zip archive read/write | ZIP64 with deflate/store methods, file listing | [x] |
| S3.2.10 | Format benchmark | Parse/serialize throughput for all format implementations | [x] |

### Phase S4: System & Utilities (2 sprints, 20 tasks)

#### Sprint S4.1: File System & Process (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S4.1.1 | Path manipulation | `join`, `parent`, `extension`, `normalize`, `canonicalize` | [x] |
| S4.1.2 | Directory walking (recursive) | Depth-first traversal with glob filtering and symlink control | [x] |
| S4.1.3 | File watching (inotify) | Watch files/dirs for create, modify, delete, rename events | [x] |
| S4.1.4 | Temp file/dir creation | Auto-cleanup temporary files with unique names | [x] |
| S4.1.5 | File locking (advisory) | Shared/exclusive flock for concurrent access control | [x] |
| S4.1.6 | Process spawning | `fork`/`exec` with stdin/stdout/stderr pipe capture | [x] |
| S4.1.7 | Signal handling | Register handlers for SIGINT, SIGTERM, SIGHUP, SIGUSR1 | [x] |
| S4.1.8 | Environment variables | `env_get`, `env_set`, `env_vars` iteration | [x] |
| S4.1.9 | Command-line argument parsing | Clap-style derive parser with subcommands, flags, values | [x] |
| S4.1.10 | Logging framework | Levels (trace→error), sinks (file, stderr), structured formatting | [x] |

#### Sprint S4.2: Concurrency & Terminal Utilities (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S4.2.1 | Progress bar | Animated progress indicator with ETA and throughput | [x] |
| S4.2.2 | Colored terminal output | ANSI escape codes for fg/bg colors, bold, underline | [x] |
| S4.2.3 | Table formatting | Column-aligned ASCII/Unicode tables with wrapping | [x] |
| S4.2.4 | Timer/stopwatch | High-resolution elapsed time measurement (nanosecond) | [x] |
| S4.2.5 | Thread pool | Fixed-size pool with task submission and join handles | [x] |
| S4.2.6 | Channel (bounded/unbounded) | MPSC channels with backpressure and select support | [x] |
| S4.2.7 | Concurrent HashMap | Lock-striped or lock-free concurrent hash map | [x] |
| S4.2.8 | Atomic counter | AtomicI64/AtomicU64 with fetch_add, compare_exchange | [x] |
| S4.2.9 | Rate limiter (token bucket) | Token bucket algorithm for rate-limited operations | [x] |
| S4.2.10 | Utility benchmark | Performance measurement for all stdlib utilities | [x] |

---

## Option 7: Language Server v3 (5 sprints, 50 tasks) ✅ COMPLETE

**Goal:** World-class IDE experience — on par with rust-analyzer
**Impact:** Developer productivity; IDE experience often determines language adoption

### Phase L1: Semantic Analysis (2 sprints, 20 tasks)

#### Sprint L1.1: Navigation & Symbols (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L1.1.1 | Semantic tokens | 24 token types + 8 modifiers for rich syntax coloring | [x] |
| L1.1.2 | Go-to-definition (cross-file) | Jump to function/struct/enum definition across modules | [x] |
| L1.1.3 | Find all references | List all usages of a symbol across workspace | [x] |
| L1.1.4 | Go-to-implementation | Jump from trait to concrete impl blocks | [x] |
| L1.1.5 | Type hierarchy | Supertrait/subtrait tree view (incoming/outgoing) | [x] |
| L1.1.6 | Call hierarchy | Incoming callers + outgoing callees tree | [x] |
| L1.1.7 | Workspace symbol search | Fuzzy search across all files (`Ctrl+T`) | [x] |
| L1.1.8 | Document symbol outline | File-level tree of functions, structs, enums, impls | [x] |
| L1.1.9 | Import suggestions | Auto-suggest `use` statement for unresolved symbols | [x] |
| L1.1.10 | Unused import detection | Warn on imports with no references in scope | [x] |

#### Sprint L1.2: Hints & Intelligence (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L1.2.1 | Dead code dimming | Gray out unreachable statements and unused functions | [x] |
| L1.2.2 | Type-on-hover | Full type signature with doc comments on mouse hover | [x] |
| L1.2.3 | Parameter info | Active parameter highlighting in function call | [x] |
| L1.2.4 | Inlay hints: types | Inferred type annotations for `let` bindings | [x] |
| L1.2.5 | Inlay hints: parameter names | Named parameter labels at call sites | [x] |
| L1.2.6 | Inlay hints: chained methods | Return type after each method in chain | [x] |
| L1.2.7 | Lens: test count per function | CodeLens showing "N tests" above each function | [x] |
| L1.2.8 | Lens: reference count | CodeLens showing "N references" above declarations | [x] |
| L1.2.9 | Semantic folding ranges | Fold by semantic blocks (impl, match arms, doc comments) | [x] |
| L1.2.10 | Breadcrumb navigation | File → module → struct → fn path in editor header | [x] |

### Phase L2: Refactoring (2 sprints, 20 tasks)

#### Sprint L2.1: Core Refactorings (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L2.1.1 | Rename symbol (cross-file) | Rename variable/function/struct across all usages | [x] |
| L2.1.2 | Extract function | Selection → new function with parameter inference | [x] |
| L2.1.3 | Extract variable | Expression → `let` binding with inferred name | [x] |
| L2.1.4 | Inline variable | Replace variable with its value at all usage sites | [x] |
| L2.1.5 | Inline function | Replace call with function body (single-use functions) | [x] |
| L2.1.6 | Convert `if-else` to `match` | Transform chained if-else into match expression | [x] |
| L2.1.7 | Convert `match` to `if-else` | Transform match with 2-3 arms to if-else chain | [x] |
| L2.1.8 | Add missing match arms | Generate exhaustive arms for enum match expression | [x] |
| L2.1.9 | Generate trait impl stubs | Scaffold all required methods for `impl Trait for Type` | [x] |
| L2.1.10 | Generate constructor | Create `fn new()` with all fields as parameters | [x] |

#### Sprint L2.2: Code Actions & Assists (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L2.2.1 | Add type annotation | Insert inferred type on `let` binding or return type | [x] |
| L2.2.2 | Remove unused imports | Delete `use` statements with no references | [x] |
| L2.2.3 | Organize imports | Sort alphabetically, group by std/external/local | [x] |
| L2.2.4 | Convert `for` to iterator chain | `for x in arr { ... }` → `arr.iter().map().collect()` | [x] |
| L2.2.5 | Wrap in `Some()`/`Ok()` | Quick-wrap expression in Option/Result constructor | [x] |
| L2.2.6 | Add `?` operator | Convert `match result { Ok(v)=>v, Err(e)=>return Err(e) }` to `?` | [x] |
| L2.2.7 | Generate documentation comment | `///` stub with parameter and return descriptions | [x] |
| L2.2.8 | Convert string to f-string | `"Hello " + name` → `f"Hello {name}"` | [x] |
| L2.2.9 | Move item to new file | Extract struct/fn/impl to separate `.fj` module file | [x] |
| L2.2.10 | Refactoring preview | Show diff before applying any refactoring action | [x] |

### Phase L3: Diagnostics & Fixes (1 sprint, 10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L3.1 | Quick fix: add missing import | Auto-insert `use` statement | [x] |
| L3.2 | Quick fix: add type annotation | Infer and insert type | [x] |
| L3.3 | Quick fix: fix typo | "Did you mean `println`?" | [x] |
| L3.4 | Quick fix: make mutable | Add `mut` when reassigned | [x] |
| L3.5 | Quick fix: add missing field | Struct literal completion | [x] |
| L3.6 | Quick fix: implement trait | Generate method stubs | [x] |
| L3.7 | Diagnostic: ownership error | Suggest clone/borrow | [x] |
| L3.8 | Diagnostic: type mismatch | Show expected vs actual | [x] |
| L3.9 | Diagnostic: unreachable code | Gray out dead branches | [x] |
| L3.10 | Diagnostic: deprecated API | Strikethrough + suggestion | [x] |

---

## Option 8: Formal Verification v2 (6 sprints, 60 tasks) ✅ COMPLETE

**Goal:** Prove program correctness with pre/post conditions, invariants, and SMT solver
**Impact:** Safety-critical certification (DO-178C, ISO 26262) for embedded ML

### Phase V1: Specification Language (2 sprints, 20 tasks)

#### Sprint V1.1: Annotations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V1.1.1 | `@requires` precondition | `@requires(x > 0)` | [x] |
| V1.1.2 | `@ensures` postcondition | `@ensures(result >= 0)` | [x] |
| V1.1.3 | `@invariant` loop invariant | `@invariant(i < n)` | [x] |
| V1.1.4 | `@assert` proof obligation | Compile-time assertion | [x] |
| V1.1.5 | `@decreases` termination proof | Variant expression | [x] |
| V1.1.6 | `old(x)` expression | Value of x at function entry | [x] |
| V1.1.7 | `forall` quantifier | `@ensures(forall(i, 0..n, arr[i] >= 0))` | [x] |
| V1.1.8 | `exists` quantifier | Existential quantification | [x] |
| V1.1.9 | Ghost variables | Specification-only state | [x] |
| V1.1.10 | Spec parsing in analyzer | Parse and validate annotations | [x] |

#### Sprint V1.2: Verification Conditions (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V1.2.1 | Weakest precondition calculus | wp(stmt, postcondition) | [x] |
| V1.2.2 | SSA transformation | Single static assignment for VC gen | [x] |
| V1.2.3 | Loop invariant checking | Verify invariant holds at entry + preserved | [x] |
| V1.2.4 | Function contract verification | Precondition→body→postcondition | [x] |
| V1.2.5 | Array bounds proof | Prove `i < len(arr)` statically | [x] |
| V1.2.6 | Integer overflow proof | Prove no overflow in arithmetic | [x] |
| V1.2.7 | Null safety proof | Prove Option unwrap is safe | [x] |
| V1.2.8 | Division by zero proof | Prove divisor != 0 | [x] |
| V1.2.9 | Termination proof | Prove decreasing variant | [x] |
| V1.2.10 | VC export (SMT-LIB2) | Export verification conditions | [x] |

### Phase V2: SMT Solver Integration (2 sprints, 20 tasks)

#### Sprint V2.1: Z3 Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V2.1.1 | Z3 integration via z3-sys FFI | Dynamic linking to libz3, context/solver lifecycle | [x] |
| V2.1.2 | SMT-LIB2 format generation | Translate VCs to standard SMT-LIB2 s-expressions | [x] |
| V2.1.3 | Bitvector theory for integer ops | Fixed-width integer arithmetic (i8→BV8, i64→BV64) | [x] |
| V2.1.4 | Array theory for verification | SMT array sort for Fajar arrays and tensor indices | [x] |
| V2.1.5 | Real arithmetic for floating point | Approximate f64 operations with real number theory | [x] |
| V2.1.6 | Solver timeout (5s per VC) | Per-verification-condition timeout with unknown result | [x] |
| V2.1.7 | Counterexample extraction | Extract satisfying model when VC is disproved | [x] |
| V2.1.8 | Counter-model display | Human-readable "Found counterexample: x=5, y=-1" | [x] |
| V2.1.9 | Incremental solving (push/pop) | Solver context stacking for related VCs | [x] |
| V2.1.10 | Proof caching | Cache verified VCs to skip re-verification on unchanged code | [x] |

#### Sprint V2.2: Solver Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V2.2.1 | Parallel verification | Per-function parallel VC checking with thread pool | [x] |
| V2.2.2 | Quantifier instantiation heuristics | E-matching and MBQI for forall/exists quantifiers | [x] |
| V2.2.3 | Unsat core extraction | Minimal set of constraints causing proof failure | [x] |
| V2.2.4 | Theory combination | Arrays + bitvectors + reals combined decision procedure | [x] |
| V2.2.5 | Custom Fajar theory plugin | Domain-specific theory for Fajar-specific types (Tensor, Option) | [x] |
| V2.2.6 | Solver benchmarking (Z3 vs CVC5) | Performance comparison with fallback on timeout | [x] |
| V2.2.7 | Verification result caching | Persist results to disk for incremental compilation | [x] |
| V2.2.8 | CI integration (`fj verify`) | `fj verify` command with pass/fail exit code | [x] |
| V2.2.9 | Verification coverage report | Percentage of functions with contracts verified | [x] |
| V2.2.10 | SMT solver fallback chain | Z3 → CVC5 → timeout:unknown cascade strategy | [x] |

### Phase V3: Tensor Shape Verification (2 sprints, 20 tasks)

#### Sprint V3.1: Shape Proofs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V3.1.1 | Tensor shape as dependent types | Shape parameters in type system: `Tensor<[N, M]>` | [x] |
| V3.1.2 | Matmul shape compatibility proof | Prove `[A,B] @ [B,C] → [A,C]` dimension agreement | [x] |
| V3.1.3 | Reshape validity proof | Prove product of dimensions is preserved (N*M == N'*M') | [x] |
| V3.1.4 | Broadcast rule verification | NumPy-style broadcast compatibility checking | [x] |
| V3.1.5 | Conv2d output shape calculation | `(H - K + 2P) / S + 1` proven in bounds | [x] |
| V3.1.6 | Concatenation axis validation | Prove all tensors match on non-concatenation axes | [x] |
| V3.1.7 | Split size validation | Prove split sizes sum to original dimension | [x] |
| V3.1.8 | Transpose permutation check | Verify permutation is valid (no duplicates, in range) | [x] |
| V3.1.9 | Batch dimension tracking | Shape preservation through forward/backward pipeline | [x] |
| V3.1.10 | Shape polymorphism | Unknown dimensions `?` with constraint propagation | [x] |

#### Sprint V3.2: Shape Infrastructure & Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V3.2.1 | Symbolic shape variables | Named dimensions (`N`, `Batch`, `SeqLen`) in types | [x] |
| V3.2.2 | Shape constraint propagation | Forward-propagate known shapes through computation graph | [x] |
| V3.2.3 | Shape error messages | "Expected [32, 784], got [32, 768]" with source location | [x] |
| V3.2.4 | ONNX shape inference verification | Verify ONNX model shapes match Fajar type annotations | [x] |
| V3.2.5 | Training shape compatibility | Prove forward pass shapes match backward pass expectations | [x] |
| V3.2.6 | Quantization shape preservation | Prove INT8 quantization preserves tensor dimensions | [x] |
| V3.2.7 | Shape verification for custom layers | User-defined layers checked against declared shape contracts | [x] |
| V3.2.8 | Dynamic shape bounds | Prove bounds `1 <= dim <= MAX_DIM` at runtime boundaries | [x] |
| V3.2.9 | Shape verification benchmark | Verification time for 100-layer model shape checking | [x] |
| V3.2.10 | Blog: "Proving ML Correctness" | Writeup with shape proof examples and benchmark results | [x] |

---

## Appendix: Decision Matrix

| Criterion | Opt 1 | Opt 2 | Opt 3 | Opt 4 | Opt 5 | Opt 6 | Opt 7 | Opt 8 |
|-----------|-------|-------|-------|-------|-------|-------|-------|-------|
| **User acquisition** | ★★★★★ | ★★ | ★★ | ★★★ | ★★★★ | ★★★★ | ★★★★★ | ★★ |
| **Differentiation** | ★★ | ★★★ | ★★★★ | ★★★★★ | ★★ | ★★★ | ★★★ | ★★★★★ |
| **Developer productivity** | ★★★ | ★★★★★ | ★ | ★★ | ★★★★ | ★★★★★ | ★★★★★ | ★★ |
| **Safety-critical** | ★ | ★★★ | ★★ | ★★★★★ | ★ | ★★ | ★ | ★★★★★ |
| **Effort** | Medium | High | Very High | Medium | Medium | High | Medium | Medium |
| **Dependency** | None | None | Nova v2.0 | Q6A (partial) | LLVM | None | LSP exists | Analyzer |

### Recommended Paths

**Path A — "Adopt Me"** (maximize users): 1 → 7 → 6
Playground + IDE + stdlib = complete developer experience

**Path B — "Trust Me"** (maximize safety): 8 → 4 → 2
Formal verification + RT ML + debugging = safety-critical certification

**Path C — "Build With Me"** (maximize ecosystem): 5 → 6 → 1
FFI + stdlib + playground = integrate with existing tools

**Path D — "Showcase"** (maximize wow factor): 3 → 4 → 2
Nova OS + RT ML + time-travel debugger = unique demo
