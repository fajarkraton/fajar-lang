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

## Option 2: Profiler & Time-Travel Debugger (8 sprints, 80 tasks)

**Goal:** Production-grade profiling + reverse debugging for embedded ML code
**Impact:** Debug tensor shape mismatches, memory leaks, and performance bottlenecks

### Phase D1: Profiler (3 sprints, 30 tasks)

#### Sprint D1.1: Instrumentation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1.1 | Function entry/exit hooks | Timestamp + call depth tracking | [ ] |
| D1.1.2 | Call graph builder | Parent→child edges with call counts | [ ] |
| D1.1.3 | Execution time per function | Wall clock + CPU time | [ ] |
| D1.1.4 | Memory allocation tracking | alloc/free pairs with sizes | [ ] |
| D1.1.5 | Tensor operation profiling | Shape, dtype, compute time per op | [ ] |
| D1.1.6 | Hot path detection | Top 10 functions by cumulative time | [ ] |
| D1.1.7 | Loop iteration counting | Iterations per loop with avg time | [ ] |
| D1.1.8 | Branch prediction stats | if/else taken ratio | [ ] |
| D1.1.9 | Sampling profiler | Statistical sampling at 1kHz | [ ] |
| D1.1.10 | Profile data format | Chrome Trace Event JSON | [ ] |

#### Sprint D1.2: Flamegraph Generation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.2.1 | Stack frame capture | Collapsed stack format | [ ] |
| D1.2.2 | SVG flamegraph renderer | Interactive zoom/filter | [ ] |
| D1.2.3 | Differential flamegraph | Compare two runs (red/blue) | [ ] |
| D1.2.4 | Reverse flamegraph | Callee-first (icicle chart) | [ ] |
| D1.2.5 | Memory flamegraph | Allocation-weighted stacks | [ ] |
| D1.2.6 | `fj profile` CLI command | `fj profile run examples/mnist.fj` | [ ] |
| D1.2.7 | HTML report generation | Self-contained single-file report | [ ] |
| D1.2.8 | Threshold filtering | Hide functions < 1% total time | [ ] |
| D1.2.9 | Source annotation | Click flamegraph → jump to source | [ ] |
| D1.2.10 | JSON export | For CI/CD regression tracking | [ ] |

#### Sprint D1.3: Memory Profiler (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.3.1 | Allocation timeline | Heap size over time | [ ] |
| D1.3.2 | Leak detection | Unreachable allocations at exit | [ ] |
| D1.3.3 | Allocation site tracking | File:line for each alloc | [ ] |
| D1.3.4 | Peak memory analysis | High-water mark + contributing allocs | [ ] |
| D1.3.5 | Tensor memory tracking | Tensor count, total bytes, peak | [ ] |
| D1.3.6 | Fragmentation analysis | Free block distribution | [ ] |
| D1.3.7 | Object graph dump | Reference chains for leak diagnosis | [ ] |
| D1.3.8 | `fj memprof` CLI | `fj memprof run program.fj` | [ ] |
| D1.3.9 | GC pressure metrics | Allocation rate, collection frequency | [ ] |
| D1.3.10 | Valgrind-style report | "Definitely lost", "possibly lost" | [ ] |

### Phase D2: Time-Travel Debugger (3 sprints, 30 tasks)

#### Sprint D2.1: Execution Recording (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1.1 | State snapshot on each statement | Variable values + PC | [ ] |
| D2.1.2 | Snapshot compression | Delta encoding (only changed vars) | [ ] |
| D2.1.3 | Circular buffer (1M snapshots) | Ring buffer with configurable depth | [ ] |
| D2.1.4 | Checkpoint system | Full snapshot every N statements | [ ] |
| D2.1.5 | Replay engine | Forward replay from checkpoint | [ ] |
| D2.1.6 | `step-back` command | Reverse single statement | [ ] |
| D2.1.7 | `reverse-continue` | Run backwards to previous breakpoint | [ ] |
| D2.1.8 | `reverse-next` | Step back over function calls | [ ] |
| D2.1.9 | Watchpoint trigger history | "When did x change to 42?" | [ ] |
| D2.1.10 | Recording overhead control | 2-5x slowdown acceptable | [ ] |

#### Sprint D2.2: DAP Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.2.1 | DAP reverse capabilities | `supportsStepBack: true` | [ ] |
| D2.2.2 | VS Code reverse debugging UI | Back arrows in debug toolbar | [ ] |
| D2.2.3 | Variable history | "Show all values of x" panel | [ ] |
| D2.2.4 | Execution timeline UI | Scrubber bar (drag to any point in time) | [ ] |
| D2.2.5 | Conditional reverse | "Go back to when `loss < 0.1`" | [ ] |
| D2.2.6 | Call stack history | Full call stack at each point | [ ] |
| D2.2.7 | Memory view at time T | Inspect heap state at any snapshot | [ ] |
| D2.2.8 | Tensor visualization | Show tensor values at each step | [ ] |
| D2.2.9 | Data breakpoints (reverse) | "What wrote to address 0xFF00?" | [ ] |
| D2.2.10 | Export trace | Save recording for offline analysis | [ ] |

#### Sprint D2.3: ML-Specific Debugging (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.3.1 | Gradient inspection | Show ∂loss/∂w at each backward step | [ ] |
| D2.3.2 | Tensor shape tracker | Shape changes through pipeline | [ ] |
| D2.3.3 | NaN/Inf detector | Break on first NaN in any tensor | [ ] |
| D2.3.4 | Loss curve live plot | Loss value at each training step | [ ] |
| D2.3.5 | Weight histogram | Distribution of weights per layer | [ ] |
| D2.3.6 | Activation visualization | Heatmap of layer outputs | [ ] |
| D2.3.7 | Gradient explosion detector | Alert when gradient norm > threshold | [ ] |
| D2.3.8 | Learning rate schedule plot | LR over time | [ ] |
| D2.3.9 | Batch data inspector | View input batch at each step | [ ] |
| D2.3.10 | Model architecture diagram | Auto-generated layer graph | [ ] |

### Phase D3: Integration (2 sprints, 20 tasks)

#### Sprint D3.1: CLI Tools (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.1.1 | `fj debug --record` | Enable recording mode | [ ] |
| D3.1.2 | `fj debug --replay file.trace` | Replay saved recording | [ ] |
| D3.1.3 | `fj profile --flamegraph` | Generate flamegraph SVG | [ ] |
| D3.1.4 | `fj profile --memory` | Memory profiling mode | [ ] |
| D3.1.5 | `fj profile --tensor` | Tensor operation profiling | [ ] |
| D3.1.6 | Profile comparison | `fj profile diff a.json b.json` | [ ] |
| D3.1.7 | CI integration | `fj profile --check --max-time 100ms` | [ ] |
| D3.1.8 | Benchmark regression | Fail CI if >10% slower | [ ] |
| D3.1.9 | Profile annotations | `@profile fn heavy_work()` | [ ] |
| D3.1.10 | REPL profiling | Profile expressions in REPL | [ ] |

#### Sprint D3.2: Documentation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.2.1 | Profiler user guide | Getting started + examples | [ ] |
| D3.2.2 | Debugger user guide | Breakpoints, stepping, reverse | [ ] |
| D3.2.3 | ML debugging tutorial | "Finding the vanishing gradient" | [ ] |
| D3.2.4 | Memory profiling tutorial | "Tracking tensor memory leaks" | [ ] |
| D3.2.5 | Flamegraph interpretation | How to read flamegraphs | [ ] |
| D3.2.6 | VS Code extension update | New debug/profile commands | [ ] |
| D3.2.7 | Performance optimization guide | Common patterns + fixes | [ ] |
| D3.2.8 | Embedded profiling guide | Profiling on Dragon Q6A | [ ] |
| D3.2.9 | API reference | Profiler/debugger Rust APIs | [ ] |
| D3.2.10 | Blog: "Time-Travel Debugging" | Announcement + demo video | [ ] |

---

## Option 3: FajarOS Nova v3.0 "Aurora" (12 sprints, 120 tasks)

**Goal:** Multi-core SMP, USB 3.0, display manager, sound, in-kernel package manager
**Impact:** Production-quality desktop OS written 100% in Fajar Lang

### Phase A1: SMP & Scheduler v2 (3 sprints, 30 tasks)

*(Per-CPU run queues, work stealing, CPU affinity, load balancing, IPI,
NUMA awareness, tickless idle, CFS-like fair scheduling, priority inheritance,
RT scheduling class, CPU hotplug, per-CPU variables, spinlock with backoff,
RCU read-side, preemption points, migration threads, /proc/cpuinfo live,
htop-style process monitor, SMP benchmarks, SMP stress test)*

### Phase A2: USB 3.0 & Device Framework (3 sprints, 30 tasks)

*(xHCI controller init, device enumeration, USB hub support, bulk/interrupt/isochronous
transfers, USB mass storage (SCSI), USB HID (keyboard/mouse), USB audio class,
USB ethernet adapter, USB device hot-plug/unplug, devfs entries, udev-style rules,
device power management, USB 3.0 SuperSpeed, descriptor parsing, endpoint management,
driver registration framework, bus scan on boot, USB serial/FTDI, lsusb command,
USB benchmark)*

### Phase A3: Display Manager & Compositor (3 sprints, 30 tasks)

*(Multi-monitor detection, mode setting (VESA/GOP), resolution switching,
Wayland-style compositor protocol, surface allocation, damage tracking,
vsync double/triple buffering, alpha blending, window animations (fade/slide),
screen rotation, DPI scaling, cursor themes, wallpaper engine, screen lock,
multi-desktop/workspace, notification popups, clipboard manager, drag-and-drop,
screenshot with region select, screen recording to raw video, window snapping
(left/right/maximize), alt-tab switcher, compositor benchmark, display benchmark,
GPU-accelerated compositing, font rendering with anti-aliasing (subpixel))*

### Phase A4: System Services (3 sprints, 30 tasks)

*(Sound server (mixing daemon), PulseAudio-style API, Bluetooth stack (HCI + L2CAP),
WiFi WPA2 supplicant, power management daemon, battery monitor, systemd-style
service manager, service dependencies, watchdog timer, core dump handler,
swap partition, tmpfs, devtmpfs, kernel module loading, sysctl interface,
dmesg ring buffer, kernel panic handler with stack trace, out-of-memory killer,
ACPI power button handler, suspend/resume, RTC alarm wakeup, thermal throttling,
fan control, CPU frequency scaling (cpufreq), system update mechanism,
initramfs unpacking, boot splash screen, system benchmark suite)*

---

## Option 4: Real-Time ML Pipeline (6 sprints, 60 tasks)

**Goal:** End-to-end sensor → inference → actuator pipeline with latency guarantees
**Impact:** Core differentiator — no other language does this with compiler-enforced safety

### Phase R1: Sensor Framework (2 sprints, 20 tasks)

#### Sprint R1.1: Sensor Abstraction (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.1.1 | Sensor trait | `trait Sensor { fn read() -> SensorData }` | [ ] |
| R1.1.2 | SensorData type | Timestamped, typed sensor readings | [ ] |
| R1.1.3 | IMU driver | Accelerometer + gyroscope (MPU6050) | [ ] |
| R1.1.4 | Camera frame capture | Raw frame → Tensor conversion | [ ] |
| R1.1.5 | ADC driver | Analog-to-digital (temperature, pressure) | [ ] |
| R1.1.6 | GPS NMEA parser | Lat/lon/alt from UART GPS module | [ ] |
| R1.1.7 | LiDAR point cloud | Distance array from scanning LiDAR | [ ] |
| R1.1.8 | Microphone PCM | Audio samples for voice detection | [ ] |
| R1.1.9 | Sensor fusion | Kalman filter for IMU + GPS | [ ] |
| R1.1.10 | Sensor data logger | Ring buffer with timestamp | [ ] |

#### Sprint R1.2: Data Pipeline (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.2.1 | Pipeline DSL | `sensor |> preprocess |> infer |> act` | [ ] |
| R1.2.2 | Preprocessing stage | Normalization, windowing, FFT | [ ] |
| R1.2.3 | Feature extraction | Rolling mean, variance, peak detection | [ ] |
| R1.2.4 | Batching strategy | Collect N samples before inference | [ ] |
| R1.2.5 | Ring buffer allocator | Zero-copy circular buffer | [ ] |
| R1.2.6 | Backpressure handling | Drop oldest on overflow | [ ] |
| R1.2.7 | Multi-sensor fusion | Align timestamps across sensors | [ ] |
| R1.2.8 | Data augmentation | Random noise, rotation for training | [ ] |
| R1.2.9 | Streaming windowing | Sliding window with overlap | [ ] |
| R1.2.10 | Pipeline benchmark | End-to-end latency measurement | [ ] |

### Phase R2: Inference Engine (2 sprints, 20 tasks)

#### Sprint R2.1: Model Runtime (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R2.1.1 | Model loader | FJML/ONNX/TFLite → runtime graph | [ ] |
| R2.1.2 | Inference scheduler | Priority queue with deadline | [ ] |
| R2.1.3 | @device inference context | Tensor ops isolated from kernel | [ ] |
| R2.1.4 | Batch inference | Multiple inputs in one pass | [ ] |
| R2.1.5 | Model hot-swap | Replace model without restart | [ ] |
| R2.1.6 | Multi-model pipeline | Chain: detector → classifier → tracker | [ ] |
| R2.1.7 | Confidence threshold | Filter low-confidence predictions | [ ] |
| R2.1.8 | Inference caching | Cache repeated inputs (LRU) | [ ] |
| R2.1.9 | Quantized inference | INT8 fast path on CPU/NPU | [ ] |
| R2.1.10 | Latency SLA | Guarantee < 10ms per inference | [ ] |

#### Sprint R2.2: Actuator Framework (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R2.2.1 | Actuator trait | `trait Actuator { fn act(cmd: Command) }` | [ ] |
| R2.2.2 | PWM motor control | Speed + direction via PWM duty cycle | [ ] |
| R2.2.3 | Servo control | Angular position (0-180°) | [ ] |
| R2.2.4 | GPIO digital output | On/off for relays, LEDs | [ ] |
| R2.2.5 | CAN bus command | Automotive actuator commands | [ ] |
| R2.2.6 | Safety interlock | Emergency stop on anomaly detection | [ ] |
| R2.2.7 | PID controller | Proportional-integral-derivative loop | [ ] |
| R2.2.8 | Command smoothing | Ramp rate limiting for motors | [ ] |
| R2.2.9 | Actuator feedback | Closed-loop with sensor reading | [ ] |
| R2.2.10 | Fail-safe defaults | Safe state on communication loss | [ ] |

### Phase R3: Integration & Demo (2 sprints, 20 tasks)

#### Sprint R3.1: End-to-End Pipeline (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R3.1.1 | @kernel → @device bridge | Zero-copy sensor data → tensor | [ ] |
| R3.1.2 | @device → @kernel bridge | Inference result → actuator command | [ ] |
| R3.1.3 | @safe orchestrator | Pipeline coordination with error handling | [ ] |
| R3.1.4 | Deadline scheduler | Hard real-time task priorities | [ ] |
| R3.1.5 | Jitter measurement | < 1ms variance on 10ms deadline | [ ] |
| R3.1.6 | Worst-case execution time | Static WCET analysis | [ ] |
| R3.1.7 | Priority inversion prevention | Priority inheritance protocol | [ ] |
| R3.1.8 | Watchdog integration | Reset on missed deadline | [ ] |
| R3.1.9 | Telemetry export | Pipeline metrics → serial/network | [ ] |
| R3.1.10 | Power-aware scheduling | Reduce frequency when idle | [ ] |

#### Sprint R3.2: Demo Applications (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R3.2.1 | Drone autopilot | IMU → stabilization → motor control | [ ] |
| R3.2.2 | Object tracker | Camera → YOLO → servo follow | [ ] |
| R3.2.3 | Anomaly detector | Vibration sensor → FFT → classifier → alert | [ ] |
| R3.2.4 | Voice command | Microphone → keyword detection → GPIO | [ ] |
| R3.2.5 | Autonomous rover | LiDAR → obstacle avoid → motor | [ ] |
| R3.2.6 | Predictive maintenance | Sensor trends → failure prediction → alert | [ ] |
| R3.2.7 | Smart agriculture | Soil moisture → irrigation control | [ ] |
| R3.2.8 | Industrial quality control | Camera → defect detection → reject gate | [ ] |
| R3.2.9 | Pipeline benchmark report | All demos with latency/accuracy | [ ] |
| R3.2.10 | Blog: "RT ML in Fajar Lang" | Architecture + benchmarks | [ ] |

---

## Option 5: FFI v2 — C++/Python/Rust Interop (5 sprints, 50 tasks)

**Goal:** Seamlessly call C++, Python, and Rust libraries from Fajar Lang
**Impact:** Access entire ecosystems (PyTorch, OpenCV, Tokio) without reimplementing

### Phase F1: C++ Interop (2 sprints, 20 tasks)

*(C++ header parsing via libclang, name mangling, class method calls, template
instantiation, RAII bridging, std::string/std::vector conversion, exception handling
(catch → Result), namespace resolution, `extern "C++" {}` blocks, smart pointer bridging,
virtual method dispatch, operator overloading bridge, compile-time type mapping,
CMake integration, pkg-config support, OpenCV bridge demo, Eigen matrix bridge,
C++ → Fajar callback, ABI compatibility check, FFI benchmark)*

### Phase F2: Python Interop (2 sprints, 20 tasks)

*(CPython embedding via libpython, PyObject conversion, NumPy → Tensor bridge,
call Python functions from Fajar, call Fajar functions from Python, GIL management,
module import system, dict/list/tuple conversion, exception → Result mapping,
`@python` annotation for hybrid code, Jupyter kernel for Fajar Lang,
pandas DataFrame → Map conversion, matplotlib bridge for plotting,
pip package wrapping, PyTorch model loading, virtual environment detection,
`fj python-bridge` CLI, async Python ↔ async Fajar, memory sharing (zero-copy),
Python interop benchmark)*

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

## Option 6: Standard Library v3 (8 sprints, 80 tasks)

**Goal:** Comprehensive stdlib rivaling Rust/Go for real-world applications
**Impact:** Self-sufficient language — no need for FFI for common tasks

### Phase S1: Async Networking (2 sprints, 20 tasks)

*(TCP client/server, UDP sockets, HTTP client (GET/POST/PUT/DELETE), HTTP server
with routing, WebSocket client/server, DNS resolver, TLS/SSL via rustls,
connection pooling, timeout/retry, async stream reading, multipart form data,
URL parsing, cookie handling, HTTP/2 multiplexing, keep-alive, proxy support,
SOCKS5 support, rate limiting, circuit breaker, network benchmark)*

### Phase S2: Crypto & Security (2 sprints, 20 tasks)

*(SHA-256/384/512, HMAC, AES-128/256 (CBC/GCM), RSA 2048/4096,
Ed25519 signing/verification, X25519 key exchange, PBKDF2/Argon2 password hashing,
random bytes (CSPRNG), Base64 encode/decode, Hex encode/decode,
JWT creation/verification, X.509 certificate parsing, TLS certificate validation,
constant-time comparison, secure memory zeroing, key derivation (HKDF),
digital signature (ECDSA P-256), certificate chain validation, crypto benchmark,
FIPS 140-2 compliance check)*

### Phase S3: Data Formats (2 sprints, 20 tasks)

*(JSON parser/serializer (streaming), TOML parser, YAML parser, CSV reader/writer,
XML SAX parser, MessagePack binary serialization, Protocol Buffers (code gen),
Regular expressions (NFA/DFA engine), date/time (ISO 8601, RFC 3339, timezone),
UUID v4/v7 generation, URI/URL parser, MIME type detection, gzip/deflate compression,
zstd compression, tar archive read/write, zip archive read/write,
INI file parser, environment variable expansion, template engine, format benchmark)*

### Phase S4: System & Utilities (2 sprints, 20 tasks)

*(Path manipulation (join, parent, extension, normalize), directory walking (recursive),
file watching (inotify), temp file/dir creation, file locking (advisory),
process spawning (fork/exec with pipe), signal handling, environment variables,
command-line argument parsing (clap-style), progress bar, colored terminal output,
table formatting, logging framework (levels, sinks, formatting), timer/stopwatch,
thread pool, channel (bounded/unbounded), concurrent HashMap, atomic counter,
rate limiter (token bucket), utility benchmark)*

---

## Option 7: Language Server v3 (5 sprints, 50 tasks)

**Goal:** World-class IDE experience — on par with rust-analyzer
**Impact:** Developer productivity; IDE experience often determines language adoption

### Phase L1: Semantic Analysis (2 sprints, 20 tasks)

*(Semantic tokens (24 token types, 8 modifiers), go-to-definition (cross-file),
find all references, go-to-implementation, type hierarchy, call hierarchy
(incoming/outgoing), workspace symbol search, document symbol outline,
import suggestions, unused import detection, dead code dimming,
type-on-hover with full signature, parameter info with active parameter,
inlay hints (types, parameter names, chained methods), lens: test count per function,
lens: reference count, lens: impl count for trait, signature help with overload,
semantic folding ranges, breadcrumb navigation)*

### Phase L2: Refactoring (2 sprints, 20 tasks)

*(Rename symbol (cross-file), extract function, extract variable, inline variable,
inline function, convert `if-else` to `match`, convert `match` to `if-else`,
add missing match arms, generate trait impl stubs, generate constructor,
add type annotation, remove unused imports, organize imports (sort + group),
convert `for` to iterator chain, wrap in `Some()`/`Ok()`, add `?` operator,
generate documentation comment, convert string to f-string, move item to new file,
refactoring preview (diff before apply))*

### Phase L3: Diagnostics & Fixes (1 sprint, 10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L3.1 | Quick fix: add missing import | Auto-insert `use` statement | [ ] |
| L3.2 | Quick fix: add type annotation | Infer and insert type | [ ] |
| L3.3 | Quick fix: fix typo | "Did you mean `println`?" | [ ] |
| L3.4 | Quick fix: make mutable | Add `mut` when reassigned | [ ] |
| L3.5 | Quick fix: add missing field | Struct literal completion | [ ] |
| L3.6 | Quick fix: implement trait | Generate method stubs | [ ] |
| L3.7 | Diagnostic: ownership error | Suggest clone/borrow | [ ] |
| L3.8 | Diagnostic: type mismatch | Show expected vs actual | [ ] |
| L3.9 | Diagnostic: unreachable code | Gray out dead branches | [ ] |
| L3.10 | Diagnostic: deprecated API | Strikethrough + suggestion | [ ] |

---

## Option 8: Formal Verification v2 (6 sprints, 60 tasks)

**Goal:** Prove program correctness with pre/post conditions, invariants, and SMT solver
**Impact:** Safety-critical certification (DO-178C, ISO 26262) for embedded ML

### Phase V1: Specification Language (2 sprints, 20 tasks)

#### Sprint V1.1: Annotations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V1.1.1 | `@requires` precondition | `@requires(x > 0)` | [ ] |
| V1.1.2 | `@ensures` postcondition | `@ensures(result >= 0)` | [ ] |
| V1.1.3 | `@invariant` loop invariant | `@invariant(i < n)` | [ ] |
| V1.1.4 | `@assert` proof obligation | Compile-time assertion | [ ] |
| V1.1.5 | `@decreases` termination proof | Variant expression | [ ] |
| V1.1.6 | `old(x)` expression | Value of x at function entry | [ ] |
| V1.1.7 | `forall` quantifier | `@ensures(forall(i, 0..n, arr[i] >= 0))` | [ ] |
| V1.1.8 | `exists` quantifier | Existential quantification | [ ] |
| V1.1.9 | Ghost variables | Specification-only state | [ ] |
| V1.1.10 | Spec parsing in analyzer | Parse and validate annotations | [ ] |

#### Sprint V1.2: Verification Conditions (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V1.2.1 | Weakest precondition calculus | wp(stmt, postcondition) | [ ] |
| V1.2.2 | SSA transformation | Single static assignment for VC gen | [ ] |
| V1.2.3 | Loop invariant checking | Verify invariant holds at entry + preserved | [ ] |
| V1.2.4 | Function contract verification | Precondition→body→postcondition | [ ] |
| V1.2.5 | Array bounds proof | Prove `i < len(arr)` statically | [ ] |
| V1.2.6 | Integer overflow proof | Prove no overflow in arithmetic | [ ] |
| V1.2.7 | Null safety proof | Prove Option unwrap is safe | [ ] |
| V1.2.8 | Division by zero proof | Prove divisor != 0 | [ ] |
| V1.2.9 | Termination proof | Prove decreasing variant | [ ] |
| V1.2.10 | VC export (SMT-LIB2) | Export verification conditions | [ ] |

### Phase V2: SMT Solver Integration (2 sprints, 20 tasks)

*(Z3 integration via z3-sys FFI, SMT-LIB2 format generation, bitvector theory for
integer ops, array theory for tensor verification, real arithmetic for floating point,
solver timeout (5s per VC), counterexample extraction, counter-model display,
incremental solving (push/pop), proof caching, parallel verification (per-function),
quantifier instantiation heuristics, unsat core extraction for error localization,
theory combination (arrays + bitvectors + reals), custom Fajar theory plugin,
solver benchmarking (Z3 vs CVC5), verification result caching, CI integration
(`fj verify`), verification coverage report, SMT solver fallback chain)*

### Phase V3: Tensor Shape Verification (2 sprints, 20 tasks)

*(Tensor shape as dependent types, matmul shape compatibility proof,
reshape validity proof, broadcast rule verification, conv2d output shape calculation,
concatenation axis validation, split size validation, transpose permutation check,
batch dimension tracking through pipeline, shape polymorphism (unknown dims),
symbolic shape variables, shape constraint propagation, shape error messages
with expected vs actual, ONNX shape inference verification, training shape compatibility
(forward matches backward), quantization shape preservation, shape verification
for custom layers, dynamic shape bounds, shape verification benchmark,
blog: "Proving ML Correctness")*

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
