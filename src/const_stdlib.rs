//! Const standard library — functions evaluable at compile time in `comptime {}` and `const fn`.
//!
//! These extend the `ComptimeEvaluator` with standard library operations that
//! are safe and deterministic at compile time.
//!
//! # Categories
//!
//! - **Math**: abs, min, max, clamp, pow
//! - **String**: str_len, str_eq, str_contains, str_starts_with
//! - **Array**: array_len, array_get, array_push (returns new array)
//! - **Option**: unwrap_or, is_some, map (simulated via Null/value)
//! - **Result**: unwrap_or (simulated via value/Null)
//! - **Hash**: hash_str, hash_bytes (FNV-1a)
//! - **Formatting**: format_int, format_float
//! - **Bit manipulation**: count_ones, leading_zeros, trailing_zeros
//! - **Conversion**: i32_to_i64, f64_to_f32, int_to_float, float_to_int

use crate::analyzer::comptime::ComptimeValue;

// ═══════════════════════════════════════════════════════════════════════
// K7.1: Const Math Functions
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `abs`.
pub fn const_abs(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Int(v) => Some(ComptimeValue::Int(v.abs())),
        ComptimeValue::Float(v) => Some(ComptimeValue::Float(v.abs())),
        _ => None,
    }
}

/// Compile-time `min`.
pub fn const_min(a: &ComptimeValue, b: &ComptimeValue) -> Option<ComptimeValue> {
    match (a, b) {
        (ComptimeValue::Int(x), ComptimeValue::Int(y)) => Some(ComptimeValue::Int(*x.min(y))),
        (ComptimeValue::Float(x), ComptimeValue::Float(y)) => Some(ComptimeValue::Float(x.min(*y))),
        _ => None,
    }
}

/// Compile-time `max`.
pub fn const_max(a: &ComptimeValue, b: &ComptimeValue) -> Option<ComptimeValue> {
    match (a, b) {
        (ComptimeValue::Int(x), ComptimeValue::Int(y)) => Some(ComptimeValue::Int(*x.max(y))),
        (ComptimeValue::Float(x), ComptimeValue::Float(y)) => Some(ComptimeValue::Float(x.max(*y))),
        _ => None,
    }
}

/// Compile-time `clamp`.
pub fn const_clamp(
    val: &ComptimeValue,
    lo: &ComptimeValue,
    hi: &ComptimeValue,
) -> Option<ComptimeValue> {
    match (val, lo, hi) {
        (ComptimeValue::Int(v), ComptimeValue::Int(l), ComptimeValue::Int(h)) => {
            Some(ComptimeValue::Int(*v.max(l).min(h)))
        }
        (ComptimeValue::Float(v), ComptimeValue::Float(l), ComptimeValue::Float(h)) => {
            Some(ComptimeValue::Float(v.max(*l).min(*h)))
        }
        _ => None,
    }
}

/// Compile-time integer `pow`.
pub fn const_pow(base: &ComptimeValue, exp: &ComptimeValue) -> Option<ComptimeValue> {
    match (base, exp) {
        (ComptimeValue::Int(b), ComptimeValue::Int(e)) => {
            if *e < 0 {
                Some(ComptimeValue::Int(0))
            } else {
                Some(ComptimeValue::Int(b.wrapping_pow(*e as u32)))
            }
        }
        (ComptimeValue::Float(b), ComptimeValue::Float(e)) => {
            Some(ComptimeValue::Float(b.powf(*e)))
        }
        (ComptimeValue::Float(b), ComptimeValue::Int(e)) => {
            Some(ComptimeValue::Float(b.powi(*e as i32)))
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K7.2: Const String Operations
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `str_len`.
pub fn const_str_len(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Str(s) => Some(ComptimeValue::Int(s.len() as i64)),
        _ => None,
    }
}

/// Compile-time `str_eq`.
pub fn const_str_eq(a: &ComptimeValue, b: &ComptimeValue) -> Option<ComptimeValue> {
    match (a, b) {
        (ComptimeValue::Str(x), ComptimeValue::Str(y)) => Some(ComptimeValue::Bool(x == y)),
        _ => None,
    }
}

/// Compile-time `str_contains`.
pub fn const_str_contains(
    haystack: &ComptimeValue,
    needle: &ComptimeValue,
) -> Option<ComptimeValue> {
    match (haystack, needle) {
        (ComptimeValue::Str(h), ComptimeValue::Str(n)) => {
            Some(ComptimeValue::Bool(h.contains(n.as_str())))
        }
        _ => None,
    }
}

/// Compile-time `str_starts_with`.
pub fn const_str_starts_with(s: &ComptimeValue, prefix: &ComptimeValue) -> Option<ComptimeValue> {
    match (s, prefix) {
        (ComptimeValue::Str(h), ComptimeValue::Str(p)) => {
            Some(ComptimeValue::Bool(h.starts_with(p.as_str())))
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K7.3: Const Array Operations
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `array_len`.
pub fn const_array_len(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Array(arr) => Some(ComptimeValue::Int(arr.len() as i64)),
        _ => None,
    }
}

/// Compile-time `array_get` (safe indexing).
pub fn const_array_get(arr: &ComptimeValue, idx: &ComptimeValue) -> Option<ComptimeValue> {
    match (arr, idx) {
        (ComptimeValue::Array(a), ComptimeValue::Int(i)) => a.get(*i as usize).cloned(),
        _ => None,
    }
}

/// Compile-time `array_push` — returns a new array with the element appended.
pub fn const_array_push(arr: &ComptimeValue, val: &ComptimeValue) -> Option<ComptimeValue> {
    match arr {
        ComptimeValue::Array(a) => {
            let mut new = a.clone();
            new.push(val.clone());
            Some(ComptimeValue::Array(new))
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K7.4: Const Option Methods (Null = None, non-Null = Some)
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `unwrap_or` for Option.
pub fn const_unwrap_or(val: &ComptimeValue, default: &ComptimeValue) -> ComptimeValue {
    match val {
        ComptimeValue::Null => default.clone(),
        other => other.clone(),
    }
}

/// Compile-time `is_some`.
pub fn const_is_some(val: &ComptimeValue) -> ComptimeValue {
    ComptimeValue::Bool(!matches!(val, ComptimeValue::Null))
}

/// Compile-time `is_none`.
pub fn const_is_none(val: &ComptimeValue) -> ComptimeValue {
    ComptimeValue::Bool(matches!(val, ComptimeValue::Null))
}

// ═══════════════════════════════════════════════════════════════════════
// K7.5: Const Result Methods (value = Ok, Null = Err)
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `is_ok`.
pub fn const_is_ok(val: &ComptimeValue) -> ComptimeValue {
    const_is_some(val)
}

/// Compile-time `unwrap_or` for Result.
pub fn const_result_unwrap_or(val: &ComptimeValue, default: &ComptimeValue) -> ComptimeValue {
    const_unwrap_or(val, default)
}

// ═══════════════════════════════════════════════════════════════════════
// K7.6: Const Hash Functions (FNV-1a)
// ═══════════════════════════════════════════════════════════════════════

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001B3;

/// Compile-time FNV-1a hash for a string.
pub fn const_hash_str(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Str(s) => {
            let hash = fnv1a_hash(s.as_bytes());
            Some(ComptimeValue::Int(hash as i64))
        }
        _ => None,
    }
}

/// Compile-time FNV-1a hash for a byte array.
pub fn const_hash_bytes(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Array(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| {
                    if let ComptimeValue::Int(b) = v {
                        Some(*b as u8)
                    } else {
                        None
                    }
                })
                .collect();
            let hash = fnv1a_hash(&bytes);
            Some(ComptimeValue::Int(hash as i64))
        }
        _ => None,
    }
}

fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// K7.7: Const Formatting
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `format_int`.
pub fn const_format_int(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Int(v) => Some(ComptimeValue::Str(v.to_string())),
        _ => None,
    }
}

/// Compile-time `format_float`.
pub fn const_format_float(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Float(v) => Some(ComptimeValue::Str(v.to_string())),
        ComptimeValue::Int(v) => Some(ComptimeValue::Str((*v as f64).to_string())),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K7.8: Const Bit Manipulation
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `count_ones` (popcount).
pub fn const_count_ones(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Int(v) => Some(ComptimeValue::Int(v.count_ones() as i64)),
        _ => None,
    }
}

/// Compile-time `leading_zeros`.
pub fn const_leading_zeros(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Int(v) => Some(ComptimeValue::Int(v.leading_zeros() as i64)),
        _ => None,
    }
}

/// Compile-time `trailing_zeros`.
pub fn const_trailing_zeros(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Int(v) => Some(ComptimeValue::Int(v.trailing_zeros() as i64)),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K7.9: Const Conversion
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time `int_to_float`.
pub fn const_int_to_float(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Int(v) => Some(ComptimeValue::Float(*v as f64)),
        _ => None,
    }
}

/// Compile-time `float_to_int`.
pub fn const_float_to_int(val: &ComptimeValue) -> Option<ComptimeValue> {
    match val {
        ComptimeValue::Float(v) => Some(ComptimeValue::Int(*v as i64)),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dispatcher — resolve const stdlib call by name
// ═══════════════════════════════════════════════════════════════════════

/// Evaluate a const stdlib function by name.
///
/// Returns `None` if the name is not a known const stdlib function.
pub fn eval_const_stdlib(name: &str, args: &[ComptimeValue]) -> Option<ComptimeValue> {
    match name {
        // K7.1: Math
        "abs" => args.first().and_then(const_abs),
        "min" => const_min(args.first()?, args.get(1)?),
        "max" => const_max(args.first()?, args.get(1)?),
        "clamp" => const_clamp(args.first()?, args.get(1)?, args.get(2)?),
        "pow" => const_pow(args.first()?, args.get(1)?),

        // K7.2: String
        "str_len" => args.first().and_then(const_str_len),
        "str_eq" => const_str_eq(args.first()?, args.get(1)?),
        "str_contains" => const_str_contains(args.first()?, args.get(1)?),
        "str_starts_with" => const_str_starts_with(args.first()?, args.get(1)?),

        // K7.3: Array
        "array_len" | "len" => {
            let first = args.first()?;
            const_array_len(first).or_else(|| const_str_len(first))
        }
        "array_get" => const_array_get(args.first()?, args.get(1)?),
        "array_push" => const_array_push(args.first()?, args.get(1)?),

        // K7.4: Option
        "unwrap_or" => Some(const_unwrap_or(args.first()?, args.get(1)?)),
        "is_some" => Some(const_is_some(args.first()?)),
        "is_none" => Some(const_is_none(args.first()?)),

        // K7.5: Result
        "is_ok" => Some(const_is_ok(args.first()?)),

        // K7.6: Hash
        "hash_str" => args.first().and_then(const_hash_str),
        "hash_bytes" => args.first().and_then(const_hash_bytes),

        // K7.7: Formatting
        "format_int" | "to_string" => {
            let first = args.first()?;
            const_format_int(first).or_else(|| const_format_float(first))
        }
        "format_float" => args.first().and_then(const_format_float),

        // K7.8: Bit manipulation
        "count_ones" => args.first().and_then(const_count_ones),
        "leading_zeros" => args.first().and_then(const_leading_zeros),
        "trailing_zeros" => args.first().and_then(const_trailing_zeros),

        // K7.9: Conversion
        "int_to_float" => args.first().and_then(const_int_to_float),
        "float_to_int" => args.first().and_then(const_float_to_int),

        _ => None,
    }
}

/// Returns the list of all known const stdlib function names.
pub fn known_const_stdlib_functions() -> &'static [&'static str] {
    &[
        "abs",
        "min",
        "max",
        "clamp",
        "pow",
        "str_len",
        "str_eq",
        "str_contains",
        "str_starts_with",
        "array_len",
        "len",
        "array_get",
        "array_push",
        "unwrap_or",
        "is_some",
        "is_none",
        "is_ok",
        "hash_str",
        "hash_bytes",
        "format_int",
        "format_float",
        "to_string",
        "count_ones",
        "leading_zeros",
        "trailing_zeros",
        "int_to_float",
        "float_to_int",
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K7.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── K7.1: Const math ──

    #[test]
    fn k7_1_abs() {
        assert_eq!(
            const_abs(&ComptimeValue::Int(-5)),
            Some(ComptimeValue::Int(5))
        );
        assert_eq!(
            const_abs(&ComptimeValue::Int(5)),
            Some(ComptimeValue::Int(5))
        );
        assert_eq!(
            const_abs(&ComptimeValue::Float(-3.14)),
            Some(ComptimeValue::Float(3.14))
        );
    }

    #[test]
    fn k7_1_min_max() {
        let a = ComptimeValue::Int(3);
        let b = ComptimeValue::Int(7);
        assert_eq!(const_min(&a, &b), Some(ComptimeValue::Int(3)));
        assert_eq!(const_max(&a, &b), Some(ComptimeValue::Int(7)));
    }

    #[test]
    fn k7_1_clamp() {
        let val = ComptimeValue::Int(15);
        let lo = ComptimeValue::Int(0);
        let hi = ComptimeValue::Int(10);
        assert_eq!(const_clamp(&val, &lo, &hi), Some(ComptimeValue::Int(10)));

        let val2 = ComptimeValue::Int(-5);
        assert_eq!(const_clamp(&val2, &lo, &hi), Some(ComptimeValue::Int(0)));
    }

    #[test]
    fn k7_1_pow() {
        assert_eq!(
            const_pow(&ComptimeValue::Int(2), &ComptimeValue::Int(10)),
            Some(ComptimeValue::Int(1024))
        );
    }

    // ── K7.2: Const string ──

    #[test]
    fn k7_2_str_len() {
        assert_eq!(
            const_str_len(&ComptimeValue::Str("hello".into())),
            Some(ComptimeValue::Int(5))
        );
    }

    #[test]
    fn k7_2_str_eq() {
        let a = ComptimeValue::Str("abc".into());
        let b = ComptimeValue::Str("abc".into());
        let c = ComptimeValue::Str("xyz".into());
        assert_eq!(const_str_eq(&a, &b), Some(ComptimeValue::Bool(true)));
        assert_eq!(const_str_eq(&a, &c), Some(ComptimeValue::Bool(false)));
    }

    #[test]
    fn k7_2_str_contains() {
        let s = ComptimeValue::Str("hello world".into());
        assert_eq!(
            const_str_contains(&s, &ComptimeValue::Str("world".into())),
            Some(ComptimeValue::Bool(true))
        );
        assert_eq!(
            const_str_contains(&s, &ComptimeValue::Str("xyz".into())),
            Some(ComptimeValue::Bool(false))
        );
    }

    #[test]
    fn k7_2_str_starts_with() {
        let s = ComptimeValue::Str("fajar-lang".into());
        assert_eq!(
            const_str_starts_with(&s, &ComptimeValue::Str("fajar".into())),
            Some(ComptimeValue::Bool(true))
        );
    }

    // ── K7.3: Const array ──

    #[test]
    fn k7_3_array_len() {
        let arr = ComptimeValue::Array(vec![ComptimeValue::Int(1), ComptimeValue::Int(2)]);
        assert_eq!(const_array_len(&arr), Some(ComptimeValue::Int(2)));
    }

    #[test]
    fn k7_3_array_get() {
        let arr = ComptimeValue::Array(vec![
            ComptimeValue::Int(10),
            ComptimeValue::Int(20),
            ComptimeValue::Int(30),
        ]);
        assert_eq!(
            const_array_get(&arr, &ComptimeValue::Int(1)),
            Some(ComptimeValue::Int(20))
        );
        assert_eq!(const_array_get(&arr, &ComptimeValue::Int(5)), None);
    }

    #[test]
    fn k7_3_array_push() {
        let arr = ComptimeValue::Array(vec![ComptimeValue::Int(1)]);
        let result = const_array_push(&arr, &ComptimeValue::Int(2));
        assert_eq!(
            result,
            Some(ComptimeValue::Array(vec![
                ComptimeValue::Int(1),
                ComptimeValue::Int(2)
            ]))
        );
    }

    // ── K7.4: Const Option ──

    #[test]
    fn k7_4_unwrap_or() {
        assert_eq!(
            const_unwrap_or(&ComptimeValue::Int(42), &ComptimeValue::Int(0)),
            ComptimeValue::Int(42)
        );
        assert_eq!(
            const_unwrap_or(&ComptimeValue::Null, &ComptimeValue::Int(0)),
            ComptimeValue::Int(0)
        );
    }

    #[test]
    fn k7_4_is_some_is_none() {
        assert_eq!(
            const_is_some(&ComptimeValue::Int(42)),
            ComptimeValue::Bool(true)
        );
        assert_eq!(
            const_is_some(&ComptimeValue::Null),
            ComptimeValue::Bool(false)
        );
        assert_eq!(
            const_is_none(&ComptimeValue::Null),
            ComptimeValue::Bool(true)
        );
    }

    // ── K7.5: Const Result ──

    #[test]
    fn k7_5_is_ok() {
        assert_eq!(
            const_is_ok(&ComptimeValue::Int(1)),
            ComptimeValue::Bool(true)
        );
        assert_eq!(
            const_is_ok(&ComptimeValue::Null),
            ComptimeValue::Bool(false)
        );
    }

    // ── K7.6: Const hash ──

    #[test]
    fn k7_6_hash_str() {
        let h1 = const_hash_str(&ComptimeValue::Str("hello".into())).unwrap();
        let h2 = const_hash_str(&ComptimeValue::Str("hello".into())).unwrap();
        let h3 = const_hash_str(&ComptimeValue::Str("world".into())).unwrap();
        assert_eq!(h1, h2); // deterministic
        assert_ne!(h1, h3); // different strings → different hashes
    }

    #[test]
    fn k7_6_hash_bytes() {
        let arr = ComptimeValue::Array(vec![ComptimeValue::Int(0xDE), ComptimeValue::Int(0xAD)]);
        let h = const_hash_bytes(&arr);
        assert!(h.is_some());
    }

    // ── K7.7: Const formatting ──

    #[test]
    fn k7_7_format_int() {
        assert_eq!(
            const_format_int(&ComptimeValue::Int(42)),
            Some(ComptimeValue::Str("42".into()))
        );
    }

    #[test]
    fn k7_7_format_float() {
        let result = const_format_float(&ComptimeValue::Float(3.14)).unwrap();
        if let ComptimeValue::Str(s) = result {
            assert!(s.starts_with("3.14"));
        } else {
            panic!("expected string");
        }
    }

    // ── K7.8: Const bit manipulation ──

    #[test]
    fn k7_8_count_ones() {
        assert_eq!(
            const_count_ones(&ComptimeValue::Int(0b1010_1010)),
            Some(ComptimeValue::Int(4))
        );
        assert_eq!(
            const_count_ones(&ComptimeValue::Int(0)),
            Some(ComptimeValue::Int(0))
        );
        assert_eq!(
            const_count_ones(&ComptimeValue::Int(-1)),
            Some(ComptimeValue::Int(64))
        ); // all bits set
    }

    #[test]
    fn k7_8_leading_zeros() {
        assert_eq!(
            const_leading_zeros(&ComptimeValue::Int(1)),
            Some(ComptimeValue::Int(63))
        );
        assert_eq!(
            const_leading_zeros(&ComptimeValue::Int(0)),
            Some(ComptimeValue::Int(64))
        );
    }

    #[test]
    fn k7_8_trailing_zeros() {
        assert_eq!(
            const_trailing_zeros(&ComptimeValue::Int(8)),
            Some(ComptimeValue::Int(3))
        ); // 0b1000
        assert_eq!(
            const_trailing_zeros(&ComptimeValue::Int(1)),
            Some(ComptimeValue::Int(0))
        );
    }

    // ── K7.9: Const conversion ──

    #[test]
    fn k7_9_int_to_float() {
        assert_eq!(
            const_int_to_float(&ComptimeValue::Int(42)),
            Some(ComptimeValue::Float(42.0))
        );
    }

    #[test]
    fn k7_9_float_to_int() {
        assert_eq!(
            const_float_to_int(&ComptimeValue::Float(3.7)),
            Some(ComptimeValue::Int(3)) // truncates
        );
    }

    // ── K7.10: Dispatcher integration ──

    #[test]
    fn k7_10_dispatcher() {
        assert_eq!(
            eval_const_stdlib("abs", &[ComptimeValue::Int(-10)]),
            Some(ComptimeValue::Int(10))
        );
        assert_eq!(
            eval_const_stdlib("str_len", &[ComptimeValue::Str("hi".into())]),
            Some(ComptimeValue::Int(2))
        );
        assert_eq!(
            eval_const_stdlib("count_ones", &[ComptimeValue::Int(0xFF)]),
            Some(ComptimeValue::Int(8))
        );
        assert_eq!(
            eval_const_stdlib("int_to_float", &[ComptimeValue::Int(5)]),
            Some(ComptimeValue::Float(5.0))
        );
        assert_eq!(eval_const_stdlib("unknown_fn", &[]), None);
    }

    #[test]
    fn k7_10_known_functions_list() {
        let known = known_const_stdlib_functions();
        assert!(known.len() >= 25);
        assert!(known.contains(&"abs"));
        assert!(known.contains(&"hash_str"));
        assert!(known.contains(&"count_ones"));
        assert!(known.contains(&"float_to_int"));
    }
}
