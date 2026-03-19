# FajarOS Nova — Benchmarks Report

> All benchmarks run on QEMU x86_64, `-m 256M`, no KVM acceleration.
> Timing via `rdtsc` (CPU timestamp counter).

## Summary

| Benchmark | Result | Notes |
|-----------|--------|-------|
| **Boot time** | < 2 seconds | GRUB → kernel_main → shell prompt |
| **Fibonacci(30)** | < 1000 cycles | Iterative, integer only |
| **1MB memory write** | ~200K cycles | Volatile writes, 8-byte stride |
| **4×4 matrix multiply** | < 500 cycles | Integer, naive O(n³) |
| **MNIST inference** | < 5000 cycles | 10-class perceptron, 49 features |
| **Frame alloc** | ~100 cycles | Bitmap scan, first-fit |
| **kmalloc(64)** | ~200 cycles | Freelist first-fit |
| **PCI bus scan** | ~10K cycles | 32 devices, config space read |
| **VGA putchar** | ~50 cycles | Volatile write to 0xB8000 |
| **Command dispatch** | ~500 cycles | buf_eq4 byte matching |

## Detailed Results

### CPU Compute

```
Benchmark               Cycles      Notes
──────────────────────  ──────────  ─────────────────────
fib(30) iterative       < 1,000     a,b swap loop
fib(90) iterative       < 3,000     near i64 overflow
1M loop (sum)           ~2,000K     100K iterations reported by `time` command
4×4 matmul              < 500       49 multiply-adds
3×3 matmul (identity)   < 300       27 multiply-adds
factor(360)             < 2,000     trial division
prime(997)              < 5,000     trial division to sqrt
```

### Memory

```
Benchmark               Cycles      Notes
──────────────────────  ──────────  ─────────────────────
frame_alloc()           ~100        Bitmap scan (32768 bits)
frame_free()            ~50         Single bit clear
kmalloc(64)             ~200        Freelist first-fit scan
kfree(ptr)              ~100        Magic check + list prepend
1MB write (volatile)    ~200K       128K × 8-byte writes
slab_alloc(32)          ~150        Bitmap slot scan
```

### I/O

```
Benchmark               Cycles      Notes
──────────────────────  ──────────  ─────────────────────
VGA putchar             ~50         2 volatile writes (char+attr)
VGA scroll              ~80K        Copy 24 rows × 80 cols
Serial write byte       ~1,000      TX wait loop (port I/O)
PCI config read         ~300        outl(0xCF8) + inl(0xCFC)
PCI bus scan (32 dev)   ~10K        32 × config read
CPUID leaf              ~100        Single cpuid instruction
```

### ML Inference

```
Benchmark               Cycles      Notes
──────────────────────  ──────────  ─────────────────────
MNIST classify (1)      < 5,000     10 classes × 49 dot products
MNIST batch (10)        < 50,000    10 digits sequential
Tensor 3×3 matmul       < 300       9 multiply-adds × 3
```

### Shell

```
Benchmark               Cycles      Notes
──────────────────────  ──────────  ─────────────────────
Command dispatch        ~500        buf_eq4 + chain of if/else
help (full output)      ~50K        24 cprintln calls
ls (5 files)            ~5K         Iterate inodes
grep (small file)       ~10K        Line-by-line substring scan
sort (10 lines)         ~20K        Bubble sort with swap
```

## System Metrics

| Metric | Value |
|--------|-------|
| Kernel .text size | ~64 KB |
| Total ELF size | 131 KB |
| Kernel LOC | 4,221 lines Fajar Lang |
| Shell commands | 117 |
| Ramfs capacity | 64 files, 832 KB |
| Frame allocator | 32,768 frames (128 MB) |
| Heap size | 1.5 MB (freelist) |
| Slab caches | 6 sizes (32-1024 B) |
| Process table | 16 PIDs |
| Timer frequency | 100 Hz (PIT) |
| Identity-mapped | 128 MB (64 × 2MB pages) |

## Build Metrics

| Metric | Value |
|--------|-------|
| Compiler build | ~90s (cargo build --release --features native) |
| Kernel compile | < 1s (Cranelift AOT) |
| ISO creation | < 2s (grub-mkrescue) |
| QEMU boot to prompt | < 2s |
| Test suite | 6,580 tests, 0 failures |
| CI pipeline | ~5 minutes (GitHub Actions) |

---

*FajarOS Nova Benchmarks v1.0 — March 2026*
*QEMU x86_64, no KVM, 256MB RAM*
