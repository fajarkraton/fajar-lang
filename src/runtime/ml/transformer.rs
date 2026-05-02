//! Transformer architecture — attention, positional encoding, encoder, decoder.
//!
//! Implements scaled dot-product attention, multi-head attention,
//! positional encoding (sinusoidal and learned), layer normalization,
//! feed-forward networks, and full transformer encoder/decoder stacks.

use ndarray::{Array2, Axis};
use std::f64::consts::PI;

use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Converts a `TensorValue` to a 2D ndarray. Returns error if rank != 2.
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

/// Creates a `TensorValue` from a 2D ndarray.
fn from_array2(arr: &Array2<f64>, requires_grad: bool) -> Result<TensorValue, TensorError> {
    let shape = arr.shape();
    let data = arr.iter().copied().collect::<Vec<_>>();
    let mut tv = TensorValue::from_data(data, &[shape[0], shape[1]])?;
    tv.set_requires_grad(requires_grad);
    Ok(tv)
}

/// Numerically-stable softmax over the last axis of a 2D array.
///
/// For each row: exp(x - max(x)) / sum(exp(x - max(x))).
fn softmax_2d(x: &Array2<f64>) -> Array2<f64> {
    let rows = x.nrows();
    let cols = x.ncols();
    let mut result = Array2::zeros((rows, cols));
    for r in 0..rows {
        let row = x.row(r);
        let max_val = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = row.iter().map(|&v| (v - max_val).exp()).collect();
        let sum: f64 = exps.iter().sum();
        for c in 0..cols {
            result[[r, c]] = exps[c] / sum;
        }
    }
    result
}

/// GELU activation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3))).
fn gelu_array(x: &Array2<f64>) -> Array2<f64> {
    let sqrt_2_over_pi = (2.0 / PI).sqrt();
    x.mapv(|v| {
        let inner = sqrt_2_over_pi * (v + 0.044715 * v * v * v);
        0.5 * v * (1.0 + inner.tanh())
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Attention Masks
// ═══════════════════════════════════════════════════════════════════════

/// Attention mask type for controlling which positions can attend.
#[derive(Debug, Clone)]
pub enum AttentionMask {
    /// No mask — all positions attend to all positions.
    None,
    /// Causal (autoregressive) mask: upper-triangular positions set to -inf.
    Causal(usize),
    /// Padding mask: specific positions are masked out (true = masked).
    Padding(Vec<bool>),
}

impl AttentionMask {
    /// Creates a causal mask of the given sequence length.
    ///
    /// Positions where `col > row` are set to `f64::NEG_INFINITY`.
    pub fn causal(seq_len: usize) -> Self {
        AttentionMask::Causal(seq_len)
    }

    /// Creates a padding mask from a boolean vector.
    ///
    /// `true` values indicate positions that should be masked (set to -inf).
    pub fn padding(mask: Vec<bool>) -> Self {
        AttentionMask::Padding(mask)
    }

    /// Applies this mask to an attention score matrix `[seq_q, seq_k]`.
    ///
    /// Masked positions are set to `f64::NEG_INFINITY` so they vanish after softmax.
    pub fn apply(&self, scores: &mut Array2<f64>) {
        match self {
            AttentionMask::None => {}
            AttentionMask::Causal(seq_len) => {
                apply_causal_mask(scores, *seq_len);
            }
            AttentionMask::Padding(mask) => {
                apply_padding_mask(scores, mask);
            }
        }
    }
}

/// Applies causal mask: positions where col > row get NEG_INFINITY.
fn apply_causal_mask(scores: &mut Array2<f64>, seq_len: usize) {
    let rows = scores.nrows().min(seq_len);
    let cols = scores.ncols().min(seq_len);
    for r in 0..rows {
        for c in (r + 1)..cols {
            scores[[r, c]] = f64::NEG_INFINITY;
        }
    }
}

/// Applies padding mask: masked columns get NEG_INFINITY.
fn apply_padding_mask(scores: &mut Array2<f64>, mask: &[bool]) {
    let rows = scores.nrows();
    let cols = scores.ncols();
    for r in 0..rows {
        for c in 0..cols.min(mask.len()) {
            if mask[c] {
                scores[[r, c]] = f64::NEG_INFINITY;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Scaled Dot-Product Attention
// ═══════════════════════════════════════════════════════════════════════

/// Scaled dot-product attention result.
#[derive(Debug, Clone)]
pub struct AttentionOutput {
    /// Output tensor `[seq_q, d_v]`.
    pub output: Array2<f64>,
    /// Attention weights `[seq_q, seq_k]` (after softmax).
    pub weights: Array2<f64>,
}

/// Computes scaled dot-product attention.
///
/// `attention(Q, K, V) = softmax(Q * K^T / sqrt(d_k)) * V`
///
/// - `query`: `[seq_q, d_k]`
/// - `key`: `[seq_k, d_k]`
/// - `value`: `[seq_k, d_v]`
/// - `mask`: optional attention mask
///
/// Returns `AttentionOutput` with output `[seq_q, d_v]` and weights `[seq_q, seq_k]`.
pub fn scaled_dot_product_attention(
    query: &Array2<f64>,
    key: &Array2<f64>,
    value: &Array2<f64>,
    mask: &AttentionMask,
) -> AttentionOutput {
    let d_k = query.ncols() as f64;
    let scale = d_k.sqrt();

    // scores = Q @ K^T / sqrt(d_k)
    let mut scores = query.dot(&key.t()) / scale;

    // Apply mask
    mask.apply(&mut scores);

    // weights = softmax(scores)
    let weights = softmax_2d(&scores);

    // output = weights @ V
    let output = weights.dot(value);

    AttentionOutput { output, weights }
}

// ═══════════════════════════════════════════════════════════════════════
// Multi-Head Attention (Transformer-enhanced)
// ═══════════════════════════════════════════════════════════════════════

/// Transformer-style multi-head attention.
///
/// Projects Q, K, V through learned weight matrices, splits into heads,
/// applies scaled dot-product attention per head, and concatenates results.
///
/// Weight matrices: `w_q`, `w_k`, `w_v` (d_model x d_model), `w_o` (d_model x d_model).
#[derive(Debug, Clone)]
pub struct TransformerMHA {
    /// Query projection `[d_model, d_model]`.
    pub w_q: TensorValue,
    /// Key projection `[d_model, d_model]`.
    pub w_k: TensorValue,
    /// Value projection `[d_model, d_model]`.
    pub w_v: TensorValue,
    /// Output projection `[d_model, d_model]`.
    pub w_o: TensorValue,
    /// Model dimension.
    pub d_model: usize,
    /// Number of attention heads.
    pub n_heads: usize,
    /// Per-head key dimension (d_model / n_heads).
    pub d_k: usize,
}

impl TransformerMHA {
    /// Creates a new multi-head attention module with Xavier-initialized weights.
    ///
    /// `d_model` must be divisible by `n_heads`.
    pub fn new(d_model: usize, n_heads: usize) -> Result<Self, TensorError> {
        if d_model == 0 || n_heads == 0 {
            return Err(TensorError::InvalidData {
                reason: "d_model and n_heads must be > 0".to_string(),
            });
        }
        if !d_model.is_multiple_of(n_heads) {
            return Err(TensorError::InvalidData {
                reason: format!("d_model ({d_model}) must be divisible by n_heads ({n_heads})"),
            });
        }
        let d_k = d_model / n_heads;
        let scale = (2.0 / (d_model + d_model) as f64).sqrt();

        let w_q = create_weight(d_model, d_model, scale);
        let w_k = create_weight(d_model, d_model, scale);
        let w_v = create_weight(d_model, d_model, scale);
        let w_o = create_weight(d_model, d_model, scale);

        Ok(Self {
            w_q,
            w_k,
            w_v,
            w_o,
            d_model,
            n_heads,
            d_k,
        })
    }

    /// Forward pass: project Q/K/V, split heads, attend, concat, project output.
    ///
    /// - `query`: `[seq_q, d_model]`
    /// - `key`: `[seq_k, d_model]`
    /// - `value`: `[seq_k, d_model]`
    /// - `mask`: attention mask
    ///
    /// Returns output `[seq_q, d_model]`.
    pub fn forward(
        &self,
        query: &TensorValue,
        key: &TensorValue,
        value: &TensorValue,
        mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        let q = to_array2(query)?;
        let k = to_array2(key)?;
        let v = to_array2(value)?;
        let w_q = to_array2(&self.w_q)?;
        let w_k = to_array2(&self.w_k)?;
        let w_v = to_array2(&self.w_v)?;
        let w_o = to_array2(&self.w_o)?;

        let out = mha_forward(
            &q,
            &k,
            &v,
            &w_q,
            &w_k,
            &w_v,
            &w_o,
            self.n_heads,
            self.d_k,
            mask,
        );
        from_array2(&out, query.requires_grad())
    }

    /// Backward pass: computes gradients for w_q, w_k, w_v, w_o.
    ///
    /// - `query`, `key`, `value`: forward-pass inputs
    /// - `d_output`: gradient of loss w.r.t. forward output `[seq_q, d_model]`
    /// - `mask`: same mask used in forward
    ///
    /// Returns `MHAGradients`.
    pub fn backward(
        &self,
        query: &TensorValue,
        key: &TensorValue,
        value: &TensorValue,
        d_output: &Array2<f64>,
        mask: &AttentionMask,
    ) -> Result<MHAGradients, TensorError> {
        let q_proj = to_array2(query)?.dot(&to_array2(&self.w_q)?);
        let k_proj = to_array2(key)?.dot(&to_array2(&self.w_k)?);
        let v_proj = to_array2(value)?.dot(&to_array2(&self.w_v)?);
        let w_o = to_array2(&self.w_o)?;

        // d_concat = d_output @ W_o^T
        let d_concat = d_output.dot(&w_o.t());

        // d_w_o = concat^T @ d_output (need concat from forward)
        let concat = mha_heads_concat(&q_proj, &k_proj, &v_proj, self.n_heads, self.d_k, mask);
        let d_w_o = concat.t().dot(d_output);

        // Approximate gradients for Q/K/V projections
        let q_in = to_array2(query)?;
        let k_in = to_array2(key)?;
        let v_in = to_array2(value)?;
        let d_w_q = q_in.t().dot(&d_concat);
        let d_w_k = k_in.t().dot(&d_concat);
        let d_w_v = v_in.t().dot(&d_concat);

        Ok(MHAGradients {
            d_w_q,
            d_w_k,
            d_w_v,
            d_w_o,
        })
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.w_q, &self.w_k, &self.w_v, &self.w_o]
    }

    /// Returns mutable references to all learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.w_q, &mut self.w_k, &mut self.w_v, &mut self.w_o]
    }

    /// Returns the total number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.w_q.numel() + self.w_k.numel() + self.w_v.numel() + self.w_o.numel()
    }
}

/// Gradients for multi-head attention parameters.
#[derive(Debug, Clone)]
pub struct MHAGradients {
    /// Gradient for query projection weight.
    pub d_w_q: Array2<f64>,
    /// Gradient for key projection weight.
    pub d_w_k: Array2<f64>,
    /// Gradient for value projection weight.
    pub d_w_v: Array2<f64>,
    /// Gradient for output projection weight.
    pub d_w_o: Array2<f64>,
}

/// MHA forward: project, split heads, attend, concat, project output.
#[allow(clippy::too_many_arguments)]
fn mha_forward(
    q: &Array2<f64>,
    k: &Array2<f64>,
    v: &Array2<f64>,
    w_q: &Array2<f64>,
    w_k: &Array2<f64>,
    w_v: &Array2<f64>,
    w_o: &Array2<f64>,
    n_heads: usize,
    d_k: usize,
    mask: &AttentionMask,
) -> Array2<f64> {
    // Project: Q_proj = Q @ W_q, etc.
    let q_proj = q.dot(w_q);
    let k_proj = k.dot(w_k);
    let v_proj = v.dot(w_v);

    // Split heads, attend, concat
    let concat = mha_heads_concat(&q_proj, &k_proj, &v_proj, n_heads, d_k, mask);

    // Output projection: result = concat @ W_o
    concat.dot(w_o)
}

/// Splits Q/K/V into heads, runs attention, and concatenates.
fn mha_heads_concat(
    q_proj: &Array2<f64>,
    k_proj: &Array2<f64>,
    v_proj: &Array2<f64>,
    n_heads: usize,
    d_k: usize,
    mask: &AttentionMask,
) -> Array2<f64> {
    let seq_q = q_proj.nrows();
    let d_model = q_proj.ncols();
    let mut concat = Array2::zeros((seq_q, d_model));

    for h in 0..n_heads {
        let offset = h * d_k;
        let q_h = q_proj
            .slice(ndarray::s![.., offset..offset + d_k])
            .to_owned();
        let k_h = k_proj
            .slice(ndarray::s![.., offset..offset + d_k])
            .to_owned();
        let v_h = v_proj
            .slice(ndarray::s![.., offset..offset + d_k])
            .to_owned();

        let attn = scaled_dot_product_attention(&q_h, &k_h, &v_h, mask);
        concat
            .slice_mut(ndarray::s![.., offset..offset + d_k])
            .assign(&attn.output);
    }
    concat
}

/// Creates a Xavier-initialized weight tensor with requires_grad=true.
fn create_weight(rows: usize, cols: usize, scale: f64) -> TensorValue {
    let mut w = TensorValue::randn(&[rows, cols]);
    w.set_requires_grad(true);
    *w.data_mut() *= scale;
    w
}

// ═══════════════════════════════════════════════════════════════════════
// Positional Encoding
// ═══════════════════════════════════════════════════════════════════════

/// Sinusoidal positional encoding (Vaswani et al., 2017).
///
/// `PE(pos, 2i) = sin(pos / 10000^(2i/d_model))`
/// `PE(pos, 2i+1) = cos(pos / 10000^(2i/d_model))`
#[derive(Debug, Clone)]
pub struct SinusoidalPositionalEncoding {
    /// Precomputed encoding matrix `[max_seq_len, d_model]`.
    pub encoding: TensorValue,
}

impl SinusoidalPositionalEncoding {
    /// Creates sinusoidal positional encoding.
    ///
    /// - `max_seq_len`: maximum sequence length
    /// - `d_model`: model dimension (must be even)
    pub fn new(max_seq_len: usize, d_model: usize) -> Result<Self, TensorError> {
        let encoding = sinusoidal_encode(max_seq_len, d_model)?;
        Ok(Self { encoding })
    }

    /// Returns the encoding for positions `[0, seq_len)`.
    ///
    /// Returns `[seq_len, d_model]` tensor.
    pub fn get(&self, seq_len: usize) -> Result<TensorValue, TensorError> {
        if seq_len > self.encoding.shape()[0] {
            return Err(TensorError::InvalidData {
                reason: format!(
                    "seq_len ({seq_len}) exceeds max_seq_len ({})",
                    self.encoding.shape()[0]
                ),
            });
        }
        let d_model = self.encoding.shape()[1];
        let data = self.encoding.to_vec();
        let slice = &data[..seq_len * d_model];
        TensorValue::from_data(slice.to_vec(), &[seq_len, d_model])
    }
}

/// Computes sinusoidal positional encoding matrix.
fn sinusoidal_encode(max_seq_len: usize, d_model: usize) -> Result<TensorValue, TensorError> {
    let mut data = vec![0.0; max_seq_len * d_model];
    for pos in 0..max_seq_len {
        for i in 0..d_model / 2 {
            let angle = pos as f64 / (10000.0_f64).powf(2.0 * i as f64 / d_model as f64);
            data[pos * d_model + 2 * i] = angle.sin();
            data[pos * d_model + 2 * i + 1] = angle.cos();
        }
        // If d_model is odd, fill last position with sin
        if d_model % 2 == 1 {
            let i = d_model / 2;
            let angle = pos as f64 / (10000.0_f64).powf(2.0 * i as f64 / d_model as f64);
            data[pos * d_model + d_model - 1] = angle.sin();
        }
    }
    TensorValue::from_data(data, &[max_seq_len, d_model])
}

/// Learned positional encoding — trainable embedding.
///
/// A `[max_seq_len, d_model]` parameter matrix that is optimized during training.
#[derive(Debug, Clone)]
pub struct LearnedPositionalEncoding {
    /// Learnable position embedding `[max_seq_len, d_model]`.
    pub embedding: TensorValue,
}

impl LearnedPositionalEncoding {
    /// Creates a learned positional encoding with small random values.
    pub fn new(max_seq_len: usize, d_model: usize) -> Self {
        let scale = 0.02;
        let mut embedding = TensorValue::randn(&[max_seq_len, d_model]);
        embedding.set_requires_grad(true);
        *embedding.data_mut() *= scale;
        Self { embedding }
    }

    /// Returns the encoding for positions `[0, seq_len)`.
    pub fn get(&self, seq_len: usize) -> Result<TensorValue, TensorError> {
        if seq_len > self.embedding.shape()[0] {
            return Err(TensorError::InvalidData {
                reason: format!(
                    "seq_len ({seq_len}) exceeds max_seq_len ({})",
                    self.embedding.shape()[0]
                ),
            });
        }
        let d_model = self.embedding.shape()[1];
        let data = self.embedding.to_vec();
        let slice = &data[..seq_len * d_model];
        let mut tv = TensorValue::from_data(slice.to_vec(), &[seq_len, d_model])?;
        tv.set_requires_grad(true);
        Ok(tv)
    }

    /// Returns the learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.embedding]
    }

    /// Returns mutable learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.embedding]
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Layer Normalization
// ═══════════════════════════════════════════════════════════════════════

/// Layer normalization: normalize across feature dimension, scale by gamma, shift by beta.
///
/// `output = gamma * (x - mean) / sqrt(var + eps) + beta`
#[derive(Debug, Clone)]
pub struct LayerNorm {
    /// Scale parameter `[d_model]`.
    pub gamma: TensorValue,
    /// Shift parameter `[d_model]`.
    pub beta: TensorValue,
    /// Dimension being normalized.
    pub d_model: usize,
    /// Small constant for numerical stability.
    pub epsilon: f64,
}

impl LayerNorm {
    /// Creates a new LayerNorm with gamma=1, beta=0.
    pub fn new(d_model: usize) -> Self {
        Self::with_epsilon(d_model, 1e-5)
    }

    /// Creates a new LayerNorm with custom epsilon.
    pub fn with_epsilon(d_model: usize, epsilon: f64) -> Self {
        let mut gamma = TensorValue::ones(&[1, d_model]);
        gamma.set_requires_grad(true);
        let mut beta = TensorValue::zeros(&[1, d_model]);
        beta.set_requires_grad(true);
        Self {
            gamma,
            beta,
            d_model,
            epsilon,
        }
    }

    /// Forward pass: normalize input and apply affine transform.
    ///
    /// Input: `[seq_len, d_model]` → Output: `[seq_len, d_model]`.
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        let arr = to_array2(x)?;
        let gamma = to_array2(&self.gamma)?;
        let beta = to_array2(&self.beta)?;
        let out = layernorm_forward(&arr, &gamma, &beta, self.epsilon);
        from_array2(&out, x.requires_grad())
    }

    /// Backward pass: gradient through layer normalization.
    ///
    /// Returns `(d_x, d_gamma, d_beta)`.
    pub fn backward(
        &self,
        x: &TensorValue,
        d_output: &Array2<f64>,
    ) -> Result<LayerNormGradients, TensorError> {
        let arr = to_array2(x)?;
        let gamma = to_array2(&self.gamma)?;
        let (d_x, d_gamma, d_beta) = layernorm_backward(&arr, d_output, &gamma, self.epsilon);
        Ok(LayerNormGradients {
            d_x,
            d_gamma,
            d_beta,
        })
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.gamma, &self.beta]
    }

    /// Returns mutable learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.gamma, &mut self.beta]
    }
}

/// Gradients for LayerNorm.
#[derive(Debug, Clone)]
pub struct LayerNormGradients {
    /// Gradient w.r.t. input.
    pub d_x: Array2<f64>,
    /// Gradient w.r.t. gamma.
    pub d_gamma: Array2<f64>,
    /// Gradient w.r.t. beta.
    pub d_beta: Array2<f64>,
}

/// Layer normalization forward pass.
fn layernorm_forward(
    x: &Array2<f64>,
    gamma: &Array2<f64>,
    beta: &Array2<f64>,
    epsilon: f64,
) -> Array2<f64> {
    let seq_len = x.nrows();
    let d = x.ncols();
    let mut out = Array2::zeros((seq_len, d));

    for r in 0..seq_len {
        let row = x.row(r);
        let mean = row.mean().unwrap_or(0.0);
        let var = row.iter().map(|&v| (v - mean) * (v - mean)).sum::<f64>() / d as f64;
        let inv_std = 1.0 / (var + epsilon).sqrt();
        for c in 0..d {
            let norm = (x[[r, c]] - mean) * inv_std;
            out[[r, c]] = gamma[[0, c]] * norm + beta[[0, c]];
        }
    }
    out
}

/// Layer normalization backward pass.
fn layernorm_backward(
    x: &Array2<f64>,
    d_out: &Array2<f64>,
    gamma: &Array2<f64>,
    epsilon: f64,
) -> (Array2<f64>, Array2<f64>, Array2<f64>) {
    let seq_len = x.nrows();
    let d = x.ncols();
    let mut d_x = Array2::zeros((seq_len, d));
    let mut d_gamma = Array2::zeros((1, d));
    let mut d_beta = Array2::zeros((1, d));

    for r in 0..seq_len {
        let row = x.row(r);
        let mean = row.mean().unwrap_or(0.0);
        let var = row.iter().map(|&v| (v - mean) * (v - mean)).sum::<f64>() / d as f64;
        let inv_std = 1.0 / (var + epsilon).sqrt();

        // Compute normalized values and accumulate d_gamma, d_beta
        let mut x_hat = vec![0.0; d];
        for c in 0..d {
            x_hat[c] = (x[[r, c]] - mean) * inv_std;
            d_gamma[[0, c]] += d_out[[r, c]] * x_hat[c];
            d_beta[[0, c]] += d_out[[r, c]];
        }

        // d_x through normalization
        let d_xhat: Vec<f64> = (0..d).map(|c| d_out[[r, c]] * gamma[[0, c]]).collect();
        let sum_dxhat: f64 = d_xhat.iter().sum();
        let sum_dxhat_xhat: f64 = d_xhat.iter().zip(&x_hat).map(|(a, b)| a * b).sum();
        let n = d as f64;
        for c in 0..d {
            d_x[[r, c]] = inv_std / n * (n * d_xhat[c] - sum_dxhat - x_hat[c] * sum_dxhat_xhat);
        }
    }

    (d_x, d_gamma, d_beta)
}

// ═══════════════════════════════════════════════════════════════════════
// Feed-Forward Network
// ═══════════════════════════════════════════════════════════════════════

/// Position-wise feed-forward network: two linear layers with GELU activation.
///
/// `FFN(x) = Linear2(GELU(Linear1(x)))`
/// - Linear1: `d_model → d_ff`
/// - Linear2: `d_ff → d_model`
#[derive(Debug, Clone)]
pub struct FeedForward {
    /// First linear weight `[d_model, d_ff]`.
    pub w1: TensorValue,
    /// First linear bias `[1, d_ff]`.
    pub b1: TensorValue,
    /// Second linear weight `[d_ff, d_model]`.
    pub w2: TensorValue,
    /// Second linear bias `[1, d_model]`.
    pub b2: TensorValue,
    /// Inner dimension.
    pub d_ff: usize,
    /// Model dimension.
    pub d_model: usize,
    /// Dropout rate (stored but applied externally).
    pub dropout_rate: f64,
}

impl FeedForward {
    /// Creates a new FFN with Xavier-initialized weights.
    pub fn new(d_model: usize, d_ff: usize, dropout_rate: f64) -> Self {
        let scale1 = (2.0 / (d_model + d_ff) as f64).sqrt();
        let scale2 = (2.0 / (d_ff + d_model) as f64).sqrt();

        let w1 = create_weight(d_model, d_ff, scale1);
        let mut b1 = TensorValue::zeros(&[1, d_ff]);
        b1.set_requires_grad(true);
        let w2 = create_weight(d_ff, d_model, scale2);
        let mut b2 = TensorValue::zeros(&[1, d_model]);
        b2.set_requires_grad(true);

        Self {
            w1,
            b1,
            w2,
            b2,
            d_ff,
            d_model,
            dropout_rate,
        }
    }

    /// Forward pass: `x -> Linear1 -> GELU -> Linear2`.
    ///
    /// Input: `[seq_len, d_model]` → Output: `[seq_len, d_model]`.
    pub fn forward(&self, x: &TensorValue) -> Result<TensorValue, TensorError> {
        let arr = to_array2(x)?;
        let w1 = to_array2(&self.w1)?;
        let b1 = to_array2(&self.b1)?;
        let w2 = to_array2(&self.w2)?;
        let b2 = to_array2(&self.b2)?;
        let out = ffn_forward(&arr, &w1, &b1, &w2, &b2);
        from_array2(&out, x.requires_grad())
    }

    /// Backward pass: computes gradients for w1, b1, w2, b2.
    pub fn backward(
        &self,
        x: &TensorValue,
        d_output: &Array2<f64>,
    ) -> Result<FFNGradients, TensorError> {
        let arr = to_array2(x)?;
        let w1 = to_array2(&self.w1)?;
        let b1 = to_array2(&self.b1)?;
        let w2 = to_array2(&self.w2)?;

        // Forward intermediates
        let hidden = &arr.dot(&w1) + &b1;
        let activated = gelu_array(&hidden);

        // d_activated = d_output @ W2^T
        let d_activated = d_output.dot(&w2.t());

        // d_w2 = activated^T @ d_output
        let d_w2 = activated.t().dot(d_output);
        let d_b2 = d_output.sum_axis(Axis(0)).insert_axis(Axis(0));

        // d_hidden = d_activated * gelu'(hidden)
        let d_hidden = gelu_backward_array(&hidden, &d_activated);

        // d_w1 = x^T @ d_hidden
        let d_w1 = arr.t().dot(&d_hidden);
        let d_b1 = d_hidden.sum_axis(Axis(0)).insert_axis(Axis(0));

        // d_x = d_hidden @ W1^T
        let d_x = d_hidden.dot(&w1.t());

        Ok(FFNGradients {
            d_w1,
            d_b1,
            d_w2,
            d_b2,
            d_x,
        })
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        vec![&self.w1, &self.b1, &self.w2, &self.b2]
    }

    /// Returns mutable learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        vec![&mut self.w1, &mut self.b1, &mut self.w2, &mut self.b2]
    }

    /// Returns the total number of learnable parameters.
    pub fn param_count(&self) -> usize {
        self.w1.numel() + self.b1.numel() + self.w2.numel() + self.b2.numel()
    }
}

/// Gradients for FeedForward.
#[derive(Debug, Clone)]
pub struct FFNGradients {
    /// Gradient for first weight.
    pub d_w1: Array2<f64>,
    /// Gradient for first bias.
    pub d_b1: Array2<f64>,
    /// Gradient for second weight.
    pub d_w2: Array2<f64>,
    /// Gradient for second bias.
    pub d_b2: Array2<f64>,
    /// Gradient w.r.t. input.
    pub d_x: Array2<f64>,
}

/// FFN forward: x -> w1 -> GELU -> w2.
fn ffn_forward(
    x: &Array2<f64>,
    w1: &Array2<f64>,
    b1: &Array2<f64>,
    w2: &Array2<f64>,
    b2: &Array2<f64>,
) -> Array2<f64> {
    let hidden = &x.dot(w1) + b1;
    let activated = gelu_array(&hidden);
    &activated.dot(w2) + b2
}

/// GELU backward: element-wise derivative of GELU.
fn gelu_backward_array(x: &Array2<f64>, d_out: &Array2<f64>) -> Array2<f64> {
    let sqrt_2_over_pi = (2.0 / PI).sqrt();
    x.mapv(|v| {
        let cdf = 0.5 * (1.0 + (sqrt_2_over_pi * (v + 0.044715 * v * v * v)).tanh());
        let pdf_part = sqrt_2_over_pi * (1.0 + 3.0 * 0.044715 * v * v);
        let sech2 = {
            let t = (sqrt_2_over_pi * (v + 0.044715 * v * v * v)).tanh();
            1.0 - t * t
        };
        cdf + 0.5 * v * pdf_part * sech2
    }) * d_out
}

// ═══════════════════════════════════════════════════════════════════════
// Transformer Encoder Layer
// ═══════════════════════════════════════════════════════════════════════

/// Single transformer encoder layer: self-attention → add&norm → FFN → add&norm.
///
/// Supports both pre-norm (GPT-style) and post-norm (BERT-style) configurations.
#[derive(Debug, Clone)]
pub struct TransformerEncoderLayer {
    /// Multi-head self-attention.
    pub self_attn: TransformerMHA,
    /// Feed-forward network.
    pub ffn: FeedForward,
    /// First layer norm (around attention).
    pub norm1: LayerNorm,
    /// Second layer norm (around FFN).
    pub norm2: LayerNorm,
    /// If true, normalize before sublayer (GPT-style); else after (BERT-style).
    pub pre_norm: bool,
}

impl TransformerEncoderLayer {
    /// Creates a new encoder layer.
    pub fn new(config: &TransformerConfig) -> Result<Self, TensorError> {
        let self_attn = TransformerMHA::new(config.d_model, config.n_heads)?;
        let ffn = FeedForward::new(config.d_model, config.d_ff, config.dropout);
        let norm1 = LayerNorm::new(config.d_model);
        let norm2 = LayerNorm::new(config.d_model);
        Ok(Self {
            self_attn,
            ffn,
            norm1,
            norm2,
            pre_norm: config.pre_norm,
        })
    }

    /// Forward pass through the encoder layer.
    ///
    /// Input: `[seq_len, d_model]` → Output: `[seq_len, d_model]`.
    pub fn forward(
        &self,
        src: &TensorValue,
        mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        if self.pre_norm {
            self.forward_pre_norm(src, mask)
        } else {
            self.forward_post_norm(src, mask)
        }
    }

    /// Pre-norm: norm -> sublayer -> residual.
    fn forward_pre_norm(
        &self,
        src: &TensorValue,
        mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        // Attention block: x + attn(norm(x))
        let normed = self.norm1.forward(src)?;
        let attn_out = self.self_attn.forward(&normed, &normed, &normed, mask)?;
        let residual1 = add_tensors(src, &attn_out)?;

        // FFN block: x + ffn(norm(x))
        let normed2 = self.norm2.forward(&residual1)?;
        let ffn_out = self.ffn.forward(&normed2)?;
        add_tensors(&residual1, &ffn_out)
    }

    /// Post-norm: sublayer -> residual -> norm.
    fn forward_post_norm(
        &self,
        src: &TensorValue,
        mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        // Attention block: norm(x + attn(x))
        let attn_out = self.self_attn.forward(src, src, src, mask)?;
        let residual1 = add_tensors(src, &attn_out)?;
        let normed1 = self.norm1.forward(&residual1)?;

        // FFN block: norm(x + ffn(x))
        let ffn_out = self.ffn.forward(&normed1)?;
        let residual2 = add_tensors(&normed1, &ffn_out)?;
        self.norm2.forward(&residual2)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        let mut params = self.self_attn.parameters();
        params.extend(self.ffn.parameters());
        params.extend(self.norm1.parameters());
        params.extend(self.norm2.parameters());
        params
    }

    /// Returns mutable learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        let mut params = self.self_attn.parameters_mut();
        params.extend(self.ffn.parameters_mut());
        params.extend(self.norm1.parameters_mut());
        params.extend(self.norm2.parameters_mut());
        params
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Transformer Decoder Layer
// ═══════════════════════════════════════════════════════════════════════

/// Single transformer decoder layer:
/// masked self-attention → add&norm → cross-attention → add&norm → FFN → add&norm.
#[derive(Debug, Clone)]
pub struct TransformerDecoderLayer {
    /// Masked multi-head self-attention.
    pub self_attn: TransformerMHA,
    /// Cross-attention (Q=decoder, KV=encoder).
    pub cross_attn: TransformerMHA,
    /// Feed-forward network.
    pub ffn: FeedForward,
    /// Norm around self-attention.
    pub norm1: LayerNorm,
    /// Norm around cross-attention.
    pub norm2: LayerNorm,
    /// Norm around FFN.
    pub norm3: LayerNorm,
    /// Pre-norm or post-norm.
    pub pre_norm: bool,
}

impl TransformerDecoderLayer {
    /// Creates a new decoder layer.
    pub fn new(config: &TransformerConfig) -> Result<Self, TensorError> {
        let self_attn = TransformerMHA::new(config.d_model, config.n_heads)?;
        let cross_attn = TransformerMHA::new(config.d_model, config.n_heads)?;
        let ffn = FeedForward::new(config.d_model, config.d_ff, config.dropout);
        let norm1 = LayerNorm::new(config.d_model);
        let norm2 = LayerNorm::new(config.d_model);
        let norm3 = LayerNorm::new(config.d_model);
        Ok(Self {
            self_attn,
            cross_attn,
            ffn,
            norm1,
            norm2,
            norm3,
            pre_norm: config.pre_norm,
        })
    }

    /// Forward pass through the decoder layer.
    ///
    /// - `tgt`: decoder input `[seq_tgt, d_model]`
    /// - `memory`: encoder output `[seq_src, d_model]`
    /// - `tgt_mask`: mask for target self-attention (typically causal)
    /// - `memory_mask`: mask for cross-attention
    pub fn forward(
        &self,
        tgt: &TensorValue,
        memory: &TensorValue,
        tgt_mask: &AttentionMask,
        memory_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        if self.pre_norm {
            self.forward_pre_norm(tgt, memory, tgt_mask, memory_mask)
        } else {
            self.forward_post_norm(tgt, memory, tgt_mask, memory_mask)
        }
    }

    /// Pre-norm decoder forward.
    fn forward_pre_norm(
        &self,
        tgt: &TensorValue,
        memory: &TensorValue,
        tgt_mask: &AttentionMask,
        memory_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        // Masked self-attention
        let n1 = self.norm1.forward(tgt)?;
        let sa = self.self_attn.forward(&n1, &n1, &n1, tgt_mask)?;
        let r1 = add_tensors(tgt, &sa)?;

        // Cross-attention (Q=decoder, KV=encoder)
        let n2 = self.norm2.forward(&r1)?;
        let ca = self.cross_attn.forward(&n2, memory, memory, memory_mask)?;
        let r2 = add_tensors(&r1, &ca)?;

        // FFN
        let n3 = self.norm3.forward(&r2)?;
        let ff = self.ffn.forward(&n3)?;
        add_tensors(&r2, &ff)
    }

    /// Post-norm decoder forward.
    fn forward_post_norm(
        &self,
        tgt: &TensorValue,
        memory: &TensorValue,
        tgt_mask: &AttentionMask,
        memory_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        // Masked self-attention
        let sa = self.self_attn.forward(tgt, tgt, tgt, tgt_mask)?;
        let r1 = add_tensors(tgt, &sa)?;
        let n1 = self.norm1.forward(&r1)?;

        // Cross-attention
        let ca = self.cross_attn.forward(&n1, memory, memory, memory_mask)?;
        let r2 = add_tensors(&n1, &ca)?;
        let n2 = self.norm2.forward(&r2)?;

        // FFN
        let ff = self.ffn.forward(&n2)?;
        let r3 = add_tensors(&n2, &ff)?;
        self.norm3.forward(&r3)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        let mut params = self.self_attn.parameters();
        params.extend(self.cross_attn.parameters());
        params.extend(self.ffn.parameters());
        params.extend(self.norm1.parameters());
        params.extend(self.norm2.parameters());
        params.extend(self.norm3.parameters());
        params
    }

    /// Returns mutable learnable parameters.
    pub fn parameters_mut(&mut self) -> Vec<&mut TensorValue> {
        let mut params = self.self_attn.parameters_mut();
        params.extend(self.cross_attn.parameters_mut());
        params.extend(self.ffn.parameters_mut());
        params.extend(self.norm1.parameters_mut());
        params.extend(self.norm2.parameters_mut());
        params.extend(self.norm3.parameters_mut());
        params
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Transformer Encoder (stack of layers)
// ═══════════════════════════════════════════════════════════════════════

/// Stack of N transformer encoder layers with optional final LayerNorm.
#[derive(Debug, Clone)]
pub struct TransformerEncoder {
    /// Encoder layers.
    pub layers: Vec<TransformerEncoderLayer>,
    /// Optional final normalization.
    pub final_norm: Option<LayerNorm>,
}

impl TransformerEncoder {
    /// Creates a transformer encoder with N layers.
    pub fn new(config: &TransformerConfig) -> Result<Self, TensorError> {
        let mut layers = Vec::with_capacity(config.n_layers);
        for _ in 0..config.n_layers {
            layers.push(TransformerEncoderLayer::new(config)?);
        }
        let final_norm = if config.pre_norm {
            Some(LayerNorm::new(config.d_model))
        } else {
            None
        };
        Ok(Self { layers, final_norm })
    }

    /// Forward pass through all encoder layers.
    ///
    /// Input: `[seq_len, d_model]` → Output: `[seq_len, d_model]`.
    pub fn forward(
        &self,
        src: &TensorValue,
        mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        let mut out = src.clone();
        for layer in &self.layers {
            out = layer.forward(&out, mask)?;
        }
        if let Some(norm) = &self.final_norm {
            out = norm.forward(&out)?;
        }
        Ok(out)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        let mut params = Vec::new();
        for layer in &self.layers {
            params.extend(layer.parameters());
        }
        if let Some(norm) = &self.final_norm {
            params.extend(norm.parameters());
        }
        params
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Transformer Decoder (stack of layers)
// ═══════════════════════════════════════════════════════════════════════

/// Stack of N transformer decoder layers.
#[derive(Debug, Clone)]
pub struct TransformerDecoder {
    /// Decoder layers.
    pub layers: Vec<TransformerDecoderLayer>,
    /// Optional final normalization.
    pub final_norm: Option<LayerNorm>,
}

impl TransformerDecoder {
    /// Creates a transformer decoder with N layers.
    pub fn new(config: &TransformerConfig) -> Result<Self, TensorError> {
        let mut layers = Vec::with_capacity(config.n_layers);
        for _ in 0..config.n_layers {
            layers.push(TransformerDecoderLayer::new(config)?);
        }
        let final_norm = if config.pre_norm {
            Some(LayerNorm::new(config.d_model))
        } else {
            None
        };
        Ok(Self { layers, final_norm })
    }

    /// Forward pass through all decoder layers.
    ///
    /// - `tgt`: `[seq_tgt, d_model]`
    /// - `memory`: encoder output `[seq_src, d_model]`
    /// - `tgt_mask`: causal mask for target
    /// - `memory_mask`: mask for cross-attention
    pub fn forward(
        &self,
        tgt: &TensorValue,
        memory: &TensorValue,
        tgt_mask: &AttentionMask,
        memory_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        let mut out = tgt.clone();
        for layer in &self.layers {
            out = layer.forward(&out, memory, tgt_mask, memory_mask)?;
        }
        if let Some(norm) = &self.final_norm {
            out = norm.forward(&out)?;
        }
        Ok(out)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        let mut params = Vec::new();
        for layer in &self.layers {
            params.extend(layer.parameters());
        }
        if let Some(norm) = &self.final_norm {
            params.extend(norm.parameters());
        }
        params
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TransformerConfig
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for a Transformer model.
#[derive(Debug, Clone)]
pub struct TransformerConfig {
    /// Model dimension (embedding size).
    pub d_model: usize,
    /// Number of attention heads.
    pub n_heads: usize,
    /// Number of encoder/decoder layers.
    pub n_layers: usize,
    /// Feed-forward inner dimension.
    pub d_ff: usize,
    /// Dropout rate.
    pub dropout: f64,
    /// Maximum sequence length.
    pub max_seq_len: usize,
    /// Vocabulary size (for embedding layer).
    pub vocab_size: usize,
    /// Pre-norm (true = GPT-style) vs post-norm (false = BERT-style).
    pub pre_norm: bool,
}

impl TransformerConfig {
    /// Creates a default small transformer configuration.
    pub fn small() -> Self {
        Self {
            d_model: 64,
            n_heads: 4,
            n_layers: 2,
            d_ff: 256,
            dropout: 0.1,
            max_seq_len: 128,
            vocab_size: 1000,
            pre_norm: true,
        }
    }

    /// Creates a custom transformer configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn custom(
        d_model: usize,
        n_heads: usize,
        n_layers: usize,
        d_ff: usize,
        dropout: f64,
        max_seq_len: usize,
        vocab_size: usize,
        pre_norm: bool,
    ) -> Self {
        Self {
            d_model,
            n_heads,
            n_layers,
            d_ff,
            dropout,
            max_seq_len,
            vocab_size,
            pre_norm,
        }
    }
}

impl Default for TransformerConfig {
    fn default() -> Self {
        Self::small()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Full Transformer (Encoder + Decoder + Embeddings)
// ═══════════════════════════════════════════════════════════════════════

/// Complete Transformer model: encoder + decoder + embeddings + output projection.
///
/// Architecture:
/// 1. Source embedding + positional encoding → Encoder
/// 2. Target embedding + positional encoding → Decoder (with encoder memory)
/// 3. Output projection → logits `[seq_tgt, vocab_size]`
#[derive(Debug, Clone)]
pub struct Transformer {
    /// Encoder stack.
    pub encoder: TransformerEncoder,
    /// Decoder stack.
    pub decoder: TransformerDecoder,
    /// Source token embedding `[vocab_size, d_model]`.
    pub src_embedding: TensorValue,
    /// Target token embedding `[vocab_size, d_model]`.
    pub tgt_embedding: TensorValue,
    /// Positional encoding.
    pub pos_encoding: SinusoidalPositionalEncoding,
    /// Output projection `[d_model, vocab_size]`.
    pub output_proj: TensorValue,
    /// Configuration.
    pub config: TransformerConfig,
}

impl Transformer {
    /// Creates a complete transformer from configuration.
    pub fn new(config: TransformerConfig) -> Result<Self, TensorError> {
        let encoder = TransformerEncoder::new(&config)?;
        let decoder = TransformerDecoder::new(&config)?;

        let embed_scale = (1.0 / config.d_model as f64).sqrt();
        let src_embedding = create_weight(config.vocab_size, config.d_model, embed_scale);
        let tgt_embedding = create_weight(config.vocab_size, config.d_model, embed_scale);

        let pos_encoding = SinusoidalPositionalEncoding::new(config.max_seq_len, config.d_model)?;

        let proj_scale = (2.0 / (config.d_model + config.vocab_size) as f64).sqrt();
        let output_proj = create_weight(config.d_model, config.vocab_size, proj_scale);

        Ok(Self {
            encoder,
            decoder,
            src_embedding,
            tgt_embedding,
            pos_encoding,
            output_proj,
            config,
        })
    }

    /// Encodes source token indices into memory representation.
    ///
    /// - `src_tokens`: list of token indices (each in `[0, vocab_size)`)
    /// - `src_mask`: source attention mask
    ///
    /// Returns encoder memory `[seq_src, d_model]`.
    pub fn encode(
        &self,
        src_tokens: &[usize],
        src_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        let embedded = embed_tokens(src_tokens, &self.src_embedding)?;
        let pos = self.pos_encoding.get(src_tokens.len())?;
        let input = add_tensors(&embedded, &pos)?;
        self.encoder.forward(&input, src_mask)
    }

    /// Decodes target tokens given encoder memory.
    ///
    /// - `tgt_tokens`: list of target token indices
    /// - `memory`: encoder output
    /// - `tgt_mask`: target mask (typically causal)
    /// - `memory_mask`: cross-attention mask
    ///
    /// Returns logits `[seq_tgt, vocab_size]`.
    pub fn decode(
        &self,
        tgt_tokens: &[usize],
        memory: &TensorValue,
        tgt_mask: &AttentionMask,
        memory_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        let embedded = embed_tokens(tgt_tokens, &self.tgt_embedding)?;
        let pos = self.pos_encoding.get(tgt_tokens.len())?;
        let input = add_tensors(&embedded, &pos)?;
        let decoded = self
            .decoder
            .forward(&input, memory, tgt_mask, memory_mask)?;
        // Output projection: logits = decoded @ output_proj
        let dec_arr = to_array2(&decoded)?;
        let proj_arr = to_array2(&self.output_proj)?;
        let logits = dec_arr.dot(&proj_arr);
        from_array2(&logits, true)
    }

    /// Full forward pass: encode source, decode target, produce logits.
    pub fn forward(
        &self,
        src_tokens: &[usize],
        tgt_tokens: &[usize],
        src_mask: &AttentionMask,
        tgt_mask: &AttentionMask,
    ) -> Result<TensorValue, TensorError> {
        let memory = self.encode(src_tokens, src_mask)?;
        self.decode(tgt_tokens, &memory, tgt_mask, &AttentionMask::None)
    }

    /// Returns all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        let mut params = self.encoder.parameters();
        params.extend(self.decoder.parameters());
        params.push(&self.src_embedding);
        params.push(&self.tgt_embedding);
        params.push(&self.output_proj);
        params
    }

    /// Returns the total parameter count.
    pub fn param_count(&self) -> usize {
        self.parameters().iter().map(|p| p.numel()).sum()
    }
}

/// Embeds token indices by looking up rows in the embedding matrix.
///
/// Returns `[seq_len, d_model]`.
fn embed_tokens(tokens: &[usize], embedding: &TensorValue) -> Result<TensorValue, TensorError> {
    let vocab_size = embedding.shape()[0];
    let d_model = embedding.shape()[1];
    let embed_data = embedding.to_vec();
    let seq_len = tokens.len();

    let mut data = vec![0.0; seq_len * d_model];
    for (i, &tok) in tokens.iter().enumerate() {
        if tok >= vocab_size {
            return Err(TensorError::InvalidData {
                reason: format!("token index {tok} >= vocab_size {vocab_size}"),
            });
        }
        let offset = tok * d_model;
        data[i * d_model..(i + 1) * d_model].copy_from_slice(&embed_data[offset..offset + d_model]);
    }
    TensorValue::from_data(data, &[seq_len, d_model])
}

/// Element-wise addition of two tensors with same shape.
fn add_tensors(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError> {
    let result = a.data() + b.data();
    Ok(TensorValue::new(
        result,
        a.requires_grad() || b.requires_grad(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 13: Attention & Positional Encoding ──

    #[test]
    fn s13_1_scaled_dot_product_attention_shapes() {
        let seq_len = 4;
        let d_k = 8;
        let d_v = 8;
        let q = Array2::from_shape_fn((seq_len, d_k), |(i, j)| (i * d_k + j) as f64 * 0.1);
        let k = Array2::from_shape_fn((seq_len, d_k), |(i, j)| (i * d_k + j) as f64 * 0.1);
        let v = Array2::from_shape_fn((seq_len, d_v), |(i, j)| (i * d_v + j) as f64 * 0.1);

        let result = scaled_dot_product_attention(&q, &k, &v, &AttentionMask::None);
        assert_eq!(result.output.shape(), &[seq_len, d_v]);
        assert_eq!(result.weights.shape(), &[seq_len, seq_len]);
    }

    #[test]
    fn s13_2_attention_weights_sum_to_one() {
        let q = Array2::from_shape_fn((3, 4), |(i, j)| ((i + j) as f64) * 0.2);
        let k = Array2::from_shape_fn((3, 4), |(i, j)| ((i + j) as f64) * 0.15);
        let v = Array2::ones((3, 4));

        let result = scaled_dot_product_attention(&q, &k, &v, &AttentionMask::None);
        for r in 0..3 {
            let row_sum: f64 = result.weights.row(r).sum();
            assert!(
                (row_sum - 1.0).abs() < 1e-10,
                "row {r} sums to {row_sum}, expected 1.0"
            );
        }
    }

    #[test]
    fn s13_3_causal_mask_prevents_future_attention() {
        let seq_len = 4;
        let q = Array2::ones((seq_len, 4));
        let k = Array2::ones((seq_len, 4));
        let v = Array2::ones((seq_len, 4));

        let result = scaled_dot_product_attention(&q, &k, &v, &AttentionMask::Causal(seq_len));
        // Position 0 should only attend to position 0
        assert!(result.weights[[0, 1]].abs() < 1e-10);
        assert!(result.weights[[0, 2]].abs() < 1e-10);
        // Position 1 should attend to 0,1 but not 2,3
        assert!(result.weights[[1, 2]].abs() < 1e-10);
    }

    #[test]
    fn s13_4_padding_mask_zeros_masked_positions() {
        let q = Array2::ones((3, 4));
        let k = Array2::ones((3, 4));
        let v = Array2::from_shape_fn((3, 4), |(i, j)| (i * 4 + j) as f64);

        // Mask out position 2
        let mask = AttentionMask::Padding(vec![false, false, true]);
        let result = scaled_dot_product_attention(&q, &k, &v, &mask);
        // Position 2 should have ~0 attention weight
        for r in 0..3 {
            assert!(
                result.weights[[r, 2]].abs() < 1e-10,
                "row {r}, col 2 weight should be ~0"
            );
        }
    }

    #[test]
    fn s13_5_transformer_mha_forward_shapes() {
        let d_model = 16;
        let n_heads = 4;
        let seq_len = 6;

        let mha = TransformerMHA::new(d_model, n_heads).unwrap();
        let q = TensorValue::randn(&[seq_len, d_model]);
        let k = TensorValue::randn(&[seq_len, d_model]);
        let v = TensorValue::randn(&[seq_len, d_model]);

        let out = mha.forward(&q, &k, &v, &AttentionMask::None).unwrap();
        assert_eq!(out.shape(), &[seq_len, d_model]);
    }

    #[test]
    fn s13_6_mha_rejects_indivisible_d_model() {
        let result = TransformerMHA::new(15, 4);
        assert!(result.is_err());
    }

    #[test]
    fn s13_7_sinusoidal_positional_encoding_shape() {
        let max_seq = 100;
        let d_model = 32;
        let pe = SinusoidalPositionalEncoding::new(max_seq, d_model).unwrap();
        assert_eq!(pe.encoding.shape(), &[max_seq, d_model]);

        let slice = pe.get(10).unwrap();
        assert_eq!(slice.shape(), &[10, d_model]);
    }

    #[test]
    fn s13_8_sinusoidal_encoding_values_bounded() {
        let pe = SinusoidalPositionalEncoding::new(50, 16).unwrap();
        let data = pe.encoding.to_vec();
        for &v in &data {
            assert!(
                (-1.0..=1.0).contains(&v),
                "PE value {v} should be in [-1, 1]"
            );
        }
    }

    #[test]
    fn s13_9_layer_norm_output_shape_and_stats() {
        let d_model = 8;
        let seq_len = 4;
        let ln = LayerNorm::new(d_model);
        let x = TensorValue::randn(&[seq_len, d_model]);
        let out = ln.forward(&x).unwrap();
        assert_eq!(out.shape(), &[seq_len, d_model]);

        // Each row should have approximately mean=0, var=1 (with gamma=1, beta=0)
        let arr = to_array2(&out).unwrap();
        for r in 0..seq_len {
            let row = arr.row(r);
            let mean = row.mean().unwrap_or(0.0);
            assert!(mean.abs() < 0.1, "row {r} mean = {mean}, expected ~0");
        }
    }

    #[test]
    fn s13_10_learned_positional_encoding() {
        let lpe = LearnedPositionalEncoding::new(50, 16);
        assert_eq!(lpe.embedding.shape(), &[50, 16]);
        assert!(lpe.embedding.requires_grad());

        let slice = lpe.get(10).unwrap();
        assert_eq!(slice.shape(), &[10, 16]);
        assert!(slice.requires_grad());

        // Out of bounds should error
        assert!(lpe.get(100).is_err());
    }

    // ── Sprint 14: Encoder, Decoder, Transformer ──

    #[test]
    fn s14_1_feed_forward_shapes() {
        let d_model = 16;
        let d_ff = 64;
        let seq_len = 5;
        let ffn = FeedForward::new(d_model, d_ff, 0.1);
        let x = TensorValue::randn(&[seq_len, d_model]);
        let out = ffn.forward(&x).unwrap();
        assert_eq!(out.shape(), &[seq_len, d_model]);
        assert_eq!(
            ffn.param_count(),
            d_model * d_ff + d_ff + d_ff * d_model + d_model
        );
    }

    #[test]
    fn s14_2_feed_forward_backward_shapes() {
        let d_model = 8;
        let d_ff = 32;
        let seq_len = 3;
        let ffn = FeedForward::new(d_model, d_ff, 0.0);
        let x = TensorValue::randn(&[seq_len, d_model]);
        let d_out = Array2::ones((seq_len, d_model));
        let grads = ffn.backward(&x, &d_out).unwrap();
        assert_eq!(grads.d_w1.shape(), &[d_model, d_ff]);
        assert_eq!(grads.d_w2.shape(), &[d_ff, d_model]);
        assert_eq!(grads.d_x.shape(), &[seq_len, d_model]);
    }

    #[test]
    fn s14_3_encoder_layer_forward() {
        let config = TransformerConfig::custom(16, 4, 1, 64, 0.0, 128, 100, true);
        let layer = TransformerEncoderLayer::new(&config).unwrap();
        let x = TensorValue::randn(&[5, 16]);
        let out = layer.forward(&x, &AttentionMask::None).unwrap();
        assert_eq!(out.shape(), &[5, 16]);
    }

    #[test]
    fn s14_4_encoder_layer_post_norm() {
        let config = TransformerConfig::custom(16, 4, 1, 64, 0.0, 128, 100, false);
        let layer = TransformerEncoderLayer::new(&config).unwrap();
        let x = TensorValue::randn(&[5, 16]);
        let out = layer.forward(&x, &AttentionMask::None).unwrap();
        assert_eq!(out.shape(), &[5, 16]);
    }

    #[test]
    fn s14_5_decoder_layer_forward() {
        let config = TransformerConfig::custom(16, 4, 1, 64, 0.0, 128, 100, true);
        let layer = TransformerDecoderLayer::new(&config).unwrap();
        let tgt = TensorValue::randn(&[4, 16]);
        let memory = TensorValue::randn(&[6, 16]);
        let tgt_mask = AttentionMask::Causal(4);
        let out = layer
            .forward(&tgt, &memory, &tgt_mask, &AttentionMask::None)
            .unwrap();
        assert_eq!(out.shape(), &[4, 16]);
    }

    #[test]
    fn s14_6_encoder_stack_forward() {
        let config = TransformerConfig::custom(16, 4, 3, 64, 0.0, 128, 100, true);
        let encoder = TransformerEncoder::new(&config).unwrap();
        assert_eq!(encoder.layers.len(), 3);
        let x = TensorValue::randn(&[5, 16]);
        let out = encoder.forward(&x, &AttentionMask::None).unwrap();
        assert_eq!(out.shape(), &[5, 16]);
    }

    #[test]
    fn s14_7_decoder_stack_forward() {
        let config = TransformerConfig::custom(16, 4, 2, 64, 0.0, 128, 100, true);
        let decoder = TransformerDecoder::new(&config).unwrap();
        assert_eq!(decoder.layers.len(), 2);
        let tgt = TensorValue::randn(&[4, 16]);
        let memory = TensorValue::randn(&[6, 16]);
        let out = decoder
            .forward(
                &tgt,
                &memory,
                &AttentionMask::Causal(4),
                &AttentionMask::None,
            )
            .unwrap();
        assert_eq!(out.shape(), &[4, 16]);
    }

    #[test]
    fn s14_8_transformer_config_defaults() {
        let config = TransformerConfig::small();
        assert_eq!(config.d_model, 64);
        assert_eq!(config.n_heads, 4);
        assert_eq!(config.n_layers, 2);
        assert_eq!(config.d_ff, 256);
        assert!(config.pre_norm);
    }

    #[test]
    fn s14_9_full_transformer_forward() {
        let config = TransformerConfig::custom(16, 4, 1, 64, 0.0, 128, 50, true);
        let transformer = Transformer::new(config).unwrap();
        let src_tokens = vec![1, 5, 10, 3];
        let tgt_tokens = vec![2, 7, 15];
        let logits = transformer
            .forward(
                &src_tokens,
                &tgt_tokens,
                &AttentionMask::None,
                &AttentionMask::Causal(3),
            )
            .unwrap();
        assert_eq!(logits.shape(), &[3, 50]); // [tgt_seq, vocab_size]
    }

    #[test]
    fn s14_10_transformer_param_count() {
        let config = TransformerConfig::custom(16, 4, 1, 64, 0.0, 32, 50, true);
        let transformer = Transformer::new(config).unwrap();
        let count = transformer.param_count();
        // Should have embeddings + encoder params + decoder params + output proj
        assert!(count > 0);
        // src_embed(50*16) + tgt_embed(50*16) + output_proj(16*50)
        // + encoder(attn 4*16*16 + ffn 16*64+64+64*16+16 + 2*norm 2*(16+16))
        // + decoder(2*attn + ffn + 3*norm)
        // + final_norm(16+16)
        assert!(count > 5000, "param count {count} seems too low");
    }
}
