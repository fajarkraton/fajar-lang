//! Dependent arrays — `Array<T, const N: usize>` with compile-time length
//! tracking, bounds check elimination, length propagation, and split/concat.

use std::collections::HashMap;
use std::fmt;

use super::nat::{check_nat_eq, NatError, NatValue};

// ═══════════════════════════════════════════════════════════════════════
// S2.1: Array<T, N> Type
// ═══════════════════════════════════════════════════════════════════════

/// A dependent array type: `Array<T, const N: usize>`.
///
/// The element type `element_ty` is a string representation, and the
/// length `len` is a Nat value that may be concrete or parametric.
#[derive(Debug, Clone, PartialEq)]
pub struct DepArray {
    /// Element type name (e.g., `"i32"`, `"f64"`, `"T"`).
    pub element_ty: String,
    /// Compile-time length.
    pub len: NatValue,
}

impl fmt::Display for DepArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Array<{}, {}>", self.element_ty, self.len)
    }
}

impl DepArray {
    /// Creates a new dependent array type with a concrete length.
    pub fn concrete(element_ty: &str, len: u64) -> Self {
        Self {
            element_ty: element_ty.into(),
            len: NatValue::Literal(len),
        }
    }

    /// Creates a new dependent array type with a parametric length.
    pub fn parametric(element_ty: &str, param: &str) -> Self {
        Self {
            element_ty: element_ty.into(),
            len: NatValue::Param(param.into()),
        }
    }

    /// Creates a dependent array with inferred length.
    pub fn inferred(element_ty: &str) -> Self {
        Self {
            element_ty: element_ty.into(),
            len: NatValue::Inferred,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.2: Array Literal Inference
// ═══════════════════════════════════════════════════════════════════════

/// Infers the Nat value for an array literal given its element count.
///
/// `let a: Array<i32, _> = [1, 2, 3]` → resolves `_` to `NatValue::Literal(3)`.
pub fn infer_array_literal_length(element_count: usize) -> NatValue {
    NatValue::Literal(element_count as u64)
}

/// Checks that an inferred length matches an expected dependent length.
pub fn check_literal_length(
    expected: &NatValue,
    literal_count: usize,
    env: &HashMap<String, u64>,
) -> Result<(), NatError> {
    let found = NatValue::Literal(literal_count as u64);
    check_nat_eq(expected, &found, env, "array literal length")
}

// ═══════════════════════════════════════════════════════════════════════
// S2.3: Bounds Check Elimination
// ═══════════════════════════════════════════════════════════════════════

/// Determines whether a bounds check can be elided for a given index.
///
/// When the index is a known constant strictly less than the array length,
/// the runtime bounds check is unnecessary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundsCheckResult {
    /// The access is provably safe — no runtime check needed.
    Elide,
    /// Cannot prove safety — keep runtime check.
    Keep,
    /// The access is provably out of bounds — emit compile error.
    OutOfBounds,
}

/// Checks whether an index is within bounds of a dependent array at compile time.
pub fn check_bounds(
    array_len: &NatValue,
    index: &NatValue,
    env: &HashMap<String, u64>,
) -> BoundsCheckResult {
    let len_val = array_len.evaluate(env);
    let idx_val = index.evaluate(env);

    match (len_val, idx_val) {
        (Some(len), Some(idx)) => {
            if idx < len {
                BoundsCheckResult::Elide
            } else {
                BoundsCheckResult::OutOfBounds
            }
        }
        _ => BoundsCheckResult::Keep,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.4: Length Propagation — concat
// ═══════════════════════════════════════════════════════════════════════

/// Computes the result type of concatenating two dependent arrays.
///
/// `concat(Array<T, A>, Array<T, B>) -> Array<T, A + B>`
pub fn concat_type(a: &DepArray, b: &DepArray) -> Result<DepArray, String> {
    if a.element_ty != b.element_ty {
        return Err(format!(
            "cannot concat Array<{}, _> with Array<{}, _>: element types differ",
            a.element_ty, b.element_ty
        ));
    }
    Ok(DepArray {
        element_ty: a.element_ty.clone(),
        len: NatValue::Add(Box::new(a.len.clone()), Box::new(b.len.clone())),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S2.5: Slice-to-Array Conversion
// ═══════════════════════════════════════════════════════════════════════

/// Result of a `slice.try_into_array::<N>()` attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceConversion {
    /// Compile-time guaranteed success (slice length is known and matches N).
    Guaranteed,
    /// Requires runtime length check.
    RuntimeCheck,
    /// Compile-time guaranteed failure (slice length is known and mismatches N).
    Impossible,
}

/// Checks whether a slice-to-array conversion is statically provable.
pub fn check_slice_conversion(
    slice_len: &NatValue,
    target_n: &NatValue,
    env: &HashMap<String, u64>,
) -> SliceConversion {
    let sl = slice_len.evaluate(env);
    let tn = target_n.evaluate(env);

    match (sl, tn) {
        (Some(s), Some(t)) if s == t => SliceConversion::Guaranteed,
        (Some(s), Some(t)) if s != t => SliceConversion::Impossible,
        _ => SliceConversion::RuntimeCheck,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.6: Fixed-Size Window
// ═══════════════════════════════════════════════════════════════════════

/// Validates that a window size `W` is valid for an array of length `N`.
///
/// Windows are valid when `W <= N` and `W > 0`.
pub fn validate_window(
    array_len: &NatValue,
    window_size: &NatValue,
    env: &HashMap<String, u64>,
) -> Result<(), String> {
    let n = array_len.evaluate(env);
    let w = window_size.evaluate(env);

    if let Some(w_val) = w {
        if w_val == 0 {
            return Err("window size must be greater than 0".into());
        }
        if let Some(n_val) = n {
            if w_val > n_val {
                return Err(format!("window size {w_val} exceeds array length {n_val}"));
            }
        }
    }
    Ok(())
}

/// Computes the number of windows of size W in an array of length N.
///
/// `windows_count(N, W) = N - W + 1`
pub fn windows_count(array_len: &NatValue, window_size: &NatValue) -> NatValue {
    // N - W + 1
    NatValue::Add(
        Box::new(NatValue::Sub(
            Box::new(array_len.clone()),
            Box::new(window_size.clone()),
        )),
        Box::new(NatValue::Literal(1)),
    )
}

// ═══════════════════════════════════════════════════════════════════════
// S2.7: Split at Index
// ═══════════════════════════════════════════════════════════════════════

/// Computes the result types of splitting an array at index K.
///
/// `array.split_at::<K>() -> (Array<T, K>, Array<T, N - K>)`
pub fn split_at_types(arr: &DepArray, k: &NatValue) -> (DepArray, DepArray) {
    let left = DepArray {
        element_ty: arr.element_ty.clone(),
        len: k.clone(),
    };
    let right = DepArray {
        element_ty: arr.element_ty.clone(),
        len: NatValue::Sub(Box::new(arr.len.clone()), Box::new(k.clone())),
    };
    (left, right)
}

/// Validates that a split index K is within bounds [0, N].
pub fn validate_split_index(
    array_len: &NatValue,
    split_at: &NatValue,
    env: &HashMap<String, u64>,
) -> Result<(), String> {
    let n = array_len.evaluate(env);
    let k = split_at.evaluate(env);

    if let (Some(n_val), Some(k_val)) = (n, k) {
        if k_val > n_val {
            return Err(format!("split index {k_val} exceeds array length {n_val}"));
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// S2.8: Type Error Messages
// ═══════════════════════════════════════════════════════════════════════

/// Formats a clear diagnostic for a length mismatch.
pub fn format_length_mismatch(
    expected: &DepArray,
    found: &DepArray,
    env: &HashMap<String, u64>,
) -> String {
    let exp_len = expected.len.evaluate(env);
    let found_len = found.len.evaluate(env);

    let mut msg = format!("expected {expected}, found {found}");
    if let (Some(e), Some(f)) = (exp_len, found_len) {
        msg.push_str(&format!(" (length {e} vs {f})"));
    }
    msg
}

// ═══════════════════════════════════════════════════════════════════════
// S2.9: Interop with Vec
// ═══════════════════════════════════════════════════════════════════════

/// Describes the conversion result of `Vec<T>.try_into_array::<N>()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VecConversion {
    /// Requires runtime length check (Vec length is unknown at compile time).
    Runtime,
    /// Vec length is statically known and matches target.
    StaticMatch,
    /// Vec length is statically known and does not match.
    StaticMismatch { vec_len: u64, target_n: u64 },
}

/// Checks whether a Vec-to-Array conversion is valid.
pub fn check_vec_to_array(
    vec_len: Option<u64>,
    target_n: &NatValue,
    env: &HashMap<String, u64>,
) -> VecConversion {
    let tn = target_n.evaluate(env);

    match (vec_len, tn) {
        (Some(vl), Some(t)) if vl == t => VecConversion::StaticMatch,
        (Some(vl), Some(t)) => VecConversion::StaticMismatch {
            vec_len: vl,
            target_n: t,
        },
        _ => VecConversion::Runtime,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S2.1 — Array<T, N>
    #[test]
    fn s2_1_dep_array_concrete() {
        let arr = DepArray::concrete("i32", 5);
        assert_eq!(arr.to_string(), "Array<i32, 5>");
        assert_eq!(arr.len.evaluate(&HashMap::new()), Some(5));
    }

    #[test]
    fn s2_1_dep_array_parametric() {
        let arr = DepArray::parametric("f64", "N");
        assert_eq!(arr.to_string(), "Array<f64, N>");
        let mut env = HashMap::new();
        env.insert("N".into(), 10);
        assert_eq!(arr.len.evaluate(&env), Some(10));
    }

    // S2.2 — Array Literal Inference
    #[test]
    fn s2_2_infer_literal_length() {
        let n = infer_array_literal_length(3);
        assert_eq!(n, NatValue::Literal(3));
    }

    #[test]
    fn s2_2_check_literal_length_match() {
        let expected = NatValue::Literal(3);
        assert!(check_literal_length(&expected, 3, &HashMap::new()).is_ok());
    }

    #[test]
    fn s2_2_check_literal_length_mismatch() {
        let expected = NatValue::Literal(3);
        assert!(check_literal_length(&expected, 4, &HashMap::new()).is_err());
    }

    // S2.3 — Bounds Check Elimination
    #[test]
    fn s2_3_bounds_elide() {
        let len = NatValue::Literal(10);
        let idx = NatValue::Literal(3);
        assert_eq!(
            check_bounds(&len, &idx, &HashMap::new()),
            BoundsCheckResult::Elide
        );
    }

    #[test]
    fn s2_3_bounds_out_of_bounds() {
        let len = NatValue::Literal(5);
        let idx = NatValue::Literal(5);
        assert_eq!(
            check_bounds(&len, &idx, &HashMap::new()),
            BoundsCheckResult::OutOfBounds
        );
    }

    #[test]
    fn s2_3_bounds_keep_unknown() {
        let len = NatValue::Param("N".into());
        let idx = NatValue::Literal(0);
        assert_eq!(
            check_bounds(&len, &idx, &HashMap::new()),
            BoundsCheckResult::Keep
        );
    }

    // S2.4 — Concat
    #[test]
    fn s2_4_concat_type() {
        let a = DepArray::concrete("i32", 3);
        let b = DepArray::concrete("i32", 4);
        let result = concat_type(&a, &b).unwrap();
        assert_eq!(result.element_ty, "i32");
        assert_eq!(result.len.evaluate(&HashMap::new()), Some(7));
    }

    #[test]
    fn s2_4_concat_type_mismatch() {
        let a = DepArray::concrete("i32", 3);
        let b = DepArray::concrete("f64", 4);
        assert!(concat_type(&a, &b).is_err());
    }

    // S2.5 — Slice Conversion
    #[test]
    fn s2_5_slice_conversion_guaranteed() {
        let sl = NatValue::Literal(5);
        let tn = NatValue::Literal(5);
        assert_eq!(
            check_slice_conversion(&sl, &tn, &HashMap::new()),
            SliceConversion::Guaranteed
        );
    }

    #[test]
    fn s2_5_slice_conversion_impossible() {
        let sl = NatValue::Literal(3);
        let tn = NatValue::Literal(5);
        assert_eq!(
            check_slice_conversion(&sl, &tn, &HashMap::new()),
            SliceConversion::Impossible
        );
    }

    // S2.6 — Fixed-Size Window
    #[test]
    fn s2_6_window_valid() {
        let n = NatValue::Literal(10);
        let w = NatValue::Literal(3);
        assert!(validate_window(&n, &w, &HashMap::new()).is_ok());
    }

    #[test]
    fn s2_6_window_too_large() {
        let n = NatValue::Literal(3);
        let w = NatValue::Literal(5);
        assert!(validate_window(&n, &w, &HashMap::new()).is_err());
    }

    #[test]
    fn s2_6_window_zero() {
        let n = NatValue::Literal(10);
        let w = NatValue::Literal(0);
        assert!(validate_window(&n, &w, &HashMap::new()).is_err());
    }

    #[test]
    fn s2_6_windows_count() {
        let n = NatValue::Literal(10);
        let w = NatValue::Literal(3);
        let count = windows_count(&n, &w);
        assert_eq!(count.evaluate(&HashMap::new()), Some(8)); // 10 - 3 + 1 = 8
    }

    // S2.7 — Split at Index
    #[test]
    fn s2_7_split_at() {
        let arr = DepArray::concrete("i32", 10);
        let k = NatValue::Literal(4);
        let (left, right) = split_at_types(&arr, &k);
        assert_eq!(left.len.evaluate(&HashMap::new()), Some(4));
        assert_eq!(right.len.evaluate(&HashMap::new()), Some(6));
    }

    #[test]
    fn s2_7_split_validation_ok() {
        let n = NatValue::Literal(10);
        let k = NatValue::Literal(5);
        assert!(validate_split_index(&n, &k, &HashMap::new()).is_ok());
    }

    #[test]
    fn s2_7_split_validation_fail() {
        let n = NatValue::Literal(5);
        let k = NatValue::Literal(7);
        assert!(validate_split_index(&n, &k, &HashMap::new()).is_err());
    }

    // S2.8 — Error Messages
    #[test]
    fn s2_8_length_mismatch_message() {
        let expected = DepArray::concrete("i32", 4);
        let found = DepArray::concrete("i32", 3);
        let msg = format_length_mismatch(&expected, &found, &HashMap::new());
        assert!(msg.contains("Array<i32, 4>"));
        assert!(msg.contains("Array<i32, 3>"));
        assert!(msg.contains("length 4 vs 3"));
    }

    // S2.9 — Vec Interop
    #[test]
    fn s2_9_vec_to_array_match() {
        let target = NatValue::Literal(5);
        assert_eq!(
            check_vec_to_array(Some(5), &target, &HashMap::new()),
            VecConversion::StaticMatch
        );
    }

    #[test]
    fn s2_9_vec_to_array_mismatch() {
        let target = NatValue::Literal(5);
        let result = check_vec_to_array(Some(3), &target, &HashMap::new());
        assert!(matches!(result, VecConversion::StaticMismatch { .. }));
    }

    #[test]
    fn s2_9_vec_to_array_runtime() {
        let target = NatValue::Literal(5);
        assert_eq!(
            check_vec_to_array(None, &target, &HashMap::new()),
            VecConversion::Runtime
        );
    }
}
