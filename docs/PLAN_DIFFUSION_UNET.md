# Plan: Real Diffusion UNet — Sprint 3

> **Goal:** `diffusion_denoise()` runs a real UNet forward pass, not just scaling
> **Module:** ml_advanced/diffusion [sim] → [x]
> **Estimated LOC:** ~690
> **Risk:** MEDIUM-HIGH

---

## Architecture (Minimal UNet for 8x8 images)

```
INPUT: [batch, 1, 8, 8]
  |
  timestep t → sinusoidal_embed(t, 32) → Dense(32,32) → SiLU → t_emb [batch, 32]
  |
  ENCODER
  ├── Conv2d(1→16, k=3, s=1, p=1) → GroupNorm(4,16) → SiLU → +t_proj → h1 [B,16,8,8] ← skip_1
  └── Conv2d(16→32, k=3, s=2, p=1) → GroupNorm(4,32) → SiLU → +t_proj → h2 [B,32,4,4] ← skip_2
  |
  BOTTLENECK
  └── Conv2d(32→32, k=3, s=1, p=1) → GroupNorm(4,32) → SiLU → +t_proj → mid [B,32,4,4]
  |
  DECODER
  ├── concat(mid, skip_2) [B,64,4,4] → Conv2d(64→32) → GroupNorm → SiLU → +t_proj → d2
  ├── upsample_2x(d2) [B,32,8,8]
  └── concat(d2_up, skip_1) [B,48,8,8] → Conv2d(48→16) → GroupNorm → SiLU → +t_proj → d1
  |
  OUTPUT: Conv2d(16→1, k=1) → noise_pred [batch, 1, 8, 8]
```

**~45,000 parameters** (1000x smaller than real DDPM — intentionally CPU-friendly)

---

## New Primitives Required

| Component | File | LOC | Description |
|-----------|------|-----|-------------|
| GroupNorm | layers.rs | ~120 | Channel-group normalization (standard for UNet) |
| SiLU tracked | ops.rs | ~30 | x·sigmoid(x) with backward |
| Upsample 2x tracked | ops.rs | ~60 | Nearest-neighbor 2x upscale with backward |
| concat_tracked | ops.rs | ~40 | Concat along axis with gradient split |
| Dense::forward_tracked | layers.rs | ~15 | Compose matmul_tracked + add_tracked |

---

## DiffusionUNet Struct

```rust
pub struct DiffusionUNet {
    // Timestep embedding: Dense(32→32) + 5 projections to each block
    time_mlp_1: Dense,
    time_proj_{1,2,m,3,4}: Dense,    // project t_emb to each block's channel count

    // Encoder: 2 levels
    enc_conv1: Conv2d, enc_norm1: GroupNorm,   // 1→16, 8x8
    enc_conv2: Conv2d, enc_norm2: GroupNorm,   // 16→32, stride-2 → 4x4

    // Bottleneck
    mid_conv: Conv2d, mid_norm: GroupNorm,     // 32→32, 4x4

    // Decoder: 2 levels + skip concat
    dec_conv2: Conv2d, dec_norm2: GroupNorm,   // 64→32 (after skip concat)
    dec_conv1: Conv2d, dec_norm1: GroupNorm,   // 48→16 (after skip concat)

    // Output
    out_conv: Conv2d,                           // 16→1, k=1
}
```

Methods: `new()`, `forward()`, `forward_tracked()`, `parameters()`, `parameters_mut()`, `param_count()`

---

## Training Loop

```
schedule = cosine_schedule(100)    // 100 diffusion steps
model = DiffusionUNet::new(1, 32)
optimizer = Adam(lr=1e-3)

for epoch in 0..200:
    for batch in data.chunks(4):   // batch of 4 8x8 images
        t = random_timestep(0..100)
        noise = randn(batch.shape)
        x_noisy = sqrt(α_t)·x + sqrt(1-α_t)·noise

        tape = Tape::new()
        noise_pred = model.forward_tracked(x_noisy, t, tape)
        loss = mse_loss_tracked(noise_pred, noise, tape)

        grads = tape.backward(loss)
        distribute_grads(model, grads)
        optimizer.step(model.parameters_mut())
        model.zero_grad()
```

**Training data:** Simple 8x8 Gaussian blobs (synthetic)
**Success criteria:** Loss decreases; denoised samples resemble training data

---

## Sampling (Inference)

```
x = randn([1, 1, 8, 8])           // pure noise
for t in (T-1)..0:
    noise_pred = model.forward(x, t)   // untracked
    x = ddpm_reverse_step(x, noise_pred, schedule, t)
    if t > 0: x += sqrt(β_t) · randn(x.shape)
return x                              // denoised sample
```

---

## Files to Create/Modify

| File | Action | LOC |
|------|--------|-----|
| `src/runtime/ml/layers.rs` | Add GroupNorm + Dense::forward_tracked | ~135 |
| `src/runtime/ml/ops.rs` | Add silu_tracked, upsample_2x_tracked, concat_tracked | ~130 |
| `src/ml_advanced/diffusion_unet.rs` | NEW: DiffusionUNet + training + sampling | ~350 |
| `src/ml_advanced/mod.rs` | Register module | ~1 |
| `src/ml_advanced/diffusion.rs` | Add TensorValue-based add_noise | ~20 |
| `src/interpreter/eval/builtins.rs` | Replace [sim] builtins with real UNet | ~40 |
| Tests | Unit + integration | ~100 |
| **Total** | | **~690** |

---

## Implementation Phases

**Phase 1 — Primitives (no deps between these):**
1. GroupNorm forward + forward_tracked
2. silu / silu_tracked
3. upsample_nearest_2x / tracked
4. concat_tracked
5. Dense::forward_tracked

**Phase 2 — UNet (depends on Phase 1):**
6. DiffusionUNet struct + new()
7. forward() (untracked, for inference)
8. forward_tracked() (for training)
9. parameters() / parameters_mut()

**Phase 3 — Training/Sampling (depends on Phase 2):**
10. diffusion_train_step()
11. diffusion_sample() (DDPM)
12. diffusion_sample_ddim() (faster)

**Phase 4 — Integration:**
13. Replace [sim] builtins
14. Remove from SIMULATED_BUILTINS

---

## Risks

| Risk | Mitigation |
|------|-----------|
| Conv2d stride-2 backward less tested | Add dedicated gradient check test |
| Tape ID management across 15 layers | Assign stable IDs in new(), not per-forward |
| GroupNorm backward complexity | Implement standard formula, validate with numerical gradients |
| Broadcasting in t_emb addition | Existing add_tracked handles broadcast via reduce_broadcast |
| Memory (~1.2 MB peak for batch=4) | Fine for CPU |

---

## Test Plan

| Test | Criteria |
|------|----------|
| GroupNorm shape | [4,16,8,8] → same shape, ~0 mean, ~1 var per group |
| SiLU values | silu(0)=0, silu(1)≈0.731 |
| Upsample shape | [1,1,4,4] → [1,1,8,8], each pixel replicated |
| UNet forward shape | [4,1,8,8] → [4,1,8,8] |
| UNet different timesteps | Different t → different outputs |
| UNet backward | All params get non-zero gradients |
| **Overfit single sample** | **200 steps on 1 image → loss < 0.01** |
| **Denoise > random** | **MSE(sample, data) < MSE(noise, data)** |

---

*Shared prerequisite with RL plan: Dense::forward_tracked (5 LOC)*
*Ready for execution in 1-2 sessions*
