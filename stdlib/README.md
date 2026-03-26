# Self-Hosted Compiler — Architecture Guide

## Overview

The Fajar Lang self-hosted compiler is written entirely in Fajar Lang (`.fj` files).
It tokenizes, parses, and analyzes Fajar Lang source code using only the language's
own builtins — no external dependencies.

**Total: 3,076 lines across 6 modules**

## Modules

### lexer.fj (513 lines)
Tokenizes source text into an array of integer token kind tags.

- **Input:** `source: str`
- **Output:** `[i64]` — array of token kind tags (0=EOF, 130=IntLit, 133=Ident, etc.)
- **Key function:** `pub fn tokenize(source: str) -> [i64]`
- **Features:** 80+ token kinds, 60+ keywords, hex/bin/oct numbers, char/f-string literals, nested block comments
- **Tests:** 50/50 pass
- **SQ11.8:** `offset_to_line()`, `offset_to_col()`, `format_pos()` for error positioning

### parser.fj (784 lines)
Recursive descent parser that processes token streams.

- **Input:** `tokens: [i64]` from lexer
- **Output:** `i64` — number of parsed items
- **Key function:** `pub fn parse_program(tokens: [i64]) -> i64`
- **Handles:** fn, let, const, return, break, continue, if/else, while, for, loop, match, struct, enum, impl, trait, use, annotations
- **SQ11.5:** `parse_program_recovering()` — collects multiple errors, synchronizes, continues
- **Tests:** 30/30 pass

### analyzer.fj (432 lines)
Semantic analysis with scope tracking and type inference.

- **Input:** `tokens: [i64], source: str`
- **Output:** `AnalyzerState` with error collection
- **Key function:** `pub fn analyze_tokens(tokens, source) -> AnalyzerState`
- **Features:** Symbol table (var_names/types/moved), function registration, type inference from token kinds, context flags (in_function, in_loop)
- **SQ11.4:** Push/pop scope with `var_depths` tracking
- **Error codes:** 8 (undefined var/fn, duplicate, return/break outside context, type mismatch, use-after-move, arg count)
- **Tests:** 20/20 pass

### ast.fj (231 lines)
AST node constructors and utilities.

- **Node types:** int, float, str, bool, ident, binop, unary, let, return, fn, struct, use
- **Key functions:** `ast_type()`, `ast_is()`, `ast_print()` for introspection and display
- **SQ11.1:** Array-based AST nodes `["tag", field1, field2, ...]`
- **SQ11.2:** Pretty printer: `ast_let("x","i64","42")` → `"let x: i64 = 42"`

### compiler.fj (243 lines)
End-to-end compilation pipeline combining lexer + parser + analyzer.

- **Key function:** `pub fn compile(source, file) -> CompileResult`
- **Features:** Bootstrap verification (15 programs), differential testing, stress testing
- **Bootstrap:** Stage 0 (Rust fj) → Stage 1 (self-hosted) verified

### core.fj, nn.fj, os.fj, hal.fj, drivers.fj, codegen.fj
Standard library modules for ML, OS, hardware, and code generation.

## How to Run

```bash
# Run self-hosted lexer tests (50 tests)
cat stdlib/lexer.fj examples/selfhost_lexer_v3.fj > /tmp/t.fj && fj run /tmp/t.fj

# Run self-hosted parser tests (30 tests)
cat stdlib/lexer.fj stdlib/parser.fj > /tmp/t.fj && fj run /tmp/t.fj

# Run self-hosted analyzer tests (20 tests)
cat stdlib/lexer.fj stdlib/analyzer.fj examples/selfhost_analyzer_v3.fj > /tmp/t.fj && fj run /tmp/t.fj

# Run bootstrap verification (15 programs)
cat stdlib/lexer.fj stdlib/parser.fj stdlib/analyzer.fj stdlib/compiler.fj examples/selfhost_bootstrap_v3.fj > /tmp/t.fj && fj run /tmp/t.fj
```

## Performance

| Metric | Self-hosted | Rust |
|--------|-----------|------|
| Tokenize 50 functions (2,430 chars) | ~6.4s | ~0.26s |
| Ratio | 24x slower | baseline |

The self-hosted lexer is 24x slower due to per-character string allocation.
Future improvements: `char_at` builtin, JIT compilation.

## Known Limitations

1. **No real AST tree** — parser returns item count, AST builder creates flat string arrays
2. **Stage 2 bootstrap not achieved** — self-hosted compiler can't compile itself (stack overflow)
3. **Performance** — 24x slower than Rust (acceptable for self-hosting proof, not production)
4. **Token spans** — positions computed post-hoc via offset_to_line/col, not stored per token
