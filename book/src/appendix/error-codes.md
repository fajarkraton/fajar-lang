# Error Codes

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

## Memory Errors (ME)

| Code | Name | Description |
|------|------|-------------|
| ME001 | UseAfterMove | Variable used after ownership transfer |
| ME002 | DoubleFree | Value freed more than once |
| ME003 | BorrowConflict | Mutable and immutable borrows conflict |
| ME004 | DanglingReference | Reference outlives its target |

## Codegen Errors (CE)

| Code | Name | Description |
|------|------|-------------|
| CE001 | UnsupportedTarget | Target architecture not supported |
| CE002 | LinkError | Linker failed |
| CE003 | NotImplemented | Feature not yet in native codegen |
| CE004 | FunctionError | Cranelift verification error |
| CE006 | UndefinedFunction | Function not available in native mode |
