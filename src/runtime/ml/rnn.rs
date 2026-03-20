//! Recurrent neural network layers — LSTM and GRU cells.
//!
//! Implements Long Short-Term Memory (LSTM) and Gated Recurrent Unit (GRU)
//! cells with forward and backward (BPTT) support.

use ndarray::{Array2, Axis, s};

use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Activation helpers
// ═══════════════════════════════════════════════════════════════════════

/// Element-wise sigmoid: 1 / (1 + exp(-x)).
fn sigmoid(x: &Array2<f64>) -> Array2<f64> {
    x.mapv(|v| 1.0 / (1.0 + (-v).exp()))
}

/// Element-wise sigmoid derivative: s * (1 - s).
fn sigmoid_deriv(s: &Array2<f64>) -> Array2<f64> {
    s * &(1.0 - s)
}

/// Element-wise tanh derivative: 1 - t^2.
fn tanh_deriv(t: &Array2<f64>) -> Array2<f64> {
    1.0 - &(t * t)
}

// ═══════════════════════════════════════════════════════════════════════
// LSTM Cell
// ═══════════════════════════════════════════════════════════════════════

/// Long Short-Term Memory (LSTM) cell.
///
/// Weight layout (concatenated gates: forget, input, output, candidate):
/// - `w_ih`: shape `[4*hidden_size, input_size]` — input-to-hidden weights
/// - `w_hh`: shape `[4*hidden_size, hidden_size]` — hidden-to-hidden weights
/// - `b_ih`: shape `[1, 4*hidden_size]` — input-to-hidden bias
/// - `b_hh`: shape `[1, 4*hidden_size]` — hidden-to-hidden bias
///
/// Gate order: forget, input, output, candidate (cell gate).
#[derive(Debug, Clone)]
pub struct LSTMCell {
    /// Input-to-hidden weights `[4H, input_size]`.
    pub w_ih: TensorValue,
    /// Hidden-to-hidden weights `[4H, hidden_size]`.
    pub w_hh: TensorValue,
    /// Input-to-hidden bias `[1, 4H]`.
    pub b_ih: TensorValue,
    /// Hidden-to-hidden bias `[1, 4H]`.
    pub b_hh: TensorValue,
    /// Input feature size.
    pub input_size: usize,
    /// Hidden state size.
    pub hidden_size: usize,
}

impl LSTMCell {
    /// Creates a new LSTM cell with Xavier-initialized weights and zero biases.
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        let gate_size = 4 * hidden_size;
        let scale_ih = (2.0 / (input_size + hidden_size) as f64).sqrt();
        let scale_hh = (2.0 / (hidden_size + hidden_size) as f64).sqrt();

        let mut w_ih = TensorValue::randn(&[gate_size, input_size]);
        w_ih.set_requires_grad(true);
        *w_ih.data_mut() *= scale_ih;

        let mut w_hh = TensorValue::randn(&[gate_size, hidden_size]);
        w_hh.set_requires_grad(true);
        *w_hh.data_mut() *= scale_hh;

        let mut b_ih = TensorValue::zeros(&[1, gate_size]);
        b_ih.set_requires_grad(true);

        let mut b_hh = TensorValue::zeros(&[1, gate_size]);
        b_hh.set_requires_grad(true);

        Self {
            w_ih,
            w_hh,
            b_ih,
            b_hh,
            input_size,
            hidden_size,
        }
    }

    /// Single-step forward pass.
    ///
    /// - `x`: input tensor `[batch, input_size]`
    /// - `h_prev`: previous hidden state `[batch, hidden_size]`
    /// - `c_prev`: previous cell state `[batch, hidden_size]`
    ///
    /// Returns `(h_t, c_t)` — new hidden state and cell state.
    pub fn forward_step(
        &self,
        x: &TensorValue,
        h_prev: &TensorValue,
        c_prev: &TensorValue,
    ) -> Result<(TensorValue, TensorValue), TensorError> {
        let x2 = to_array2(x)?;
        let h2 = to_array2(h_prev)?;
        let c2 = to_array2(c_prev)?;
        let w_ih2 = to_array2(&self.w_ih)?;
        let w_hh2 = to_array2(&self.w_hh)?;

        let b_ih_flat = self.b_ih.to_vec();
        let b_hh_flat = self.b_hh.to_vec();

        let (h_new, c_new) =
            lstm_forward_step(&x2, &h2, &c2, &w_ih2, &w_hh2, &b_ih_flat, &b_hh_flat);

        let batch = x.shape()[0];
        let h_out = TensorValue::from_data(
            h_new.into_raw_vec_and_offset().0,
            &[batch, self.hidden_size],
        )?;
        let c_out = TensorValue::from_data(
            c_new.into_raw_vec_and_offset().0,
            &[batch, self.hidden_size],
        )?;
        Ok((h_out, c_out))
    }

    /// Sequence forward: processes `[batch, seq_len, input_size]` input.
    ///
    /// Returns `(hidden_states, (h_final, c_final))`:
    /// - `hidden_states`: `[batch, seq_len, hidden_size]` — all hidden states
    /// - `h_final`: `[batch, hidden_size]` — last hidden state
    /// - `c_final`: `[batch, hidden_size]` — last cell state
    pub fn forward_sequence(
        &self,
        x: &TensorValue,
        h0: Option<&TensorValue>,
        c0: Option<&TensorValue>,
    ) -> Result<(TensorValue, TensorValue, TensorValue), TensorError> {
        if x.ndim() != 3 {
            return Err(TensorError::RankMismatch {
                expected: 3,
                got: x.ndim(),
            });
        }
        let shape = x.shape();
        let batch = shape[0];
        let seq_len = shape[1];

        let mut h = match h0 {
            Some(h) => h.clone(),
            None => TensorValue::zeros(&[batch, self.hidden_size]),
        };
        let mut c = match c0 {
            Some(c) => c.clone(),
            None => TensorValue::zeros(&[batch, self.hidden_size]),
        };

        let mut all_hidden = Vec::with_capacity(seq_len * batch * self.hidden_size);

        let x_data = x.data();
        for t in 0..seq_len {
            // Extract time step t: x[:, t, :]
            let mut step_data = vec![0.0; batch * self.input_size];
            for b in 0..batch {
                for f in 0..self.input_size {
                    step_data[b * self.input_size + f] = x_data[[b, t, f]];
                }
            }
            let x_t = TensorValue::from_data(step_data, &[batch, self.input_size])?;

            let (h_new, c_new) = self.forward_step(&x_t, &h, &c)?;
            all_hidden.extend(h_new.to_vec());
            h = h_new;
            c = c_new;
        }

        let hidden_states =
            TensorValue::from_data(all_hidden, &[batch, seq_len, self.hidden_size])?;
        Ok((hidden_states, h, c))
    }

    /// Backward pass through time (BPTT) for LSTM.
    ///
    /// Given the gradient of the loss w.r.t. the final hidden state,
    /// computes gradients for all weights and biases.
    ///
    /// - `inputs`: list of `[batch, input_size]` tensors, one per timestep
    /// - `hidden_states`: list of `(h_t, c_t)` at each timestep
    /// - `d_h_final`: gradient of loss w.r.t. final hidden state `[batch, hidden_size]`
    ///
    /// Returns `(d_w_ih, d_w_hh, d_b_ih, d_b_hh)`.
    pub fn backward(
        &self,
        inputs: &[TensorValue],
        hidden_states: &[(Array2<f64>, Array2<f64>)],
        gate_caches: &[LSTMGateCache],
        d_h_final: &Array2<f64>,
    ) -> Result<LSTMGradients, TensorError> {
        let gate_size = 4 * self.hidden_size;
        let batch = d_h_final.shape()[0];

        let mut d_w_ih = Array2::<f64>::zeros((gate_size, self.input_size));
        let mut d_w_hh = Array2::<f64>::zeros((gate_size, self.hidden_size));
        let mut d_b_ih = Array2::<f64>::zeros((1, gate_size));
        let mut d_b_hh = Array2::<f64>::zeros((1, gate_size));

        let mut d_h = d_h_final.clone();
        let mut d_c = Array2::<f64>::zeros((batch, self.hidden_size));

        let seq_len = inputs.len();

        for t in (0..seq_len).rev() {
            let cache = &gate_caches[t];
            let c_prev = if t > 0 {
                &hidden_states[t - 1].1
            } else {
                &Array2::<f64>::zeros((batch, self.hidden_size))
            };

            // d_c += d_h * o_t * tanh'(c_t)
            let tanh_c = cache.c_t.mapv(f64::tanh);
            d_c = &d_c + &(&d_h * &cache.o_t * &tanh_deriv(&tanh_c));

            // Gate gradients
            let d_f = &d_c * c_prev * &sigmoid_deriv(&cache.f_t);
            let d_i = &d_c * &cache.g_t * &sigmoid_deriv(&cache.i_t);
            let d_o = &d_h * &tanh_c * &sigmoid_deriv(&cache.o_t);
            let d_g = &d_c * &cache.i_t * &tanh_deriv(&cache.g_t);

            // Concatenate gate grads: [batch, 4H]
            let mut d_gates = Array2::<f64>::zeros((batch, gate_size));
            let h = self.hidden_size;
            d_gates.slice_mut(s![.., 0..h]).assign(&d_f);
            d_gates.slice_mut(s![.., h..2 * h]).assign(&d_i);
            d_gates.slice_mut(s![.., 2 * h..3 * h]).assign(&d_o);
            d_gates.slice_mut(s![.., 3 * h..4 * h]).assign(&d_g);

            // Weight gradients
            let x_t = to_array2(&inputs[t])?;
            let h_prev = if t > 0 {
                hidden_states[t - 1].0.clone()
            } else {
                Array2::<f64>::zeros((batch, self.hidden_size))
            };

            d_w_ih = d_w_ih + d_gates.t().dot(&x_t);
            d_w_hh = d_w_hh + d_gates.t().dot(&h_prev);

            // Bias gradients: sum over batch
            d_b_ih = d_b_ih + d_gates.sum_axis(Axis(0)).insert_axis(Axis(0));
            d_b_hh = d_b_hh + d_gates.sum_axis(Axis(0)).insert_axis(Axis(0));

            // Propagate to previous timestep
            let w_hh2 = to_array2(&self.w_hh)?;
            d_h = d_gates.dot(&w_hh2);
            d_c = &d_c * &cache.f_t;
        }

        Ok(LSTMGradients {
            d_w_ih,
            d_w_hh,
            d_b_ih,
            d_b_hh,
        })
    }

    /// Computes gate caches for BPTT.
    ///
    /// Runs forward and stores intermediate gate values for backward pass.
    #[allow(clippy::type_complexity)]
    pub fn forward_with_cache(
        &self,
        inputs: &[TensorValue],
        h0: &Array2<f64>,
        c0: &Array2<f64>,
    ) -> Result<(Vec<(Array2<f64>, Array2<f64>)>, Vec<LSTMGateCache>), TensorError> {
        let w_ih2 = to_array2(&self.w_ih)?;
        let w_hh2 = to_array2(&self.w_hh)?;
        let b_ih_flat = self.b_ih.to_vec();
        let b_hh_flat = self.b_hh.to_vec();

        let mut h = h0.clone();
        let mut c = c0.clone();
        let mut states = Vec::with_capacity(inputs.len());
        let mut caches = Vec::with_capacity(inputs.len());

        for input in inputs {
            let x2 = to_array2(input)?;
            let (h_new, c_new, cache) =
                lstm_forward_step_with_cache(&x2, &h, &c, &w_ih2, &w_hh2, &b_ih_flat, &b_hh_flat);
            states.push((h_new.clone(), c_new.clone()));
            caches.push(cache);
            h = h_new;
            c = c_new;
        }

        Ok((states, caches))
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.w_ih, &self.w_hh, &self.b_ih, &self.b_hh]
    }

    /// Returns mutable references to all learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![
            &mut self.w_ih,
            &mut self.w_hh,
            &mut self.b_ih,
            &mut self.b_hh,
        ]
    }

    /// Returns the total number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.w_ih.numel() + self.w_hh.numel() + self.b_ih.numel() + self.b_hh.numel()
    }
}

/// Cached gate values from LSTM forward pass for backward computation.
#[derive(Debug, Clone)]
pub struct LSTMGateCache {
    /// Forget gate output.
    pub f_t: Array2<f64>,
    /// Input gate output.
    pub i_t: Array2<f64>,
    /// Output gate output.
    pub o_t: Array2<f64>,
    /// Candidate cell state (g_t / c~_t).
    pub g_t: Array2<f64>,
    /// Cell state at this timestep.
    pub c_t: Array2<f64>,
}

/// Gradients for LSTM parameters.
#[derive(Debug, Clone)]
pub struct LSTMGradients {
    /// Gradient for input-to-hidden weights.
    pub d_w_ih: Array2<f64>,
    /// Gradient for hidden-to-hidden weights.
    pub d_w_hh: Array2<f64>,
    /// Gradient for input-to-hidden bias.
    pub d_b_ih: Array2<f64>,
    /// Gradient for hidden-to-hidden bias.
    pub d_b_hh: Array2<f64>,
}

/// LSTM forward step (internal): computes one timestep.
fn lstm_forward_step(
    x: &Array2<f64>,
    h_prev: &Array2<f64>,
    c_prev: &Array2<f64>,
    w_ih: &Array2<f64>,
    w_hh: &Array2<f64>,
    b_ih: &[f64],
    b_hh: &[f64],
) -> (Array2<f64>, Array2<f64>) {
    let hidden_size = h_prev.shape()[1];

    // gates = x @ W_ih^T + h_prev @ W_hh^T + b_ih + b_hh
    let mut gates = x.dot(&w_ih.t()) + h_prev.dot(&w_hh.t());
    let batch = gates.shape()[0];
    let gate_size = gates.shape()[1];
    for b in 0..batch {
        for j in 0..gate_size {
            gates[[b, j]] += b_ih[j] + b_hh[j];
        }
    }

    // Split into 4 gates
    let f_t = sigmoid(&gates.slice(s![.., 0..hidden_size]).to_owned());
    let i_t = sigmoid(&gates.slice(s![.., hidden_size..2 * hidden_size]).to_owned());
    let o_t = sigmoid(
        &gates
            .slice(s![.., 2 * hidden_size..3 * hidden_size])
            .to_owned(),
    );
    let g_t = gates
        .slice(s![.., 3 * hidden_size..4 * hidden_size])
        .mapv(f64::tanh);

    // Cell state: c_t = f_t * c_prev + i_t * g_t
    let c_t = &f_t * c_prev + &i_t * &g_t;

    // Hidden state: h_t = o_t * tanh(c_t)
    let h_t = &o_t * &c_t.mapv(f64::tanh);

    (h_t, c_t)
}

/// LSTM forward step with cache for backward pass.
fn lstm_forward_step_with_cache(
    x: &Array2<f64>,
    h_prev: &Array2<f64>,
    c_prev: &Array2<f64>,
    w_ih: &Array2<f64>,
    w_hh: &Array2<f64>,
    b_ih: &[f64],
    b_hh: &[f64],
) -> (Array2<f64>, Array2<f64>, LSTMGateCache) {
    let hidden_size = h_prev.shape()[1];

    let mut gates = x.dot(&w_ih.t()) + h_prev.dot(&w_hh.t());
    let batch = gates.shape()[0];
    let gate_size = gates.shape()[1];
    for b in 0..batch {
        for j in 0..gate_size {
            gates[[b, j]] += b_ih[j] + b_hh[j];
        }
    }

    let f_t = sigmoid(&gates.slice(s![.., 0..hidden_size]).to_owned());
    let i_t = sigmoid(&gates.slice(s![.., hidden_size..2 * hidden_size]).to_owned());
    let o_t = sigmoid(
        &gates
            .slice(s![.., 2 * hidden_size..3 * hidden_size])
            .to_owned(),
    );
    let g_t = gates
        .slice(s![.., 3 * hidden_size..4 * hidden_size])
        .mapv(f64::tanh);

    let c_t = &f_t * c_prev + &i_t * &g_t;
    let h_t = &o_t * &c_t.mapv(f64::tanh);

    let cache = LSTMGateCache {
        f_t,
        i_t,
        o_t,
        g_t,
        c_t: c_t.clone(),
    };

    (h_t, c_t, cache)
}

// ═══════════════════════════════════════════════════════════════════════
// GRU Cell
// ═══════════════════════════════════════════════════════════════════════

/// Gated Recurrent Unit (GRU) cell.
///
/// Weight layout (concatenated gates: reset, update, candidate):
/// - `w_ih`: shape `[3*hidden_size, input_size]` — input-to-hidden weights
/// - `w_hh`: shape `[3*hidden_size, hidden_size]` — hidden-to-hidden weights
/// - `b_ih`: shape `[1, 3*hidden_size]` — input-to-hidden bias
/// - `b_hh`: shape `[1, 3*hidden_size]` — hidden-to-hidden bias
///
/// Gate order: reset, update, candidate (new).
#[derive(Debug, Clone)]
pub struct GRUCell {
    /// Input-to-hidden weights `[3H, input_size]`.
    pub w_ih: TensorValue,
    /// Hidden-to-hidden weights `[3H, hidden_size]`.
    pub w_hh: TensorValue,
    /// Input-to-hidden bias `[1, 3H]`.
    pub b_ih: TensorValue,
    /// Hidden-to-hidden bias `[1, 3H]`.
    pub b_hh: TensorValue,
    /// Input feature size.
    pub input_size: usize,
    /// Hidden state size.
    pub hidden_size: usize,
}

impl GRUCell {
    /// Creates a new GRU cell with Xavier-initialized weights and zero biases.
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        let gate_size = 3 * hidden_size;
        let scale_ih = (2.0 / (input_size + hidden_size) as f64).sqrt();
        let scale_hh = (2.0 / (hidden_size + hidden_size) as f64).sqrt();

        let mut w_ih = TensorValue::randn(&[gate_size, input_size]);
        w_ih.set_requires_grad(true);
        *w_ih.data_mut() *= scale_ih;

        let mut w_hh = TensorValue::randn(&[gate_size, hidden_size]);
        w_hh.set_requires_grad(true);
        *w_hh.data_mut() *= scale_hh;

        let mut b_ih = TensorValue::zeros(&[1, gate_size]);
        b_ih.set_requires_grad(true);

        let mut b_hh = TensorValue::zeros(&[1, gate_size]);
        b_hh.set_requires_grad(true);

        Self {
            w_ih,
            w_hh,
            b_ih,
            b_hh,
            input_size,
            hidden_size,
        }
    }

    /// Single-step forward pass.
    ///
    /// - `x`: input tensor `[batch, input_size]`
    /// - `h_prev`: previous hidden state `[batch, hidden_size]`
    ///
    /// Returns `h_t` — new hidden state.
    pub fn forward_step(
        &self,
        x: &TensorValue,
        h_prev: &TensorValue,
    ) -> Result<TensorValue, TensorError> {
        let x2 = to_array2(x)?;
        let h2 = to_array2(h_prev)?;
        let w_ih2 = to_array2(&self.w_ih)?;
        let w_hh2 = to_array2(&self.w_hh)?;

        let b_ih_flat = self.b_ih.to_vec();
        let b_hh_flat = self.b_hh.to_vec();

        let h_new = gru_forward_step(&x2, &h2, &w_ih2, &w_hh2, &b_ih_flat, &b_hh_flat);

        let batch = x.shape()[0];
        let h_out = TensorValue::from_data(
            h_new.into_raw_vec_and_offset().0,
            &[batch, self.hidden_size],
        )?;
        Ok(h_out)
    }

    /// Sequence forward: processes `[batch, seq_len, input_size]` input.
    ///
    /// Returns `(hidden_states, h_final)`:
    /// - `hidden_states`: `[batch, seq_len, hidden_size]`
    /// - `h_final`: `[batch, hidden_size]`
    pub fn forward_sequence(
        &self,
        x: &TensorValue,
        h0: Option<&TensorValue>,
    ) -> Result<(TensorValue, TensorValue), TensorError> {
        if x.ndim() != 3 {
            return Err(TensorError::RankMismatch {
                expected: 3,
                got: x.ndim(),
            });
        }
        let shape = x.shape();
        let batch = shape[0];
        let seq_len = shape[1];

        let mut h = match h0 {
            Some(h) => h.clone(),
            None => TensorValue::zeros(&[batch, self.hidden_size]),
        };

        let mut all_hidden = Vec::with_capacity(seq_len * batch * self.hidden_size);

        let x_data = x.data();
        for t in 0..seq_len {
            let mut step_data = vec![0.0; batch * self.input_size];
            for b in 0..batch {
                for f in 0..self.input_size {
                    step_data[b * self.input_size + f] = x_data[[b, t, f]];
                }
            }
            let x_t = TensorValue::from_data(step_data, &[batch, self.input_size])?;

            let h_new = self.forward_step(&x_t, &h)?;
            all_hidden.extend(h_new.to_vec());
            h = h_new;
        }

        let hidden_states =
            TensorValue::from_data(all_hidden, &[batch, seq_len, self.hidden_size])?;
        Ok((hidden_states, h))
    }

    /// Backward pass through time (BPTT) for GRU.
    ///
    /// - `inputs`: list of `[batch, input_size]` tensors, one per timestep
    /// - `hidden_states`: list of `h_t` at each timestep
    /// - `gate_caches`: cached gate values from forward pass
    /// - `d_h_final`: gradient of loss w.r.t. final hidden state `[batch, hidden_size]`
    ///
    /// Returns `GRUGradients`.
    pub fn backward(
        &self,
        inputs: &[TensorValue],
        hidden_states: &[Array2<f64>],
        gate_caches: &[GRUGateCache],
        d_h_final: &Array2<f64>,
    ) -> Result<GRUGradients, TensorError> {
        let gate_size = 3 * self.hidden_size;
        let batch = d_h_final.shape()[0];

        let mut d_w_ih = Array2::<f64>::zeros((gate_size, self.input_size));
        let mut d_w_hh = Array2::<f64>::zeros((gate_size, self.hidden_size));
        let mut d_b_ih = Array2::<f64>::zeros((1, gate_size));
        let mut d_b_hh = Array2::<f64>::zeros((1, gate_size));

        let mut d_h = d_h_final.clone();
        let seq_len = inputs.len();
        let h = self.hidden_size;

        for t in (0..seq_len).rev() {
            let cache = &gate_caches[t];
            let h_prev = if t > 0 {
                &hidden_states[t - 1]
            } else {
                &Array2::<f64>::zeros((batch, self.hidden_size))
            };

            // d_n = d_h * (1 - z_t) * tanh'(n_t)
            let d_n = &d_h * &(1.0 - &cache.z_t) * &tanh_deriv(&cache.n_t);

            // d_z = d_h * (h_prev - n_t) * sigmoid'(z_t)
            let d_z = &d_h * &(h_prev - &cache.n_t) * &sigmoid_deriv(&cache.z_t);

            // d_r: n_t depends on r_t * h_prev via w_hh candidate part
            // For simplicity, compute via the full gate gradient
            let w_hh2 = to_array2(&self.w_hh)?;
            let w_hh_n = w_hh2.slice(s![2 * h..3 * h, ..]).to_owned();
            let d_rh = d_n.dot(&w_hh_n); // [batch, hidden_size]
            let d_r = &d_rh * h_prev * &sigmoid_deriv(&cache.r_t);

            // Concatenate gate grads: [batch, 3H]
            let mut d_gates = Array2::<f64>::zeros((batch, gate_size));
            d_gates.slice_mut(s![.., 0..h]).assign(&d_r);
            d_gates.slice_mut(s![.., h..2 * h]).assign(&d_z);
            d_gates.slice_mut(s![.., 2 * h..3 * h]).assign(&d_n);

            let x_t = to_array2(&inputs[t])?;
            d_w_ih = d_w_ih + d_gates.t().dot(&x_t);
            d_w_hh = d_w_hh + d_gates.t().dot(h_prev);

            d_b_ih = d_b_ih + d_gates.sum_axis(Axis(0)).insert_axis(Axis(0));
            d_b_hh = d_b_hh + d_gates.sum_axis(Axis(0)).insert_axis(Axis(0));

            // Propagate gradient to previous timestep
            d_h = d_gates.dot(&to_array2(&self.w_hh)?) + &d_h * &cache.z_t;
        }

        Ok(GRUGradients {
            d_w_ih,
            d_w_hh,
            d_b_ih,
            d_b_hh,
        })
    }

    /// Computes gate caches for BPTT.
    pub fn forward_with_cache(
        &self,
        inputs: &[TensorValue],
        h0: &Array2<f64>,
    ) -> Result<(Vec<Array2<f64>>, Vec<GRUGateCache>), TensorError> {
        let w_ih2 = to_array2(&self.w_ih)?;
        let w_hh2 = to_array2(&self.w_hh)?;
        let b_ih_flat = self.b_ih.to_vec();
        let b_hh_flat = self.b_hh.to_vec();

        let mut h = h0.clone();
        let mut states = Vec::with_capacity(inputs.len());
        let mut caches = Vec::with_capacity(inputs.len());

        for input in inputs {
            let x2 = to_array2(input)?;
            let (h_new, cache) =
                gru_forward_step_with_cache(&x2, &h, &w_ih2, &w_hh2, &b_ih_flat, &b_hh_flat);
            states.push(h_new.clone());
            caches.push(cache);
            h = h_new;
        }

        Ok((states, caches))
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.w_ih, &self.w_hh, &self.b_ih, &self.b_hh]
    }

    /// Returns mutable references to all learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![
            &mut self.w_ih,
            &mut self.w_hh,
            &mut self.b_ih,
            &mut self.b_hh,
        ]
    }

    /// Returns the total number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.w_ih.numel() + self.w_hh.numel() + self.b_ih.numel() + self.b_hh.numel()
    }
}

/// Cached gate values from GRU forward pass for backward computation.
#[derive(Debug, Clone)]
pub struct GRUGateCache {
    /// Reset gate output.
    pub r_t: Array2<f64>,
    /// Update gate output.
    pub z_t: Array2<f64>,
    /// Candidate hidden state (n_t / h~_t).
    pub n_t: Array2<f64>,
}

/// Gradients for GRU parameters.
#[derive(Debug, Clone)]
pub struct GRUGradients {
    /// Gradient for input-to-hidden weights.
    pub d_w_ih: Array2<f64>,
    /// Gradient for hidden-to-hidden weights.
    pub d_w_hh: Array2<f64>,
    /// Gradient for input-to-hidden bias.
    pub d_b_ih: Array2<f64>,
    /// Gradient for hidden-to-hidden bias.
    pub d_b_hh: Array2<f64>,
}

/// GRU forward step (internal): computes one timestep.
fn gru_forward_step(
    x: &Array2<f64>,
    h_prev: &Array2<f64>,
    w_ih: &Array2<f64>,
    w_hh: &Array2<f64>,
    b_ih: &[f64],
    b_hh: &[f64],
) -> Array2<f64> {
    let hidden_size = h_prev.shape()[1];

    let mut ih = x.dot(&w_ih.t());
    let mut hh = h_prev.dot(&w_hh.t());
    let batch = ih.shape()[0];
    let gate_size = ih.shape()[1];
    for b in 0..batch {
        for j in 0..gate_size {
            ih[[b, j]] += b_ih[j];
            hh[[b, j]] += b_hh[j];
        }
    }

    // Reset and update gates use combined ih + hh
    let r_t = sigmoid(
        &(&ih.slice(s![.., 0..hidden_size]).to_owned()
            + &hh.slice(s![.., 0..hidden_size]).to_owned()),
    );
    let z_t = sigmoid(
        &(&ih.slice(s![.., hidden_size..2 * hidden_size]).to_owned()
            + &hh.slice(s![.., hidden_size..2 * hidden_size]).to_owned()),
    );

    // Candidate: n_t = tanh(ih_n + r_t * hh_n)
    let ih_n = ih
        .slice(s![.., 2 * hidden_size..3 * hidden_size])
        .to_owned();
    let hh_n = hh
        .slice(s![.., 2 * hidden_size..3 * hidden_size])
        .to_owned();
    let n_t = (&ih_n + &(&r_t * &hh_n)).mapv(f64::tanh);

    // h_t = (1 - z_t) * n_t + z_t * h_prev
    &(1.0 - &z_t) * &n_t + &z_t * h_prev
}

/// GRU forward step with cache for backward pass.
fn gru_forward_step_with_cache(
    x: &Array2<f64>,
    h_prev: &Array2<f64>,
    w_ih: &Array2<f64>,
    w_hh: &Array2<f64>,
    b_ih: &[f64],
    b_hh: &[f64],
) -> (Array2<f64>, GRUGateCache) {
    let hidden_size = h_prev.shape()[1];

    let mut ih = x.dot(&w_ih.t());
    let mut hh = h_prev.dot(&w_hh.t());
    let batch = ih.shape()[0];
    let gate_size = ih.shape()[1];
    for b in 0..batch {
        for j in 0..gate_size {
            ih[[b, j]] += b_ih[j];
            hh[[b, j]] += b_hh[j];
        }
    }

    let r_t = sigmoid(
        &(&ih.slice(s![.., 0..hidden_size]).to_owned()
            + &hh.slice(s![.., 0..hidden_size]).to_owned()),
    );
    let z_t = sigmoid(
        &(&ih.slice(s![.., hidden_size..2 * hidden_size]).to_owned()
            + &hh.slice(s![.., hidden_size..2 * hidden_size]).to_owned()),
    );

    let ih_n = ih
        .slice(s![.., 2 * hidden_size..3 * hidden_size])
        .to_owned();
    let hh_n = hh
        .slice(s![.., 2 * hidden_size..3 * hidden_size])
        .to_owned();
    let n_t = (&ih_n + &(&r_t * &hh_n)).mapv(f64::tanh);

    let h_t = &(1.0 - &z_t) * &n_t + &z_t * h_prev;

    let cache = GRUGateCache { r_t, z_t, n_t };

    (h_t, cache)
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Converts a TensorValue to a 2D ndarray. Returns error if rank != 2.
fn to_array2(t: &TensorValue) -> Result<Array2<f64>, TensorError> {
    if t.ndim() != 2 {
        return Err(TensorError::RankMismatch {
            expected: 2,
            got: t.ndim(),
        });
    }
    let shape = t.shape();
    let data = t.to_vec();
    Array2::from_shape_vec((shape[0], shape[1]), data).map_err(|e| TensorError::InvalidData {
        reason: e.to_string(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── LSTM Tests ──

    #[test]
    fn lstm_cell_creation_correct_shapes() {
        let lstm = LSTMCell::new(10, 20);
        assert_eq!(lstm.w_ih.shape(), &[80, 10]); // 4*20 x 10
        assert_eq!(lstm.w_hh.shape(), &[80, 20]); // 4*20 x 20
        assert_eq!(lstm.b_ih.shape(), &[1, 80]);
        assert_eq!(lstm.b_hh.shape(), &[1, 80]);
        assert_eq!(lstm.input_size, 10);
        assert_eq!(lstm.hidden_size, 20);
    }

    #[test]
    fn lstm_forward_step_produces_correct_shape() {
        let lstm = LSTMCell::new(4, 8);
        let x = TensorValue::randn(&[2, 4]); // batch=2, input=4
        let h0 = TensorValue::zeros(&[2, 8]); // batch=2, hidden=8
        let c0 = TensorValue::zeros(&[2, 8]);

        let (h_t, c_t) = lstm.forward_step(&x, &h0, &c0).unwrap();
        assert_eq!(h_t.shape(), &[2, 8]);
        assert_eq!(c_t.shape(), &[2, 8]);
    }

    #[test]
    fn lstm_hidden_state_bounded_by_tanh() {
        let lstm = LSTMCell::new(3, 5);
        let x = TensorValue::randn(&[1, 3]);
        let h0 = TensorValue::zeros(&[1, 5]);
        let c0 = TensorValue::zeros(&[1, 5]);

        let (h_t, _c_t) = lstm.forward_step(&x, &h0, &c0).unwrap();
        // h_t = o_t * tanh(c_t), tanh bounds to [-1, 1], o_t in [0, 1]
        for &val in h_t.to_vec().iter() {
            assert!(val >= -1.0 && val <= 1.0, "h_t value {val} out of [-1, 1]");
        }
    }

    #[test]
    fn lstm_sequence_forward_correct_shapes() {
        let lstm = LSTMCell::new(4, 8);
        let x = TensorValue::randn(&[2, 5, 4]); // batch=2, seq=5, input=4

        let (hidden_states, h_final, c_final) = lstm.forward_sequence(&x, None, None).unwrap();
        assert_eq!(hidden_states.shape(), &[2, 5, 8]); // batch, seq, hidden
        assert_eq!(h_final.shape(), &[2, 8]);
        assert_eq!(c_final.shape(), &[2, 8]);
    }

    #[test]
    fn lstm_sequence_with_initial_state() {
        let lstm = LSTMCell::new(3, 4);
        let x = TensorValue::randn(&[1, 3, 3]); // batch=1, seq=3, input=3
        let h0 = TensorValue::from_data(vec![0.5; 4], &[1, 4]).unwrap();
        let c0 = TensorValue::from_data(vec![0.1; 4], &[1, 4]).unwrap();

        let result = lstm.forward_sequence(&x, Some(&h0), Some(&c0));
        assert!(result.is_ok());
        let (hs, hf, cf) = result.unwrap();
        assert_eq!(hs.shape(), &[1, 3, 4]);
        assert_eq!(hf.shape(), &[1, 4]);
        assert_eq!(cf.shape(), &[1, 4]);
    }

    #[test]
    fn lstm_sequence_rejects_wrong_rank() {
        let lstm = LSTMCell::new(4, 8);
        let x = TensorValue::randn(&[2, 4]); // 2D, not 3D

        let result = lstm.forward_sequence(&x, None, None);
        assert!(matches!(result, Err(TensorError::RankMismatch { .. })));
    }

    #[test]
    fn lstm_param_count_correct() {
        let lstm = LSTMCell::new(10, 20);
        // w_ih: 80*10=800, w_hh: 80*20=1600, b_ih: 80, b_hh: 80
        assert_eq!(lstm.param_count(), 800 + 1600 + 80 + 80);
    }

    #[test]
    fn lstm_backward_produces_gradients() {
        let lstm = LSTMCell::new(3, 4);
        let batch = 2;
        let seq_len = 3;

        let inputs: Vec<TensorValue> = (0..seq_len)
            .map(|_| TensorValue::randn(&[batch, 3]))
            .collect();
        let h0 = Array2::<f64>::zeros((batch, 4));
        let c0 = Array2::<f64>::zeros((batch, 4));

        let (states, caches) = lstm.forward_with_cache(&inputs, &h0, &c0).unwrap();
        let d_h = Array2::<f64>::ones((batch, 4));

        let grads = lstm.backward(&inputs, &states, &caches, &d_h).unwrap();
        assert_eq!(grads.d_w_ih.shape(), &[16, 3]); // 4*4=16, input=3
        assert_eq!(grads.d_w_hh.shape(), &[16, 4]);
        assert_eq!(grads.d_b_ih.shape(), &[1, 16]);
        assert_eq!(grads.d_b_hh.shape(), &[1, 16]);

        // Gradients should be non-zero
        assert!(grads.d_w_ih.iter().any(|&v| v.abs() > 1e-10));
    }

    #[test]
    fn lstm_parameters_returns_all_four() {
        let lstm = LSTMCell::new(3, 4);
        assert_eq!(lstm.parameters().len(), 4);
    }

    // ── GRU Tests ──

    #[test]
    fn gru_cell_creation_correct_shapes() {
        let gru = GRUCell::new(10, 20);
        assert_eq!(gru.w_ih.shape(), &[60, 10]); // 3*20 x 10
        assert_eq!(gru.w_hh.shape(), &[60, 20]); // 3*20 x 20
        assert_eq!(gru.b_ih.shape(), &[1, 60]);
        assert_eq!(gru.b_hh.shape(), &[1, 60]);
        assert_eq!(gru.input_size, 10);
        assert_eq!(gru.hidden_size, 20);
    }

    #[test]
    fn gru_forward_step_produces_correct_shape() {
        let gru = GRUCell::new(4, 8);
        let x = TensorValue::randn(&[2, 4]); // batch=2, input=4
        let h0 = TensorValue::zeros(&[2, 8]); // batch=2, hidden=8

        let h_t = gru.forward_step(&x, &h0).unwrap();
        assert_eq!(h_t.shape(), &[2, 8]);
    }

    #[test]
    fn gru_hidden_state_bounded() {
        let gru = GRUCell::new(3, 5);
        let x = TensorValue::randn(&[1, 3]);
        let h0 = TensorValue::zeros(&[1, 5]);

        let h_t = gru.forward_step(&x, &h0).unwrap();
        // GRU output: interpolation between h_prev and tanh(candidate)
        // Starting from zero h0, values should be in [-1, 1]
        for &val in h_t.to_vec().iter() {
            assert!(
                val >= -1.0 && val <= 1.0,
                "GRU h_t value {val} out of [-1, 1]"
            );
        }
    }

    #[test]
    fn gru_sequence_forward_correct_shapes() {
        let gru = GRUCell::new(4, 8);
        let x = TensorValue::randn(&[2, 5, 4]); // batch=2, seq=5, input=4

        let (hidden_states, h_final) = gru.forward_sequence(&x, None).unwrap();
        assert_eq!(hidden_states.shape(), &[2, 5, 8]);
        assert_eq!(h_final.shape(), &[2, 8]);
    }

    #[test]
    fn gru_sequence_with_initial_state() {
        let gru = GRUCell::new(3, 4);
        let x = TensorValue::randn(&[1, 3, 3]);
        let h0 = TensorValue::from_data(vec![0.5; 4], &[1, 4]).unwrap();

        let (hs, hf) = gru.forward_sequence(&x, Some(&h0)).unwrap();
        assert_eq!(hs.shape(), &[1, 3, 4]);
        assert_eq!(hf.shape(), &[1, 4]);
    }

    #[test]
    fn gru_sequence_rejects_wrong_rank() {
        let gru = GRUCell::new(4, 8);
        let x = TensorValue::randn(&[2, 4]); // 2D, not 3D

        let result = gru.forward_sequence(&x, None);
        assert!(matches!(result, Err(TensorError::RankMismatch { .. })));
    }

    #[test]
    fn gru_param_count_correct() {
        let gru = GRUCell::new(10, 20);
        // w_ih: 60*10=600, w_hh: 60*20=1200, b_ih: 60, b_hh: 60
        assert_eq!(gru.param_count(), 600 + 1200 + 60 + 60);
    }

    #[test]
    fn gru_backward_produces_gradients() {
        let gru = GRUCell::new(3, 4);
        let batch = 2;
        let seq_len = 3;

        let inputs: Vec<TensorValue> = (0..seq_len)
            .map(|_| TensorValue::randn(&[batch, 3]))
            .collect();
        let h0 = Array2::<f64>::zeros((batch, 4));

        let (states, caches) = gru.forward_with_cache(&inputs, &h0).unwrap();
        let d_h = Array2::<f64>::ones((batch, 4));

        let grads = gru.backward(&inputs, &states, &caches, &d_h).unwrap();
        assert_eq!(grads.d_w_ih.shape(), &[12, 3]); // 3*4=12, input=3
        assert_eq!(grads.d_w_hh.shape(), &[12, 4]);
        assert_eq!(grads.d_b_ih.shape(), &[1, 12]);
        assert_eq!(grads.d_b_hh.shape(), &[1, 12]);

        // Gradients should be non-zero
        assert!(grads.d_w_ih.iter().any(|&v| v.abs() > 1e-10));
    }

    #[test]
    fn gru_parameters_returns_all_four() {
        let gru = GRUCell::new(3, 4);
        assert_eq!(gru.parameters().len(), 4);
    }

    #[test]
    fn gru_multiple_steps_change_hidden() {
        let gru = GRUCell::new(3, 4);
        let x1 = TensorValue::randn(&[1, 3]);
        let h0 = TensorValue::zeros(&[1, 4]);

        let h1 = gru.forward_step(&x1, &h0).unwrap();
        let h2 = gru.forward_step(&x1, &h1).unwrap();

        // h1 and h2 should differ (since hidden state evolves)
        assert_ne!(h1.to_vec(), h2.to_vec());
    }

    #[test]
    fn lstm_multiple_steps_evolve_cell_state() {
        let lstm = LSTMCell::new(3, 4);
        let x1 = TensorValue::randn(&[1, 3]);
        let h0 = TensorValue::zeros(&[1, 4]);
        let c0 = TensorValue::zeros(&[1, 4]);

        let (h1, c1) = lstm.forward_step(&x1, &h0, &c0).unwrap();
        let (h2, c2) = lstm.forward_step(&x1, &h1, &c1).unwrap();

        // Cell state should evolve
        assert_ne!(c1.to_vec(), c2.to_vec());
        assert_ne!(h1.to_vec(), h2.to_vec());
    }
}
