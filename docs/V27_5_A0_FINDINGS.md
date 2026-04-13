# V27.5 Phase 0 — Pre-Flight Audit Findings

**Date:** 2026-04-14
**Method:** Runnable commands, hands-on verification

## Results

| # | Check | Expected | Actual | Match? |
|---|-------|----------|--------|--------|
| A0.1 | Lib tests | 7,611+ | **7,611 pass, 0 fail** | YES |
| A0.2 | Integ test count | 2,553+ | **2,575** | YES (+22 from V27 feature flag tests) |
| A0.3 | Cargo version | 27.0.0 | **27.0.0** | YES |
| A0.4 | Interrupt codegen LOC | baseline | **75 lines** | BASELINE SET |
| A0.5 | @host/@app tokens | 0 | **0** | YES (confirms gap) |
| A0.6 | Refinement interpreter checks | ~2 | **2 lines** | YES (only let-bind) |
| A0.7 | MAX_KERNEL_TENSOR_DIM | 16 | **16** (ai_kernel.rs:84) | YES (confirms need) |
| A0.8 | Self-hosting LOC | ~15,880 | **15,880** | YES |
| A0.9 | Wrapper non-test calls | 0 | **0** (definition only at linker.rs:3830) | YES |
| A0.10 | Framebuffer builtins | exists | **builtins.rs:2943** (fb_init, fb_write_pixel, fb_fill_rect) | YES |
| A0.11 | Cap<T> in analyzer | 0 | **0** | YES (confirms gap) |
| A0.12 | Clippy + fmt | green | **0 warnings, exit 0** | YES |

## Surprises

1. **A0.9:** grep initially showed 3 results but 2 are in `#[cfg(test)]` blocks. Real non-test calls = 0. Confirmed.
2. **A0.10:** `fb_fill_rect` already exists as a builtin stub (returns 0). Only needs MMIO wiring, not full implementation.
3. **A0.2:** Integration tests increased to 2,575 (+22 from V27 feature flag tests added earlier this session).

## Gate

A0 complete. All baselines verified. P1-P5 unblocked.
