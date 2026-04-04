//! Neural network layer abstractions — Dense, Dropout, BatchNorm.
//!
//! Layers hold learnable parameters and implement forward passes.

use super::autograd::Tape;
use super::ops;
use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Dense (Linear) Layer
// ═══════════════════════════════════════════════════════════════════════

/// Fully-connected (dense) layer: `y = x @ W + b`.
///
/// Weight shape: `[in_features, out_features]`
/// Bias shape: `[1, out_features]`
#[derive(Debug, Clone)]
pub struct Dense {
    /// Weight matrix.
    pub weight: TensorValue,
    /// Bias vector.
    pub bias: TensorValue,
}

impl Dense {
    /// Creates a Dense layer with random weights and zero bias.
    pub fn new(in_features: usize, out_features: usize) -> Self {
        let mut weight = TensorValue::randn(&[in_features, out_features]);
        weight.set_requires_grad(true);
        // Xavier initialization: scale by sqrt(2 / (in + out))
        let scale = (2.0 / (in_features + out_features) as f64).sqrt();
        *weight.data_mut() *= scale;

        let mut bias = TensorValue::zeros(&[1, out_features]);
        bias.set_requires_grad(true);

        Self { weight, bias }
    }

    /// Forward pass: `x @ W + b`.
    ///
    /// Input shape: `[batch, in_features]`
    /// Output shape: `[batch, out_features]`
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        let xw = ops::matmul(x, &self.weight)?;
        ops::add(&xw, &self.bias)
    }

    /// Forward pass with autograd tape recording: `x @ W + b` (tracked).
    pub fn forward_tracked(
        &self,
        x: &TensorValue,
        tape: &mut crate::runtime::ml::autograd::Tape,
    ) -> Result<TensorValue, TensorError> {
        let xw = ops::matmul_tracked(x, &self.weight, tape)?;
        ops::add_tracked(&xw, &self.bias, tape)
    }

    /// Returns all learnable parameters (weight and bias).
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.weight, &self.bias]
    }

    /// Returns mutable references to all learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.weight, &mut self.bias]
    }

    /// Returns the number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.weight.numel() + self.bias.numel()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GroupNorm
// ═══════════════════════════════════════════════════════════════════════

/// Group normalization: normalizes within channel groups over spatial dims.
///
/// Input: `[batch, channels, ...]` (any spatial dims after channels)
/// Normalizes over spatial dimensions within each group of channels.
#[derive(Debug, Clone)]
pub struct GroupNorm {
    /// Number of groups.
    pub num_groups: usize,
    /// Number of channels.
    pub num_channels: usize,
    /// Learnable scale parameter (initialized to 1).
    pub gamma: TensorValue,
    /// Learnable shift parameter (initialized to 0).
    pub beta: TensorValue,
    /// Epsilon for numerical stability.
    eps: f64,
}

impl GroupNorm {
    /// Create a new GroupNorm layer.
    pub fn new(num_groups: usize, num_channels: usize) -> Self {
        assert!(
            num_channels.is_multiple_of(num_groups),
            "channels ({num_channels}) must be divisible by groups ({num_groups})"
        );
        let mut gamma = TensorValue::from_data(vec![1.0; num_channels], &[1, num_channels])
            .expect("gamma init");
        gamma.set_requires_grad(true);
        let mut beta = TensorValue::zeros(&[1, num_channels]);
        beta.set_requires_grad(true);
        Self {
            num_groups,
            num_channels,
            gamma,
            beta,
            eps: 1e-5,
        }
    }

    /// Forward pass: normalize over spatial dims within each channel group.
    ///
    /// For 4D input [B, C, H, W]: reshape to [B, G, C/G, H, W], normalize over (2,3,4).
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        let shape = x.shape();
        let b = shape[0];
        let c = shape[1];
        let spatial: usize = shape[2..].iter().product();
        let g = self.num_groups;
        let cpg = c / g; // channels per group

        let data = x.data();
        let mut out_data = data.clone();

        // Normalize per (batch, group)
        for bi in 0..b {
            for gi in 0..g {
                let ch_start = gi * cpg;
                let ch_end = ch_start + cpg;
                // Collect all values in this group
                let mut vals = Vec::with_capacity(cpg * spatial);
                for ci in ch_start..ch_end {
                    for si in 0..spatial {
                        let idx = bi * c * spatial + ci * spatial + si;
                        vals.push(data.as_slice().unwrap_or(&[])[idx]);
                    }
                }
                let n = vals.len() as f64;
                let mean: f64 = vals.iter().sum::<f64>() / n;
                let var: f64 = vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
                let std = (var + self.eps).sqrt();
                // Normalize and apply gamma/beta
                for ci in ch_start..ch_end {
                    let gamma_c = self.gamma.data().as_slice().unwrap_or(&[1.0])[ci];
                    let beta_c = self.beta.data().as_slice().unwrap_or(&[0.0])[ci];
                    for si in 0..spatial {
                        let idx = bi * c * spatial + ci * spatial + si;
                        let slice = out_data.as_slice_mut().expect("contiguous ndarray");
                        slice[idx] = (slice[idx] - mean) / std * gamma_c + beta_c;
                    }
                }
            }
        }
        Ok(TensorValue::new(out_data, x.requires_grad()))
    }

    /// Parameters: gamma + beta.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.gamma, &self.beta]
    }

    /// Mutable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.gamma, &mut self.beta]
    }

    /// Parameter count.
    pub fn param_count(&self) -> usize {
        self.num_channels * 2
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dropout
// ═══════════════════════════════════════════════════════════════════════

/// Dropout layer: randomly zeroes elements during training.
///
/// During evaluation (training=false), acts as identity.
#[derive(Debug, Clone)]
pub struct Dropout {
    /// Probability of zeroing an element (0.0 to 1.0).
    p: f64,
    /// Whether the layer is in training mode.
    training: bool,
}

impl Dropout {
    /// Creates a Dropout layer with the given drop probability.
    pub fn new(p: f64) -> Self {
        Self { p, training: true }
    }

    /// Sets training mode.
    pub fn set_training(&mut self, training: bool) {
        self.training = training;
    }

    /// Forward pass: randomly zeroes elements during training.
    pub fn forward(&self, x: &TensorValue) -> TensorValue {
        if !self.training || self.p == 0.0 {
            return x.clone();
        }

        use ndarray_rand::RandomExt;
        use ndarray_rand::rand_distr::Uniform;
        let mask = ndarray::ArrayD::random(x.shape(), Uniform::new(0.0, 1.0));
        let scale = 1.0 / (1.0 - self.p);
        let result = x.data().mapv(|v| v) * mask.mapv(|v| if v > self.p { scale } else { 0.0 });
        TensorValue::new(result, x.requires_grad())
    }

    /// Returns the drop probability.
    pub fn p(&self) -> f64 {
        self.p
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BatchNorm
// ═══════════════════════════════════════════════════════════════════════

/// Batch normalization: `y = gamma * (x - mean) / sqrt(var + eps) + beta`.
///
/// Normalizes across the batch dimension (axis 0), then applies learned
/// scale (gamma) and shift (beta).
#[derive(Debug, Clone)]
pub struct BatchNorm {
    /// Learnable scale parameter (gamma).
    pub gamma: TensorValue,
    /// Learnable shift parameter (beta).
    pub beta: TensorValue,
    /// Small constant for numerical stability.
    eps: f64,
}

impl BatchNorm {
    /// Creates a BatchNorm layer for `num_features` features.
    pub fn new(num_features: usize) -> Self {
        let mut gamma = TensorValue::ones(&[1, num_features]);
        gamma.set_requires_grad(true);
        let mut beta = TensorValue::zeros(&[1, num_features]);
        beta.set_requires_grad(true);

        Self {
            gamma,
            beta,
            eps: 1e-5,
        }
    }

    /// Forward pass: normalize, scale, shift.
    ///
    /// Input shape: `[batch, features]`
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        if x.ndim() != 2 {
            return Err(TensorError::RankMismatch {
                expected: 2,
                got: x.ndim(),
            });
        }

        let shape = x.shape();
        let batch = shape[0];
        let features = shape[1];

        // Compute mean and variance along batch dimension (axis 0)
        let mut mean_data = vec![0.0; features];
        let mut var_data = vec![0.0; features];

        let x_data = x.data();
        for j in 0..features {
            let mut sum = 0.0;
            for i in 0..batch {
                sum += x_data[[i, j]];
            }
            mean_data[j] = sum / batch as f64;

            let mut var_sum = 0.0;
            for i in 0..batch {
                let diff = x_data[[i, j]] - mean_data[j];
                var_sum += diff * diff;
            }
            var_data[j] = var_sum / batch as f64;
        }

        // Normalize: (x - mean) / sqrt(var + eps)
        let mut result_data = vec![0.0; batch * features];
        for i in 0..batch {
            for j in 0..features {
                result_data[i * features + j] =
                    (x_data[[i, j]] - mean_data[j]) / (var_data[j] + self.eps).sqrt();
            }
        }

        let normalized = TensorValue::from_data(result_data, &[batch, features])?;

        // Apply scale and shift: gamma * normalized + beta
        let scaled = ops::mul(&normalized, &self.gamma)?;
        ops::add(&scaled, &self.beta)
    }

    /// Returns all learnable parameters (gamma and beta).
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.gamma, &self.beta]
    }

    /// Returns mutable references to all learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.gamma, &mut self.beta]
    }

    /// Returns the number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.gamma.numel() + self.beta.numel()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Conv2d Layer
// ═══════════════════════════════════════════════════════════════════════

/// 2D convolution layer using im2col + matmul approach.
///
/// Input shape: `[batch, in_channels, height, width]`
/// Output shape: `[batch, out_channels, out_h, out_w]`
/// where `out_h = (height + 2*padding - kernel_size) / stride + 1`
#[derive(Debug, Clone)]
pub struct Conv2d {
    /// Filter weights: `[out_channels, in_channels * kernel_size * kernel_size]`.
    pub weight: TensorValue,
    /// Bias: `[1, out_channels]`.
    pub bias: TensorValue,
    /// Kernel (filter) size.
    pub kernel_size: usize,
    /// Stride.
    pub stride: usize,
    /// Padding (zero-padding on each side).
    pub padding: usize,
    /// Number of input channels.
    pub in_channels: usize,
    /// Number of output channels.
    pub out_channels: usize,
}

impl Conv2d {
    /// Creates a Conv2d layer with Xavier-initialized weights.
    pub fn new(
        in_channels: usize,
        out_channels: usize,
        kernel_size: usize,
        stride: usize,
        padding: usize,
    ) -> Self {
        let fan_in = in_channels * kernel_size * kernel_size;
        let fan_out = out_channels * kernel_size * kernel_size;
        let scale = (2.0 / (fan_in + fan_out) as f64).sqrt();

        let mut weight = TensorValue::randn(&[out_channels, fan_in]);
        *weight.data_mut() *= scale;
        weight.set_requires_grad(true);

        let mut bias = TensorValue::zeros(&[1, out_channels]);
        bias.set_requires_grad(true);

        Self {
            weight,
            bias,
            kernel_size,
            stride,
            padding,
            in_channels,
            out_channels,
        }
    }

    /// Forward pass using im2col + matmul.
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        if x.ndim() != 4 {
            return Err(TensorError::RankMismatch {
                expected: 4,
                got: x.ndim(),
            });
        }
        let shape = x.shape();
        let (batch, _in_ch, h, w) = (shape[0], shape[1], shape[2], shape[3]);
        let out_h = (h + 2 * self.padding - self.kernel_size) / self.stride + 1;
        let out_w = (w + 2 * self.padding - self.kernel_size) / self.stride + 1;
        let col_size = self.in_channels * self.kernel_size * self.kernel_size;

        // im2col: extract patches
        let mut col_data = Vec::with_capacity(batch * out_h * out_w * col_size);
        let x_data = x.data();
        for b in 0..batch {
            for oh in 0..out_h {
                for ow in 0..out_w {
                    for c in 0..self.in_channels {
                        for kh in 0..self.kernel_size {
                            for kw in 0..self.kernel_size {
                                let ih = oh * self.stride + kh;
                                let iw = ow * self.stride + kw;
                                let ih = ih as isize - self.padding as isize;
                                let iw = iw as isize - self.padding as isize;
                                if ih >= 0 && ih < h as isize && iw >= 0 && iw < w as isize {
                                    col_data.push(x_data[[b, c, ih as usize, iw as usize]]);
                                } else {
                                    col_data.push(0.0); // zero padding
                                }
                            }
                        }
                    }
                }
            }
        }

        // col shape: [batch * out_h * out_w, col_size]
        let col = TensorValue::from_data(col_data, &[batch * out_h * out_w, col_size])?;

        // matmul: col @ weight.T → [batch * out_h * out_w, out_channels]
        let wt = ops::transpose(&self.weight)?;
        let out_flat = ops::matmul(&col, &wt)?;

        // Add bias (broadcast)
        let out_biased = ops::add(&out_flat, &self.bias)?;

        // out_biased is [batch*out_h*out_w, out_channels]
        // Rearrange to [batch, out_channels, out_h, out_w]
        let out_data = out_biased.data();
        let mut final_data = vec![0.0; batch * self.out_channels * out_h * out_w];
        for b in 0..batch {
            for oh in 0..out_h {
                for ow in 0..out_w {
                    let row = b * out_h * out_w + oh * out_w + ow;
                    for oc in 0..self.out_channels {
                        final_data[b * self.out_channels * out_h * out_w
                            + oc * out_h * out_w
                            + oh * out_w
                            + ow] = out_data[[row, oc]];
                    }
                }
            }
        }

        let result_arr = ndarray::ArrayD::from_shape_vec(
            ndarray::IxDyn(&[batch, self.out_channels, out_h, out_w]),
            final_data,
        )
        .map_err(|e| TensorError::InvalidData {
            reason: e.to_string(),
        })?;
        Ok(TensorValue::new(result_arr, x.requires_grad()))
    }

    /// Forward pass with autograd tracking for backward gradient computation.
    ///
    /// Records im2col + matmul operations on the tape so that `backward()`
    /// computes gradients for weight, bias, and input.
    ///
    /// Backward:
    /// - `dW = col.T @ d_out_flat`
    /// - `db = sum(d_out_flat, axis=0)`
    /// - `dX = col2im(d_out_flat @ W)`
    pub fn forward_tracked(
        &self,
        x: &TensorValue,
        tape: &mut Tape,
    ) -> Result<TensorValue, TensorError> {
        if x.ndim() != 4 {
            return Err(TensorError::RankMismatch {
                expected: 4,
                got: x.ndim(),
            });
        }
        let shape = x.shape();
        let (batch, _in_ch, h, w) = (shape[0], shape[1], shape[2], shape[3]);
        let out_h = (h + 2 * self.padding - self.kernel_size) / self.stride + 1;
        let out_w = (w + 2 * self.padding - self.kernel_size) / self.stride + 1;
        let col_size = self.in_channels * self.kernel_size * self.kernel_size;

        // im2col: extract patches
        let mut col_data = Vec::with_capacity(batch * out_h * out_w * col_size);
        let x_data = x.data();
        for b in 0..batch {
            for oh in 0..out_h {
                for ow in 0..out_w {
                    for c in 0..self.in_channels {
                        for kh in 0..self.kernel_size {
                            for kw in 0..self.kernel_size {
                                let ih = oh * self.stride + kh;
                                let iw = ow * self.stride + kw;
                                let ih = ih as isize - self.padding as isize;
                                let iw = iw as isize - self.padding as isize;
                                if ih >= 0 && ih < h as isize && iw >= 0 && iw < w as isize {
                                    col_data.push(x_data[[b, c, ih as usize, iw as usize]]);
                                } else {
                                    col_data.push(0.0);
                                }
                            }
                        }
                    }
                }
            }
        }

        let col = TensorValue::from_data(col_data, &[batch * out_h * out_w, col_size])?;
        let wt = ops::transpose(&self.weight)?;
        let out_flat = ops::matmul(&col, &wt)?;
        let out_biased = ops::add(&out_flat, &self.bias)?;

        // Rearrange [batch*out_h*out_w, out_channels] → [batch, out_channels, out_h, out_w]
        let out_data = out_biased.data();
        let out_channels = self.out_channels;
        let mut final_data = vec![0.0; batch * out_channels * out_h * out_w];
        for b in 0..batch {
            for oh in 0..out_h {
                for ow in 0..out_w {
                    let row = b * out_h * out_w + oh * out_w + ow;
                    for oc in 0..out_channels {
                        final_data[b * out_channels * out_h * out_w
                            + oc * out_h * out_w
                            + oh * out_w
                            + ow] = out_data[[row, oc]];
                    }
                }
            }
        }

        let result_arr = ndarray::ArrayD::from_shape_vec(
            ndarray::IxDyn(&[batch, out_channels, out_h, out_w]),
            final_data,
        )
        .map_err(|e| TensorError::InvalidData {
            reason: e.to_string(),
        })?;
        let mut result = TensorValue::new(result_arr, true);

        // Record on tape for backward pass
        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let w_id = self.weight.id().unwrap_or_else(|| tape.fresh_id());
        let b_id = self.bias.id().unwrap_or_else(|| tape.fresh_id());
        let x_id = x.id().unwrap_or_else(|| tape.fresh_id());

        let col_data_saved = col.data().clone();
        let w_data_saved = self.weight.data().clone();
        let x_shape_saved = shape.to_vec();
        let kernel_size = self.kernel_size;
        let stride = self.stride;
        let padding = self.padding;
        let in_channels = self.in_channels;

        tape.record(
            out_id,
            vec![x_id, w_id, b_id],
            Box::new(move |grad_out| {
                // grad_out: [batch, out_channels, out_h, out_w]
                // Flatten to [batch*out_h*out_w, out_channels]
                let go = grad_out;
                let go_shape = go.shape();
                let batch = go_shape[0];
                let out_ch = go_shape[1];
                let out_h = go_shape[2];
                let out_w = go_shape[3];

                // Flatten grad_out: [batch, out_ch, out_h, out_w] → [batch*out_h*out_w, out_ch]
                let mut go_flat_data = vec![0.0; batch * out_h * out_w * out_ch];
                for b in 0..batch {
                    for oh in 0..out_h {
                        for ow in 0..out_w {
                            let row = b * out_h * out_w + oh * out_w + ow;
                            for oc in 0..out_ch {
                                go_flat_data[row * out_ch + oc] = go[[b, oc, oh, ow]];
                            }
                        }
                    }
                }
                let go_flat = ndarray::ArrayD::from_shape_vec(
                    ndarray::IxDyn(&[batch * out_h * out_w, out_ch]),
                    go_flat_data,
                )
                .expect("Conv2d backward: reshape grad_output to [batch*out_h*out_w, out_ch]");

                // dW = col.T @ go_flat → [col_size, out_channels]
                let col_2d = col_data_saved
                    .clone()
                    .into_shape_with_order(ndarray::Ix2(
                        batch * out_h * out_w,
                        in_channels * kernel_size * kernel_size,
                    ))
                    .expect("Conv2d backward: reshape col_data to [batch*out_h*out_w, in_ch*k*k] for dW computation");
                let col_t = col_2d.t();
                let go_flat_2d = go_flat
                    .clone()
                    .into_shape_with_order(ndarray::Ix2(batch * out_h * out_w, out_ch))
                    .expect("Conv2d backward: reshape go_flat to Ix2 [batch*out_h*out_w, out_ch] for dW dot product");
                let grad_w = col_t.dot(&go_flat_2d).into_dyn();

                // db = sum(go_flat, axis=0) → [1, out_channels]
                let go_2d = go_flat
                    .clone()
                    .into_shape_with_order(ndarray::Ix2(batch * out_h * out_w, out_ch))
                    .expect("Conv2d backward: reshape go_flat to Ix2 [batch*out_h*out_w, out_ch] for bias gradient sum");
                let grad_b_1d = go_2d.sum_axis(ndarray::Axis(0));
                let grad_b = grad_b_1d
                    .into_shape_with_order(ndarray::IxDyn(&[1, out_ch]))
                    .expect("Conv2d backward: reshape bias gradient to [1, out_ch]")
                    .into_dyn();

                // dX = col2im(go_flat @ W) — accumulate into input shape
                let h = x_shape_saved[2];
                let w_dim = x_shape_saved[3];
                let mut grad_x = ndarray::ArrayD::zeros(ndarray::IxDyn(&x_shape_saved));

                // go_flat @ W → [batch*out_h*out_w, col_size]
                let go_2d = go_flat
                    .into_shape_with_order(ndarray::Ix2(batch * out_h * out_w, out_ch))
                    .expect("Conv2d backward: reshape go_flat to Ix2 [batch*out_h*out_w, out_ch] for dX computation");
                let w_2d = w_data_saved
                    .clone()
                    .into_shape_with_order(ndarray::Ix2(
                        out_ch,
                        in_channels * kernel_size * kernel_size,
                    ))
                    .expect("Conv2d backward: reshape weight to Ix2 [out_ch, in_ch*k*k] for dX dot product");
                let dcol = go_2d.dot(&w_2d);

                // col2im: scatter dcol back to grad_x
                for b in 0..batch {
                    for oh in 0..out_h {
                        for ow in 0..out_w {
                            let row = b * out_h * out_w + oh * out_w + ow;
                            let mut col_idx = 0;
                            for c in 0..in_channels {
                                for kh in 0..kernel_size {
                                    for kw in 0..kernel_size {
                                        let ih = (oh * stride + kh) as isize - padding as isize;
                                        let iw = (ow * stride + kw) as isize - padding as isize;
                                        if ih >= 0
                                            && ih < h as isize
                                            && iw >= 0
                                            && iw < w_dim as isize
                                        {
                                            grad_x[[b, c, ih as usize, iw as usize]] +=
                                                dcol[[row, col_idx]];
                                        }
                                        col_idx += 1;
                                    }
                                }
                            }
                        }
                    }
                }

                vec![grad_x, grad_w, grad_b]
            }),
        );

        Ok(result)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.weight, &self.bias]
    }

    /// Returns mutable references to all learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.weight, &mut self.bias]
    }

    /// Returns the number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.weight.numel() + self.bias.numel()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Attention Layer
// ═══════════════════════════════════════════════════════════════════════

/// Scaled dot-product attention: `softmax(Q @ K.T / sqrt(d_k)) @ V`.
///
/// Input shapes: Q=[seq_q, d_k], K=[seq_k, d_k], V=[seq_k, d_v]
/// Output shape: [seq_q, d_v]
pub fn scaled_dot_product_attention(
    q: &TensorValue,
    k: &TensorValue,
    v: &TensorValue,
) -> Result<TensorValue, TensorError> {
    if q.ndim() != 2 || k.ndim() != 2 || v.ndim() != 2 {
        return Err(TensorError::RankMismatch {
            expected: 2,
            got: q.ndim(),
        });
    }
    let d_k = q.shape()[1] as f64;
    let kt = ops::transpose(k)?;
    let scores = ops::matmul(q, &kt)?;
    // Scale by 1/sqrt(d_k)
    let scale = TensorValue::full(scores.shape(), 1.0 / d_k.sqrt());
    let scaled = ops::mul(&scores, &scale)?;
    let weights = ops::softmax(&scaled);
    ops::matmul(&weights, v)
}

/// Tracked attention with backward gradient through softmax and matmul.
///
/// Records operations so that backward() computes dQ, dK, dV.
///
/// Backward:
/// - `dV = attn_weights.T @ d_out`
/// - `d_attn = d_out @ V.T`
/// - `d_scores = d_attn * softmax_jacobian`
/// - `dQ = d_scores @ K / sqrt(d_k)`
/// - `dK = d_scores.T @ Q / sqrt(d_k)`
pub fn scaled_dot_product_attention_tracked(
    q: &TensorValue,
    k: &TensorValue,
    v: &TensorValue,
    tape: &mut Tape,
) -> Result<TensorValue, TensorError> {
    if q.ndim() != 2 || k.ndim() != 2 || v.ndim() != 2 {
        return Err(TensorError::RankMismatch {
            expected: 2,
            got: q.ndim(),
        });
    }
    let d_k = q.shape()[1] as f64;
    let kt = ops::transpose(k)?;
    let scores = ops::matmul(q, &kt)?;
    let scale = TensorValue::full(scores.shape(), 1.0 / d_k.sqrt());
    let scaled = ops::mul(&scores, &scale)?;
    let weights = ops::softmax(&scaled);
    let mut result = ops::matmul(&weights, v)?;

    // Record on tape
    let out_id = tape.fresh_id();
    result.set_id(out_id);
    let q_id = q.id().unwrap_or_else(|| tape.fresh_id());
    let k_id = k.id().unwrap_or_else(|| tape.fresh_id());
    let v_id = v.id().unwrap_or_else(|| tape.fresh_id());

    let q_data = q.data().clone();
    let k_data = k.data().clone();
    let v_data = v.data().clone();
    let weights_data = weights.data().clone();

    tape.record(
        out_id,
        vec![q_id, k_id, v_id],
        Box::new(move |grad_out| {
            let go = grad_out;
            let scale_val = 1.0 / d_k.sqrt();

            // dV = attn_weights.T @ d_out
            let w_2d = weights_data
                .clone()
                .into_shape_with_order(ndarray::Ix2(
                    weights_data.shape()[0],
                    weights_data.shape()[1],
                ))
                .expect("attention backward: reshape attn_weights to Ix2 [seq_q, seq_k] for dV computation");
            let w_t = w_2d.t();
            let go_2d = go
                .clone()
                .into_shape_with_order(ndarray::Ix2(go.shape()[0], go.shape()[1]))
                .expect("attention backward: reshape grad_output to Ix2 [seq_q, d_v] for dV computation");
            let grad_v = w_t.dot(&go_2d);

            // d_attn = d_out @ V.T
            let v_2d = v_data
                .clone()
                .into_shape_with_order(ndarray::Ix2(v_data.shape()[0], v_data.shape()[1]))
                .expect("attention backward: reshape V to Ix2 [seq_k, d_v] for d_attn computation");
            let v_t = v_2d.t();
            let d_attn = go_2d.dot(&v_t);

            // Softmax backward: d_scores[i] = attn[i] * (d_attn[i] - sum(d_attn[i] * attn[i]))
            // Applied row-wise
            let rows = weights_data.shape()[0];
            let cols = weights_data.shape()[1];
            let mut d_scores = ndarray::Array2::zeros(ndarray::Ix2(rows, cols));
            for r in 0..rows {
                let mut dot_sum = 0.0;
                for c in 0..cols {
                    dot_sum += d_attn[[r, c]] * w_2d[[r, c]];
                }
                for c in 0..cols {
                    d_scores[[r, c]] = w_2d[[r, c]] * (d_attn[[r, c]] - dot_sum);
                }
            }

            // dQ = d_scores @ K * scale
            let k_2d = k_data
                .clone()
                .into_shape_with_order(ndarray::Ix2(k_data.shape()[0], k_data.shape()[1]))
                .expect("attention backward: reshape K to Ix2 [seq_k, d_k] for dQ computation");
            let grad_q = d_scores.dot(&k_2d) * scale_val;

            // dK = d_scores.T @ Q * scale
            let d_scores_t = d_scores.t();
            let q_2d = q_data
                .clone()
                .into_shape_with_order(ndarray::Ix2(q_data.shape()[0], q_data.shape()[1]))
                .expect("attention backward: reshape Q to Ix2 [seq_q, d_k] for dK computation");
            let grad_k = d_scores_t.to_owned().dot(&q_2d) * scale_val;

            vec![grad_q.into_dyn(), grad_k.into_dyn(), grad_v.into_dyn()]
        }),
    );

    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Multi-Head Attention
// ═══════════════════════════════════════════════════════════════════════

/// Multi-head attention: splits Q, K, V into `num_heads` heads,
/// applies scaled dot-product attention per head, then concatenates.
///
/// Input shapes: Q=[seq_q, d_model], K=[seq_k, d_model], V=[seq_k, d_model]
/// Output shape: [seq_q, d_model]
#[derive(Debug, Clone)]
pub struct MultiHeadAttention {
    /// Number of attention heads.
    pub num_heads: usize,
    /// Model dimension (must be divisible by num_heads).
    pub d_model: usize,
    /// Per-head dimension (d_model / num_heads).
    pub d_k: usize,
    /// Learned projection: Q → [d_model, d_model].
    pub w_q: TensorValue,
    /// Learned projection: K → [d_model, d_model].
    pub w_k: TensorValue,
    /// Learned projection: V → [d_model, d_model].
    pub w_v: TensorValue,
    /// Output projection: [d_model, d_model].
    pub w_o: TensorValue,
}

impl MultiHeadAttention {
    /// Creates a multi-head attention layer with Xavier-initialized weights.
    pub fn new(d_model: usize, num_heads: usize) -> Self {
        assert!(
            d_model.is_multiple_of(num_heads),
            "d_model must be divisible by num_heads"
        );
        let d_k = d_model / num_heads;
        let scale = (2.0 / (d_model + d_model) as f64).sqrt();

        let make_weight = |rows, cols| {
            let mut w = TensorValue::randn(&[rows, cols]);
            *w.data_mut() *= scale;
            w.set_requires_grad(true);
            w
        };

        Self {
            num_heads,
            d_model,
            d_k,
            w_q: make_weight(d_model, d_model),
            w_k: make_weight(d_model, d_model),
            w_v: make_weight(d_model, d_model),
            w_o: make_weight(d_model, d_model),
        }
    }

    /// Forward pass: project, split heads, attend per head, concatenate, project output.
    pub fn forward(
        &self,
        q: &TensorValue,
        k: &TensorValue,
        v: &TensorValue,
    ) -> Result<TensorValue, TensorError> {
        // Project: Q' = Q @ W_q, K' = K @ W_k, V' = V @ W_v
        let q_proj = ops::matmul(q, &self.w_q)?;
        let k_proj = ops::matmul(k, &self.w_k)?;
        let v_proj = ops::matmul(v, &self.w_v)?;

        // Split into heads: [seq, d_model] → num_heads × [seq, d_k]
        let q_heads = ops::split(&q_proj, 1, self.d_k)?;
        let k_heads = ops::split(&k_proj, 1, self.d_k)?;
        let v_heads = ops::split(&v_proj, 1, self.d_k)?;

        // Per-head attention
        let mut head_outputs = Vec::with_capacity(self.num_heads);
        for i in 0..self.num_heads {
            let head_out = scaled_dot_product_attention(&q_heads[i], &k_heads[i], &v_heads[i])?;
            head_outputs.push(head_out);
        }

        // Concatenate heads: num_heads × [seq_q, d_k] → [seq_q, d_model]
        let concatenated = ops::concat(&head_outputs, 1)?;

        // Output projection
        ops::matmul(&concatenated, &self.w_o)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.w_q, &self.w_k, &self.w_v, &self.w_o]
    }

    /// Returns the number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.w_q.numel() + self.w_k.numel() + self.w_v.numel() + self.w_o.numel()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LayerNorm
// ═══════════════════════════════════════════════════════════════════════

/// Layer normalization: normalizes across the feature dimension.
///
/// Input shape: `[batch, features]`
/// Formula: `y = gamma * (x - mean) / sqrt(var + eps) + beta`
/// where mean and var are computed per-sample (axis 1).
#[derive(Debug, Clone)]
pub struct LayerNorm {
    /// Learnable scale (gamma).
    pub gamma: TensorValue,
    /// Learnable shift (beta).
    pub beta: TensorValue,
    /// Epsilon for numerical stability.
    eps: f64,
}

impl LayerNorm {
    /// Creates a LayerNorm for `num_features` features.
    pub fn new(num_features: usize) -> Self {
        let mut gamma = TensorValue::ones(&[1, num_features]);
        gamma.set_requires_grad(true);
        let mut beta = TensorValue::zeros(&[1, num_features]);
        beta.set_requires_grad(true);
        Self {
            gamma,
            beta,
            eps: 1e-5,
        }
    }

    /// Forward pass: per-sample normalization across features.
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        if x.ndim() != 2 {
            return Err(TensorError::RankMismatch {
                expected: 2,
                got: x.ndim(),
            });
        }
        let shape = x.shape();
        let (batch, features) = (shape[0], shape[1]);
        let x_data = x.data();

        let mut result_data = vec![0.0; batch * features];
        for i in 0..batch {
            // Compute mean and var across features for this sample
            let mut sum = 0.0;
            for j in 0..features {
                sum += x_data[[i, j]];
            }
            let mean = sum / features as f64;

            let mut var_sum = 0.0;
            for j in 0..features {
                let diff = x_data[[i, j]] - mean;
                var_sum += diff * diff;
            }
            let var = var_sum / features as f64;

            for j in 0..features {
                result_data[i * features + j] = (x_data[[i, j]] - mean) / (var + self.eps).sqrt();
            }
        }

        let normalized = TensorValue::from_data(result_data, &[batch, features])?;
        let scaled = ops::mul(&normalized, &self.gamma)?;
        ops::add(&scaled, &self.beta)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.gamma, &self.beta]
    }

    /// Returns the number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.gamma.numel() + self.beta.numel()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Embedding Layer
// ═══════════════════════════════════════════════════════════════════════

/// Embedding layer: maps integer indices to dense vectors.
///
/// Weight shape: `[num_embeddings, embedding_dim]`
#[derive(Debug, Clone)]
pub struct Embedding {
    /// Embedding weight matrix.
    pub weight: TensorValue,
    /// Vocabulary size.
    pub num_embeddings: usize,
    /// Embedding dimension.
    pub embedding_dim: usize,
}

impl Embedding {
    /// Creates an Embedding layer with random weights.
    pub fn new(num_embeddings: usize, embedding_dim: usize) -> Self {
        let mut weight = TensorValue::randn(&[num_embeddings, embedding_dim]);
        weight.set_requires_grad(true);
        Self {
            weight,
            num_embeddings,
            embedding_dim,
        }
    }

    /// Forward pass: look up embeddings for integer indices.
    ///
    /// `indices` is a 1D tensor of integer indices.
    /// Returns shape `[len(indices), embedding_dim]`.
    pub fn forward(&self, indices: &[usize]) -> Result<TensorValue, TensorError> {
        let mut data = Vec::with_capacity(indices.len() * self.embedding_dim);
        let w_data = self.weight.data();
        for &idx in indices {
            if idx >= self.num_embeddings {
                return Err(TensorError::ShapeMismatch {
                    expected: vec![self.num_embeddings],
                    got: vec![idx],
                });
            }
            for j in 0..self.embedding_dim {
                data.push(w_data[[idx, j]]);
            }
        }
        TensorValue::from_data(data, &[indices.len(), self.embedding_dim])
    }

    /// Forward pass with autograd tracking (scatter-add gradient).
    ///
    /// Backward: gradient is scattered back to the weight rows that were looked up.
    /// `dW[idx] += d_out[i]` for each index.
    pub fn forward_tracked(
        &self,
        indices: &[usize],
        tape: &mut Tape,
    ) -> Result<TensorValue, TensorError> {
        let mut result = self.forward(indices)?;
        result.set_requires_grad(true);

        let out_id = tape.fresh_id();
        result.set_id(out_id);
        let w_id = self.weight.id().unwrap_or_else(|| tape.fresh_id());

        let indices_saved = indices.to_vec();
        let num_embeddings = self.num_embeddings;
        let embedding_dim = self.embedding_dim;

        tape.record(
            out_id,
            vec![w_id],
            Box::new(move |grad_out| {
                // grad_out: [len(indices), embedding_dim]
                // scatter-add into [num_embeddings, embedding_dim]
                let mut grad_w =
                    ndarray::ArrayD::zeros(ndarray::IxDyn(&[num_embeddings, embedding_dim]));
                for (i, &idx) in indices_saved.iter().enumerate() {
                    for j in 0..embedding_dim {
                        grad_w[[idx, j]] += grad_out[[i, j]];
                    }
                }
                vec![grad_w]
            }),
        );

        Ok(result)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.weight]
    }

    /// Returns the number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.weight.numel()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Dense ──

    #[test]
    fn dense_forward_shape() {
        let layer = Dense::new(3, 5);
        let x = TensorValue::from_data(vec![1.0; 6], &[2, 3]).unwrap();
        let y = layer.forward(&x).unwrap();
        assert_eq!(y.shape(), &[2, 5]);
    }

    #[test]
    fn dense_param_count() {
        let layer = Dense::new(4, 3);
        // weight: 4*3=12, bias: 1*3=3, total=15
        assert_eq!(layer.param_count(), 15);
    }

    #[test]
    fn dense_requires_grad() {
        let layer = Dense::new(2, 2);
        assert!(layer.weight.requires_grad());
        assert!(layer.bias.requires_grad());
    }

    #[test]
    fn dense_parameters() {
        let layer = Dense::new(2, 3);
        assert_eq!(layer.parameters().len(), 2);
    }

    // ── Dropout ──

    #[test]
    fn dropout_eval_mode_identity() {
        let mut layer = Dropout::new(0.5);
        layer.set_training(false);
        let x = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let y = layer.forward(&x);
        assert_eq!(y.to_vec(), x.to_vec());
    }

    #[test]
    fn dropout_zero_p_identity() {
        let layer = Dropout::new(0.0);
        let x = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let y = layer.forward(&x);
        assert_eq!(y.to_vec(), x.to_vec());
    }

    #[test]
    fn dropout_training_zeroes_some() {
        let layer = Dropout::new(0.9); // High dropout for testing
        let x = TensorValue::from_data(vec![1.0; 100], &[100]).unwrap();
        let y = layer.forward(&x);
        let zeroed = y.to_vec().iter().filter(|&&v| v == 0.0).count();
        // With p=0.9, expect roughly 90% zeroed (allow wide tolerance)
        assert!(zeroed > 50, "expected many zeroed elements, got {zeroed}");
    }

    #[test]
    fn dropout_preserves_shape() {
        let layer = Dropout::new(0.5);
        let x = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap();
        let y = layer.forward(&x);
        assert_eq!(y.shape(), &[3, 4]);
    }

    #[test]
    fn dropout_p() {
        let layer = Dropout::new(0.3);
        assert_eq!(layer.p(), 0.3);
    }

    // ── BatchNorm ──

    #[test]
    fn batchnorm_forward_shape() {
        let layer = BatchNorm::new(4);
        let x = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap();
        let y = layer.forward(&x).unwrap();
        assert_eq!(y.shape(), &[3, 4]);
    }

    #[test]
    fn batchnorm_param_count() {
        let layer = BatchNorm::new(5);
        // gamma: 1*5=5, beta: 1*5=5, total=10
        assert_eq!(layer.param_count(), 10);
    }

    #[test]
    fn batchnorm_normalizes() {
        let layer = BatchNorm::new(2);
        // Input with clear mean and variance
        let x = TensorValue::from_data(vec![1.0, 10.0, 3.0, 30.0, 5.0, 50.0], &[3, 2]).unwrap();
        let y = layer.forward(&x).unwrap();
        let data = y.to_vec();
        // After normalization with default gamma=1, beta=0:
        // Each feature column should have mean ≈ 0 and std ≈ 1
        // Feature 0: [1, 3, 5] → mean=3, std=sqrt(8/3)
        let mean_f0 = (data[0] + data[2] + data[4]) / 3.0;
        assert!(mean_f0.abs() < 1e-5, "mean should be ~0, got {mean_f0}");
    }

    #[test]
    fn batchnorm_requires_rank2() {
        let layer = BatchNorm::new(3);
        let x = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        assert!(matches!(
            layer.forward(&x),
            Err(TensorError::RankMismatch { expected: 2, .. })
        ));
    }

    #[test]
    fn batchnorm_requires_grad() {
        let layer = BatchNorm::new(3);
        assert!(layer.gamma.requires_grad());
        assert!(layer.beta.requires_grad());
    }

    // ── Conv2d ──

    #[test]
    fn conv2d_output_shape() {
        let layer = Conv2d::new(1, 2, 3, 1, 0);
        // Input: [1, 1, 5, 5] → Output: [1, 2, 3, 3]
        let x = TensorValue::from_data(vec![1.0; 25], &[1, 1, 5, 5]).unwrap();
        let y = layer.forward(&x).unwrap();
        assert_eq!(y.shape(), &[1, 2, 3, 3]);
    }

    #[test]
    fn conv2d_with_padding() {
        let layer = Conv2d::new(1, 1, 3, 1, 1);
        // Input: [1, 1, 5, 5] with padding=1 → Output: [1, 1, 5, 5] (same size)
        let x = TensorValue::from_data(vec![1.0; 25], &[1, 1, 5, 5]).unwrap();
        let y = layer.forward(&x).unwrap();
        assert_eq!(y.shape(), &[1, 1, 5, 5]);
    }

    #[test]
    fn conv2d_with_stride() {
        let layer = Conv2d::new(1, 1, 3, 2, 0);
        // Input: [1, 1, 5, 5], stride=2 → Output: [1, 1, 2, 2]
        let x = TensorValue::from_data(vec![1.0; 25], &[1, 1, 5, 5]).unwrap();
        let y = layer.forward(&x).unwrap();
        assert_eq!(y.shape(), &[1, 1, 2, 2]);
    }

    #[test]
    fn conv2d_param_count() {
        let layer = Conv2d::new(3, 16, 3, 1, 0);
        // weight: 16 * (3*3*3) = 16*27 = 432, bias: 16
        assert_eq!(layer.param_count(), 432 + 16);
    }

    #[test]
    fn conv2d_requires_rank4() {
        let layer = Conv2d::new(1, 1, 3, 1, 0);
        let x = TensorValue::from_data(vec![1.0; 9], &[3, 3]).unwrap();
        assert!(matches!(
            layer.forward(&x),
            Err(TensorError::RankMismatch { expected: 4, .. })
        ));
    }

    // ── Attention ──

    #[test]
    fn attention_output_shape() {
        let q = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap(); // [3, 4]
        let k = TensorValue::from_data(vec![1.0; 20], &[5, 4]).unwrap(); // [5, 4]
        let v = TensorValue::from_data(vec![1.0; 30], &[5, 6]).unwrap(); // [5, 6]
        let out = scaled_dot_product_attention(&q, &k, &v).unwrap();
        assert_eq!(out.shape(), &[3, 6]); // [seq_q, d_v]
    }

    #[test]
    fn attention_uniform_weights() {
        // When Q and K produce equal scores, attention weights should be uniform
        let q = TensorValue::from_data(vec![0.0; 4], &[1, 4]).unwrap();
        let k = TensorValue::from_data(vec![0.0; 8], &[2, 4]).unwrap();
        let v = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let out = scaled_dot_product_attention(&q, &k, &v).unwrap();
        let data = out.to_vec();
        // With uniform attention: out = mean of V rows = [2.0, 3.0]
        assert!((data[0] - 2.0).abs() < 1e-4);
        assert!((data[1] - 3.0).abs() < 1e-4);
    }

    // ── LayerNorm ──

    #[test]
    fn layernorm_output_shape() {
        let layer = LayerNorm::new(4);
        let x = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap();
        let y = layer.forward(&x).unwrap();
        assert_eq!(y.shape(), &[3, 4]);
    }

    #[test]
    fn layernorm_normalizes_per_sample() {
        let layer = LayerNorm::new(3);
        let x = TensorValue::from_data(vec![1.0, 4.0, 7.0, 10.0, 13.0, 16.0], &[2, 3]).unwrap();
        let y = layer.forward(&x).unwrap();
        let data = y.to_vec();
        // Each row should have mean ≈ 0
        let mean_row0 = (data[0] + data[1] + data[2]) / 3.0;
        assert!(
            mean_row0.abs() < 1e-5,
            "row 0 mean should be ~0, got {mean_row0}"
        );
        let mean_row1 = (data[3] + data[4] + data[5]) / 3.0;
        assert!(
            mean_row1.abs() < 1e-5,
            "row 1 mean should be ~0, got {mean_row1}"
        );
    }

    #[test]
    fn layernorm_param_count() {
        let layer = LayerNorm::new(5);
        assert_eq!(layer.param_count(), 10); // gamma(5) + beta(5)
    }

    #[test]
    fn layernorm_requires_rank2() {
        let layer = LayerNorm::new(3);
        let x = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        assert!(matches!(
            layer.forward(&x),
            Err(TensorError::RankMismatch { expected: 2, .. })
        ));
    }

    // ── Embedding ──

    #[test]
    fn embedding_lookup() {
        let mut emb = Embedding::new(5, 3);
        // Set known weights for testing
        *emb.weight.data_mut() =
            ndarray::ArrayD::from_shape_vec(vec![5, 3], (0..15).map(|x| x as f64).collect())
                .unwrap();

        let result = emb.forward(&[0, 2, 4]).unwrap();
        assert_eq!(result.shape(), &[3, 3]);
        let data = result.to_vec();
        // Row 0 = embedding[0] = [0, 1, 2]
        assert_eq!(data[0], 0.0);
        assert_eq!(data[1], 1.0);
        assert_eq!(data[2], 2.0);
        // Row 1 = embedding[2] = [6, 7, 8]
        assert_eq!(data[3], 6.0);
        assert_eq!(data[4], 7.0);
        assert_eq!(data[5], 8.0);
    }

    #[test]
    fn embedding_out_of_range() {
        let emb = Embedding::new(5, 3);
        assert!(emb.forward(&[10]).is_err());
    }

    #[test]
    fn embedding_param_count() {
        let emb = Embedding::new(100, 64);
        assert_eq!(emb.param_count(), 6400);
    }

    // ── Conv2d backward ──

    #[test]
    fn conv2d_forward_tracked_output_shape() {
        let layer = Conv2d::new(1, 2, 3, 1, 0);
        let x = TensorValue::from_data(vec![1.0; 25], &[1, 1, 5, 5]).unwrap();
        let mut tape = Tape::new();
        let y = layer.forward_tracked(&x, &mut tape).unwrap();
        assert_eq!(y.shape(), &[1, 2, 3, 3]);
    }

    #[test]
    fn conv2d_backward_produces_gradients() {
        let mut layer = Conv2d::new(1, 1, 3, 1, 0);
        let mut x = TensorValue::from_data(vec![1.0; 25], &[1, 1, 5, 5]).unwrap();
        x.set_requires_grad(true);
        let mut tape = Tape::new();
        let x_id = tape.fresh_id();
        x.set_id(x_id);
        let w_id = tape.fresh_id();
        layer.weight.set_id(w_id);
        let b_id = tape.fresh_id();
        layer.bias.set_id(b_id);
        let y = layer.forward_tracked(&x, &mut tape).unwrap();
        let loss = ops::sum_tracked(&y, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
        assert!(grads.contains_key(&w_id), "weight gradient should exist");
        assert!(grads.contains_key(&x_id), "input gradient should exist");
    }

    // ── Attention backward ──

    #[test]
    fn attention_tracked_output_shape() {
        let q = TensorValue::from_data(vec![1.0; 8], &[2, 4]).unwrap();
        let k = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap();
        let v = TensorValue::from_data(vec![1.0; 6], &[3, 2]).unwrap();
        let mut tape = Tape::new();
        let out = scaled_dot_product_attention_tracked(&q, &k, &v, &mut tape).unwrap();
        assert_eq!(out.shape(), &[2, 2]);
    }

    #[test]
    fn attention_backward_produces_gradients() {
        let mut q = TensorValue::from_data(vec![0.1, 0.2, 0.3, 0.4], &[2, 2]).unwrap();
        let mut k = TensorValue::from_data(vec![0.5, 0.6, 0.7, 0.8], &[2, 2]).unwrap();
        let mut v = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        q.set_requires_grad(true);
        k.set_requires_grad(true);
        v.set_requires_grad(true);
        let mut tape = Tape::new();
        let q_id = tape.fresh_id();
        q.set_id(q_id);
        let k_id = tape.fresh_id();
        k.set_id(k_id);
        let v_id = tape.fresh_id();
        v.set_id(v_id);
        let out = scaled_dot_product_attention_tracked(&q, &k, &v, &mut tape).unwrap();
        let loss = ops::sum_tracked(&out, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
        assert!(grads.contains_key(&q_id), "Q gradient should exist");
        assert!(grads.contains_key(&k_id), "K gradient should exist");
        assert!(grads.contains_key(&v_id), "V gradient should exist");
    }

    // ── Embedding backward ──

    #[test]
    fn embedding_tracked_output_shape() {
        let emb = Embedding::new(10, 4);
        let mut tape = Tape::new();
        let out = emb.forward_tracked(&[2, 5, 7], &mut tape).unwrap();
        assert_eq!(out.shape(), &[3, 4]);
    }

    #[test]
    fn embedding_backward_scatter_add() {
        let emb = Embedding::new(5, 3);
        let mut tape = Tape::new();
        let w_id = tape.fresh_id();
        // Need to set weight id manually for tracking
        let mut emb = emb;
        emb.weight.set_id(w_id);
        let out = emb.forward_tracked(&[1, 3, 1], &mut tape).unwrap();
        let loss = ops::sum_tracked(&out, &mut tape);
        let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
        let grad_w = &grads[&w_id];
        // Index 1 used twice → gradient should be 2x at row 1
        // Each row gets all-ones gradient from sum, so row 1 should be [2, 2, 2]
        assert_eq!(grad_w.shape(), &[5, 3]);
        assert!(
            (grad_w[[1, 0]] - 2.0).abs() < 1e-10,
            "row 1 should have grad=2"
        );
        assert!(
            (grad_w[[3, 0]] - 1.0).abs() < 1e-10,
            "row 3 should have grad=1"
        );
        assert!(
            (grad_w[[0, 0]] - 0.0).abs() < 1e-10,
            "row 0 should have grad=0"
        );
    }

    // ── Multi-head attention ──

    #[test]
    fn multihead_attention_output_shape() {
        let mha = MultiHeadAttention::new(8, 2);
        let q = TensorValue::from_data(vec![1.0; 24], &[3, 8]).unwrap();
        let k = TensorValue::from_data(vec![1.0; 32], &[4, 8]).unwrap();
        let v = TensorValue::from_data(vec![1.0; 32], &[4, 8]).unwrap();
        let out = mha.forward(&q, &k, &v).unwrap();
        assert_eq!(out.shape(), &[3, 8]);
    }

    #[test]
    fn multihead_attention_param_count() {
        let mha = MultiHeadAttention::new(16, 4);
        // 4 weight matrices of 16x16 = 4*256 = 1024
        assert_eq!(mha.param_count(), 1024);
    }

    #[test]
    fn multihead_attention_single_head_matches() {
        // With 1 head, multi-head attention should behave like single-head
        let mha = MultiHeadAttention::new(4, 1);
        let q = TensorValue::from_data(vec![0.1; 8], &[2, 4]).unwrap();
        let k = TensorValue::from_data(vec![0.2; 12], &[3, 4]).unwrap();
        let v = TensorValue::from_data(vec![0.3; 12], &[3, 4]).unwrap();
        let out = mha.forward(&q, &k, &v).unwrap();
        assert_eq!(out.shape(), &[2, 4]);
    }

    // ── Reshape/Split/Concat ops ──

    #[test]
    fn reshape_basic() {
        let t = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let r = ops::reshape(&t, &[3, 2]).unwrap();
        assert_eq!(r.shape(), &[3, 2]);
    }

    #[test]
    fn reshape_mismatch_error() {
        let t = TensorValue::from_data(vec![1.0; 6], &[2, 3]).unwrap();
        assert!(ops::reshape(&t, &[4, 2]).is_err());
    }

    #[test]
    fn split_basic() {
        let t = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap();
        let chunks = ops::split(&t, 1, 2).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].shape(), &[3, 2]);
        assert_eq!(chunks[1].shape(), &[3, 2]);
    }

    #[test]
    fn concat_basic() {
        let a = TensorValue::from_data(vec![1.0; 6], &[3, 2]).unwrap();
        let b = TensorValue::from_data(vec![2.0; 6], &[3, 2]).unwrap();
        let c = ops::concat(&[a, b], 1).unwrap();
        assert_eq!(c.shape(), &[3, 4]);
    }
}
