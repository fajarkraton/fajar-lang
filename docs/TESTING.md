# TESTING

> Strategi Testing & Quality Assurance — Fajar Lang Comprehensive Test Plan

---

## Daftar Isi

1. [Testing Philosophy](#1-testing-philosophy)
2. [Kategori Test](#2-kategori-test)
3. [Coverage Requirements per Komponen](#3-coverage-requirements-per-komponen)
4. [Test Patterns & Macros](#4-test-patterns--macros)
5. [Context Annotation Test Matrix](#5-context-annotation-test-matrix)
6. [Benchmark Targets](#6-benchmark-targets)
7. [CI/CD Pipeline](#7-cicd-pipeline)

---

## 1. Testing Philosophy

Fajar Lang mengikuti pendekatan TDD (Test-Driven Development) yang ketat. Setiap fitur dimulai dengan test, baru kemudian implementasi. Prinsip utama:

- **RED:** Tulis test yang gagal — test mendeskripsikan behavior yang diinginkan
- **GREEN:** Tulis kode minimal agar test lulus — tidak lebih
- **REFACTOR:** Perbaiki kode tanpa mengubah behavior — test tetap hijau

> **Golden Rule:** Jika sebuah fungsi tidak memiliki test, fungsi itu dianggap tidak ada. Tidak ada kode tanpa test yang boleh di-merge.

---

## 2. Kategori Test

### 2.1 Unit Tests

Test untuk individual function atau method. Berada di file yang sama dengan implementasi menggunakan `#[cfg(test)] mod tests`.

```rust
// src/lexer/token.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer_produces_int_token_for_decimal_literal() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::IntLit(42));
    }

    #[test]
    fn lexer_reports_error_for_unterminated_string() {
        let result = tokenize(r#""hello"#);
        assert!(result.is_err());
    }
}
```

### 2.2 Integration Tests

Test untuk interaksi antar komponen. Berada di folder `tests/`:

| File | Scope | Deskripsi |
|------|-------|-----------|
| `tests/lexer_tests.rs` | Lexer | Full tokenization dari source code lengkap |
| `tests/parser_tests.rs` | Lexer + Parser | Parse complete programs dari string |
| `tests/eval_tests.rs` | Full Pipeline | Execute Fajar Lang programs, verify output |
| `tests/os_tests.rs` | OS Runtime | Memory alloc, IRQ, syscall simulation |
| `tests/ml_tests.rs` | ML Runtime | Tensor ops, autograd correctness |

### 2.3 End-to-End Tests

Test yang menjalankan file `.fj` lengkap dan memverifikasi output:

```rust
// tests/e2e_tests.rs
#[test]
fn e2e_hello_world() {
    let output = run_fj_file("examples/hello.fj");
    assert_eq!(output.stdout, "Hello from Fajar Lang!\n");
    assert_eq!(output.exit_code, 0);
}

#[test]
fn e2e_fibonacci() {
    let output = run_fj_file("examples/fibonacci.fj");
    assert!(output.stdout.contains("55"));
}
```

### 2.4 Property-Based Tests

Untuk komponen yang harus memenuhi invariant tertentu:

```rust
// Menggunakan proptest atau quickcheck
use proptest::prelude::*;

proptest! {
    #[test]
    fn lexer_never_produces_empty_span(source in "[a-zA-Z0-9 +\\-*/=]+") {
        if let Ok(tokens) = tokenize(&source) {
            for token in &tokens {
                prop_assert!(token.span.end > token.span.start);
            }
        }
    }
}
```

---

## 3. Coverage Requirements per Komponen

| Komponen | Coverage Target | Metric | Catatan |
|----------|-----------------|--------|---------|
| Lexer | 100% | Setiap TokenKind | Termasuk edge case: empty string, unicode, max int |
| Parser | 100% | Setiap AST node | Termasuk error recovery test |
| Analyzer | 100% | Setiap error kind | Setiap SemanticError variant harus di-trigger oleh test |
| Interpreter | 100% | Setiap built-in | Semua Value variant, semua operator kombinasi |
| OS Runtime | 100% | Setiap primitive | Simulated: alloc, free, map_page, IRQ, port I/O |
| ML Runtime | 100% | Setiap op + grad | Numerical gradient check vs analytical gradient |
| CLI | 90%+ | Setiap subcommand | run, repl, check, fmt, dump-tokens, dump-ast |

---

## 4. Test Patterns & Macros

### 4.1 Token Assertion Macro

```rust
macro_rules! assert_tokens {
    ($src:expr, $($kind:expr),+) => {
        let tokens = tokenize($src).unwrap();
        let kinds: Vec<_> = tokens.iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| t.kind.clone()).collect();
        assert_eq!(kinds, vec![$($kind),+]);
    };
}

// Usage:
assert_tokens!("let x = 42",
    TokenKind::Let,
    TokenKind::Ident("x".into()),
    TokenKind::Eq,
    TokenKind::IntLit(42)
);
```

### 4.2 Eval Assertion Macro

```rust
macro_rules! assert_eval {
    ($src:expr, $expected:expr) => {
        let mut interp = Interpreter::new();
        let result = interp.eval_source($src).unwrap();
        assert_eq!(result, $expected);
    };
}

// Usage:
assert_eval!("1 + 2 * 3", Value::Int(7));
assert_eval!("true && false", Value::Bool(false));
```

### 4.3 Error Assertion Pattern

```rust
macro_rules! assert_compile_error {
    ($src:expr, $error_code:expr) => {
        let result = compile($src);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.code() == $error_code),
            "Expected error {}, got: {:?}", $error_code, errors);
    };
}

// Usage:
assert_compile_error!(
    "@kernel fn f() { String::new() }",
    "KE001"  // heap alloc in @kernel
);
```

### 4.4 Gradient Check Pattern

```rust
fn numerical_gradient(f: impl Fn(f64) -> f64, x: f64) -> f64 {
    let eps = 1e-5;
    (f(x + eps) - f(x - eps)) / (2.0 * eps)
}

#[test]
fn autograd_sigmoid_gradient_matches_numerical() {
    let x = TensorValue::scalar(0.5);
    let y = sigmoid(&x);
    y.backward();
    let analytical = x.grad().unwrap();
    let numerical = numerical_gradient(|v| 1.0 / (1.0 + (-v).exp()), 0.5);
    assert!((analytical - numerical).abs() < 1e-4);
}
```

---

## 5. Context Annotation Test Matrix

Setiap kombinasi context + operation harus di-test:

| Operasi | `@safe` | `@kernel` | `@device` | `@unsafe` |
|---------|---------|-----------|-----------|-----------|
| `let x = 42` | ✅ OK | ✅ OK | ✅ OK | ✅ OK |
| `String::new()` | ✅ OK | ❌ ERROR KE001 | ✅ OK | ✅ OK |
| `zeros(3,4)` (direct) | ❌ ERROR | ❌ ERROR KE002 | ✅ OK | ✅ OK |
| `alloc!(4096)` | ❌ ERROR | ✅ OK | ❌ ERROR DE002 | ✅ OK |
| `*mut T` dereference | ❌ ERROR | ✅ OK | ❌ ERROR DE001 | ✅ OK |
| `irq_register!()` | ❌ ERROR | ✅ OK | ❌ ERROR DE002 | ✅ OK |
| `port_write!()` | ❌ ERROR | ✅ OK | ❌ ERROR DE002 | ✅ OK |
| `relu()` (direct) | ❌ ERROR | ❌ ERROR KE002 | ✅ OK | ✅ OK |
| `x.backward()` | ❌ ERROR | ❌ ERROR KE002 | ✅ OK | ✅ OK |
| Call `@device` fn | ✅ OK | ❌ ERROR | ✅ OK | ✅ OK |
| Call `@kernel` fn | ✅ OK | ✅ OK | ❌ ERROR | ✅ OK |

> **Note:** `@safe` can CALL `@device`/`@kernel` functions (bridge pattern) but cannot directly use their primitives. See FAJAR_LANG_SPEC.md Section 6.7 for the context calling convention.

---

## 6. Benchmark Targets

| Operasi | Target | Catatan |
|---------|--------|---------|
| Tokenize 1000 lines | < 5ms | Menggunakan criterion |
| Parse 1000 lines | < 10ms | Recursive descent + Pratt |
| Analyze 1000 lines | < 20ms | Type check + context validation |
| Eval fibonacci(30) | < 500ms | Tree-walking, tanpa optimization |
| Tensor matmul 100x100 | < 1ms | Via ndarray BLAS |
| Full pipeline hello.fj | < 50ms | Lex + Parse + Analyze + Eval |

```rust
// benches/interpreter_bench.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_lexer(c: &mut Criterion) {
    let source = include_str!("../examples/fibonacci.fj");
    c.bench_function("lex_fibonacci", |b| {
        b.iter(|| tokenize(source))
    });
}
```

---

## 7. CI/CD Pipeline

Setiap commit harus melewati pipeline berikut (GitHub Actions):

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all
      - run: cargo clippy -- -D warnings
      - run: cargo fmt -- --check
      - run: cargo doc --no-deps
```

---

*Test Suite Version: 1.0 | Optimized for: Claude Code + Opus 4.6*
