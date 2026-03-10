//! Automatic differentiation — tape-based reverse-mode autograd.
//!
//! Records operations on a `Tape` and replays them in reverse during
//! `backward()` to compute gradients via the chain rule.

use ndarray::ArrayD;
use std::collections::HashMap;

use super::tensor::TensorError;

/// Unique identifier for a tensor in the computation graph.
pub type TensorId = u64;

/// Gradient function: given the gradient of the output, returns gradients for each input.
///
/// The returned `Vec` has one entry per input tensor, in the same order as `inputs`
/// in the corresponding `TapeEntry`.
pub type GradFn = Box<dyn Fn(&ArrayD<f64>) -> Vec<ArrayD<f64>>>;

/// A single recorded operation in the computation graph.
pub struct TapeEntry {
    /// The output tensor id.
    pub output_id: TensorId,
    /// Input tensor ids (in order).
    pub input_ids: Vec<TensorId>,
    /// Function to compute input gradients from output gradient.
    pub grad_fn: GradFn,
}

/// Tape-based computation graph for automatic differentiation.
///
/// Records forward operations and replays them in reverse for backward pass.
pub struct Tape {
    /// Recorded operations, in forward execution order.
    entries: Vec<TapeEntry>,
    /// Counter for generating unique tensor ids.
    next_id: TensorId,
    /// Whether recording is enabled (disabled inside `no_grad` blocks).
    recording: bool,
}

impl Tape {
    /// Creates a new empty tape with recording enabled.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
            recording: true,
        }
    }

    /// Generates a fresh unique tensor id.
    pub fn fresh_id(&mut self) -> TensorId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Records an operation on the tape.
    ///
    /// No-op if recording is disabled (inside `no_grad` context).
    pub fn record(&mut self, output_id: TensorId, input_ids: Vec<TensorId>, grad_fn: GradFn) {
        if !self.recording {
            return;
        }
        self.entries.push(TapeEntry {
            output_id,
            input_ids,
            grad_fn,
        });
    }

    /// Returns whether recording is currently enabled.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Disables recording (for `no_grad` context).
    pub fn set_recording(&mut self, enabled: bool) {
        self.recording = enabled;
    }

    /// Returns the number of recorded operations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the tape is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all recorded operations.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Runs the backward pass from a scalar output tensor.
    ///
    /// Walks the tape in reverse, applying the chain rule to accumulate
    /// gradients for each tensor that `requires_grad`.
    ///
    /// Returns a map from `TensorId` → gradient `ArrayD<f64>`.
    pub fn backward(
        &self,
        output_id: TensorId,
        output_shape: &[usize],
    ) -> Result<HashMap<TensorId, ArrayD<f64>>, TensorError> {
        let mut grads: HashMap<TensorId, ArrayD<f64>> = HashMap::new();

        // Seed gradient: ones with the output shape (scalar = [])
        let seed = ArrayD::ones(output_shape);
        grads.insert(output_id, seed);

        // Walk tape in reverse
        for entry in self.entries.iter().rev() {
            // Get the gradient for this entry's output (skip if no gradient flows here)
            let grad_output = match grads.get(&entry.output_id) {
                Some(g) => g.clone(),
                None => continue,
            };

            // Compute input gradients
            let input_grads = (entry.grad_fn)(&grad_output);

            // Accumulate into each input
            for (input_id, input_grad) in entry.input_ids.iter().zip(input_grads.into_iter()) {
                grads
                    .entry(*input_id)
                    .and_modify(|existing| *existing += &input_grad)
                    .or_insert(input_grad);
            }
        }

        Ok(grads)
    }
}

impl Default for Tape {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Tape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tape")
            .field("entries", &self.entries.len())
            .field("next_id", &self.next_id)
            .field("recording", &self.recording)
            .finish()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_id_increments() {
        let mut tape = Tape::new();
        assert_eq!(tape.fresh_id(), 0);
        assert_eq!(tape.fresh_id(), 1);
        assert_eq!(tape.fresh_id(), 2);
    }

    #[test]
    fn tape_starts_empty() {
        let tape = Tape::new();
        assert!(tape.is_empty());
        assert_eq!(tape.len(), 0);
        assert!(tape.is_recording());
    }

    #[test]
    fn record_adds_entry() {
        let mut tape = Tape::new();
        tape.record(0, vec![1, 2], Box::new(|g| vec![g.clone(), g.clone()]));
        assert_eq!(tape.len(), 1);
    }

    #[test]
    fn no_grad_disables_recording() {
        let mut tape = Tape::new();
        tape.set_recording(false);
        tape.record(0, vec![1], Box::new(|g| vec![g.clone()]));
        assert!(tape.is_empty());
    }

    #[test]
    fn clear_removes_entries() {
        let mut tape = Tape::new();
        tape.record(0, vec![1], Box::new(|g| vec![g.clone()]));
        tape.record(1, vec![2], Box::new(|g| vec![g.clone()]));
        assert_eq!(tape.len(), 2);
        tape.clear();
        assert!(tape.is_empty());
    }

    #[test]
    fn backward_scalar_identity() {
        // f(x) = x, so df/dx = 1
        let mut tape = Tape::new();
        // x has id 0, output has id 1
        // Operation: output = identity(x), grad_fn: grad_input = grad_output
        tape.record(1, vec![0], Box::new(|g| vec![g.clone()]));

        let grads = tape.backward(1, &[]).unwrap();
        let grad_x = grads.get(&0).unwrap();
        assert_eq!(grad_x.iter().next().copied().unwrap(), 1.0);
    }

    #[test]
    fn backward_addition() {
        // f(a, b) = a + b, df/da = 1, df/db = 1
        let mut tape = Tape::new();
        // a=0, b=1, output=2
        tape.record(2, vec![0, 1], Box::new(|g| vec![g.clone(), g.clone()]));

        let grads = tape.backward(2, &[]).unwrap();
        assert_eq!(grads.get(&0).unwrap().iter().next().copied().unwrap(), 1.0);
        assert_eq!(grads.get(&1).unwrap().iter().next().copied().unwrap(), 1.0);
    }

    #[test]
    fn backward_chain_rule() {
        // f(x) = 2 * (x + 1)
        // Step 1: y = x + 1  → dy/dx = 1
        // Step 2: z = 2 * y  → dz/dy = 2
        // Chain: dz/dx = dz/dy * dy/dx = 2 * 1 = 2
        let mut tape = Tape::new();
        // x=0, const_1=1 (no grad), y=2, const_2=3 (no grad), z=4
        tape.record(2, vec![0, 1], Box::new(|g| vec![g.clone(), g.clone()])); // y = x + 1
        tape.record(
            4,
            vec![2, 3],
            Box::new(|g| {
                // z = 2 * y: dz/dy = 2 (the constant), dz/d(const) = y
                let grad_y = g.mapv(|v| v * 2.0);
                let grad_const = g.clone(); // we don't care about this
                vec![grad_y, grad_const]
            }),
        );

        let grads = tape.backward(4, &[]).unwrap();
        assert_eq!(grads.get(&0).unwrap().iter().next().copied().unwrap(), 2.0);
    }

    #[test]
    fn backward_accumulates_multiple_uses() {
        // f(x) = x + x = 2x → df/dx = 2
        let mut tape = Tape::new();
        // x=0, output=1, both inputs are x
        tape.record(1, vec![0, 0], Box::new(|g| vec![g.clone(), g.clone()]));

        let grads = tape.backward(1, &[]).unwrap();
        // Gradient should be accumulated: 1 + 1 = 2
        assert_eq!(grads.get(&0).unwrap().iter().next().copied().unwrap(), 2.0);
    }

    #[test]
    fn backward_no_entry_for_output_returns_empty() {
        let tape = Tape::new(); // empty tape
        let grads = tape.backward(99, &[]).unwrap();
        // Only the seed gradient for the output exists
        assert!(grads.contains_key(&99));
        assert_eq!(grads.len(), 1);
    }

    #[test]
    fn backward_vector_gradients() {
        // f(x) = x element-wise, shape [3]
        let mut tape = Tape::new();
        tape.record(1, vec![0], Box::new(|g| vec![g.clone()]));

        let grads = tape.backward(1, &[3]).unwrap();
        let grad_x = grads.get(&0).unwrap();
        assert_eq!(grad_x.shape(), &[3]);
        assert!(grad_x.iter().all(|&v| v == 1.0));
    }

    // ═══════════════════════════════════════════════════════════════════
    // Numerical finite-difference gradient checking
    // ═══════════════════════════════════════════════════════════════════

    /// Compute numerical gradient via central difference: (f(x+eps) - f(x-eps)) / (2*eps).
    /// `f` takes an input array and returns a scalar output.
    fn numerical_grad(x: &ArrayD<f64>, f: &dyn Fn(&ArrayD<f64>) -> f64) -> ArrayD<f64> {
        let eps = 1e-5;
        let mut grad = ArrayD::zeros(x.shape());
        let flat_len = x.len();
        for i in 0..flat_len {
            let mut x_plus = x.clone();
            let mut x_minus = x.clone();
            x_plus.as_slice_mut().unwrap()[i] += eps;
            x_minus.as_slice_mut().unwrap()[i] -= eps;
            let df = (f(&x_plus) - f(&x_minus)) / (2.0 * eps);
            grad.as_slice_mut().unwrap()[i] = df;
        }
        grad
    }

    #[test]
    fn numcheck_add_gradient() {
        // f(a, b) = sum(a + b)
        let a = ArrayD::from_shape_vec(vec![3], vec![1.0, 2.0, 3.0]).unwrap();
        let b = ArrayD::from_shape_vec(vec![3], vec![4.0, 5.0, 6.0]).unwrap();

        // Analytical: df/da = [1, 1, 1]
        let mut tape = Tape::new();
        let a_id = tape.fresh_id(); // 0
        let b_id = tape.fresh_id(); // 1
        let c_id = tape.fresh_id(); // 2
        let s_id = tape.fresh_id(); // 3
                                    // c = a + b
        tape.record(
            c_id,
            vec![a_id, b_id],
            Box::new(|g| vec![g.clone(), g.clone()]),
        );
        // s = sum(c)
        let n = 3;
        tape.record(
            s_id,
            vec![c_id],
            Box::new(move |_g| vec![ArrayD::ones(vec![n])]),
        );
        let grads = tape.backward(s_id, &[]).unwrap();
        let analytical = grads.get(&a_id).unwrap();

        // Numerical
        let b_clone = b.clone();
        let numerical = numerical_grad(&a, &|x| (x + &b_clone).sum());

        for (an, nu) in analytical.iter().zip(numerical.iter()) {
            assert!((an - nu).abs() < 1e-4, "analytical={an}, numerical={nu}");
        }
    }

    #[test]
    fn numcheck_mul_gradient() {
        // f(a, b) = sum(a * b), df/da = b
        let a = ArrayD::from_shape_vec(vec![3], vec![2.0, 3.0, 4.0]).unwrap();
        let b = ArrayD::from_shape_vec(vec![3], vec![5.0, 6.0, 7.0]).unwrap();

        // Analytical
        let mut tape = Tape::new();
        let a_id = tape.fresh_id();
        let b_id = tape.fresh_id();
        let c_id = tape.fresh_id();
        let s_id = tape.fresh_id();
        let a_c = a.clone();
        let b_c = b.clone();
        tape.record(
            c_id,
            vec![a_id, b_id],
            Box::new(move |g| vec![g * &b_c, g * &a_c]),
        );
        let n = 3;
        tape.record(
            s_id,
            vec![c_id],
            Box::new(move |_g| vec![ArrayD::ones(vec![n])]),
        );
        let grads = tape.backward(s_id, &[]).unwrap();
        let analytical = grads.get(&a_id).unwrap();

        // Numerical
        let b_clone = b.clone();
        let numerical = numerical_grad(&a, &|x| (x * &b_clone).sum());

        for (an, nu) in analytical.iter().zip(numerical.iter()) {
            assert!((an - nu).abs() < 1e-4, "analytical={an}, numerical={nu}");
        }
    }

    #[test]
    fn numcheck_relu_gradient() {
        // f(x) = sum(relu(x))
        let x = ArrayD::from_shape_vec(vec![4], vec![-2.0, -0.5, 0.5, 3.0]).unwrap();

        // Analytical
        let mut tape = Tape::new();
        let x_id = tape.fresh_id();
        let r_id = tape.fresh_id();
        let s_id = tape.fresh_id();
        let x_c = x.clone();
        tape.record(
            r_id,
            vec![x_id],
            Box::new(move |g| {
                let mask = x_c.mapv(|v| if v > 0.0 { 1.0 } else { 0.0 });
                vec![g * &mask]
            }),
        );
        let n = 4;
        tape.record(
            s_id,
            vec![r_id],
            Box::new(move |_g| vec![ArrayD::ones(vec![n])]),
        );
        let grads = tape.backward(s_id, &[]).unwrap();
        let analytical = grads.get(&x_id).unwrap();

        // Numerical
        let numerical = numerical_grad(&x, &|v| v.mapv(|e| if e > 0.0 { e } else { 0.0 }).sum());

        for (an, nu) in analytical.iter().zip(numerical.iter()) {
            assert!((an - nu).abs() < 1e-4, "analytical={an}, numerical={nu}");
        }
    }

    #[test]
    fn numcheck_sigmoid_gradient() {
        // f(x) = sum(sigmoid(x))
        let x = ArrayD::from_shape_vec(vec![3], vec![-1.0, 0.0, 1.0]).unwrap();
        let sigmoid = |v: f64| 1.0 / (1.0 + (-v).exp());

        // Analytical: sigmoid'(x) = sigmoid(x) * (1 - sigmoid(x))
        let mut tape = Tape::new();
        let x_id = tape.fresh_id();
        let sig_id = tape.fresh_id();
        let s_id = tape.fresh_id();
        let x_c = x.clone();
        tape.record(
            sig_id,
            vec![x_id],
            Box::new(move |g| {
                let s = x_c.mapv(|v: f64| 1.0 / (1.0 + (-v).exp()));
                let ds = &s * &s.mapv(|v: f64| 1.0 - v);
                vec![g * &ds]
            }),
        );
        let n = 3;
        tape.record(
            s_id,
            vec![sig_id],
            Box::new(move |_g| vec![ArrayD::ones(vec![n])]),
        );
        let grads = tape.backward(s_id, &[]).unwrap();
        let analytical = grads.get(&x_id).unwrap();

        // Numerical
        let numerical = numerical_grad(&x, &|v| v.mapv(sigmoid).sum());

        for (an, nu) in analytical.iter().zip(numerical.iter()) {
            assert!((an - nu).abs() < 1e-4, "analytical={an}, numerical={nu}");
        }
    }

    #[test]
    fn numcheck_tanh_gradient() {
        // f(x) = sum(tanh(x)), tanh'(x) = 1 - tanh(x)^2
        let x = ArrayD::from_shape_vec(vec![3], vec![-1.0, 0.0, 1.0]).unwrap();

        // Analytical
        let mut tape = Tape::new();
        let x_id = tape.fresh_id();
        let t_id = tape.fresh_id();
        let s_id = tape.fresh_id();
        let x_c = x.clone();
        tape.record(
            t_id,
            vec![x_id],
            Box::new(move |g| {
                let t = x_c.mapv(|v: f64| v.tanh());
                let dt = t.mapv(|v: f64| 1.0 - v * v);
                vec![g * &dt]
            }),
        );
        let n = 3;
        tape.record(
            s_id,
            vec![t_id],
            Box::new(move |_g| vec![ArrayD::ones(vec![n])]),
        );
        let grads = tape.backward(s_id, &[]).unwrap();
        let analytical = grads.get(&x_id).unwrap();

        // Numerical
        let numerical = numerical_grad(&x, &|v| v.mapv(|e| e.tanh()).sum());

        for (an, nu) in analytical.iter().zip(numerical.iter()) {
            assert!((an - nu).abs() < 1e-4, "analytical={an}, numerical={nu}");
        }
    }
}
