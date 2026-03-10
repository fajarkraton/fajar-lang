//! Embedded-oriented benchmarks for Fajar Lang.
//!
//! Benchmarks stack-allocated tensor operations and fixed-point arithmetic
//! to measure embedded inference performance without heap allocation.

use criterion::{criterion_group, criterion_main, Criterion};
use fajar_lang::runtime::ml::fixed_point::{Q16_16, Q8_8};
use fajar_lang::runtime::ml::stack_tensor::{stack_dense_forward, stack_matmul, StackTensor};

fn bench_stack_tensor_add(c: &mut Criterion) {
    let a = StackTensor::<256>::from_slice(&[1.0; 64], &[8, 8]).unwrap();
    let b = StackTensor::<256>::from_slice(&[2.0; 64], &[8, 8]).unwrap();

    c.bench_function("stack_tensor_add_8x8", |bench| {
        bench.iter(|| a.add(&b).unwrap())
    });
}

fn bench_stack_tensor_relu(c: &mut Criterion) {
    let mut data = [0.0f64; 64];
    for (i, v) in data.iter_mut().enumerate() {
        *v = (i as f64) - 32.0; // Mix of positive and negative
    }
    let t = StackTensor::<256>::from_slice(&data, &[8, 8]).unwrap();

    c.bench_function("stack_tensor_relu_8x8", |bench| bench.iter(|| t.relu()));
}

fn bench_stack_matmul_4x4(c: &mut Criterion) {
    let a = StackTensor::<256>::from_slice(
        &[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ],
        &[4, 4],
    )
    .unwrap();
    let b = StackTensor::<256>::from_slice(&[1.0; 16], &[4, 4]).unwrap();

    c.bench_function("stack_matmul_4x4", |bench| {
        bench.iter(|| stack_matmul::<256>(&a, &b).unwrap())
    });
}

fn bench_stack_dense_forward(c: &mut Criterion) {
    // Simulate a small embedded inference: 4 inputs → 8 hidden
    let input = StackTensor::<256>::from_slice(&[0.1, 0.2, 0.3, 0.4], &[1, 4]).unwrap();
    let weights = StackTensor::<256>::from_slice(&[0.1; 32], &[4, 8]).unwrap();
    let bias = StackTensor::<256>::from_slice(&[0.0; 8], &[1, 8]).unwrap();

    c.bench_function("stack_dense_4_to_8", |bench| {
        bench.iter(|| stack_dense_forward::<256>(&input, &weights, &bias).unwrap())
    });
}

fn bench_stack_inference_pipeline(c: &mut Criterion) {
    // Full embedded inference: 4 → 8 (relu) → 3 (output)
    let input = StackTensor::<256>::from_slice(&[0.1, 0.2, 0.3, 0.4], &[1, 4]).unwrap();
    let w1 = StackTensor::<256>::from_slice(&[0.1; 32], &[4, 8]).unwrap();
    let b1 = StackTensor::<256>::from_slice(&[0.0; 8], &[1, 8]).unwrap();
    let w2 = StackTensor::<256>::from_slice(&[0.1; 24], &[8, 3]).unwrap();
    let b2 = StackTensor::<256>::from_slice(&[0.0; 3], &[1, 3]).unwrap();

    c.bench_function("stack_inference_4_8_3", |bench| {
        bench.iter(|| {
            // Layer 1: dense + relu
            let z1 = stack_dense_forward::<256>(&input, &w1, &b1).unwrap();
            let a1 = z1.relu();
            // Layer 2: dense (output)
            let output = stack_dense_forward::<256>(&a1, &w2, &b2).unwrap();
            output.argmax()
        })
    });
}

fn bench_fixed_point_q16_16(c: &mut Criterion) {
    let a = Q16_16::from_f64(3.14);
    let b = Q16_16::from_f64(2.71);

    c.bench_function("fixed_point_q16_16_mul", |bench| {
        bench.iter(|| {
            let mut acc = Q16_16::from_f64(0.0);
            for _ in 0..100 {
                acc = acc + a * b;
            }
            acc
        })
    });
}

fn bench_fixed_point_q8_8(c: &mut Criterion) {
    let a = Q8_8::from_f64(1.5);
    let b = Q8_8::from_f64(0.75);

    c.bench_function("fixed_point_q8_8_mul", |bench| {
        bench.iter(|| {
            let mut acc = Q8_8::from_f64(0.0);
            for _ in 0..100 {
                acc = acc + a * b;
            }
            acc
        })
    });
}

criterion_group!(
    benches,
    bench_stack_tensor_add,
    bench_stack_tensor_relu,
    bench_stack_matmul_4x4,
    bench_stack_dense_forward,
    bench_stack_inference_pipeline,
    bench_fixed_point_q16_16,
    bench_fixed_point_q8_8,
);
criterion_main!(benches);
