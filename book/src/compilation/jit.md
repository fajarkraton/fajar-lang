# Tiered JIT Compilation

Fajar Lang uses a tiered JIT compilation strategy that balances startup speed with peak performance.

## Tiers

```
Interpreter → Baseline JIT → Optimizing JIT
    (cold)      (warm)          (hot)
```

| Tier | Compile Time | Code Quality | When |
|------|-------------|-------------|------|
| Interpreter | None | Slowest | First call |
| Baseline JIT | <1ms | Good | After 100 calls |
| Optimizing JIT | ~10ms | Excellent | After 1000 calls with profile |

## How It Works

1. **All functions start in the interpreter** — zero startup cost
2. **Execution counters** track how often each function is called
3. **Hot threshold** (default: 100) triggers baseline JIT compilation
4. **Profile collection** records branch frequencies and type distributions
5. **Optimizing threshold** (default: 1000) triggers optimizing JIT with profile data

```fajar
fn fibonacci(n: i64) -> i64 {
    // Call 1-99: interpreted
    // Call 100: baseline JIT compiles this function (<1ms)
    // Call 1000: optimizing JIT recompiles with inlining + constant propagation
    if n <= 1 { n }
    else { fibonacci(n - 1) + fibonacci(n - 2) }
}
```

## On-Stack Replacement (OSR)

Long-running loops are promoted mid-execution without restarting:

```fajar
fn sum_million() -> i64 {
    let mut total = 0
    let mut i = 0
    while i < 1_000_000 {
        // After ~100 loop iterations, OSR kicks in:
        // the loop is JIT-compiled and execution continues
        // from the current iteration in native code
        total = total + i
        i = i + 1
    }
    total
}
```

OSR transfers the current stack state (local variables, loop counters) from the interpreter to compiled code.

## Deoptimization

If an optimistic assumption proves wrong, the optimizing JIT can bail out:

```fajar
fn dispatch(obj: dyn Animal) {
    // Optimizing JIT assumes obj is always Dog (monomorphic)
    // If a Cat appears, deoptimize back to baseline
    obj.speak()
}
```

Deoptimization preserves correctness while allowing aggressive optimizations for common cases.

## Profile-Guided Optimization

The JIT collects:
- **Branch profiles** — which `if` branches are taken most often
- **Type profiles** — which concrete types appear at dynamic dispatch sites
- **Call frequency** — which functions are called most often

This data guides inlining decisions, branch prediction, and specialization.

## Configuration

```bash
fj run --jit-threshold 50 program.fj    # lower threshold (faster JIT)
fj run --no-jit program.fj              # interpreter only
```
