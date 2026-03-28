# Fajar Lang Developer Certification Program

Three certification levels recognizing proficiency in Fajar Lang development.

## Level 1: Fajar Lang Associate (FLA)

**Audience:** Developers with basic Fajar Lang knowledge.

**Exam Topics:**
- Language syntax: variables, types, functions, control flow
- Structs, enums, pattern matching
- Error handling with `Result<T, E>` and `Option<T>`
- Module system and `use` imports
- Basic standard library: `std::io`, `std::string`, `std::math`
- Using the CLI: `fj run`, `fj check`, `fj repl`

**Format:** 40 multiple-choice questions, 60 minutes.
**Passing Score:** 70%

**Study Materials:**
- Book chapters 1-6 (Fundamentals)
- `examples/hello.fj` through `examples/calculator.fj`
- Workshop Part 1

---

## Level 2: Fajar Lang Professional (FLP)

**Audience:** Developers building production applications.

**Exam Topics:**
- Generics and trait system (monomorphization, trait objects)
- Ownership, move semantics, borrow checking
- Iterators, closures, and the pipeline operator
- Tensor operations and autograd basics
- Neural network layers (Dense, Conv2d)
- Context annotations: `@safe`, `@device`, `@kernel`, `@unsafe`
- Testing with `@test`, benchmarking, documentation generation
- Package management with `fj.toml`

**Format:** 30 multiple-choice + 3 coding exercises, 90 minutes.
**Passing Score:** 75%
**Prerequisite:** FLA certification or equivalent experience.

**Study Materials:**
- Book chapters 7-15 (Advanced Features, ML, Safety)
- `examples/mnist.fj`, `examples/trait_objects.fj`
- Workshop Parts 1-2

---

## Level 3: Fajar Lang Expert (FLE)

**Audience:** Engineers building embedded AI systems or OS components.

**Exam Topics:**
- Cross-compilation for ARM64, RISC-V, bare-metal targets
- `@kernel` programming: memory management, IRQ, syscalls
- `@device` programming: optimized inference pipelines
- Bridge pattern: kernel-to-device-to-safe data flow
- Cranelift and LLVM backend internals
- Quantization (INT8) and model optimization
- Concurrency: async/await, channels, atomics
- Contributing to the compiler: parser, analyzer, codegen

**Format:** 20 multiple-choice + 2 system design + 1 live coding, 120 minutes.
**Passing Score:** 80%
**Prerequisite:** FLP certification.

**Study Materials:**
- Full book (all chapters)
- FajarOS Nova source study
- Workshop Parts 1-3
- `docs/ARCHITECTURE.md`, `docs/FAJAR_LANG_SPEC.md`

---

## Certification Process

1. **Register** at the Fajar Lang community portal
2. **Study** using the materials listed for your target level
3. **Schedule** an exam session (online proctored or community event)
4. **Take the exam** within the allocated time
5. **Receive results** within 48 hours
6. **Badge issued** as a verifiable digital credential

## Recertification

- Certifications are valid for 2 years
- Recertify by passing the current exam or completing 3 approved community contributions

## Community Exam Proctors

Community leaders may apply to become exam proctors for in-person events.
Contact: certification@fajarlang.org
