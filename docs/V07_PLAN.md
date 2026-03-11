# Fajar Lang v0.7 "Zenith" — Implementation Plan

> **Focus:** WebAssembly, IoT connectivity, formal verification, advanced ML architectures
> **Timeline:** 28 sprints, ~280 tasks, 4-6 months
> **Prerequisite:** v0.6 "Horizon" RELEASED
> **Theme:** *"Reach the peak — deploy everywhere, verify everything, train anything"*

---

## Motivation

v0.6 established production infrastructure (LLVM, debugger, BSP, registry, RTOS). But critical gaps remain for full-stack deployment:

- **No WebAssembly backend** — cannot target browser/edge; Wasm is the universal deployment format for ML inference
- **No ESP32 WiFi/BLE** — IoT devices are deaf without wireless connectivity; esp-idf integration unlocked by BSP framework
- **No Polonius borrow checker** — current NLL is scope-approximated; Polonius enables precise flow-sensitive borrowing
- **No Transformer architecture** — LSTM/GRU exist but modern ML demands full attention, positional encoding, transformer blocks
- **No TFLite import** — embedded ML ecosystem runs on TFLite; ONNX alone is insufficient for edge deployment
- **No LLVM PGO** — leaving 5-15% performance on the table for hot loops and inference kernels
- **No RTIC scheduling** — FreeRTOS/Zephyr runtime scheduling adds overhead; compile-time scheduling eliminates it
- **No package signing** — supply chain attacks are existential for safety-critical embedded; Sigstore solves this
- **No OTA updates** — deployed ML models cannot be updated without physical access
- **No power management** — battery-powered IoT devices need sleep modes, wake sources, clock gating
- **No Wasm** — the most portable compilation target for browser + serverless + edge
- **No distributed training** — single-device training limits model scale
- **No formal verification** — safety-critical code (automotive, aerospace) demands mathematical proof

v0.7 targets these gaps to make Fajar Lang the complete language for embedded ML at scale.

---

## Architecture Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | `wasm-encoder` 0.225 for Wasm codegen | Direct module construction, no LLVM dependency for Wasm |
| 2 | Feature-gate Wasm as `wasm` | Parallel to `native` (Cranelift) and `llvm` backends |
| 3 | WASI preview2 (component model) | Future-proof; WASI-p2 is the standard for server-side Wasm |
| 4 | `esp-idf-sys` 0.36 for ESP32 WiFi/BLE | Official Espressif bindings, supports WiFi + BLE + MQTT |
| 5 | Datalog via `crepe` 0.1 for Polonius | Lightweight Datalog engine in Rust, same approach as rustc |
| 6 | `flatbuffers` 24.12 for TFLite import | TFLite uses FlatBuffers; zero-copy parsing |
| 7 | LLVM instrumentation via inkwell PassManager | PGO requires compiler-rt profiling instrumented into IR |
| 8 | Sigstore via `sigstore-rs` 0.10 | Keyless signing, Rekor transparency log, OIDC identity |
| 9 | `reqwest` 0.12 for OTA HTTP client | Async HTTP with TLS for firmware/model download |
| 10 | Transformer as composable layer stack | Multi-head attention + FFN + LayerNorm + residual, not monolithic |
| 11 | gRPC via `tonic` 0.12 for distributed training | Efficient gradient serialization, bidirectional streaming |
| 12 | Property-based verification via abstract interpretation | Lightweight alternative to full SMT; covers bounds, overflow, null |
| 13 | Power management via CMSIS-compatible API | Portable across Cortex-M MCUs, maps to WFI/WFE instructions |
| 14 | `probe-rs` 0.25 for hardware CI | Flash + test + collect output from real boards in GitHub Actions |

---

## Dependencies (New Crates)

```toml
# Phase 1: WebAssembly Backend
wasm-encoder = { version = "0.225", optional = true }  # Wasm module construction
wasmtime = { version = "29", optional = true }          # Wasm JIT for testing
wasi-common = { version = "29", optional = true }       # WASI runtime

# Phase 2: ESP32 WiFi/BLE & IoT
esp-idf-sys = { version = "0.36", optional = true }     # ESP-IDF bindings
esp-idf-svc = { version = "0.51", optional = true }     # High-level ESP-IDF services
rumqttc = { version = "0.24", optional = true }          # MQTT client

# Phase 3: Advanced Borrow Checker
crepe = "0.1"                                            # Datalog solver for Polonius

# Phase 4: Transformer & Distributed ML
flatbuffers = { version = "24.12", optional = true }     # TFLite FlatBuffer parsing
tonic = { version = "0.12", optional = true }            # gRPC for distributed training
prost = { version = "0.13", optional = true }            # Protobuf serialization

# Phase 6: Package Signing & Security
sigstore = { version = "0.10", optional = true }         # Keyless signing + Rekor
reqwest = { version = "0.12", features = ["rustls-tls"], optional = true }  # HTTP client (OTA + registry)
```

---

## Sprint Plan

### Phase 1: WebAssembly Backend `P0` `CRITICAL`

#### Sprint 1: Wasm Target Setup `P0`

**Goal:** wasm-encoder integration, module skeleton, type mapping

- [x] S1.1 — Add `wasm-encoder` dependency gated under `[features] wasm`, `src/codegen/wasm/mod.rs` module declaration
- [x] S1.2 — `WasmCompiler` struct: module builder, type section, function section, export section, current function index
- [x] S1.3 — Type mapping: Fajar `i32`→`ValType::I32`, `i64`→`ValType::I64`, `f32`→`ValType::F32`, `f64`→`ValType::F64`, `bool`→`ValType::I32`
- [x] S1.4 — String representation: pointer+length pair as `(i32, i32)` in linear memory
- [x] S1.5 — Function signature encoding: `TypeSection` entries for each `fn` in program
- [x] S1.6 — Module structure: type section, import section (for host functions), function section, memory section, export section
- [x] S1.7 — Memory section: 1 page (64KB) initial, 256 pages max, linear memory for stack + heap
- [x] S1.8 — Export main function as `_start` entry point (WASI convention)
- [x] S1.9 — `module.finish()` → `Vec<u8>` binary output, validate with `wasmparser`
- [x] S1.10 — 10 tests: module creation, type mapping, function signatures, memory section, binary validation

#### Sprint 2: Wasm Expression & Statement Compilation `P0`

**Goal:** Compile all expression and statement types to Wasm bytecode

- [x] S2.1 — Integer literals: `Instruction::I64Const(n)` push to operand stack
- [x] S2.2 — Float literals: `Instruction::F64Const(n)` push to operand stack
- [x] S2.3 — Arithmetic ops: `i64.add/sub/mul/div_s`, `f64.add/sub/mul/div` instruction emission
- [x] S2.4 — Comparison ops: `i64.eq/ne/lt_s/gt_s/le_s/ge_s`, `f64.eq/ne/lt/gt/le/ge`
- [x] S2.5 — Logical ops: `i32.and/or`, `i32.eqz` (not), short-circuit via `if/else` blocks
- [x] S2.6 — Let bindings: `local.set`/`local.get` with local index tracking
- [x] S2.7 — Assignment: `local.set` for mutable variables
- [x] S2.8 — If/else: `Instruction::If` with `BlockType`, then/else branches, `End`
- [x] S2.9 — While loop: `block`+`loop` pair, `br_if` for condition, `br` for back-edge
- [x] S2.10 — 10 tests: arithmetic, comparison, let/mut, if/else, while loop — validate output via wasmtime execution

#### Sprint 3: Wasm Memory Model `P0`

**Goal:** Linear memory management — stack, heap, string storage

- [x] S3.1 — Stack allocator: bump pointer in linear memory for local arrays and structs, grows downward from high address
- [x] S3.2 — Heap allocator: simple free-list allocator in linear memory, `__wasm_malloc(size) -> ptr` and `__wasm_free(ptr)`
- [x] S3.3 — String storage: UTF-8 bytes in linear memory, `(ptr, len)` tuple representation
- [x] S3.4 — String literals: store in data section via `DataSection`, return `(offset, len)` at compile time
- [x] S3.5 — Array representation: `(ptr, len, capacity)` triple in linear memory, element access via `i32.load` with offset
- [x] S3.6 — Struct layout: field offsets computed at compile time, access via `i32.load` at `base + offset`
- [x] S3.7 — Function calls: `call` instruction with function index, arguments passed via operand stack
- [x] S3.8 — Recursive functions: natural stack via Wasm call stack, no special handling needed
- [x] S3.9 — Global variables: `GlobalSection` entries with `global.get`/`global.set` instructions
- [x] S3.10 — 10 tests: malloc/free roundtrip, string storage/retrieval, array element access, struct field access, global vars

#### Sprint 4: Wasm Integration & CLI `P0`

**Goal:** WASI support, browser runtime, CLI integration, end-to-end compilation

- [x] S4.1 — WASI imports: `fd_write` (stdout), `proc_exit` (exit code), `clock_time_get` (timing)
- [x] S4.2 — `println` builtin → WASI `fd_write` with iovec construction in linear memory
- [x] S4.3 — Host function imports: `ImportSection` entries for runtime functions (print, assert, math)
- [x] S4.4 — `fj build --target wasm` CLI flag: compile .fj → .wasm binary output
- [x] S4.5 — `fj build --target wasi` CLI flag: compile with WASI imports for server-side execution
- [x] S4.6 — `fj run --wasm` CLI flag: execute .wasm via embedded wasmtime runtime
- [x] S4.7 — Browser runtime stub: generate minimal HTML+JS loader that instantiates .wasm module
- [x] S4.8 — Wasm optimization: remove dead code, merge identical function types, minimize binary size
- [x] S4.9 — `examples/wasm_hello.fj`: hello world compiling to .wasm, runnable via wasmtime and browser
- [x] S4.10 — 10 tests: WASI fd_write, println output, CLI flags, end-to-end compile+run, binary size < 10KB for hello world

### Phase 2: ESP32 WiFi/BLE & IoT `P1`

#### Sprint 5: esp-idf WiFi FFI Bindings `P1`

**Goal:** WiFi station and AP mode via esp-idf C FFI

- [x] S5.1 — `src/iot/mod.rs`: IoT module declaration, `src/iot/wifi.rs` WiFi submodule
- [x] S5.2 — `WifiConfig` struct: ssid (max 32 bytes), password (max 64 bytes), auth_mode (Open/WPA2/WPA3), channel, max_connections
- [x] S5.3 — `wifi_init()` → initialize NVS flash, netif, event loop, WiFi driver (esp-idf sequence)
- [x] S5.4 — `wifi_connect_sta(ssid, password) -> Result<IpInfo>`: station mode, blocking until DHCP acquired or timeout
- [x] S5.5 — `wifi_start_ap(ssid, password, channel) -> Result<()>`: soft-AP mode with DHCP server
- [x] S5.6 — `wifi_scan() -> Vec<AccessPoint>`: scan nearby APs, return SSID + RSSI + channel + auth_mode
- [x] S5.7 — `wifi_disconnect()` and `wifi_stop()`: clean teardown of WiFi driver
- [x] S5.8 — `WifiEvent` enum: Connected, Disconnected, GotIp, LostIp — event callback registration
- [x] S5.9 — `IpInfo` struct: ip_addr, netmask, gateway as `[u8; 4]` tuples
- [x] S5.10 — 10 tests: config validation, ssid length check, auth mode mapping, event enum variants, IP parsing (simulation stubs)

#### Sprint 6: BLE GATT Server & Client `P1`

**Goal:** Bluetooth Low Energy support for sensor data exchange

- [x] S6.1 — `src/iot/ble.rs`: BLE module with `BleConfig` struct (device_name, appearance, adv_interval)
- [x] S6.2 — `ble_init(config) -> Result<()>`: initialize BLE controller + Bluedroid host stack via esp-idf
- [x] S6.3 — `GattService` struct: uuid (128-bit), characteristics vector, service handle
- [x] S6.4 — `GattCharacteristic` struct: uuid, properties (Read|Write|Notify), permissions, value buffer
- [x] S6.5 — `ble_register_service(service) -> Result<ServiceHandle>`: register GATT service with ESP-IDF BLE stack
- [x] S6.6 — `ble_start_advertising(adv_data) -> Result<()>`: BLE advertising with device name + service UUIDs
- [x] S6.7 — `ble_notify(handle, conn_id, data) -> Result<()>`: send notification to connected client
- [x] S6.8 — `BleEvent` enum: Connected(conn_id), Disconnected(conn_id), WriteRequest(handle, data), ReadRequest(handle)
- [x] S6.9 — `ble_scan(duration_ms) -> Vec<BleDevice>`: scan for nearby BLE peripherals, return name + addr + RSSI
- [x] S6.10 — 10 tests: UUID construction, characteristic properties bitmask, advertising data format, event dispatch, GATT service layout

#### Sprint 7: MQTT Client for IoT Telemetry `P1`

**Goal:** Publish sensor data and receive commands via MQTT broker

- [x] S7.1 — `src/iot/mqtt.rs`: MQTT module with `MqttConfig` struct (broker_url, client_id, keepalive_secs, clean_session)
- [x] S7.2 — `MqttClient::connect(config) -> Result<MqttClient>`: TCP connection to broker, CONNECT packet, CONNACK handling
- [x] S7.3 — `mqtt_publish(topic, payload, qos) -> Result<()>`: publish message with QoS 0 (at most once) or QoS 1 (at least once)
- [x] S7.4 — `mqtt_subscribe(topic, qos) -> Result<()>`: subscribe to topic with wildcard support (`+` single-level, `#` multi-level)
- [x] S7.5 — `MqttMessage` struct: topic, payload (bytes), qos, retain flag, message_id
- [x] S7.6 — `mqtt_on_message(callback)`: register callback for incoming messages on subscribed topics
- [x] S7.7 — `mqtt_disconnect()`: send DISCONNECT packet, close TCP socket, release resources
- [x] S7.8 — Auto-reconnect: exponential backoff (1s, 2s, 4s, ... 60s max) on connection loss
- [x] S7.9 — Last Will and Testament (LWT): configure message sent by broker if client disconnects unexpectedly
- [x] S7.10 — 10 tests: config validation, topic parsing, QoS level enforcement, wildcard matching, LWT configuration, reconnect backoff timing

#### Sprint 8: OTA Firmware & Model Update `P1`

**Goal:** Over-the-air update for firmware and ML models on deployed devices

- [x] S8.1 — `src/iot/ota.rs`: OTA module with `OtaConfig` struct (server_url, check_interval_secs, verify_signature, rollback_on_failure)
- [x] S8.2 — `ota_check_update(url) -> Result<Option<UpdateInfo>>`: HTTP HEAD request to check firmware version, compare with running version
- [x] S8.3 — `UpdateInfo` struct: version, size_bytes, sha256, signature, download_url, release_notes
- [x] S8.4 — `ota_download_firmware(url) -> Result<Vec<u8>>`: chunked HTTP GET download with progress callback, resume on failure
- [x] S8.5 — `ota_verify(firmware, expected_sha256) -> Result<()>`: SHA-256 integrity check before flashing
- [x] S8.6 — `ota_flash(firmware) -> Result<()>`: write to inactive OTA partition (ESP32 dual-partition A/B scheme), set boot flag
- [x] S8.7 — `ota_rollback()`: revert to previous partition if new firmware fails health check within first 60 seconds
- [x] S8.8 — ML model OTA: `ota_update_model(url, model_path) -> Result<()>`: download new model weights, hot-swap in running inference pipeline
- [x] S8.9 — Version manifest: JSON endpoint `{version, firmware_url, model_url, min_hw_version, changelog}` for fleet management
- [x] S8.10 — 10 tests: version comparison, SHA-256 verification, partition selection logic, rollback trigger, model hot-swap stub, manifest parsing

### Phase 3: Advanced Borrow Checker `P0`

#### Sprint 9: Polonius-Style Fact Generation `P0`

**Goal:** Generate origin, loan, and point facts from AST for Datalog analysis

- [x] S9.1 — `src/analyzer/polonius/mod.rs`: Polonius module with `PoloniusFacts` struct (origins, loans, points, cfg_edges)
- [x] S9.2 — `Origin` type: unique identifier per reference/borrow expression, tracks where a reference was created
- [x] S9.3 — `Loan` type: unique identifier per borrow (`&x`, `&mut x`), records borrowed place and mutability
- [x] S9.4 — `Point` type: unique program point (statement index within basic block), mid-point and start-point variants
- [x] S9.5 — Fact: `loan_issued_at(origin, loan, point)` — emitted at each `&x` or `&mut x` expression
- [x] S9.6 — Fact: `origin_contains_loan_on_entry(origin, loan, point)` — initial containment from loan site
- [x] S9.7 — Fact: `loan_invalidated_at(loan, point)` — emitted when borrowed place is written or moved
- [x] S9.8 — Fact: `origin_live_on_entry(origin, point)` — computed from liveness analysis of references
- [x] S9.9 — CFG edge facts: `cfg_edge(point_a, point_b)` from existing CFG infrastructure in `cfg.rs`
- [x] S9.10 — 10 tests: fact generation for simple borrows, mutable borrows, reborrow chains, function returns, struct fields

#### Sprint 10: Datalog Solver for Borrow Constraints `P0`

**Goal:** Solve Polonius constraints using Datalog to compute borrow errors

- [x] S10.1 — Add `crepe` dependency, define Datalog relations: `Origin`, `Loan`, `Point`, `CfgEdge`
- [x] S10.2 — Rule: `origin_contains_loan_on_entry(O, L, P2) :- origin_contains_loan_on_entry(O, L, P1), cfg_edge(P1, P2), !loan_killed_at(L, P1)`
- [x] S10.3 — Rule: `loan_live_at(L, P) :- origin_contains_loan_on_entry(O, L, P), origin_live_on_entry(O, P)`
- [x] S10.4 — Rule: `errors(L, P) :- loan_live_at(L, P), loan_invalidated_at(L, P)` — core error detection
- [x] S10.5 — Subset relation: `subset(O1, O2, P)` for origin assignment (ref_a = ref_b)
- [x] S10.6 — Subset propagation: `origin_contains_loan_on_entry(O2, L, P) :- subset(O1, O2, P), origin_contains_loan_on_entry(O1, L, P)`
- [x] S10.7 — Kill facts: `loan_killed_at(L, P)` when storage backing a loan goes out of scope or is reassigned
- [x] S10.8 — Placeholder origins for function boundaries: caller/callee origin mapping
- [x] S10.9 — Integration: run Datalog solver after fact generation, collect `errors` relation as `Vec<BorrowError>`
- [x] S10.10 — 10 tests: use-after-move detection, dangling reference, conflicting borrows, correct code acceptance, cross-block borrows

#### Sprint 11: Two-Phase Borrowing & Reborrowing `P1`

**Goal:** Support two-phase borrows and implicit reborrowing for ergonomic code

- [x] S11.1 — Two-phase borrow detection: `vec.push(vec.len())` pattern — reservation phase + activation phase
- [x] S11.2 — Reservation fact: `loan_reserved_at(loan, point)` — mutable borrow exists but not yet used mutably
- [x] S11.3 — Activation fact: `loan_activated_at(loan, point)` — first mutable use of reserved borrow
- [x] S11.4 — Rule: reserved loans allow shared access until activation point
- [x] S11.5 — Reborrowing: `let r2 = &*r1` creates new loan with subset relationship to original
- [x] S11.6 — Reborrow chain tracking: `reborrow_of(new_loan, original_loan)` for error message context
- [x] S11.7 — Mutable reborrow: `let r2 = &mut *r1` temporarily suspends original borrow
- [x] S11.8 — Nested borrow support: `&self.field` borrows `self` at field granularity (place projection)
- [x] S11.9 — Place projection facts: `field_of(place, field_name)` and `index_of(place, index_origin)` for fine-grained tracking
- [x] S11.10 — 10 tests: two-phase push pattern, reborrow chain, nested field borrow, place projection, mutable reborrow suspension

#### Sprint 12: Borrow Checker Error Improvements `P1`

**Goal:** Human-readable error messages with suggestions and migration path from NLL

- [x] S12.1 — Error template: "cannot borrow `X` as mutable because it is also borrowed as immutable" with source span highlights
- [x] S12.2 — Error template: "cannot move out of `X` because it is borrowed" with borrow location annotation
- [x] S12.3 — Error template: "`X` does not live long enough — borrowed value dropped while still in use" with lifetime visualization
- [x] S12.4 — Suggestion engine: "consider cloning the value", "consider using a reference", "try moving the borrow earlier"
- [x] S12.5 — Loan timeline visualization: ASCII art showing borrow ranges on code lines (like `|--- borrow of x starts here`)
- [x] S12.6 — Error code integration: ME011 TwoPhaseConflict, ME012 ReborrowConflict, ME013 PlaceConflict
- [x] S12.7 — Feature flag: `--polonius` enables Polonius checker, `--nll` uses existing NLL (default remains NLL until Polonius stable)
- [x] S12.8 — Polonius vs NLL comparison mode: `--borrow-check=compare` runs both, reports differences
- [x] S12.9 — Performance: Polonius solver completes in < 100ms for programs up to 10K LOC
- [x] S12.10 — 10 tests: all error templates render correctly, suggestions are appropriate, feature flags work, comparison mode output

### Phase 4: Transformer & Distributed ML `P1`

#### Sprint 13: Multi-Head Attention & Positional Encoding `P1`

**Goal:** Core Transformer building blocks — attention mechanism and position encoding

- [x] S13.1 — `ScaledDotProductAttention`: Q*K^T / sqrt(d_k), softmax, matmul with V — operates on (batch, heads, seq_len, d_k) tensors
- [x] S13.2 — Attention mask: causal mask (upper triangular -inf) for autoregressive decoding, padding mask for variable-length sequences
- [x] S13.3 — `MultiHeadAttention` layer: split Q/K/V into `n_heads`, parallel attention, concatenate, linear projection
- [x] S13.4 — MHA forward: `fn forward(query, key, value, mask) -> Tensor` with `W_q`, `W_k`, `W_v`, `W_o` weight matrices
- [x] S13.5 — MHA backward: gradient through concat, per-head attention, Q/K/V projections
- [x] S13.6 — `SinusoidalPositionalEncoding`: PE(pos, 2i) = sin(pos / 10000^(2i/d_model)), PE(pos, 2i+1) = cos(...)
- [x] S13.7 — `LearnedPositionalEncoding`: trainable embedding table of shape (max_seq_len, d_model) with gradient
- [x] S13.8 — `LayerNorm` layer: normalize across feature dimension, learnable gamma and beta parameters
- [x] S13.9 — LayerNorm backward: gradient through normalization, gamma, beta updates
- [x] S13.10 — 10 tests: attention scores shape, causal mask, MHA output shape, positional encoding values, LayerNorm normalization

#### Sprint 14: Transformer Encoder & Decoder Blocks `P1`

**Goal:** Full Transformer encoder and decoder with residual connections

- [x] S14.1 — `FeedForward` layer: Linear(d_model, d_ff) → GELU → Dropout → Linear(d_ff, d_model), expansion ratio typically 4x
- [x] S14.2 — FeedForward backward: gradient through both linear layers and activation
- [x] S14.3 — `TransformerEncoderLayer`: self-attention → add&norm → feedforward → add&norm (pre-norm or post-norm configurable)
- [x] S14.4 — `TransformerDecoderLayer`: masked self-attention → add&norm → cross-attention(Q=decoder, KV=encoder) → add&norm → FFN → add&norm
- [x] S14.5 — `TransformerEncoder`: stack of N encoder layers, optional final LayerNorm
- [x] S14.6 — `TransformerDecoder`: stack of N decoder layers with encoder output (memory) input
- [x] S14.7 — `Transformer` struct: encoder + decoder + src/tgt embedding + positional encoding + output linear projection
- [x] S14.8 — `TransformerConfig` struct: d_model, n_heads, n_layers, d_ff, dropout, max_seq_len, vocab_size, pre_norm flag
- [x] S14.9 — `examples/transformer_seq2seq.fj`: sequence-to-sequence translation with tiny vocabulary (< 100 tokens)
- [x] S14.10 — 10 tests: encoder output shape, decoder output shape, residual connection values, full transformer forward pass, gradient flow

#### Sprint 15: TFLite Model Import `P1`

**Goal:** Parse TensorFlow Lite .tflite files and map operations to Fajar ML runtime

- [x] S15.1 — Add `flatbuffers` dependency, `src/runtime/ml/tflite/mod.rs` module declaration
- [x] S15.2 — TFLite schema: FlatBuffer schema structs (Model, SubGraph, Tensor, Operator, Buffer, QuantizationParameters)
- [x] S15.3 — `tflite_load(path) -> Result<TfLiteModel>`: read .tflite file, parse FlatBuffer root table
- [x] S15.4 — Tensor mapping: TFLite TensorType (FLOAT32, INT8, UINT8) → Fajar `DType` (F32, I8, U8)
- [x] S15.5 — Op mapping: Conv2D → `fj_conv2d`, DepthwiseConv2D → `fj_depthwise_conv2d`, FullyConnected → `fj_dense`
- [x] S15.6 — Op mapping: Reshape, Softmax, ReLU, ReLU6, Add, Mul, MaxPool2D, AveragePool2D → corresponding Fajar ops
- [x] S15.7 — Quantization import: per-tensor and per-axis scales + zero_points → Fajar INT8 quantization format
- [x] S15.8 — `tflite_infer(model, input) -> Result<Tensor>`: run imported model graph sequentially through mapped operators
- [x] S15.9 — MobileNet-v2 test: import pre-trained TFLite MobileNet, run inference on 224x224 dummy input, verify output shape (1, 1000)
- [x] S15.10 — 10 tests: FlatBuffer parsing, tensor type mapping, Conv2D op, quantized model load, MobileNet output shape, invalid model error

#### Sprint 16: Distributed Training `P1`

**Goal:** Parameter server architecture for multi-node gradient aggregation

- [x] S16.1 — `src/runtime/ml/distributed/mod.rs`: distributed module with `DistributedConfig` struct (role, world_size, rank, server_addr)
- [x] S16.2 — `Role` enum: ParameterServer, Worker — server holds canonical weights, workers compute gradients
- [x] S16.3 — gRPC service definition: `PushGradients(gradients) -> Ack`, `PullWeights() -> Weights`, `Barrier(rank) -> Ack`
- [x] S16.4 — `ParameterServer`: accept gradients from workers, aggregate (mean), update weights, serve updated weights
- [x] S16.5 — `Worker`: compute forward + backward on local data shard, push gradients, pull updated weights
- [x] S16.6 — Gradient aggregation strategies: `AllReduceMean` (default), `AllReduceSum`, `TopK` (sparse gradients, keep top-K% by magnitude)
- [x] S16.7 — Synchronous training: barrier after each batch — all workers must push before server aggregates
- [x] S16.8 — Asynchronous training: workers push/pull independently — stale gradients bounded by `max_staleness` parameter
- [x] S16.9 — Data sharding: `DataLoader::shard(rank, world_size)` returns non-overlapping data partition for each worker
- [x] S16.10 — 10 tests: server/worker roundtrip (loopback), gradient aggregation correctness, barrier synchronization, data sharding, async staleness

### Phase 5: LLVM PGO & RTIC `P2`

#### Sprint 17: LLVM Instrumented Build `P2`

**Goal:** Instrument LLVM IR with profiling counters for profile data collection

- [x] S17.1 — `src/codegen/llvm/pgo.rs`: PGO module with `PgoMode` enum (Instrument, Optimize, Sample)
- [x] S17.2 — Instrumentation pass: insert `llvm.instrprof.increment` intrinsic at each basic block entry
- [x] S17.3 — Profile data filename: embed `__llvm_profile_filename` global with output path (e.g., `default_%p.profraw`)
- [x] S17.4 — `fj build --pgo=instrument` CLI flag: compile with profiling instrumentation enabled
- [x] S17.5 — Profile runtime link: ensure `compiler-rt` profiling library is linked for `__llvm_profile_write_file()`
- [x] S17.6 — `fj pgo merge <profraw_files>` CLI: invoke `llvm-profdata merge` to combine multiple .profraw → .profdata
- [x] S17.7 — Profile data validation: verify .profdata matches current source (function hash comparison)
- [x] S17.8 — Branch weight metadata: `!prof` metadata on `br` instructions from profile counters
- [x] S17.9 — Function entry counts: track call frequencies for inlining decisions
- [x] S17.10 — 10 tests: instrumentation insertion, profraw generation (mock), merge command, profile data validation, branch weight metadata

#### Sprint 18: PGO Optimization Pass `P2`

**Goal:** Use profile data to produce optimized binaries with improved branch prediction and inlining

- [x] S18.1 — `fj build --pgo=optimize --profile=data.profdata` CLI: compile with profile-guided optimization
- [x] S18.2 — Profile attachment: `module.set_profile_data()` via inkwell, attach block frequencies to IR
- [x] S18.3 — Hot/cold function splitting: functions with low call count marked `cold`, moved to separate section
- [x] S18.4 — Hot path optimization: frequently-taken branches get fall-through layout (reduce branch mispredictions)
- [x] S18.5 — PGO-guided inlining: increase inline threshold for hot call sites, decrease for cold
- [x] S18.6 — Indirect call promotion: profile data reveals `fn_ptr(x)` usually calls `concrete_fn` → speculative devirtualization
- [x] S18.7 — Loop unrolling hints: loops with known trip count from profile → `llvm.loop.unroll.count` metadata
- [x] S18.8 — PGO report: `fj pgo report data.profdata` → summary (total functions, hot/cold counts, coverage %)
- [x] S18.9 — Benchmark: fibonacci(30) + matrix multiply PGO vs non-PGO — expect 5-15% speedup
- [x] S18.10 — 10 tests: profile attachment, hot/cold splitting, inline threshold adjustment, loop unroll hint, benchmark improvement verification

#### Sprint 19: RTIC Compile-Time Scheduler `P2`

**Goal:** Priority-based interrupt task scheduling resolved entirely at compile time

- [x] S19.1 — `src/rtos/rtic/mod.rs`: RTIC module with `RticApp` struct (tasks, resources, priorities, device peripherals)
- [x] S19.2 — `@task(priority = N, binds = INTERRUPT)` annotation: declare interrupt-driven task with static priority
- [x] S19.3 — `@resource` annotation on struct fields: shared resources between tasks with automatic locking
- [x] S19.4 — Priority analysis: compute ceiling priority for each resource (maximum of all accessor task priorities)
- [x] S19.5 — `@init` function: runs before scheduler, returns initialized resources (shared state) and optional monotonics
- [x] S19.6 — `@idle` function: lowest-priority task, runs when no interrupt pending (WFI instruction in loop)
- [x] S19.7 — Resource proxy generation: each task receives `cx: TaskContext` with only the resources it declared access to
- [x] S19.8 — Critical section elimination: if task priority >= resource ceiling, no lock needed (compile-time proof)
- [x] S19.9 — Stack analysis: single shared stack per priority level, worst-case stack = sum of max-frame per level
- [x] S19.10 — 10 tests: priority assignment, ceiling calculation, critical section elimination, stack analysis, init/idle function detection

#### Sprint 20: RTIC Code Generation `P2`

**Goal:** Generate interrupt handler code and resource lock mechanisms from RTIC annotations

- [x] S20.1 — Vector table generation: map `binds = INTERRUPT` to ISR vector entry for target MCU
- [x] S20.2 — Interrupt handler trampoline: ISR entry → save context → call task function → restore → return
- [x] S20.3 — BASEPRI-based locking (Cortex-M): `lock()` raises BASEPRI to resource ceiling, restores on unlock
- [x] S20.4 — Software task spawning: `task::spawn()` enqueues message + sets PendSV for deferred execution
- [x] S20.5 — Message passing: task with `capacity = N` gets SPSC queue for incoming messages
- [x] S20.6 — Monotonic timer: `@monotonic(binds = TIM2, default = true)` for `spawn_after(duration)` scheduling
- [x] S20.7 — Timer queue: sorted linked list of scheduled tasks, fired from monotonic ISR
- [x] S20.8 — `examples/rtic_blinky.fj`: LED toggle via periodic timer task + GPIO resource on STM32F407
- [x] S20.9 — Deadlock-freedom proof: static analysis confirms no circular resource dependency across priority levels
- [x] S20.10 — 10 tests: vector table entries, BASEPRI lock values, software task queue, timer scheduling order, deadlock-freedom check

### Phase 6: Package Signing & Security `P1`

#### Sprint 21: Sigstore Integration `P1`

**Goal:** Keyless package signing using Sigstore (Fulcio + Rekor transparency log)

- [x] S21.1 — Add `sigstore` crate dependency (feature-gated under `signing`), `src/package/signing.rs` module
- [x] S21.2 — OIDC identity flow: `fj login --sigstore` opens browser for GitHub/Google OIDC, obtains identity token
- [x] S21.3 — Fulcio certificate request: exchange OIDC token for short-lived X.509 signing certificate (10 minute validity)
- [x] S21.4 — Package signing: compute SHA-256 of .fjpkg tarball, sign digest with Fulcio certificate private key
- [x] S21.5 — Rekor transparency log: upload signature + certificate + artifact hash to Rekor, receive log entry UUID
- [x] S21.6 — Signature bundle: `FjSignatureBundle` struct (certificate PEM, signature bytes, rekor_log_id, rekor_log_index)
- [x] S21.7 — `fj publish --sign`: automatically sign package before upload, include bundle in registry metadata
- [x] S21.8 — Signature storage: registry stores `signature.json` alongside each package version
- [x] S21.9 — Trust root: embed Sigstore TUF root of trust for Fulcio + Rekor certificate verification
- [x] S21.10 — 10 tests: OIDC flow mock, certificate request mock, digest signing, Rekor entry construction, bundle serialization

#### Sprint 22: Package Verification `P1`

**Goal:** Verify package signatures on install, enforce signing policy

- [x] S22.1 — `fj install` verification: download package + signature bundle, verify before extraction
- [x] S22.2 — Certificate verification: validate Fulcio certificate chain against embedded Sigstore root CA
- [x] S22.3 — Signature verification: verify ECDSA/Ed25519 signature on package digest using certificate public key
- [x] S22.4 — Rekor verification: query Rekor API to confirm log entry exists and inclusion proof is valid
- [x] S22.5 — Identity verification: check certificate SAN (Subject Alternative Name) matches expected publisher email/repo
- [x] S22.6 — `--require-signatures` flag: reject unsigned packages (default: warn only)
- [x] S22.7 — `--trusted-publishers` config: list of email/repo identities allowed to publish (e.g., `fajar@primecore.id`)
- [x] S22.8 — Signature cache: store verified signature status in `~/.fj/cache/verified/` to skip re-verification
- [x] S22.9 — `fj verify <package>`: standalone command to verify an already-installed package signature
- [x] S22.10 — 10 tests: valid signature passes, invalid signature fails, expired cert fails, Rekor inclusion mock, trusted publisher filter

#### Sprint 23: Dependency Vulnerability Scanning `P1`

**Goal:** Check installed packages against known vulnerability advisories

- [x] S23.1 — `src/package/audit.rs`: vulnerability scanning module with `Advisory` struct (id, package, versions_affected, severity, description)
- [x] S23.2 — Advisory database format: JSON array of advisories, fetched from registry `GET /api/v1/advisories`
- [x] S23.3 — `fj audit`: scan `fj.lock` dependencies against advisory database, report affected packages
- [x] S23.4 — Severity levels: Critical, High, Medium, Low — with color-coded terminal output (red, yellow, blue, dim)
- [x] S23.5 — Version range matching: `>=1.0.0, <1.2.3` style affected version ranges, semver comparison
- [x] S23.6 — `fj audit --fix`: suggest updated versions that resolve advisories, update `fj.toml` if safe
- [x] S23.7 — CI integration: `fj audit --exit-code` returns non-zero if Critical/High advisories found (for CI gates)
- [x] S23.8 — Advisory submission: `fj advisory submit` CLI for reporting new vulnerabilities to registry
- [x] S23.9 — Ignore list: `[audit.ignore]` in `fj.toml` for known false positives with justification string
- [x] S23.10 — 10 tests: advisory matching, version range intersection, severity filtering, fix suggestion, ignore list, exit code

#### Sprint 24: Supply Chain Security `P1`

**Goal:** SBOM generation and reproducible builds for auditable software supply chain

- [x] S24.1 — `src/package/sbom.rs`: SBOM module with `SbomDocument` struct (format, packages, relationships, creation_info)
- [x] S24.2 — CycloneDX output: `fj sbom --format cyclonedx` generates CycloneDX 1.6 JSON with all direct + transitive dependencies
- [x] S24.3 — SPDX output: `fj sbom --format spdx` generates SPDX 2.3 JSON with license and copyright info
- [x] S24.4 — Package metadata in SBOM: name, version, purl (pkg:fj/name@version), sha256, license (SPDX expression)
- [x] S24.5 — Dependency relationships: DEPENDS_ON edges between packages, distinguishing direct vs transitive
- [x] S24.6 — Reproducible builds: `fj build --reproducible` strips timestamps, randomized addresses, sorts sections deterministically
- [x] S24.7 — Build provenance: `fj build --provenance` generates SLSA provenance statement (builder, source, build config)
- [x] S24.8 — Binary hash registry: `fj attest <binary>` uploads SHA-256 of built binary to transparency log for verification
- [x] S24.9 — `fj supply-chain report`: summary of all dependencies, signatures, advisories, SBOM status in one view
- [x] S24.10 — 10 tests: CycloneDX schema validation, SPDX output format, purl construction, reproducible build determinism, provenance fields

### Phase 7: Power Management & Production Polish `P2`

#### Sprint 25: Power Management APIs `P2`

**Goal:** Sleep modes, wake sources, clock gating for battery-powered embedded devices

- [x] S25.1 — `src/runtime/os/power.rs`: power management module with `PowerMode` enum (Run, Sleep, Stop, Standby, Shutdown)
- [x] S25.2 — `enter_sleep_mode(mode)`: execute WFI (Wait For Interrupt) for Cortex-M, configurable SLEEPDEEP for Stop/Standby
- [x] S25.3 — `WakeSource` enum: Interrupt(irq_number), RtcAlarm(timestamp), GpioPin(pin, edge), WakeupTimer(duration_ms)
- [x] S25.4 — `configure_wake_source(source) -> Result<()>`: enable EXTI line, RTC alarm, or WKUP pin before entering low-power mode
- [x] S25.5 — Clock gating: `clock_enable(peripheral)` / `clock_disable(peripheral)` via RCC APB/AHB enable registers
- [x] S25.6 — `PowerBudget` struct: measure active current, sleep current, wake latency — report estimated battery life
- [x] S25.7 — Voltage scaling: `set_vos(level)` for Cortex-M7/M33 — trade clock speed for power (VOS1=max perf, VOS3=min power)
- [x] S25.8 — `@low_power` annotation: analyzer warns if function uses peripherals without enabling their clocks first
- [x] S25.9 — Auto clock gating: compiler analysis determines unused peripherals per code path, inserts clock_disable calls
- [x] S25.10 — 10 tests: power mode transitions, wake source configuration, clock gating register values, voltage scaling, auto-gating analysis

#### Sprint 26: Real Hardware CI `P2`

**Goal:** GitHub Actions + self-hosted runners with probe-rs for on-hardware testing

- [x] S26.1 — `.github/workflows/hardware-ci.yml`: self-hosted runner job with `runs-on: [self-hosted, arm-board]` label
- [x] S26.2 — probe-rs test runner: `fj test --board stm32f407 --probe stlink` flashes binary, captures semihosting output, checks assertions
- [x] S26.3 — Test harness for embedded: `#[embedded_test]` attribute compiles test to firmware, runs on device, reports pass/fail via semihosting
- [x] S26.4 — QEMU fallback: if no physical board detected, run against QEMU target (`qemu-system-arm -machine lm3s6965evb`) for CI
- [x] S26.5 — Flash timeout: 30-second timeout per test binary flash+run, kill and report failure on timeout
- [x] S26.6 — Serial output capture: collect UART output from board during test execution for debugging failures
- [x] S26.7 — Board matrix: CI tests against STM32F407, RP2040 (via picoprobe), ESP32 (via esptool) — skip unavailable boards
- [x] S26.8 — Test result aggregation: JUnit XML output from embedded test runner for GitHub Actions test reporting
- [x] S26.9 — Hardware CI badge: `![Hardware CI](https://img.shields.io/...)` status in README
- [x] S26.10 — 10 tests: workflow YAML validation, probe-rs command construction, QEMU fallback detection, serial capture mock, JUnit XML format

#### Sprint 27: LSP Improvements `P2`

**Goal:** Enhanced auto-completion, rename refactoring, and diagnostics

- [x] S27.1 — Completion: trigger on `.` (field/method), `::` (module/associated), `<` (generic params) — context-aware candidate list
- [x] S27.2 — Completion candidates: local variables, function parameters, struct fields, enum variants, imported names, builtins
- [x] S27.3 — Completion detail: show type signature, doc comment preview, kind icon (function, variable, struct, enum)
- [x] S27.4 — Rename symbol: `textDocument/rename` — find all references, validate new name, apply workspace edit
- [x] S27.5 — Find all references: `textDocument/references` — cross-file search for variable, function, type, field usages
- [x] S27.6 — Go to type definition: `textDocument/typeDefinition` — jump from variable to its type declaration
- [x] S27.7 — Inlay hints: show inferred types for `let` bindings, parameter names at call sites, return type for closures
- [x] S27.8 — Diagnostic improvements: real-time error reporting with fix suggestions (quick-fix code actions)
- [x] S27.9 — Workspace symbols: `workspace/symbol` — fuzzy search across all files for functions, types, constants
- [x] S27.10 — 10 tests: completion candidates, rename consistency, reference finding, inlay hint positions, workspace symbol search

#### Sprint 28: Release Preparation `P2`

**Goal:** Changelog, version bumps, benchmarks, documentation, final examples

- [x] S28.1 — Version bump: update `Cargo.toml` version to `0.7.0`, `CLAUDE.md` status section, `src/main.rs` version string
- [x] S28.2 — `docs/CHANGELOG.md` update: v0.7.0 "Zenith" entry with all 7 phases summarized
- [x] S28.3 — mdBook update: new chapters for Wasm backend, ESP32 WiFi/BLE, Transformer, borrow checker, package signing
- [x] S28.4 — `examples/wasm_ml_inference.fj`: compile ML model to Wasm for browser-side inference
- [x] S28.5 — `examples/esp32_iot_sensor.fj`: WiFi connect → MQTT publish sensor data → BLE notify → OTA check
- [x] S28.6 — `examples/transformer_classify.fj`: small Transformer text classifier with positional encoding + attention
- [x] S28.7 — `examples/distributed_mnist.fj`: 2-worker distributed MNIST training with parameter server
- [x] S28.8 — Benchmark suite: Wasm vs Cranelift vs LLVM performance comparison (fibonacci, matmul, inference)
- [x] S28.9 — Regression test: full test suite passes (2,293+ baseline, zero failures, zero clippy warnings)
- [x] S28.10 — 10 tests: version string correct, changelog entry present, all new examples compile, benchmark harness runs

---

## Dependencies

```
Phase 1 (Wasm) ──────────────────────── Independent (new codegen backend)
Phase 2 (ESP32/IoT) ────────────────── Requires v0.6 BSP framework (esp32.rs exists)
Phase 3 (Polonius) ──────────────────── Independent (analyzer-only, replaces NLL)
Phase 4 (Transformer) ──────────────── Requires v0.6 ML runtime (LSTM/GRU, autograd)
Phase 4 (TFLite) ────────────────────── Requires v0.6 ML runtime (tensor ops, quantization)
Phase 4 (Distributed) ──────────────── Requires Phase 4 S13-S14 (Transformer layers)
Phase 5 (PGO) ──────────────────────── Requires v0.6 LLVM backend (inkwell)
Phase 5 (RTIC) ─────────────────────── Requires v0.6 BSP + RTOS (board support, vector tables)
Phase 6 (Signing) ──────────────────── Requires v0.6 package registry (publish/install flow)
Phase 6 (Vulnerability) ────────────── Requires v0.6 registry server (advisory endpoint)
Phase 7 (Power) ────────────────────── Requires v0.6 BSP (register-level HAL)
Phase 7 (Hardware CI) ──────────────── Requires v0.6 BSP + flash tools (probe-rs, esptool)
Phase 7 (LSP) ──────────────────────── Independent (extends existing tower-lsp server)
```

**Critical path:** Phase 4 S13-S14 (Transformer) → Phase 4 S16 (Distributed uses Transformer for demo)

**Parallel tracks:**
- Track A: Phase 1 (Wasm, independent)
- Track B: Phase 2 (ESP32/IoT, depends on v0.6 BSP)
- Track C: Phase 3 (Polonius, independent analyzer work)
- Track D: Phase 4 (Transformer + TFLite + Distributed, sequential within phase)
- Track E: Phase 5 (PGO + RTIC, depends on v0.6 LLVM + BSP)
- Track F: Phase 6 (Signing + Security, depends on v0.6 registry)
- Track G: Phase 7 (Power + CI + LSP, mixed dependencies)

---

## Success Criteria

- [x] `fj build --target wasm` produces valid .wasm binary, runnable via wasmtime and in browser
- [x] Wasm hello world binary < 10KB, ML inference model < 500KB
- [x] `wifi_connect_sta(ssid, password)` connects ESP32 to WiFi AP (simulation + real hardware)
- [x] BLE GATT server advertises and accepts connections from nRF Connect mobile app
- [x] MQTT client publishes telemetry to Mosquitto broker at 10 messages/second
- [x] OTA update downloads and flashes new firmware without physical access
- [x] Polonius borrow checker accepts `vec.push(vec.len())` pattern (two-phase borrows)
- [x] Polonius detects use-after-move and dangling reference with clear error messages
- [x] `TransformerEncoder` with 2 layers, 4 heads, d_model=64 produces correct output shape
- [x] TFLite MobileNet-v2 import produces correct output shape (1, 1000)
- [x] Distributed training with 2 workers converges on MNIST (> 85% accuracy)
- [x] PGO-optimized binary runs 5-15% faster than non-PGO on compute benchmarks
- [x] RTIC compile-time scheduler eliminates runtime lock overhead (no BASEPRI for ceiling-safe access)
- [x] `fj publish --sign` uploads signed package, `fj install --require-signatures` verifies before extract
- [x] `fj audit` detects known-vulnerable dependency and reports severity + fix suggestion
- [x] `fj sbom --format cyclonedx` generates valid CycloneDX 1.6 JSON
- [x] Power management: STM32 enters Stop mode, wakes on RTC alarm within 1ms
- [x] Hardware CI: at least one physical board runs test suite in GitHub Actions
- [x] LSP: auto-completion shows struct fields after `.`, rename refactors across files
- [x] All existing 2,293 tests pass (zero regressions), 0 clippy warnings

---

## Stats Targets

| Metric | v0.6 (current) | v0.7 (target) |
|--------|----------------|---------------|
| Tests | 2,293 | 5,000+ |
| LOC | ~115,000 | ~165,000 |
| Examples | 33 | 42+ |
| Error codes | 90+ | 105+ |
| Codegen backends | 2 (Cranelift + LLVM) | 3 (+ Wasm) |
| BSP boards | 6 | 6 (+ ESP32 WiFi/BLE enabled) |
| RTOS support | 2 (FreeRTOS + Zephyr) | 3 (+ RTIC compile-time) |
| ML layers | LSTM, GRU, Dense, Conv2d, MHA, BatchNorm, Dropout, Embedding | + Transformer, LayerNorm, FeedForward, PositionalEncoding |
| ML import formats | ONNX | ONNX + TFLite |
| IoT protocols | 0 | 3 (WiFi, BLE, MQTT) |
| Package security | checksum only | Sigstore signing + vulnerability scanning + SBOM |
| Borrow checker | NLL (scope-based) | Polonius (flow-sensitive) + two-phase borrows |

---

## Non-Goals (Deferred to v0.8+)

- GPU-accelerated training (CUDA tensor core kernels) — requires custom CUDA codegen, not just FFI
- Self-hosted compiler (full bootstrap in .fj) — self-hosting lexer/parser done in v0.3, full compiler too complex
- Async trait methods — requires GAT (Generic Associated Types) which is not yet in the type system
- Incremental compilation — requires dependency graph + artifact cache, major compiler rewrite
- Language server protocol for Wasm target (debug Wasm in VS Code) — DAP over Wasm needs Chrome DevTools bridge
- Thread-safe garbage collector — Fajar uses ownership; GC conflicts with embedded/real-time goals
- LoRaWAN connectivity — niche IoT protocol, defer until demand materializes
- Model pruning & knowledge distillation — optimization techniques beyond quantization, defer to ML-focused release
- Auto-differentiation for custom ops — current autograd handles standard ops; custom op gradients need JVP/VJP framework

---

*V07_PLAN.md v1.0 | Created 2026-03-11 | 7 Phases, 28 Sprints, 280 Tasks*
