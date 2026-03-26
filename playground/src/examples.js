// ═══════════════════════════════════════════════════
// Example Gallery — curated playground examples
// ═══════════════════════════════════════════════════

export const examples = [
  {
    slug: 'hello-world', title: 'Hello World', difficulty: 'beginner', category: 'basics',
    code: `// Hello World in Fajar Lang
fn main() {
    let name = "Fajar Lang"
    println(f"Hello, {name}!")
    println("The language for embedded ML + OS integration")
}`,
  },
  {
    slug: 'fibonacci', title: 'Fibonacci', difficulty: 'beginner', category: 'basics',
    code: `// Fibonacci with recursion and loop
fn fib_recursive(n: i32) -> i32 {
    if n <= 1 { return n }
    fib_recursive(n - 1) + fib_recursive(n - 2)
}

fn fib_iterative(n: i32) -> i32 {
    let mut a = 0
    let mut b = 1
    for _ in 0..n {
        let tmp = a + b
        a = b
        b = tmp
    }
    a
}

fn main() {
    for i in 0..15 {
        println(f"fib({i}) = {fib_recursive(i)}")
    }
}`,
  },
  {
    slug: 'pattern-matching', title: 'Pattern Matching', difficulty: 'beginner', category: 'basics',
    code: `enum Shape {
    Circle(f64),
    Rect(f64, f64),
    Triangle(f64, f64, f64),
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14159 * r * r,
        Shape::Rect(w, h) => w * h,
        Shape::Triangle(a, b, c) => {
            let s = (a + b + c) / 2.0
            sqrt(s * (s - a) * (s - b) * (s - c))
        },
    }
}

fn main() {
    let shapes = [
        Shape::Circle(5.0),
        Shape::Rect(4.0, 6.0),
        Shape::Triangle(3.0, 4.0, 5.0),
    ]
    for shape in shapes {
        println(f"Area: {area(shape)}")
    }
}`,
  },
  {
    slug: 'pipeline', title: 'Pipeline Operator', difficulty: 'beginner', category: 'basics',
    code: `// The |> operator chains function calls left-to-right
fn double(x: i32) -> i32 { x * 2 }
fn add_ten(x: i32) -> i32 { x + 10 }
fn square(x: i32) -> i32 { x * x }
fn is_even(x: i32) -> bool { x % 2 == 0 }

fn main() {
    // Without pipeline (read inside-out):
    let a = square(add_ten(double(5)))

    // With pipeline (read left-to-right):
    let b = 5 |> double |> add_ten |> square

    println(f"Result: {a}")
    println(f"Same:   {b}")
    println(f"Is even: {b |> is_even}")
}`,
  },
  {
    slug: 'error-handling', title: 'Error Handling', difficulty: 'intermediate', category: 'safety',
    code: `fn divide(a: f64, b: f64) -> Result<f64, str> {
    if b == 0.0 { return Err("division by zero") }
    Ok(a / b)
}

fn sqrt_safe(x: f64) -> Result<f64, str> {
    if x < 0.0 { return Err("negative input") }
    Ok(sqrt(x))
}

fn compute(a: f64, b: f64) -> Result<f64, str> {
    let ratio = divide(a, b)?
    let root = sqrt_safe(ratio)?
    Ok(root)
}

fn main() {
    let tests = [(100.0, 4.0), (9.0, 0.0), (-16.0, 1.0)]
    for (a, b) in tests {
        match compute(a, b) {
            Ok(v) => println(f"compute({a}, {b}) = {v}"),
            Err(e) => println(f"compute({a}, {b}) error: {e}"),
        }
    }
}`,
  },
  {
    slug: 'structs-traits', title: 'Structs & Traits', difficulty: 'intermediate', category: 'basics',
    code: `trait Drawable {
    fn draw(self) -> str
    fn area(self) -> f64
}

struct Circle { radius: f64 }
struct Square { side: f64 }

impl Drawable for Circle {
    fn draw(self) -> str { f"Circle(r={self.radius})" }
    fn area(self) -> f64 { 3.14159 * self.radius * self.radius }
}

impl Drawable for Square {
    fn draw(self) -> str { f"Square(s={self.side})" }
    fn area(self) -> f64 { self.side * self.side }
}

fn print_shape(shape: dyn Drawable) {
    println(f"{shape.draw()} -> area = {shape.area()}")
}

fn main() {
    print_shape(Circle { radius: 5.0 })
    print_shape(Square { side: 4.0 })
}`,
  },
  {
    slug: 'iterators', title: 'Iterators & Closures', difficulty: 'intermediate', category: 'basics',
    code: `fn main() {
    let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

    // Filter even numbers, double them, collect
    let result = numbers
        .iter()
        .filter(|x| x % 2 == 0)
        .map(|x| x * 2)
        .collect()

    println(f"Doubled evens: {result}")

    // Sum with fold
    let sum = numbers.iter().fold(0, |acc, x| acc + x)
    println(f"Sum: {sum}")

    // Find first > 5
    let found = numbers.iter().find(|x| x > 5)
    println(f"First > 5: {found}")
}`,
  },
  {
    slug: 'tensor-ops', title: 'Tensor Operations', difficulty: 'advanced', category: 'ml',
    code: `@device
fn tensor_demo() {
    // Create tensors
    let a = zeros(3, 3)
    let b = ones(3, 3)
    let c = randn(3, 3)

    // Matrix multiply
    let product = matmul(b, c)
    println(f"Shape: {product.shape()}")

    // Activation functions
    let activated = relu(product)
    let probs = softmax(activated)
    println(f"Sum of softmax: {probs.sum()}")

    // Xavier initialization for neural network
    let weights = xavier(784, 128)
    println(f"Xavier shape: {weights.shape()}")
}`,
  },
  {
    slug: 'ml-training', title: 'ML Training Loop', difficulty: 'advanced', category: 'ml',
    code: `@device
fn train() {
    // Simple linear regression: y = 2x + 1
    let x = from_data([[1.0], [2.0], [3.0], [4.0]])
    let y_true = from_data([[3.0], [5.0], [7.0], [9.0]])

    let w = randn(1, 1)
    set_requires_grad(w, true)
    let b = zeros(1, 1)
    set_requires_grad(b, true)

    let lr = 0.01
    for epoch in 0..100 {
        // Forward
        let y_pred = matmul(x, w) + b
        let loss_val = mse_loss(y_pred, y_true)

        // Backward
        backward(loss_val)

        if epoch % 20 == 0 {
            println(f"Epoch {epoch}: loss = {loss_val}")
        }
    }
    println("Training complete!")
}`,
  },
  {
    slug: 'kernel-demo', title: 'OS Kernel Code', difficulty: 'advanced', category: 'os',
    code: `// @kernel context: no heap, no tensor — hardware only
@kernel
fn kernel_init() {
    // VGA text mode
    let vga_base: i64 = 0xB8000
    let msg = "FajarOS booting..."
    let mut i: i64 = 0
    while i < 18 {
        volatile_write_u8(vga_base + i * 2, msg[i])
        volatile_write_u8(vga_base + i * 2 + 1, 0x0A) // green
        i = i + 1
    }

    // Setup interrupts
    irq_disable()
    // ... GDT, IDT, PIC setup ...
    irq_enable()
}

// @device context: tensor ops, no raw pointers
@device
fn inference(input: Tensor) -> Tensor {
    let weights = xavier(128, 10)
    matmul(input, weights) |> relu |> softmax
}

// @safe context: orchestration
@safe
fn main() {
    println("Fajar Lang: where kernel and ML share one type system")
}`,
  },
  {
    slug: 'async-await', title: 'Async/Await', difficulty: 'advanced', category: 'advanced',
    code: `async fn fetch_data(url: str) -> Result<str, str> {
    println(f"Fetching {url}...")
    // Simulated async I/O
    Ok(f"data from {url}")
}

async fn process() {
    let result1 = fetch_data("api/users").await
    let result2 = fetch_data("api/posts").await

    match result1 {
        Ok(data) => println(f"Users: {data}"),
        Err(e) => println(f"Error: {e}"),
    }

    match result2 {
        Ok(data) => println(f"Posts: {data}"),
        Err(e) => println(f"Error: {e}"),
    }
}

fn main() {
    // Run async function
    process()
    println("All done!")
}`,
  },
  {
    slug: 'generics', title: 'Generics & Option', difficulty: 'intermediate', category: 'basics',
    code: `fn max_of<T: Ord>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

fn find_first<T>(arr: [T], predicate: fn(T) -> bool) -> Option<T> {
    for item in arr {
        if predicate(item) { return Some(item) }
    }
    None
}

fn main() {
    println(f"max(3, 7) = {max_of(3, 7)}")
    println(f"max(hello, world) = {max_of(\"hello\", \"world\")}")

    let numbers = [10, 25, 3, 47, 12, 8]
    match find_first(numbers, |x| x > 20) {
        Some(n) => println(f"First > 20: {n}"),
        None => println("None found > 20"),
    }
}`,
  },
];

export function loadExample(slug) {
  return examples.find(e => e.slug === slug);
}

export function examplesByCategory(category) {
  return examples.filter(e => e.category === category);
}

export function categories() {
  return [...new Set(examples.map(e => e.category))];
}
