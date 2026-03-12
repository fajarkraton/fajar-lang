# Error Codes

Fajar Lang uses structured error codes across 12 categories. All errors include source locations and actionable suggestions.

## Lex Errors (LE)

| Code | Name | Description |
|------|------|-------------|
| LE001 | UnterminatedString | String literal missing closing quote |
| LE002 | InvalidChar | Unrecognized character in source |
| LE003 | UnterminatedComment | Block comment missing `*/` |
| LE004 | InvalidNumber | Malformed numeric literal |
| LE005 | InvalidEscape | Unknown escape sequence in string |
| LE006 | UnterminatedChar | Character literal missing closing quote |
| LE007 | EmptyChar | Empty character literal |
| LE008 | MultiChar | Character literal with multiple characters |

## Parse Errors (PE)

| Code | Name | Description |
|------|------|-------------|
| PE001 | UnexpectedToken | Token not expected at this position |
| PE002 | ExpectedExpression | Expression expected but not found |
| PE003 | ExpectedType | Type annotation expected |
| PE004 | UnmatchedParen | Mismatched parentheses or brackets |
| PE005 | ExpectedBlock | Block `{ }` expected |
| PE006 | InvalidPattern | Invalid pattern in match arm |
| PE007 | ExpectedIdentifier | Identifier name expected |
| PE008 | DuplicateField | Duplicate field in struct |
| PE009 | InvalidAssignment | Invalid left-hand side of assignment |
| PE010 | ExpectedSemicolon | Statement separator expected |

## Semantic Errors (SE)

| Code | Name | Description |
|------|------|-------------|
| SE001 | UndefinedVariable | Variable not defined in scope |
| SE002 | UndefinedFunction | Function not defined |
| SE003 | UndefinedType | Type not defined |
| SE004 | TypeMismatch | Expected type does not match actual type |
| SE005 | ArityMismatch | Wrong number of arguments |
| SE006 | ImmutableAssign | Cannot assign to immutable variable |
| SE007 | DuplicateDefinition | Name already defined in scope |
| SE008 | ReturnOutsideFunction | `return` outside function body |
| SE009 | UnusedVariable | Variable declared but never used (warning) |
| SE010 | UnreachableCode | Code after `return` or `break` (warning) |
| SE011 | MissingReturn | Function may not return a value |
| SE012 | InvalidContext | Operation invalid in current context |
| SE017 | AsyncReturnType | Async function must return `Future<T>` |
| SE019 | UnusedImport | Imported name never referenced (warning) |
| SE020 | UnreachablePattern | Match arm unreachable after wildcard (warning) |
| SE021 | LifetimeMismatch | Lifetime annotation conflict |

## Kernel Errors (KE)

| Code | Name | Description |
|------|------|-------------|
| KE001 | HeapAllocInKernel | Heap allocation in `@kernel` context |
| KE002 | TensorInKernel | Tensor operation in `@kernel` context |
| KE003 | DeviceCallInKernel | Calling `@device` function from `@kernel` |
| KE004 | InvalidKernelOp | Operation not allowed in `@kernel` |

## Device Errors (DE)

| Code | Name | Description |
|------|------|-------------|
| DE001 | RawPointerInDevice | Raw pointer in `@device` context |
| DE002 | HardwareInDevice | Hardware access in `@device` context |
| DE003 | InvalidDeviceOp | Operation not allowed in `@device` |

## Tensor Errors (TE)

| Code | Name | Description |
|------|------|-------------|
| TE001 | ShapeMismatch | Tensor shapes incompatible for operation |
| TE002 | InvalidReshape | Cannot reshape — total elements differ |
| TE003 | DimOutOfRange | Dimension index exceeds tensor rank |
| TE004 | EmptyTensor | Operation requires non-empty tensor |
| TE005 | DtypeMismatch | Tensor data type mismatch |
| TE006 | GradientError | Gradient computation failed |
| TE007 | QuantizationError | Quantization range error |
| TE008 | DeviceError | Tensor device transfer failed |
| TE009 | CompileTimeShapeError | Compile-time shape verification failed |

## Runtime Errors (RE)

| Code | Name | Description |
|------|------|-------------|
| RE001 | DivisionByZero | Division or modulo by zero |
| RE002 | IndexOutOfBounds | Array/tensor index out of range |
| RE003 | StackOverflow | Maximum recursion depth exceeded |
| RE004 | IntegerOverflow | Integer arithmetic overflow |
| RE005 | NullDereference | Null pointer dereference |
| RE006 | AssertionFailed | `assert` or `assert_eq` failed |
| RE007 | Timeout | Execution time limit exceeded |
| RE008 | OutOfMemory | Memory allocation failed |

## Memory Errors (ME)

| Code | Name | Description |
|------|------|-------------|
| ME001 | UseAfterMove | Variable used after ownership transfer |
| ME002 | DoubleFree | Value freed more than once |
| ME003 | BorrowConflict | Mutable and immutable borrows conflict |
| ME004 | DanglingReference | Reference outlives its target |
| ME005 | MoveInLoop | Value moved inside loop iteration |
| ME006 | PartialMove | Partially moved struct accessed |
| ME007 | BorrowInClosure | Closure captures conflicting borrow |
| ME008 | MutableAliasing | Multiple mutable references to same data |
| ME009 | LifetimeExpired | Borrowed reference outlives its source |
| ME010 | LifetimeConstraint | Lifetime constraint cannot be satisfied |

## Codegen Errors (CE)

| Code | Name | Description |
|------|------|-------------|
| CE001 | UnsupportedTarget | Target architecture not supported |
| CE002 | LinkError | Linker failed |
| CE003 | NotImplemented | Feature not yet in native codegen |
| CE004 | FunctionError | Cranelift verification error |
| CE005 | TypeCoercionError | Cannot coerce types in codegen |
| CE006 | UndefinedFunction | Function not available in native mode |
| CE007 | SymbolConflict | Duplicate symbol name |
| CE008 | AbiMismatch | ABI incompatibility |
| CE009 | LlvmError | LLVM backend error |
| CE010 | WasmError | WebAssembly backend error |

## Effect Errors (EE)

| Code | Name | Description |
|------|------|-------------|
| EE001 | UnhandledEffect | Effect not handled in scope |
| EE002 | EffectMismatch | Function's effect set doesn't match declaration |
| EE003 | PurityViolation | `#[pure]` function performs effectful operation |
| EE004 | EffectInKernel | `Alloc` effect used in `@kernel` |
| EE005 | EffectInDevice | `IO` effect used in `@device` |
| EE006 | ResumeTypeMismatch | Resume value type doesn't match handler |
| EE007 | EffectRecursion | Recursive effect handling |
| EE008 | HandlerMissing | Required effect handler not provided |

## Linear Type Errors (LN)

| Code | Name | Description |
|------|------|-------------|
| LN001 | UseAfterConsume | Linear resource used after consumption |
| LN002 | ResourceNotConsumed | Linear resource dropped without consumption |
| LN003 | DoubleConsume | Linear resource consumed twice |
| LN004 | ResourceEscaped | Linear resource escaped its scope |
| LN005 | BorrowConsumed | Borrow of already consumed resource |
| LN006 | LinearInNonLinear | Linear resource in non-linear context |
| LN007 | MustUseIgnored | Must-use resource return value ignored |
| LN008 | PinViolation | Pin protocol violated (configure-once) |

## GAT Errors (GE)

| Code | Name | Description |
|------|------|-------------|
| GE001 | GatKindMismatch | Associated type kind doesn't match declaration |
| GE002 | UnsatisfiedGatBound | GAT type parameter constraint not met |
| GE003 | ObjectUnsafe | Trait with GAT cannot be used as trait object |
