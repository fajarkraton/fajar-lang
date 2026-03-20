//! ARM64-specific benchmarks for Fajar Lang on Dragon Q6A.
//!
//! Benchmarks interpreter, tensor ops, and edge AI builtins
//! to measure performance on Kryo 670 (A78 big + A55 LITTLE).
//!
//! Run on Q6A: `cargo bench --bench arm64_bench`
//! Run on host: `cargo bench --bench arm64_bench` (works cross-platform)

use criterion::{Criterion, criterion_group, criterion_main};
use fajar_lang::interpreter::Interpreter;

fn bench_interpreter_fibonacci(c: &mut Criterion) {
    let code = r#"
fn fib(n: i64) -> i64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}
fib(20)
"#;
    c.bench_function("arm64_fib_20_interpreted", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.eval_source(code).unwrap();
        })
    });
}

fn bench_interpreter_loop(c: &mut Criterion) {
    let code = r#"
let mut sum = 0
let mut i = 0
while i < 1000 {
    sum = sum + i
    i = i + 1
}
sum
"#;
    c.bench_function("arm64_loop_1000", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.eval_source(code).unwrap();
        })
    });
}

fn bench_tensor_matmul(c: &mut Criterion) {
    let code = r#"
let a = tensor_randn(32, 32)
let b = tensor_randn(32, 32)
let c = tensor_matmul(a, b)
"#;
    c.bench_function("arm64_tensor_matmul_32x32", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.eval_source(code).unwrap();
        })
    });
}

fn bench_tensor_pipeline(c: &mut Criterion) {
    let code = r#"
let input = tensor_randn(1, 64)
let w = tensor_xavier(64, 10)
let h = tensor_relu(tensor_matmul(input, w))
let probs = tensor_softmax(h)
let class = tensor_argmax(probs)
"#;
    c.bench_function("arm64_inference_pipeline", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.eval_source(code).unwrap();
        })
    });
}

fn bench_edge_builtins(c: &mut Criterion) {
    let code = r#"
let t = cpu_temp()
let f = cpu_freq()
let m = mem_usage()
let u = sys_uptime()
"#;
    c.bench_function("arm64_edge_builtins", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.eval_source(code).unwrap();
        })
    });
}

fn bench_string_operations(c: &mut Criterion) {
    let code = r#"
let mut s = ""
let mut i = 0
while i < 100 {
    s = s + "x"
    i = i + 1
}
len(s)
"#;
    c.bench_function("arm64_string_concat_100", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.eval_source(code).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_interpreter_fibonacci,
    bench_interpreter_loop,
    bench_tensor_matmul,
    bench_tensor_pipeline,
    bench_edge_builtins,
    bench_string_operations,
);
criterion_main!(benches);
