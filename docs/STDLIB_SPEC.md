# STDLIB SPEC

> Spesifikasi Standard Library — Fajar Lang Built-in Modules & Functions

---

## 1. Overview Standard Library

Standard library Fajar Lang terbagi menjadi tiga domain utama yang mencerminkan dual-domain nature bahasa ini:

| Module | Domain | Deskripsi |
|--------|--------|-----------|
| `std::` | General | Core types, collections, I/O, string manipulation |
| `os::` | OS/Systems | Memory management, interrupts, syscalls, port I/O |
| `nn::` | ML/AI | Tensor ops, layers, activations, autograd, optimizers |

---

## 2. `std::` — Core Standard Library

### 2.1 `std::io`

```fajar
// Output
fn print(value: impl Display) -> void
fn println(value: impl Display) -> void
fn eprint(value: impl Display) -> void
fn eprintln(value: impl Display) -> void

// Input
fn read_line() -> Result<str, IoError>

// File I/O (hanya di @safe context)
fn read_file(path: str) -> Result<str, IoError>
fn write_file(path: str, content: str) -> Result<void, IoError>
```

### 2.2 `std::collections`

```fajar
// Vec<T> — Dynamic array
fn Vec::new() -> Vec<T>
fn Vec::with_capacity(cap: usize) -> Vec<T>
fn Vec::push(&mut self, item: T) -> void
fn Vec::pop(&mut self) -> Option<T>
fn Vec::len(&self) -> usize
fn Vec::is_empty(&self) -> bool
fn Vec::get(&self, index: usize) -> Option<&T>

// HashMap<K, V> — Hash table
fn HashMap::new() -> HashMap<K, V>
fn HashMap::insert(&mut self, key: K, value: V) -> Option<V>
fn HashMap::get(&self, key: &K) -> Option<&V>
fn HashMap::contains_key(&self, key: &K) -> bool
fn HashMap::remove(&mut self, key: &K) -> Option<V>

// Lainnya: HashSet<T>, VecDeque<T>, BTreeMap<K,V>
```

### 2.3 `std::string`

```fajar
fn String::new() -> String
fn String::from(s: &str) -> String
fn String::len(&self) -> usize
fn String::push_str(&mut self, s: &str) -> void
fn String::contains(&self, pattern: &str) -> bool
fn String::split(&self, sep: &str) -> Vec<String>
fn String::trim(&self) -> &str
fn String::to_uppercase(&self) -> String
fn String::to_lowercase(&self) -> String
```

### 2.4 `std::math`

```fajar
const PI: f64 = 3.14159265358979323846
const E: f64 = 2.71828182845904523536

fn abs(x: f64) -> f64
fn sqrt(x: f64) -> f64
fn pow(base: f64, exp: f64) -> f64
fn log(x: f64) -> f64          // natural log
fn log2(x: f64) -> f64
fn log10(x: f64) -> f64
fn sin(x: f64) -> f64
fn cos(x: f64) -> f64
fn tan(x: f64) -> f64
fn floor(x: f64) -> f64
fn ceil(x: f64) -> f64
fn round(x: f64) -> f64
fn min(a: f64, b: f64) -> f64
fn max(a: f64, b: f64) -> f64
fn clamp(x: f64, lo: f64, hi: f64) -> f64
```

### 2.5 `std::convert`

```fajar
// Explicit type conversion — NO implicit casts
trait From<T> { fn from(value: T) -> Self }
trait Into<T> { fn into(self) -> T }
trait TryFrom<T> { fn try_from(value: T) -> Result<Self, ConvertError> }

// Integer conversions
impl From<i32> for i64 { ... }  // widening: always safe
impl TryFrom<i64> for i32 { ... }  // narrowing: might fail
```

---

## 3. `os::` — OS Primitives Library

> ⚠️ **Context:** Semua fungsi `os::` hanya boleh dipanggil di `@kernel` atau `@unsafe` context.

### 3.1 `os::memory`

```fajar
// Raw memory allocation
macro alloc!(size: usize) -> *mut u8
macro free!(ptr: *mut u8, size: usize) -> void

// Virtual memory management
macro map_page!(va: VirtAddr, pa: PhysAddr, flags: PageFlags) -> Result<void, MapError>
macro unmap_page!(va: VirtAddr) -> Result<void, MapError>

// Flags
const MEM_READ: PageFlags
const MEM_WRITE: PageFlags
const MEM_EXEC: PageFlags
const MEM_USER: PageFlags

// Type-safe addresses
struct VirtAddr(u64)
struct PhysAddr(u64)
impl VirtAddr { fn new(addr: u64) -> Self }
impl PhysAddr { fn new(addr: u64) -> Self }
```

### 3.2 `os::irq`

```fajar
// Interrupt handling
macro irq_register!(num: u8, handler: fn() -> void) -> void
macro irq_enable!() -> void
macro irq_disable!() -> void

// IRQ numbers (x86)
const IRQ_TIMER: u8 = 0x20
const IRQ_KEYBOARD: u8 = 0x21
const IRQ_SERIAL: u8 = 0x24
```

### 3.3 `os::syscall`

```fajar
// Syscall definition
macro syscall_define!(num: u32, handler: fn(args...) -> i64) -> void

// Standard syscall numbers
const SYS_READ: u32 = 0
const SYS_WRITE: u32 = 1
const SYS_OPEN: u32 = 2
const SYS_CLOSE: u32 = 3
const SYS_EXIT: u32 = 60
```

### 3.4 `os::io`

```fajar
// Port I/O (x86)
macro port_write!(port: u16, value: u8) -> void
macro port_read!(port: u16) -> u8
macro port_write16!(port: u16, value: u16) -> void
macro port_read16!(port: u16) -> u16
```

---

## 4. `nn::` — Neural Network Library

> ⚠️ **Context:** Semua fungsi `nn::` **hanya boleh dipanggil langsung di `@device` atau `@unsafe` context**. Kode `@safe` dapat memanggil fungsi berlabel `@device` yang secara internal menggunakan `nn::` (bridge pattern), tapi tidak dapat memanggil `nn::` functions secara langsung.

### 4.1 `nn::tensor` — Tensor Creation

```fajar
fn zeros(shape: &[usize]) -> Tensor
fn ones(shape: &[usize]) -> Tensor
fn randn(shape: &[usize]) -> Tensor       // normal distribution
fn xavier(rows: usize, cols: usize) -> Tensor  // Xavier initialization
fn eye(n: usize) -> Tensor                 // identity matrix
fn from_data(data: &[f32], shape: &[usize]) -> Tensor
fn arange(start: f32, end: f32, step: f32) -> Tensor
fn linspace(start: f32, end: f32, steps: usize) -> Tensor
```

### 4.2 `nn::ops` — Tensor Operations

```fajar
// Arithmetic (operator overloaded)
impl Add for Tensor    // a + b
impl Sub for Tensor    // a - b
impl Mul for Tensor    // a * b (element-wise)
impl Div for Tensor    // a / b
impl Matmul for Tensor // a @ b (matrix multiply)

// Shape manipulation
fn Tensor::reshape(&self, shape: &[usize]) -> Tensor
fn Tensor::transpose(&self) -> Tensor        // .T shorthand
fn Tensor::flatten(&self) -> Tensor
fn Tensor::squeeze(&self, dim: usize) -> Tensor
fn Tensor::unsqueeze(&self, dim: usize) -> Tensor

// Reduction
fn Tensor::sum(&self) -> Tensor
fn Tensor::mean(&self) -> Tensor
fn Tensor::max(&self) -> Tensor
fn Tensor::min(&self) -> Tensor
fn Tensor::argmax(&self, dim: usize) -> Tensor
```

### 4.3 `nn::activation` — Activation Functions

```fajar
fn relu(x: &Tensor) -> Tensor
fn sigmoid(x: &Tensor) -> Tensor
fn tanh(x: &Tensor) -> Tensor
fn softmax(x: &Tensor, dim: usize) -> Tensor
fn gelu(x: &Tensor) -> Tensor
fn leaky_relu(x: &Tensor, neg_slope: f32) -> Tensor
```

### 4.4 `nn::loss` — Loss Functions

```fajar
fn mse_loss(pred: &Tensor, target: &Tensor) -> Tensor
fn cross_entropy(logits: &Tensor, labels: &Tensor) -> Tensor
fn bce_loss(pred: &Tensor, target: &Tensor) -> Tensor
fn l1_loss(pred: &Tensor, target: &Tensor) -> Tensor
```

### 4.5 `nn::layer` — Neural Network Layers

```fajar
struct Dense { weights: Tensor, bias: Tensor }
fn Dense::new(in_features: usize, out_features: usize) -> Dense
fn Dense::forward(&self, input: &Tensor) -> Tensor

struct Conv2d { kernel: Tensor, bias: Tensor, stride: usize, padding: usize }
fn Conv2d::new(in_ch: usize, out_ch: usize, kernel_size: usize) -> Conv2d
fn Conv2d::forward(&self, input: &Tensor) -> Tensor

struct Attention { wq: Tensor, wk: Tensor, wv: Tensor, d_model: usize }
fn Attention::new(d_model: usize, n_heads: usize) -> Attention
fn Attention::forward(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor

fn embedding(vocab_size: usize, embed_dim: usize) -> Tensor
fn dropout(x: &Tensor, p: f32) -> Tensor
fn batch_norm(x: &Tensor) -> Tensor
fn layer_norm(x: &Tensor) -> Tensor
```

### 4.6 `nn::autograd` — Automatic Differentiation

```fajar
fn Tensor::requires_grad(&mut self, flag: bool) -> void
fn Tensor::backward(&self) -> void
fn Tensor::grad(&self) -> Option<Tensor>
macro no_grad { /* block */ }  // disable gradient tracking
fn Tensor::zero_grad(&mut self) -> void
```

### 4.7 `nn::optim` — Optimizers

```fajar
trait Optimizer {
    fn step(&mut self) -> void
    fn zero_grad(&mut self) -> void
}

struct SGD { params: Vec<Tensor>, lr: f32 }
fn sgd(params: Vec<Tensor>, lr: f32) -> SGD

struct Adam { params: Vec<Tensor>, lr: f32, beta1: f32, beta2: f32, eps: f32 }
fn adam(params: Vec<Tensor>, lr: f32) -> Adam
fn adam_with(params: Vec<Tensor>, lr: f32, beta1: f32, beta2: f32) -> Adam
```

### 4.8 `nn::data` — Data Utilities

```fajar
fn load_csv(path: str) -> Result<Tensor, IoError>
fn load_mnist(path: str) -> Result<(Tensor, Tensor), IoError>
fn normalize(x: &Tensor, mean: f32, std: f32) -> Tensor
fn one_hot(labels: &Tensor, num_classes: usize) -> Tensor
fn train_test_split(x: &Tensor, ratio: f32) -> (Tensor, Tensor)
```

---

## 5. Built-in Functions (Global)

| Fungsi | Signature | Deskripsi |
|--------|-----------|-----------|
| `print` | `fn print(value: impl Display)` | Print tanpa newline |
| `println` | `fn println(value: impl Display)` | Print dengan newline |
| `len` | `fn len(collection: impl Sized) -> usize` | Panjang collection/string |
| `type_of` | `fn type_of(value: any) -> str` | Nama tipe runtime |
| `assert!` | `macro assert!(condition: bool)` | Panic jika false |
| `assert_eq!` | `macro assert_eq!(left, right)` | Panic jika tidak equal |
| `panic!` | `macro panic!(msg: str)` | Terminate program |
| `todo!` | `macro todo!()` | Placeholder (panic) |
| `dbg!` | `macro dbg!(value: any) -> any` | Debug print + return value |

---

*Stdlib Version: 0.1 | Modules: 3 domains, 14 sub-modules*
