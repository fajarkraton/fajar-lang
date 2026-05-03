/* C baseline: 256x256 mandelbrot escape iteration.
 * Compile: gcc -O2 mandelbrot.c -o mandelbrot
 */
#include <stdio.h>
#include <stdint.h>

static int64_t iterations(double cx, double cy, int64_t max_iter) {
    double x = 0.0, y = 0.0;
    for (int64_t i = 0; i < max_iter; i++) {
        double x2 = x * x;
        double y2 = y * y;
        if (x2 + y2 > 4.0) {
            return i;
        }
        double new_x = x2 - y2 + cx;
        y = 2.0 * x * y + cy;
        x = new_x;
    }
    return max_iter;
}

int main(void) {
    const int64_t width = 256, height = 256, max_iter = 200;
    int64_t checksum = 0;

    for (int64_t py = 0; py < height; py++) {
        for (int64_t px = 0; px < width; px++) {
            double cx = (double)px / (double)width * 3.5 - 2.5;
            double cy = (double)py / (double)height * 2.0 - 1.0;
            checksum += iterations(cx, cy, max_iter);
        }
    }

    printf("mandelbrot checksum = %ld\n", (long)checksum);
    return 0;
}
