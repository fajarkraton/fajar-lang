# Fajar Lang Tutorials

Learn Fajar Lang step by step — from Hello World to OS development and ML.

## Lessons

| # | Tutorial | Topics | Run |
|---|----------|--------|-----|
| 1 | [Hello World](01_hello_world.fj) | Variables, types, println, constants | `fj run tutorials/01_hello_world.fj` |
| 2 | [Control Flow](02_control_flow.fj) | if/else, while, for, match, guards | `fj run tutorials/02_control_flow.fj` |
| 3 | [Data Structures](03_data_structures.fj) | Arrays, structs, enums, tuples, maps | `fj run tutorials/03_data_structures.fj` |
| 4 | [Functions & Closures](04_functions_closures.fj) | Closures, higher-order, map/filter/fold | `fj run tutorials/04_functions_closures.fj` |
| 5 | [Error Handling](05_error_handling.fj) | Option, Result, match, chaining | `fj run tutorials/05_error_handling.fj` |
| 6 | [Traits](06_traits.fj) | trait, impl, polymorphism, builder | `fj run tutorials/06_traits.fj` |
| 7 | [Async](07_async.fj) | async/await, join, spawn | `fj run tutorials/07_async.fj` |
| 8 | [OS Development](08_os_kernel.fj) | @kernel, syscalls, page tables | `fj run tutorials/08_os_kernel.fj` |
| 9 | [ML & Tensors](09_ml_tensors.fj) | Neural networks, ReLU, softmax | `fj run tutorials/09_ml_tensors.fj` |
| 10 | [Complete Project](10_project.fj) | Statistics library, all features | `fj run tutorials/10_project.fj` |

## Run All Tutorials

```bash
for f in tutorials/[0-9]*.fj; do
    echo "=== $(basename $f) ==="
    fj run "$f"
    echo ""
done
```

## Prerequisites

- [Fajar Lang](https://github.com/fajarkraton/fajar-lang) v6.1.0+
- `cargo install fajar-lang` or build from source
