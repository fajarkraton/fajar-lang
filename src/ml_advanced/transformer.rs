//! Transformer Inference — multi-head attention, causal masking, RoPE,
//! KV cache, flash attention, GQA, RMSNorm, SwiGLU, token sampling.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S13.1: Multi-Head Self-Attention
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for multi-head attention.
#[derive(Debug, Clone)]
pub struct AttentionConfig {
    /// Model dimension.
    pub d_model: usize,
    /// Number of attention heads.
    pub num_heads: usize,
    /// Number of KV heads (for GQA; equals num_heads for MHA).
    pub num_kv_heads: usize,
    /// Head dimension (d_model / num_heads).
    pub head_dim: usize,
}

impl AttentionConfig {
    /// Creates a standard MHA config.
    pub fn new(d_model: usize, num_heads: usize) -> Self {
        AttentionConfig {
            d_model,
            num_heads,
            num_kv_heads: num_heads,
            head_dim: d_model / num_heads,
        }
    }

    /// Creates a GQA config with fewer KV heads.
    pub fn with_gqa(d_model: usize, num_heads: usize, num_kv_heads: usize) -> Self {
        AttentionConfig {
            d_model,
            num_heads,
            num_kv_heads,
            head_dim: d_model / num_heads,
        }
    }
}

/// Computes scaled dot-product attention scores.
/// Returns attention weights (softmaxed) for a single head.
/// q, k: [seq_len, head_dim] flattened; v: [seq_len, head_dim] flattened.
pub fn scaled_dot_product_attention(
    q: &[f64],
    k: &[f64],
    v: &[f64],
    seq_len: usize,
    head_dim: usize,
    mask: Option<&[f64]>,
) -> Vec<f64> {
    let scale = (head_dim as f64).sqrt();

    // Compute Q @ K^T / sqrt(d_k) → [seq_len, seq_len]
    let mut scores = vec![0.0; seq_len * seq_len];
    for i in 0..seq_len {
        for j in 0..seq_len {
            let mut dot = 0.0;
            for d in 0..head_dim {
                dot += q[i * head_dim + d] * k[j * head_dim + d];
            }
            scores[i * seq_len + j] = dot / scale;
        }
    }

    // Apply mask
    if let Some(m) = mask {
        for i in 0..seq_len * seq_len {
            if m[i] == 0.0 {
                scores[i] = f64::NEG_INFINITY;
            }
        }
    }

    // Softmax per row
    for i in 0..seq_len {
        let row_start = i * seq_len;
        let max_val = scores[row_start..row_start + seq_len]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let mut sum = 0.0;
        for j in 0..seq_len {
            scores[row_start + j] = (scores[row_start + j] - max_val).exp();
            sum += scores[row_start + j];
        }
        if sum > 0.0 {
            for j in 0..seq_len {
                scores[row_start + j] /= sum;
            }
        }
    }

    // Attention @ V → [seq_len, head_dim]
    let mut output = vec![0.0; seq_len * head_dim];
    for i in 0..seq_len {
        for d in 0..head_dim {
            let mut sum = 0.0;
            for j in 0..seq_len {
                sum += scores[i * seq_len + j] * v[j * head_dim + d];
            }
            output[i * head_dim + d] = sum;
        }
    }

    output
}

// ═══════════════════════════════════════════════════════════════════════
// S13.2: Causal Masking
// ═══════════════════════════════════════════════════════════════════════

/// Creates a causal (lower-triangular) mask for autoregressive attention.
pub fn causal_mask(seq_len: usize) -> Vec<f64> {
    let mut mask = vec![0.0; seq_len * seq_len];
    for i in 0..seq_len {
        for j in 0..=i {
            mask[i * seq_len + j] = 1.0;
        }
    }
    mask
}

// ═══════════════════════════════════════════════════════════════════════
// S13.3: Rotary Position Embeddings (RoPE)
// ═══════════════════════════════════════════════════════════════════════

/// Computes RoPE frequencies for a given dimension and position.
pub fn rope_frequencies(head_dim: usize, base: f64) -> Vec<f64> {
    let half = head_dim / 2;
    (0..half)
        .map(|i| 1.0 / base.powf(2.0 * i as f64 / head_dim as f64))
        .collect()
}

/// Applies RoPE to a query/key vector at a given position.
pub fn apply_rope(vec: &mut [f64], position: usize, freqs: &[f64]) {
    let half = freqs.len();
    for i in 0..half {
        let angle = position as f64 * freqs[i];
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let x0 = vec[2 * i];
        let x1 = vec[2 * i + 1];
        vec[2 * i] = x0 * cos_a - x1 * sin_a;
        vec[2 * i + 1] = x0 * sin_a + x1 * cos_a;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.4: KV Cache
// ═══════════════════════════════════════════════════════════════════════

/// A key-value cache for incremental decoding.
#[derive(Debug, Clone)]
pub struct KvCache {
    /// Cached keys: [num_layers][cached_seq_len * head_dim].
    pub keys: Vec<Vec<f64>>,
    /// Cached values: [num_layers][cached_seq_len * head_dim].
    pub values: Vec<Vec<f64>>,
    /// Head dimension.
    pub head_dim: usize,
    /// Current cached sequence length.
    pub cached_len: usize,
}

impl KvCache {
    /// Creates an empty KV cache.
    pub fn new(num_layers: usize, head_dim: usize) -> Self {
        KvCache {
            keys: vec![Vec::new(); num_layers],
            values: vec![Vec::new(); num_layers],
            head_dim,
            cached_len: 0,
        }
    }

    /// Appends new key-value entries for a layer.
    pub fn append(&mut self, layer: usize, new_keys: &[f64], new_values: &[f64]) {
        self.keys[layer].extend_from_slice(new_keys);
        self.values[layer].extend_from_slice(new_values);
        self.cached_len = self.keys[layer].len() / self.head_dim;
    }

    /// Returns the full key sequence for a layer.
    pub fn get_keys(&self, layer: usize) -> &[f64] {
        &self.keys[layer]
    }

    /// Returns the full value sequence for a layer.
    pub fn get_values(&self, layer: usize) -> &[f64] {
        &self.values[layer]
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        for k in &mut self.keys {
            k.clear();
        }
        for v in &mut self.values {
            v.clear();
        }
        self.cached_len = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.5: Flash Attention (simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Flash attention configuration.
#[derive(Debug, Clone)]
pub struct FlashAttentionConfig {
    /// Block size for tiling.
    pub block_size: usize,
    /// Whether causal masking is enabled.
    pub causal: bool,
}

impl Default for FlashAttentionConfig {
    fn default() -> Self {
        FlashAttentionConfig {
            block_size: 64,
            causal: true,
        }
    }
}

/// Estimates memory usage for standard vs flash attention.
pub fn memory_comparison(seq_len: usize, num_heads: usize) -> (usize, usize) {
    let standard = seq_len * seq_len * num_heads * 8; // O(N^2) bytes
    let flash = seq_len * num_heads * 8; // O(N) bytes
    (standard, flash)
}

// ═══════════════════════════════════════════════════════════════════════
// S13.6: Grouped Query Attention
// ═══════════════════════════════════════════════════════════════════════

/// Returns the number of query heads per KV head group.
pub fn gqa_group_size(num_heads: usize, num_kv_heads: usize) -> usize {
    num_heads / num_kv_heads
}

// ═══════════════════════════════════════════════════════════════════════
// S13.7: Layer Normalization
// ═══════════════════════════════════════════════════════════════════════

/// RMSNorm: root mean square normalization (pre-norm, used by LLaMA).
pub fn rms_norm(x: &[f64], weight: &[f64], eps: f64) -> Vec<f64> {
    let n = x.len();
    let rms = (x.iter().map(|&v| v * v).sum::<f64>() / n as f64 + eps).sqrt();
    x.iter()
        .zip(weight.iter())
        .map(|(&xi, &wi)| xi / rms * wi)
        .collect()
}

/// Standard LayerNorm.
pub fn layer_norm(x: &[f64], weight: &[f64], bias: &[f64], eps: f64) -> Vec<f64> {
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let var = x.iter().map(|&v| (v - mean) * (v - mean)).sum::<f64>() / n;
    let std = (var + eps).sqrt();
    x.iter()
        .zip(weight.iter().zip(bias.iter()))
        .map(|(&xi, (&wi, &bi))| (xi - mean) / std * wi + bi)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S13.8: SwiGLU Activation
// ═══════════════════════════════════════════════════════════════════════

/// SiLU (Swish) activation: x * sigmoid(x).
pub fn silu(x: f64) -> f64 {
    x * (1.0 / (1.0 + (-x).exp()))
}

/// SwiGLU activation: SiLU(x * W1) * (x * W2).
pub fn swiglu(gate: &[f64], up: &[f64]) -> Vec<f64> {
    gate.iter()
        .zip(up.iter())
        .map(|(&g, &u)| silu(g) * u)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S13.9: Token Sampling
// ═══════════════════════════════════════════════════════════════════════

/// Sampling strategy for text generation.
#[derive(Debug, Clone)]
pub enum SamplingStrategy {
    /// Greedy: always pick the highest probability token.
    Greedy,
    /// Temperature scaling.
    Temperature(f64),
    /// Top-k: sample from the k highest probability tokens.
    TopK(usize),
    /// Top-p (nucleus): sample from smallest set whose cumulative probability >= p.
    TopP(f64),
}

impl fmt::Display for SamplingStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SamplingStrategy::Greedy => write!(f, "Greedy"),
            SamplingStrategy::Temperature(t) => write!(f, "Temperature({t})"),
            SamplingStrategy::TopK(k) => write!(f, "TopK({k})"),
            SamplingStrategy::TopP(p) => write!(f, "TopP({p})"),
        }
    }
}

/// Applies temperature scaling to logits.
pub fn apply_temperature(logits: &[f64], temperature: f64) -> Vec<f64> {
    logits.iter().map(|&l| l / temperature).collect()
}

/// Applies top-k filtering: keep only the k largest logits.
pub fn apply_top_k(logits: &[f64], k: usize) -> Vec<f64> {
    let mut indexed: Vec<(usize, f64)> = logits.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut result = vec![f64::NEG_INFINITY; logits.len()];
    for &(idx, val) in indexed.iter().take(k) {
        result[idx] = val;
    }
    result
}

/// Applies top-p (nucleus) filtering: keep tokens whose cumulative probability >= p.
pub fn apply_top_p(logits: &[f64], p: f64) -> Vec<f64> {
    // Softmax first
    let max_val = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logits.iter().map(|&l| (l - max_val).exp()).collect();
    let sum: f64 = exps.iter().sum();
    let probs: Vec<f64> = exps.iter().map(|&e| e / sum).collect();

    let mut indexed: Vec<(usize, f64)> = probs.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut cumulative = 0.0;
    let mut result = vec![f64::NEG_INFINITY; logits.len()];
    for &(idx, prob) in &indexed {
        if cumulative >= p {
            break;
        }
        result[idx] = logits[idx];
        cumulative += prob;
    }
    result
}

/// Returns the index of the maximum logit (greedy sampling).
pub fn argmax(logits: &[f64]) -> usize {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S13.1 — Multi-Head Self-Attention
    #[test]
    fn s13_1_attention_config() {
        let cfg = AttentionConfig::new(512, 8);
        assert_eq!(cfg.head_dim, 64);
        assert_eq!(cfg.num_kv_heads, 8);
    }

    #[test]
    fn s13_1_scaled_dot_product() {
        let q = vec![1.0, 0.0, 0.0, 1.0]; // 2x2
        let k = vec![1.0, 0.0, 0.0, 1.0];
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let output = scaled_dot_product_attention(&q, &k, &v, 2, 2, None);
        assert_eq!(output.len(), 4);
        // With identity-like Q,K, output should blend V rows
    }

    // S13.2 — Causal Masking
    #[test]
    fn s13_2_causal_mask() {
        let mask = causal_mask(3);
        // Row 0: [1, 0, 0], Row 1: [1, 1, 0], Row 2: [1, 1, 1]
        assert_eq!(mask[0], 1.0);
        assert_eq!(mask[1], 0.0);
        assert_eq!(mask[3], 1.0);
        assert_eq!(mask[4], 1.0);
        assert_eq!(mask[8], 1.0);
    }

    #[test]
    fn s13_2_causal_attention() {
        let q = vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0]; // 3x2
        let k = vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let v = vec![1.0, 0.0, 0.0, 1.0, 0.5, 0.5];
        let mask = causal_mask(3);
        let output = scaled_dot_product_attention(&q, &k, &v, 3, 2, Some(&mask));
        assert_eq!(output.len(), 6);
    }

    // S13.3 — RoPE
    #[test]
    fn s13_3_rope_frequencies() {
        let freqs = rope_frequencies(4, 10000.0);
        assert_eq!(freqs.len(), 2);
        assert!((freqs[0] - 1.0).abs() < 1e-6); // 1/10000^0 = 1
    }

    #[test]
    fn s13_3_apply_rope() {
        let freqs = rope_frequencies(4, 10000.0);
        let mut vec = vec![1.0, 0.0, 1.0, 0.0];
        apply_rope(&mut vec, 0, &freqs);
        // At position 0, cos(0)=1, sin(0)=0 → no change
        assert!((vec[0] - 1.0).abs() < 1e-6);
    }

    // S13.4 — KV Cache
    #[test]
    fn s13_4_kv_cache() {
        let mut cache = KvCache::new(2, 4);
        cache.append(0, &[1.0, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0]);
        assert_eq!(cache.cached_len, 1);
        assert_eq!(cache.get_keys(0).len(), 4);

        cache.append(0, &[9.0, 10.0, 11.0, 12.0], &[13.0, 14.0, 15.0, 16.0]);
        assert_eq!(cache.cached_len, 2);
    }

    #[test]
    fn s13_4_kv_cache_clear() {
        let mut cache = KvCache::new(1, 2);
        cache.append(0, &[1.0, 2.0], &[3.0, 4.0]);
        cache.clear();
        assert_eq!(cache.cached_len, 0);
        assert!(cache.get_keys(0).is_empty());
    }

    // S13.5 — Flash Attention
    #[test]
    fn s13_5_memory_comparison() {
        let (standard, flash) = memory_comparison(1024, 32);
        assert!(standard > flash);
        assert_eq!(standard, 1024 * 1024 * 32 * 8); // O(N^2)
    }

    // S13.6 — GQA
    #[test]
    fn s13_6_gqa_group_size() {
        assert_eq!(gqa_group_size(32, 8), 4); // LLaMA 2 70B style
        assert_eq!(gqa_group_size(32, 32), 1); // Standard MHA
    }

    // S13.7 — Layer Normalization
    #[test]
    fn s13_7_rms_norm() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let w = vec![1.0, 1.0, 1.0, 1.0];
        let result = rms_norm(&x, &w, 1e-6);
        assert_eq!(result.len(), 4);
        // RMS = sqrt((1+4+9+16)/4) = sqrt(7.5)
        let rms = (7.5_f64 + 1e-6).sqrt();
        assert!((result[0] - 1.0 / rms).abs() < 1e-6);
    }

    #[test]
    fn s13_7_layer_norm() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let w = vec![1.0, 1.0, 1.0, 1.0];
        let b = vec![0.0, 0.0, 0.0, 0.0];
        let result = layer_norm(&x, &w, &b, 1e-6);
        // Mean=2.5, Var=1.25, output should be centered and scaled
        let mean: f64 = result.iter().sum::<f64>() / 4.0;
        assert!(mean.abs() < 1e-6); // Zero mean
    }

    // S13.8 — SwiGLU
    #[test]
    fn s13_8_silu() {
        assert!((silu(0.0) - 0.0).abs() < 1e-6);
        assert!(silu(5.0) > 4.9); // Approaches x for large x
    }

    #[test]
    fn s13_8_swiglu() {
        let gate = vec![1.0, 2.0, 3.0];
        let up = vec![1.0, 1.0, 1.0];
        let result = swiglu(&gate, &up);
        assert_eq!(result.len(), 3);
        assert!((result[0] - silu(1.0)).abs() < 1e-6);
    }

    // S13.9 — Token Sampling
    #[test]
    fn s13_9_argmax() {
        assert_eq!(argmax(&[1.0, 5.0, 3.0, 2.0]), 1);
    }

    #[test]
    fn s13_9_temperature() {
        let logits = vec![1.0, 2.0, 3.0];
        let scaled = apply_temperature(&logits, 0.5);
        assert_eq!(scaled, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn s13_9_top_k() {
        let logits = vec![1.0, 5.0, 3.0, 2.0, 4.0];
        let filtered = apply_top_k(&logits, 2);
        assert_eq!(argmax(&filtered), 1); // 5.0 is max
                                          // Only 2 values should be non-neg-inf
        let valid: Vec<_> = filtered
            .iter()
            .filter(|&&v| v > f64::NEG_INFINITY)
            .collect();
        assert_eq!(valid.len(), 2);
    }

    #[test]
    fn s13_9_top_p() {
        let logits = vec![10.0, 1.0, 0.1]; // Highly skewed
        let filtered = apply_top_p(&logits, 0.9);
        // Token 0 has ~99% prob, so only it should survive
        assert!(filtered[0] > f64::NEG_INFINITY);
    }

    // S13.10 — Integration
    #[test]
    fn s13_10_sampling_strategy_display() {
        assert_eq!(SamplingStrategy::Greedy.to_string(), "Greedy");
        assert_eq!(SamplingStrategy::TopK(50).to_string(), "TopK(50)");
    }

    #[test]
    fn s13_10_flash_config() {
        let cfg = FlashAttentionConfig::default();
        assert_eq!(cfg.block_size, 64);
        assert!(cfg.causal);
    }
}
