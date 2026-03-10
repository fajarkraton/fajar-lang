# Modules

## Module Declaration

Organize code into modules:

```fajar
mod math {
    pub fn square(x: i64) -> i64 { x * x }
    pub fn cube(x: i64) -> i64 { x * x * x }

    fn helper() { /* private */ }
}
```

## Using Module Items

```fajar
use math::square

let result = square(5)         // 25
let result2 = math::cube(3)   // 27 (qualified path)
```

## Visibility

Items are private by default. Use `pub` to make them public:

```fajar
mod shapes {
    pub struct Circle {
        pub radius: f64        // public field
    }

    pub fn new_circle(r: f64) -> Circle {
        Circle { radius: r }
    }

    fn internal_helper() {     // private function
        // only accessible within this module
    }
}
```

## Standard Library Modules

Fajar's standard library is organized into modules:

```fajar
use std::math    // PI, E, sqrt, sin, cos, etc.
use std::io      // print, println, read_file, write_file
use std::string  // trim, split, replace, contains, etc.
```

## The `extern` Keyword

Interface with C code via FFI:

```fajar
extern "C" {
    fn printf(format: *const u8, ...) -> i32
}
```
