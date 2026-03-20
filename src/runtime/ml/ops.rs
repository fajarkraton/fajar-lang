//! Tensor operations — element-wise arithmetic, matmul, transpose, reshape.
//!
//! All operations return new `TensorValue`s (immutable semantics).
//! Broadcasting follows NumPy-style rules for element-wise ops.

use ndarray::ArrayD;

use super::autograd::Tape;
use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Element-wise operations
// ═══════════════════════════════════════════════════════════════════════

/// Checks if two shapes are broadcast-compatible.
///
/// Returns `Ok(())` if broadcastable, `Err` otherwise.
fn check_broadcast(a: &[usize], b: &[usize]) -> Result<(), TensorError> {
    let max_ndim = a.len().max(b.len());
    for i in 0..max_ndim {
        let da = if i < a.len() { a[a.len() - 1 - i] } else { 1 };
        let db = if i < b.len() { b[b.len() - 1 - i] } else { 1 };
        if da != db && da != 1 && db != 1 {
            return Err(TensorError::ShapeMismatch {
                expected: a.to_vec(),
                got: b.to_vec(),
            });
        }
    }
    Ok(())
}

/// Element-wise addition with broadcasting.
pub fn add(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(a.shape(), b.shape())?;
    let result = a.data() + b.data();
    Ok(TensorValue::new(
        result,
        a.requires_grad() || b.requires_grad(),
    ))
}

/// Element-wise subtraction with broadcasting.
pub fn sub(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(a.shape(), b.shape())?;
    let result = a.data() - b.data();
    Ok(TensorValue::new(
        result,
        a.requires_grad() || b.requires_grad(),
    ))
}

/// Element-wise multiplication with broadcasting.
pub fn mul(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(a.shape(), b.shape())?;
    let result = a.data() * b.data();
    Ok(TensorValue::new(
        result,
        a.requires_grad() || b.requires_grad(),
    ))
}

/// Element-wise division with broadcasting.
pub fn div(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError> {
    if b.data().iter().any(|&v| v == 0.0) {
        return Err(TensorError::DivisionByZero);
    }
    check_broadcast(a.shape(), b.shape())?;
    let result = a.data() / b.data();
    Ok(TensorValue::new(
        result,
        a.requires_grad() || b.requires_grad(),
    ))
}

/// Element-wise negation.
pub fn neg(a: &TensorValue) -> TensorValue {
    let result = -a.data().clone();
    TensorValue::new(result, a.requires_grad())
}

// ═══════════════════════════════════════════════════════════════════════
// Matrix operations
// ═══════════════════════════════════════════════════════════════════════

/// Matrix multiplication (2D tensors only).
///
/// Computes `a @ b` where `a` has shape `[m, k]` and `b` has shape `[k, n]`.
/// Returns a tensor with shape `[m, n]`.
pub fn matmul(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError> {
    if a.ndim() != 2 || b.ndim() != 2 {
        return Err(TensorError::RankMismatch {
            expected: 2,
            got: if a.ndim() != 2 { a.ndim() } else { b.ndim() },
        });
    }

    let a_shape = a.shape();
    let b_shape = b.shape();
    let k_a = a_shape[1];
    let k_b = b_shape[0];

    if k_a != k_b {
        return Err(TensorError::MatmulShapeMismatch {
            left: a_shape.to_vec(),
            right: b_shape.to_vec(),
            left_inner: k_a,
            right_inner: k_b,
        });
    }

    // Convert to 2D arrays for dot product (use standard layout for non-contiguous inputs)
    let a2 = a
        .data()
        .as_standard_layout()
        .into_owned()
        .into_shape_with_order((a_shape[0], a_shape[1]))
        .map_err(|e| TensorError::InvalidData {
            reason: e.to_string(),
        })?;
    let b2 = b
        .data()
        .as_standard_layout()
        .into_owned()
        .into_shape_with_order((b_shape[0], b_shape[1]))
        .map_err(|e| TensorError::InvalidData {
            reason: e.to_string(),
        })?;

    let result = a2.dot(&b2);
    let result_dyn = result.into_dyn();

    Ok(TensorValue::new(
        result_dyn,
        a.requires_grad() || b.requires_grad(),
    ))
}

/// Transposes the last two dimensions of a tensor.
///
/// For 2D tensors: swaps rows and columns.
pub fn transpose(a: &TensorValue) -> Result<TensorValue, TensorError> {
    if a.ndim() < 2 {
        return Err(TensorError::RankMismatch {
            expected: 2,
            got: a.ndim(),
        });
    }
    // Use as_standard_layout() to ensure contiguous memory after transpose
    let transposed = a.data().t().as_standard_layout().into_owned();
    Ok(TensorValue::new(transposed.into_dyn(), a.requires_grad()))
}

/// Flattens a tensor to 1D.
pub fn flatten(a: &TensorValue) -> TensorValue {
    let data = a.to_vec();
    let n = data.len();
    TensorValue::new(
        ArrayD::from_shape_vec(vec![n], data).expect("flatten: shape always valid"),
        a.requires_grad(),
    )
}

/// Reshapes a tensor to the given shape (total elements must match).
pub fn reshape(a: &TensorValue, new_shape: &[usize]) -> Result<TensorValue, TensorError> {
    let expected_numel: usize = new_shape.iter().product();
    if expected_numel != a.numel() {
        return Err(TensorError::ShapeMismatch {
            expected: new_shape.to_vec(),
            got: a.shape().to_vec(),
        });
    }
    // Ensure contiguous layout before reshape
    let data = a.data().as_standard_layout().into_owned();
    let reshaped = data
        .into_shape_with_order(ndarray::IxDyn(new_shape))
        .map_err(|e| TensorError::InvalidData {
            reason: e.to_string(),
        })?;
    Ok(TensorValue::new(reshaped, a.requires_grad()))
}

/// Splits a tensor along the given axis into chunks of `split_size`.
///
/// Returns a vector of tensors. The last chunk may be smaller.
pub fn split(
    a: &TensorValue,
    axis: usize,
    split_size: usize,
) -> Result<Vec<TensorValue>, TensorError> {
    let shape = a.shape();
    if axis >= shape.len() {
        return Err(TensorError::RankMismatch {
            expected: axis + 1,
            got: shape.len(),
        });
    }
    let dim = shape[axis];
    let data = a.data();
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < dim {
        let end = (offset + split_size).min(dim);
        let slice = data.slice_axis(ndarray::Axis(axis), ndarray::Slice::from(offset..end));
        let chunk = slice.to_owned();
        result.push(TensorValue::new(chunk, a.requires_grad()));
        offset = end;
    }
    Ok(result)
}

/// Concatenates tensors along the given axis.
pub fn concat(tensors: &[TensorValue], axis: usize) -> Result<TensorValue, TensorError> {
    if tensors.is_empty() {
        return Err(TensorError::InvalidData {
            reason: "cannot concat empty list".to_string(),
        });
    }
    let views: Vec<_> = tensors.iter().map(|t| t.data().view()).collect();
    let concatenated = ndarray::concatenate(ndarray::Axis(axis), &views).map_err(|e| {
        TensorError::InvalidData {
            reason: e.to_string(),
        }
    })?;
    let grad = tensors.iter().any(|t| t.requires_grad());
    Ok(TensorValue::new(concatenated, grad))
}

/// Computes the sum of all elements, returning a scalar tensor.
pub fn sum(a: &TensorValue) -> TensorValue {
    let total: f64 = a.data().iter().sum();
    TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![total]).expect("sum: scalar shape"),
        a.requires_grad(),
    )
}

/// Computes the mean of all elements, returning a scalar tensor.
pub fn mean(a: &TensorValue) -> TensorValue {
    let n = a.numel() as f64;
    let total: f64 = a.data().iter().sum();
    TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![total / n]).expect("mean: scalar shape"),
        a.requires_grad(),
    )
}

/// Removes dimensions of size 1 at the given axis.
pub fn squeeze(a: &TensorValue, axis: usize) -> Result<TensorValue, TensorError> {
    let shape = a.shape();
    if axis >= shape.len() || shape[axis] != 1 {
        return Err(TensorError::InvalidData {
            reason: format!(
                "squeeze: axis {axis} has size {} (must be 1)",
                if axis < shape.len() { shape[axis] } else { 0 }
            ),
        });
    }
    let new_shape: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != axis)
        .map(|(_, &d)| d)
        .collect();
    let data = a.to_vec();
    TensorValue::from_data(data, &new_shape)
}

/// Inserts a dimension of size 1 at the given axis.
pub fn unsqueeze(a: &TensorValue, axis: usize) -> Result<TensorValue, TensorError> {
    let shape = a.shape();
    if axis > shape.len() {
        return Err(TensorError::InvalidData {
            reason: format!(
                "unsqueeze: axis {axis} out of range for ndim {}",
                shape.len()
            ),
        });
    }
    let mut new_shape: Vec<usize> = shape.to_vec();
    new_shape.insert(axis, 1);
    let data = a.to_vec();
    TensorValue::from_data(data, &new_shape)
}

/// Returns the maximum element as a scalar tensor.
pub fn max(a: &TensorValue) -> TensorValue {
    let val = a.data().iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![val]).expect("max: scalar"),
        a.requires_grad(),
    )
}

/// Returns the minimum element as a scalar tensor.
pub fn min(a: &TensorValue) -> TensorValue {
    let val = a.data().iter().cloned().fold(f64::INFINITY, f64::min);
    TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![val]).expect("min: scalar"),
        a.requires_grad(),
    )
}

/// Returns the index of the maximum element as a scalar tensor.
pub fn argmax(a: &TensorValue) -> TensorValue {
    let (idx, _) =
        a.data()
            .iter()
            .enumerate()
            .fold((0, f64::NEG_INFINITY), |(best_i, best_v), (i, &v)| {
                if v > best_v { (i, v) } else { (best_i, best_v) }
            });
    TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![idx as f64]).expect("argmax: scalar"),
        false,
    )
}

/// Creates a 1D tensor with values from `start` to `end` (exclusive), step `step`.
pub fn arange(start: f64, end: f64, step: f64) -> Result<TensorValue, TensorError> {
    if step == 0.0 {
        return Err(TensorError::InvalidData {
            reason: "arange: step cannot be zero".into(),
        });
    }
    let mut data = Vec::new();
    let mut v = start;
    if step > 0.0 {
        while v < end {
            data.push(v);
            v += step;
        }
    } else {
        while v > end {
            data.push(v);
            v += step;
        }
    }
    let n = data.len();
    Ok(TensorValue::new(
        ArrayD::from_shape_vec(vec![n], data).expect("arange: shape"),
        false,
    ))
}

/// Creates a 1D tensor with `steps` evenly spaced values from `start` to `end` (inclusive).
pub fn linspace(start: f64, end: f64, steps: usize) -> Result<TensorValue, TensorError> {
    if steps == 0 {
        return Err(TensorError::InvalidData {
            reason: "linspace: steps must be > 0".into(),
        });
    }
    let data: Vec<f64> = if steps == 1 {
        vec![start]
    } else {
        let step = (end - start) / (steps - 1) as f64;
        (0..steps).map(|i| start + step * i as f64).collect()
    };
    Ok(TensorValue::new(
        ArrayD::from_shape_vec(vec![steps], data).expect("linspace: shape"),
        false,
    ))
}

/// Xavier (Glorot) uniform initialization for a matrix.
///
/// Values drawn from uniform(-limit, limit) where limit = sqrt(6 / (rows + cols)).
pub fn xavier(rows: usize, cols: usize) -> TensorValue {
    use ndarray_rand::RandomExt;
    use ndarray_rand::rand_distr::Uniform;
    let limit = (6.0 / (rows + cols) as f64).sqrt();
    let arr = ArrayD::random(vec![rows, cols], Uniform::new(-limit, limit));
    let mut t = TensorValue::new(arr, false);
    t.set_requires_grad(true);
    t
}

/// L1 loss: mean(|pred - target|).
///
/// Returns a scalar tensor.
pub fn l1_loss(pred: &TensorValue, target: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(pred.shape(), target.shape())?;
    let diff = pred.data() - target.data();
    let abs_diff = diff.mapv(f64::abs);
    let n = abs_diff.len() as f64;
    let total: f64 = abs_diff.iter().sum();
    Ok(TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![total / n]).expect("l1: scalar"),
        pred.requires_grad() || target.requires_grad(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Activation functions
// ═══════════════════════════════════════════════════════════════════════

/// ReLU activation: max(0, x) element-wise.
pub fn relu(a: &TensorValue) -> TensorValue {
    let result = a.data().mapv(|x| x.max(0.0));
    TensorValue::new(result, a.requires_grad())
}

/// Sigmoid activation: 1 / (1 + exp(-x)) element-wise.
pub fn sigmoid(a: &TensorValue) -> TensorValue {
    let result = a.data().mapv(|x| 1.0 / (1.0 + (-x).exp()));
    TensorValue::new(result, a.requires_grad())
}

/// Tanh activation: element-wise hyperbolic tangent.
pub fn tanh_act(a: &TensorValue) -> TensorValue {
    let result = a.data().mapv(f64::tanh);
    TensorValue::new(result, a.requires_grad())
}

/// Softmax activation: exp(x) / sum(exp(x)) with log-sum-exp trick for numerical stability.
///
/// Operates over the entire tensor (flattened). For multi-dimensional softmax
/// along a specific axis, use a reshape + softmax + reshape pattern.
pub fn softmax(a: &TensorValue) -> TensorValue {
    // Log-sum-exp trick: subtract max for numerical stability
    let max_val = a.data().iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exp_shifted = a.data().mapv(|x| (x - max_val).exp());
    let sum_exp: f64 = exp_shifted.iter().sum();
    let result = exp_shifted / sum_exp;
    TensorValue::new(result, a.requires_grad())
}

/// GELU activation: x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3))).
///
/// Gaussian Error Linear Unit — used in transformer architectures.
pub fn gelu(a: &TensorValue) -> TensorValue {
    let sqrt_2_over_pi = (2.0_f64 / std::f64::consts::PI).sqrt();
    let result = a.data().mapv(|x| {
        let inner = sqrt_2_over_pi * (x + 0.044715 * x.powi(3));
        x * 0.5 * (1.0 + inner.tanh())
    });
    TensorValue::new(result, a.requires_grad())
}

/// Leaky ReLU activation: max(alpha * x, x) element-wise.
///
/// Default alpha = 0.01.
pub fn leaky_relu(a: &TensorValue, alpha: f64) -> TensorValue {
    let result = a.data().mapv(|x| if x >= 0.0 { x } else { alpha * x });
    TensorValue::new(result, a.requires_grad())
}

// ═══════════════════════════════════════════════════════════════════════
// Tracked operations (with autograd tape recording)
// ═══════════════════════════════════════════════════════════════════════

/// Tracked addition: records gradient function on tape.
///
/// Grad: d(a+b)/da = 1, d(a+b)/db = 1
pub fn add_tracked(
    a: &TensorValue,
    b: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = add(a, b)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let b_id = b.id().unwrap_or_else(|| tape.fresh_id());
        let a_shape = a.shape().to_vec();
        let b_shape = b.shape().to_vec();
        tape.record(
            out_id,
            vec![a_id, b_id],
            Box::new(move |g| vec![reduce_broadcast(g, &a_shape), reduce_broadcast(g, &b_shape)]),
        );
    }
    Ok(result)
}

/// Tracked subtraction: records gradient function on tape.
///
/// Grad: d(a-b)/da = 1, d(a-b)/db = -1
pub fn sub_tracked(
    a: &TensorValue,
    b: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = sub(a, b)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let b_id = b.id().unwrap_or_else(|| tape.fresh_id());
        let a_shape = a.shape().to_vec();
        let b_shape = b.shape().to_vec();
        tape.record(
            out_id,
            vec![a_id, b_id],
            Box::new(move |g| {
                let grad_b = -reduce_broadcast(g, &b_shape);
                vec![reduce_broadcast(g, &a_shape), grad_b]
            }),
        );
    }
    Ok(result)
}

/// Tracked multiplication: records gradient function on tape.
///
/// Grad: d(a*b)/da = b, d(a*b)/db = a
pub fn mul_tracked(
    a: &TensorValue,
    b: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = mul(a, b)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let b_id = b.id().unwrap_or_else(|| tape.fresh_id());
        let a_data = a.data().clone();
        let b_data = b.data().clone();
        let a_shape = a.shape().to_vec();
        let b_shape = b.shape().to_vec();
        tape.record(
            out_id,
            vec![a_id, b_id],
            Box::new(move |g| {
                vec![
                    reduce_broadcast(&(g * &b_data), &a_shape),
                    reduce_broadcast(&(g * &a_data), &b_shape),
                ]
            }),
        );
    }
    Ok(result)
}

/// Tracked division: records gradient function on tape.
///
/// Grad: d(a/b)/da = 1/b, d(a/b)/db = -a/b^2
pub fn div_tracked(
    a: &TensorValue,
    b: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = div(a, b)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let b_id = b.id().unwrap_or_else(|| tape.fresh_id());
        let a_data = a.data().clone();
        let b_data = b.data().clone();
        let a_shape = a.shape().to_vec();
        let b_shape = b.shape().to_vec();
        tape.record(
            out_id,
            vec![a_id, b_id],
            Box::new(move |g| {
                let grad_a = g / &b_data;
                let grad_b = -(g * &a_data) / (&b_data * &b_data);
                vec![
                    reduce_broadcast(&grad_a, &a_shape),
                    reduce_broadcast(&grad_b, &b_shape),
                ]
            }),
        );
    }
    Ok(result)
}

/// Tracked matmul: records gradient function on tape.
///
/// Grad: d(A@B)/dA = grad @ B^T, d(A@B)/dB = A^T @ grad
pub fn matmul_tracked(
    a: &TensorValue,
    b: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = matmul(a, b)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let b_id = b.id().unwrap_or_else(|| tape.fresh_id());
        let a_data = a.data().clone();
        let b_data = b.data().clone();
        let a_shape = a.shape().to_vec();
        let b_shape = b.shape().to_vec();
        tape.record(
            out_id,
            vec![a_id, b_id],
            Box::new(move |g| {
                // grad_a = g @ b^T
                // Shapes are guaranteed valid from matmul operands.
                match (
                    g.clone().into_shape_with_order((a_shape[0], b_shape[1])),
                    b_data
                        .clone()
                        .into_shape_with_order((b_shape[0], b_shape[1])),
                    a_data
                        .clone()
                        .into_shape_with_order((a_shape[0], a_shape[1])),
                ) {
                    (Ok(g2), Ok(b2), Ok(a2)) => {
                        let grad_a = g2.dot(&b2.t()).into_dyn();
                        let grad_b = a2.t().dot(&g2).into_dyn();
                        vec![grad_a, grad_b]
                    }
                    _ => {
                        // Fallback: zero gradients if reshape fails (should never happen)
                        vec![
                            ArrayD::zeros(ndarray::IxDyn(&a_shape)),
                            ArrayD::zeros(ndarray::IxDyn(&b_shape)),
                        ]
                    }
                }
            }),
        );
    }
    Ok(result)
}

/// Tracked ReLU: records gradient function on tape.
///
/// Grad: d(relu(x))/dx = 1 if x > 0, else 0
pub fn relu_tracked(a: &TensorValue, tape: &mut Tape) -> TensorValue {
    let mut result = relu(a);
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let a_data = a.data().clone();
        tape.record(
            out_id,
            vec![a_id],
            Box::new(move |g| {
                let mask = a_data.mapv(|x| if x > 0.0 { 1.0 } else { 0.0 });
                vec![g * &mask]
            }),
        );
    }
    result
}

/// Tracked sigmoid: records gradient function on tape.
///
/// Grad: d(sigmoid(x))/dx = sigmoid(x) * (1 - sigmoid(x))
pub fn sigmoid_tracked(a: &TensorValue, tape: &mut Tape) -> TensorValue {
    let result = sigmoid(a);
    let mut out = result.clone();
    if out.requires_grad() {
        let out_id = tape.fresh_id();
        out.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let sig_data = result.data().clone();
        tape.record(
            out_id,
            vec![a_id],
            Box::new(move |g| {
                let dsig = &sig_data * &(1.0 - &sig_data);
                vec![g * &dsig]
            }),
        );
    }
    out
}

/// Tracked tanh: records gradient function on tape.
///
/// Grad: d(tanh(x))/dx = 1 - tanh(x)^2
pub fn tanh_tracked(a: &TensorValue, tape: &mut Tape) -> TensorValue {
    let result = tanh_act(a);
    let mut out = result.clone();
    if out.requires_grad() {
        let out_id = tape.fresh_id();
        out.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let tanh_data = result.data().clone();
        tape.record(
            out_id,
            vec![a_id],
            Box::new(move |g| {
                let dtanh = 1.0 - &tanh_data * &tanh_data;
                vec![g * &dtanh]
            }),
        );
    }
    out
}

/// Tracked sum: records gradient function on tape.
///
/// Grad: d(sum(x))/dx_i = 1 for all i
pub fn sum_tracked(a: &TensorValue, tape: &mut Tape) -> TensorValue {
    let mut result = sum(a);
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let a_shape = a.shape().to_vec();
        tape.record(
            out_id,
            vec![a_id],
            Box::new(move |g| {
                let scalar = g.iter().next().copied().unwrap_or(1.0);
                vec![ArrayD::from_elem(a_shape.clone(), scalar)]
            }),
        );
    }
    result
}

/// Tracked mean: records gradient function on tape.
///
/// Grad: d(mean(x))/dx_i = 1/n for all i
pub fn mean_tracked(a: &TensorValue, tape: &mut Tape) -> TensorValue {
    let mut result = mean(a);
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let a_id = a.id().unwrap_or_else(|| tape.fresh_id());
        let a_shape = a.shape().to_vec();
        let n = a.numel() as f64;
        tape.record(
            out_id,
            vec![a_id],
            Box::new(move |g| {
                let scalar = g.iter().next().copied().unwrap_or(1.0) / n;
                vec![ArrayD::from_elem(a_shape.clone(), scalar)]
            }),
        );
    }
    result
}

/// Reduces a gradient to match a target shape by summing along broadcast dimensions.
///
/// When broadcasting expanded dimensions from shape `target` to match a larger output,
/// the gradient must be summed along those dimensions to get back to the original shape.
fn reduce_broadcast(grad: &ArrayD<f64>, target_shape: &[usize]) -> ArrayD<f64> {
    if grad.shape() == target_shape {
        return grad.clone();
    }

    // If target is scalar, sum everything
    if target_shape.is_empty() {
        let total: f64 = grad.iter().sum();
        // Scalar shape with one element — construction is infallible here.
        return ArrayD::from_shape_vec(vec![], vec![total])
            .unwrap_or_else(|_| ArrayD::from_elem(ndarray::IxDyn(&[]), total));
    }

    // Sum along axes that were broadcast (size 1 in target but larger in grad)
    let grad_shape = grad.shape();
    let ndim = grad_shape.len();
    let target_ndim = target_shape.len();

    // Pad target shape with leading 1s to match grad dimensionality
    let mut padded_target = vec![1usize; ndim.saturating_sub(target_ndim)];
    padded_target.extend_from_slice(target_shape);

    let mut result = grad.clone();

    // Sum along axes where target had size 1 (or was missing → padded to 1)
    // Process from highest axis to lowest to keep axis indices valid
    for axis in (0..ndim).rev() {
        if padded_target[axis] == 1 && result.shape()[axis] > 1 {
            result = result
                .sum_axis(ndarray::Axis(axis))
                .insert_axis(ndarray::Axis(axis));
        }
    }

    // Remove leading dimensions that were added by padding
    if result.shape().len() > target_shape.len() {
        let data: Vec<f64> = result.iter().copied().collect();
        result =
            ArrayD::from_shape_vec(target_shape.to_vec(), data).unwrap_or_else(|_| result.clone());
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════
// Numerical gradient checking utility
// ═══════════════════════════════════════════════════════════════════════

/// Computes numerical gradient for a scalar function using central differences.
///
/// `f` takes a flat `Vec<f64>` and returns a scalar. `x` is the input.
/// Returns the numerical gradient with the same length as `x`.
pub fn numerical_gradient<F>(f: F, x: &[f64], epsilon: f64) -> Vec<f64>
where
    F: Fn(&[f64]) -> f64,
{
    let mut grad = vec![0.0; x.len()];
    let mut x_plus = x.to_vec();
    let mut x_minus = x.to_vec();

    for i in 0..x.len() {
        x_plus[i] = x[i] + epsilon;
        x_minus[i] = x[i] - epsilon;
        grad[i] = (f(&x_plus) - f(&x_minus)) / (2.0 * epsilon);
        x_plus[i] = x[i];
        x_minus[i] = x[i];
    }

    grad
}

// ═══════════════════════════════════════════════════════════════════════
// Loss functions
// ═══════════════════════════════════════════════════════════════════════

/// Mean Squared Error loss: mean((pred - target)^2).
///
/// Returns a scalar tensor.
pub fn mse_loss(pred: &TensorValue, target: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(pred.shape(), target.shape())?;
    let diff = pred.data() - target.data();
    let sq = &diff * &diff;
    let n = sq.len() as f64;
    let total: f64 = sq.iter().sum();
    Ok(TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![total / n]).expect("mse: scalar"),
        pred.requires_grad() || target.requires_grad(),
    ))
}

/// Cross-entropy loss: -sum(target * log(pred)).
///
/// `pred` should be probabilities (output of softmax). A small epsilon is added
/// to avoid log(0).
pub fn cross_entropy(pred: &TensorValue, target: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(pred.shape(), target.shape())?;
    let eps = 1e-12;
    let log_pred = pred.data().mapv(|x| (x.max(eps)).ln());
    let prod = target.data() * &log_pred;
    let total: f64 = -prod.iter().sum::<f64>();
    Ok(TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![total]).expect("ce: scalar"),
        pred.requires_grad() || target.requires_grad(),
    ))
}

/// Binary cross-entropy loss: -mean(target * log(pred) + (1-target) * log(1-pred)).
///
/// `pred` should be in (0, 1). Clamped with epsilon for numerical stability.
pub fn bce_loss(pred: &TensorValue, target: &TensorValue) -> Result<TensorValue, TensorError> {
    check_broadcast(pred.shape(), target.shape())?;
    let eps = 1e-12;
    let n = pred.numel() as f64;
    let total: f64 = pred
        .data()
        .iter()
        .zip(target.data().iter())
        .map(|(&p, &t)| {
            let p_clamp = p.clamp(eps, 1.0 - eps);
            -(t * p_clamp.ln() + (1.0 - t) * (1.0 - p_clamp).ln())
        })
        .sum();
    Ok(TensorValue::new(
        ArrayD::from_shape_vec(vec![], vec![total / n]).expect("bce: scalar"),
        pred.requires_grad() || target.requires_grad(),
    ))
}

/// Tracked MSE loss with autograd recording.
///
/// Grad: d(mse)/d(pred) = 2*(pred - target)/n
pub fn mse_loss_tracked(
    pred: &TensorValue,
    target: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = mse_loss(pred, target)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let pred_id = pred.id().unwrap_or_else(|| tape.fresh_id());
        let pred_data = pred.data().clone();
        let target_data = target.data().clone();
        let n = pred.numel() as f64;
        tape.record(
            out_id,
            vec![pred_id],
            Box::new(move |g| {
                let scalar = g.iter().next().copied().unwrap_or(1.0);
                let grad = (&pred_data - &target_data).mapv(|v| 2.0 * v * scalar / n);
                vec![grad]
            }),
        );
    }
    Ok(result)
}

/// Tracked cross-entropy loss with autograd recording.
///
/// Grad: d(ce)/d(pred) = -target/pred
pub fn cross_entropy_tracked(
    pred: &TensorValue,
    target: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = cross_entropy(pred, target)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let pred_id = pred.id().unwrap_or_else(|| tape.fresh_id());
        let pred_data = pred.data().clone();
        let target_data = target.data().clone();
        tape.record(
            out_id,
            vec![pred_id],
            Box::new(move |g| {
                let scalar = g.iter().next().copied().unwrap_or(1.0);
                let eps = 1e-12;
                let grad: Vec<f64> = target_data
                    .iter()
                    .zip(pred_data.iter())
                    .map(|(&t, &p)| -t / p.max(eps) * scalar)
                    .collect();
                let shape = pred_data.shape().to_vec();
                let grad_arr = ArrayD::from_shape_vec(shape.clone(), grad)
                    .unwrap_or_else(|_| ArrayD::zeros(ndarray::IxDyn(&shape)));
                vec![grad_arr]
            }),
        );
    }
    Ok(result)
}

/// Tracked BCE loss with autograd recording.
///
/// Grad: d(bce)/d(pred) = (-target/pred + (1-target)/(1-pred)) / n
pub fn bce_loss_tracked(
    pred: &TensorValue,
    target: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    let mut result = bce_loss(pred, target)?;
    if result.requires_grad() {
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let pred_id = pred.id().unwrap_or_else(|| tape.fresh_id());
        let pred_data = pred.data().clone();
        let target_data = target.data().clone();
        let n = pred.numel() as f64;
        tape.record(
            out_id,
            vec![pred_id],
            Box::new(move |g| {
                let scalar = g.iter().next().copied().unwrap_or(1.0);
                let eps = 1e-12;
                let grad: Vec<f64> = pred_data
                    .iter()
                    .zip(target_data.iter())
                    .map(|(&p, &t)| {
                        let p_clamp = p.clamp(eps, 1.0 - eps);
                        (-t / p_clamp + (1.0 - t) / (1.0 - p_clamp)) * scalar / n
                    })
                    .collect();
                let shape = pred_data.shape().to_vec();
                let grad_arr = ArrayD::from_shape_vec(shape.clone(), grad)
                    .unwrap_or_else(|_| ArrayD::zeros(ndarray::IxDyn(&shape)));
                vec![grad_arr]
            }),
        );
    }
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Element-wise ──

    #[test]
    fn add_same_shape() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let b = TensorValue::from_data(vec![4.0, 5.0, 6.0], &[3]).unwrap();
        let c = add(&a, &b).unwrap();
        assert_eq!(c.to_vec(), vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn sub_same_shape() {
        let a = TensorValue::from_data(vec![5.0, 10.0], &[2]).unwrap();
        let b = TensorValue::from_data(vec![3.0, 4.0], &[2]).unwrap();
        let c = sub(&a, &b).unwrap();
        assert_eq!(c.to_vec(), vec![2.0, 6.0]);
    }

    #[test]
    fn mul_same_shape() {
        let a = TensorValue::from_data(vec![2.0, 3.0], &[2]).unwrap();
        let b = TensorValue::from_data(vec![4.0, 5.0], &[2]).unwrap();
        let c = mul(&a, &b).unwrap();
        assert_eq!(c.to_vec(), vec![8.0, 15.0]);
    }

    #[test]
    fn div_same_shape() {
        let a = TensorValue::from_data(vec![10.0, 20.0], &[2]).unwrap();
        let b = TensorValue::from_data(vec![2.0, 5.0], &[2]).unwrap();
        let c = div(&a, &b).unwrap();
        assert_eq!(c.to_vec(), vec![5.0, 4.0]);
    }

    #[test]
    fn div_by_zero_error() {
        let a = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        let b = TensorValue::from_data(vec![0.0], &[1]).unwrap();
        assert!(matches!(div(&a, &b), Err(TensorError::DivisionByZero)));
    }

    #[test]
    fn neg_tensor() {
        let a = TensorValue::from_data(vec![1.0, -2.0, 3.0], &[3]).unwrap();
        let b = neg(&a);
        assert_eq!(b.to_vec(), vec![-1.0, 2.0, -3.0]);
    }

    #[test]
    fn add_shape_mismatch() {
        let a = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let b = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        assert!(matches!(
            add(&a, &b),
            Err(TensorError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn broadcasting_scalar_to_vector() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let b = TensorValue::from_data(vec![10.0], &[1]).unwrap();
        let c = add(&a, &b).unwrap();
        assert_eq!(c.to_vec(), vec![11.0, 12.0, 13.0]);
    }

    #[test]
    fn broadcasting_2d() {
        // [2,3] + [1,3] should broadcast
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let b = TensorValue::from_data(vec![10.0, 20.0, 30.0], &[1, 3]).unwrap();
        let c = add(&a, &b).unwrap();
        assert_eq!(c.to_vec(), vec![11.0, 22.0, 33.0, 14.0, 25.0, 36.0]);
    }

    #[test]
    fn requires_grad_propagation() {
        let mut a = TensorValue::zeros(&[2]);
        a.set_requires_grad(true);
        let b = TensorValue::zeros(&[2]);
        let c = add(&a, &b).unwrap();
        assert!(c.requires_grad());
    }

    // ── Matmul ──

    #[test]
    fn matmul_2x3_times_3x2() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let b = TensorValue::from_data(vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0], &[3, 2]).unwrap();
        let c = matmul(&a, &b).unwrap();
        assert_eq!(c.shape(), &[2, 2]);
        // [1*7+2*9+3*11, 1*8+2*10+3*12, 4*7+5*9+6*11, 4*8+5*10+6*12]
        assert_eq!(c.to_vec(), vec![58.0, 64.0, 139.0, 154.0]);
    }

    #[test]
    fn matmul_shape_mismatch() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let b = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[3, 2]).unwrap();
        assert!(matches!(
            matmul(&a, &b),
            Err(TensorError::MatmulShapeMismatch { .. })
        ));
    }

    #[test]
    fn matmul_rank_mismatch() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let b = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[3, 2]).unwrap();
        assert!(matches!(
            matmul(&a, &b),
            Err(TensorError::RankMismatch { .. })
        ));
    }

    #[test]
    fn matmul_identity() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let eye = TensorValue::eye(2);
        let c = matmul(&a, &eye).unwrap();
        assert_eq!(c.to_vec(), vec![1.0, 2.0, 3.0, 4.0]);
    }

    // ── Transpose ──

    #[test]
    fn transpose_2x3() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let b = transpose(&a).unwrap();
        assert_eq!(b.shape(), &[3, 2]);
        assert_eq!(b.to_vec(), vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn transpose_rank1_error() {
        let a = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        assert!(matches!(
            transpose(&a),
            Err(TensorError::RankMismatch { .. })
        ));
    }

    // ── Flatten ──

    #[test]
    fn flatten_2d() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let b = flatten(&a);
        assert_eq!(b.shape(), &[4]);
        assert_eq!(b.to_vec(), vec![1.0, 2.0, 3.0, 4.0]);
    }

    // ── Sum / Mean ──

    #[test]
    fn sum_tensor() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let s = sum(&a);
        assert_eq!(s.to_scalar().unwrap(), 10.0);
    }

    #[test]
    fn mean_tensor() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        let m = mean(&a);
        assert_eq!(m.to_scalar().unwrap(), 2.5);
    }

    // ── Activation Functions ──

    #[test]
    fn relu_positive_and_negative() {
        let a = TensorValue::from_data(vec![-2.0, -1.0, 0.0, 1.0, 2.0], &[5]).unwrap();
        let b = relu(&a);
        assert_eq!(b.to_vec(), vec![0.0, 0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn relu_preserves_shape() {
        let a = TensorValue::from_data(vec![-1.0, 2.0, -3.0, 4.0], &[2, 2]).unwrap();
        let b = relu(&a);
        assert_eq!(b.shape(), &[2, 2]);
        assert_eq!(b.to_vec(), vec![0.0, 2.0, 0.0, 4.0]);
    }

    #[test]
    fn sigmoid_known_values() {
        let a = TensorValue::from_data(vec![0.0], &[1]).unwrap();
        let b = sigmoid(&a);
        assert!((b.to_scalar().unwrap() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn sigmoid_range() {
        // sigmoid should always be in [0, 1]
        let a = TensorValue::from_data(vec![-100.0, -1.0, 0.0, 1.0, 100.0], &[5]).unwrap();
        let b = sigmoid(&a);
        for &v in b.to_vec().iter() {
            assert!((0.0..=1.0).contains(&v), "sigmoid out of range: {v}");
        }
        // Moderate inputs strictly in (0, 1)
        let c = TensorValue::from_data(vec![-5.0, 0.0, 5.0], &[3]).unwrap();
        let d = sigmoid(&c);
        for &v in d.to_vec().iter() {
            assert!(v > 0.0 && v < 1.0, "sigmoid moderate out of range: {v}");
        }
    }

    #[test]
    fn sigmoid_symmetry() {
        // sigmoid(-x) = 1 - sigmoid(x)
        let a = TensorValue::from_data(vec![2.0], &[1]).unwrap();
        let neg_a = TensorValue::from_data(vec![-2.0], &[1]).unwrap();
        let s_pos = sigmoid(&a).to_scalar().unwrap();
        let s_neg = sigmoid(&neg_a).to_scalar().unwrap();
        assert!((s_pos + s_neg - 1.0).abs() < 1e-10);
    }

    #[test]
    fn tanh_known_values() {
        let a = TensorValue::from_data(vec![0.0], &[1]).unwrap();
        let b = tanh_act(&a);
        assert!((b.to_scalar().unwrap()).abs() < 1e-10);
    }

    #[test]
    fn tanh_range() {
        // tanh should always be in (-1, 1)
        let a = TensorValue::from_data(vec![-100.0, -1.0, 0.0, 1.0, 100.0], &[5]).unwrap();
        let b = tanh_act(&a);
        for &v in b.to_vec().iter() {
            assert!(v >= -1.0 && v <= 1.0, "tanh out of range: {v}");
        }
    }

    #[test]
    fn softmax_sums_to_one() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let b = softmax(&a);
        let total: f64 = b.to_vec().iter().sum();
        assert!((total - 1.0).abs() < 1e-10);
    }

    #[test]
    fn softmax_all_positive() {
        let a = TensorValue::from_data(vec![-10.0, 0.0, 10.0], &[3]).unwrap();
        let b = softmax(&a);
        for &v in b.to_vec().iter() {
            assert!(v > 0.0, "softmax should be positive: {v}");
        }
    }

    #[test]
    fn softmax_numerical_stability() {
        // Large values should not cause overflow thanks to log-sum-exp trick
        let a = TensorValue::from_data(vec![1000.0, 1001.0, 1002.0], &[3]).unwrap();
        let b = softmax(&a);
        let total: f64 = b.to_vec().iter().sum();
        assert!((total - 1.0).abs() < 1e-10);
        // No NaN or Inf
        for &v in b.to_vec().iter() {
            assert!(v.is_finite(), "softmax produced non-finite: {v}");
        }
    }

    #[test]
    fn softmax_uniform_input() {
        // Equal inputs → equal outputs (1/n each)
        let a = TensorValue::from_data(vec![5.0, 5.0, 5.0, 5.0], &[4]).unwrap();
        let b = softmax(&a);
        for &v in b.to_vec().iter() {
            assert!((v - 0.25).abs() < 1e-10);
        }
    }

    #[test]
    fn gelu_known_values() {
        // GELU(0) = 0
        let a = TensorValue::from_data(vec![0.0], &[1]).unwrap();
        let b = gelu(&a);
        assert!((b.to_scalar().unwrap()).abs() < 1e-10);
    }

    #[test]
    fn gelu_positive_for_positive() {
        // GELU(x) > 0 for x > 0
        let a = TensorValue::from_data(vec![0.5, 1.0, 2.0, 5.0], &[4]).unwrap();
        let b = gelu(&a);
        for &v in b.to_vec().iter() {
            assert!(v > 0.0, "gelu should be positive for positive input: {v}");
        }
    }

    #[test]
    fn gelu_approx_relu_for_large() {
        // For large positive x, GELU(x) ≈ x
        let a = TensorValue::from_data(vec![10.0], &[1]).unwrap();
        let b = gelu(&a);
        assert!((b.to_scalar().unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn leaky_relu_positive_passthrough() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let b = leaky_relu(&a, 0.01);
        assert_eq!(b.to_vec(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn leaky_relu_negative_scaled() {
        let a = TensorValue::from_data(vec![-10.0, -1.0, 0.0, 1.0], &[4]).unwrap();
        let b = leaky_relu(&a, 0.1);
        assert_eq!(b.to_vec(), vec![-1.0, -0.1, 0.0, 1.0]);
    }

    #[test]
    fn leaky_relu_default_alpha() {
        let a = TensorValue::from_data(vec![-100.0], &[1]).unwrap();
        let b = leaky_relu(&a, 0.01);
        assert_eq!(b.to_scalar().unwrap(), -1.0);
    }

    #[test]
    fn activation_requires_grad_propagation() {
        let mut a = TensorValue::zeros(&[3]);
        a.set_requires_grad(true);
        assert!(relu(&a).requires_grad());
        assert!(sigmoid(&a).requires_grad());
        assert!(tanh_act(&a).requires_grad());
        assert!(softmax(&a).requires_grad());
        assert!(gelu(&a).requires_grad());
        assert!(leaky_relu(&a, 0.01).requires_grad());
    }

    // ── Tracked Operations & Gradient Checking ──

    fn make_grad_tensor(data: Vec<f64>, shape: &[usize]) -> TensorValue {
        let mut t = TensorValue::from_data(data, shape).unwrap();
        t.set_requires_grad(true);
        t
    }

    /// Checks analytical gradient (from tape backward) against numerical gradient.
    fn check_gradient(analytical: &[f64], numerical: &[f64], tol: f64, op_name: &str) {
        assert_eq!(
            analytical.len(),
            numerical.len(),
            "{op_name}: gradient length mismatch"
        );
        for (i, (a, n)) in analytical.iter().zip(numerical.iter()).enumerate() {
            assert!(
                (a - n).abs() < tol,
                "{op_name}: gradient mismatch at index {i}: analytical={a}, numerical={n}"
            );
        }
    }

    #[test]
    fn grad_add() {
        let mut tape = Tape::new();
        let mut a = make_grad_tensor(vec![1.0, 2.0, 3.0], &[3]);
        let mut b = make_grad_tensor(vec![4.0, 5.0, 6.0], &[3]);
        a.set_id(tape.fresh_id());
        b.set_id(tape.fresh_id());
        let c = add_tracked(&a, &b, &mut tape).unwrap();

        // sum to get scalar for backward
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let grad_b: Vec<f64> = grads[&b.id().unwrap()].iter().copied().collect();

        // d(sum(a+b))/da = [1, 1, 1], d(sum(a+b))/db = [1, 1, 1]
        assert_eq!(grad_a, vec![1.0, 1.0, 1.0]);
        assert_eq!(grad_b, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn grad_sub() {
        let mut tape = Tape::new();
        let mut a = make_grad_tensor(vec![5.0, 10.0], &[2]);
        let mut b = make_grad_tensor(vec![3.0, 4.0], &[2]);
        a.set_id(tape.fresh_id());
        b.set_id(tape.fresh_id());
        let c = sub_tracked(&a, &b, &mut tape).unwrap();
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let grad_b: Vec<f64> = grads[&b.id().unwrap()].iter().copied().collect();
        assert_eq!(grad_a, vec![1.0, 1.0]);
        assert_eq!(grad_b, vec![-1.0, -1.0]);
    }

    #[test]
    fn grad_mul() {
        let mut tape = Tape::new();
        let mut a = make_grad_tensor(vec![2.0, 3.0], &[2]);
        let mut b = make_grad_tensor(vec![4.0, 5.0], &[2]);
        a.set_id(tape.fresh_id());
        b.set_id(tape.fresh_id());
        let c = mul_tracked(&a, &b, &mut tape).unwrap();
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let grad_b: Vec<f64> = grads[&b.id().unwrap()].iter().copied().collect();
        // d(sum(a*b))/da = b, d(sum(a*b))/db = a
        assert_eq!(grad_a, vec![4.0, 5.0]);
        assert_eq!(grad_b, vec![2.0, 3.0]);
    }

    #[test]
    fn grad_div() {
        let mut tape = Tape::new();
        let mut a = make_grad_tensor(vec![6.0, 10.0], &[2]);
        let mut b = make_grad_tensor(vec![2.0, 5.0], &[2]);
        a.set_id(tape.fresh_id());
        b.set_id(tape.fresh_id());
        let c = div_tracked(&a, &b, &mut tape).unwrap();
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let grad_b: Vec<f64> = grads[&b.id().unwrap()].iter().copied().collect();
        // d(a/b)/da = 1/b, d(a/b)/db = -a/b^2
        assert_eq!(grad_a, vec![0.5, 0.2]);
        check_gradient(&grad_b, &[-1.5, -0.4], 1e-10, "div_b");
    }

    #[test]
    fn grad_matmul() {
        let mut tape = Tape::new();
        // A: [2, 3], B: [3, 2]
        let mut a = make_grad_tensor(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let mut b = make_grad_tensor(vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0], &[3, 2]);
        a.set_id(tape.fresh_id());
        b.set_id(tape.fresh_id());
        let c = matmul_tracked(&a, &b, &mut tape).unwrap();
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let grad_b: Vec<f64> = grads[&b.id().unwrap()].iter().copied().collect();

        // Numerical check
        let a_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b_data = vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0];

        let num_grad_a = numerical_gradient(
            |x| {
                let at = TensorValue::from_data(x.to_vec(), &[2, 3]).unwrap();
                let bt = TensorValue::from_data(b_data.clone(), &[3, 2]).unwrap();
                let ct = matmul(&at, &bt).unwrap();
                sum(&ct).to_scalar().unwrap()
            },
            &a_data,
            1e-5,
        );
        check_gradient(&grad_a, &num_grad_a, 1e-4, "matmul_a");

        let num_grad_b = numerical_gradient(
            |x| {
                let at = TensorValue::from_data(a_data.clone(), &[2, 3]).unwrap();
                let bt = TensorValue::from_data(x.to_vec(), &[3, 2]).unwrap();
                let ct = matmul(&at, &bt).unwrap();
                sum(&ct).to_scalar().unwrap()
            },
            &b_data,
            1e-5,
        );
        check_gradient(&grad_b, &num_grad_b, 1e-4, "matmul_b");
    }

    #[test]
    fn grad_relu() {
        let mut tape = Tape::new();
        let mut a = make_grad_tensor(vec![-2.0, -1.0, 0.5, 2.0], &[4]);
        a.set_id(tape.fresh_id());
        let c = relu_tracked(&a, &mut tape);
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        // relu grad: 0 for negative, 1 for positive
        assert_eq!(grad_a, vec![0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn grad_sigmoid_numerical() {
        let mut tape = Tape::new();
        let data = vec![0.5, -0.3, 1.0];
        let mut a = make_grad_tensor(data.clone(), &[3]);
        a.set_id(tape.fresh_id());
        let c = sigmoid_tracked(&a, &mut tape);
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let num = numerical_gradient(
            |x| {
                let t = TensorValue::from_data(x.to_vec(), &[3]).unwrap();
                sum(&sigmoid(&t)).to_scalar().unwrap()
            },
            &data,
            1e-5,
        );
        check_gradient(&grad_a, &num, 1e-4, "sigmoid");
    }

    #[test]
    fn grad_tanh_numerical() {
        let mut tape = Tape::new();
        let data = vec![0.5, -1.0, 2.0];
        let mut a = make_grad_tensor(data.clone(), &[3]);
        a.set_id(tape.fresh_id());
        let c = tanh_tracked(&a, &mut tape);
        let loss = sum_tracked(&c, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        let num = numerical_gradient(
            |x| {
                let t = TensorValue::from_data(x.to_vec(), &[3]).unwrap();
                sum(&tanh_act(&t)).to_scalar().unwrap()
            },
            &data,
            1e-5,
        );
        check_gradient(&grad_a, &num, 1e-4, "tanh");
    }

    #[test]
    fn grad_mean() {
        let mut tape = Tape::new();
        let mut a = make_grad_tensor(vec![1.0, 2.0, 3.0, 4.0], &[4]);
        a.set_id(tape.fresh_id());
        let loss = mean_tracked(&a, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_a: Vec<f64> = grads[&a.id().unwrap()].iter().copied().collect();
        // d(mean(x))/dx_i = 1/n = 0.25
        assert_eq!(grad_a, vec![0.25, 0.25, 0.25, 0.25]);
    }

    #[test]
    fn grad_chain_mul_add() {
        // f(x) = sum((x * 2) + 3)
        // df/dx = 2 for each element
        let mut tape = Tape::new();
        let mut x = make_grad_tensor(vec![1.0, 2.0, 3.0], &[3]);
        let two = TensorValue::from_data(vec![2.0, 2.0, 2.0], &[3]).unwrap();
        let three = TensorValue::from_data(vec![3.0, 3.0, 3.0], &[3]).unwrap();
        x.set_id(tape.fresh_id());
        let y = mul_tracked(&x, &two, &mut tape).unwrap();
        let z = add_tracked(&y, &three, &mut tape).unwrap();
        let loss = sum_tracked(&z, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();

        let grad_x: Vec<f64> = grads[&x.id().unwrap()].iter().copied().collect();
        assert_eq!(grad_x, vec![2.0, 2.0, 2.0]);
    }

    #[test]
    fn numerical_gradient_basic() {
        // f(x) = x^2, f'(x) = 2x
        let grad = numerical_gradient(|x| x[0] * x[0], &[3.0], 1e-5);
        assert!((grad[0] - 6.0).abs() < 1e-4);
    }

    #[test]
    fn reduce_broadcast_same_shape() {
        let g = ArrayD::ones(vec![2, 3]);
        let result = reduce_broadcast(&g, &[2, 3]);
        assert_eq!(result.shape(), &[2, 3]);
    }

    #[test]
    fn reduce_broadcast_scalar() {
        let g = ArrayD::ones(vec![3]);
        let result = reduce_broadcast(&g, &[]);
        assert_eq!(result.shape(), &[] as &[usize]);
        assert_eq!(result.iter().next().copied().unwrap(), 3.0);
    }

    #[test]
    fn reduce_broadcast_dimension() {
        // Broadcast from [1, 3] to [2, 3], reduce back to [1, 3]
        let g = ArrayD::ones(vec![2, 3]);
        let result = reduce_broadcast(&g, &[1, 3]);
        assert_eq!(result.shape(), &[1, 3]);
        assert!(result.iter().all(|&v| v == 2.0)); // summed along axis 0
    }

    // ── Loss Functions ──

    #[test]
    fn mse_loss_zero_for_equal() {
        let pred = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let target = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let loss = mse_loss(&pred, &target).unwrap();
        assert!((loss.to_scalar().unwrap()).abs() < 1e-10);
    }

    #[test]
    fn mse_loss_known_value() {
        // pred=[1,2], target=[3,4] → diff=[2,2] → sq=[4,4] → mean=4
        let pred = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let target = TensorValue::from_data(vec![3.0, 4.0], &[2]).unwrap();
        let loss = mse_loss(&pred, &target).unwrap();
        assert!((loss.to_scalar().unwrap() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn cross_entropy_known_value() {
        // One-hot target=[0,1,0], pred=[0.1, 0.8, 0.1]
        // CE = -(0*log(0.1) + 1*log(0.8) + 0*log(0.1)) = -log(0.8)
        let pred = TensorValue::from_data(vec![0.1, 0.8, 0.1], &[3]).unwrap();
        let target = TensorValue::from_data(vec![0.0, 1.0, 0.0], &[3]).unwrap();
        let loss = cross_entropy(&pred, &target).unwrap();
        let expected = -(0.8_f64).ln();
        assert!((loss.to_scalar().unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn cross_entropy_perfect_prediction() {
        // Perfect: target=[0,1], pred=[0,1] → CE should be close to 0
        let pred = TensorValue::from_data(vec![0.0001, 0.9999], &[2]).unwrap();
        let target = TensorValue::from_data(vec![0.0, 1.0], &[2]).unwrap();
        let loss = cross_entropy(&pred, &target).unwrap();
        assert!(loss.to_scalar().unwrap() < 0.001);
    }

    #[test]
    fn bce_loss_known_value() {
        // pred=[0.5], target=[1.0] → -[1*log(0.5) + 0*log(0.5)] = -log(0.5) = ln(2)
        let pred = TensorValue::from_data(vec![0.5], &[1]).unwrap();
        let target = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        let loss = bce_loss(&pred, &target).unwrap();
        assert!((loss.to_scalar().unwrap() - 2.0_f64.ln()).abs() < 1e-10);
    }

    #[test]
    fn bce_loss_perfect_prediction() {
        let pred = TensorValue::from_data(vec![0.999, 0.001], &[2]).unwrap();
        let target = TensorValue::from_data(vec![1.0, 0.0], &[2]).unwrap();
        let loss = bce_loss(&pred, &target).unwrap();
        assert!(loss.to_scalar().unwrap() < 0.01);
    }

    #[test]
    fn mse_loss_shape_mismatch() {
        let pred = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let target = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        assert!(matches!(
            mse_loss(&pred, &target),
            Err(TensorError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn grad_mse_loss_numerical() {
        let mut tape = Tape::new();
        let pred_data = vec![1.0, 2.0, 3.0];
        let target_data = vec![1.5, 2.5, 3.5];
        let mut pred = make_grad_tensor(pred_data.clone(), &[3]);
        let target = TensorValue::from_data(target_data.clone(), &[3]).unwrap();
        pred.set_id(tape.fresh_id());
        let loss = mse_loss_tracked(&pred, &target, &mut tape).unwrap();
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
        let grad_pred: Vec<f64> = grads[&pred.id().unwrap()].iter().copied().collect();

        let num = numerical_gradient(
            |x| {
                let p = TensorValue::from_data(x.to_vec(), &[3]).unwrap();
                let t = TensorValue::from_data(target_data.clone(), &[3]).unwrap();
                mse_loss(&p, &t).unwrap().to_scalar().unwrap()
            },
            &pred_data,
            1e-5,
        );
        check_gradient(&grad_pred, &num, 1e-4, "mse_loss");
    }

    #[test]
    fn grad_bce_loss_numerical() {
        let mut tape = Tape::new();
        let pred_data = vec![0.7, 0.3];
        let target_data = vec![1.0, 0.0];
        let mut pred = make_grad_tensor(pred_data.clone(), &[2]);
        let target = TensorValue::from_data(target_data.clone(), &[2]).unwrap();
        pred.set_id(tape.fresh_id());
        let loss = bce_loss_tracked(&pred, &target, &mut tape).unwrap();
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
        let grad_pred: Vec<f64> = grads[&pred.id().unwrap()].iter().copied().collect();

        let num = numerical_gradient(
            |x| {
                let p = TensorValue::from_data(x.to_vec(), &[2]).unwrap();
                let t = TensorValue::from_data(target_data.clone(), &[2]).unwrap();
                bce_loss(&p, &t).unwrap().to_scalar().unwrap()
            },
            &pred_data,
            1e-5,
        );
        check_gradient(&grad_pred, &num, 1e-3, "bce_loss");
    }

    // ── Squeeze / Unsqueeze ──

    #[test]
    fn squeeze_removes_axis() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[1, 3]).unwrap();
        let b = squeeze(&a, 0).unwrap();
        assert_eq!(b.shape(), &[3]);
        assert_eq!(b.to_vec(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn squeeze_non_one_axis_error() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        assert!(squeeze(&a, 0).is_err());
    }

    #[test]
    fn squeeze_out_of_range_error() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[1, 3]).unwrap();
        assert!(squeeze(&a, 5).is_err());
    }

    #[test]
    fn unsqueeze_adds_axis() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let b = unsqueeze(&a, 0).unwrap();
        assert_eq!(b.shape(), &[1, 3]);
        assert_eq!(b.to_vec(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn unsqueeze_at_end() {
        let a = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let b = unsqueeze(&a, 1).unwrap();
        assert_eq!(b.shape(), &[2, 1]);
    }

    #[test]
    fn unsqueeze_out_of_range_error() {
        let a = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        assert!(unsqueeze(&a, 5).is_err());
    }

    // ── Max / Min / Argmax ──

    #[test]
    fn max_tensor() {
        let a = TensorValue::from_data(vec![3.0, 1.0, 4.0, 1.0, 5.0], &[5]).unwrap();
        let m = max(&a);
        assert_eq!(m.to_scalar().unwrap(), 5.0);
    }

    #[test]
    fn min_tensor() {
        let a = TensorValue::from_data(vec![3.0, 1.0, 4.0, 1.0, 5.0], &[5]).unwrap();
        let m = min(&a);
        assert_eq!(m.to_scalar().unwrap(), 1.0);
    }

    #[test]
    fn argmax_tensor() {
        let a = TensorValue::from_data(vec![3.0, 1.0, 4.0, 1.0, 5.0], &[5]).unwrap();
        let idx = argmax(&a);
        assert_eq!(idx.to_scalar().unwrap(), 4.0); // index 4 has value 5.0
    }

    #[test]
    fn argmax_first_element() {
        let a = TensorValue::from_data(vec![9.0, 1.0, 2.0], &[3]).unwrap();
        let idx = argmax(&a);
        assert_eq!(idx.to_scalar().unwrap(), 0.0);
    }

    // ── Arange / Linspace ──

    #[test]
    fn arange_basic() {
        let a = arange(0.0, 5.0, 1.0).unwrap();
        assert_eq!(a.shape(), &[5]);
        assert_eq!(a.to_vec(), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn arange_fractional_step() {
        let a = arange(0.0, 1.0, 0.5).unwrap();
        assert_eq!(a.shape(), &[2]);
        assert_eq!(a.to_vec(), vec![0.0, 0.5]);
    }

    #[test]
    fn arange_zero_step_error() {
        assert!(arange(0.0, 5.0, 0.0).is_err());
    }

    #[test]
    fn linspace_basic() {
        let a = linspace(0.0, 1.0, 5).unwrap();
        assert_eq!(a.shape(), &[5]);
        let vals = a.to_vec();
        assert!((vals[0] - 0.0).abs() < 1e-10);
        assert!((vals[4] - 1.0).abs() < 1e-10);
        assert!((vals[2] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn linspace_single_point() {
        let a = linspace(5.0, 5.0, 1).unwrap();
        assert_eq!(a.shape(), &[1]);
        assert_eq!(a.to_vec(), vec![5.0]);
    }

    #[test]
    fn linspace_zero_steps_error() {
        assert!(linspace(0.0, 1.0, 0).is_err());
    }

    // ── Xavier ──

    #[test]
    fn xavier_shape() {
        let w = xavier(3, 4);
        assert_eq!(w.shape(), &[3, 4]);
    }

    #[test]
    fn xavier_bounded() {
        let w = xavier(100, 100);
        let limit = (6.0_f64 / 200.0).sqrt();
        for &v in w.to_vec().iter() {
            assert!(
                v.abs() <= limit + 0.01,
                "xavier value {v} exceeds limit {limit}"
            );
        }
    }

    // ── L1 Loss ──

    #[test]
    fn l1_loss_zero_for_equal() {
        let pred = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let target = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let loss_val = l1_loss(&pred, &target).unwrap();
        assert!((loss_val.to_scalar().unwrap()).abs() < 1e-10);
    }

    #[test]
    fn l1_loss_known_value() {
        // pred=[1,2], target=[3,5] → abs_diff=[2,3] → mean=2.5
        let pred = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let target = TensorValue::from_data(vec![3.0, 5.0], &[2]).unwrap();
        let loss_val = l1_loss(&pred, &target).unwrap();
        assert!((loss_val.to_scalar().unwrap() - 2.5).abs() < 1e-10);
    }

    #[test]
    fn l1_loss_shape_mismatch() {
        let pred = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let target = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        assert!(matches!(
            l1_loss(&pred, &target),
            Err(TensorError::ShapeMismatch { .. })
        ));
    }
}
