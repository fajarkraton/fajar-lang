# ERROR CODES

> Katalog Kode Error Lengkap — Fajar Lang Compiler Error Reference

---

## 1. Format Kode Error

Setiap error di Fajar Lang menggunakan format kode yang konsisten:

```
Format: [PREFIX][NUMBER]

Prefix:
  LE  = Lex Error (tokenization)
  PE  = Parse Error (syntax)
  SE  = Semantic Error (type/scope)
  KE  = Kernel Context Error
  DE  = Device Context Error
  TE  = Tensor Error (shape/type)
  RE  = Runtime Error (execution)
  ME  = Memory Error (ownership/borrow)
  CE  = Codegen Error (native compilation)
  EE  = Effect Error (algebraic effects)
  LN  = Linear Type Error (resource tracking)
  GE  = GAT Error (generic associated types)

Number: 3-digit sequential (001, 002, ...)
```

> **Error Display:** Semua error menggunakan `miette` untuk output yang beautiful dengan source highlighting, mirip compiler Rust.

---

## 2. Lex Errors (LE)

| Code | Nama | Deskripsi | Contoh Trigger |
|------|------|-----------|----------------|
| LE001 | UnexpectedChar | Karakter tidak dikenali | `@ #` di posisi salah |
| LE002 | UnterminatedString | String literal tidak ditutup | `"hello` (tanpa closing `"`) |
| LE003 | UnterminatedComment | Block comment tidak ditutup | `/*` tanpa `*/` |
| LE004 | InvalidNumberLiteral | Format angka salah | `0xGG`, `0b12` |
| LE005 | InvalidEscape | Escape sequence tidak valid | `"\q"` (bukan `\n`, `\t`, dll) |
| LE006 | NumberOverflow | Integer literal melebihi batas | `99999999999999999999` |
| LE007 | EmptyCharLiteral | Char literal kosong | `''` (tanpa karakter) |
| LE008 | MultiCharLiteral | Char literal > 1 karakter | `'ab'` |

### Contoh Output LE002:

```
error[LE002]: unterminated string literal
  --> main.fj:3:12
   |
3  | let name = "hello
   |            ^ string starts here but never ends
   |
   = help: add closing `"` to terminate the string
```

---

## 3. Parse Errors (PE)

| Code | Nama | Deskripsi | Contoh Trigger |
|------|------|-----------|----------------|
| PE001 | UnexpectedToken | Token tidak sesuai grammar | `let = 42` (missing ident) |
| PE002 | ExpectedExpression | Diharapkan expression | `let x = ;` (missing expr) |
| PE003 | ExpectedType | Diharapkan type annotation | `fn f(x: ) { }` |
| PE004 | ExpectedIdentifier | Diharapkan identifier | `let 42 = x` |
| PE005 | ExpectedBlock | Diharapkan `{ ... }` | `if true return x` |
| PE006 | UnmatchedParen | Bracket/paren tidak cocok | `(1 + 2` |
| PE007 | InvalidPattern | Pattern matching invalid | `match x { 1 + 2 => }` |
| PE008 | DuplicateField | Field struct duplikat | `Point { x: 1, x: 2 }` |
| PE009 | InvalidAssignment | Invalid left-hand side assignment | `1 + 2 = x` |
| PE010 | ExpectedSemicolon | Statement separator expected | Missing `;` or newline |

---

## 4. Semantic Errors (SE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| SE001 | UndefinedVariable | Variabel belum dideklarasikan |
| SE002 | UndefinedFunction | Fungsi belum dideklarasikan |
| SE003 | UndefinedType | Tipe belum dideklarasikan |
| SE004 | TypeMismatch | Tipe tidak cocok (expected vs actual) |
| SE005 | ArgumentCountMismatch | Jumlah argumen fungsi tidak sesuai |
| SE006 | ImmutableAssignment | Assignment ke variabel immutable (tanpa `mut`) |
| SE007 | DuplicateDefinition | Nama sudah didefinisikan di scope yang sama |
| SE008 | ReturnOutsideFunction | `return` di luar function body |
| SE009 | UnusedVariable | Variabel dideklarasikan tapi tidak dipakai (warning) |
| SE010 | UnreachableCode | Kode setelah return/break tidak bisa dijangkau (warning) |
| SE011 | MissingReturn | Fungsi mungkin tidak mengembalikan value |
| SE012 | InvalidContext | Operasi invalid di context saat ini |
| SE017 | AsyncReturnType | Async function harus return `Future<T>` |
| SE019 | UnusedImport | Import yang tidak digunakan (warning) |
| SE020 | UnreachablePattern | Match arm unreachable setelah wildcard (warning) |
| SE021 | LifetimeMismatch | Lifetime annotation conflict |

---

## 5. Context Errors

### 5.1 Kernel Context Errors (KE)

| Code | Nama | Deskripsi | Contoh |
|------|------|-----------|--------|
| KE001 | HeapAllocInKernel | Heap allocation di `@kernel` | `String::new()`, `Vec::new()` |
| KE002 | TensorInKernel | Tensor operation di `@kernel` | `zeros()`, `relu()`, `.backward()` |
| KE003 | DeviceCallInKernel | Calling `@device` function dari `@kernel` | `@device fn` dipanggil dalam `@kernel` |
| KE004 | InvalidKernelOp | Operasi tidak diperbolehkan di `@kernel` | Operasi yang memerlukan heap |

#### Contoh Output KE001:

```
error[KE001]: heap allocation not allowed in @kernel context
  --> kernel.fj:8:5
   |
7  | @kernel
8  | fn init_kernel() {
9  |     let s = String::new();
   |             ^^^^^^^^^^^^^ heap allocation here
   |
   = note: @kernel context operates without heap allocator
   = help: use stack-allocated arrays or alloc!() for raw memory
```

### 5.2 Device Context Errors (DE)

| Code | Nama | Deskripsi | Contoh |
|------|------|-----------|--------|
| DE001 | RawPointerInDevice | Raw pointer di `@device` | `*mut T`, `*const T` |
| DE002 | HardwareInDevice | Hardware access di `@device` | `irq_register!`, `port_write!`, `map_page!` |
| DE003 | InvalidDeviceOp | Operasi tidak diperbolehkan di `@device` | `asm!()`, raw memory access |

---

## 6. Tensor Errors (TE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| TE001 | ShapeMismatch | Dimensi tensor tidak kompatibel untuk operasi |
| TE002 | InvalidReshape | Cannot reshape — total elemen berbeda |
| TE003 | DimOutOfRange | Dimension index melebihi rank tensor |
| TE004 | EmptyTensor | Operasi pada tensor dengan 0 elemen |
| TE005 | DtypeMismatch | Operasi antara tensor dengan tipe data berbeda |
| TE006 | GradientError | Gradient computation gagal |
| TE007 | QuantizationError | Quantization range error |
| TE008 | DeviceError | Tensor device transfer gagal |
| TE009 | CompileTimeShapeError | Compile-time shape verification gagal |

### Contoh Output TE001:

```
error[TE001]: tensor shape mismatch
  --> model.fj:12:15
   |
12 |     let c = a @ b
   |               ^ shapes incompatible for matmul
   |
   = note: left shape: [3, 4], right shape: [5, 6]
   = note: expected right rows = 4, got 5
   = help: transpose b to shape [6, 5] or reshape a
```

---

## 7. Runtime Errors (RE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| RE001 | DivisionByZero | Pembagian dengan nol |
| RE002 | IndexOutOfBounds | Akses array/slice di luar batas |
| RE003 | StackOverflow | Rekursi terlalu dalam (> 1024 frames) |
| RE004 | IntegerOverflow | Overflow pada arithmetic (debug mode) |
| RE005 | NullDereference | Null pointer dereference |
| RE006 | AssertionFailed | `assert` atau `assert_eq` gagal |
| RE007 | Timeout | Execution time limit exceeded |
| RE008 | OutOfMemory | Memory allocation gagal |

---

## 8. Memory Errors (ME)

| Code | Nama | Deskripsi |
|------|------|-----------|
| ME001 | UseAfterMove | Akses value setelah ownership berpindah |
| ME002 | DoubleFree | Dealokasi memori yang sudah di-free |
| ME003 | BorrowConflict | Simultaneous mutable + immutable borrow |
| ME004 | DanglingReference | Reference ke value yang sudah di-drop |
| ME005 | MoveInLoop | Value moved inside loop iteration |
| ME006 | PartialMove | Partially moved struct accessed |
| ME007 | BorrowInClosure | Closure captures conflicting borrow |
| ME008 | MutableAliasing | Multiple mutable references ke data yang sama |
| ME009 | LifetimeExpired | Borrowed reference outlives source |
| ME010 | LifetimeConstraint | Lifetime constraint cannot be satisfied |

---

## 9. Codegen Errors (CE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| CE001 | UnsupportedTarget | Target architecture tidak didukung |
| CE002 | LinkError | Linker gagal |
| CE003 | NotImplemented | Fitur belum ada di native codegen |
| CE004 | FunctionError | Cranelift verification error |
| CE005 | TypeCoercionError | Cannot coerce types di codegen |
| CE006 | UndefinedFunction | Function tidak tersedia di native mode |
| CE007 | SymbolConflict | Duplicate symbol name |
| CE008 | AbiMismatch | ABI incompatibility |
| CE009 | LlvmError | LLVM backend error |
| CE010 | WasmError | WebAssembly backend error |

---

## 10. Effect Errors (EE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| EE001 | UnhandledEffect | Effect tidak di-handle dalam scope |
| EE002 | EffectMismatch | Effect set fungsi tidak sesuai deklarasi |
| EE003 | PurityViolation | `#[pure]` function melakukan operasi effectful |
| EE004 | EffectInKernel | `Alloc` effect digunakan di `@kernel` |
| EE005 | EffectInDevice | `IO` effect digunakan di `@device` |
| EE006 | ResumeTypeMismatch | Resume value type tidak sesuai handler |
| EE007 | EffectRecursion | Recursive effect handling |
| EE008 | HandlerMissing | Required effect handler tidak disediakan |

---

## 11. Linear Type Errors (LN)

| Code | Nama | Deskripsi |
|------|------|-----------|
| LN001 | UseAfterConsume | Linear resource digunakan setelah consumption |
| LN002 | ResourceNotConsumed | Linear resource di-drop tanpa consumption |
| LN003 | DoubleConsume | Linear resource di-consume dua kali |
| LN004 | ResourceEscaped | Linear resource escaped scope-nya |
| LN005 | BorrowConsumed | Borrow dari resource yang sudah consumed |
| LN006 | LinearInNonLinear | Linear resource di non-linear context |
| LN007 | MustUseIgnored | Must-use resource return value diabaikan |
| LN008 | PinViolation | Pin protocol dilanggar (configure-once) |

---

## 12. GAT Errors (GE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| GE001 | GatKindMismatch | Associated type kind tidak sesuai deklarasi |
| GE002 | UnsatisfiedGatBound | GAT type parameter constraint tidak terpenuhi |
| GE003 | ObjectUnsafe | Trait dengan GAT tidak bisa digunakan sebagai trait object |

---

*Error Codes Version: 3.0 | Total: 95 error codes across 12 categories*
*Updated: 2026-03-12 (v3.0 — added CE, EE, LN, GE categories; updated SE, ME, TE codes)*
