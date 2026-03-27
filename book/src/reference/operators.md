# Operators

Fajar Lang operators organized by precedence from lowest (1) to highest (19). Lower precedence binds more loosely.

## Precedence Table

| Level | Category | Operators | Associativity |
|-------|----------|-----------|---------------|
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
| 19 | Primary | Literals, identifiers | -- |

## Arithmetic Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `+` | Addition | `3 + 4` | `7` |
| `-` | Subtraction | `10 - 3` | `7` |
| `*` | Multiplication | `6 * 7` | `42` |
| `/` | Division | `15 / 4` | `3` (integer) |
| `%` | Modulo | `17 % 5` | `2` |
| `**` | Power | `2 ** 10` | `1024` |
| `@` | Matrix Multiply | `a @ b` | matmul result |

```fajar
let x = 2 ** 3 + 1    // 9 (power before add)
let y = 10 % 3 * 2    // 2 (mod and mul same level, left-to-right)
```

## Comparison Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `==` | Equal | `3 == 3` | `true` |
| `!=` | Not Equal | `3 != 4` | `true` |
| `<` | Less Than | `1 < 2` | `true` |
| `>` | Greater Than | `5 > 3` | `true` |
| `<=` | Less or Equal | `3 <= 3` | `true` |
| `>=` | Greater or Equal | `4 >= 5` | `false` |

```fajar
let in_range = x >= 0 && x <= 100
```

## Logical Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `&&` | Logical AND | `true && false` | `false` |
| `\|\|` | Logical OR | `true \|\| false` | `true` |
| `!` | Logical NOT | `!true` | `false` |

Short-circuit evaluation: `&&` stops on first `false`, `||` stops on first `true`.

```fajar
let valid = x > 0 && x < 100
let fallback = primary() || secondary()
```

## Bitwise Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `&` | Bitwise AND | `0xFF & 0x0F` | `0x0F` |
| `\|` | Bitwise OR | `0xF0 \| 0x0F` | `0xFF` |
| `^` | Bitwise XOR | `0xFF ^ 0x0F` | `0xF0` |
| `~` | Bitwise NOT | `~0xFF` | platform-dependent |
| `<<` | Left Shift | `1 << 8` | `256` |
| `>>` | Right Shift | `256 >> 4` | `16` |

```fajar
let flags = READ | WRITE        // combine flags
let masked = value & 0xFF       // extract low byte
let shifted = 1 << bit_pos      // set single bit
```

## Assignment Operators

| Operator | Equivalent | Example |
|----------|------------|---------|
| `=` | Direct assign | `x = 5` |
| `+=` | `x = x + y` | `count += 1` |
| `-=` | `x = x - y` | `balance -= cost` |
| `*=` | `x = x * y` | `total *= factor` |
| `/=` | `x = x / y` | `avg /= count` |
| `%=` | `x = x % y` | `angle %= 360` |
| `&=` | `x = x & y` | `flags &= mask` |
| `\|=` | `x = x \| y` | `flags \|= bit` |
| `^=` | `x = x ^ y` | `bits ^= toggle` |
| `<<=` | `x = x << y` | `val <<= 2` |
| `>>=` | `x = x >> y` | `val >>= 1` |

All compound assignments require `mut`:

```fajar
let mut x = 10
x += 5   // x is now 15
```

## Pipeline Operator

The pipeline operator `|>` passes the left-hand value as the first argument to the right-hand function.

```fajar
let result = 5 |> double |> add_one |> to_string
// equivalent to: to_string(add_one(double(5)))
```

## Range Operators

| Operator | Name | Example | Description |
|----------|------|---------|-------------|
| `..` | Exclusive range | `0..5` | 0, 1, 2, 3, 4 |
| `..=` | Inclusive range | `0..=5` | 0, 1, 2, 3, 4, 5 |

```fajar
for i in 0..10 {
    println(i)  // 0 through 9
}
```

## Try Operator

The `?` operator propagates errors from `Result` values.

```fajar
fn load() -> Result<str, str> {
    let data = read_file("config.toml")?   // return Err if failed
    Ok(data)
}
```

## Type Cast

The `as` keyword performs explicit type conversion.

```fajar
let x: i32 = 42
let y: f64 = x as f64      // 42.0
let z: u8 = 200 as u8      // 200
```
