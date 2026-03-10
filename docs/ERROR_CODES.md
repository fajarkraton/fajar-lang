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
| PE009 | TrailingSeparator | Koma/titikkoma di akhir | `fn f(a, b,)` (warning) |
| PE010 | InvalidAnnotation | Annotation tidak valid | `@unknown fn f() { }` |

---

## 4. Semantic Errors (SE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| SE001 | UndefinedVariable | Variabel belum dideklarasikan |
| SE002 | UndefinedFunction | Fungsi belum dideklarasikan |
| SE003 | UndefinedType | Tipe belum dideklarasikan |
| SE004 | TypeMismatch | Tipe tidak cocok (expected vs actual) |
| SE005 | ArgumentCountMismatch | Jumlah argumen fungsi tidak sesuai |
| SE006 | DuplicateDefinition | Nama sudah didefinisikan di scope yang sama |
| SE007 | ImmutableAssignment | Assignment ke variabel immutable (tanpa `mut`) |
| SE008 | MissingReturnType | Fungsi tidak mengembalikan tipe yang dideklarasikan |
| SE009 | UnusedVariable | Variabel dideklarasikan tapi tidak dipakai (warning) |
| SE010 | UnreachableCode | Kode setelah return/break tidak bisa dijangkau (warning) |
| SE011 | NonExhaustiveMatch | Match expression tidak mencakup semua pattern |
| SE012 | MissingField | Struct initialization tidak lengkap |

---

## 5. Context Errors

### 5.1 Kernel Context Errors (KE)

| Code | Nama | Deskripsi | Contoh |
|------|------|-----------|--------|
| KE001 | HeapAllocInKernel | Heap allocation di `@kernel` | `String::new()`, `Vec::new()` |
| KE002 | TensorInKernel | Tensor operation di `@kernel` | `zeros()`, `relu()`, `.backward()` |
| KE003 | AsyncInKernel | Async operation di `@kernel` | `await`, `async fn` |
| KE004 | ClosureInKernel | Closure dengan capture di `@kernel` | `\|x\| x + captured_var` |

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
| DE002 | OsPrimitiveInDevice | OS primitive di `@device` | `irq_register!`, `port_write!`, `map_page!` |
| DE003 | InlineAsmInDevice | Inline assembly di `@device` | `asm!()` |

---

## 6. Tensor Errors (TE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| TE001 | ShapeMismatch | Dimensi tensor tidak kompatibel untuk operasi |
| TE002 | MatmulShapeMismatch | Inner dimensions tidak cocok untuk matrix multiply |
| TE003 | BroadcastError | Shape tidak bisa di-broadcast |
| TE004 | InvalidAxis | Axis melebihi dimensi tensor |
| TE005 | EmptyTensor | Operasi pada tensor dengan 0 elemen |
| TE006 | GradNotEnabled | `backward()` dipanggil tanpa `requires_grad` |
| TE007 | DoubleBackward | `backward()` dipanggil dua kali tanpa `retain_graph` |
| TE008 | DtypeMismatch | Operasi antara tensor dengan tipe data berbeda |

### Contoh Output TE002:

```
error[TE002]: matrix multiplication shape mismatch
  --> model.fj:12:15
   |
12 |     let c = a @ b
   |               ^ matmul requires inner dimensions to match
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
| RE005 | NullAccess | Akses `Option::None` tanpa unwrap check |
| RE006 | InvalidCast | Type cast yang tidak valid pada runtime |
| RE007 | AssertionFailed | `assert!()` atau `assert_eq!()` gagal |
| RE008 | PanicExplicit | `panic!()` dipanggil eksplisit |

---

## 8. Memory Errors (ME)

| Code | Nama | Deskripsi |
|------|------|-----------|
| ME001 | UseAfterMove | Akses value setelah ownership berpindah |
| ME002 | DoubleFree | Dealokasi memori yang sudah di-free |
| ME003 | BorrowConflict | Simultaneous mutable + immutable borrow |
| ME004 | MutableBorrowConflict | Dua mutable borrow ke value yang sama |
| ME005 | DanglingReference | Reference ke value yang sudah di-drop |
| ME006 | AllocFailed | `alloc!()` gagal (out of memory) |
| ME007 | InvalidFree | `free!()` pada address yang bukan dari `alloc!()` |
| ME008 | UnalignedAccess | Akses memory pada address yang tidak aligned |

---

*Error Codes Version: 1.1 | Total: 61 error codes across 8 categories*
*Updated: 2026-03-05 (count correction: LE=8, PE=10, SE=12, KE=4, DE=3, TE=8, RE=8, ME=8 = 61)*
