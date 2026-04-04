# Honest Status — V20.5 "Hardening"

> **Date:** 2026-04-04
> **Purpose:** Per-builtin status table. No inflated claims.

---

## Labeling System

```
[x]   = PRODUCTION — user runs it, correct results, tested
[sim] = SIMULATED — runs correctly but underlying mechanism is fake
         (e.g., CPU pretends to be GPU, synchronous pretends to be async)
[f]   = FRAMEWORK — code exists, not callable from .fj
[s]   = STUB — near-empty placeholder
```

---

## Module Counts

```
Before V20.5:  48 [x], 0 [sim], 5 [f], 3 [s]  <- INFLATED
After V20.5:   42 [x], 6 [sim], 5 [f], 3 [s]  <- HONEST
After V21:     47 [x], 1 [sim], 5 [f], 3 [s]  <- 5 sim→x upgrades
After V21.1:   48 [x], 0 [sim], 5 [f], 3 [s]  <- const_alloc honest [x]
```

### V21 Upgrades (6 [sim] → 5 [x] + 1 [sim])

| Builtin | V20.5 | V21 | Reason |
|---------|-------|-----|--------|
| actor_spawn/send/supervise | [sim] | **[x]** | Real std::thread + mpsc channels |
| accelerate | [sim] | **[x]** | Real workload classification + GPU detection + CPU dispatch |
| pipeline_run | [sim] | **[x]** | Real sequential composition with error propagation |
| diffusion_create/denoise | [sim] | **[x]** | Real UNet neural network (forward/train/sample) |
| rl_agent_create/step | [sim] | **[x]** | Real DQN + CartPole physics environment |
| const_alloc | [sim] | **[x]** | Creates correct ConstAllocation; .rodata via codegen @section |

---

## Test Counts (V20.5)

```
Lib tests:         8,287 (8,285 pass, 2 pre-existing failures in registry/incremental)
Integration tests: 2,358 (ALL pass, 0 failures)
  - eval_tests:    948
  - v20_builtin:   31 (28 builtin + 3 span tests) <- NEW in V20.5
  - context_safety: 148
  - nova_v2:       138
  - validation:    97
  - safety:        96
  - property:      78
  - effect:        77
  - comptime:      56
  - ... (32 more files)
Total:             ~10,645
```

---

## Production [x] Builtins — Tested, Real Implementation

| Category | Builtins | Tests |
|----------|---------|-------|
| Core I/O | println, print, eprintln, read_file, write_file, file_exists | 100+ |
| Core | len, type_of, assert, assert_eq, panic, todo, dbg | 50+ |
| Array | push, pop, sort, reverse, map, filter, reduce | 15+ |
| HashMap | map_new, map_insert, map_get, map_get_or, map_remove, map_contains_key, map_keys, map_values, map_len | 10+ |
| Tensor creation | zeros, ones, randn, from_data, eye, xavier, arange, linspace | 20+ |
| Tensor ops | matmul, transpose, reshape, flatten, squeeze, concat, split | 15+ |
| Activations | relu, sigmoid, tanh, softmax, gelu, leaky_relu | 10+ |
| Loss | mse_loss, cross_entropy, bce_loss, l1_loss | 8+ |
| Autograd | backward, grad, requires_grad, set_requires_grad | 10+ |
| Layers | Dense, Conv2d, MultiHeadAttention, forward, layer_params | 8+ |
| Optimizers | SGD, Adam, step, zero_grad | 6+ |
| Metrics | accuracy, precision, recall, f1_score | 4+ |
| Quantization | quantize_int8, dequantize_int8, quantized_matmul | 10 |
| Networking | http_get, http_post, tcp_connect, dns_resolve | 4+ |
| FFI | ffi_load_library, ffi_call | 2+ |
| Channels | channel_create, channel_send, channel_recv | 3+ |
| Async | async_sleep, async_spawn, async_join, async_timeout | 4+ |
| Regex | regex_match, regex_find, regex_replace | 3+ |
| Crypto | sha256, aes_encrypt, aes_decrypt | 3+ |
| Reflection | const_type_name, const_field_names | 2+ |
| Macros | macro_rules! | 5+ |
| Const | const_size_of, const_align_of | 4 |
| Map | map_get_or | 2 |

## Simulated [sim] Builtins — NONE REMAINING

All previously-simulated builtins have been upgraded to production [x]:
- V21: actors, accelerate, pipeline, diffusion, rl_agent (5 upgrades)
- V21.1: const_alloc (creates correct ConstAllocation; .rodata via @section codegen)

## Framework [f] Modules — Code Exists, Not Callable from .fj

| Module | Lines | Wire Planned |
|--------|-------|-------------|
| const_* (8 modules) | 4,531 | Future |
| demos/ | 16,257 | Archive candidate |

*Note: rtos/ (8K) and iot/ (5K) were removed in V20.8 dead code cleanup.*

## Stub [s] Modules

| Module | Status |
|--------|--------|
| generators_v12 | Superseded by V18 generators |
| wasi_v12 | Superseded by wasi_p2 |

*Note: stdlib/ (95 LOC) was removed in V20.8 dead code cleanup.*

---

## V20.5 Changes Summary

| What | Before (V20) | After (V20.5) |
|------|-------------|---------------|
| Module count | 48 [x], 0 [sim] | 42 [x], 6 [sim] |
| V20 builtin tests | 0 | 31 (28 builtins + 3 spans) |
| Error-swallowing in accelerate/actor_send | unwrap_or(Null) | Proper ? propagation |
| Runtime error spans | No source location | Binary/Call/Index errors show span |
| 4 crashing tests | SIGABRT | 16MB thread wrapper (pass) |
| pipeline_run errors | Swallowed | Propagated with stage name |
| Simulated builtins | Unlabeled | [sim] prefix + one-time warning |
| Doc example unwrap | .unwrap() | .expect("eval failed") |

---

*V20.5 "Hardening" — honest foundation for FajarQuant*
*"Honest 42 modules is worth more than inflated 48."*
