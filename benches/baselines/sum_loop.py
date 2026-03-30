#!/usr/bin/env python3
"""Python baseline: sum 1..10000 x 10000 iterations"""
import time

start = time.perf_counter()
total = 0
for _ in range(10000):
    s = 0
    for i in range(10000):
        s += i
    total = s
elapsed = time.perf_counter() - start
print(f"sum(0..10000) = {total}, 10000 iterations in {elapsed:.6f} s ({elapsed*1e6/10000:.3f} us/iter)")
