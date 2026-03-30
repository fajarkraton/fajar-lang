// Go baseline: sum 1..10000 x 10000 iterations
// Run: go run sum_loop.go
package main

import (
	"fmt"
	"time"
)

func main() {
	start := time.Now()
	var total int64
	for iter := 0; iter < 10000; iter++ {
		var sum int64
		for i := int64(0); i < 10000; i++ {
			sum += i
		}
		total = sum
	}
	elapsed := time.Since(start)
	fmt.Printf("sum(0..10000) = %d, 10000 iterations in %.6f s (%.3f us/iter)\n",
		total, elapsed.Seconds(), float64(elapsed.Microseconds())/10000)
}
