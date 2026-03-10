// C baseline: sum 1..10000 — compile with: gcc -O2 -o sum sum_loop.c
#include <stdio.h>
#include <time.h>

int main(void) {
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);

    long total = 0;
    for (int iter = 0; iter < 10000; iter++) {
        long sum = 0;
        for (long i = 0; i < 10000; i++) {
            sum += i;
        }
        total = sum; // prevent optimization
    }

    clock_gettime(CLOCK_MONOTONIC, &end);
    double elapsed = (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec) / 1e9;
    printf("sum(0..10000) = %ld, 10000 iterations in %.6f s (%.3f us/iter)\n",
           total, elapsed, elapsed * 1e6 / 10000);
    return 0;
}
