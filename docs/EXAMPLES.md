# EXAMPLES

> Contoh Program & Tutorial — Fajar Lang Learn by Example

---

## 1. Beginner — Dasar-Dasar

### 1.1 Hello World

```fajar
// examples/hello.fj
use std::io::println

fn main() -> void {
    println("Hello from Fajar Lang!")
}
```

### 1.2 Variables & Types

```fajar
// examples/variables.fj
fn main() -> void {
    // Immutable (default)
    let x: i32 = 42
    let name: str = "Fajar"
    let pi: f64 = 3.14159
    let active: bool = true

    // Mutable
    let mut counter: i32 = 0
    counter = counter + 1

    // Type inference
    let inferred = 100    // i64 (default integer)
    let decimal = 3.14    // f64 (default float)

    // Constants (compile-time)
    const MAX_SIZE: usize = 1024

    println("x = " + x.to_string())
}
```

### 1.3 Control Flow

```fajar
// examples/control_flow.fj
fn main() -> void {
    // If/else (is expression!)
    let x = 42
    let label = if x > 0 { "positive" } else { "non-positive" }

    // While loop
    let mut i = 0
    while i < 5 {
        println(i)
        i = i + 1
    }

    // For loop (iterator-based)
    for n in 0..10 {
        println(n)
    }

    // Match (exhaustive)
    let grade = match x {
        90..=100 => "A",
        80..=89  => "B",
        70..=79  => "C",
        _        => "F",
    }
}
```

### 1.4 Functions

```fajar
// examples/functions.fj

// Basic function
fn add(a: i32, b: i32) -> i32 {
    a + b    // last expression = return value
}

// Recursive
fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

// Higher-order
fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

fn main() -> void {
    println(add(3, 4))           // 7
    println(factorial(10))       // 3628800
    println(apply(|x| x * 2, 5)) // 10
}
```

### 1.5 Structs & Enums

```fajar
// examples/structs.fj

struct Point { x: f64, y: f64 }

impl Point {
    fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x
        let dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
    }
}

enum Shape {
    Circle(f64),          // radius
    Rectangle(f64, f64),  // width, height
}

fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle(r)       => 3.14159 * r * r,
        Shape::Rectangle(w, h) => w * h,
    }
}
```

---

## 2. Intermediate — Pattern & Idioms

### 2.1 Error Handling

```fajar
// examples/error_handling.fj
use std::io::read_file

fn parse_number(s: str) -> Result<i64, str> {
    if valid {
        Ok(parsed_value)
    } else {
        Err("invalid number format")
    }
}

fn load_config(path: str) -> Result<Config, str> {
    let content = read_file(path)?   // ? propagates error
    let port = parse_number(content.get("port"))?
    Ok(Config { port })
}

fn main() -> void {
    match load_config("config.txt") {
        Ok(cfg) => println("Port: " + cfg.port.to_string()),
        Err(e)  => eprintln("Error: " + e),
    }
}
```

### 2.2 Pipeline Operator

```fajar
// examples/pipeline.fj
// Pipeline operator |> untuk functional style

fn double(x: i32) -> i32 { x * 2 }
fn add_one(x: i32) -> i32 { x + 1 }
fn to_string(x: i32) -> str { x.to_string() }

fn main() -> void {
    // Tanpa pipeline (nested calls — sulit dibaca)
    let result = to_string(add_one(double(5)))

    // Dengan pipeline (kiri ke kanan — natural)
    let result = 5 |> double |> add_one |> to_string
    // result = "11"
}
```

### 2.3 Generics & Traits

```fajar
// examples/generics.fj

trait Printable {
    fn to_display_string(&self) -> str
}

fn print_all<T: Printable>(items: &[T]) -> void {
    for item in items {
        println(item.to_display_string())
    }
}

struct Pair<A, B> { first: A, second: B }

impl<A: Printable, B: Printable> Pair<A, B> {
    fn describe(&self) -> str {
        "(" + self.first.to_display_string() + ", "
            + self.second.to_display_string() + ")"
    }
}
```

---

## 3. OS Domain — Sistem Programming

### 3.1 Kernel Memory Manager

```fajar
// examples/memory_map.fj
use os::memory::{alloc, free, map_page, VirtAddr, PhysAddr}
use os::memory::{MEM_READ, MEM_WRITE}

@kernel
fn setup_page_table() -> Result<void, MapError> {
    let va = VirtAddr::new(0xFFFF_8000_0000_0000)
    let pa = PhysAddr::new(0x0000_0000_0010_0000)
    map_page!(va, pa, MEM_READ | MEM_WRITE)?

    let region = alloc!(4096)
    // ... use region ...
    free!(region, 4096)

    Ok(())
}
```

### 3.2 Interrupt Handler

```fajar
// examples/irq_handler.fj
use os::irq::{irq_register, irq_enable, IRQ_KEYBOARD}

@kernel
fn keyboard_handler() -> void {
    let scancode = port_read!(0x60)
    println("Key pressed: " + scancode.to_string())
}

@kernel
fn init_interrupts() -> void {
    irq_register!(IRQ_KEYBOARD, keyboard_handler)
    irq_enable!()
}
```

---

## 4. ML Domain — Machine Learning

### 4.1 Simple Neural Network (XOR)

```fajar
// examples/xor_nn.fj
use nn::*

struct XorNet {
    hidden: Dense,
    output: Dense,
}

impl XorNet {
    fn new() -> XorNet {
        XorNet {
            hidden: Dense::new(2, 8),
            output: Dense::new(8, 1),
        }
    }

    @device
    fn forward(&self, x: &Tensor) -> Tensor {
        x |> self.hidden.forward |> relu |> self.output.forward |> sigmoid
    }
}

fn main() -> void {
    let model = XorNet::new()
    let optimizer = adam(model.parameters(), 0.01)

    let x = Tensor::from_data(&[0,0, 0,1, 1,0, 1,1], &[4, 2])
    let y = Tensor::from_data(&[0, 1, 1, 0], &[4, 1])

    for epoch in 0..1000 {
        let pred = model.forward(&x)
        let loss = bce_loss(&pred, &y)
        optimizer.zero_grad()
        loss.backward()
        optimizer.step()
    }
}
```

### 4.2 MNIST Classifier

```fajar
// examples/mnist_forward.fj
use nn::*

struct MnistNet {
    fc1: Dense,    // 784 -> 128
    fc2: Dense,    // 128 -> 64
    fc3: Dense,    // 64 -> 10
}

impl MnistNet {
    fn new() -> MnistNet {
        MnistNet {
            fc1: Dense::new(784, 128),
            fc2: Dense::new(128, 64),
            fc3: Dense::new(64, 10),
        }
    }

    @device(cpu)
    fn forward(&self, x: &Tensor) -> Tensor {
        let h1 = relu(&self.fc1.forward(x))
        let h2 = relu(&self.fc2.forward(&h1))
        softmax(&self.fc3.forward(&h2), 1)
    }
}
```

---

## 5. Cross-Domain — OS + ML Bridge

### 5.1 AI-Powered Kernel Monitor

Contoh paling powerful dari Fajar Lang: menggabungkan OS primitives dan ML inference dalam satu program, satu type system, satu compiler.

```fajar
// examples/ai_kernel_monitor.fj
use os::memory::{alloc, free, VirtAddr}
use os::irq::{irq_register, IRQ_TIMER}
use nn::*

// === @kernel: Collect system metrics ===
@kernel
fn collect_metrics() -> [f32; 4] {
    let cpu_usage = port_read!(0x80) as f32 / 255.0
    let mem_usage = port_read!(0x81) as f32 / 255.0
    let io_wait   = port_read!(0x82) as f32 / 255.0
    let irq_rate  = port_read!(0x83) as f32 / 255.0
    [cpu_usage, mem_usage, io_wait, irq_rate]
}

// === @device: Run inference ===
struct AnomalyDetector { fc1: Dense, fc2: Dense }

@device
fn detect_anomaly(model: &AnomalyDetector, data: &Tensor) -> f32 {
    let h = relu(&model.fc1.forward(data))
    let score = sigmoid(&model.fc2.forward(&h))
    score.item()  // scalar value
}

// === @safe: Bridge OS + ML ===
@safe
fn monitor_system() -> void {
    let model = AnomalyDetector { ... }

    loop {
        let metrics = collect_metrics()  // from @kernel
        let tensor = Tensor::from_data(&metrics, &[1, 4])
        let anomaly_score = detect_anomaly(&model, &tensor)

        if anomaly_score > 0.8 {
            eprintln("ALERT: System anomaly detected!")
        }
    }
}
```

> **Key Insight:** Tidak ada bahasa lain yang bisa melakukan ini. C tidak punya ML native. Python tidak bisa OS dev. Rust tidak punya context annotations. Fajar Lang unifies both domains.

---

*Examples Version: 1.0 | Programs: 12 complete examples across 5 difficulty levels*
