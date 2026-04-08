# FajarQuant Kernel Integration Plan

> **Version:** 1.0 (2026-04-08)
> **Author:** Muhamad Fajar Putranto, SE., SH., MH. (TaxPrime / PrimeCore.id)
> **Goal:** FajarOS becomes the world's first OS with kernel-native LLM inference
> **Model:** Claude Opus 4.6 exclusively
> **Status:** PLANNING

---

## Executive Summary

FajarQuant currently exists only in the Fajar Lang compiler runtime (Rust/ndarray).
FajarOS already has a kernel-native quantization engine (quantize.fj, tensor.fj,
inference.fj) that works in @kernel context with no heap allocation.

This plan bridges the gap: port FajarQuant's three innovations (PCA rotation,
fused attention, hierarchical allocation) to FajarOS bare-metal, then build
kernel-native LLM inference on top.

**End state:** `nova> ask "how to mount USB?"` runs a 135M-parameter LLM
entirely inside the kernel, with FajarQuant-compressed KV cache.

---

## Architecture Overview

```
                        FajarOS Kernel
    ┌──────────────────────────────────────────────────┐
    │                                                  │
    │  Shell (nova>)                                   │
    │    │                                             │
    │    ├── ask "prompt"    ──── LLM Pipeline ────┐   │
    │    ├── classify         ── ML Scheduler      │   │
    │    └── quant-bench      ── Benchmarks        │   │
    │                                              │   │
    │  ┌─── LLM Inference Engine ────────────────┐ │   │
    │  │                                         │ │   │
    │  │  Tokenizer (BPE, @kernel)               │ │   │
    │  │       │                                 │ │   │
    │  │  Embedding Lookup (@kernel)             │ │   │
    │  │       │                                 │ │   │
    │  │  Transformer Layers (×N)                │ │   │
    │  │    ├── LayerNorm (@kernel)              │ │   │
    │  │    ├── QKV Projection (@kernel)         │ │   │
    │  │    ├── FajarQuant Attention (@kernel) ◄─┼─┘   │
    │  │    │   ├── PCA Rotation (pre-computed)  │     │
    │  │    │   ├── Fused Codebook Dot Product   │     │
    │  │    │   └── Hierarchical KV Cache        │     │
    │  │    ├── FFN (@kernel)                    │     │
    │  │    └── Residual + LayerNorm             │     │
    │  │       │                                 │     │
    │  │  LM Head → Logits → Token              │     │
    │  │       │                                 │     │
    │  │  Detokenizer → Serial Output           │     │
    │  └─────────────────────────────────────────┘     │
    │                                                  │
    │  ┌─── Memory Layout ───────────────────────────┐ │
    │  │ 0xB00000  Tensor Pool (1 MB, 32 slots)      │ │
    │  │ 0xB10000  Quantization Codebooks            │ │
    │  │ 0xB20000  Model Metadata                    │ │
    │  │ 0xC00000  Model Weights (loaded from disk)  │ │
    │  │ 0x4000000 KV Cache Ring Buffer (quantized)  │ │
    │  └─────────────────────────────────────────────┘ │
    └──────────────────────────────────────────────────┘
```

---

## What Already Exists (Foundation)

| Component | File | LOC | Status |
|-----------|------|-----|--------|
| 2-bit/4-bit Lloyd-Max quantization | `kernel/compute/quantize.fj` | 168 | @kernel, working |
| Fixed-slot tensor engine (32 slots, 128 dims) | `kernel/compute/tensor.fj` | 294 | @kernel, working |
| 2-layer neural network inference | `kernel/compute/inference.fj` | 216 | @kernel, working |
| Matrix multiply kernel | `kernel/compute/kernels.fj` | 162 | @kernel, working |
| Compute buffers (16 x 4KB) | `kernel/compute/buffers.fj` | 64 | @kernel, working |
| MNIST 7x7 classifier | `shell/commands.fj` | 206 | @kernel, working |
| Frame allocator (120MB free) | `kernel/mm/frames.fj` | 80 | @kernel, working |
| RamFS file storage | `fs/ramfs.fj` | 400+ | @kernel, working |
| NVMe disk I/O | `drivers/nvme.fj` | 600+ | @kernel, working |

**Key insight:** FajarOS already has a working @kernel tensor engine with
no-heap quantization. We're extending it, not building from scratch.

---

## What FajarQuant Adds (3 Innovations)

| Innovation | Benefit | Kernel Adaptation |
|------------|---------|-------------------|
| **PCA Rotation** | 4-6% better MSE | Pre-compute rotation matrix offline, store as i64 flat array |
| **Fused Attention** | O(N*d) -> O(2^b) memory | Codebook dot product on quantized indices (no dequant buffer) |
| **Hierarchical Allocation** | 48.7% bit savings | Lookup table for tier assignment, demote old tokens in-place |

---

## Constraints

| Constraint | Value | Impact |
|-----------|-------|--------|
| Kernel ELF max | 15 MB | Model weights must load from disk, not embed in ELF |
| QEMU RAM | 512 MB | SmolLM-135M 2-bit (34MB) fits in RAM, not in ELF |
| Tensor pool | 1 MB (32 slots x 128 dims) | Need to extend for 768-dim transformer layers |
| Heap | 16 MB at 0x7000000 | Available for weight buffer during layer-by-layer inference |
| No heap in @kernel | Compiler-enforced | All quantization ops must use pre-allocated memory |
| Fixed-point arithmetic | i64 scaled x1000 | Sufficient precision for 2-4 bit quantization |

---

## Phase 1: Bare-Metal FajarQuant (kernel/compute/fajarquant.fj)

**Goal:** Port the 3 FajarQuant innovations to @kernel context.
**Duration:** 1 session
**Dependencies:** None (builds on existing quantize.fj + tensor.fj)

### Architecture

```
kernel/compute/fajarquant.fj
    ├── Adaptive PCA rotation (pre-computed i64 matrix)
    ├── Fused codebook dot product (quantized attention)
    ├── Hierarchical bit scheduler (tier lookup table)
    └── KV cache ring buffer (fixed-capacity, no heap)
```

### Memory Layout

```
0xB10000  CODEBOOK_BASE     Quantization codebooks (4 bit-widths x 16 centroids)
0xB10200  ROTATION_BASE     Pre-computed PCA rotation matrix (128x128 i64)
0xB30000  HIER_SCHEDULE     Hierarchical tier lookup (max 16K tokens)
0xB30100  KV_CACHE_BASE     Quantized KV cache ring buffer
          KV_CACHE_SIZE     Configurable (default: 2048 tokens x 2 x dim bytes)
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 1.1 | Port codebook data structures to i64 fixed-point | `quant-bench` shows correct MSE | 1h |
| 1.2 | Implement PCA rotation as pre-computed flat i64 matrix | Rotate + quantize matches Rust MSE within 1% | 2h |
| 1.3 | Implement fused codebook dot product (@kernel) | `q^T * codebook[idx]` matches standard dot product | 2h |
| 1.4 | Implement hierarchical bit scheduler (tier lookup) | Tier assignment matches Python script output | 1h |
| 1.5 | Implement KV cache ring buffer (fixed-capacity) | Append/read/demote work, no heap allocation | 2h |
| 1.6 | Shell command `quant-bench` | Runs benchmark, prints MSE comparison | 1h |
| 1.7 | Shell command `quant-info` | Shows codebook, rotation, cache stats | 0.5h |
| 1.8 | Integration tests (5 tests in kernel_tests.fj) | All pass in QEMU `test-all` | 1h |

**Gate:** `nova> quant-bench` runs, shows FajarQuant MSE < TurboQuant MSE.

**Deliverable:** `kernel/compute/fajarquant.fj` (~400 LOC, all @kernel)

---

## Phase 2: Tensor Engine Extension (128 -> 768 dims)

**Goal:** Support transformer-sized tensors (d=768 for SmolLM).
**Duration:** 1 session
**Dependencies:** Phase 1

### Why

Current tensor engine supports max 128 dimensions per slot (tensor.fj line 5).
Transformer models use d_model=768 (SmolLM) or d_model=2048 (TinyLlama).
Need to extend capacity while keeping @kernel constraints.

### Design

```
Current:  32 slots x 128 elements = 32 KB (at 0xB00000)
Extended: 16 large slots x 1024 elements = 128 KB (at 0xB40000)
          + 32 small slots x 128 elements = 32 KB (at 0xB00000, unchanged)

Total tensor memory: 160 KB (fits easily in identity-mapped region)
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 2.1 | Add large tensor pool (16 slots x 1024 dims at 0xB40000) | `tensor_alloc_large(1024)` returns valid slot | 1h |
| 2.2 | Extend matmul for 768x768 (tiled, 64x64 blocks) | Correct result for 768x768 identity test | 3h |
| 2.3 | Add LayerNorm (@kernel, no heap) | Output matches PyTorch within 0.1% | 1h |
| 2.4 | Add GELU activation (@kernel) | Matches PyTorch GELU within 0.01 | 0.5h |
| 2.5 | Add softmax for attention scores | Sum-to-1 verified, numerically stable | 0.5h |
| 2.6 | Shell command `tensor-bench` | Reports GFLOPS for matmul sizes | 1h |

**Gate:** `nova> tensor-bench` shows 768x768 matmul completes without crash.

**Deliverable:** Extended `kernel/compute/tensor.fj` (~200 LOC additional)

---

## Phase 3: Model Weight Loader (disk -> kernel memory)

**Goal:** Load quantized model weights from NVMe/RamFS into kernel memory.
**Duration:** 1 session
**Dependencies:** Phase 2

### Why

SmolLM-135M at 2-bit = 34 MB. Cannot embed in 15 MB kernel ELF.
Must load from disk (NVMe or RamFS) into pre-allocated kernel memory.

### Binary Format: `.fjm` (Fajar Model)

```
Header (64 bytes):
  magic:      "FJM1"              (4 bytes)
  version:    1                    (4 bytes)
  model_type: 1=SmolLM,2=TinyLlama (4 bytes)
  n_layers:   12                   (4 bytes)
  d_model:    768                  (4 bytes)
  n_heads:    12                   (4 bytes)
  d_head:     64                   (4 bytes)
  vocab_size: 49152                (4 bytes)
  quant_bits: 2                    (4 bytes)
  total_size: N bytes              (4 bytes)
  reserved:   ...                  (24 bytes)

Per-Layer Block (repeated n_layers times):
  layer_id:   i32
  block_size: i32
  qkv_weight: d_model * 3 * d_model * quant_bits/8 bytes (quantized)
  ffn_w1:     d_model * 4*d_model * quant_bits/8 bytes
  ffn_w2:     4*d_model * d_model * quant_bits/8 bytes
  ln1_gamma:  d_model * 8 bytes (f64, not quantized)
  ln1_beta:   d_model * 8 bytes (f64, not quantized)
  ln2_gamma:  d_model * 8 bytes (f64, not quantized)
  ln2_beta:   d_model * 8 bytes (f64, not quantized)
  rotation:   d_head * d_head * 8 bytes (PCA rotation matrix)
  codebook:   2^quant_bits * 8 bytes (Lloyd-Max centroids)

Embedding Table:
  vocab_size * d_model * quant_bits/8 bytes

LM Head:
  d_model * vocab_size * quant_bits/8 bytes
```

### Memory Map for Loaded Model

```
0xC00000   MODEL_META      Header (64 bytes)
0xC00040   MODEL_EMBED     Embedding table (~6 MB at 2-bit for 49K vocab)
0x1200000  MODEL_LAYERS    Layer weights (layer-by-layer, ~2 MB each)
0x4000000  KV_CACHE        Quantized KV cache (hierarchical)
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 3.1 | Define `.fjm` binary format (header + per-layer) | Format spec documented | 0.5h |
| 3.2 | Python script: convert HuggingFace model to `.fjm` | `python export_fjm.py --model SmolLM-135M --bits 2` produces valid file | 3h |
| 3.3 | Kernel `fjm_load()`: parse header from RamFS | Header fields correct after load | 1h |
| 3.4 | Kernel `fjm_load_layer(n)`: load layer N into tensor pool | Layer weights accessible via tensor_get() | 2h |
| 3.5 | Kernel `fjm_load_embed()`: load embedding table | Embedding lookup returns correct vector | 1h |
| 3.6 | Frame allocator: reserve contiguous region for model | `frame_alloc_contiguous(N)` returns N consecutive frames | 1h |
| 3.7 | Shell command `model-load <file>` | Loads .fjm, prints layer count and size | 1h |
| 3.8 | Shell command `model-info` | Shows loaded model metadata | 0.5h |

**Gate:** `nova> model-load smollm.fjm` loads header + 1 layer without crash.

**Deliverable:** `kernel/compute/model_loader.fj` + `scripts/export_fjm.py`

---

## Phase 4: Tokenizer (BPE, @kernel)

**Goal:** Byte-Pair Encoding tokenizer that runs in @kernel context.
**Duration:** 1 session
**Dependencies:** Phase 3

### Why

LLM inference requires tokenizing user input to token IDs and detokenizing
output IDs back to text. Must work in @kernel (no heap for string ops).

### Design

```
Simplified BPE for kernel:
  - Vocabulary: 49,152 tokens (SmolLM)
  - Token table: stored in RamFS, loaded at boot
  - Encode: greedy longest-match (not full BPE merge)
  - Decode: direct lookup from token ID to byte sequence

Memory:
  0xB50000  TOKEN_TABLE    Vocab entries (49K x 16 bytes = 768 KB)
  0xC10000  TOKEN_SCRATCH  Encode/decode scratch buffer (4 KB)
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 4.1 | Python: export SmolLM tokenizer to binary table | `export_tokenizer.py` produces token_table.bin | 1h |
| 4.2 | Kernel `tokenize(text_addr, text_len, out_addr)` | "hello" -> correct token IDs | 2h |
| 4.3 | Kernel `detokenize(token_id, out_addr)` | Token ID -> correct byte sequence | 1h |
| 4.4 | Shell command `tokenize <text>` | Shows token IDs for input text | 0.5h |
| 4.5 | Load token table from RamFS at boot | Token table accessible after boot | 1h |

**Gate:** `nova> tokenize hello world` outputs correct token IDs.

**Deliverable:** `kernel/compute/tokenizer.fj` (~200 LOC)

---

## Phase 5: Transformer Forward Pass (@kernel)

**Goal:** Single-token forward pass through transformer with quantized attention.
**Duration:** 2 sessions
**Dependencies:** Phase 1-4

### Pipeline

```
Input: token_id (i64)
  │
  ├── Embedding lookup: token_id → x[768]
  │
  ├── For each layer (0..11):
  │   ├── LayerNorm(x)
  │   ├── QKV = x @ W_qkv (quantized matmul)
  │   ├── Q, K, V = split(QKV, 3)
  │   ├── K_rot = PCA_rotate(K)          ← FajarQuant Innovation 1
  │   ├── KV_cache.append(K_rot, V)      ← FajarQuant Innovation 3 (hierarchical)
  │   ├── attn = fused_attention(Q, KV_cache, codebook)  ← FajarQuant Innovation 2
  │   ├── x = x + attn @ W_o
  │   ├── LayerNorm(x)
  │   ├── ffn = GELU(x @ W1) @ W2       (quantized matmul)
  │   └── x = x + ffn
  │
  ├── LayerNorm(x)
  ├── logits = x @ LM_head              (quantized matmul)
  └── next_token = argmax(logits) or sample(logits, temperature)
```

### Quantized Matrix Multiply

```
Standard:  Y = X @ W      where W is f64[M,N]
Quantized: Y = X @ dequant(W_q, codebook)
         = sum_j  X[j] * codebook[W_q[j]]   (fused, no dequant buffer)

@kernel fn qmatmul(x_slot: i64, wq_addr: i64, codebook_addr: i64,
                    rows: i64, cols: i64, out_slot: i64) {
    // For each output element: dot product of x with quantized weight column
    // Uses codebook lookup, no intermediate dequantized matrix
}
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 5.1 | Quantized matmul: x @ dequant(W_q) via codebook | Output matches Python within 1% | 3h |
| 5.2 | Multi-head attention split (Q/K/V from QKV) | Shapes correct: [n_heads, d_head] | 1h |
| 5.3 | PCA-rotated K insertion into KV cache | Rotated keys stored, retrievable | 1h |
| 5.4 | Fused attention: Q @ KV_cache via codebook | Output matches standard attention within 1% | 3h |
| 5.5 | Hierarchical KV cache demotion on append | Old tokens demoted to fewer bits | 1h |
| 5.6 | Full transformer layer (LN + attn + FFN + residual) | Single layer output matches PyTorch | 3h |
| 5.7 | Full forward pass (12 layers, layer-by-layer load) | Final logits produce valid token distribution | 3h |
| 5.8 | Argmax + temperature sampling | Generates coherent tokens | 1h |
| 5.9 | Shell command `infer <text>` (1 token) | Generates 1 next token from prompt | 1h |

**Gate:** `nova> infer "The capital of"` → produces a reasonable next token.

**Deliverable:** `kernel/compute/transformer.fj` (~600 LOC)

---

## Phase 6: Autoregressive Generation + Shell Integration

**Goal:** `nova> ask "question"` generates multi-token response.
**Duration:** 1 session
**Dependencies:** Phase 5

### Design

```
@kernel fn cmd_ask() {
    // 1. Tokenize user prompt
    let n_tokens = tokenize(prompt_addr, prompt_len, token_buf)

    // 2. Prefill: process all prompt tokens
    let mut pos: i64 = 0
    while pos < n_tokens {
        transformer_forward(token_buf[pos], pos)
        pos = pos + 1
    }

    // 3. Autoregressive decode: generate up to 128 tokens
    let mut gen: i64 = 0
    while gen < 128 {
        let next = transformer_forward(prev_token, pos + gen)
        if next == EOS_TOKEN { gen = 128 }  // stop
        else {
            detokenize(next, out_buf)
            serial_print_str(out_buf)  // stream output
            prev_token = next
            gen = gen + 1
        }
    }
}
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 6.1 | Prefill loop (process all prompt tokens) | KV cache populated correctly | 2h |
| 6.2 | Autoregressive decode loop (generate N tokens) | Generates coherent text | 2h |
| 6.3 | Streaming output (token-by-token to serial) | Text appears progressively | 0.5h |
| 6.4 | EOS detection + max length limit | Stops at EOS or 128 tokens | 0.5h |
| 6.5 | Shell command `ask <prompt>` | Full question-answer interaction | 1h |
| 6.6 | Shell command `gen <prompt> [max_tokens]` | Open-ended generation | 0.5h |
| 6.7 | Performance measurement (tokens/sec) | Prints speed after generation | 0.5h |

**Gate:** `nova> ask "What is 2+2?"` → generates a reasonable answer.

**Deliverable:** Updated `shell/commands.fj` with `ask` and `gen` commands.

---

## Phase 7: Smart Kernel Scheduler (ML-Driven)

**Goal:** Replace round-robin scheduler with attention-based predictor.
**Duration:** 1 session
**Dependencies:** Phase 1-2 (does NOT need full LLM)

### Why

This is the second use case: use FajarQuant's fused attention to predict
which process needs CPU next, based on process behavior history.

### Design

```
Process History = sequence of (cpu_ticks, io_waits, mem_allocs) per tick
    → Stored as quantized KV cache (FajarQuant hierarchical)
    → Query = current system state
    → Attention scores = per-process priority
    → Schedule = argmax(scores)

Model: Tiny 2-layer attention (d=16, 1 head)
    → Weights pre-trained offline on scheduler traces
    → Pre-baked into kernel (< 1 KB weights)
    → No model loading needed
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 7.1 | Process history buffer (ring, 64 entries per PID) | Records cpu/io/mem stats per tick | 1h |
| 7.2 | Quantize process history into KV cache | 2-bit quantized, hierarchical tiers | 1h |
| 7.3 | Tiny attention model (d=16, pre-trained weights) | Forward pass runs in < 1ms | 2h |
| 7.4 | Scheduler integration: attention score -> priority | Process with highest score gets CPU | 2h |
| 7.5 | Shell command `sched-mode ml` / `sched-mode rr` | Toggle between ML and round-robin | 0.5h |
| 7.6 | Benchmark: ML scheduler vs round-robin | Measure context switch latency | 1h |

**Gate:** `nova> sched-mode ml` activates ML scheduler, processes still run.

**Deliverable:** `kernel/sched/ml_scheduler.fj` (~300 LOC)

---

## Phase 8: Edge AI Pipeline (Sensor -> ML -> Actuator)

**Goal:** End-to-end demo: read sensor data, run inference, output action.
**Duration:** 1 session
**Dependencies:** Phase 5 or Phase 7

### Why

This is the third use case: FajarOS as an edge AI device OS for drones,
robots, and IoT. The entire pipeline runs in one binary.

### Design

```
@kernel fn read_sensor() -> [i64; 8] {
    // Read from I/O port or shared memory
    // Returns 8-element feature vector
}

@device fn classify(features: [i64; 8]) -> i64 {
    // 2-layer classifier (reuse inference.fj model)
    // Returns action class (0=idle, 1=forward, 2=turn, 3=stop)
}

@kernel fn actuate(action: i64) {
    // Write to I/O port or shared memory
    port_outb(MOTOR_PORT, action)
}

@safe fn pipeline() {
    let raw = read_sensor()
    let action = classify(raw)
    actuate(action)
}
```

### Tasks

| # | Task | Verification | Est. |
|---|------|-------------|------|
| 8.1 | Simulated sensor (reads from VirtIO or shared mem) | Returns 8 i64 values | 1h |
| 8.2 | Simulated actuator (writes to serial) | Prints "ACTION: forward/turn/stop" | 0.5h |
| 8.3 | Pipeline loop (100Hz tick) | Runs sensor->classify->actuate continuously | 1h |
| 8.4 | Shell command `pipeline start` / `pipeline stop` | Starts/stops the loop | 0.5h |
| 8.5 | Shell command `pipeline stats` | Shows inference rate (Hz) + action distribution | 0.5h |
| 8.6 | Context safety test: @safe calls @kernel+@device | Compiler rejects invalid cross-context calls | 1h |

**Gate:** `nova> pipeline start` runs at 100Hz, prints actions.

**Deliverable:** `kernel/compute/pipeline.fj` (~200 LOC)

---

## Model Selection Analysis

| Model | Params | 2-bit Size | KV Cache (4K ctx) | Fits in RAM? | Quality |
|-------|--------|-----------|-------------------|-------------|---------|
| **SmolLM-135M** | 135M | 34 MB | 2 MB (quantized) | Yes (512MB) | Basic Q&A |
| TinyLlama-1.1B | 1.1B | 275 MB | 16 MB (quantized) | Tight | Good Q&A |
| Phi-2 2.7B | 2.7B | 675 MB | 40 MB (quantized) | No (512MB) | Excellent |
| SmolLM-360M | 360M | 90 MB | 5 MB (quantized) | Yes (512MB) | Decent Q&A |

**Recommendation:** Start with **SmolLM-135M** (34 MB, fits easily).
Upgrade to SmolLM-360M later if quality insufficient.

### SmolLM-135M Architecture

```
Layers:     12 transformer blocks
d_model:    768
n_heads:    12
d_head:     64
FFN dim:    3072 (4x)
Vocab:      49,152
Context:    2048 tokens
```

### Memory Budget (SmolLM-135M, 2-bit, 512MB QEMU)

```
Component                   Size        Cumulative
─────────────────────────   ─────       ──────────
Kernel ELF (.text+.data)    2 MB        2 MB
Page tables + frame bitmap  1 MB        3 MB
Process table + IPC         1 MB        4 MB
Tensor pools (small+large)  160 KB      ~4 MB
Codebooks + rotation        32 KB       ~4 MB
Token table                 768 KB      ~5 MB
Embedding table (2-bit)     6 MB        11 MB
12 layer weights (2-bit)    24 MB       35 MB
LM head (2-bit)             4 MB        39 MB
KV cache (2-bit, 2K ctx)    2 MB        41 MB
Shell + VGA + drivers       2 MB        43 MB
Free                        469 MB      512 MB
                            ──────
Utilization:                8.4% of RAM
```

---

## Execution Order & Dependencies

```
Phase 1 ─── bare-metal FajarQuant ──┬──→ Phase 2 ─── tensor extension
                                    │        │
                                    │        ├──→ Phase 3 ─── model loader
                                    │        │        │
                                    │        │        ├──→ Phase 4 ─── tokenizer
                                    │        │        │        │
                                    │        │        │        └──→ Phase 5 ─── transformer
                                    │        │        │                 │
                                    │        │        │                 └──→ Phase 6 ─── ask command
                                    │        │        │
                                    │        │        └──→ Phase 8 ─── edge pipeline
                                    │        │
                                    └────────┴──→ Phase 7 ─── ML scheduler

Critical path: 1 → 2 → 3 → 4 → 5 → 6 (kernel LLM)
Independent:   1 → 7 (ML scheduler, can start after Phase 1)
Independent:   1 → 2 → 8 (edge pipeline, can start after Phase 2)
```

---

## Success Criteria

| Milestone | Criteria | Demo |
|-----------|---------|------|
| **M1: FajarQuant in kernel** | Phases 1-2 | `nova> quant-bench` shows MSE < TurboQuant |
| **M2: Model loads** | Phase 3 | `nova> model-load smollm.fjm` shows 12 layers |
| **M3: First token** | Phases 4-5 | `nova> infer "hello"` produces valid next token |
| **M4: First conversation** | Phase 6 | `nova> ask "What is 2+2?"` answers correctly |
| **M5: Smart scheduler** | Phase 7 | `nova> sched-mode ml` works under load |
| **M6: Edge AI demo** | Phase 8 | `nova> pipeline start` runs at 100Hz |

---

## What Makes This Unique

No other OS in existence has:

1. **Kernel-native LLM inference** — not a userspace app, not a driver, but inside the kernel
2. **Compile-time safety for ML** — @kernel guarantees no heap in quantization, @device guarantees no raw pointers in tensor ops
3. **Quantized attention in Ring 0** — fused codebook dot product runs at kernel privilege, zero syscall overhead
4. **OS scheduler informed by attention** — the kernel literally uses transformer attention to schedule processes
5. **Single-binary AI OS** — kernel + ML runtime + quantization + model = one ELF, one language, one type system

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|-----------|
| 768-dim matmul too slow in @kernel | LLM unusable | Tiled matmul with AVX2 (LLVM backend has it) |
| SmolLM 2-bit quality too low | Incoherent answers | Upgrade to 3-bit (50 MB) or SmolLM-360M |
| KV cache overflow at long context | Crash | Hierarchical auto-eviction (Phase 1.5) |
| Model loading from NVMe too slow | Long boot | Lazy load (load layers on first `ask`) |
| Fixed-point precision insufficient | Wrong results | Use scaled i64 (x10000) or selective f64 for LayerNorm |

---

*FajarQuant Kernel Integration Plan v1.0*
*Target: FajarOS becomes the world's first OS with kernel-native LLM inference*
*Updated: 2026-04-08*
