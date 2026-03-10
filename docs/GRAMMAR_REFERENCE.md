# GRAMMAR REFERENCE

> Formal EBNF Grammar Specification — Fajar Lang

---

## 1. Notation

```
Notation EBNF yang digunakan:
  rule      = definition ;
  'literal' = terminal string
  UPPER     = token dari lexer
  |         = alternatif
  [ ... ]   = opsional (0 atau 1)
  { ... }   = repetisi (0 atau lebih)
  ( ... )   = grouping
```

---

## 2. Program Structure

```ebnf
program     = { item } EOF ;

item        = annotation fn_def
            | annotation struct_def
            | annotation enum_def
            | annotation const_def
            | impl_block
            | use_decl
            | mod_decl
            | trait_def ;

annotation  = [ '@' IDENT [ '(' annotation_args ')' ] ] ;
annotation_args = IDENT { ',' IDENT } ;
```

---

## 3. Declarations

```ebnf
fn_def      = 'fn' IDENT [ generic_params ] '(' params ')' [ '->' type_expr ] block_expr ;
params      = [ param { ',' param } ] ;
param       = IDENT ':' type_expr ;
generic_params = '<' generic_param { ',' generic_param } '>' ;
generic_param  = IDENT [ ':' trait_bound { '+' trait_bound } ] ;
trait_bound    = IDENT [ '<' type_expr { ',' type_expr } '>' ] ;

struct_def  = 'struct' IDENT [ generic_params ] '{' struct_fields '}' ;
struct_fields = field { ',' field } [ ',' ] ;
field       = IDENT ':' type_expr ;

enum_def    = 'enum' IDENT [ generic_params ] '{' enum_variants '}' ;
enum_variants = variant { ',' variant } [ ',' ] ;
variant     = IDENT [ '(' type_list ')' ] ;

impl_block  = 'impl' [ generic_params ] [ trait_name 'for' ] type_name '{' { annotation fn_def } '}' ;

trait_def   = 'trait' IDENT [ generic_params ] '{' { trait_method } '}' ;
trait_method = 'fn' IDENT '(' params ')' [ '->' type_expr ] [ block_expr ] ';'? ;

const_def   = 'const' IDENT ':' type_expr '=' expr ';'? ;

use_decl    = 'use' use_path ';' ;
use_path    = IDENT { '::' IDENT } [ '::' ( '*' | '{' IDENT { ',' IDENT } '}' ) ] ;

mod_decl    = 'mod' IDENT [ '{' { item } '}' ] ;
```

---

## 4. Statements

```ebnf
stmt        = let_stmt | const_stmt | expr_stmt | return_stmt ;

let_stmt    = 'let' [ 'mut' ] IDENT [ ':' type_expr ] '=' expr ';'? ;
const_stmt  = 'const' IDENT ':' type_expr '=' expr ';'? ;

expr_stmt   = expr ';'? ;
return_stmt = 'return' [ expr ] ';'? ;
```

### 4.1 Semicolon Rules

Semicolons are **optional** statement terminators. In blocks:
- Expression **with** semicolon → statement (value discarded)
- Expression **without** semicolon as last in block → block's return value
- `let`, `const`, `return`, `use` → semicolon optional but recommended

---

## 5. Expressions (Precedence Order)

Dari terendah ke tertinggi (19 levels):

```ebnf
expr        = assignment ;

// Level 1: Assignment (Right-associative)
assignment  = pipeline [ ('=' | '+=' | '-=' | '*=' | '/=' | '%='
                         | '&=' | '|=' | '^=' | '<<=' | '>>=') pipeline ] ;

// Level 2: Pipeline (Left-associative)
pipeline    = logic_or { '|>' logic_or } ;

// Level 3: Logical OR (Left-associative)
logic_or    = logic_and { '||' logic_and } ;

// Level 4: Logical AND (Left-associative)
logic_and   = bit_or { '&&' bit_or } ;

// Level 5: Bitwise OR (Left-associative)
bit_or      = bit_xor { '|' bit_xor } ;

// Level 6: Bitwise XOR (Left-associative)
bit_xor     = bit_and { '^' bit_and } ;

// Level 7: Bitwise AND (Left-associative)
bit_and     = equality { '&' equality } ;

// Level 8: Equality (Left-associative)
equality    = comparison { ('==' | '!=') comparison } ;

// Level 9: Comparison (Left-associative)
comparison  = range { ('<' | '>' | '<=' | '>=') range } ;

// Level 10: Range (Non-associative)
range       = bit_shift [ '..' [ '=' ] bit_shift ] ;

// Level 11: Bit Shift (Left-associative)
bit_shift   = addition { ('<<' | '>>') addition } ;

// Level 12: Addition (Left-associative)
addition    = multiply { ('+' | '-') multiply } ;

// Level 13: Multiplication / Matrix Multiply (Left-associative)
multiply    = power { ('*' | '/' | '%' | '@') power } ;

// Level 14: Power (Right-associative)
power       = cast [ '**' cast ] ;

// Level 15: Type Cast (Left-associative)
cast        = unary [ 'as' type_expr ] ;

// Level 16: Unary (Right-associative, prefix)
unary       = ( '!' | '-' | '~' | '&' | '&mut' ) unary | try_expr ;

// Level 17: Try / Error Propagation (Postfix)
try_expr    = postfix [ '?' ] ;

// Level 18: Postfix (Left-associative)
postfix     = primary { call | index | field | method } ;

call        = '(' [ call_arg { ',' call_arg } ] ')' ;
call_arg    = [ IDENT ':' ] expr ;
index       = '[' expr ']' ;
field       = '.' IDENT ;
method      = '.' IDENT call ;

// Level 19: Primary (Atoms)
primary     = INT_LIT | FLOAT_LIT | STRING_LIT | BOOL_LIT | CHAR_LIT
            | IDENT
            | '(' expr ')'
            | array_lit | tuple_lit | tensor_lit
            | if_expr | match_expr | while_expr | for_expr | loop_expr
            | block_expr
            | closure_expr ;
```

### 5.1 Named Arguments

Function calls support both positional and named arguments:

```ebnf
call_arg    = [ IDENT ':' ] expr ;
```

Example: `add(a: 1, b: 2)` or `add(1, 2)`. Named and positional cannot be mixed.

---

## 6. Type Expressions

```ebnf
type_expr   = simple_type | generic_type | tensor_type | pointer_type
            | tuple_type | array_type | slice_type | fn_type
            | ref_type ;

simple_type = 'bool' | 'i8' | 'i16' | 'i32' | 'i64' | 'i128' | 'isize'
            | 'u8' | 'u16' | 'u32' | 'u64' | 'u128' | 'usize'
            | 'f32' | 'f64' | 'char' | 'str' | 'void' | 'never'
            | IDENT ;

generic_type = IDENT '<' type_expr { ',' type_expr } '>' ;
tensor_type  = ( 'tensor' | 'Tensor' ) '<' simple_type '>' '[' dim { ',' dim } ']' ;
dim          = INT_LIT | '*' ;
pointer_type = '*const' type_expr | '*mut' type_expr ;
ref_type     = '&' [ 'mut' ] type_expr ;
tuple_type   = '(' type_expr { ',' type_expr } ')' ;
array_type   = '[' type_expr ';' INT_LIT ']' ;
slice_type   = '[' type_expr ']' ;
fn_type      = 'fn' '(' type_list ')' '->' type_expr ;
```

---

## 7. Pattern Matching

```ebnf
match_expr  = 'match' expr '{' { match_arm } '}' ;
match_arm   = pattern [ 'if' expr ] '=>' ( expr | block_expr ) ','? ;
pattern     = literal_pat | ident_pat | tuple_pat | struct_pat
            | enum_pat | range_pat | wildcard_pat ;

literal_pat = INT_LIT | FLOAT_LIT | STRING_LIT | BOOL_LIT ;
ident_pat   = IDENT ;
tuple_pat   = '(' pattern { ',' pattern } ')' ;
struct_pat  = IDENT '{' field_pat { ',' field_pat } '}' ;
field_pat   = IDENT [ ':' pattern ] ;
enum_pat    = IDENT '::' IDENT [ '(' pattern { ',' pattern } ')' ] ;
range_pat   = INT_LIT '..' [ '=' ] INT_LIT ;
wildcard_pat = '_' ;
```

---

## 8. Block & Control Flow Expressions

```ebnf
block_expr  = '{' { stmt } [ expr ] '}' ;

if_expr     = 'if' expr block_expr [ 'else' ( if_expr | block_expr ) ] ;
while_expr  = 'while' expr block_expr ;
for_expr    = 'for' IDENT 'in' expr block_expr ;
loop_expr   = 'loop' block_expr ;

match_expr  = 'match' expr '{' { match_arm } '}' ;
match_arm   = pattern [ 'if' expr ] '=>' ( expr | block_expr ) ','? ;
```

---

## 9. Closure Expressions

```ebnf
closure_expr = '|' [ closure_param { ',' closure_param } ] '|'
               [ '->' type_expr ] ( expr | block_expr ) ;
closure_param = IDENT [ ':' type_expr ] ;
```

Examples:
```fajar
|x| x * 2
|x: i32, y: i32| -> i32 { x + y }
|| println("hello")
```

---

## 10. Gradient Control Block

```ebnf
no_grad_block = 'with' 'no_grad' block_expr ;
```

Example:
```fajar
with no_grad {
    let pred = model.forward(input)
}
```

> Note: `with` is a contextual keyword, only recognized before `no_grad`.

---

## 11. Operator Precedence Summary

| Level | Name | Operators | Assoc |
|-------|------|-----------|-------|
| 1 | Assignment | `= += -= *= /= %= &= \|= ^= <<= >>=` | Right |
| 2 | Pipeline | `\|>` | Left |
| 3 | Logical OR | `\|\|` | Left |
| 4 | Logical AND | `&&` | Left |
| 5 | Bitwise OR | `\|` | Left |
| 6 | Bitwise XOR | `^` | Left |
| 7 | Bitwise AND | `&` | Left |
| 8 | Equality | `== !=` | Left |
| 9 | Comparison | `< > <= >=` | Left |
| 10 | Range | `.. ..=` | None |
| 11 | Bit Shift | `<< >>` | Left |
| 12 | Addition | `+ -` | Left |
| 13 | Multiply | `* / % @` | Left |
| 14 | Power | `**` | Right |
| 15 | Type Cast | `as` | Left |
| 16 | Unary | `! - ~ & &mut` | Right |
| 17 | Try | `?` | Postfix |
| 18 | Postfix | `. () [] .method()` | Left |
| 19 | Primary | Literals, idents, groups | - |

---

*Grammar Version: 0.2 | Updated: 2026-03-05 | Aligned with FAJAR_LANG_SPEC.md v0.1 + Gap Analysis fixes*
