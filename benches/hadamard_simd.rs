//! Benchmark: Hadamard scalar vs AVX2
//!
//! B5.L2.2 verification: `cargo bench --bench hadamard_simd`
//! Target: AVX2 path ≥ 2x speedup over scalar at D ≥ 128.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use fajar_lang::runtime::ml::TensorValue;
use fajar_lang::runtime::ml::ops::{hadamard, hadamard_avx2, hadamard_quantize};
use fajar_lang::runtime::ml::quantize::QuantizedValue;

fn bench_hadamard(c: &mut Criterion) {
    let mut group = c.benchmark_group("hadamard");

    for d in [16, 64, 128, 256, 512, 1024] {
        let data: Vec<f64> = (0..d)
            .map(|i| (i as f64) * 0.01 - (d as f64) * 0.005)
            .collect();
        let tensor = TensorValue::from_data(data, &[d]).unwrap();

        group.bench_with_input(BenchmarkId::new("scalar", d), &tensor, |b, t| {
            b.iter(|| hadamard(black_box(t)).unwrap())
        });

        group.bench_with_input(BenchmarkId::new("avx2", d), &tensor, |b, t| {
            b.iter(|| hadamard_avx2(black_box(t)).unwrap())
        });
    }

    // Batch benchmark: 32 rows × 128 dims (realistic KV cache slice)
    let batch_data: Vec<f64> = (0..32 * 128).map(|i| (i as f64) * 0.001).collect();
    let batch = TensorValue::from_data(batch_data, &[32, 128]).unwrap();

    group.bench_with_input(BenchmarkId::new("scalar", "32x128"), &batch, |b, t| {
        b.iter(|| hadamard(black_box(t)).unwrap())
    });

    group.bench_with_input(BenchmarkId::new("avx2", "32x128"), &batch, |b, t| {
        b.iter(|| hadamard_avx2(black_box(t)).unwrap())
    });

    group.finish();
}

/// B5.L5: Fused hadamard_quantize vs separate hadamard → quantize pipeline.
fn bench_fused_vs_separate(c: &mut Criterion) {
    let mut group = c.benchmark_group("fused_vs_separate");

    for d in [64, 128, 256, 512] {
        let data: Vec<f64> = (0..d)
            .map(|i| (i as f64) * 0.01 - (d as f64) * 0.005)
            .collect();
        let tensor = TensorValue::from_data(data, &[d]).unwrap();

        group.bench_with_input(BenchmarkId::new("separate", d), &tensor, |b, t| {
            b.iter(|| {
                let h = hadamard(black_box(t)).unwrap();
                QuantizedValue::quantize(&h, 4).unwrap()
            })
        });

        group.bench_with_input(BenchmarkId::new("fused", d), &tensor, |b, t| {
            b.iter(|| hadamard_quantize(black_box(t), 4).unwrap())
        });
    }

    // Batch: 32 rows × 128 dims
    let batch_data: Vec<f64> = (0..32 * 128).map(|i| (i as f64) * 0.001).collect();
    let batch = TensorValue::from_data(batch_data, &[32, 128]).unwrap();

    group.bench_with_input(BenchmarkId::new("separate", "32x128"), &batch, |b, t| {
        b.iter(|| {
            let h = hadamard(black_box(t)).unwrap();
            QuantizedValue::quantize(&h, 4).unwrap()
        })
    });

    group.bench_with_input(BenchmarkId::new("fused", "32x128"), &batch, |b, t| {
        b.iter(|| hadamard_quantize(black_box(t), 4).unwrap())
    });

    group.finish();
}

criterion_group!(benches, bench_hadamard, bench_fused_vs_separate);
criterion_main!(benches);
