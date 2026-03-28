# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 5.5.x (Illumination) | Yes — current release |
| 5.4.x (Zenith) | Yes — security fixes |
| 5.3.x (Bastion) | Yes — critical fixes only |
| 5.2.x (Nexus) | No — upgrade recommended |
| < 5.2.0 | No — end of life |

---

## Reporting a Vulnerability

If you discover a security vulnerability in Fajar Lang, **please report it responsibly**.

### Contact

**Email:** security@primecore.id

**Do NOT** open a public GitHub issue for security vulnerabilities. Use the email above for confidential disclosure.

### What to include

- Description of the vulnerability
- Steps to reproduce (minimal `.fj` program if applicable)
- Which component is affected (compiler, runtime, codegen, etc.)
- Potential impact assessment
- Suggested fix (if you have one)

### Response timeline

| Stage | Timeline |
|-------|----------|
| Acknowledgement | Within 48 hours |
| Initial assessment | Within 7 days |
| Fix development | Within 30 days (critical) / 90 days (moderate) |
| Public disclosure | After fix is released, coordinated with reporter |

We will keep you informed of progress throughout the process. If we determine the report is not a security issue, we will explain why and suggest filing a regular issue if appropriate.

---

## Security Model

Fajar Lang's security is built on the principle of **"Security by Construction"** — if the program compiles, it satisfies the safety invariants enforced by the compiler.

### Context isolation

The compiler enforces strict isolation between execution contexts:

| Context | Capabilities | Restrictions |
|---------|-------------|--------------|
| `@safe` | Standard operations, call other contexts via bridge | No raw pointers, no hardware access, no direct tensor |
| `@kernel` | Raw memory, IRQ, syscalls, port I/O, page tables | No heap allocation, no tensor operations |
| `@device` | Tensor ops, autograd, GPU compute, model inference | No raw pointers, no IRQ, no hardware access |
| `@unsafe` | All capabilities | Must be explicitly opted into |

Cross-context calls are verified at compile time. A `@kernel` function cannot call a `@device` function, and vice versa. The `@safe` context bridges both through controlled interfaces.

### Memory safety

- **Ownership system** — move semantics prevent use-after-free (no lifetime annotations required)
- **Borrow checker** — many shared references (`&T`) OR one mutable reference (`&mut T`)
- **Null safety** — no null pointers; `Option<T>` for optional values
- **Bounds checking** — array and slice access checked at runtime
- **Integer overflow** — checked in debug mode, wrapping configurable in release

### Type safety

- **No implicit type conversions** — all casts must be explicit with `as`
- **PhysAddr and VirtAddr** — distinct types prevent address confusion in OS code
- **Tensor shape checking** — dimensions verified at compile time where possible
- **Exhaustive match** — all enum variants must be handled

### Compiler enforcement

The following error codes are related to security enforcement:

| Code | Category | Description |
|------|----------|-------------|
| KE001 | Kernel | Heap allocation in `@kernel` context |
| KE002 | Kernel | Tensor operation in `@kernel` context |
| KE003 | Kernel | Cross-context call violation |
| DE001 | Device | Raw pointer in `@device` context |
| DE002 | Device | Hardware access in `@device` context |
| ME001 | Memory | Use after move |
| ME002 | Memory | Mutable borrow while shared borrow exists |
| SE020 | Safety | Hardware access in `@safe` context |

---

## Known Security Properties

### What Fajar Lang guarantees (when compiled without `@unsafe`)

1. No use-after-free
2. No double-free
3. No null pointer dereference
4. No buffer overflow (bounds-checked access)
5. No data races (ownership + borrow rules)
6. No uninitialized memory reads
7. Context isolation between kernel and device code

### What Fajar Lang does NOT guarantee

1. Freedom from logic bugs (the program may compute wrong results)
2. Termination (infinite loops are possible)
3. Freedom from resource exhaustion (stack overflow, OOM)
4. Safety of `@unsafe` blocks (developer responsibility)
5. Safety of FFI calls to C libraries
6. Side-channel resistance (timing attacks, etc.)

### Compiler hardening options

When compiling with security hardening enabled:

- **Stack canaries** — detect stack buffer overflows
- **Control-flow integrity (CFI)** — prevent control-flow hijacking
- **Address sanitizer simulation** — detect memory errors in debug builds
- **`-fharden` flag** — enables all hardening options

---

## Security of the Compiler Itself

The Fajar Lang compiler is written in Rust (~290,000 LOC) and benefits from Rust's own memory safety guarantees. The compiler codebase:

- Contains no `unsafe` blocks outside `src/codegen/` and `src/runtime/os/`
- Every `unsafe` block has a `// SAFETY:` comment explaining the invariant
- Passes `cargo clippy -- -D warnings` with zero warnings
- Is tested with 6,286 tests including property-based testing
- No `.unwrap()` calls in `src/` (only in test code)

---

## Bug Bounty Program

The Fajar Lang Bug Bounty Program rewards security researchers who discover and responsibly disclose vulnerabilities in the compiler, runtime, and related tooling.

### Scope

The following components are in scope for the bug bounty program:

| Component | Examples |
|-----------|---------|
| **Compiler** | Lexer, parser, analyzer, type checker -- any input that causes incorrect compilation or crashes |
| **Runtime** | Interpreter, bytecode VM -- memory safety violations, sandbox escapes |
| **Codegen** | Cranelift and LLVM backends -- generated code that violates safety invariants |
| **FFI boundaries** | C interop, extern functions -- any way to bypass type or memory safety through FFI |
| **Context isolation** | `@kernel`/`@device`/`@safe` enforcement -- any way to access restricted operations from wrong context |

Out of scope: documentation typos, denial-of-service via large inputs (unless disproportionate resource consumption), issues in third-party dependencies (report upstream), social engineering, and the project website.

### Severity Levels and Rewards

| Severity | Description | Reward |
|----------|-------------|--------|
| **Critical** | Memory safety violation in generated code, sandbox escape, arbitrary code execution through crafted `.fj` input | $500 |
| **High** | Context isolation bypass (`@safe` code accessing `@kernel`-only operations), type system unsoundness allowing undefined behavior | $250 |
| **Medium** | Compiler crash on valid input, incorrect codegen that produces wrong results (but no safety violation), borrow checker bypass | $100 |
| **Low** | Compiler accepts invalid code without error, misleading error messages that hide real issues, minor information leaks | Recognition in release notes |

Rewards are paid via GitHub Sponsors, PayPal, or bank transfer at the reporter's preference. One reward per unique vulnerability.

### How to Report

1. **Email:** Send a detailed report to **security@fajarlang.dev**
2. **Include:** A minimal `.fj` program that reproduces the issue, the expected vs. actual behavior, which component is affected, and your assessment of severity
3. **Do NOT** open a public GitHub issue for security vulnerabilities

### Response SLA

| Stage | Timeline |
|-------|----------|
| Acknowledgment of report | Within 48 hours |
| Initial assessment and severity classification | Within 7 days |
| Fix development (critical/high) | Within 30 days |
| Fix development (medium/low) | Within 90 days |
| Public disclosure | Coordinated with reporter after fix is released |

### Rules

- Only test against your own installations or the public playground (do not test against other users' systems)
- Do not disclose the vulnerability publicly until a fix is released and we coordinate disclosure
- One report per vulnerability; duplicate reports receive no reward (first reporter wins)
- Reports must include a reproducible proof of concept
- Researchers who follow responsible disclosure will be credited in release notes (with permission)

---

## Contact

- **Security reports:** security@primecore.id
- **General inquiries:** fajar@primecore.id
- **GitHub:** [github.com/fajarkraton/fajar-lang](https://github.com/fajarkraton/fajar-lang)
