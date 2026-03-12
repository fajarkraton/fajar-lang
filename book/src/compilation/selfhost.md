# Self-Hosting

Fajar Lang can compile itself — the compiler is partially written in Fajar Lang (`.fj` source files).

## Bootstrap Chain

The self-hosting compiler uses a 3-stage bootstrap:

```
Stage 0: Rust compiler (src/)
    ↓ compiles
Stage 1: Fajar compiler written in .fj (stdlib/lexer.fj, stdlib/parser.fj)
    ↓ compiles
Stage 2: Fajar compiler compiled by Stage 1
    ↓ verify
Binary comparison: Stage 1 output == Stage 2 output
```

If Stage 1 and Stage 2 produce identical binaries, the compiler is correctly self-hosting.

## Self-Hosted Components

### Lexer (`stdlib/lexer.fj`)

The lexer is fully implemented in Fajar Lang:

```fajar
fn tokenize(source: str) -> [Token] {
    let mut tokens = []
    let mut pos = 0

    while pos < len(source) {
        let ch = char_at(source, pos)
        match ch {
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            // ... all token kinds
        }
        pos = pos + 1
    }

    tokens
}
```

### Parser (`stdlib/parser.fj`)

The parser uses a shunting-yard algorithm for expression parsing:

```fajar
fn parse_expr(tokens: [Token]) -> Expr {
    let mut output = []
    let mut operators = []

    for token in tokens {
        match token {
            Token::Number(n) => output.push(Expr::Lit(n)),
            Token::Plus | Token::Minus => {
                while !operators.is_empty() && precedence(operators.last()) >= precedence(token) {
                    apply_operator(&mut output, operators.pop())
                }
                operators.push(token)
            },
            // ...
        }
    }
    // ...
}
```

### Type Checker

Hindley-Milner type inference implemented in .fj with scope resolution, unification, and borrow analysis.

### Codegen

Cranelift IR generation from .fj — the compiler can emit native code for itself.

## Reproducible Builds

Deterministic compilation guarantees:

- **Same source → same binary** across platforms
- **FNV-1a hashing** for source provenance tracking
- **Content-addressable build cache** — artifacts keyed by content hash
- **Cross-platform verification** — build on Linux, verify on macOS

```bash
fj build --reproducible program.fj
# Outputs hash of produced binary for verification
```

## Running the Bootstrap

```bash
# Stage 0: build compiler with Rust
cargo build --release

# Stage 1: compile .fj compiler with Stage 0
./target/release/fj build stdlib/compiler.fj -o stage1

# Stage 2: compile .fj compiler with Stage 1
./stage1 build stdlib/compiler.fj -o stage2

# Verify: must be identical
diff stage1 stage2
```
