//! Tensor shape types — `Tensor<const ROWS, const COLS>` with compile-time
//! shape verification for matmul, transpose, reshape, flatten, broadcast,
//! and higher-rank tensors.

use std::collections::HashMap;
use std::fmt;

use super::nat::{NatError, NatValue, check_nat_eq};

// ═══════════════════════════════════════════════════════════════════════
// S3.1: Tensor<N, M> Type
// ═══════════════════════════════════════════════════════════════════════

/// A dependent tensor type carrying compile-time dimensions.
///
/// Dimensions are Nat values that may be concrete, parametric, or
/// inferred. This extends the existing runtime `Tensor` type with
/// static shape information.
#[derive(Debug, Clone, PartialEq)]
pub struct DepTensor {
    /// Element type name (e.g., `"f32"`, `"f64"`).
    pub element_ty: String,
    /// Shape dimensions (compile-time Nat values).
    pub dims: Vec<NatValue>,
}

impl fmt::Display for DepTensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dims_str: Vec<String> = self.dims.iter().map(|d| d.to_string()).collect();
        write!(f, "Tensor<{}, {}>", self.element_ty, dims_str.join(", "))
    }
}

impl DepTensor {
    /// Creates a 2D tensor with concrete dimensions.
    pub fn matrix(element_ty: &str, rows: u64, cols: u64) -> Self {
        Self {
            element_ty: element_ty.into(),
            dims: vec![NatValue::Literal(rows), NatValue::Literal(cols)],
        }
    }

    /// Creates a 2D tensor with parametric dimensions.
    pub fn parametric_2d(element_ty: &str, rows: &str, cols: &str) -> Self {
        Self {
            element_ty: element_ty.into(),
            dims: vec![NatValue::Param(rows.into()), NatValue::Param(cols.into())],
        }
    }

    /// Returns the rank (number of dimensions).
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Returns the total number of elements (product of all dimensions).
    pub fn total_elements(&self, env: &HashMap<String, u64>) -> Option<u64> {
        let mut product = 1u64;
        for d in &self.dims {
            let val = d.evaluate(env)?;
            product = product.checked_mul(val)?;
        }
        Some(product)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.2: Shape Inference from Construction
// ═══════════════════════════════════════════════════════════════════════

/// Infers a DepTensor type from a tensor constructor call.
///
/// `zeros(3, 4)` → `Tensor<f64, 3, 4>`
/// `ones(5, 5)` → `Tensor<f64, 5, 5>`
pub fn infer_from_constructor(fn_name: &str, args: &[u64], element_ty: &str) -> Option<DepTensor> {
    match fn_name {
        "zeros" | "ones" | "randn" | "eye" | "xavier" if args.len() == 2 => {
            Some(DepTensor::matrix(element_ty, args[0], args[1]))
        }
        "zeros" | "ones" | "randn" if args.len() == 1 => Some(DepTensor {
            element_ty: element_ty.into(),
            dims: vec![NatValue::Literal(args[0])],
        }),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.3: Matmul Shape Checking
// ═══════════════════════════════════════════════════════════════════════

/// Checks matmul compatibility: `Tensor<A, B> * Tensor<B, C> -> Tensor<A, C>`.
///
/// The inner dimensions must match.
pub fn check_matmul(
    left: &DepTensor,
    right: &DepTensor,
    env: &HashMap<String, u64>,
) -> Result<DepTensor, NatError> {
    if left.rank() != 2 || right.rank() != 2 {
        return Err(NatError::Mismatch {
            expected: NatValue::Literal(2),
            found: NatValue::Literal(left.rank() as u64),
            expected_val: Some(2),
            found_val: Some(left.rank() as u64),
            context: "matmul requires rank-2 tensors".into(),
        });
    }

    // Inner dimensions: left[1] must equal right[0]
    check_nat_eq(&left.dims[1], &right.dims[0], env, "matmul inner dimension")?;

    Ok(DepTensor {
        element_ty: left.element_ty.clone(),
        dims: vec![left.dims[0].clone(), right.dims[1].clone()],
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S3.4: Transpose Shape Flip
// ═══════════════════════════════════════════════════════════════════════

/// Computes the transpose type: `Tensor<A, B>.transpose() -> Tensor<B, A>`.
pub fn transpose_type(tensor: &DepTensor) -> Result<DepTensor, String> {
    if tensor.rank() != 2 {
        return Err(format!(
            "transpose requires rank-2 tensor, got rank {}",
            tensor.rank()
        ));
    }
    Ok(DepTensor {
        element_ty: tensor.element_ty.clone(),
        dims: vec![tensor.dims[1].clone(), tensor.dims[0].clone()],
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S3.5: Reshape Validation
// ═══════════════════════════════════════════════════════════════════════

/// Validates a reshape: total elements must be preserved (A*B == C*D).
pub fn check_reshape(
    source: &DepTensor,
    target_dims: &[NatValue],
    env: &HashMap<String, u64>,
) -> Result<DepTensor, NatError> {
    let source_total = product_nat(&source.dims);
    let target_total = product_nat(target_dims);

    check_nat_eq(&source_total, &target_total, env, "reshape element count")?;

    Ok(DepTensor {
        element_ty: source.element_ty.clone(),
        dims: target_dims.to_vec(),
    })
}

/// Computes the product of a list of Nat values.
fn product_nat(dims: &[NatValue]) -> NatValue {
    if dims.is_empty() {
        return NatValue::Literal(1);
    }
    let mut result = dims[0].clone();
    for d in &dims[1..] {
        result = NatValue::Mul(Box::new(result), Box::new(d.clone()));
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S3.6: Flatten Type
// ═══════════════════════════════════════════════════════════════════════

/// Computes the flatten type: `Tensor<A, B>.flatten() -> Tensor<1, A*B>`.
pub fn flatten_type(tensor: &DepTensor) -> DepTensor {
    let total = product_nat(&tensor.dims);
    DepTensor {
        element_ty: tensor.element_ty.clone(),
        dims: vec![NatValue::Literal(1), total],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.7: Broadcast Rules
// ═══════════════════════════════════════════════════════════════════════

/// Result of checking broadcast compatibility.
#[derive(Debug, Clone, PartialEq)]
pub enum BroadcastResult {
    /// Shapes are compatible — result shape is the broadcast.
    Compatible(Vec<NatValue>),
    /// Shapes are incompatible.
    Incompatible {
        /// Position of the mismatch.
        dim_index: usize,
        /// Left dim value.
        left: NatValue,
        /// Right dim value.
        right: NatValue,
    },
}

/// Checks broadcast compatibility of two tensors for element-wise ops.
///
/// Rules: dimensions are compatible if they are equal, or one of them is 1.
pub fn check_broadcast(
    left: &DepTensor,
    right: &DepTensor,
    env: &HashMap<String, u64>,
) -> BroadcastResult {
    let max_rank = left.rank().max(right.rank());
    let mut result_dims = Vec::new();

    for i in 0..max_rank {
        let l_idx = if i < left.rank() {
            left.rank() - 1 - (max_rank - 1 - i)
        } else {
            usize::MAX
        };
        let r_idx = if i < right.rank() {
            right.rank() - 1 - (max_rank - 1 - i)
        } else {
            usize::MAX
        };

        let l_dim = if l_idx < left.rank() {
            &left.dims[l_idx]
        } else {
            &NatValue::Literal(1)
        };
        let r_dim = if r_idx < right.rank() {
            &right.dims[r_idx]
        } else {
            &NatValue::Literal(1)
        };

        let lv = l_dim.evaluate(env);
        let rv = r_dim.evaluate(env);

        match (lv, rv) {
            (Some(l), Some(r)) if l == r => result_dims.push(NatValue::Literal(l)),
            (Some(1), Some(r)) => result_dims.push(NatValue::Literal(r)),
            (Some(l), Some(1)) => result_dims.push(NatValue::Literal(l)),
            (Some(_), Some(_)) => {
                return BroadcastResult::Incompatible {
                    dim_index: i,
                    left: l_dim.clone(),
                    right: r_dim.clone(),
                };
            }
            _ => {
                // One or both are unknown — defer, use a placeholder.
                result_dims.push(l_dim.clone());
            }
        }
    }

    BroadcastResult::Compatible(result_dims)
}

// ═══════════════════════════════════════════════════════════════════════
// S3.8: Higher-Rank Tensors
// ═══════════════════════════════════════════════════════════════════════

/// Maximum supported tensor rank.
pub const MAX_TENSOR_RANK: usize = 4;

/// Creates a higher-rank tensor (up to rank 4).
pub fn higher_rank_tensor(element_ty: &str, dims: Vec<NatValue>) -> Result<DepTensor, String> {
    if dims.len() > MAX_TENSOR_RANK {
        return Err(format!(
            "tensor rank {} exceeds maximum supported rank {MAX_TENSOR_RANK}",
            dims.len()
        ));
    }
    if dims.is_empty() {
        return Err("tensor must have at least 1 dimension".into());
    }
    Ok(DepTensor {
        element_ty: element_ty.into(),
        dims,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S3.9: Shape Error Diagnostics
// ═══════════════════════════════════════════════════════════════════════

/// Formats a rich shape mismatch diagnostic.
pub fn format_shape_error(
    operation: &str,
    left: &DepTensor,
    right: &DepTensor,
    env: &HashMap<String, u64>,
) -> String {
    let mut msg = format!("shape mismatch in {operation}:\n");
    msg.push_str(&format!("  left:  {left}\n"));
    msg.push_str(&format!("  right: {right}\n"));

    if let (Some(le), Some(re)) = (left.total_elements(env), right.total_elements(env)) {
        msg.push_str(&format!("  left elements:  {le}\n"));
        msg.push_str(&format!("  right elements: {re}\n"));
    }
    msg
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S3.1 — Tensor<N, M>
    #[test]
    fn s3_1_dep_tensor_matrix() {
        let t = DepTensor::matrix("f64", 3, 4);
        assert_eq!(t.rank(), 2);
        assert_eq!(t.total_elements(&HashMap::new()), Some(12));
        assert_eq!(t.to_string(), "Tensor<f64, 3, 4>");
    }

    #[test]
    fn s3_1_dep_tensor_parametric() {
        let t = DepTensor::parametric_2d("f32", "A", "B");
        assert_eq!(t.rank(), 2);
        let mut env = HashMap::new();
        env.insert("A".into(), 5);
        env.insert("B".into(), 3);
        assert_eq!(t.total_elements(&env), Some(15));
    }

    // S3.2 — Constructor Inference
    #[test]
    fn s3_2_infer_zeros() {
        let t = infer_from_constructor("zeros", &[3, 4], "f64").unwrap();
        assert_eq!(t.dims.len(), 2);
        assert_eq!(t.total_elements(&HashMap::new()), Some(12));
    }

    #[test]
    fn s3_2_infer_ones() {
        let t = infer_from_constructor("ones", &[5, 5], "f64").unwrap();
        assert_eq!(t.total_elements(&HashMap::new()), Some(25));
    }

    #[test]
    fn s3_2_infer_unknown_fn() {
        assert!(infer_from_constructor("custom_init", &[3, 4], "f64").is_none());
    }

    // S3.3 — Matmul
    #[test]
    fn s3_3_matmul_compatible() {
        let a = DepTensor::matrix("f64", 3, 4);
        let b = DepTensor::matrix("f64", 4, 5);
        let result = check_matmul(&a, &b, &HashMap::new()).unwrap();
        assert_eq!(result.dims[0].evaluate(&HashMap::new()), Some(3));
        assert_eq!(result.dims[1].evaluate(&HashMap::new()), Some(5));
    }

    #[test]
    fn s3_3_matmul_incompatible() {
        let a = DepTensor::matrix("f64", 3, 4);
        let b = DepTensor::matrix("f64", 5, 6);
        assert!(check_matmul(&a, &b, &HashMap::new()).is_err());
    }

    #[test]
    fn s3_3_matmul_parametric() {
        let a = DepTensor::parametric_2d("f64", "A", "B");
        let b = DepTensor::parametric_2d("f64", "B", "C");
        let mut env = HashMap::new();
        env.insert("A".into(), 3);
        env.insert("B".into(), 4);
        env.insert("C".into(), 5);
        let result = check_matmul(&a, &b, &env).unwrap();
        assert_eq!(result.dims[0].evaluate(&env), Some(3));
        assert_eq!(result.dims[1].evaluate(&env), Some(5));
    }

    // S3.4 — Transpose
    #[test]
    fn s3_4_transpose() {
        let t = DepTensor::matrix("f64", 3, 4);
        let tr = transpose_type(&t).unwrap();
        assert_eq!(tr.dims[0].evaluate(&HashMap::new()), Some(4));
        assert_eq!(tr.dims[1].evaluate(&HashMap::new()), Some(3));
    }

    #[test]
    fn s3_4_transpose_rank1_error() {
        let t = DepTensor {
            element_ty: "f64".into(),
            dims: vec![NatValue::Literal(5)],
        };
        assert!(transpose_type(&t).is_err());
    }

    // S3.5 — Reshape
    #[test]
    fn s3_5_reshape_valid() {
        let t = DepTensor::matrix("f64", 3, 4);
        let target = vec![NatValue::Literal(2), NatValue::Literal(6)];
        let result = check_reshape(&t, &target, &HashMap::new()).unwrap();
        assert_eq!(result.total_elements(&HashMap::new()), Some(12));
    }

    #[test]
    fn s3_5_reshape_invalid() {
        let t = DepTensor::matrix("f64", 3, 4);
        let target = vec![NatValue::Literal(2), NatValue::Literal(7)];
        assert!(check_reshape(&t, &target, &HashMap::new()).is_err());
    }

    // S3.6 — Flatten
    #[test]
    fn s3_6_flatten() {
        let t = DepTensor::matrix("f64", 3, 4);
        let flat = flatten_type(&t);
        assert_eq!(flat.rank(), 2);
        assert_eq!(flat.dims[0].evaluate(&HashMap::new()), Some(1));
        assert_eq!(flat.total_elements(&HashMap::new()), Some(12));
    }

    // S3.7 — Broadcast
    #[test]
    fn s3_7_broadcast_same_shape() {
        let a = DepTensor::matrix("f64", 3, 4);
        let b = DepTensor::matrix("f64", 3, 4);
        let result = check_broadcast(&a, &b, &HashMap::new());
        assert!(matches!(result, BroadcastResult::Compatible(_)));
    }

    #[test]
    fn s3_7_broadcast_with_one() {
        let a = DepTensor::matrix("f64", 3, 4);
        let b = DepTensor::matrix("f64", 1, 4);
        let result = check_broadcast(&a, &b, &HashMap::new());
        if let BroadcastResult::Compatible(dims) = result {
            assert_eq!(dims[0].evaluate(&HashMap::new()), Some(3));
            assert_eq!(dims[1].evaluate(&HashMap::new()), Some(4));
        } else {
            panic!("expected compatible broadcast");
        }
    }

    #[test]
    fn s3_7_broadcast_incompatible() {
        let a = DepTensor::matrix("f64", 3, 4);
        let b = DepTensor::matrix("f64", 2, 4);
        let result = check_broadcast(&a, &b, &HashMap::new());
        assert!(matches!(result, BroadcastResult::Incompatible { .. }));
    }

    // S3.8 — Higher-Rank
    #[test]
    fn s3_8_rank3_tensor() {
        let t = higher_rank_tensor(
            "f32",
            vec![
                NatValue::Literal(2),
                NatValue::Literal(3),
                NatValue::Literal(4),
            ],
        )
        .unwrap();
        assert_eq!(t.rank(), 3);
        assert_eq!(t.total_elements(&HashMap::new()), Some(24));
    }

    #[test]
    fn s3_8_rank4_tensor() {
        let t = higher_rank_tensor(
            "f32",
            vec![
                NatValue::Literal(2),
                NatValue::Literal(3),
                NatValue::Literal(4),
                NatValue::Literal(5),
            ],
        )
        .unwrap();
        assert_eq!(t.rank(), 4);
        assert_eq!(t.total_elements(&HashMap::new()), Some(120));
    }

    #[test]
    fn s3_8_rank5_rejected() {
        let dims = (0..5).map(|i| NatValue::Literal(i + 1)).collect();
        assert!(higher_rank_tensor("f32", dims).is_err());
    }

    #[test]
    fn s3_8_rank0_rejected() {
        assert!(higher_rank_tensor("f32", vec![]).is_err());
    }

    // S3.9 — Diagnostics
    #[test]
    fn s3_9_shape_error_format() {
        let a = DepTensor::matrix("f64", 3, 4);
        let b = DepTensor::matrix("f64", 5, 6);
        let msg = format_shape_error("matmul", &a, &b, &HashMap::new());
        assert!(msg.contains("shape mismatch in matmul"));
        assert!(msg.contains("Tensor<f64, 3, 4>"));
        assert!(msg.contains("Tensor<f64, 5, 6>"));
    }

    // S3.10 — Additional
    #[test]
    fn s3_10_reshape_preserves_element_count() {
        let t = DepTensor {
            element_ty: "f32".into(),
            dims: vec![
                NatValue::Literal(2),
                NatValue::Literal(3),
                NatValue::Literal(4),
            ],
        };
        let target = vec![NatValue::Literal(6), NatValue::Literal(4)];
        let result = check_reshape(&t, &target, &HashMap::new()).unwrap();
        assert_eq!(result.total_elements(&HashMap::new()), Some(24));
    }

    #[test]
    fn s3_10_matmul_parametric_deferred() {
        // When inner dims are both parametric but different names, no error
        // (deferred until monomorphization).
        let a = DepTensor::parametric_2d("f64", "A", "B");
        let b = DepTensor::parametric_2d("f64", "C", "D");
        // B and C are different params, but unresolved — deferred.
        let result = check_matmul(&a, &b, &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn s3_10_flatten_rank3() {
        let t = higher_rank_tensor(
            "f32",
            vec![
                NatValue::Literal(2),
                NatValue::Literal(3),
                NatValue::Literal(4),
            ],
        )
        .unwrap();
        let flat = flatten_type(&t);
        assert_eq!(flat.rank(), 2);
        assert_eq!(flat.total_elements(&HashMap::new()), Some(24));
    }
}
