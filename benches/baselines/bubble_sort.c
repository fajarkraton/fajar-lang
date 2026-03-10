// C baseline: bubble sort — compile with: gcc -O2 -o bubble bubble_sort.c
#include <stdio.h>
#include <time.h>

void bubble_sort(long arr[], int n) {
    for (int i = 0; i < n - 1; i++) {
        for (int j = 0; j < n - 1 - i; j++) {
            if (arr[j] > arr[j + 1]) {
                long tmp = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = tmp;
            }
        }
    }
}

int main(void) {
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);

    long result = 0;
    for (int iter = 0; iter < 1000; iter++) {
        long arr[] = {64, 34, 25, 12, 22, 11, 90, 1, 45, 78};
        bubble_sort(arr, 10);
        result = arr[0]; // 1 (sorted first element)
    }

    clock_gettime(CLOCK_MONOTONIC, &end);
    double elapsed = (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec) / 1e9;
    printf("bubble_sort first = %ld, 1000 iterations in %.6f s (%.3f us/iter)\n",
           result, elapsed, elapsed * 1e6 / 1000);
    return 0;
}
