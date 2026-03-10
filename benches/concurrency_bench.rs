//! Concurrency benchmarks for Fajar Lang v0.3.
//!
//! Sprint 13.4: criterion benchmarks for concurrency primitives.
//! Measures channel throughput, mutex contention, atomic operations,
//! and async task spawn/join overhead.

#[cfg(feature = "native")]
mod benches {
    use criterion::{criterion_group, Criterion};
    use fajar_lang::codegen::cranelift::runtime_fns::{
        fj_rt_atomic_add, fj_rt_atomic_cas, fj_rt_atomic_free, fj_rt_atomic_load, fj_rt_atomic_new,
        fj_rt_channel_free, fj_rt_channel_new, fj_rt_channel_recv, fj_rt_channel_send,
        fj_rt_executor_free, fj_rt_executor_new, fj_rt_executor_run, fj_rt_executor_spawn,
        fj_rt_future_new, fj_rt_future_set_result, fj_rt_mutex_free, fj_rt_mutex_lock,
        fj_rt_mutex_new, fj_rt_mutex_store, fj_rt_thread_join, fj_rt_thread_spawn,
    };

    /// Benchmark: channel throughput — send N messages through unbounded channel.
    fn bench_channel_throughput(c: &mut Criterion) {
        c.bench_function("channel_throughput_10k_msgs", |b| {
            b.iter(|| {
                let ch = fj_rt_channel_new();
                for i in 0..10_000i64 {
                    fj_rt_channel_send(ch, i);
                }
                let mut sum = 0i64;
                for _ in 0..10_000 {
                    sum += fj_rt_channel_recv(ch);
                }
                assert_eq!(sum, 49_995_000);
                fj_rt_channel_free(ch);
            })
        });
    }

    /// Benchmark: mutex contention — N threads incrementing a shared counter.
    fn bench_mutex_contention(c: &mut Criterion) {
        // Single-threaded lock/unlock cycle to measure raw mutex overhead
        c.bench_function("mutex_lock_unlock_10k_ops", |b| {
            b.iter(|| {
                let m = fj_rt_mutex_new(0);
                for i in 0..10_000i64 {
                    let _ = fj_rt_mutex_lock(m);
                    fj_rt_mutex_store(m, i);
                }
                let val = fj_rt_mutex_lock(m);
                assert_eq!(val, 9_999);
                fj_rt_mutex_free(m);
            })
        });

        // Multi-threaded: 4 threads each incrementing counter 1000 times
        // Uses thread_spawn with a function that does atomic add
        c.bench_function("mutex_contention_4_threads", |b| {
            b.iter(|| {
                let counter = fj_rt_atomic_new(0);
                let counter_addr = counter as i64;

                // Spawn 4 threads, each adds 1000 to the atomic counter
                let mut handles = Vec::new();
                for _ in 0..4 {
                    extern "C" fn worker(counter_ptr: i64) -> i64 {
                        let ptr = counter_ptr as *mut u8;
                        for _ in 0..1000 {
                            fj_rt_atomic_add(ptr, 1);
                        }
                        0
                    }
                    let h = fj_rt_thread_spawn(worker as *const u8, counter_addr);
                    handles.push(h);
                }

                for h in handles {
                    fj_rt_thread_join(h);
                }

                let val = fj_rt_atomic_load(counter);
                assert_eq!(val, 4000);
                fj_rt_atomic_free(counter);
            })
        });
    }

    /// Benchmark: atomic operations — CAS loop and atomic adds.
    fn bench_atomic_operations(c: &mut Criterion) {
        c.bench_function("atomic_add_100k_ops", |b| {
            b.iter(|| {
                let a = fj_rt_atomic_new(0);
                for _ in 0..100_000 {
                    fj_rt_atomic_add(a, 1);
                }
                let val = fj_rt_atomic_load(a);
                assert_eq!(val, 100_000);
                fj_rt_atomic_free(a);
            })
        });

        c.bench_function("atomic_cas_100k_ops", |b| {
            b.iter(|| {
                let a = fj_rt_atomic_new(0);
                for i in 0..100_000i64 {
                    // CAS loop: expected = i, desired = i + 1
                    loop {
                        let old = fj_rt_atomic_cas(a, i, i + 1);
                        if old == i {
                            break;
                        }
                    }
                }
                let val = fj_rt_atomic_load(a);
                assert_eq!(val, 100_000);
                fj_rt_atomic_free(a);
            })
        });
    }

    /// Benchmark: async task spawn/join overhead.
    fn bench_async_spawn_join(c: &mut Criterion) {
        c.bench_function("async_spawn_join_1k_tasks", |b| {
            b.iter(|| {
                let exec = fj_rt_executor_new();

                // Spawn 1000 futures, each immediately resolved
                for i in 0..1_000i64 {
                    let future = fj_rt_future_new();
                    fj_rt_future_set_result(future, i);
                    fj_rt_executor_spawn(exec, future);
                }

                let completed = fj_rt_executor_run(exec);
                assert_eq!(completed, 1_000);
                fj_rt_executor_free(exec);
            })
        });
    }

    criterion_group!(
        concurrency_benches,
        bench_channel_throughput,
        bench_mutex_contention,
        bench_atomic_operations,
        bench_async_spawn_join,
    );
}

#[cfg(feature = "native")]
criterion::criterion_main!(benches::concurrency_benches);

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("Concurrency benchmarks require --features native");
}
