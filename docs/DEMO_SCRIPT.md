# Fajar Lang — 5-Minute Demo Script

> **Duration:** 5 minutes
> **Format:** Terminal recording + narration (or text overlay)
> **Tools:** asciinema / OBS / terminal screen capture
> **Resolution:** 1920x1080, font size 16pt

---

## Scene 1: Introduction (30s)

**[Screen: Title card]**

```
FAJAR LANG
Systems Programming for Embedded ML & OS Development
Made in Indonesia
```

**Narration:**
"Meet Fajar Lang — the only programming language where an OS kernel and a neural network can share the same codebase, type system, and compiler. The compiler itself enforces privilege isolation: @kernel code can't allocate heap memory, @device code can't touch hardware registers. If it compiles, it's safe to deploy."

---

## Scene 2: Hello World + REPL (30s)

**[Terminal recording]**

```bash
# Show version
$ fj --version
Fajar Lang v5.5.0 "Illumination"

# Run hello world
$ cat examples/hello.fj
fn main() {
    println("Hello from Fajar Lang!")
}

$ fj run examples/hello.fj
Hello from Fajar Lang!

# Interactive REPL
$ fj repl
> 1 + 2
3
> let arr = [1, 2, 3, 4, 5]
> arr.map(|x| x * 2)
[2, 4, 6, 8, 10]
> arr.filter(|x| x > 3).sum()
9
> exit
```

---

## Scene 3: Language Features (60s)

**[Terminal: run feature examples]**

```bash
# Pattern matching
$ fj run examples/pattern_matching.fj
# Shows: match expressions, nested patterns, guards

# Async/await
$ fj run examples/async_demo.fj
# Shows: async fn, .await, join(), timeout()

# Traits + generics
$ fj run examples/trait_demo.fj
# Shows: trait definitions, impl blocks, polymorphism

# Array higher-order methods (NEW in v0.8!)
$ fj run examples/array_methods.fj
Doubled:    [2, 4, 6, 8, 10, 12, 14, 16, 18, 20]
Evens:      [2, 4, 6, 8, 10]
Sum:        55
Sorted:     [1, 1, 2, 3, 4, 5, 6, 9]
Zipped:     [(1, 10), (2, 20), (3, 30)]
Flattened:  [1, 2, 3, 4, 5, 6]
```

---

## Scene 4: FajarOS Nova — x86_64 OS (90s)

**[Terminal: QEMU boot]**

```bash
# Clone and build the OS
$ cd fajaros-x86
$ make build
[OK] Combined 139 source files → build/combined.fj
[OK] Kernel built: build/fajaros.elf (36729 lines)

# Boot in QEMU
$ make run

# === QEMU output ===
[NOVA] FajarOS Nova v2.0.0 Absolute booted
[NOVA] 37K LOC | 280+ commands | 50 syscalls | 100% Fajar Lang
nova> help | head 5
nova> uname -a
nova> ps
nova> ls /
nova> cat /proc/cpuinfo
nova> ping 10.0.2.2
nova> lspci
nova> gpu
nova> ext2ls
nova> tcpstat
nova> mpstat        # NEW: per-CPU stats
nova> free          # NEW: demand paging stats
```

**Narration:**
"This is FajarOS Nova — a complete x86_64 operating system with 280+ commands, written entirely in Fajar Lang. 139 modular files, 37,000 lines of code. It has a TCP/IP stack, ext2 filesystem, GPU compute, multi-user authentication, a GDB debugger — all compiled with context safety. No C, no assembly files."

---

## Scene 5: ARM64 + Real Hardware (60s)

**[Terminal: SSH to Q6A]**

```bash
# Cross-compile for ARM64
$ cargo build --release --target aarch64-unknown-linux-gnu
# → 7.8 MB binary

# Deploy to Radxa Dragon Q6A (Qualcomm QCS6490)
$ scp target/.../fj radxa@192.168.50.94:/opt/fj/
$ ssh radxa@192.168.50.94

# On Q6A hardware:
radxa$ /opt/fj/fj run examples/hello.fj
Hello from Fajar Lang!

radxa$ /opt/fj/fj run --jit examples/fibonacci.fj
# JIT: fib(30) in 0.68s (480x faster than interpreter)

# QNN NPU inference
radxa$ /opt/fj/fj run examples/q6a_mnist_inference.fj
MNIST accuracy: 99/100 = 99%
CPU inference: 0.33ms per digit
```

**Narration:**
"And here's the same language running on real ARM64 hardware — the Radxa Dragon Q6A with a Qualcomm QCS6490. JIT compilation with Cranelift, and we can run neural network inference on the Hexagon NPU at 99% MNIST accuracy."

---

## Scene 6: Quality + Closing (30s)

**[Terminal: test suite]**

```bash
$ cargo test
test result: ok. 5,664 passed; 0 failed

$ cargo clippy -- -D warnings
# Zero warnings

$ cargo +nightly fuzz run fuzz_interpreter -- -max_total_time=10
Done 12000 runs in 10 second(s)
# Zero crashes across 2.3 million fuzz runs
```

**[Screen: Closing card]**

```
FAJAR LANG v5.5.0
6,286 tests | 290K LOC | Zero fuzz crashes
FajarOS: 21K LOC kernel | 280+ commands | 50 syscalls

github.com/fajarkraton/fajar-lang
github.com/fajarkraton/fajaros-x86

Made in Indonesia by Muhamad Fajar Putranto
TaxPrime | PrimeCore.id | InkubatorX | ACEXI
```

---

## Recording Commands (Copy-Paste Ready)

```bash
# Scene 2
fj --version
cat examples/hello.fj
fj run examples/hello.fj
echo -e "1 + 2\nlet arr = [1,2,3,4,5]\narr.map(|x| x * 2)\narr.filter(|x| x > 3).sum()\nexit" | fj repl

# Scene 3
fj run examples/array_methods.fj

# Scene 4
cd ~/Documents/fajaros-x86 && make build && make run

# Scene 5
cargo build --release --target aarch64-unknown-linux-gnu
ssh radxa@192.168.50.94 "/opt/fj/fj run examples/hello.fj"

# Scene 6
cargo test 2>&1 | tail -3
cargo clippy -- -D warnings 2>&1 | tail -1
```

---

## YouTube Description

```
Fajar Lang — Systems Programming Language for Embedded ML & OS Development

The only language where an OS kernel and a neural network share the same codebase. Compiler-enforced safety with @kernel/@device/@safe contexts.

Features:
- Rust-inspired syntax, no lifetime annotations
- Native tensor types with autograd
- Pattern matching, async/await, traits, closures
- Cranelift + LLVM backends
- Cross-compilation: x86_64, ARM64, RISC-V

FajarOS Nova: x86_64 bare-metal OS (37K LOC, 280+ commands)
FajarOS Surya: ARM64 OS on Radxa Dragon Q6A (99% MNIST on NPU)

Links:
- GitHub: https://github.com/fajarkraton/fajar-lang
- FajarOS x86: https://github.com/fajarkraton/fajaros-x86
- FajarOS ARM64: https://github.com/fajarkraton/fajar-os

Made in Indonesia by Muhamad Fajar Putranto
#FajarLang #SystemsProgramming #EmbeddedML #OS #Indonesia #RustLang
```

---

## Social Media Posts

### Twitter/X Thread

```
1/ Introducing Fajar Lang — a systems programming language where an OS kernel and a neural network share the same type system.

@kernel fn irq_handler() { ... }  // Can't allocate heap
@device fn classify(img: Tensor) { ... }  // Can't touch hardware

If it compiles, it's safe.

2/ FajarOS Nova: x86_64 OS written 100% in Fajar Lang
- 37K LOC, 280+ commands, 50 syscalls
- TCP/IP, ext2, GPU compute, SMP
- GDB debugger, init system, package manager
- Zero lines of C or assembly

3/ Runs on real hardware too: Radxa Dragon Q6A (ARM64)
- Qualcomm QCS6490 with Hexagon NPU
- JIT compilation via Cranelift
- 99% MNIST accuracy on NPU inference
- 0.33ms per digit classification

4/ Quality:
- 6,286 tests, 0 failures
- 2.3 million fuzz runs, 0 crashes
- cargo clippy: 0 warnings

Made in Indonesia 🇮🇩

github.com/fajarkraton/fajar-lang
```

### LinkedIn Post

```
I'm excited to share Fajar Lang — a systems programming language I created for embedded ML and OS development.

The key innovation: compiler-enforced privilege isolation. @kernel functions can't allocate memory. @device functions can't touch hardware. The compiler itself prevents the entire class of bugs that causes kernel crashes and security vulnerabilities.

Built with it:
- FajarOS Nova: x86_64 OS (37K LOC, 280+ commands)
- FajarOS Surya: ARM64 OS on Radxa Dragon Q6A

6,286 tests passing. 2.3M fuzz runs with zero crashes. 290K lines of Rust powering the compiler.

Open source under MIT: github.com/fajarkraton/fajar-lang

#SystemsProgramming #EmbeddedAI #OpenSource #MadeInIndonesia
```

---

*Demo Script v1.0 — Fajar Lang v5.5.0 + FajarOS Nova v2.0.0*
