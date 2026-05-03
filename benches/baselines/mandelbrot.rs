// Rust baseline: 256x256 mandelbrot escape iteration.
// Compile: rustc -O mandelbrot.rs -o mandelbrot

fn iterations(cx: f64, cy: f64, max_iter: i64) -> i64 {
    let mut x = 0.0_f64;
    let mut y = 0.0_f64;
    for i in 0..max_iter {
        let x2 = x * x;
        let y2 = y * y;
        if x2 + y2 > 4.0 {
            return i;
        }
        let new_x = x2 - y2 + cx;
        y = 2.0 * x * y + cy;
        x = new_x;
    }
    max_iter
}

fn main() {
    let width: i64 = 256;
    let height: i64 = 256;
    let max_iter: i64 = 200;
    let mut checksum: i64 = 0;

    for py in 0..height {
        for px in 0..width {
            let cx = (px as f64) / (width as f64) * 3.5 - 2.5;
            let cy = (py as f64) / (height as f64) * 2.0 - 1.0;
            checksum += iterations(cx, cy, max_iter);
        }
    }

    println!("mandelbrot checksum = {checksum}");
}
