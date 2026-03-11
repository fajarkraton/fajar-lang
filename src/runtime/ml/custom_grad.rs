//! Custom autodiff operations for Fajar Lang.
//!
//! Provides JVP (forward-mode) and VJP (reverse-mode) automatic
//! differentiation for user-defined operations. Includes a registry
//! for custom ops, numerical gradient checking, and built-in custom
//! activation functions (Swish, Mish, GELU, FocalLoss).

use ndarray::Array2;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from custom autodiff operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum CustomGradError {
    /// Unknown operation name.
    #[error("unknown custom op: '{name}'")]
    UnknownOp {
        /// The unregistered operation name.
        name: String,
    },

    /// Shape mismatch in gradient computation.
    #[error("gradient shape mismatch: expected {expected:?}, got {got:?}")]
    ShapeMismatch {
        /// Expected shape.
        expected: [usize; 2],
        /// Actual shape.
        got: [usize; 2],
    },

    /// Numerical gradient check failed.
    #[error("numerical gradient check failed: max error {max_error:.6} exceeds tolerance {tolerance:.6}")]
    GradientCheckFailed {
        /// Maximum absolute error between analytical and numerical gradients.
        max_error: f64,
        /// Tolerance threshold.
        tolerance: f64,
    },

    /// Invalid epsilon for numerical gradient.
    #[error("invalid epsilon {epsilon}: must be > 0")]
    InvalidEpsilon {
        /// The invalid epsilon.
        epsilon: f64,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Custom op trait
// ═══════════════════════════════════════════════════════════════════════

/// A user-defined differentiable operation.
///
/// Implementations must provide both the forward computation and
/// the backward (gradient) computation.
pub trait CustomOp: std::fmt::Debug {
    /// The name of this operation (used for registry lookup).
    fn name(&self) -> &str;

    /// Forward pass: compute output from inputs.
    fn forward(&self, input: &Array2<f64>) -> Array2<f64>;

    /// Backward pass: compute input gradient from output gradient.
    ///
    /// `grad_output` has the same shape as the forward output.
    /// Returns a gradient with the same shape as the forward input.
    fn backward(&self, input: &Array2<f64>, grad_output: &Array2<f64>) -> Array2<f64>;
}

// ═══════════════════════════════════════════════════════════════════════
// JVP and VJP operations
// ═══════════════════════════════════════════════════════════════════════

/// Forward-mode autodiff: Jacobian-Vector Product (JVP).
///
/// Computes `J * v` where J is the Jacobian of `op` at `input` and
/// `v` is the tangent vector. Efficient for functions with few inputs.
#[derive(Debug)]
pub struct JvpOp<'a> {
    /// The underlying custom op.
    op: &'a dyn CustomOp,
}

impl<'a> JvpOp<'a> {
    /// Creates a new JVP wrapper around a custom op.
    pub fn new(op: &'a dyn CustomOp) -> Self {
        Self { op }
    }

    /// Computes the JVP: forward output and tangent propagation.
    ///
    /// Returns `(output, jvp)` where `jvp = J * tangent`.
    pub fn compute(
        &self,
        input: &Array2<f64>,
        tangent: &Array2<f64>,
    ) -> (Array2<f64>, Array2<f64>) {
        let output = self.op.forward(input);
        // JVP = backward(input, tangent) interpreted as forward tangent
        let jvp = self.op.backward(input, tangent);
        (output, jvp)
    }
}

/// Reverse-mode autodiff: Vector-Jacobian Product (VJP).
///
/// Computes `v^T * J` where J is the Jacobian of `op` at `input` and
/// `v` is the cotangent vector. Efficient for functions with few outputs.
#[derive(Debug)]
pub struct VjpOp<'a> {
    /// The underlying custom op.
    op: &'a dyn CustomOp,
}

impl<'a> VjpOp<'a> {
    /// Creates a new VJP wrapper around a custom op.
    pub fn new(op: &'a dyn CustomOp) -> Self {
        Self { op }
    }

    /// Computes the VJP: forward output and cotangent propagation.
    ///
    /// Returns `(output, vjp)` where `vjp = cotangent^T * J`.
    pub fn compute(
        &self,
        input: &Array2<f64>,
        cotangent: &Array2<f64>,
    ) -> (Array2<f64>, Array2<f64>) {
        let output = self.op.forward(input);
        let vjp = self.op.backward(input, cotangent);
        (output, vjp)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Custom op registry
// ═══════════════════════════════════════════════════════════════════════

/// Function-pointer based custom op for registration without trait objects.
#[derive(Clone)]
pub struct FnCustomOp {
    /// Operation name.
    name: String,
    /// Forward function.
    forward_fn: fn(&Array2<f64>) -> Array2<f64>,
    /// Backward function.
    backward_fn: fn(&Array2<f64>, &Array2<f64>) -> Array2<f64>,
}

impl std::fmt::Debug for FnCustomOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FnCustomOp")
            .field("name", &self.name)
            .finish()
    }
}

impl CustomOp for FnCustomOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, input: &Array2<f64>) -> Array2<f64> {
        (self.forward_fn)(input)
    }

    fn backward(&self, input: &Array2<f64>, grad_output: &Array2<f64>) -> Array2<f64> {
        (self.backward_fn)(input, grad_output)
    }
}

/// Registry for custom differentiable operations.
///
/// Stores named operations that can be looked up and applied during
/// forward/backward passes.
#[derive(Debug)]
pub struct CustomOpRegistry {
    /// Registered operations by name.
    ops: HashMap<String, FnCustomOp>,
}

impl CustomOpRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            ops: HashMap::new(),
        }
    }

    /// Creates a registry pre-loaded with built-in custom ops.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtin_ops();
        registry
    }

    /// Registers a custom operation.
    pub fn register(
        &mut self,
        name: &str,
        forward_fn: fn(&Array2<f64>) -> Array2<f64>,
        backward_fn: fn(&Array2<f64>, &Array2<f64>) -> Array2<f64>,
    ) {
        self.ops.insert(
            name.to_string(),
            FnCustomOp {
                name: name.to_string(),
                forward_fn,
                backward_fn,
            },
        );
    }

    /// Looks up a registered operation by name.
    pub fn get(&self, name: &str) -> Result<&FnCustomOp, CustomGradError> {
        self.ops
            .get(name)
            .ok_or_else(|| CustomGradError::UnknownOp {
                name: name.to_string(),
            })
    }

    /// Returns the number of registered operations.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Returns all registered operation names.
    pub fn names(&self) -> Vec<&str> {
        self.ops.keys().map(|s| s.as_str()).collect()
    }

    /// Applies a registered op's forward pass.
    pub fn forward(&self, name: &str, input: &Array2<f64>) -> Result<Array2<f64>, CustomGradError> {
        let op = self.get(name)?;
        Ok(op.forward(input))
    }

    /// Applies a registered op's backward pass.
    pub fn backward(
        &self,
        name: &str,
        input: &Array2<f64>,
        grad_output: &Array2<f64>,
    ) -> Result<Array2<f64>, CustomGradError> {
        let op = self.get(name)?;
        Ok(op.backward(input, grad_output))
    }

    /// Registers all built-in custom operations.
    fn register_builtin_ops(&mut self) {
        self.register("swish", swish_forward, swish_backward);
        self.register("mish", mish_forward, mish_backward);
        self.register("gelu_approx", gelu_approx_forward, gelu_approx_backward);
    }
}

impl Default for CustomOpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Built-in custom ops: Swish, Mish, GELU approximation
// ═══════════════════════════════════════════════════════════════════════

/// Swish activation: `x * sigmoid(x)`.
fn swish_forward(input: &Array2<f64>) -> Array2<f64> {
    input.mapv(|x| x * sigmoid_scalar(x))
}

/// Swish backward: `sigmoid(x) + x * sigmoid(x) * (1 - sigmoid(x))`.
fn swish_backward(input: &Array2<f64>, grad_output: &Array2<f64>) -> Array2<f64> {
    let grad = input.mapv(|x| {
        let s = sigmoid_scalar(x);
        s + x * s * (1.0 - s)
    });
    &grad * grad_output
}

/// Mish activation: `x * tanh(softplus(x))` where `softplus(x) = ln(1 + exp(x))`.
fn mish_forward(input: &Array2<f64>) -> Array2<f64> {
    input.mapv(|x| x * softplus_scalar(x).tanh())
}

/// Mish backward (approximation).
fn mish_backward(input: &Array2<f64>, grad_output: &Array2<f64>) -> Array2<f64> {
    let grad = input.mapv(|x| {
        let sp = softplus_scalar(x);
        let tanh_sp = sp.tanh();
        let sech2 = 1.0 - tanh_sp * tanh_sp;
        let sigmoid_x = sigmoid_scalar(x);
        tanh_sp + x * sech2 * sigmoid_x
    });
    &grad * grad_output
}

/// GELU approximation: `0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))`.
fn gelu_approx_forward(input: &Array2<f64>) -> Array2<f64> {
    let sqrt_2_pi = (2.0 / std::f64::consts::PI).sqrt();
    input.mapv(|x| {
        let inner = sqrt_2_pi * (x + 0.044715 * x.powi(3));
        0.5 * x * (1.0 + inner.tanh())
    })
}

/// GELU approximation backward.
fn gelu_approx_backward(input: &Array2<f64>, grad_output: &Array2<f64>) -> Array2<f64> {
    let sqrt_2_pi = (2.0 / std::f64::consts::PI).sqrt();
    let grad = input.mapv(|x| {
        let inner = sqrt_2_pi * (x + 0.044715 * x.powi(3));
        let tanh_inner = inner.tanh();
        let sech2 = 1.0 - tanh_inner * tanh_inner;
        let d_inner = sqrt_2_pi * (1.0 + 3.0 * 0.044715 * x * x);
        0.5 * (1.0 + tanh_inner) + 0.5 * x * sech2 * d_inner
    });
    &grad * grad_output
}

/// Scalar sigmoid helper.
fn sigmoid_scalar(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Scalar softplus helper: `ln(1 + exp(x))` with overflow protection.
fn softplus_scalar(x: f64) -> f64 {
    if x > 20.0 {
        x
    } else if x < -20.0 {
        0.0
    } else {
        (1.0 + x.exp()).ln()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Focal loss (custom op)
// ═══════════════════════════════════════════════════════════════════════

/// Focal loss custom op for class-imbalanced classification.
///
/// `FL(p) = -alpha * (1 - p)^gamma * log(p)`
///
/// Focuses learning on hard (misclassified) examples.
#[derive(Debug, Clone)]
pub struct FocalLoss {
    /// Balancing factor (typically 0.25).
    pub alpha: f64,
    /// Focusing parameter (typically 2.0).
    pub gamma: f64,
}

impl FocalLoss {
    /// Creates a new focal loss op.
    pub fn new(alpha: f64, gamma: f64) -> Self {
        Self { alpha, gamma }
    }

    /// Computes focal loss for predictions and targets.
    ///
    /// `predictions` and `targets` must have the same shape.
    pub fn compute(
        &self,
        predictions: &Array2<f64>,
        targets: &Array2<f64>,
    ) -> Result<f64, CustomGradError> {
        let p_shape = [predictions.nrows(), predictions.ncols()];
        let t_shape = [targets.nrows(), targets.ncols()];
        if p_shape != t_shape {
            return Err(CustomGradError::ShapeMismatch {
                expected: t_shape,
                got: p_shape,
            });
        }

        let eps = 1e-10;
        let mut total = 0.0;
        for (p, t) in predictions.iter().zip(targets.iter()) {
            let p_safe = p.clamp(eps, 1.0 - eps);
            let focal_weight = (1.0 - p_safe).powf(self.gamma);
            total -= self.alpha * focal_weight * t * p_safe.ln();
        }

        let n = predictions.len();
        Ok(if n > 0 { total / n as f64 } else { 0.0 })
    }

    /// Computes gradient of focal loss w.r.t. predictions.
    pub fn gradient(
        &self,
        predictions: &Array2<f64>,
        targets: &Array2<f64>,
    ) -> Result<Array2<f64>, CustomGradError> {
        let p_shape = [predictions.nrows(), predictions.ncols()];
        let t_shape = [targets.nrows(), targets.ncols()];
        if p_shape != t_shape {
            return Err(CustomGradError::ShapeMismatch {
                expected: t_shape,
                got: p_shape,
            });
        }

        let eps = 1e-10;
        let n = predictions.len() as f64;
        let mut grad = Array2::zeros((predictions.nrows(), predictions.ncols()));

        for ((idx, p), t) in predictions.indexed_iter().zip(targets.iter()) {
            let p_safe = p.clamp(eps, 1.0 - eps);
            let one_minus_p = 1.0 - p_safe;
            let focal = one_minus_p.powf(self.gamma);
            // d/dp [-alpha * (1-p)^gamma * t * ln(p)]
            let d_log = -self.alpha * focal * t / p_safe;
            let d_focal =
                self.alpha * self.gamma * one_minus_p.powf(self.gamma - 1.0) * t * p_safe.ln();
            grad[idx] = (d_log + d_focal) / n;
        }

        Ok(grad)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Numerical gradient check
// ═══════════════════════════════════════════════════════════════════════

/// Checks analytical gradients against numerical gradients.
///
/// Uses central differences: `(f(x+eps) - f(x-eps)) / (2*eps)`.
/// Returns `Ok(max_error)` if the check passes, `Err` if it fails.
pub fn numerical_gradient_check(
    op: &dyn CustomOp,
    input: &Array2<f64>,
    epsilon: f64,
    tolerance: f64,
) -> Result<f64, CustomGradError> {
    if epsilon <= 0.0 {
        return Err(CustomGradError::InvalidEpsilon { epsilon });
    }

    let rows = input.nrows();
    let cols = input.ncols();

    // Compute analytical gradient with ones as grad_output
    let grad_output = Array2::ones((rows, cols));
    let analytical = op.backward(input, &grad_output);

    // Compute numerical gradient via central differences
    let mut numerical = Array2::zeros((rows, cols));
    for r in 0..rows {
        for c in 0..cols {
            let mut x_plus = input.clone();
            let mut x_minus = input.clone();
            x_plus[[r, c]] += epsilon;
            x_minus[[r, c]] -= epsilon;

            let f_plus: f64 = op.forward(&x_plus).iter().sum();
            let f_minus: f64 = op.forward(&x_minus).iter().sum();

            numerical[[r, c]] = (f_plus - f_minus) / (2.0 * epsilon);
        }
    }

    // Find max absolute difference
    let max_error = analytical
        .iter()
        .zip(numerical.iter())
        .map(|(a, n)| (a - n).abs())
        .fold(0.0_f64, f64::max);

    if max_error > tolerance {
        return Err(CustomGradError::GradientCheckFailed {
            max_error,
            tolerance,
        });
    }

    Ok(max_error)
}

// ═══════════════════════════════════════════════════════════════════════
// Composition: chain two custom ops
// ═══════════════════════════════════════════════════════════════════════

/// Chains two custom ops: applies `first` then `second`.
///
/// Forward: `second(first(input))`
/// Backward: `first.backward(input, second.backward(first(input), grad))`
#[derive(Debug)]
pub struct ChainedOp {
    /// First operation name.
    first_name: String,
    /// Second operation name.
    second_name: String,
    /// First op functions.
    first: FnCustomOp,
    /// Second op functions.
    second: FnCustomOp,
}

impl ChainedOp {
    /// Creates a chained op from two registered ops in a registry.
    pub fn from_registry(
        registry: &CustomOpRegistry,
        first: &str,
        second: &str,
    ) -> Result<Self, CustomGradError> {
        let first_op = registry.get(first)?.clone();
        let second_op = registry.get(second)?.clone();
        Ok(Self {
            first_name: first.to_string(),
            second_name: second.to_string(),
            first: first_op,
            second: second_op,
        })
    }

    /// Forward pass: second(first(input)).
    pub fn forward(&self, input: &Array2<f64>) -> Array2<f64> {
        let intermediate = self.first.forward(input);
        self.second.forward(&intermediate)
    }

    /// Backward pass via chain rule.
    pub fn backward(&self, input: &Array2<f64>, grad_output: &Array2<f64>) -> Array2<f64> {
        let intermediate = self.first.forward(input);
        let grad_second = self.second.backward(&intermediate, grad_output);
        self.first.backward(input, &grad_second)
    }

    /// Returns the names of the chained operations.
    pub fn names(&self) -> (&str, &str) {
        (&self.first_name, &self.second_name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s15_1_custom_op_registration_and_lookup() {
        let mut registry = CustomOpRegistry::new();
        registry.register("double", |x| x * 2.0, |_input, grad| grad * 2.0);

        assert_eq!(registry.len(), 1);
        assert!(registry.get("double").is_ok());
        assert!(registry.get("unknown").is_err());
    }

    #[test]
    fn s15_2_custom_op_forward_backward() {
        let mut registry = CustomOpRegistry::new();
        registry.register("square", |x| x * x, |input, grad| &(input * 2.0) * grad);

        let input = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let output = registry.forward("square", &input).unwrap();

        assert!((output[[0, 0]] - 1.0).abs() < 1e-10);
        assert!((output[[0, 1]] - 4.0).abs() < 1e-10);
        assert!((output[[1, 0]] - 9.0).abs() < 1e-10);
        assert!((output[[1, 1]] - 16.0).abs() < 1e-10);

        let grad_out = Array2::ones((2, 2));
        let grad_in = registry.backward("square", &input, &grad_out).unwrap();

        assert!((grad_in[[0, 0]] - 2.0).abs() < 1e-10);
        assert!((grad_in[[0, 1]] - 4.0).abs() < 1e-10);
        assert!((grad_in[[1, 0]] - 6.0).abs() < 1e-10);
        assert!((grad_in[[1, 1]] - 8.0).abs() < 1e-10);
    }

    #[test]
    fn s15_3_jvp_forward_mode() {
        let mut registry = CustomOpRegistry::new();
        registry.register("scale3", |x| x * 3.0, |_input, grad| grad * 3.0);

        let op = registry.get("scale3").unwrap();
        let jvp = JvpOp::new(op);

        let input = Array2::from_shape_vec((1, 3), vec![1.0, 2.0, 3.0]).unwrap();
        let tangent = Array2::ones((1, 3));

        let (output, jvp_result) = jvp.compute(&input, &tangent);

        // output = 3 * input
        assert!((output[[0, 0]] - 3.0).abs() < 1e-10);
        // jvp = 3 * tangent
        assert!((jvp_result[[0, 0]] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn s15_4_vjp_reverse_mode() {
        let mut registry = CustomOpRegistry::new();
        registry.register("scale3", |x| x * 3.0, |_input, grad| grad * 3.0);

        let op = registry.get("scale3").unwrap();
        let vjp = VjpOp::new(op);

        let input = Array2::from_shape_vec((1, 3), vec![1.0, 2.0, 3.0]).unwrap();
        let cotangent = Array2::ones((1, 3));

        let (output, vjp_result) = vjp.compute(&input, &cotangent);

        assert!((output[[0, 0]] - 3.0).abs() < 1e-10);
        assert!((vjp_result[[0, 0]] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn s15_5_numerical_gradient_check_passes() {
        let mut registry = CustomOpRegistry::new();
        registry.register("square", |x| x * x, |input, grad| &(input * 2.0) * grad);

        let op = registry.get("square").unwrap();
        let input = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();

        let max_error = numerical_gradient_check(op, &input, 1e-5, 1e-3).unwrap();
        assert!(max_error < 1e-3);
    }

    #[test]
    fn s15_6_swish_builtin_op() {
        let registry = CustomOpRegistry::with_builtins();
        assert!(registry.get("swish").is_ok());

        let input = Array2::from_shape_vec((1, 3), vec![0.0, 1.0, -1.0]).unwrap();
        let output = registry.forward("swish", &input).unwrap();

        // swish(0) = 0 * sigmoid(0) = 0 * 0.5 = 0
        assert!((output[[0, 0]]).abs() < 1e-10);
        // swish(1) = 1 * sigmoid(1) ≈ 0.7311
        assert!((output[[0, 1]] - 0.7311).abs() < 0.01);
    }

    #[test]
    fn s15_7_mish_builtin_op() {
        let registry = CustomOpRegistry::with_builtins();
        let input = Array2::from_shape_vec((1, 3), vec![0.0, 1.0, -1.0]).unwrap();
        let output = registry.forward("mish", &input).unwrap();

        // mish(0) = 0 * tanh(softplus(0)) = 0 * tanh(ln(2)) = 0
        assert!((output[[0, 0]]).abs() < 1e-10);
        // mish(1) ≈ 0.8651
        assert!((output[[0, 1]] - 0.8651).abs() < 0.01);
    }

    #[test]
    fn s15_8_focal_loss_computation() {
        let predictions =
            Array2::from_shape_vec((2, 3), vec![0.9, 0.05, 0.05, 0.1, 0.8, 0.1]).unwrap();
        let targets = Array2::from_shape_vec((2, 3), vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap();

        let focal = FocalLoss::new(0.25, 2.0);
        let loss = focal.compute(&predictions, &targets).unwrap();

        assert!(loss.is_finite());
        assert!(loss >= 0.0);
        // Well-classified examples should have low focal loss
        assert!(
            loss < 0.1,
            "loss {loss} should be small for good predictions"
        );
    }

    #[test]
    fn s15_9_chained_ops_composition() {
        let registry = CustomOpRegistry::with_builtins();
        let chain = ChainedOp::from_registry(&registry, "swish", "mish").unwrap();

        let input = Array2::from_shape_vec((1, 2), vec![1.0, -1.0]).unwrap();
        let output = chain.forward(&input);

        // Should produce mish(swish(input))
        let swish_out = registry.forward("swish", &input).unwrap();
        let expected = registry.forward("mish", &swish_out).unwrap();

        for (a, b) in output.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    #[test]
    fn s15_10_gradient_check_detects_wrong_gradient() {
        let mut registry = CustomOpRegistry::new();
        // Intentionally wrong backward: returns constant 1.0 instead of 2*x
        registry.register(
            "bad_square",
            |x| x * x,
            |_input, grad| grad.clone(), // WRONG: should be 2*x*grad
        );

        let op = registry.get("bad_square").unwrap();
        let input = Array2::from_shape_vec((1, 2), vec![3.0, 4.0]).unwrap();

        let result = numerical_gradient_check(op, &input, 1e-5, 1e-3);
        assert!(result.is_err(), "should detect wrong gradient");
    }
}
