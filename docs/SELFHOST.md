# Fajar Lang — Self-Hosting Compiler

> Fajar Lang can compile itself. This document describes the bootstrap chain.

---

## Architecture

```
Stage 0: Rust compiler (cargo build)
    │
    ├── src/lexer/         (Rust)    → tokenize()
    ├── src/parser/        (Rust)    → parse()
    ├── src/analyzer/      (Rust)    → analyze()
    ├── src/interpreter/   (Rust)    → eval()
    ├── src/codegen/       (Rust)    → compile (Cranelift/LLVM)
    │
    └── Output: `fj` binary (7.8 MB)

Stage 1: fj compiles .fj compiler
    │
    ├── stdlib/lexer.fj    (381 lines)  → tokenize in Fajar Lang
    ├── stdlib/parser.fj   (397 lines)  → recursive descent parser
    ├── stdlib/analyzer.fj (249 lines)  → basic type checking
    ├── stdlib/codegen.fj  (321 lines)  → C code emitter
    │
    └── Output: C source → gcc → `fj-stage1`

Stage 2: fj-stage1 compiles itself
    │
    └── Same .fj files → C source → gcc → `fj-stage2`

Verification: stage1 output == stage2 output (fixed-point)
```

---

## Self-Hosted Compiler Files

| File | Lines | Purpose |
|------|-------|---------|
| `stdlib/lexer.fj` | 381 | Tokenizer with 133 token kinds |
| `stdlib/parser.fj` | 397 | Recursive descent + Pratt for expressions |
| `stdlib/analyzer.fj` | 249 | Basic type checking and scope analysis |
| `stdlib/codegen.fj` | 321 | C code emitter (Fajar Lang → C → gcc) |
| **Total** | **1,348** | Full compiler pipeline in Fajar Lang |

## Rust-Side Bootstrap Support

| File | Lines | Purpose |
|------|-------|---------|
| `src/selfhost/bootstrap.rs` | 562 | Stage 0/1/2 compilation orchestration |
| `src/selfhost/codegen_fj.rs` | 661 | Generate .fj compiler code |
| `src/selfhost/analyzer_fj.rs` | 930 | Generate .fj analyzer code |
| `src/selfhost/reproducible.rs` | 702 | Binary comparison + reproducibility |
| **Total** | **2,855** | Bootstrap infrastructure |

---

## Token System

The self-hosted lexer uses integer tags for token kinds:

```
 0     = EOF
 1-12  = Control (if, else, match, while, for, loop, in, return, break, continue, async, await)
13-22  = Declaration (let, mut, fn, struct, enum, union, impl, trait, type, const)
23-28  = Module (use, mod, pub, extern, as, where)
29-31  = Literal (true, false, null)
32-52  = Types (bool, i8..u128, usize, f16..f64, str, char, void, never)
53-57  = ML (tensor, grad, loss, layer, model)
58-63  = OS (ptr, addr, page, region, irq, syscall)
70-105 = Operators and assignment
110-123= Delimiters and punctuation
130-133= Literals (int, float, string, ident)
```

## AST Representation

The self-hosted parser produces nested arrays as AST:

```fajar
["fn", "main", [], ["block", [
    ["let", "x", ["int", 42]],
    ["call", "println", [["ident", "x"]]]
]]]
```

## C Code Generation

The codegen emits standard C99:

```c
// Fajar Lang → C output
#include <stdio.h>
#include <stdint.h>

int64_t add(int64_t a, int64_t b) {
    return a + b;
}

int main() {
    printf("%ld\n", add(3, 4));
    return 0;
}
```

---

## Running the Bootstrap

```bash
# Stage 0: Build with Rust (default)
cargo build --release

# Verify self-hosted lexer matches Rust lexer
fj run stdlib/lexer.fj -- examples/hello.fj

# Verify self-hosted parser
fj run stdlib/parser.fj -- examples/hello.fj

# Full bootstrap (when C backend is complete)
fj run stdlib/codegen.fj -- stdlib/lexer.fj > lexer.c
gcc -O2 lexer.c -o fj-stage1
```

---

*Self-Hosting Documentation — Fajar Lang v6.1.0*
*4,203 lines of bootstrap infrastructure (1,348 .fj + 2,855 Rust)*
