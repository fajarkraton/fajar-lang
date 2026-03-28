# Conference Talk: Fajar Lang -- One Language for ML and OS

> Speaker guide and outline for presenting Fajar Lang at conferences, meetups, and tech talks.

---

## Talk Metadata

| Field | Value |
|-------|-------|
| **Title** | Fajar Lang: One Language for ML and OS |
| **Subtitle** | Compiler-Enforced Safety for Embedded AI Systems |
| **Duration** | 30 minutes (25 min talk + 5 min Q&A) |
| **Level** | Intermediate (assumes familiarity with systems programming concepts) |
| **Audience** | Systems programmers, embedded engineers, ML engineers, PL researchers |
| **Key takeaway** | A single language can safely unify OS kernel code and ML inference through compiler-enforced context isolation |

---

## Abstract (for CFP submissions)

Building embedded AI systems today requires stitching together C for kernels, Python for training, and Rust or C++ for inference runtimes. Every boundary is a potential bug. Fajar Lang eliminates these boundaries with a statically-typed systems language where `@kernel` code cannot call tensor operations and `@device` code cannot access hardware registers -- enforced at compile time, not by convention. In this talk, we demonstrate how one language, one compiler, and one type system can build everything from bare-metal OS kernels to neural network inference pipelines, with 292,000 lines of production Rust powering the compiler and two full operating systems (x86_64 and ARM64) written entirely in Fajar Lang.

---

## 30-Minute Version

### Section 1: The Problem (5 minutes)

**Goal:** Establish why existing solutions are insufficient.

**Talking Points:**

1. **The multi-language gap.** Embedded AI systems today require at least 3 languages: C/C++ for the kernel, Python for ML training, and a runtime language for inference. Each boundary is a surface for bugs.

2. **Real-world failure modes.** A flight controller that calls `malloc` during an interrupt handler. An inference path that accidentally writes to a hardware register. A type mismatch at the C-Python FFI boundary that silently corrupts data. These bugs are caught at runtime -- if at all.

3. **Convention vs enforcement.** Existing approaches rely on coding guidelines ("don't call heap functions in ISRs") rather than compiler guarantees. Guidelines are forgotten; compilers are not.

4. **The question.** What if a single language could guarantee, at compile time, that OS kernel code and ML inference code cannot interfere with each other?

**Slide 1:** Title slide -- "Fajar Lang: One Language for ML and OS"
**Slide 2:** Diagram of the multi-language embedded AI stack (C + Python + Rust + glue)
**Slide 3:** Three real-world failure modes with severity labels

### Section 2: Live Demo (10 minutes)

**Goal:** Show Fajar Lang in action. The demo is the centerpiece of the talk.

**Demo Script:**

```
DEMO 1: Hello World and Basics (2 min)
─────────────────────────────────────────
$ fj repl
fj> let x: i32 = 42
fj> let name = "Fajar"
fj> println(f"Hello from {name}, x = {x}")
Hello from Fajar, x = 42
fj> fn double(n: i32) -> i32 { n * 2 }
fj> 5 |> double |> double
20

Talking point: Static types with inference, f-strings, pipeline operator.

DEMO 2: ML / Tensor Operations (3 min)
─────────────────────────────────────────
$ fj run examples/tensor_demo.fj

// Show file content:
@device fn classify(input: Tensor) -> Tensor {
    let w1 = randn(784, 128)
    let b1 = zeros(1, 128)
    let h = relu(matmul(input, w1) + b1)
    let w2 = randn(128, 10)
    let out = softmax(matmul(h, w2))
    out
}

Talking point: Tensor is a first-class type. @device context
means this function cannot touch hardware registers.

DEMO 3: @kernel vs @device Isolation (3 min)
─────────────────────────────────────────
@kernel fn read_sensor() -> u32 {
    // Works: hardware access
    let val = port_read(0x3F8)
    // COMPILE ERROR: tensor op in @kernel context
    // let t = zeros(3, 3)  // <-- KE002: Tensor operations not allowed in @kernel
    val
}

@device fn process(data: Tensor) -> Tensor {
    // Works: tensor operations
    let result = relu(data)
    // COMPILE ERROR: raw pointer in @device context
    // let p: *mut u8 = 0x1000 as *mut u8  // <-- DE001
    result
}

Talking point: The compiler enforces domain isolation. This is
not a runtime check -- it's a compile-time guarantee.

DEMO 4: The Bridge Pattern (2 min)
─────────────────────────────────────────
@safe fn drone_loop() {
    let raw = read_sensor()       // calls @kernel fn
    let input = Tensor::from(raw) // safe conversion
    let action = process(input)   // calls @device fn
    execute(action)               // back to hardware
}

Talking point: @safe functions can call both @kernel and @device,
but cannot directly use hardware OR tensor ops. This is the
bridge -- the only safe meeting point.
```

**Slide 4:** REPL demo (live or recorded backup)
**Slide 5:** Tensor code example with @device annotation
**Slide 6:** Side-by-side @kernel and @device showing compile errors
**Slide 7:** Bridge pattern diagram (three contexts with arrows)

### Section 3: Architecture Deep Dive (10 minutes)

**Goal:** Explain how it works under the hood.

**Talking Points:**

1. **Compilation pipeline (2 min).** Source (.fj) -> Lexer (tokens) -> Parser (AST, Pratt parser with 19 precedence levels) -> Semantic Analyzer (type checking, context checking, borrow checking) -> Cranelift backend (native code) or interpreter (development mode).

2. **Context annotation system (3 min).** Four contexts: `@safe` (default), `@kernel` (OS, no heap/tensor), `@device` (ML, no raw pointers), `@unsafe` (full access). The semantic analyzer tracks context through the call graph and rejects cross-context violations. This is the key innovation.

3. **Ownership without lifetime annotations (2 min).** Fajar Lang uses Rust-inspired move semantics and borrow checking, but without explicit lifetime annotations. The compiler infers lifetimes using NLL (Non-Lexical Lifetimes) analysis. This makes the language simpler for embedded engineers who need memory safety but don't need Rust's full generality.

4. **First-class tensor types (2 min).** `Tensor` is a built-in type, not a library. The compiler knows tensor shapes and can check dimension mismatches at compile time. ndarray provides the runtime backend; Cranelift compiles tensor operations to native SIMD.

5. **What we built with it (1 min).** FajarOS Nova: 20,000+ lines of Fajar Lang, bare-metal x86_64 OS with 240+ shell commands, preemptive multitasking, NVMe storage, TCP/IP networking -- all in one language.

**Slide 8:** Compilation pipeline diagram (5 stages)
**Slide 9:** Context annotation table (4x5 grid of allowed/disallowed operations)
**Slide 10:** Borrow checker example (move semantics, use-after-move error)
**Slide 11:** Tensor type system (shape checking, matmul dimension verification)
**Slide 12:** FajarOS Nova architecture (5 layers, all Fajar Lang)

### Section 4: Results and Status (3 minutes)

**Goal:** Honest assessment of where the project stands.

**Talking Points:**

1. **By the numbers.** 292,000 lines of Rust in the compiler, 5,000+ tests, 126 example programs, 7 standard library packages. Two operating systems running on real hardware (x86_64 QEMU, ARM64 Radxa Dragon Q6A).

2. **What works today.** Core compiler (lexer through codegen), ML runtime (tensors, autograd, training loops), OS runtime (memory management, interrupts, syscalls), cross-compilation (x86_64, ARM64, RISC-V).

3. **What's in progress.** Distributed computing (networking stack), advanced FFI (C++ interop, Python embedding), formal verification (SMT solver integration), production profiling tools.

4. **How you can help.** Good-first-issues on GitHub, stdlib contributions, embedded hardware testing, documentation translations.

**Slide 13:** Dashboard of project statistics
**Slide 14:** Honest status: what works / what's in progress / what's planned

### Section 5: Q&A (5 minutes -- not on slides)

**Anticipated Questions and Answers:**

- **Q: How does this compare to Rust?** A: Fajar Lang is Rust-inspired but simpler (no lifetime annotations) and domain-specialized (built-in tensor types, context annotations). We don't aim to replace Rust for general systems programming -- we target the intersection of embedded and ML.

- **Q: Can I use existing C libraries?** A: Yes, we have C FFI via `extern "C"` blocks, similar to Rust's approach. C++ and Python interop are in development.

- **Q: What's the runtime overhead of context checking?** A: Zero. Context annotations are purely compile-time. The generated native code is the same whether you use `@kernel` or `@unsafe` -- the difference is what the compiler allows you to write.

- **Q: Is this production-ready?** A: The core language and compiler are production-quality (4,900+ passing tests, Cranelift native codegen). Some advanced features (distributed, formal verification) are still in development. We document this honestly in our gap analysis.

- **Q: Why Cranelift instead of LLVM?** A: We support both. Cranelift for fast compilation and embedded targets; LLVM for maximum optimization. Cranelift was first because it's lighter-weight and more accessible for the embedded use case.

**Slide 15:** "Get Started" -- GitHub URL, website, Discord, first steps

---

## Slide Outline Summary

| # | Title | Content |
|---|-------|---------|
| 1 | Title | "Fajar Lang: One Language for ML and OS" + speaker info |
| 2 | The Multi-Language Problem | Diagram: C + Python + Rust + glue in embedded AI |
| 3 | Real Failure Modes | malloc in ISR, tensor write to MMIO, FFI type mismatch |
| 4 | Demo: REPL | Live terminal: types, f-strings, pipeline operator |
| 5 | Demo: Tensor Ops | @device function with matmul, relu, softmax |
| 6 | Demo: Compile Errors | @kernel with tensor (KE002), @device with pointer (DE001) |
| 7 | The Bridge Pattern | Diagram: @kernel -> @safe -> @device data flow |
| 8 | Compilation Pipeline | Lexer -> Parser -> Analyzer -> Cranelift -> Binary |
| 9 | Context System | 4x5 table of allowed/disallowed operations per context |
| 10 | Ownership Model | Move semantics + NLL borrow checking, no lifetime annotations |
| 11 | Tensor Type System | First-class Tensor, shape checking at compile time |
| 12 | FajarOS Nova | 5-layer architecture, 240+ commands, all Fajar Lang |
| 13 | Project Status | 292K LOC, 5K+ tests, 126 examples, 7 packages |
| 14 | Honest Assessment | Works / In Progress / Planned (three columns) |
| 15 | Get Started | GitHub, website, Discord, good-first-issues |

---

## Preparation Checklist

- [ ] Test all demo code on the latest `fj` release before the talk.
- [ ] Prepare a recorded demo backup (screen capture) in case of live demo failure.
- [ ] Increase terminal font size to 24pt+ for visibility.
- [ ] Use a dark terminal theme with high contrast.
- [ ] Have the REPL pre-loaded with `fj repl` before the session starts.
- [ ] Bring a printed copy of the demo script as reference.
- [ ] Test slides on the venue projector (check colors, font rendering).
- [ ] Prepare 2-sentence answers for each anticipated question.

---

## Adapting for Different Formats

### Lightning Talk (5 minutes)
Focus on slides 1, 3, 6, 7, 15. Skip live demo -- use pre-recorded GIF. Core message: "Compiler-enforced domain isolation for embedded AI."

### Workshop (90 minutes)
Expand the demo section to 45 minutes with hands-on exercises. Provide a GitHub Codespace or Docker container with `fj` pre-installed. Exercises: write a @kernel function, write a @device function, intentionally trigger each compile error, build a bridge function.

### Podcast / Interview (30 minutes)
No slides or demo. Focus on the "why" (the problem), the design philosophy (explicitness over magic), and the honest status (what works vs. what's in progress). Have specific numbers ready.

---

*Conference Talk Guide v1.0 -- Fajar Lang Project*
