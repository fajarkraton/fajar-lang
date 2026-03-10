//! Performance benchmarks for the Fajar Lang interpreter.
//!
//! Sprint 7.2: criterion benchmark suite.

use criterion::{criterion_group, criterion_main, Criterion};
use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;

fn bench_lexer(c: &mut Criterion) {
    let source = "let x = 42\nlet y = x + 1\nlet z = x * y\n".repeat(100);
    c.bench_function("lex_3000_tokens", |b| b.iter(|| tokenize(&source).unwrap()));
}

fn bench_parser(c: &mut Criterion) {
    let source = "let x = 42\nlet y = x + 1\nlet z = x * y\n".repeat(100);
    let tokens = tokenize(&source).unwrap();
    c.bench_function("parse_300_stmts", |b| {
        b.iter(|| parse(tokens.clone()).unwrap())
    });
}

fn bench_fibonacci(c: &mut Criterion) {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> void {
            println(fib(20))
        }
    "#;
    c.bench_function("fibonacci_20_treewalk", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new_capturing();
            interp.eval_source(src).unwrap();
            interp.call_main().unwrap();
        })
    });
}

fn bench_loop(c: &mut Criterion) {
    let src = r#"
        fn main() -> void {
            let mut sum = 0
            let mut i = 0
            while i < 1000 {
                sum = sum + i
                i = i + 1
            }
            println(sum)
        }
    "#;
    c.bench_function("loop_1000_iterations", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new_capturing();
            interp.eval_source(src).unwrap();
            interp.call_main().unwrap();
        })
    });
}

fn bench_string_ops(c: &mut Criterion) {
    let src = r#"
        fn main() -> void {
            let mut s = ""
            let mut i = 0
            while i < 100 {
                s = s + "x"
                i = i + 1
            }
            println(len(s))
        }
    "#;
    c.bench_function("string_concat_100", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new_capturing();
            interp.eval_source(src).unwrap();
            interp.call_main().unwrap();
        })
    });
}

#[cfg(feature = "native")]
fn bench_fibonacci_native(c: &mut Criterion) {
    use fajar_lang::codegen::cranelift::CraneliftCompiler;
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(30) }
    "#;

    // Compile once, benchmark just the execution
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = CraneliftCompiler::new().unwrap();
    compiler.compile_program(&program).unwrap();
    let fn_ptr = compiler.get_fn_ptr("main").unwrap();
    // SAFETY: main() compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };

    c.bench_function("fibonacci_30_native", |b| {
        b.iter(|| {
            let result = main_fn();
            assert_eq!(result, 832040);
        })
    });
}

#[cfg(feature = "native")]
fn bench_fibonacci_treewalk_30(c: &mut Criterion) {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> void {
            fib(30)
        }
    "#;
    c.bench_function("fibonacci_30_treewalk", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new_capturing();
            interp.eval_source(src).unwrap();
            interp.call_main().unwrap();
        })
    });
}

#[cfg(feature = "native")]
fn bench_loop_native(c: &mut Criterion) {
    use fajar_lang::codegen::cranelift::CraneliftCompiler;
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 1000 {
                sum = sum + i
                i = i + 1
            }
            sum
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = CraneliftCompiler::new().unwrap();
    compiler.compile_program(&program).unwrap();
    let fn_ptr = compiler.get_fn_ptr("main").unwrap();
    // SAFETY: main() compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };

    c.bench_function("loop_1000_native", |b| {
        b.iter(|| {
            let result = main_fn();
            assert_eq!(result, 499500);
        })
    });
}

criterion_group!(
    benches,
    bench_lexer,
    bench_parser,
    bench_fibonacci,
    bench_loop,
    bench_string_ops
);

#[cfg(feature = "native")]
criterion_group!(
    native_benches,
    bench_fibonacci_native,
    bench_fibonacci_treewalk_30,
    bench_loop_native
);

#[cfg(not(feature = "native"))]
criterion_main!(benches);

#[cfg(feature = "native")]
criterion_main!(benches, native_benches);
