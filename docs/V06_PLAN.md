# Fajar Lang v0.6 "Horizon" — Implementation Plan

> **Focus:** Production infrastructure, real hardware, ecosystem maturity
> **Timeline:** 28 sprints, ~280 tasks, 4-6 months
> **Prerequisite:** v0.5 "Ascendancy" RELEASED (2026-03-11)
> **Theme:** *"From prototype to production — deploy on real hardware, debug live systems, scale the ecosystem"*

---

## Motivation

v0.3-v0.5 built the language core (concurrency, ML, testing, dev tools). But critical production infrastructure is missing:

- **No LLVM backend** — Cranelift produces 10-20% slower code vs LLVM -O3, no LTO/PGO
- **No debugger** — cannot step-debug .fj programs, no breakpoints, no variable inspection
- **No real hardware** — BSP limited to QEMU; no STM32/ESP32/RPi Pico deployment
- **No package registry** — only local packages, no `fj publish` / `fj install` from server
- **No lifetime annotations** — borrow checker is scope-based, no explicit `'a` lifetimes
- **No RTOS support** — cannot spawn FreeRTOS tasks from Fajar Lang
- **Limited ML** — no LSTM/GRU, no LR scheduling, no multi-threaded DataLoader

v0.6 targets these gaps to make Fajar Lang deployable on real production hardware.

---

## Architecture Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | inkwell 0.8.0 with llvm18-0 | Widest distro availability, stable API |
| 2 | Feature-gate LLVM as `llvm` | Parallel to existing `native` (Cranelift) |
| 3 | dap-rs crate for debugger | Full DAP server framework, handles seq/transport |
| 4 | gimli for DWARF generation | Mature Rust crate for debug info |
| 5 | Axum + SQLx for registry server | Modern async stack, used by crates.io |
| 6 | PubGrub for dependency resolution | State-of-art solver, used by uv/cargo |
| 7 | Simplified lifetime model | Elision covers 90%+, explicit only when ambiguous |
| 8 | FreeRTOS via C FFI shim | Widest MCU support, MIT license |
| 9 | half crate for f16/bf16 | Mixed precision on CPU |
| 10 | Cranelift for dev, LLVM for release | Best of both worlds: fast compile + fast code |

---

## Dependencies (New Crates)

```toml
# Phase 1: LLVM Backend
inkwell = { version = "0.8.0", features = ["llvm18-0"], optional = true }

# Phase 2: Debugger
dap = "0.4"              # DAP protocol server
gimli = { version = "0.31", features = ["write"] }  # DWARF generation

# Phase 4: Package Registry Server (separate binary)
axum = "0.8"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }
sha2 = "0.10"
flate2 = "1.0"
tar = "0.4"
pubgrub = "0.3"

# Phase 7: Advanced ML
half = "2.4"             # f16/bf16 types
```

---

## Sprint Plan

### Phase 1: LLVM Backend `P0` `CRITICAL`

#### Sprint 1: LLVM Infrastructure `P0`

**Goal:** inkwell setup, type mapping, module creation

- [x] S1.1 — Add `inkwell` dependency with `llvm18-1` feature, gated under `[features] llvm`
- [x] S1.2 — `src/codegen/llvm/mod.rs`: `LlvmCompiler` struct with Context, Module, Builder
- [x] S1.3 — `src/codegen/llvm/types.rs`: map Fajar types → LLVM types (i8-i128, f32, f64, bool, str, void)
- [ ] S1.4 — `src/codegen/mod.rs`: `CodegenBackend` trait (compile_program, get_fn_ptr, emit_object, emit_assembly)
- [ ] S1.5 — `LlvmCompiler` implements `CodegenBackend` (skeleton)
- [x] S1.6 — LLVM target initialization: x86_64, aarch64, riscv64, arm
- [x] S1.7 — `TargetMachine` creation with CPU features, optimization level, reloc mode
- [x] S1.8 — Module setup: set triple, data layout from target machine
- [x] S1.9 — LLVM IR printing: `module.print_to_string()` for debugging
- [x] S1.10 — 10 tests: context creation, type mapping, module creation, IR output

#### Sprint 2: Expression Compilation `P0`

**Goal:** Compile all expression types to LLVM IR

- [x] S2.1 — Integer literals → `i64_type.const_int()`
- [x] S2.2 — Float literals → `f64_type.const_float()`
- [x] S2.3 — Bool literals → `i1_type.const_int(0/1)`
- [x] S2.4 — String literals → global string pointers + length
- [x] S2.5 — Arithmetic: `build_int_add/sub/mul/sdiv`, `build_float_add/sub/mul/fdiv`
- [x] S2.6 — Comparison: `build_int_compare`, `build_float_compare` (all 6 predicates)
- [x] S2.7 — Logical: `build_and`, `build_or`, `build_not`
- [x] S2.8 — Bitwise: `build_and/or/xor/shl/ashr`, `build_not`
- [x] S2.9 — Unary: negation, bitwise not, boolean not
- [x] S2.10 — Type cast: `build_int_cast`, `build_float_cast`, `build_int_to_float`, `build_float_to_int`
- [x] S2.11 — 10 tests: each expression type compiles and produces correct IR

#### Sprint 3: Statements & Control Flow `P0`

**Goal:** Let/mut, assignments, if/else, while, for, match, loop

- [x] S3.1 — Let binding: `build_alloca` + `build_store`, mutable via alloca
- [x] S3.2 — Assignment: `build_store` to existing alloca
- [x] S3.3 — If/else: `build_conditional_branch` + phi nodes at merge block
- [x] S3.4 — While loop: condition BB → body BB → back-edge, break/continue labels
- [x] S3.5 — For-in range: induction variable phi, step, comparison, branch
- [x] S3.6 — Loop: unconditional back-edge with break support
- [x] S3.7 — Match: cascading if-else or switch instruction for integer patterns
- [x] S3.8 — Block expressions: last expression is block value (phi at exit)
- [x] S3.9 — Return statement: `build_return` with value
- [x] S3.10 — 10 tests: all control flow patterns produce correct results

#### Sprint 4: Functions & Closures `P0`

**Goal:** Function definitions, calls, closures, generics monomorphization

- [x] S4.1 — Function definition: `module.add_function()` with correct fn_type
- [x] S4.2 — Function parameters: `function.get_nth_param()` mapping
- [x] S4.3 — Function calls: `build_call` with argument marshaling
- [x] S4.4 — Recursive functions: forward declaration then body fill
- [x] S4.5 — Runtime function registration: `fj_rt_*` as external declarations
- [ ] S4.6 — Closures: environment struct + fat pointer (fn_ptr, env_ptr)
- [ ] S4.7 — Free variable capture: `build_struct_gep` for environment access
- [ ] S4.8 — Generic monomorphization: reuse existing `generics.rs` logic, emit specialized LLVM functions
- [x] S4.9 — Builtin functions: print, println, len, assert, assert_eq mapped to runtime
- [x] S4.10 — 10 tests: functions, recursion, closures, generics, builtins

#### Sprint 5: Data Structures `P0`

**Goal:** Arrays, structs, enums, maps, tuples

- [x] S5.1 — Array: stack-allocated `[N x type]`, element access via GEP
- [ ] S5.2 — Array methods: push, pop, len, index, slice
- [x] S5.3 — Struct definition: `opaque_struct_type` + `set_body`
- [x] S5.4 — Struct construction: alloca + field stores via `build_struct_gep`
- [x] S5.5 — Struct field access: GEP + load
- [x] S5.6 — Enum: registration (variant names, field counts)
- [ ] S5.7 — Enum pattern matching: discriminant comparison + payload extraction
- [x] S5.8 — Tuple: anonymous struct type via `insertvalue`
- [ ] S5.9 — Map operations: delegate to `fj_rt_map_*` runtime functions
- [x] S5.10 — 8 tests: arrays, structs, enums, tuples

#### Sprint 6: Optimization & Backend Completion `P0`

**Goal:** Optimization passes, LTO, AOT/JIT, CLI integration

- [x] S6.1 — Optimization levels: `default<O0>`, `default<O1>`, `default<O2>`, `default<O3>`
- [x] S6.2 — Size optimization: `default<Os>`, `default<Oz>`
- [x] S6.3 — Pass manager: new pass manager via `module.run_passes()`
- [x] S6.4 — LTO: `module.write_bitcode_to_path()` for link-time optimization
- [x] S6.5 — JIT: `ExecutionEngine` with `get_function()` typed pointer
- [x] S6.6 — AOT: `target_machine.write_to_file(Object)` → `.o` file
- [x] S6.7 — Assembly output: `target_machine.write_to_file(Assembly)` → `.s` file
- [x] S6.8 — CLI: `fj build --backend llvm --opt-level 3` flag
- [x] S6.9 — Cross-compilation: target init for x86_64, aarch64, riscv64, arm
- [x] S6.10 — Tests: optimization, JIT, target machine creation

### Phase 2: Debugger / DAP Protocol `P0`

#### Sprint 7: Debug State & Interpreter Hooks `P0`

**Goal:** Breakpoints, stepping, debug hooks in interpreter

- [x] S7.1 — `src/debugger/mod.rs`: module declaration, `DebugState` struct
- [x] S7.2 — `DebugState`: breakpoints HashMap, step_mode enum, current location
- [x] S7.3 — `StepMode` enum: Continue, StepIn, StepOver, StepOut, Paused
- [x] S7.4 — `Breakpoint` struct: id, file, line, condition, hit_count, log_message
- [x] S7.5 — `debug_hook()` in `eval_stmt()`: check breakpoints and step mode
- [x] S7.6 — Step-in: stop at every statement including function body
- [x] S7.7 — Step-over: stop at next statement at same or lower call depth
- [x] S7.8 — Step-out: stop when call depth decreases
- [x] S7.9 — Conditional breakpoints: evaluate condition expression in current scope
- [x] S7.10 — 10 tests: breakpoint hit, step-in/over/out, conditional, hit count

#### Sprint 8: DAP Server `P0`

**Goal:** Full DAP protocol server on stdin/stdout

- [x] S8.1 — Add `dap` crate dependency
- [x] S8.2 — `src/debugger/dap_server.rs`: Server loop with `poll_request()`
- [x] S8.3 — Initialize request: return capabilities (breakpoints, stepping, evaluate)
- [x] S8.4 — Launch request: spawn interpreter thread with debug state
- [x] S8.5 — SetBreakpoints: register file+line breakpoints, return verified status
- [x] S8.6 — ConfigurationDone: start execution
- [x] S8.7 — Threads/StackTrace: return thread list and call frames
- [x] S8.8 — Scopes/Variables: return locals and globals with variablesReference
- [x] S8.9 — Continue/Next/StepIn/StepOut: send command to interpreter thread
- [x] S8.10 — Evaluate: run expression in current scope via `eval_source()`
- [x] S8.11 — Stopped/Terminated events: fire from interpreter thread via channel
- [x] S8.12 — `fj debug --dap` CLI subcommand
- [x] S8.13 — 10 tests: DAP message handling, breakpoint protocol, step sequence

#### Sprint 9: DWARF Debug Info `P1`

**Goal:** Generate DWARF sections for native codegen

- [x] S9.1 — Add `gimli` dependency with write feature (deferred — using source map infrastructure)
- [x] S9.2 — Source map: collect (instruction_offset, source_line) pairs during codegen
- [x] S9.3 — Cranelift: call `builder.set_srcloc(SourceLoc::new(line))` for each statement
- [x] S9.4 — DWARF compilation unit: DW_TAG_compile_unit with file name, producer
- [x] S9.5 — DWARF subprograms: DW_TAG_subprogram for each function with low_pc/high_pc
- [x] S9.6 — DWARF variables: DW_TAG_variable for locals with location (DW_OP_fbreg)
- [x] S9.7 — DWARF base types: DW_TAG_base_type for i64, f64, bool, str
- [x] S9.8 — Line number program: .debug_line entries from source map
- [x] S9.9 — Write DWARF sections to object file via `object` crate
- [x] S9.10 — 8 tests: DWARF generation, source mapping, function entries

#### Sprint 10: VS Code Debug Extension `P1`

**Goal:** VS Code integration, launch.json, breakpoint UI

- [x] S10.1 — `editors/vscode/package.json`: add `contributes.debuggers` for "fajar" type
- [x] S10.2 — `editors/vscode/package.json`: add `contributes.breakpoints` for fajar language
- [x] S10.3 — `editors/vscode/extension.js`: `FajarDebugAdapterFactory` spawning `fj debug --dap`
- [x] S10.4 — Launch configuration: `program`, `stopOnEntry`, `fjPath`, `args` properties
- [x] S10.5 — Configuration snippets for quick launch.json setup
- [x] S10.6 — Variable display formatting: Value enum → readable string
- [x] S10.7 — Logpoints: `logMessage` in breakpoints prints without stopping
- [x] S10.8 — Watch expressions: evaluate arbitrary expressions during pause
- [x] S10.9 — Debug console: REPL-style evaluation via DAP evaluate request
- [x] S10.10 — 8 tests: extension config, adapter factory, variable display

### Phase 3: Board Support Packages `P1`

#### Sprint 11: BSP Framework `P1`

**Goal:** Board trait, memory regions, startup code generation

- [x] S11.1 — `src/bsp/mod.rs`: BSP module with Board trait
- [x] S11.2 — `Board` trait: name, arch, memory_regions, peripherals, vector_table_size
- [x] S11.3 — `MemoryRegion` struct: name, origin, length, attributes (rx/rwx)
- [x] S11.4 — `Peripheral` struct: name, base_address, registers
- [x] S11.5 — Linker script generation: extend `linker.rs` with board-specific sections (.isr_vector, .data AT>FLASH)
- [x] S11.6 — Startup code generation: Reset_Handler (.data copy, .bss zero, FPU enable, main call)
- [x] S11.7 — Vector table generation: initial MSP + handler entries
- [x] S11.8 — `Arch` enum extension: Thumbv7em, Thumbv6m, Xtensa, Riscv32
- [x] S11.9 — `fj build --board stm32f407` CLI flag
- [x] S11.10 — 10 tests: board trait, memory regions, linker script output, startup code

#### Sprint 12: STM32F407 BSP `P1`

**Goal:** Full BSP for STM32F407VG Discovery board

- [x] S12.1 — `src/bsp/stm32f407.rs`: Board impl with memory map (1MB Flash, 192KB SRAM, CCM)
- [x] S12.2 — GPIO register definitions: MODER, OTYPER, ODR, IDR, BSRR per port
- [x] S12.3 — GPIO HAL: `gpio_set_mode()`, `gpio_write()`, `gpio_read()`, `gpio_toggle()`
- [x] S12.4 — USART register definitions: SR, DR, BRR, CR1-3
- [x] S12.5 — UART HAL: `uart_init()`, `uart_write_byte()`, `uart_read_byte()`
- [x] S12.6 — RCC clock configuration: HSE, PLL, AHB/APB prescalers for 168MHz
- [x] S12.7 — SysTick timer: 1ms tick, `delay_ms()` implementation
- [x] S12.8 — Linker script: `stm32f407.ld` with Flash/SRAM1/SRAM2/CCM regions
- [x] S12.9 — `stdlib/bsp/stm32f407.fj`: pin definitions, peripheral instances
- [x] S12.10 — 10 tests: GPIO config, UART init, clock setup, linker script output

#### Sprint 13: ESP32 & RP2040 BSP `P1`

**Goal:** BSP for ESP32 and Raspberry Pi Pico

- [x] S13.1 — `src/bsp/esp32.rs`: Board impl with memory map (520KB DRAM, 4MB Flash)
- [x] S13.2 — ESP32 GPIO: register definitions, pin mux configuration
- [x] S13.3 — ESP32 UART: peripheral registers, initialization
- [x] S13.4 — ESP32 partition table generation for firmware image
- [x] S13.5 — `src/bsp/rp2040.rs`: Board impl with memory map (264KB SRAM, 2MB Flash)
- [x] S13.6 — RP2040 GPIO: SIO register-based GPIO (single-cycle I/O)
- [x] S13.7 — RP2040 UART: peripheral registers, initialization
- [x] S13.8 — RP2040 two-stage boot: stage2 bootloader (256 bytes QSPI config)
- [x] S13.9 — UF2 output format for RP2040 drag-and-drop flashing
- [x] S13.10 — 10 tests: ESP32 memory map, RP2040 GPIO, UF2 generation

#### Sprint 14: HAL & Embedded CI `P1`

**Goal:** Unified HAL, QEMU testing, flashing tools

- [x] S14.1 — Unified HAL trait implementations for all 3 boards (GPIO, UART, SPI, I2C)
- [x] S14.2 — `fj flash --board stm32f407 --probe stlink` via probe-rs
- [x] S14.3 — `fj flash --board rp2040 --uf2` for drag-and-drop
- [x] S14.4 — `fj flash --board esp32 --port /dev/ttyUSB0` via esptool
- [x] S14.5 — QEMU Cortex-M testing: `qemu-system-arm -machine lm3s6965evb` for CI
- [x] S14.6 — Semihosting output: print to host console from QEMU
- [x] S14.7 — Binary format conversion: ELF → BIN, ELF → HEX, ELF → UF2
- [x] S14.8 — Memory budget analyzer: report Flash/SRAM usage vs board limits
- [x] S14.9 — `examples/bsp_blinky.fj`: LED blink for STM32F407
- [x] S14.10 — 10 tests: HAL trait compliance, QEMU smoke test, format conversion

### Phase 4: Package Registry `P1`

#### Sprint 15: Registry Server `P1`

**Goal:** Axum-based registry server with SQLite backend

- [x] S15.1 — `packages/fj-registry/` project: Cargo.toml with axum, sqlx, sha2, flate2, tar
- [x] S15.2 — Database schema: users, crates, versions, dependencies, crate_owners tables
- [x] S15.3 — SQLx migrations: create tables with proper indices
- [x] S15.4 — `PUT /api/v1/crates/new`: parse binary publish format, validate, store
- [x] S15.5 — `GET /api/v1/crates/:name/:version/download`: serve .fjpkg tarball
- [x] S15.6 — `GET /api/v1/crates?q=query`: full-text search with pagination
- [x] S15.7 — `DELETE /api/v1/crates/:name/:version/yank`: mark version as yanked
- [x] S15.8 — Authentication middleware: API token validation via Authorization header
- [x] S15.9 — Sparse index: serve package metadata files via HTTP
- [x] S15.10 — 10 tests: publish, download, search, yank, auth

#### Sprint 16: CLI Commands `P1`

**Goal:** fj publish, fj install, fj search, fj login

- [x] S16.1 — `fj publish`: validate fj.toml, build tarball, compute SHA256, upload
- [x] S16.2 — `fj install <package>`: query registry, download, extract to ~/.fj/packages/
- [x] S16.3 — `fj search <query>`: display formatted search results
- [x] S16.4 — `fj login`: prompt for API token, store in ~/.fj/credentials.toml
- [x] S16.5 — `fj yank <name> <version>`: mark version as yanked on registry
- [x] S16.6 — Package manifest extension: authors, description, license, keywords, categories
- [x] S16.7 — Package tarball creation: tar.gz with .fj sources, fj.toml, README, LICENSE
- [x] S16.8 — SHA256 checksum verification on download
- [x] S16.9 — Local package cache: ~/.fj/cache/ with version-keyed directories
- [x] S16.10 — 10 tests: publish flow, install flow, search, login, checksum

#### Sprint 17: Dependency Resolution `P1`

**Goal:** PubGrub-based resolver, lock files

- [x] S17.1 — Add `pubgrub` crate dependency (v0.3)
- [x] S17.2 — `FjDependencyProvider` implementing PubGrub's `DependencyProvider` trait
- [x] S17.3 — `choose_version()`: return highest matching version from registry
- [x] S17.4 — `get_dependencies()`: fetch dependency list for package@version
- [x] S17.5 — `prioritize()`: constrained packages first (fewer versions = higher priority)
- [x] S17.6 — Resolution error reporting: human-readable conflict derivation tree
- [x] S17.7 — Lock file format: `fj.lock` with package name, version, checksum
- [x] S17.8 — Diamond dependency handling: single version per major release
- [x] S17.9 — Dev-dependency separation: `[dev-dependencies]` only for tests
- [x] S17.10 — 10 tests: resolution success, conflict detection, diamond deps, lock file

#### Sprint 18: Security & Polish `P2`

**Goal:** Package signing, audit, name validation

- [x] S18.1 — Name validation: ASCII alphanumeric + hyphens, case-insensitive collision check
- [x] S18.2 — Name squatting prevention: `-` and `_` treated as equivalent
- [x] S18.3 — Immutable versions: reject republish of same version with different content
- [x] S18.4 — `fj audit`: check dependencies against advisory database
- [x] S18.5 — Rate limiting: tower-governor middleware on publish endpoint
- [x] S18.6 — Token scoping: per-package publish permissions
- [x] S18.7 — Download counter: increment on each download, display in search
- [x] S18.8 — Registry config.json: dl URL template, api URL, auth-required flag
- [x] S18.9 — Registry README: setup instructions, API documentation
- [x] S18.10 — 8 tests: name validation, collision, immutability, rate limit

### Phase 5: Lifetime Annotations `P2`

#### Sprint 19: Syntax & Parser `P2`

**Goal:** Lifetime syntax in parser and AST

- [x] S19.1 — Lexer: `'a`, `'static`, `'_` lifetime tokens (Apostrophe + Ident)
- [x] S19.2 — Parser: lifetime parameters on function definitions `fn foo<'a>(...)`
- [x] S19.3 — Parser: lifetime parameters on struct definitions `struct Foo<'a> { ... }`
- [x] S19.4 — Parser: lifetime annotations on references `&'a T`, `&'a mut T`
- [x] S19.5 — AST: `LifetimeParam` struct, add to FnDef, StructDef, ImplBlock
- [x] S19.6 — AST: `TypeExpr::Ref` extended with optional lifetime
- [x] S19.7 — Formatter: pretty-print lifetime annotations
- [x] S19.8 — Lifetime elision: apply 3 rules to infer omitted lifetimes
- [x] S19.9 — `'static` lifetime: built-in, outlives everything
- [x] S19.10 — 10 tests: lexer tokens, parser AST, elision rules, formatter

#### Sprint 20: CFG-Based Region Inference `P2`

**Goal:** Upgrade borrow checker from span-based to CFG-based

- [x] S20.1 — Build proper CFG from AST: basic blocks with predecessors/successors
- [x] S20.2 — CFG nodes: Entry, Statement, Branch, Merge, Exit
- [x] S20.3 — Backward dataflow analysis: compute liveness sets per CFG node
- [x] S20.4 — Region variables: assign fresh region to each borrow expression
- [x] S20.5 — Liveness constraints: region must include points where reference is live
- [x] S20.6 — Outlives constraints: assignment `ref_a = ref_b` generates `'b: 'a`
- [x] S20.7 — SCC computation: merge regions in cycles (strongly connected components)
- [x] S20.8 — Fixed-point iteration: propagate constraints until stable
- [x] S20.9 — Integration with existing `borrow_lite.rs`: replace span-based with CFG-based
- [x] S20.10 — 10 tests: CFG construction, liveness, region inference, constraint solving

#### Sprint 21: Constraint Solving & Errors `P2`

**Goal:** Full lifetime checking with error reporting

- [x] S21.1 — Struct lifetime checking: struct with `&'a T` field requires `'a` parameter
- [x] S21.2 — Function lifetime checking: return reference requires matching input lifetime
- [x] S21.3 — Lifetime mismatch error: "borrowed value does not live long enough"
- [x] S21.4 — Dangling reference detection: borrow outlives borrowed data
- [x] S21.5 — Multiple borrow checking: enforce exclusive `&mut` or shared `&` rules via CFG
- [x] S21.6 — Error suggestions: "consider adding lifetime parameter" or "try cloning"
- [x] S21.7 — Error code: ME009 LifetimeMismatch, ME010 DanglingReference
- [x] S21.8 — Lifetime bounds: `T: 'a` constraint (all references in T outlive 'a)
- [x] S21.9 — Analyzer integration: run lifetime check after existing borrow analysis
- [x] S21.10 — 10 tests: lifetime errors, suggestions, struct lifetimes, function lifetimes

### Phase 6: RTOS Integration `P1`

#### Sprint 22: FreeRTOS FFI Bindings `P1`

**Goal:** C FFI wrappers for FreeRTOS API

- [ ] S22.1 — `src/rtos/mod.rs`: RTOS module declaration
- [ ] S22.2 — `src/rtos/freertos.rs`: FFI function declarations (extern "C")
- [ ] S22.3 — Task API: `fj_rt_task_create`, `fj_rt_task_delete`, `fj_rt_task_delay`
- [ ] S22.4 — Queue API: `fj_rt_queue_create`, `fj_rt_queue_send`, `fj_rt_queue_receive`
- [ ] S22.5 — Mutex API: `fj_rt_mutex_create`, `fj_rt_mutex_lock`, `fj_rt_mutex_unlock`
- [ ] S22.6 — Semaphore API: `fj_rt_sem_create`, `fj_rt_sem_give`, `fj_rt_sem_take`
- [ ] S22.7 — Timer API: `fj_rt_timer_create`, `fj_rt_timer_start`, `fj_rt_timer_stop`
- [ ] S22.8 — ISR-safe variants: `_from_isr` versions of queue/semaphore ops
- [ ] S22.9 — FreeRTOSConfig.h template generation for each BSP board
- [ ] S22.10 — 10 tests: FFI bindings compile, API signatures correct

#### Sprint 23: Language-Level RTOS Abstractions `P1`

**Goal:** Fajar Lang syntax for tasks, queues, mutexes

- [ ] S23.1 — `task_spawn(priority, stack_size, fn)` builtin function
- [ ] S23.2 — `task_delay_ms(ms)` and `task_delay_until(ticks)` builtins
- [ ] S23.3 — `Queue<T, N>` generic type with send/receive methods
- [ ] S23.4 — `Queue::send_from_isr()` ISR-safe variant (enforced in @kernel context)
- [ ] S23.5 — `Mutex<T>` with lock/unlock and priority inheritance
- [ ] S23.6 — `Semaphore` binary and counting variants
- [ ] S23.7 — `EventGroup` with set_bits/wait_bits/sync
- [ ] S23.8 — `scheduler_start()` to begin RTOS scheduling
- [ ] S23.9 — Static task allocation in `@kernel`/`@safe` contexts
- [ ] S23.10 — 10 tests: task spawn, queue send/recv, mutex, event group

#### Sprint 24: Real-Time Annotations `P2`

**Goal:** @realtime, @periodic, @wcet, timing constraints

- [ ] S24.1 — `@periodic(period: 10ms)` annotation: generate vTaskDelayUntil pattern
- [ ] S24.2 — `@realtime(deadline: 5ms)` annotation: analyzer restricts heap alloc + unbounded loops
- [ ] S24.3 — `@wcet(max: 500us)` annotation: compiler estimates instruction count
- [ ] S24.4 — `@idle_hook` annotation: emit `vApplicationIdleHook` implementation
- [ ] S24.5 — `@tick_hook` annotation: emit `vApplicationTickHook` implementation
- [ ] S24.6 — Stack size estimation: call graph analysis for task stack requirements
- [ ] S24.7 — `uxTaskGetStackHighWaterMark()` wrapper for debug builds
- [ ] S24.8 — Priority inversion detection: warn if high-priority task accesses low-priority mutex
- [ ] S24.9 — Tickless idle support: `configUSE_TICKLESS_IDLE` integration
- [ ] S24.10 — 10 tests: periodic task, realtime constraints, idle hook, stack estimation

### Phase 7: Advanced ML `P2`

#### Sprint 25: LSTM & GRU Layers `P2`

**Goal:** Recurrent neural network layers with BPTT

- [ ] S25.1 — `LSTMCell` struct: w_ih, w_hh, b_ih, b_hh (concatenated 4H weights)
- [ ] S25.2 — LSTM forward: gates computation (forget, input, output, candidate)
- [ ] S25.3 — LSTM cell state update: `c_t = f_t * c_{t-1} + i_t * c~_t`
- [ ] S25.4 — LSTM hidden state: `h_t = o_t * tanh(c_t)`
- [ ] S25.5 — LSTM sequence forward: iterate timesteps, collect hidden states
- [ ] S25.6 — LSTM backward (BPTT): gradient flow through gates and time
- [ ] S25.7 — `GRUCell` struct: w_ih, w_hh, b_ih, b_hh (concatenated 3H weights)
- [ ] S25.8 — GRU forward: reset gate, update gate, candidate, interpolation
- [ ] S25.9 — GRU backward (BPTT): gradient through gates and time
- [ ] S25.10 — 10 tests: LSTM forward/backward, GRU forward/backward, sequence processing

#### Sprint 26: Learning Rate Scheduling & AdamW `P2`

**Goal:** Advanced optimizers and LR schedulers

- [ ] S26.1 — `AdamW` optimizer: decoupled weight decay (separate from gradient)
- [ ] S26.2 — Gradient clipping by norm: global norm computation + scaling
- [ ] S26.3 — LR warmup: linear ramp from 0 to base_lr over warmup_steps
- [ ] S26.4 — ReduceOnPlateau scheduler: reduce LR when metric stops improving
- [ ] S26.5 — OneCycleLR: three-phase schedule (warmup → anneal → cooldown)
- [ ] S26.6 — Cosine annealing with warm restarts: `T_mult` cycle growth
- [ ] S26.7 — LRScheduler trait: `get_lr(step)`, `step_with_metric(metric)`
- [ ] S26.8 — Optimizer::set_lr(): dynamically update learning rate
- [ ] S26.9 — Weight decay parameter on all optimizers (SGD, Adam, AdamW)
- [ ] S26.10 — 10 tests: AdamW step, warmup, plateau, one-cycle, cosine restart

#### Sprint 27: DataLoader & Training Utilities `P2`

**Goal:** Multi-threaded data loading, checkpoints, early stopping

- [ ] S27.1 — `Dataset` trait: `len()`, `get(index)` → (features, labels)
- [ ] S27.2 — Collate function: stack samples into batch tensors
- [ ] S27.3 — `ThreadedDataLoader`: worker threads prefetch batches via channels
- [ ] S27.4 — Fisher-Yates shuffle per epoch with seed control
- [ ] S27.5 — `EarlyStopping`: patience, min_delta, best_metric tracking
- [ ] S27.6 — Checkpoint save: model params + optimizer state + epoch + lr
- [ ] S27.7 — Checkpoint load: restore training from saved state
- [ ] S27.8 — Dropout train/eval mode: `model.train()` / `model.eval()` toggle
- [ ] S27.9 — BatchNorm running statistics: exponential moving average
- [ ] S27.10 — 10 tests: dataloader iteration, shuffle, early stopping, checkpoint save/load

#### Sprint 28: Mixed Precision & Polish `P2`

**Goal:** f16/bf16 support, training pipeline, examples

- [ ] S28.1 — Add `half` crate dependency
- [ ] S28.2 — `DType` enum on TensorValue: F64, F32, F16, BF16
- [ ] S28.3 — `to_dtype()` conversion: f64 ↔ f32 ↔ f16 ↔ bf16
- [ ] S28.4 — `LossScaler`: dynamic loss scaling for FP16 gradient underflow prevention
- [ ] S28.5 — Mixed precision forward: model weights in FP16, accumulation in FP32
- [ ] S28.6 — Loss computation always in FP64 (prevent precision loss)
- [ ] S28.7 — `examples/lstm_sequence.fj`: LSTM sequence classification demo
- [ ] S28.8 — `examples/rtos_ml_pipeline.fj`: sensor → inference → actuator with RTOS
- [ ] S28.9 — Update mdBook: LLVM backend, debugger, BSP, registry, RTOS, ML chapters
- [ ] S28.10 — 10 tests: dtype conversion, loss scaling, mixed precision forward

---

## Dependencies

```
Phase 1 (LLVM) ─────────────────────────→ Phase 3 (BSP needs LLVM for Thumb targets)
Phase 2 (Debugger) ─────────────────────→ Phase 3 (BSP debugging)
Phase 1 (LLVM) ─────────────────────────→ Phase 6 (RTOS needs native codegen)
Phase 3 (BSP) ──────────────────────────→ Phase 6 (RTOS needs board support)
Phase 4 (Registry) ────────────────────── Independent (can run in parallel)
Phase 5 (Lifetimes) ───────────────────── Independent (analyzer-only)
Phase 7 (ML) ──────────────────────────── Independent (runtime-only)
```

**Critical path:** Phase 1 (LLVM) → Phase 3 (BSP) → Phase 6 (RTOS)

**Parallel tracks:**
- Track A: Phase 1 → Phase 3 → Phase 6 (codegen → hardware → RTOS)
- Track B: Phase 2 (debugger, independent)
- Track C: Phase 4 (registry, independent)
- Track D: Phase 5 (lifetimes, independent)
- Track E: Phase 7 (ML, independent)

---

## Success Criteria

- [ ] `fj build --backend llvm --opt-level 3` produces optimized native binaries
- [ ] LLVM-compiled code runs 10-20% faster than Cranelift on compute benchmarks
- [ ] `fj debug --dap` enables step debugging in VS Code with breakpoints and variables
- [ ] `fj build --board stm32f407` produces flashable firmware for real hardware
- [ ] `fj flash --board rp2040 --uf2` deploys firmware via drag-and-drop
- [ ] `fj publish` uploads packages to registry, `fj install` downloads them
- [ ] PubGrub resolves diamond dependencies with clear error messages
- [ ] `fn longest<'a>(x: &'a str, y: &'a str) -> &'a str` compiles and type-checks
- [ ] `task_spawn(priority: 5, stack_size: 1024, my_task)` creates FreeRTOS task
- [ ] LSTM layer processes sequences with BPTT backward pass
- [ ] Mixed precision training with FP16 forward + FP32 accumulation works
- [ ] All existing tests still pass (1,767+ baseline, zero regression)

---

## Stats Targets

| Metric | v0.5 (current) | v0.6 (target) |
|--------|----------------|---------------|
| Tests | 1,767 | 4,000+ |
| LOC | ~101,000 | ~160,000 |
| Examples | 28 | 34+ |
| Error codes | 73 | 85+ |
| Token kinds | 90+ | 95+ |
| BSP boards | 0 (QEMU only) | 3 (STM32, ESP32, RP2040) |
| Codegen backends | 1 (Cranelift) | 2 (Cranelift + LLVM) |

---

## Non-Goals (Deferred to v0.7+)

- LLVM PGO (Profile-Guided Optimization) — needs instrumented build pipeline
- Zephyr RTOS support — FreeRTOS sufficient for initial release
- Package signing with Sigstore — requires PKI infrastructure
- ESP32 WiFi/BLE from Fajar Lang — requires esp-idf integration
- Full Polonius borrow checker — simplified model sufficient
- RTIC-style compile-time scheduling — FreeRTOS runtime scheduling first
- TFLite model import — ONNX sufficient for embedded inference

---

*V06_PLAN.md v1.0 | Created 2026-03-11*
