# Math Functions

The `std::math` module provides mathematical constants and functions. All functions are available globally without imports.

## Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `PI` | 3.141592653589793 | Ratio of circumference to diameter |
| `E` | 2.718281828459045 | Base of the natural logarithm |

```fajar
let circumference = 2.0 * PI * radius
let growth = E ** rate
```

## Function Reference

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `abs(x: f64) -> f64` | Absolute value |
| `sqrt` | `sqrt(x: f64) -> f64` | Square root |
| `pow` | `pow(base: f64, exp: f64) -> f64` | Exponentiation |
| `sin` | `sin(x: f64) -> f64` | Sine (radians) |
| `cos` | `cos(x: f64) -> f64` | Cosine (radians) |
| `tan` | `tan(x: f64) -> f64` | Tangent (radians) |
| `floor` | `floor(x: f64) -> f64` | Round down to nearest integer |
| `ceil` | `ceil(x: f64) -> f64` | Round up to nearest integer |
| `round` | `round(x: f64) -> f64` | Round to nearest integer |
| `clamp` | `clamp(x: f64, min: f64, max: f64) -> f64` | Constrain to range |
| `min` | `min(a: f64, b: f64) -> f64` | Smaller of two values |
| `max` | `max(a: f64, b: f64) -> f64` | Larger of two values |
| `log` | `log(x: f64) -> f64` | Natural logarithm (base e) |
| `exp` | `exp(x: f64) -> f64` | e raised to the power x |

## Examples

### `abs`

Returns the absolute (non-negative) value.

```fajar
println(abs(-5.0))   // 5.0
println(abs(3.14))   // 3.14
```

### `sqrt`

Returns the square root. Panics on negative input.

```fajar
println(sqrt(16.0))  // 4.0
println(sqrt(2.0))   // 1.4142135623730951
```

### `pow`

Raises a base to an exponent. Also available as the `**` operator.

```fajar
println(pow(2.0, 10.0))  // 1024.0
println(pow(3.0, 0.5))   // 1.7320508075688772
// equivalent: 2.0 ** 10.0
```

### `sin` / `cos` / `tan`

Trigonometric functions operating on radians.

```fajar
println(sin(PI / 2.0))   // 1.0
println(cos(0.0))         // 1.0
println(tan(PI / 4.0))   // ~1.0
```

### `floor` / `ceil` / `round`

Rounding functions.

```fajar
println(floor(3.7))   // 3.0
println(ceil(3.2))    // 4.0
println(round(3.5))   // 4.0
println(round(3.4))   // 3.0
```

### `clamp`

Constrains a value to a range.

```fajar
println(clamp(15.0, 0.0, 10.0))   // 10.0
println(clamp(-5.0, 0.0, 10.0))   // 0.0
println(clamp(5.0, 0.0, 10.0))    // 5.0
```

### `min` / `max`

Returns the smaller or larger of two values.

```fajar
println(min(3.0, 7.0))   // 3.0
println(max(3.0, 7.0))   // 7.0
```

### `log` / `exp`

Natural logarithm and exponential.

```fajar
println(log(E))          // 1.0
println(log(1.0))        // 0.0
println(exp(1.0))        // 2.718281828459045
println(exp(0.0))        // 1.0
```

## Practical Example

Computing distance between two points:

```fajar
struct Point { x: f64, y: f64 }

fn distance(a: Point, b: Point) -> f64 {
    let dx = a.x - b.x
    let dy = a.y - b.y
    sqrt(dx ** 2.0 + dy ** 2.0)
}

let p1 = Point { x: 0.0, y: 0.0 }
let p2 = Point { x: 3.0, y: 4.0 }
println(distance(p1, p2))  // 5.0
```
