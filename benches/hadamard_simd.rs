//! Benchmark: Hadamard scalar vs AVX2
//!
//! B5.L2.2 verification: `cargo bench --bench hadamard_simd`
//! Target: AVX2 path ≥ 2x speedup over scalar at D ≥ 128.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use fajar_lang::runtime::ml::TensorValue;
use fajar_lang::runtime::ml::ops::{hadamard, hadamard_avx2};

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

criterion_group!(benches, bench_hadamard);
criterion_main!(benches);
