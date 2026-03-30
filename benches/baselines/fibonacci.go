// Go baseline: fibonacci(20) x 1000 iterations
// Run: go run fibonacci.go
package main

import (
	"fmt"
	"time"
)

func fib(n int64) int64 {
	if n <= 1 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	start := time.Now()
	var result int64
	for i := 0; i < 1000; i++ {
		result = fib(20)
	}
	elapsed := time.Since(start)
	fmt.Printf("fib(20) = %d, 1000 iterations in %.6f s (%.3f us/iter)\n",
		result, elapsed.Seconds(), float64(elapsed.Microseconds())/1000)
}
