/* C baseline: 64x64 matrix multiply x 100 iterations.
 * Compile: gcc -O2 matrix_multiply.c -o matrix_multiply
 */
#include <stdio.h>
#include <stdint.h>

int main(void) {
    const int64_t n = 64;
    const int64_t total = n * n;
    int64_t checksum = 0;

    for (int iter = 0; iter < 100; iter++) {
        int64_t a[4096];
        int64_t b[4096];
        int64_t c[4096];

        for (int64_t i = 0; i < total; i++) {
            a[i] = (i * 3 + 7) % 1000;
            b[i] = (i * 5 + 11) % 1000;
            c[i] = 0;
        }

        for (int64_t row = 0; row < n; row++) {
            for (int64_t col = 0; col < n; col++) {
                int64_t sum = 0;
                for (int64_t k = 0; k < n; k++) {
                    sum += a[row * n + k] * b[k * n + col];
                }
                c[row * n + col] = sum;
            }
        }

        checksum += c[0] + c[total - 1];
    }

    printf("matmul checksum = %ld\n", (long)checksum);
    return 0;
}
