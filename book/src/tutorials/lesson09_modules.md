# Lesson 9: Modules and Project Structure

## Objectives

By the end of this lesson, you will be able to:

- Organize code with `mod` and `use`
- Control visibility with `pub`
- Structure a multi-file Fajar Lang project
- Configure a project with `fj.toml`

## Why Modules?

As programs grow, putting everything in one file becomes unmanageable. Modules let you split code into logical units with clear boundaries and controlled visibility.

## Defining Modules

Use `mod` to create a module. Everything inside is private by default.

```fajar
mod math {
    pub fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    pub fn multiply(a: i64, b: i64) -> i64 {
        a * b
    }

    fn internal_helper() -> i64 {
        // This is private -- only accessible within `math`
        42
    }
}

fn main() {
    println(math::add(3, 4))        // 7
    println(math::multiply(3, 4))   // 12
    // math::internal_helper()       // ERROR: function is private
}
```

## The `use` Keyword

Bring module items into scope with `use` to avoid repeating the module path:

```fajar
mod geometry {
    pub const PI: f64 = 3.14159265358979

    pub fn circle_area(radius: f64) -> f64 {
        PI * radius * radius
    }

    pub fn circle_circumference(radius: f64) -> f64 {
        2.0 * PI * radius
    }
}

use geometry::circle_area
use geometry::circle_circumference

fn main() {
    println(circle_area(5.0))            // 78.53981633974483
    println(circle_circumference(5.0))   // 31.41592653589793
}
```

## Visibility Rules

- By default, all items in a module are **private**
- `pub` makes an item accessible from outside the module
- Struct fields can individually be `pub` or private

```fajar
mod account {
    pub struct User {
        pub name: str,
        email: str      // private field
    }

    pub fn new_user(name: str, email: str) -> User {
        User { name: name, email: email }
    }

    pub fn get_email(user: &User) -> str {
        user.email   // accessible within the module
    }
}

fn main() {
    let user = account::new_user("Fajar", "fajar@example.com")
    println(user.name)                    // OK: name is pub
    // println(user.email)               // ERROR: email is private
    println(account::get_email(&user))    // OK: accessed via pub function
}
```

## Multi-File Projects

For larger projects, each module lives in its own file. Here is a typical layout:

```
my_project/
    fj.toml
    src/
        main.fj
        math.fj
        utils.fj
        models/
            user.fj
            product.fj
```

### fj.toml -- Project Configuration

Every Fajar Lang project has a `fj.toml` at the root:

```toml
[package]
name = "my_project"
version = "0.1.0"
author = "Fajar"
edition = "2026"

[dependencies]
fj-math = "1.0"
fj-json = "1.0"
```

### main.fj -- Entry Point

```fajar
mod math
mod utils

use math::add
use utils::greet

fn main() {
    greet("World")
    println(add(2, 3))
}
```

### math.fj

```fajar
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn subtract(a: i64, b: i64) -> i64 {
    a - b
}
```

### utils.fj

```fajar
pub fn greet(name: str) {
    println(f"Hello, {name}!")
}
```

## Creating a New Project

Use the `fj new` command to scaffold a project:

```bash
fj new my_project
cd my_project
fj run
```

This creates the directory structure, `fj.toml`, and a starter `main.fj`.

## Nested Modules

Modules can be nested for deeper organization:

```fajar
mod app {
    pub mod config {
        pub const VERSION: str = "1.0.0"
        pub const DEBUG: bool = false
    }

    pub mod logger {
        pub fn info(msg: str) {
            println(f"[INFO] {msg}")
        }

        pub fn error(msg: str) {
            println(f"[ERROR] {msg}")
        }
    }
}

fn main() {
    println(f"Version: {app::config::VERSION}")
    app::logger::info("Application started")
    app::logger::error("Something went wrong")
}
```

**Expected output:**

```
Version: 1.0.0
[INFO] Application started
[ERROR] Something went wrong
```

## Exercises

### Exercise 9.1: Math Module (*)

Create a module `math_utils` with public functions `square(x: f64) -> f64` and `cube(x: f64) -> f64`. Use them from `main` and print the square and cube of `3.0`.

**Expected output:**

```
Square: 9.0
Cube: 27.0
```

### Exercise 9.2: Inventory System (**)

Design a module `inventory` with a public struct `Item { pub name: str, pub price: f64, quantity: i64 }` (quantity is private). Provide public functions `new_item(name, price, qty) -> Item`, `total_value(item) -> f64`, and `restock(item, amount) -> Item`. Demonstrate creating an item, restocking it, and printing the total value.

**Expected output:**

```
Widget: 29.97
After restock: 79.92
```
