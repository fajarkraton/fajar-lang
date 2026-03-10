// Rust baseline: fibonacci(20) — compile with: rustc -O -o fib_rs fibonacci.rs
use std::time::Instant;

fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}

fn main() {
    let start = Instant::now();
    let mut result = 0i64;
    for _ in 0..1000 {
        result = fib(20);
    }
    let elapsed = start.elapsed();
    println!(
        "fib(20) = {result}, 1000 iterations in {:.6} s ({:.3} us/iter)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / 1000.0
    );
}
