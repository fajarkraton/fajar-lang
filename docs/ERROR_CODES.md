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
| PE001 | UnexpectedToken | Token tidak sesuai grammar | `let = 42`; `fn f() {` (EOF as `}`) |
| PE002 | ExpectedExpression | Diharapkan expression | `let x = 1 +` (RHS missing) |
| PE003 | ExpectedType | Diharapkan type annotation | `fn f(x: ) { }` |
| PE004 | ExpectedPattern | Diharapkan pattern di binding/match | `match x { => 1 }` |
| PE005 | ExpectedIdentifier | Diharapkan identifier | `let = 42` |
| PE006 | UnexpectedEof | Source berakhir di tengah konstruksi | *(framework — saat ini di-route via PE001)* |
| PE007 | InvalidPattern | Pattern matching invalid | *(framework — saat ini di-route via PE001/PE004)* |
| PE008 | DuplicateField | Field struct duplikat | `P { x: 1, x: 2 }` |
| PE009 | TrailingSeparator | Trailing separator (warning) | *(framework — parser saat ini menerima trailing separator silently)* |
| PE010 | InvalidAnnotation | Annotation tidak dikenal/struktur salah | *(framework — saat ini di-route via PE001/PE002)* |
| PE011 | ModuleFileNotFound | `mod foo` tapi `foo.fj` tidak ada | *(framework — file resolution belum di-wire ke parser-driver)* |

> **PE006-PE007/PE009-PE011 framework status (2026-05-03):** Variants
> dideklarasikan di `src/parser/mod.rs::ParseError` untuk forward
> compatibility tetapi parser saat ini me-route kondisi-kondisi tersebut
> via PE001/PE002/PE004. Format string variant masih divalidasi di
> `tests/error_code_coverage.rs::coverage_pe00{6,7,9,a,b}_*_format` agar
> Display impl tidak drift. Jika di masa depan grammar membutuhkan
> diagnostic yang lebih halus, swap test ke parse-error trigger.

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
| SE013 | FfiUnsafeType / CannotInferType | Tipe non-FFI-safe di `extern fn` ATAU type inference gagal |
| SE014 | TraitBoundNotSatisfied | Generic bound tidak terpenuhi oleh argumen |
| SE015 | UnknownTrait | Trait belum di-deklarasikan dipakai di bound |
| SE016 | TraitMethodSignatureMismatch | `impl Trait` method signature tidak match deklarasi trait |
| SE017 | AwaitOutsideAsync | `.await` digunakan di luar `async fn` |
| SE018 | NotSendType | Tipe non-`Send` ditransfer ke thread lain |
| SE019 | UnusedImport | Import yang tidak digunakan (warning) |
| SE020 | UnreachablePattern / HwAccessInSafe | Match arm unreachable ATAU hardware access di `@safe` |
| SE021 | LifetimeMismatch / KernelCallFromSafe | Lifetime conflict ATAU `@kernel` dipanggil dari `@safe` |
| SE022 | IndexOutOfBounds / DeviceCallFromSafe | Compile-time index OOB ATAU `@device` dipanggil dari `@safe` |
| SE023 | QuantizedNotDequantized | `Quantized<T, B>` dipakai dimana `Tensor<T>` diharapkan — panggil `dequantize()` dulu |

#### SE023 — QuantizedNotDequantized

Tipe `Quantized` tidak boleh langsung dipakai sebagai `Tensor`. Data masih dalam
format packed (2/3/4/8-bit integer) dan akan menghasilkan garbage jika diinterpretasi
sebagai float. Gunakan `dequantize(q)` untuk konversi eksplisit.

```fajar
let t = from_data([1.0, -0.5], [2])
let q = quantize(t, 4)

// ERROR SE023: cannot use Quantized<f64, 4> where Tensor is expected
// matmul(q, q)

// OK: dequantize first
let d = dequantize(q)
matmul(d, d)
```

---

## 5. Context Errors

### 5.1 Kernel Context Errors (KE)

| Code | Nama | Deskripsi | Contoh |
|------|------|-----------|--------|
| KE001 | HeapAllocInKernel | Heap allocation di `@kernel` | `String::new()`, `Vec::new()` |
| KE002 | TensorInKernel | Tensor operation di `@kernel` | `zeros()`, `relu()`, `.backward()` |
| KE003 | DeviceCallInKernel | Calling `@device` function dari `@kernel` | `@device fn` dipanggil dalam `@kernel` |
| KE004 | InvalidKernelOp | Operasi tidak diperbolehkan di `@kernel` | Operasi yang memerlukan heap |
| KE005 | AsmInSafeContext | `asm!()` inline assembly di `@safe` | Inline asm tanpa `@unsafe` |
| KE006 | AsmInDeviceContext | `asm!()` inline assembly di `@device` | Inline asm di kode device |

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
| TE004 | RankMismatch | Tensor rank yang diharapkan tidak match |
| TE005 | NonScalarBackward | `.backward()` butuh scalar tensor (numel=1) |
| TE006 | NoGradient | Gradient tidak tersedia (`requires_grad=false` atau belum dihitung) |
| TE007 | DivisionByZero | Pembagian dengan nol pada elemen tensor |
| TE008 | TensorOpError | Operasi tensor generik gagal (catch-all) |
| TE009 | GpuShapeMismatch | GPU tensor shape mismatch |
| TE010 | GpuOutOfMemory | GPU memory exhausted (OOM) |

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
| RE002 | TypeError | Runtime type error (e.g. invalid coercion) |
| RE003 | StackOverflow | Rekursi terlalu dalam (max recursion depth) |
| RE004 | UndefinedVariable | Variabel tidak dikenal saat eval (REPL/dynamic) |
| RE005 | NotAFunction | Mencoba memanggil value yang bukan fungsi |
| RE006 | ArgumentCountMismatch | Jumlah argumen runtime tidak sesuai |
| RE007 | InvalidAssignment | Assignment ke target yang tidak valid (lvalue) |
| RE008 | OtherRuntimeError | Catch-all runtime error (assert fail, panic, dll.) |
| RE009 | IntegerOverflow | Overflow pada arithmetic |
| RE010 | IndexOutOfBounds | Akses array/slice di luar batas |

---

## 8. Memory Errors (ME)

| Code | Nama | Deskripsi |
|------|------|-----------|
| ME001 | UseAfterMove | Akses value setelah ownership berpindah |
| ME002 | DoubleFree | Dealokasi memori yang sudah di-free (runtime/OS layer) |
| ME003 | BorrowConflict | Simultaneous mutable + immutable borrow |
| ME004 | DanglingReference | Reference ke value yang sudah di-drop |
| ME005 | MoveInLoop | Value moved inside loop iteration |
| ME006 | AllocFailed | Heap/region allocation failed (OS runtime) |
| ME007 | InvalidFree | Invalid free (addr tidak valid / sudah di-free) |
| ME008 | MutableAliasing | Multiple mutable references ke data yang sama |
| ME009 | LifetimeConflict | Lifetime '{name}' conflicts with another lifetime in scope |
| ME010 | LinearNotConsumed | Linear value tidak consumed — must be used exactly once |
| ME011 | TwoPhaseConflict | Two-phase borrow conflict (polonius solver) |
| ME012 | ReborrowConflict | Reborrow conflict (polonius solver) |
| ME013 | PlaceConflict | Place / path conflict (polonius solver) |

---

## 9. Codegen Errors (CE)

| Code | Nama | Deskripsi |
|------|------|-----------|
| CE001 | UnsupportedExpr | Expression tidak didukung native codegen |
| CE002 | UnsupportedStmt | Statement tidak didukung native codegen |
| CE003 | UnsupportedType | Cannot lower type ke native representation |
| CE004 | FunctionError | Cranelift function verification error |
| CE005 | UndefinedVarInCodegen | Variabel undefined saat codegen |
| CE006 | UndefinedFunctionInCodegen | Fungsi undefined saat codegen |
| CE007 | AbiError | ABI/calling-convention error |
| CE008 | ModuleError | Cranelift module / linkage error |
| CE009 | InternalCodegenError | Internal codegen invariant violation |
| CE010 | NotYetImplemented | Fitur belum diimplementasi di native codegen |
| CE011 | ContextViolation | `@kernel`/`@device` context violated saat codegen |
| CE013 | GpuNotAvailable | GPU compute tidak tersedia di sistem |

> **Note:** CE012 sengaja di-skip untuk hindari konflik runtime tags.

### 9.1 No-std Errors (NS)

| Code | Nama | Deskripsi |
|------|------|-----------|
| NS001 | NoStdViolation | Operasi membutuhkan `std` di build `no_std` (kernel target) |

---

## 10. Compile-Time Errors (CT)

| Code | Nama | Deskripsi |
|------|------|-----------|
| CT001 | NotComptime | Expression cannot be evaluated at compile time |
| CT002 | Overflow | Arithmetic overflow in comptime evaluation |
| CT003 | DivisionByZero | Division by zero in comptime evaluation |
| CT004 | UndefinedVariable | Undefined variable in comptime context |
| CT005 | UndefinedFunction | Undefined function in comptime context |
| CT006 | RecursionLimit | Comptime evaluation recursion limit exceeded (256) |
| CT007 | IoForbidden | I/O operations not allowed in comptime |
| CT008 | TypeError | Type error in comptime evaluation |
| CT009 | HeapAllocInConstFn | Heap allocation not allowed in const fn |
| CT010 | MutableInConstFn | Mutable variables not allowed in const fn |
| CT011 | NonConstCall | Non-const function call in const fn |
| CT012 | ConstFnRecursionLimit | Const fn recursion limit exceeded |
| CT013 | ConstFnOverflow | Arithmetic overflow in const fn evaluation |

---

## 11. Effect Errors (EE)

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

## 11. Linear Type Errors (LN) — *forward-compat / future-work*

> **Status (2026-05-03):** Tabel LN001-LN008 dipertahankan untuk forward
> compatibility dengan rencana resource-typing terpisah. Implementasi
> linear-type enforcement saat ini dijalankan oleh **`ME010
> LinearNotConsumed`** (lihat §8). Tidak ada source-side `LN###`
> emission di v32 codebase. Code akan diaktivasi ulang jika resource
> tracking dipisahkan dari memory-error tree.

| Code | Nama | Deskripsi (forward-compat) |
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
| GE000 | GatTopLevel | Top-level catch-all untuk GAT-related errors |
| GE001 | ParamCountMismatch | Associated type param count tidak match trait deklarasi |
| GE002 | BoundMismatch | GAT bound trait tidak match deklarasi |
| GE003 | LifetimeCaptureError | Borrowed data tidak hidup cukup lama untuk GAT projection |
| GE004 | AsyncTraitNotObjectSafe | Async trait method tidak object-safe tanpa `#[async_trait]` |
| GE005 | NoAssocTypeOnTrait | Trait tidak punya associated type yang dirujuk |
| GE006 | DuplicateAssocType | Duplicate associated type dalam trait |
| GE007 | ParamKindMismatch | Param kind (lifetime/type/const) tidak sesuai |
| GE008 | ImplMissingAssocType | `impl Trait` tidak menyediakan associated type yang required |

---

*Error Codes Version: 4.0 | Total: 136 error codes across 13 categories
(LE 8 + PE 11 + SE 23 + KE 6 + DE 3 + TE 10 + RE 10 + ME 13 + CE 12 + NS 1 + CT 13 + EE 8 + LN 8 forward-compat + GE 9 + extras).*
*Updated: 2026-05-03 (v4.0 — reconciled with src/ emission per HONEST_AUDIT_V32 followup; PE descriptions corrected to actual variants; LN annotated forward-compat).*
