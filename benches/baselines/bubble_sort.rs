// Rust baseline: bubble sort — compile with: rustc -O -o bubble_rs bubble_sort.rs
use std::time::Instant;

fn bubble_sort(arr: &mut [i64]) {
    let n = arr.len();
    for i in 0..n.saturating_sub(1) {
        for j in 0..n - 1 - i {
            if arr[j] > arr[j + 1] {
                arr.swap(j, j + 1);
            }
        }
    }
}

fn main() {
    let start = Instant::now();
    let mut result = 0i64;
    for _ in 0..1000 {
        let mut arr = [64i64, 34, 25, 12, 22, 11, 90, 1, 45, 78];
        bubble_sort(&mut arr);
        result = arr[0];
    }
    let elapsed = start.elapsed();
    println!(
        "bubble_sort first = {result}, 1000 iterations in {:.6} s ({:.3} us/iter)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / 1000.0
    );
}
