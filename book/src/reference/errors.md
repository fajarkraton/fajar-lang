# Error Codes

Fajar Lang uses a structured error code system. All errors include source spans and are rendered with `miette` for clear diagnostics.

## Error Code Format

```
[PREFIX][NUMBER]
```

Prefixes identify the compiler phase that produced the error.

## Lex Errors (LE)

Errors during tokenization of source code.

| Code | Name | Description | Trigger Example |
|------|------|-------------|-----------------|
| LE001 | UnexpectedChar | Unrecognized character | `@` in wrong position |
| LE002 | UnterminatedString | String literal never closed | `"hello` |
| LE003 | UnterminatedComment | Block comment never closed | `/* no end` |
| LE004 | InvalidNumberLiteral | Malformed number literal | `0xGG`, `0b12` |
| LE005 | InvalidEscape | Unknown escape sequence | `"\q"` |
| LE006 | NumberOverflow | Integer literal exceeds range | `99999999999999999999` |
| LE007 | EmptyCharLiteral | Char literal with no character | `''` |
| LE008 | MultiCharLiteral | Char literal with multiple chars | `'ab'` |

**Fix:** Check string/comment delimiters, number format, and escape sequences.

## Parse Errors (PE)

Errors during syntactic analysis.

| Code | Name | Description | Trigger Example |
|------|------|-------------|-----------------|
| PE001 | UnexpectedToken | Token does not fit grammar | `let = 42` |
| PE002 | ExpectedExpression | Expression required but missing | `let x = ;` |
| PE003 | ExpectedType | Type annotation required | `fn f(x: ) {}` |
| PE004 | ExpectedIdentifier | Identifier required | `let 42 = x` |
| PE005 | ExpectedBlock | Block `{ }` required | `if true return x` |
| PE006 | UnmatchedParen | Mismatched brackets | `(1 + 2` |
| PE007 | InvalidPattern | Invalid match pattern | `match x { 1+2 => }` |
| PE008 | DuplicateField | Repeated struct field | `Point { x: 1, x: 2 }` |
| PE009 | InvalidAssignment | Bad left-hand side | `1 + 2 = x` |
| PE010 | ExpectedSemicolon | Missing statement separator | Missing newline |

**Fix:** Check syntax against the grammar reference. Ensure matching delimiters.

## Semantic Errors (SE)

Errors during type checking and scope analysis.

| Code | Name | Description | Trigger Example |
|------|------|-------------|-----------------|
| SE001 | UndefinedVariable | Variable not declared | `println(x)` without `let x` |
| SE002 | UndefinedFunction | Function not declared | `foo()` without `fn foo` |
| SE003 | UndefinedType | Type not declared | `let x: Foo = ...` |
| SE004 | TypeMismatch | Incompatible types | `let x: i32 = "hi"` |
| SE005 | ArgumentCountMismatch | Wrong number of arguments | `add(1)` for `fn add(a, b)` |
| SE006 | ImmutableAssignment | Assign to non-mut variable | `let x = 1; x = 2` |
| SE007 | DuplicateDefinition | Name already defined | Two `fn foo()` in same scope |
| SE008 | ReturnOutsideFunction | `return` outside function | `return 5` at top level |
| SE009 | UnusedVariable | Variable never used (warning) | `let x = 5` then never read |
| SE010 | UnreachableCode | Code after return (warning) | Code after `return` |
| SE011 | MissingReturn | Function may not return | Missing `return` on some paths |
| SE012 | InvalidContext | Operation invalid in context | Tensor ops in `@kernel` |

**Fix:** Check variable/function names, type annotations, and mutability.

## Kernel Context Errors (KE)

Violations of `@kernel` context restrictions.

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| KE001 | HeapAllocInKernel | Heap allocation in `@kernel` | Use stack arrays or `alloc!()` |
| KE002 | TensorInKernel | Tensor ops in `@kernel` | Move to `@device` function |
| KE003 | DeviceCallInKernel | Calling `@device` fn from `@kernel` | Use `@safe` bridge function |
| KE004 | InvalidKernelOp | Disallowed operation | Check context requirements |

## Device Context Errors (DE)

Violations of `@device` context restrictions.

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| DE001 | RawPointerInDevice | Raw pointer in `@device` | Move to `@kernel` function |
| DE002 | HardwareInDevice | Hardware access in `@device` | Move to `@kernel` function |
| DE003 | InvalidDeviceOp | Disallowed operation | Check context requirements |

## Tensor Errors (TE)

Errors in tensor operations.

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| TE001 | ShapeMismatch | Incompatible shapes | Check dimensions match for operation |
| TE002 | InvalidReshape | Element count mismatch | Ensure total elements are equal |
| TE003 | DimOutOfRange | Dimension index too large | Use valid dimension index |
| TE004 | EmptyTensor | Operation on empty tensor | Check tensor is non-empty |
| TE005 | DtypeMismatch | Mismatched data types | Cast tensors to same type |
| TE006 | GradientError | Gradient computation failed | Check computation graph |
| TE007 | QuantizationError | Quantization range error | Check value ranges |
| TE008 | DeviceError | Device transfer failed | Check device availability |

## Runtime Errors (RE)

Errors during program execution.

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| RE001 | DivisionByZero | Division by zero | Check denominator before dividing |
| RE002 | IndexOutOfBounds | Array index out of range | Validate index against `len()` |
| RE003 | StackOverflow | Recursion too deep (>1024) | Add base case or use iteration |
| RE004 | IntegerOverflow | Arithmetic overflow | Use wider type or check range |
| RE005 | NullDereference | Null pointer access | Use `Option<T>` and `match` |
| RE006 | AssertionFailed | `assert`/`assert_eq` failed | Fix the assertion condition |
| RE007 | Timeout | Execution time exceeded | Optimize or increase limit |
| RE008 | OutOfMemory | Allocation failed | Reduce memory usage |

## Memory Errors (ME)

Ownership and borrowing violations.

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| ME001 | UseAfterMove | Access after ownership moved | Clone before move, or restructure |
| ME002 | DoubleFree | Freeing already-freed memory | Remove duplicate free |
| ME003 | BorrowConflict | Mutable + immutable borrow | Separate borrow lifetimes |
| ME004 | DanglingReference | Reference to dropped value | Extend value lifetime |
| ME005 | MoveInLoop | Value moved inside loop | Clone or use reference |
| ME006 | PartialMove | Partially moved struct used | Access only unmoved fields |
| ME007 | BorrowInClosure | Closure captures conflicting borrow | Use `clone()` or restructure |
| ME008 | MutableAliasing | Multiple `&mut` to same data | Use only one `&mut` at a time |

## Codegen Errors (CE)

Errors during native code generation.

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| CE001 | UnsupportedTarget | Unknown target architecture | Use supported target |
| CE002 | LinkError | Linker failed | Check system linker is installed |
| CE003 | NotImplemented | Feature missing in codegen | Use interpreter mode |
| CE004 | FunctionError | Cranelift verification failed | Report compiler bug |
| CE005 | TypeCoercionError | Cannot coerce types | Add explicit `as` cast |
| CE006 | UndefinedFunction | Function not available native | Check function exists |
| CE007 | SymbolConflict | Duplicate symbol name | Rename one definition |
| CE008 | AbiMismatch | ABI incompatibility | Match calling conventions |
| CE009 | LlvmError | LLVM backend error | Check LLVM installation |
| CE010 | WasmError | WebAssembly error | Check wasm target support |

## Reading Error Output

```
error[SE004]: type mismatch
  --> main.fj:5:15
   |
5  |     let x: i32 = "hello"
   |                   ^^^^^^^ expected i32, found str
   |
   = help: use parse_int("hello") to convert str to i32
```

- **Error code** in brackets identifies the category
- **Arrow** points to the source file and location
- **Underline** highlights the problematic code
- **Help** suggests a fix
