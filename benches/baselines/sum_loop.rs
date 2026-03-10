// Rust baseline: sum 1..10000 — compile with: rustc -O -o sum_rs sum_loop.rs
use std::time::Instant;

fn main() {
    let start = Instant::now();
    let mut total = 0i64;
    for _ in 0..10000 {
        let mut sum = 0i64;
        for i in 0..10000 {
            sum += i;
        }
        total = sum;
    }
    let elapsed = start.elapsed();
    println!(
        "sum(0..10000) = {total}, 10000 iterations in {:.6} s ({:.3} us/iter)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / 10000.0
    );
}
