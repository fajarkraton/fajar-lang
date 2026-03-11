# Workflow — Fajar Lang v1.1 "Ascension"

> Target: Real hardware acceleration + ecosystem infrastructure
> Timeline: 40 sprints, ~400 tasks across 10 phases
> Baseline: v1.0.0 (3,392 tests, ~194K LOC, 185 files)

---

## 1. Development Philosophy

### Core Principles (v1.1 additions)

1. **HARDWARE-FIRST** — Every feature must target real silicon (Intel, AMD, NVIDIA, Jetson)
2. **MEASURE EVERYTHING** — No performance claim without benchmark proof
3. **DEPLOY OR DELETE** — If it can't run on real hardware, it doesn't ship
4. **USER-FACING** — Registry, website, playground must work for real users
5. **CORRECTNESS > SAFETY > USABILITY > PERFORMANCE** (unchanged from v1.0)

### v1.1 Domain Rules

#### Hardware Rules
- All hardware features MUST have fallback to CPU/software emulation
- CPUID/feature detection MUST be runtime, not compile-time only
- NPU/GPU code MUST handle device-not-present gracefully
- Power management APIs MUST be safe by default (no accidental overclock)

#### Numeric Format Rules
- FP4/FP8/BF16 MUST round-trip through f32 without silent precision loss
- Quantization MUST report accuracy degradation metrics
- Structured sparsity MUST verify 4:2 pattern at compile time

#### Ecosystem Rules
- Registry API MUST use HTTPS + API key authentication
- Playground MUST sandbox execution (Wasm, no filesystem access)
- CI pipelines MUST complete in < 30 minutes
- Binary releases MUST be statically linked where possible

---

## 2. Sprint Cycle (1 week per sprint)

```
+-- PLAN   -> Read V11_PLAN.md, identify sprint tasks
|           -> Check V11_SKILLS.md for implementation patterns
|
+-- DESIGN -> Define public API (structs, traits, fn signatures)
|           -> Identify hardware dependencies and fallbacks
|
+-- TEST   -> Write tests BEFORE implementation (RED phase)
|           -> Include hardware mock tests for CI (no real device needed)
|
+-- IMPL   -> Write MINIMAL code to pass tests (GREEN phase)
|           -> Hardware-specific code behind feature flags
|
+-- VERIFY -> cargo test && cargo clippy -- -D warnings && cargo fmt
|           -> Run on target hardware if available (Jetson, GPU)
|
+-- UPDATE -> Mark task [x] in V11_TASKS.md
|           -> Update V11_SKILLS.md with new patterns
```

---

## 3. Quality Gates

### 3.1 Per-Task Gate
- [ ] All tests pass (`cargo test`)
- [ ] No `.unwrap()` in `src/` (only in tests)
- [ ] All `pub` items documented
- [ ] Clippy clean (`cargo clippy -- -D warnings`)
- [ ] Formatted (`cargo fmt -- --check`)
- [ ] Hardware fallback tested (if applicable)
- [ ] New function has at least 1 test

### 3.2 Per-Sprint Gate
- [ ] No regressions from previous sprint
- [ ] Benchmarks compared (if performance-related)
- [ ] At least 1 new example or test program
- [ ] Hardware mock tests pass in CI
- [ ] Documentation updated for new APIs

### 3.3 Per-Phase Gate
- [ ] All sprints in phase complete
- [ ] Integration tests across sprint boundaries
- [ ] Phase-level demo works end-to-end
- [ ] `cargo doc` compiles without warnings
- [ ] Performance targets met (if applicable)

### 3.4 Release Gate (v1.1.0)
- [ ] All 400 tasks complete
- [ ] 4,000+ tests (0 failures)
- [ ] Clippy zero warnings
- [ ] All 10 phases verified
- [ ] fajarlang.dev live
- [ ] Package registry operational
- [ ] At least 3 real-hardware demos
- [ ] CHANGELOG.md updated
- [ ] GitHub release created

---

## 4. Session Protocol (Claude Code)

Every Claude Code session:

1. **READ** → `CLAUDE.md` (auto-loaded)
2. **READ** → `docs/V11_PLAN.md` + `docs/V11_TASKS.md` (current sprint)
3. **READ** → `docs/V11_SKILLS.md` (implementation patterns)
4. **ORIENT** → Identify next incomplete task
5. **ACT** → Implement per TDD workflow
6. **VERIFY** → `cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`
7. **UPDATE** → Mark task `[x]` in `V11_TASKS.md`

---

## 5. Branching Strategy

```
main                <- stable (v1.0.0 tag)
develop/v1.1        <- integration branch for v1.1
feat/hw-detect      <- Phase 1 features
feat/fp8-bf16       <- Phase 2 numeric formats
feat/npu            <- Phase 3 NPU integration
feat/jetson-thor    <- Phase 4 Jetson Thor BSP
feat/amx-avx10      <- Phase 5 advanced codegen
feat/cicd           <- Phase 6 CI/CD
feat/registry       <- Phase 7 package registry
feat/playground     <- Phase 8 online playground
feat/multi-accel    <- Phase 9 multi-accelerator
feat/demos          <- Phase 10 real-world demos
release/v1.1        <- release preparation
```

---

## 6. Hardware-Specific Development Rules

### 6.1 Feature Flags
```toml
[features]
npu = ["openvino-sys", "xdna-sys"]
jetson = ["cuda-13", "jetpack-7"]
amx = []  # CPU feature, runtime detected
avx512 = []  # CPU feature, runtime detected
blackwell = ["cuda-13"]
playground = ["wasm-bindgen", "web-sys"]
```

### 6.2 Hardware Abstraction Pattern
```rust
// All hardware backends implement a common trait
pub trait Accelerator {
    fn name(&self) -> &str;
    fn tops(&self) -> f64;
    fn supports_fp4(&self) -> bool;
    fn supports_fp8(&self) -> bool;
    fn infer(&self, model: &Model, input: &Tensor) -> Result<Tensor, AccelError>;
}

// Runtime dispatch
pub fn select_accelerator(profile: &HardwareProfile) -> Box<dyn Accelerator> {
    if profile.has_npu() { Box::new(NpuAccelerator::new()) }
    else if profile.has_gpu() { Box::new(GpuAccelerator::new()) }
    else { Box::new(CpuAccelerator::new()) }
}
```

### 6.3 Mock Testing Pattern
```rust
// Tests run without real hardware
#[cfg(test)]
mod tests {
    use super::*;

    fn mock_hardware_profile() -> HardwareProfile {
        HardwareProfile {
            cpu: CpuInfo { avx512: true, amx: false, ..Default::default() },
            gpu: None,
            npu: None,
        }
    }

    #[test]
    fn fallback_to_cpu_when_no_gpu() {
        let profile = mock_hardware_profile();
        let accel = select_accelerator(&profile);
        assert_eq!(accel.name(), "cpu");
    }
}
```

---

## 7. Numeric Format Development Rules

### 7.1 Precision Testing
- Every FP4/FP8/BF16 operation MUST have a round-trip test through f32
- Quantization tests MUST measure and assert maximum error bounds
- Structured sparsity MUST verify pattern validity before storage

### 7.2 Format Conversion Chain
```
f32 ──→ BF16 ──→ FP8 (E5M2) ──→ FP4 (E2M1)
 ↑                                      │
 └──────────── dequantize ──────────────┘
```

---

## 8. Ecosystem Development Rules

### 8.1 Registry Security
- All packages signed with ed25519
- API keys stored as argon2 hashes
- Rate limiting on all endpoints
- SBOM (CycloneDX) required for publish

### 8.2 Playground Sandbox
- Wasm execution only (no native)
- 5-second timeout per execution
- 16MB memory limit
- No filesystem, no network, no FFI

### 8.3 Website
- Static site (no server-side rendering)
- < 3 second load time on 3G
- Accessible (WCAG 2.1 AA)
- Mobile-responsive

---

## 9. Commit Convention (v1.1)

```
Format: <type>(<scope>): <description>

Types: feat, fix, test, refactor, docs, perf, ci, chore
Scope: hw, fp8, npu, jetson, amx, cicd, registry, playground, dispatch, demo

Examples:
  feat(hw): add CPUID-based AVX-512 detection
  feat(fp8): implement E5M2 tensor arithmetic
  feat(npu): Intel OpenVINO inference backend
  feat(jetson): Jetson Thor T5000 BSP integration
  feat(amx): Intel AMX tile matrix multiply
  ci(cicd): add multi-platform GitHub Actions matrix
  feat(registry): Cloudflare Workers API endpoint
  feat(playground): Monaco editor with .fj syntax
  feat(dispatch): CPU→NPU→GPU fallback chain
  docs(demo): drone firmware tutorial
```

---

## 10. Performance Targets (v1.1)

| Benchmark | v1.0 (baseline) | v1.1 (target) |
|-----------|-----------------|---------------|
| FP8 matmul 1024×1024 | N/A (new) | < 1ms (GPU) |
| FP4 inference (ResNet-50) | N/A (new) | < 5ms (Jetson Thor) |
| NPU ONNX inference | N/A (new) | < 10ms (50 TOPS) |
| INT8 quantized MNIST | ~50ms (CPU) | < 1ms (NPU) |
| AVX-512 dot product | ~10μs | < 1μs |
| AMX matrix multiply 16×16 | N/A (new) | < 500ns |
| Playground compile+run | N/A (new) | < 2s (Wasm) |
| Registry search | N/A (new) | < 200ms |
| Website load (3G) | N/A (new) | < 3s |
| CI pipeline total | N/A (new) | < 30min |

---

*V11_WORKFLOW.md v1.0 | Created 2026-03-11*
