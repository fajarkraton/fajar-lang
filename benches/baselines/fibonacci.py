#!/usr/bin/env python3
"""Python baseline: fibonacci(20) x 1000 iterations"""
import time

def fib(n):
    if n <= 1: return n
    return fib(n - 1) + fib(n - 2)

start = time.perf_counter()
result = 0
for _ in range(1000):
    result = fib(20)
elapsed = time.perf_counter() - start
print(f"fib(20) = {result}, 1000 iterations in {elapsed:.6f} s ({elapsed*1e6/1000:.3f} us/iter)")
