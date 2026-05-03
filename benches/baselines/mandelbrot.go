// Go baseline: 256x256 mandelbrot escape iteration.
// Build: go build -o mandelbrot mandelbrot.go

package main

import "fmt"

func iterations(cx, cy float64, maxIter int64) int64 {
	var x, y float64 = 0.0, 0.0
	for i := int64(0); i < maxIter; i++ {
		x2 := x * x
		y2 := y * y
		if x2+y2 > 4.0 {
			return i
		}
		newX := x2 - y2 + cx
		y = 2.0*x*y + cy
		x = newX
	}
	return maxIter
}

func main() {
	const width int64 = 256
	const height int64 = 256
	const maxIter int64 = 200
	var checksum int64 = 0

	for py := int64(0); py < height; py++ {
		for px := int64(0); px < width; px++ {
			cx := float64(px)/float64(width)*3.5 - 2.5
			cy := float64(py)/float64(height)*2.0 - 1.0
			checksum += iterations(cx, cy, maxIter)
		}
	}

	fmt.Printf("mandelbrot checksum = %d\n", checksum)
}
