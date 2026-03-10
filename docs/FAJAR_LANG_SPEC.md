# FAJAR LANG

Dokumentasi Lengkap Bahasa Pemrograman

Spesifikasi v0.1 | Arsitektur | Security Model | Roadmap

*"The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides."*

Versi Dokumen: 1.0 | Status: Comprehensive Draft

**Daftar Isi**

## 1. Pendahuluan

### 1.1 Apa itu Fajar Lang?

Fajar Lang (ekstensi file: .fj) adalah bahasa pemrograman statically-typed yang dirancang dari awal untuk menyatukan dua domain yang selama ini terpisah secara fundamental: OS/systems programming dan ML/AI development. Dengan satu syntax, satu type system, dan satu compiler, Fajar Lang menghilangkan kebutuhan untuk menggunakan dua bahasa berbeda ketika membangun AI-powered edge devices seperti drone otonom, robot medis, atau mobil self-driving.

### 1.2 Masalah yang Diselesaikan

Saat ini, membangun AI-powered edge device memerlukan setidaknya dua bahasa: C/Rust/Zig untuk firmware dan device driver (domain OS), serta Python/C++ untuk neural network inference (domain ML). Ini berarti dua codebase, dua toolchain, dua paradigma, dua security model, dan dua tim. Ketika data harus berpindah dari satu dunia ke dunia lain, di situlah bug paling berbahaya muncul.

Fajar Lang menghilangkan batas ini. Dalam satu file, satu type system, dan satu compiler, developer bisa menulis kode kernel yang mengelola memori dan interrupt, sekaligus kode neural network yang menjalankan inference — dengan jaminan keamanan yang diverifikasi di compile time.

### 1.3 Design Principles

1.  Explicitness over magic — tidak ada hidden allocation atau hidden cost. Semua yang terjadi terlihat jelas dari kode.

2.  Dual-context safety — @kernel mode menonaktifkan heap dan tensor; @device mode mengaktifkan tensor engine dan menonaktifkan raw pointers. Compiler menjamin isolasi ini.

3.  Rust-inspired but simpler — ownership lite tanpa lifetime annotations. Memory safety tanpa complexity yang berlebihan.

4.  Native tensor types — Tensor adalah first-class citizen di type system, bukan library tambahan. Shape diperiksa di compile time.

### 1.4 Target Audience

**Primary: Embedded AI Engineers**

Developer yang membangun device dengan ML inference langsung di hardware — drone, robot, IoT industrial, medical devices. Mereka saat ini harus switch antara C/Rust untuk firmware dan Python/C++ untuk model.

**Secondary: OS Research Teams**

Tim yang membangun OS generasi berikutnya dengan AI integration (memory prediction, intelligent scheduler, anomaly detection di kernel) — memerlukan bahasa yang bisa beroperasi di kedua level.

**Tertiary: Safety-Critical ML Systems**

Automotive (ADAS), aerospace, medical AI — di mana compile-time guarantees bukan opsional. Tensor shape safety dan context isolation adalah requirement regulasi.

## 2. Lexical Grammar

### 2.1 Keywords

**Control Flow**

> if else match while for in return break continue

**Declarations**

> let mut fn struct enum impl trait type const

**Types**

> bool i8 i16 i32 i64 i128 isize
>
> u8 u16 u32 u64 u128 usize
>
> f32 f64 str char void never

**Tensor Types (ML Domain)**

> tensor grad loss layer model

**OS Primitives**

> ptr addr page region irq syscall

**Context Annotations**

> @kernel @device @safe @unsafe @ffi

**Module System**

> use mod pub extern as

**Literals**

> true false null

### 2.2 Operators

  —————-- —————————————— ————————————--
  **Kategori**      **Operators**                              **Catatan**

  Arithmetic        \+ - \* / % \*\*                           \*\* = power

  Matrix Multiply   @                                          ML domain only

  Comparison        == != \< \> \<= \>=                        

  Logical           && || !                                  

  Bitwise (OS)      & | \^ \~ \<\< \>\>                       

  Assignment        = += -= \*= /= %= &= |= \^= \<\<= \>\>=   

  Pointer (OS)      \* & -\>                                   Dereference, reference, field access

  Range             .. ..=                                     Exclusive dan inclusive

  Pipeline          |\>                                       x |\> f = f(x)
  —————-- —————————————— ————————————--

**Operator Precedence (lowest to highest)**

  ———-- ———————————— ———————-
  **Level**   **Operators**                        **Associativity**

  1           = += -= etc                          Right

  2           |\>                                 Left

  3           ||                                 Left

  4           &&                                   Left

  5           == !=                                Left

  6           \< \> \<= \>=                        Left

  7           \+ -                                 Left

  8           \* / % @                             Left

  9           \*\*                                 Right

  10          Unary: ! - \* &                      Right

  11          Call, index, field                   Left
  ———-- ———————————— ———————-

### 2.3 Literals

> **Default types:** Integer literals default to `i64`. Float literals default to `f64`. Suffixes (`42i32`, `3.14f32`) can override.

**Integer Literals**

> 42 // decimal (default: i64)
>
> 0xFF // hexadecimal
>
> 0b1010 // binary
>
> 0o17 // octal
>
> 1_000_000 // underscore separator

**Float Literals**

> 3.14
>
> 2.718f32 // explicit f32 suffix
>
> 1.0e-4 // scientific notation

**String Literals**

> "hello world"
>
> r"raw \\n string" // raw string, no escape processing

**Boolean Literals**

> true false

**Tensor Literals**

> \[1.0, 2.0, 3.0\] // 1D tensor
>
> \[\[1.0, 2.0\], \[3.0, 4.0\]\] // 2D tensor (matrix)
>
> zeros(3, 4) // built-in initializer
>
> ones(3, 4)
>
> xavier(3, 4) // Xavier/Glorot initialization
>
> randn(3, 4) // normal distribution N(0,1)

### 2.4 Comments

> // Single line comment
>
> /\* Multi-line
>
> comment \*/
>
> /// Doc comment (for documentation generation)
>
> //! Module-level doc comment

## 3. Type System

### 3.1 Primitive Types

  —————— ————— —————————————--
  **Type**           **Size**        **Description**

  bool               1 bit           true/false

  i8 -- i128         8--128 bit      Signed integers

  u8 -- u128         8--128 bit      Unsigned integers

  isize/usize        arch            Pointer-sized integers

  f32                32 bit          Single precision float (IEEE 754)

  f64                64 bit          Double precision float (IEEE 754)

  char               32 bit          Unicode scalar value

  str                dynamic         UTF-8 string slice

  void               0               Unit type (function returns nothing)

  never              0               Diverging type (function never returns)
  —————— ————— —————————————--

### 3.2 Composite Types

**Tuple**

> let t: (i32, f32, bool) = (1, 3.14, true)
>
> let (a, b, c) = t // destructuring

**Array (fixed size)**

> let arr: \[i32; 5\] = \[1, 2, 3, 4, 5\]

**Slice (dynamic view)**

> let sl: \[i32\] = arr\[1..3\]

**Struct**

> struct Point {
>
> x: f64,
>
> y: f64,
>
> }

**Enum (Tagged Union)**

> enum Direction {
>
> North,
>
> South,
>
> East,
>
> West,
>
> Custom(f64, f64), // variant with data
>
> }

**Optional**

> let maybe: Option\<i32\> = Some(42)
>
> let none: Option\<i32\> = None

**Result**

> let ok: Result\<i32, str\> = Ok(42)
>
> let err: Result\<i32, str\> = Err("failed")

### 3.3 Tensor Types (ML Domain)

Tensor adalah tipe native di Fajar Lang — bukan objek library. Shape adalah bagian dari type system dan diperiksa di compile time.

> // Tensor\<Shape, DType\>
>
> // Shape = compile-time tuple of dimensions
>
> // DType defaults to f32
>
> tensor x: f32\[3\] // 1D, 3 elements
>
> tensor w: f32\[784, 128\] // 2D matrix
>
> tensor b: f32\[128\] // bias vector
>
> // Gradient type (wraps tensor, tracks gradients)
>
> grad g: f32\[128\]
>
> // Layer type
>
> layer dense: Layer\<784, 128\>
>
> // Type inference
>
> let y = x @ w + b // inferred as Tensor\<f32\>\[3, 128\]

**Dynamic Shapes**

> // Wildcard dimension \[\*\] for dynamic shapes
>
> tensor batch: f32\[\*, 784\] // batch size unknown at compile time

**PENTING:** Shape mismatch terdeteksi di COMPILE TIME, bukan runtime. Ini adalah differensiator utama Fajar Lang vs PyTorch/NumPy.

### 3.4 Pointer Types (OS Domain)

Pointer types hanya tersedia dalam @kernel atau @unsafe context. Di @safe context, compiler menolak penggunaan raw pointers.

> \*const T // raw const pointer
>
> \*mut T // raw mutable pointer
>
> addr\<T\> // typed virtual address
>
> phys\<T\> // typed physical address

**Type-Safe Hardware Addresses**

VirtAddr dan PhysAddr adalah DISTINCT TYPES — bukan alias untuk u64. Compiler membedakannya dan menolak penggunaan yang tertukar.

> struct VirtAddr(u64)
>
> struct PhysAddr(u64)
>
> fn map_page(va: VirtAddr, pa: PhysAddr, flags: PageFlags) { \... }
>
> let va = VirtAddr::new(0xFFFF_8000_0000_0000)
>
> let pa = PhysAddr::new(0x0000_0000_0010_0000)
>
> map_page(va, pa, PageFlags::RW) // OK
>
> map_page(pa, va, PageFlags::RW) // COMPILE ERROR: type mismatch

## 4. Expressions

### 4.1 Basic Expressions

**Arithmetic**

> 1 + 2 \* 3 // 7 (standard precedence)
>
> 2 \*\* 10 // 1024 (power)
>
> 10 / 3 // 3 (integer division)
>
> 10.0 / 3.0 // 3.333\...

**Comparison & Logical**

> 5 \> 3 // true
>
> "abc" == "abc" // true
>
> true && false // false
>
> !true // false

**Pipeline Operator**

> fn double(x: i32) -\> i32 { x \* 2 }
>
> fn add_one(x: i32) -\> i32 { x + 1 }
>
> 5 |\> double |\> add_one // 11
>
> // equivalent to: add_one(double(5))

**Range Expressions**

> 0..10 // \[0, 1, \..., 9\] exclusive
>
> 0..=10 // \[0, 1, \..., 10\] inclusive

### 4.2 Block Expressions

Blok adalah expressions — ekspresi terakhir tanpa semicolon menjadi return value.

> let result = {
>
> let a = 5
>
> let b = 10
>
> a + b // no semicolon = return value
>
> }
>
> // result == 15
>
> // If as expression
>
> let max = if a \> b { a } else { b }

### 4.3 Function Calls

> fn add(a: i32, b: i32) -\> i32 { a + b }
>
> add(1, 2) // positional
>
> add(a: 1, b: 2) // named arguments
>
> add(b: 2, a: 1) // named, out of order

### 4.4 Pattern Matching

> match value {
>
> 0 =\> "zero",
>
> 1..=9 =\> "single digit",
>
> x if x \< 0 =\> "negative",
>
> \_ =\> "other",
>
> }
>
> // Destructuring
>
> let Point { x, y } = point
>
> let (a, b, c) = tuple
>
> // Match on enum
>
> match direction {
>
> Direction::North =\> move_up(),
>
> Direction::Custom(x,y) =\> move_to(x, y),
>
> \_ =\> do_nothing(),
>
> }

## 5. Statements

### 5.0 Semicolon Rules

Semicolons are **optional** statement terminators. Key rules:
- Expression **with** semicolon → statement (value discarded)
- Expression **without** semicolon as last in block → block's return value
- `let`, `const`, `return`, `use` → semicolon optional but recommended

> let result = { let a = 5; let b = 10; a + b }  // result == 15 (no semicolon on a + b)

### 5.1 Variable Declarations

> let x = 42 // immutable, type inferred
>
> let x: i32 = 42 // immutable, explicit type
>
> let mut y = 0 // mutable
>
> let mut z: f64 = 0.0
>
> const MAX: usize = 1024 // compile-time constant

### 5.2 Control Flow

**If/Else**

> if condition {
>
> \...
>
> } else if other {
>
> \...
>
> } else {
>
> \...
>
> }

**While Loop**

> while x \> 0 {
>
> x -= 1
>
> }

**For Loop (Iterator)**

> for i in 0..10 {
>
> print(i)
>
> }
>
> for item in collection {
>
> process(item)
>
> }

**Loop with Break Value**

> let result = loop {
>
> if done { break 42 }
>
> }

### 5.3 Function Definitions

> // Basic function
>
> fn greet(name: str) -\> str {
>
> "Hello, " + name
>
> }
>
> // Multiple return via tuple
>
> fn divmod(a: i32, b: i32) -\> (i32, i32) {
>
> (a / b, a % b)
>
> }
>
> // Generic function
>
> fn max\<T: Comparable\>(a: T, b: T) -\> T {
>
> if a \> b { a } else { b }
>
> }
>
> // Higher-order function
>
> fn apply\<T\>(f: fn(T) -\> T, x: T) -\> T {
>
> f(x)
>
> }
>
> // Async function (for OS/device tasks)
>
> async fn load_model(path: str) -\> Result\<Model, Error\> {
>
> \...
>
> }

## 6. Context Annotations

Context annotations adalah fitur paling unik Fajar Lang. Mereka mengontrol runtime features yang tersedia dan di-enforce oleh compiler — bukan konvensi, bukan comment, bukan runtime check, melainkan bagian dari type system.

### 6.1 Context Hierarchy

> @unsafe ──▶ akses ke semua features
>
> @kernel ──▶ OS primitives, no heap, no tensor
>
> @device ──▶ tensor ops, no raw pointer, no IRQ
>
> @safe ──▶ default; no hardware, no raw pointer (safest subset)

### 6.2 @kernel

Untuk bare metal OS development. Heap allocation dan tensor operations dilarang oleh compiler.

> @kernel
>
> fn init_heap(base: addr\<u8\>, size: usize) {
>
> // ALLOWED:
>
> let region = alloc!(4096) // physical page allocation
>
> map_page!(virt, phys, MEM_READ | MEM_WRITE) // page tables
>
> irq_register!(0x0E, page_fault_handler) // interrupts
>
> let val: u8 = port_read!(0x60) // hardware I/O
>
> // FORBIDDEN (compile errors):
>
> let s = String::new() // error\[KE001\]: heap alloc in @kernel
>
> let t = zeros(3, 4) // error\[KE002\]: tensor op in @kernel
>
> }

### 6.3 @device

Untuk ML computation. Mendukung tensor operations dan GPU dispatch; raw pointers dan OS operations dilarang.

> @device(cpu) // run on CPU, SIMD optimized
>
> @device(gpu) // run on GPU via WGPU backend
>
> @device(auto) // runtime dispatch
>
> @device(cpu)
>
> fn forward(input: Tensor\<f32\>\[784\]) -\> Tensor\<f32\>\[10\] {
>
> // ALLOWED: tensor ops, activations, autograd, layers
>
> let h = relu(input @ W1 + b1)
>
> softmax(h @ W2 + b2)
>
> // FORBIDDEN (compile errors):
>
> let ptr: \*mut u8 = \... // error\[DE001\]: raw ptr in @device
>
> irq_register!(0x21, handler) // error\[DE002\]: IRQ in @device
>
> }

### 6.4 @safe (Default)

Context default — tidak perlu ditulis secara eksplisit. Melarang semua hardware access dan raw pointers. Ini adalah subset paling aman.

> @safe // opsional, ini sudah default
>
> fn compute_statistics(data: &\[f32\]) -\> (f32, f32) {
>
> let mean = data.iter().sum::\<f32\>() / data.len() as f32
>
> let variance = data.iter()
>
> .map(|x| (x - mean) \*\* 2.0)
>
> .sum::\<f32\>() / data.len() as f32
>
> (mean, variance)
>
> // FORBIDDEN: hardware, raw pointers, kernel ops, device ops
>
> }

### 6.5 @unsafe

Akses penuh — harus eksplisit dan diaudit. Setiap @unsafe block/function WAJIB memiliki SAFETY comment yang menjelaskan preconditions.

> @unsafe
>
> fn raw_write(addr: \*mut u8, val: u8) {
>
> // SAFETY: caller guarantees addr is valid mapped memory
>
> \*addr = val
>
> }

### 6.6 @ffi

Foreign Function Interface untuk interop dengan C libraries.

> @ffi("C")
>
> extern {
>
> fn printf(fmt: \*const u8, \...) -\> i32
>
> fn memcpy(dst: \*mut void, src: \*const void, n: usize) -\> \*mut void
>
> }

### 6.7 Context Calling Convention

Aturan pemanggilan lintas-context:

1. `@safe` **BISA** memanggil fungsi `@kernel` dan `@device` (bridge pattern)
2. `@safe` **TIDAK BISA** langsung menggunakan kernel primitives (`alloc!`, `irq_register!`, etc.)
3. `@safe` **TIDAK BISA** langsung menggunakan tensor primitives (`zeros()`, `relu()`, etc.)
4. `@kernel` **TIDAK BISA** memanggil fungsi `@device` (domain isolation)
5. `@device` **TIDAK BISA** memanggil fungsi `@kernel` (domain isolation)
6. `@unsafe` **BISA** memanggil semua fungsi dari context manapun

| Caller \ Callee | @safe | @kernel | @device | @unsafe |
|-----------------|-------|---------|---------|---------|
| `@safe` | OK | OK | OK | ERROR |
| `@kernel` | OK | OK | ERROR | OK (explicit) |
| `@device` | OK | ERROR | OK | OK (explicit) |
| `@unsafe` | OK | OK | OK | OK |

### 6.8 Cross-Context Bridge (@safe)

@safe context berfungsi sebagai bridge antara OS dan ML domain. Ini memungkinkan data mengalir dari sensor hardware ke neural network dalam satu bahasa.

> @safe
>
> fn ai_kernel_monitor(sensor_data: \[f32; 784\]) -\> KernelAction {
>
> // Konversi dari OS data ke ML tensor
>
> let input = Tensor::from_slice(sensor_data)
>
> // Jalankan inference
>
> let prediction = inference_forward(input)
>
> // Kembali ke OS action
>
> match prediction.argmax() {
>
> 0 =\> KernelAction::IncreaseMemoryPressure,
>
> 1 =\> KernelAction::ReduceSchedulerPriority,
>
> \_ =\> KernelAction::NoAction,
>
> }
>
> }

**DIFFERENSIATOR:** Tidak ada bahasa lain yang memungkinkan ini dalam satu file, satu type system, satu compiler.

## 7. OS Primitives

Built-in macros untuk OS development, hanya tersedia dalam @kernel context.

### 7.1 Memory Management

> let region = alloc!(4096) // allocate 4KB
>
> free!(region) // free region
>
> map_page!(virt_addr, phys_addr, flags) // map virtual to physical

### 7.2 Interrupt Handling

> irq_register!(0x21, keyboard_handler) // register IRQ handler
>
> irq_enable!(0x21)
>
> irq_disable!(0x21)

### 7.3 System Calls

> syscall_define!(0x01, fn exit(code: i32))
>
> syscall_define!(0x02, fn read(fd: i32, buf: \*mut u8, n: usize) -\> isize)

### 7.4 I/O Ports (x86)

> port_write!(0x60, value: u8)
>
> let val: u8 = port_read!(0x60)

### 7.5 Memory Protection Flags

> const MEM_READ: u32 = 0x01
>
> const MEM_WRITE: u32 = 0x02
>
> const MEM_EXEC: u32 = 0x04
>
> const MEM_USER: u32 = 0x08

## 8. ML Primitives

Built-in functions untuk ML development. Tersedia globally, diakselerasi saat berada dalam @device context.

### 8.1 Tensor Creation

  ———————- ———— ————————————
  **Function**           **Return**   **Description**

  zeros(shape\...)       Tensor       All zeros

  ones(shape\...)        Tensor       All ones

  xavier(shape\...)      Tensor       Xavier/Glorot initialization

  randn(shape\...)       Tensor       Normal distribution N(0,1)

  eye(n)                 Tensor       Identity matrix
  ———————- ———— ————————————

### 8.2 Tensor Arithmetic

  ———————- ———— ————————————
  **Operation**          **Return**   **Description**

  a + b                  Tensor       Element-wise add (broadcast)

  a \* b                 Tensor       Element-wise multiply

  a @ b                  Tensor       Matrix multiply

  a.T                    Tensor       Transpose
  ———————- ———— ————————————

### 8.3 Activation Functions

> relu(x) -\> Tensor
>
> sigmoid(x) -\> Tensor
>
> tanh(x) -\> Tensor
>
> softmax(x) -\> Tensor
>
> gelu(x) -\> Tensor

### 8.4 Loss Functions

> mse_loss(pred, target) -\> f32
>
> cross_entropy(pred, target) -\> f32
>
> bce_loss(pred, target) -\> f32

### 8.5 Layer Constructors

> dense(in, out) -\> Layer
>
> conv2d(in_ch, out_ch, kernel) -\> Layer
>
> attention(d_model, n_heads) -\> Layer
>
> embedding(vocab, dim) -\> Layer

### 8.6 Automatic Differentiation (Autograd)

> x.backward() // compute gradients (reverse mode)
>
> x.grad // access gradient tensor
>
> with no_grad { \... } // disable gradient tracking

### 8.7 Optimizers

> sgd(lr: f32) -\> Optimizer
>
> adam(lr: f32, beta1: f32, beta2: f32) -\> Optimizer
>
> optimizer.step(params) // update parameters
>
> optimizer.zero_grad() // reset gradients

## 9. Structs, Traits & Implementations

### 9.1 Struct Definition & Implementation

> struct LinearLayer {
>
> weights: Tensor\<f32\>\[\*, \*\], // dynamic shape
>
> bias: Tensor\<f32\>\[\*\],
>
> in_dim: usize,
>
> out_dim: usize,
>
> }
>
> impl LinearLayer {
>
> fn new(in_dim: usize, out_dim: usize) -\> Self {
>
> LinearLayer {
>
> weights: xavier(in_dim, out_dim),
>
> bias: zeros(out_dim),
>
> in_dim,
>
> out_dim,
>
> }
>
> }
>
> fn forward(self, input: Tensor\<f32\>\[\*\]) -\> Tensor\<f32\>\[\*\] {
>
> relu(input @ self.weights + self.bias)
>
> }
>
> fn parameters(self) -\> \[Tensor\<f32\>\] {
>
> \[self.weights, self.bias\]
>
> }
>
> }

### 9.2 Trait Definition & Implementation

> // Trait definition
>
> trait Module {
>
> fn forward(self, input: Tensor) -\> Tensor
>
> fn parameters(self) -\> \[Tensor\]
>
> }
>
> // Implement trait
>
> impl Module for LinearLayer {
>
> fn forward(self, input: Tensor) -\> Tensor {
>
> self.forward(input)
>
> }
>
> fn parameters(self) -\> \[Tensor\] {
>
> self.parameters()
>
> }
>
> }

## 10. Module System

### 10.1 Declaring Modules

> // file: src/math.fj
>
> mod math {
>
> pub fn square(x: f64) -\> f64 { x \* x }
>
> fn internal() { \... } // private
>
> }

### 10.2 Using Modules

> use math::square
>
> use math::\* // glob import
>
> use os::{ alloc, free, map_page } // selective import

### 10.3 Standard Library Modules

> use std::io::{ print, println, eprintln }
>
> use std::collections::{ HashMap, Vec }
>
> use nn::{ dense, relu, adam } // ML stdlib
>
> use os::{ alloc, irq_register } // OS stdlib

## 11. Error Handling

Fajar Lang menggunakan Result-based error handling (tidak ada exceptions). Ini memaksa developer untuk selalu menangani error secara eksplisit.

> // Result-based error handling
>
> fn divide(a: f64, b: f64) -\> Result\<f64, str\> {
>
> if b == 0.0 {
>
> Err("division by zero")
>
> } else {
>
> Ok(a / b)
>
> }
>
> }
>
> // ? operator — propagate errors
>
> fn compute() -\> Result\<f64, str\> {
>
> let x = divide(10.0, 2.0)? // returns Err early if fails
>
> let y = divide(x, 3.0)?
>
> Ok(x + y)
>
> }
>
> // Pattern match on Result
>
> match divide(10.0, 0.0) {
>
> Ok(val) =\> println(val),
>
> Err(msg) =\> eprintln("Error: " + msg),
>
> }
>
> // Unwrap (panics — use only in tests or prototyping)
>
> let val = divide(10.0, 2.0).unwrap()
>
> let val = divide(10.0, 2.0).expect("divide should not fail")

## 12. Memory Safety Model

Fajar Lang menggunakan "ownership lite" — terinspirasi dari Rust tetapi tanpa lifetime annotations yang kompleks. Setiap nilai dimiliki oleh satu pemilik pada satu waktu.

### 12.1 Ownership

> fn main() {
>
> let data = \[1, 2, 3, 4, 5\] // data owns the array
>
> let copy = data // MOVE: data no longer valid
>
> println(data) // COMPILE ERROR: use after move
>
> // error\[ME001\]: use of moved value \'data\'
>
> }

### 12.2 Borrow Rules

Fajar Lang memiliki dua jenis borrow: immutable (&) dan mutable (&mut). Tidak boleh ada mutable borrow dan immutable borrow secara bersamaan.

> fn process(data: &\[i32\]) -\> i32 { // immutable borrow
>
> data.iter().sum()
>
> }
>
> fn modify(data: &mut \[i32\]) { // mutable borrow
>
> data\[0\] = 99
>
> }
>
> fn main() {
>
> let mut arr = \[1, 2, 3\]
>
> let sum = process(&arr) // immutable borrow OK
>
> modify(&mut arr) // mutable borrow OK
>
> // Tidak boleh mutable + immutable bersamaan:
>
> let r1 = &arr
>
> let r2 = &mut arr // COMPILE ERROR
>
> // error\[ME002\]: cannot borrow as mutable because also borrowed immutable
>
> }

### 12.3 Null Safety

Di Fajar Lang, null tidak ada di default context. Ketiadaan nilai direpresentasikan dengan Option\<T\>.

> fn find_user(id: u64) -\> Option\<User\> {
>
> if exists(id) { Some(load_user(id)) }
>
> else { None }
>
> }
>
> // Compiler memaksa penanganan kedua kasus:
>
> match find_user(42) {
>
> Some(user) =\> println(user.name),
>
> None =\> println("User not found"),
>
> }
>
> // Shorthand dengan ? operator:
>
> let name = find_user(42)?.name // propagate None
>
> let name = find_user(42).unwrap_or_default()

### 12.4 Bounds Checking

> let arr = \[1, 2, 3, 4, 5\]
>
> let val = arr\[10\] // RuntimeError: index out of bounds
>
> let maybe = arr.get(10) // Option\<i32\> = None (safe)
>
> // In @kernel: compiler bisa generate unchecked access
>
> // tapi programmer harus eksplisit:
>
> @kernel
>
> fn fast_read(arr: &\[u8\], idx: usize) -\> u8 {
>
> // SAFETY: caller guarantees idx \< arr.len()
>
> @unsafe { arr.get_unchecked(idx) }
>
> }

### 12.5 Integer Overflow

> let a: u8 = 255
>
> let b = a + 1 // debug: panic; release: wraps to 0
>
> // Explicit wrapping:
>
> let c = a.wrapping_add(1) // always wraps → 0
>
> let d = a.checked_add(1) // Option\<u8\> = None
>
> let e = a.saturating_add(1) // 255 (saturates at max)

## 13. Security Model

Fajar Lang mengadopsi prinsip "Security by Construction" — keamanan adalah properti yang dibuktikan oleh compiler, bukan konvensi yang diharapkan dari programmer. Filosofi: If it compiles, it\'s safe (dalam domain yang telah dispesifikasikan).

### 13.1 Tiga Pilar Security

  ——————- ————————————————————————————————— —————————————
  **Pilar**           **Mekanisme**                                                                                       **Enforcement**

  Memory Safety       No use-after-free, no null deref, no buffer overflow, no data race                                  Compiler (ownership + borrow checker)

  Context Isolation   @kernel ≠ @device, no heap in kernel, no tensor in kernel, no pointer in @safe                   Compiler (context analyzer)

  Type Safety         PhysAddr ≠ VirtAddr, tensor shape check, no implicit cast, exhaustive match, Result\<T,E\> always   Compiler (type checker)
  ——————- ————————————————————————————————— —————————————

### 13.2 Security Layers

> Layer 5: Application Security → Developer responsibility
>
> Layer 4: Safe Default (@safe) → Compiler enforced
>
> Layer 3: Memory Safety → Compiler enforced
>
> Layer 2: Context Isolation → Compiler enforced
>
> Layer 1: Type Safety → Compiler enforced
>
> Layer 0: Unsafe Boundary → Manual audit, clearly marked

### 13.3 No Implicit Type Conversions

> let x: i32 = 42
>
> let y: i64 = x // COMPILE ERROR: no implicit widening
>
> let y: i64 = x as i64 // explicit cast
>
> let y: i64 = i64::from(x) // trait-based conversion (preferred)
>
> let f: f32 = 3.14
>
> let d: f64 = f // COMPILE ERROR
>
> let d: f64 = f64::from(f) // OK

### 13.4 Tensor Shape Safety

> let w1: Tensor\<f32\>\[784, 128\]
>
> let w2: Tensor\<f32\>\[128, 64\]
>
> let r1 = w1 @ w2 // OK: \[784,128\] @ \[128,64\] = \[784,64\]
>
> let r2 = w2 @ w1 // COMPILE ERROR: shape mismatch
>
> // error\[TE002\]: matrix multiply shape mismatch
>
> // left: \[128, 64\]
>
> // right: \[784, 128\]
>
> // required: left.cols == right.rows (64 ≠ 784)

## 14. Compiler Architecture

Fajar Lang compiler diimplementasikan dalam Rust dan menggunakan pipeline multi-stage.

### 14.1 Compilation Pipeline

> Source (.fj file)
>
> │ raw text
>
> ▼
>
> LEXER (src/lexer/)
>
> Input: &str
>
> Output: Vec\<Token\>
>
> Errors: LexError { line, col, message }
>
> │ token stream
>
> ▼
>
> PARSER (src/parser/)
>
> Input: Vec\<Token\>
>
> Output: AST (Program node)
>
> Method: Recursive Descent + Pratt for expressions
>
> │ AST
>
> ▼
>
> SEMANTIC ANALYZER (src/analyzer/)
>
> Input: AST
>
> Output: Typed AST + Symbol Table
>
> Tasks: type checking, scope resolution,
>
> annotation validation, tensor shape check
>
> │ typed AST
>
> │ │
>
> ▼ ▼
>
> INTERPRETER (Future) COMPILER
>
> (Phase 1-4) LLVM backend
>
> Tree-walking (Phase 5+)
>
> │
>
> ▼
>
> RUNTIME
>
> OS Runtime ML Runtime
>
> memory.rs tensor.rs
>
> irq.rs autograd.rs
>
> syscall.rs ops.rs

### 14.2 Error Type Hierarchy

> pub enum FjError {
>
> Lex(Vec\<LexError\>),
>
> Parse(Vec\<ParseError\>),
>
> Semantic(Vec\<SemanticError\>),
>
> Runtime(RuntimeError),
>
> }
>
> pub enum SemanticErrorKind {
>
> TypeMismatch { expected: Type, got: Type },
>
> UndefinedSymbol(String),
>
> InvalidAnnotation { annotation: String, context: String },
>
> TensorShapeMismatch { expected: Vec\<usize\>, got: Vec\<usize\> },
>
> KernelContextViolation(String),
>
> }

### 14.3 Key Design Decisions

-   Tree-walking interpreter (not bytecode VM) — simplest path; upgrade to bytecode/LLVM in Phase 5

-   ndarray for tensor backend — mature, well-tested, supports SIMD via blas

-   Collect-all errors (not fail-fast) — show all errors at once like Rust compiler

-   Rc\<RefCell\<\>\> for environment — closures need shared mutable access; upgrade to Arc\<Mutex\<\>\> for concurrency

-   miette for error display — beautiful error output with source highlighting

## 15. Formal Grammar (EBNF)

> program = item\* EOF
>
> item = fn_def | struct_def | enum_def | impl_block
>
> | use_decl | mod_decl | const_def
>
> fn_def = annotation\* "fn" IDENT generic_params? params
>
> "-\>" type block_expr
>
> params = "(" (param ("," param)\*)? ")"
>
> param = IDENT ":" type
>
> block_expr = "{" stmt\* expr? "}"
>
> stmt = let_stmt | expr_stmt | return_stmt
>
> let_stmt = "let" "mut"? IDENT (":" type)? "=" expr ";"?
>
> expr_stmt = expr ";"?
>
> return_stmt = "return" expr? ";"?
>
> expr = assignment
>
> assignment = IDENT assign_op expr | pipeline
>
> assign_op = "=" | "+=" | "-=" | "\*=" | "/="
>
> pipeline = comparison ("|\>" IDENT)\*
>
> comparison = addition (cmp_op addition)\*
>
> addition = multiplication (("+"|"-") multiplication)\*
>
> multiplication = unary (("\*"|"/"|"@"|"%") unary)\*
>
> unary = ("!"|"-"|"\*"|"&") unary | call
>
> call = primary ("(" args ")" | "." IDENT | "\[" expr "\]")\*
>
> primary = INT | FLOAT | STRING | BOOL | IDENT
>
> | "(" expr ")" | block_expr | if_expr
>
> | match_expr | tensor_literal
>
> type = IDENT generic_args?
>
> | "Tensor" "\<" dtype "\>" "\[" dims "\]"
>
> | "\*" "const" type | "\*" "mut" type
>
> | "(" type ("," type)\* ")"
>
> | "\[" type "\]" | "\[" type ";" expr "\]"
>
> annotation = "@" IDENT ("(" args ")")?

## 16. Complete Example Programs

### 16.1 Hello World

> use std::io::println
>
> fn main() -\> void {
>
> println("Hello from Fajar Lang!")
>
> }

### 16.2 OS Memory Manager

> use os::{ alloc, free, map_page, MEM_READ, MEM_WRITE }
>
> struct MemBlock {
>
> base: addr\<u8\>,
>
> size: usize,
>
> used: bool,
>
> }
>
> @kernel
>
> fn init_memory(heap_start: addr\<u8\>, heap_size: usize) -\> Result\<void, str\> {
>
> let block = MemBlock {
>
> base: heap_start,
>
> size: heap_size,
>
> used: false,
>
> }
>
> map_page!(heap_start, heap_start, MEM_READ | MEM_WRITE)?
>
> Ok(())
>
> }

### 16.3 Simple Neural Network (MNIST)

> use nn::{ dense, relu, softmax, cross_entropy, adam }
>
> struct MnistNet {
>
> layer1: LinearLayer,
>
> layer2: LinearLayer,
>
> }
>
> impl MnistNet {
>
> fn new() -\> Self {
>
> MnistNet {
>
> layer1: LinearLayer::new(784, 128),
>
> layer2: LinearLayer::new(128, 10),
>
> }
>
> }
>
> @device(auto)
>
> fn forward(self, x: Tensor\<f32\>\[\*, 784\]) -\> Tensor\<f32\>\[\*, 10\] {
>
> let h = relu(self.layer1.forward(x))
>
> softmax(self.layer2.forward(h))
>
> }
>
> }
>
> fn train(net: mut MnistNet, data: Dataset, epochs: i32) {
>
> let opt = adam(lr: 0.001, beta1: 0.9, beta2: 0.999)
>
> for epoch in 0..epochs {
>
> for (x, y) in data {
>
> let pred = net.forward(x)
>
> let loss = cross_entropy(pred, y)
>
> loss.backward()
>
> opt.step(net.parameters())
>
> opt.zero_grad()
>
> }
>
> println("Epoch " + epoch + " done")
>
> }
>
> }

### 16.4 Cross-Domain: AI-Powered Kernel Monitor

> // Satu file menggabungkan OS dan ML domain
>
> use os::{ alloc, irq_register }
>
> use nn::{ dense, relu, softmax }
>
> // OS domain: boot initialization
>
> @kernel
>
> fn boot_init(mem_base: PhysAddr, mem_size: usize) -\> Result\<void, OsError\> {
>
> map_page!(mem_base, mem_base, MEM_READ | MEM_WRITE)?
>
> irq_register!(0x0E, page_fault_handler)
>
> Ok(())
>
> }
>
> // ML domain: inference
>
> @device(auto)
>
> fn inference_forward(x: Tensor\<f32\>\[1, 784\]) -\> Tensor\<f32\>\[1, 10\] {
>
> let h1 = relu(x @ W1 + b1)
>
> let h2 = relu(h1 @ W2 + b2)
>
> softmax(h2 @ W3 + b3)
>
> }
>
> // Bridge: OS data -\> ML inference -\> OS action
>
> @safe
>
> fn ai_kernel_monitor(sensor_data: \[f32; 784\]) -\> KernelAction {
>
> let input = Tensor::from_slice(sensor_data)
>
> let prediction = inference_forward(input)
>
> match prediction.argmax() {
>
> 0 =\> KernelAction::IncreaseMemoryPressure,
>
> 1 =\> KernelAction::ReduceSchedulerPriority,
>
> \_ =\> KernelAction::NoAction,
>
> }
>
> }

## 17. Development Roadmap

  ———-- —————————- —————————————————- ————— ————-
  **Phase**   **Nama**                     **Goal**                                             **Durasi**      **Status**

  1           Core Language Foundation     Working tree-walking interpreter                     8--12 minggu    IN PROGRESS

  2           Type System                  Static type checking catches errors before runtime   6--8 minggu     NOT STARTED

  3           OS Runtime                   OS-level programming capabilities                    8--10 minggu    NOT STARTED

  4           ML/AI Runtime                Native tensor operations and autograd                10--14 minggu   NOT STARTED

  5           Tooling & Compiler Backend   Developer experience + native compilation            12+ minggu      NOT STARTED

  6           Standard Library             Rich stdlib for both OS and ML domains               8 minggu        NOT STARTED

  7           Production Hardening         Ready for real use in OS and AI projects             Ongoing         NOT STARTED
  ———-- —————————- —————————————————- ————— ————-

### 17.1 Phase 1 Milestones (Current)

-   1.1 Lexer — Token types, all keywords, operators, literals

-   1.2 AST Definition — All expression, statement, and item nodes

-   1.3 Parser — Pratt parser for expressions, recursive descent for statements

-   1.4 Environment & Values — Value enum, scope chain, closures

-   1.5 Interpreter (Core) — All expressions and statements evaluation

-   1.6 CLI & REPL — fj run, fj repl, error display with miette

### 17.2 Phase 2: Type System

-   Type inference (Hindley-Milner lite)

-   Generic type parameters

-   Tensor type: static shape checking where possible

-   Context annotation validation (@kernel, @device, @unsafe)

### 17.3 Phase 3: OS Runtime

-   Memory allocator (simulated heap)

-   Virtual memory mapping (page tables)

-   IRQ registration and dispatch

-   Syscall table and port I/O

### 17.4 Phase 4: ML/AI Runtime

-   TensorValue struct with autograd

-   Computation graph (dynamic, tape-based)

-   All activation functions, loss functions, optimizers

-   SIMD acceleration via ndarray + BLAS

-   @device annotation (CPU dispatch)

### 17.5 Phase 5+: Tooling & Beyond

-   Code formatter (fj fmt), LSP server, syntax highlighting

-   Package manager (fj add, fj.toml)

-   Bytecode VM (faster than tree-walking)

-   LLVM IR generation (optional native compilation)

-   GPU backend via wgpu (for @device(gpu))

## 18. Competitive Positioning

### 18.1 Positioning Map

Fajar Lang menempati "blue ocean" — tidak ada bahasa lain yang menempati kuadran kanan atas (high OS capability + high ML capability) secara kohesif dan native.

### 18.2 Head-to-Head Comparison

  ————————-- ——-- ———- ———— ———- —————-
  **Feature**                **C**    **Rust**   **Python**   **Mojo**   **Fajar Lang**

  Bare metal / OS dev        ✅       ✅         ❌           ❌         ✅

  ML / AI development        ⚠️       ⚠️         ✅           ✅         ✅

  Native tensor types        ❌       ❌         ❌           ✅         ✅

  Autograd built-in          ❌       ❌         ❌           ⚠️         ✅

  Memory safety              ❌       ✅         ✅           ⚠️         ✅

  Context annotations        ❌       ❌         ❌           ❌         ✅

  Compile-time shape check   ❌       ❌         ❌           ⚠️         ✅

  Type-safe addresses        ❌       ⚠️         ❌           ❌         ✅

  OS + ML in one language    ❌       ❌         ❌           ❌         ✅

  Null safety                ❌       ✅         ❌           ⚠️         ✅

  Formal unsafe boundary     ❌       ✅         N/A          ❌         ✅
  ————————-- ——-- ———- ———— ———- —————-

✅ = native feature ⚠️ = partial/via library ❌ = not available

### 18.3 Value Proposition

*"The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides."*

*Fajar Lang — Comprehensive Documentation v1.0*

Spec Version: 0.1 | Status: Draft | Next Review: After Phase 2
