# Operator Precedence

Operators listed from lowest to highest precedence.

| Level | Name | Operators | Associativity |
|-------|------|-----------|---------------|
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
| 19 | Primary | Literals, identifiers | - |

## Examples

```fajar
// Precedence: * before +
2 + 3 * 4          // 14, not 20

// Right-associative power
2 ** 3 ** 2         // 512 (= 2^(3^2) = 2^9)

// Pipeline (lowest non-assignment)
5 |> double |> add_one

// Comparison returns bool
let ok = x > 0 && x < 100
```
