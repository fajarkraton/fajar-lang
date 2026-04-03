# V19-V21 Complete Plan: 56/56 Modules Production

> **Date:** 2026-04-03
> **Goal:** EVERY module production. ZERO framework. ZERO stub. 56/56 = 100%.
> **Hardware:** STM32F407/H5, ESP32-S3, Radxa Dragon Q6A (existing), RTX 4090 (existing)
> **Rule:** [x] = user can `fj <command>` and it works on real hardware. NO simulation.

---

## Current State: 38/56 Production (68%)

```
[x] Production:  38 modules (368K LOC)
[f] Framework:   14 modules (need wiring)
[s] Stub:         3 modules (need real code)
[p] Partial:      1 module  (analyzer — context enforcement done, some edge cases)
                  ──
                  56 total
```

## Target: 56/56 Production (100%)

```
Need to fix: 18 modules (14 framework + 3 stub + 1 partial)
```

---

## Hardware Shopping List

| Device | For | Estimated Cost | Where |
|--------|-----|---------------|-------|
| STM32F407 Discovery | RTOS (FreeRTOS), BSP verification | ~$25 USD | Tokopedia/Shopee |
| STM32H5 Nucleo | RTOS (Zephyr), advanced HAL | ~$40 USD | Mouser/Digikey |
| ESP32-S3-DevKitC | IoT (WiFi, BLE, MQTT) | ~$15 USD | Tokopedia/Shopee |
| LoRa SX1276 module | IoT (LoRaWAN) | ~$10 USD | Tokopedia |
| USB-UART adapter | Flashing/debugging | ~$5 USD | Already have? |
| **Total** | | **~$95 USD** | |

Already owned:
- Radxa Dragon Q6A (QCS6490) — NPU/GPU verification
- NVIDIA RTX 4090 Laptop — CUDA/GPU codegen
- x86_64 Linux dev machine

---

## V19 "Precision" — 42 tasks (expanded)

Original V19 (34 tasks) + HIGH priority gaps (8 tasks) = 42 tasks.

### Phase 1: User Macros (10 tasks) — from V19 plan
*No changes — same as docs/V19_PLAN.md Phase 1*

### Phase 2: Pattern Match Destructuring (8 tasks) — from V19 plan
*No changes — same as docs/V19_PLAN.md Phase 2*

### Phase 3: Real Async I/O (6 tasks) — from V19 plan
*No changes — same as docs/V19_PLAN.md Phase 3*

### Phase 4: E2E Tests (6 tasks) — from V19 plan
*No changes — same as docs/V19_PLAN.md Phase 4*

### Phase 5: HIGH Priority Gaps (8 tasks) — NEW

| # | Task | Module | Verification |
|---|------|--------|-------------|
| 5.1 | Convert database_demo.rs → .fj | demos | `fj run examples/database_demo.fj` |
| 5.2 | Convert web_fullstack_demo.rs → .fj | demos | `fj run examples/web_demo.fj` |
| 5.3 | Convert embedded_ml_demo.rs → .fj | demos | `fj run examples/embedded_ml_demo.fj` |
| 5.4 | Convert cli_tool_demo.rs → .fj | demos | `fj run examples/cli_tool_demo.fj` |
| 5.5 | Wire `fj demo --list` to show all demos | demos | `fj demo --list` prints 10+ names |
| 5.6 | Wire lsp_v3 QuickFixKind into code_action | lsp_v3 | VS Code shows structured fixes |
| 5.7 | Wire lsp_v3 suggest_cast for SE004 | lsp_v3 | VS Code suggests `to_int(x)` |
| 5.8 | Wire const_type_name + const_field_names | const | `println(const_type_name(42))` → "i64" |

### Phase 6: Polish (4 tasks) — from V19 plan
*No changes — same as docs/V19_PLAN.md Phase 5*

**V19 Total: 42 tasks**
**V19 Target: ~44/56 modules production (79%)**

---

## V20 "Completeness" — 25 tasks

### Phase 1: Debugger v2 Record/Replay (5 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.1 | Wire `--record` flag — capture interpreter events to JSON | `fj debug --record trace.json hello.fj` produces trace file |
| 1.2 | Implement trace format: function calls, variable changes, output | `cat trace.json` shows structured events |
| 1.3 | Wire `--replay` flag — replay from trace file | `fj debug --replay trace.json` reproduces output |
| 1.4 | Add reverse stepping: `--replay --step-back` | Step backward through recorded trace |
| 1.5 | Integration test: record → replay round-trip = same output | `cargo test debug_record_replay` |

**Module elevated:** `debugger_v2` [f] → [x]

### Phase 2: Package v2 Build Scripts (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.1 | Parse `[build-script]` from fj.toml | `fj build` reads build-script section |
| 2.2 | Execute build script (shell command) before compilation | Build script runs, output captured |
| 2.3 | Apply output directives (env vars, link flags) | `cargo:rustc-link-lib` equivalent |
| 2.4 | Example: fj.toml with build script that generates config | `fj build` runs script, config available |

**Module elevated:** `package_v2` [f] → [x]

### Phase 3: ML Advanced — Diffusion + RL (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.1 | Wire `diffusion_create(steps)` builtin | Creates diffusion model |
| 3.2 | Wire `diffusion_denoise(model, noisy_tensor, step)` builtin | Denoises one step |
| 3.3 | Wire `rl_agent_create(state_dim, action_dim)` builtin | Creates RL agent |
| 3.4 | Example: `examples/diffusion_demo.fj` + `examples/rl_demo.fj` | Both `fj run` produce output |

**Module elevated:** `ml_advanced` [p] → [x]

### Phase 4: RT Pipeline Executor (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 4.1 | Wire `pipeline_create()` builtin | Creates pipeline handle |
| 4.2 | Wire `pipeline_add_stage(pipe, name, fn)` — adds processing stage | Stage registered |
| 4.3 | Wire `pipeline_run(pipe, input)` — executes all stages in sequence | Output from last stage |
| 4.4 | Example: `examples/rt_pipeline_demo.fj` — sensor → ML → actuator | `fj run` produces control output |

**Module elevated:** `rt_pipeline` [f] → [x]

### Phase 5: Accelerator Dispatch (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 5.1 | Wire `accelerate(fn_name, input)` — classify workload + dispatch | Returns device used |
| 5.2 | Dispatch to GPU (CUDA) when available, CPU fallback | `fj run` on RTX 4090 → "GPU" |
| 5.3 | Example: `examples/accelerator_demo.fj` | Shows dispatch decision |

**Module elevated:** `accelerator` [f] → [x]

### Phase 6: Concurrency v2 — Actor Supervision (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 6.1 | Wire `actor_spawn(name, fn)` — spawn supervised actor | Actor created with name |
| 6.2 | Wire `actor_supervise(actor, strategy)` — restart/stop on failure | Actor restarts on panic |
| 6.3 | Example: `examples/actor_demo.fj` | `fj run` shows actors communicating |

**Module elevated:** `concurrency_v2` [f] → [x]

### Phase 7: Const Modules (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 7.1 | Wire const_alloc + const_reflect + const_stdlib into interpreter builtins | `const_alloc(1024)` works in const fn |
| 7.2 | Wire const_bench — compile-time benchmark comparison | `fj bench --const` reports comptime stats |

**Modules elevated:** `const_alloc`, `const_reflect`, `const_stdlib`, `const_bench` [f]/[s] → [x]

**V20 Total: 25 tasks**
**V20 Target: ~52/56 modules production (93%)**

---

## V21 "Hardware" — 20 tasks

### Phase 1: RTOS Real Hardware (8 tasks)

**Requires:** STM32F407 Discovery + STM32H5 Nucleo

| # | Task | Verification |
|---|------|-------------|
| 1.1 | Wire FreeRTOS feature gate: `--features freertos` links real FreeRTOS | Compiles with FreeRTOS C lib |
| 1.2 | Implement `rtos_task_create(name, fn, priority)` with real xTaskCreate | Task runs on STM32F407 |
| 1.3 | Implement `rtos_mutex_create/lock/unlock` with real xSemaphore | Mutex works on hardware |
| 1.4 | Implement `rtos_queue_send/recv` with real xQueueSend | Queue works on hardware |
| 1.5 | Wire Zephyr feature gate: `--features zephyr` links Zephyr | Compiles with Zephyr SDK |
| 1.6 | Implement Zephyr k_thread, k_mutex, k_msgq equivalents | Runs on STM32H5 |
| 1.7 | Cross-compile: `fj build --board stm32f407 --features freertos` | Produces flashable binary |
| 1.8 | Flash test: binary boots on real STM32F407, LED blinks via RTOS task | LED blinks on hardware |

**Verification:** Connect STM32F407 via USB, flash binary, observe LED blink + UART output.

**Module elevated:** `rtos` [s] → [x]

### Phase 2: IoT Real Hardware (8 tasks)

**Requires:** ESP32-S3-DevKitC + LoRa SX1276

| # | Task | Verification |
|---|------|-------------|
| 2.1 | Wire ESP-IDF feature gate: `--features esp32` links esp-idf-sys | Compiles with ESP-IDF |
| 2.2 | Implement `wifi_connect(ssid, password)` with real esp_wifi | ESP32 connects to WiFi |
| 2.3 | Implement `ble_scan()` with real esp_ble | ESP32 scans BLE devices |
| 2.4 | Implement `mqtt_connect(broker)` with real esp_mqtt | ESP32 connects to MQTT broker |
| 2.5 | Implement `lora_send(data)` with SX1276 SPI driver | LoRa packet transmitted |
| 2.6 | Cross-compile: `fj build --board esp32s3 --features esp32` | Produces flashable binary |
| 2.7 | OTA update: `fj ota push --board esp32s3` | Firmware updated over WiFi |
| 2.8 | Flash test: ESP32 boots, connects WiFi, publishes MQTT | Real MQTT message on broker |

**Verification:** Flash ESP32, connect to WiFi router, see MQTT message on broker dashboard.

**Module elevated:** `iot` [s] ��� [x]

### Phase 3: JIT Tiered Compilation (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.1 | Implement baseline JIT: hot functions detected → Cranelift compile | `fj run --jit` shows "compiled: fib" after N calls |
| 3.2 | Implement tier promotion: interpreter → baseline → optimized | `fj run --jit --verbose` shows tier transitions |
| 3.3 | Benchmark: JIT vs interpreter speedup on fibonacci(30) | JIT measurably faster |

**Module elevated:** `jit` [f] → [x]

### Phase 4: Plugin Dynamic Loading (1 task)

| # | Task | Verification |
|---|------|-------------|
| 4.1 | Wire `fj plugin load lint.so` — loads .so, calls CompilerPlugin trait methods | `fj plugin load` runs plugin, reports lint results |

**Module elevated:** `plugin` [f] → [x]

**V21 Total: 20 tasks**
**V21 Target: 56/56 modules production (100%)**

---

## Grand Timeline

| Version | Focus | Tasks | Modules [x] | % |
|---------|-------|-------|-------------|---|
| V18 (done) | Integrity | 36 | 38/56 | 68% |
| **V19** | Precision | 42 | 44/56 | 79% |
| **V20** | Completeness | 25 | 52/56 | 93% |
| **V21** | Hardware | 20 | **56/56** | **100%** |
| **Total** | | **87 tasks** | | |

---

## Hardware Verification Matrix

| Hardware | Module | Test | Pass Criteria |
|----------|--------|------|---------------|
| STM32F407 | rtos (FreeRTOS) | LED blink via RTOS task | LED toggles at 1Hz |
| STM32H5 | rtos (Zephyr) | Thread + mutex | UART prints "mutex acquired" |
| ESP32-S3 | iot (WiFi) | Connect to AP | IP address printed |
| ESP32-S3 | iot (BLE) | Scan devices | Device names listed |
| ESP32-S3 | iot (MQTT) | Publish message | Message on broker |
| LoRa SX1276 | iot (LoRaWAN) | Send packet | Gateway receives |
| RTX 4090 | accelerator | GPU dispatch | CUDA kernel executes |
| Dragon Q6A | accelerator | NPU dispatch | QNN inference runs |

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Hardware not available in time | V21 blocks | Order now, V19-V20 don't need hardware |
| ESP-IDF build complexity | V21 Phase 2 blocks | Use esp-idf-sys crate (proven), Docker build env |
| FreeRTOS C linkage issues | V21 Phase 1 blocks | Use cc crate + pre-built FreeRTOS .a |
| JIT tiered baseline too complex | V21 Phase 3 blocks | Use Cranelift baseline (already integrated) |
| Plugin ABI versioning | V21 Phase 4 blocks | Start with C ABI only (stable) |

---

## Final State (After V21)

```
Modules:  56/56 production (100%)
Tests:    ~9,000+ (projected)
LOC:      ~480K
Examples: 280+ .fj programs
CLI:      30+ commands (all production)
Hardware: STM32, ESP32, Q6A, x86_64, RTX 4090 — all verified
RTOS:     FreeRTOS + Zephyr (real hardware)
IoT:      WiFi + BLE + MQTT + LoRaWAN (real hardware)
ML:       Dense, Conv2d, Attention, Diffusion, RL (real ndarray)
GPU:      SPIR-V, PTX, CUDA dispatch (real RTX 4090)
NPU:      QNN (real Dragon Q6A)
```

**"If it compiles in Fajar Lang, it's safe to deploy on hardware."**
— Not just a slogan, but verified on 5 hardware platforms.

---

*V19-V21 Complete 56/56 Plan — 87 tasks total*
*Every task has hardware or E2E verification.*
*Written with V13-V15 lesson: NEVER inflate, ALWAYS verify, BUY the hardware.*
