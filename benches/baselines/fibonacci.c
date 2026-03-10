// C baseline: fibonacci(20) — compile with: gcc -O2 -o fib fibonacci.c
#include <stdio.h>
#include <time.h>

long fib(long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

int main(void) {
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);

    long result = 0;
    for (int i = 0; i < 1000; i++) {
        result = fib(20);
    }

    clock_gettime(CLOCK_MONOTONIC, &end);
    double elapsed = (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec) / 1e9;
    printf("fib(20) = %ld, 1000 iterations in %.6f s (%.3f us/iter)\n",
           result, elapsed, elapsed * 1e6 / 1000);
    return 0;
}
