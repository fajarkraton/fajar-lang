// Rust baseline: 64×64 matrix multiply x 100 iterations.
// Compile: rustc -O matrix_multiply.rs -o matrix_multiply

fn main() {
    let n: i64 = 64;
    let total = (n * n) as usize;
    let mut checksum: i64 = 0;

    for _ in 0..100 {
        let mut a: Vec<i64> = vec![0; total];
        let mut b: Vec<i64> = vec![0; total];
        let mut c: Vec<i64> = vec![0; total];

        for i in 0..total as i64 {
            a[i as usize] = (i * 3 + 7) % 1000;
            b[i as usize] = (i * 5 + 11) % 1000;
        }

        for row in 0..n {
            for col in 0..n {
                let mut sum: i64 = 0;
                for k in 0..n {
                    sum += a[(row * n + k) as usize] * b[(k * n + col) as usize];
                }
                c[(row * n + col) as usize] = sum;
            }
        }

        checksum += c[0] + c[total - 1];
    }

    println!("matmul checksum = {checksum}");
}
