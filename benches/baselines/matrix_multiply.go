// Go baseline: 64x64 matrix multiply x 100 iterations.
// Build: go build -o matrix_multiply matrix_multiply.go

package main

import "fmt"

func main() {
	const n int64 = 64
	const total = n * n
	var checksum int64 = 0

	for iter := 0; iter < 100; iter++ {
		a := make([]int64, total)
		b := make([]int64, total)
		c := make([]int64, total)

		for i := int64(0); i < total; i++ {
			a[i] = (i*3 + 7) % 1000
			b[i] = (i*5 + 11) % 1000
		}

		for row := int64(0); row < n; row++ {
			for col := int64(0); col < n; col++ {
				var sum int64 = 0
				for k := int64(0); k < n; k++ {
					sum += a[row*n+k] * b[k*n+col]
				}
				c[row*n+col] = sum
			}
		}

		checksum += c[0] + c[total-1]
	}

	fmt.Printf("matmul checksum = %d\n", checksum)
}
