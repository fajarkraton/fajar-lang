# Fajar Lang Style Guide

Official coding conventions for Fajar Lang programs.

## Naming

| Item | Convention | Example |
|------|-----------|---------|
| Variables, functions | `snake_case` | `let total_count = 0` |
| Types, structs, enums | `PascalCase` | `struct SensorData` |
| Constants | `SCREAMING_SNAKE_CASE` | `const MAX_RETRIES: i32 = 5` |
| Modules | `snake_case` | `mod signal_processing` |
| Trait names | `PascalCase` (adjective when possible) | `trait Displayable` |
| Enum variants | `PascalCase` | `enum Color { Red, Blue }` |
| Type parameters | Single uppercase letter | `fn first<T>(items: [T]) -> T` |

## Formatting

- **Indentation:** 4 spaces (no tabs)
- **Line length:** 100 characters maximum
- **Braces:** Opening brace on the same line
- **Trailing commas:** Use in multi-line lists
- **Blank lines:** One between top-level items, none between related single-line items

```fajar
fn process_data(input: [f64], threshold: f64) -> Result<[f64], Error> {
    let mut results: [f64] = []

    for value in input {
        if value > threshold {
            results.push(value * 2.0)
        }
    }

    Ok(results)
}
```

## Functions

- Keep functions under 50 lines
- Use descriptive names: `calculate_distance` not `calc` or `cd`
- Return type is always explicit for public functions
- Use the pipeline operator for sequential transformations:

```fajar
let result = raw_data
    |> normalize
    |> filter_outliers
    |> compute_average
```

## Error Handling

- Use `Result<T, E>` for operations that can fail
- Propagate errors with `?` rather than manual matching when appropriate
- Never silently ignore errors
- Provide context in error messages:

```fajar
fn read_config(path: str) -> Result<Config, Error> {
    let content = read_file(path)?
    parse_toml(content)
}
```

## Context Annotations

- Always annotate functions that use hardware or tensor operations
- Place the annotation on the line directly before `fn`
- Use the most restrictive context that works:

```fajar
// Preferred: explicit context
@device
fn infer(model: Model, input: Tensor) -> Tensor {
    model.forward(input)
}

// Default: @safe (no annotation needed for pure computation)
fn add(a: i32, b: i32) -> i32 { a + b }
```

## Imports

- Group imports: standard library first, then external, then local
- One `use` per line for clarity
- Alphabetize within each group:

```fajar
use std::collections::HashMap
use std::io::read_file

use nn::layer::Dense
use nn::tensor::zeros

use crate::config::Settings
use crate::utils::validate
```

## Documentation

- All public items must have `///` doc comments
- First line is a short summary (one sentence)
- Include examples for non-trivial functions:

```fajar
/// Computes the sigmoid activation function.
///
/// Maps any real number to a value between 0 and 1.
///
/// Example:
///   let y = sigmoid(0.0)  // returns 0.5
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}
```

## Project Structure

```
my_project/
  fj.toml          # project manifest
  src/
    main.fj        # entry point
    lib.fj         # library root (if library)
    utils.fj       # utility module
  tests/
    test_main.fj   # test files
  examples/
    demo.fj        # example programs
```
