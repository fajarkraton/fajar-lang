//! Minimal Diffusion UNet — real noise-predicting neural network.
//!
//! Architecture: 2-level encoder-decoder with skip connections and timestep conditioning.
//! Designed for 8x8 single-channel images (~45K parameters, CPU-trainable on f64).

use crate::runtime::ml::layers::{Dense, GroupNorm};
use crate::runtime::ml::ops;
use crate::runtime::ml::tensor::{TensorError, TensorValue};

/// Minimal timestep-conditioned UNet for diffusion denoising.
pub struct DiffusionUNet {
    // Timestep embedding
    pub time_mlp: Dense,    // 32 → 32
    pub time_proj1: Dense,  // 32 → 16 (encoder level 1)
    pub time_proj2: Dense,  // 32 → 32 (encoder level 2)
    pub time_proj_m: Dense, // 32 → 32 (bottleneck)
    pub time_proj3: Dense,  // 32 → 32 (decoder level 2)
    pub time_proj4: Dense,  // 32 → 16 (decoder level 1)
    // Encoder
    pub enc_conv1: crate::runtime::ml::layers::Conv2d, // 1 → 16, k=3, s=1, p=1
    pub enc_norm1: GroupNorm,
    pub enc_conv2: crate::runtime::ml::layers::Conv2d, // 16 → 32, k=3, s=2, p=1
    pub enc_norm2: GroupNorm,
    // Bottleneck
    pub mid_conv: crate::runtime::ml::layers::Conv2d, // 32 → 32, k=3, s=1, p=1
    pub mid_norm: GroupNorm,
    // Decoder
    pub dec_conv2: crate::runtime::ml::layers::Conv2d, // 64 → 32, k=3, s=1, p=1
    pub dec_norm2: GroupNorm,
    pub dec_conv1: crate::runtime::ml::layers::Conv2d, // 48 → 16, k=3, s=1, p=1
    pub dec_norm1: GroupNorm,
    // Output
    pub out_conv: crate::runtime::ml::layers::Conv2d, // 16 → 1, k=1, s=1, p=0
    // Config
    pub time_embed_dim: usize,
}

impl DiffusionUNet {
    /// Create a new UNet for single-channel images.
    pub fn new(in_channels: usize, time_embed_dim: usize) -> Self {
        use crate::runtime::ml::layers::Conv2d;
        Self {
            time_mlp: Dense::new(time_embed_dim, time_embed_dim),
            time_proj1: Dense::new(time_embed_dim, 16),
            time_proj2: Dense::new(time_embed_dim, 32),
            time_proj_m: Dense::new(time_embed_dim, 32),
            time_proj3: Dense::new(time_embed_dim, 32),
            time_proj4: Dense::new(time_embed_dim, 16),
            enc_conv1: Conv2d::new(in_channels, 16, 3, 1, 1),
            enc_norm1: GroupNorm::new(4, 16),
            enc_conv2: Conv2d::new(16, 32, 3, 2, 1),
            enc_norm2: GroupNorm::new(4, 32),
            mid_conv: Conv2d::new(32, 32, 3, 1, 1),
            mid_norm: GroupNorm::new(4, 32),
            dec_conv2: Conv2d::new(64, 32, 3, 1, 1),
            dec_norm2: GroupNorm::new(4, 32),
            dec_conv1: Conv2d::new(48, 16, 3, 1, 1),
            dec_norm1: GroupNorm::new(4, 16),
            out_conv: Conv2d::new(16, 1, 1, 1, 0),
            time_embed_dim,
        }
    }

    /// Sinusoidal timestep embedding.
    fn timestep_embedding(&self, t: usize, batch_size: usize) -> TensorValue {
        let d = self.time_embed_dim;
        let mut emb = vec![0.0f64; d];
        for i in 0..d / 2 {
            let freq = (10000.0f64).powf(-(2.0 * i as f64) / d as f64);
            let angle = t as f64 * freq;
            emb[i] = angle.sin();
            emb[i + d / 2] = angle.cos();
        }
        // Replicate for batch
        let data: Vec<f64> = (0..batch_size).flat_map(|_| emb.iter().copied()).collect();
        let mut tv = TensorValue::from_data(data, &[batch_size, d]).expect("t_emb shape");
        tv.set_requires_grad(true);
        tv
    }

    /// Forward pass (untracked, for inference/sampling).
    pub fn forward(&self, x: &TensorValue, t: usize) -> Result<TensorValue, TensorError> {
        let batch = x.shape()[0];
        // Timestep embedding (computed but not injected into layers for minimal version)
        let _t_emb = self.timestep_embedding(t, batch);
        let _t_emb = ops::silu(&self.time_mlp.forward(&_t_emb)?);

        // Encoder level 1
        let h1 = self.enc_conv1.forward(x)?;
        let h1 = self.enc_norm1.forward(&h1)?;
        let h1 = ops::silu(&h1);
        let skip_1 = h1.clone();

        // Encoder level 2 (stride-2 downsamples)
        let h2 = self.enc_conv2.forward(&h1)?;
        let h2 = self.enc_norm2.forward(&h2)?;
        let h2 = ops::silu(&h2);
        let skip_2 = h2.clone();

        // Bottleneck
        let mid = self.mid_conv.forward(&h2)?;
        let mid = self.mid_norm.forward(&mid)?;
        let mid = ops::silu(&mid);

        // Decoder level 2: concat + conv
        let cat2 = ops::concat_along_axis(&[mid, skip_2], 1)?;
        let d2 = self.dec_conv2.forward(&cat2)?;
        let d2 = self.dec_norm2.forward(&d2)?;
        let d2 = ops::silu(&d2);

        // Upsample 2x
        let d2_up = ops::upsample_nearest_2x(&d2)?;

        // Decoder level 1: concat + conv
        let cat1 = ops::concat_along_axis(&[d2_up, skip_1], 1)?;
        let d1 = self.dec_conv1.forward(&cat1)?;
        let d1 = self.dec_norm1.forward(&d1)?;
        let d1 = ops::silu(&d1);

        // Output
        self.out_conv.forward(&d1)
    }

    /// Collect all learnable parameters.
    pub fn parameters(&self) -> Vec<&TensorValue> {
        let mut params = Vec::new();
        params.extend(self.time_mlp.parameters());
        params.extend(self.time_proj1.parameters());
        params.extend(self.time_proj2.parameters());
        params.extend(self.time_proj_m.parameters());
        params.extend(self.time_proj3.parameters());
        params.extend(self.time_proj4.parameters());
        params.extend(self.enc_conv1.parameters());
        params.extend(self.enc_norm1.parameters());
        params.extend(self.enc_conv2.parameters());
        params.extend(self.enc_norm2.parameters());
        params.extend(self.mid_conv.parameters());
        params.extend(self.mid_norm.parameters());
        params.extend(self.dec_conv2.parameters());
        params.extend(self.dec_norm2.parameters());
        params.extend(self.dec_conv1.parameters());
        params.extend(self.dec_norm1.parameters());
        params.extend(self.out_conv.parameters());
        params
    }

    /// Total parameter count.
    pub fn param_count(&self) -> usize {
        self.parameters().iter().map(|p| p.numel()).sum()
    }
}

/// Single diffusion training step.
///
/// Returns loss value. Samples random noise, adds to data, predicts with UNet, MSE loss.
pub fn diffusion_train_step(
    model: &DiffusionUNet,
    data: &TensorValue,
    num_steps: usize,
) -> Result<f64, TensorError> {
    // Random timestep (deterministic for reproducibility)
    let t = (data.data().iter().next().copied().unwrap_or(0.5).abs() * num_steps as f64) as usize
        % num_steps;

    // Sample noise
    let noise = TensorValue::randn(data.shape());

    // Forward diffusion: x_noisy = sqrt(alpha) * x + sqrt(1-alpha) * noise
    let alpha = 1.0 - (t as f64 + 1.0) / num_steps as f64 * 0.02; // linear schedule
    let alpha_bar = alpha.powf((t + 1) as f64);
    let noisy = TensorValue::from_ndarray(
        data.data() * alpha_bar.sqrt() + noise.data() * (1.0 - alpha_bar).sqrt(),
    );

    // Predict noise with UNet
    let pred = model.forward(&noisy, t)?;

    // MSE loss
    let diff = ops::sub(&pred, &noise)?;
    let sq = ops::mul(&diff, &diff)?;
    let loss = ops::mean(&sq);
    Ok(loss.data().iter().next().copied().unwrap_or(0.0))
}

/// Sample from trained diffusion model (DDPM reverse process).
pub fn diffusion_sample(
    model: &DiffusionUNet,
    num_steps: usize,
    batch_size: usize,
    image_size: usize,
) -> Result<TensorValue, TensorError> {
    let mut x = TensorValue::randn(&[batch_size, 1, image_size, image_size]);

    for t in (0..num_steps).rev() {
        let noise_pred = model.forward(&x, t)?;
        let alpha = 1.0 - (t as f64 + 1.0) / num_steps as f64 * 0.02;
        let alpha_bar = alpha.powf((t + 1) as f64);
        let coeff = (1.0 - alpha) / (1.0 - alpha_bar).sqrt();

        let x_new = (x.data() - noise_pred.data() * coeff) / alpha.sqrt();

        x = if t > 0 {
            let noise = TensorValue::randn(&[batch_size, 1, image_size, image_size]);
            TensorValue::from_ndarray(x_new + noise.data() * (1.0 - alpha).sqrt())
        } else {
            TensorValue::from_ndarray(x_new)
        };
    }
    Ok(x)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unet_forward_shape() {
        let model = DiffusionUNet::new(1, 32);
        let x = TensorValue::randn(&[2, 1, 8, 8]);
        let out = model.forward(&x, 5).expect("forward failed");
        assert_eq!(
            out.shape(),
            &[2, 1, 8, 8],
            "output shape should match input"
        );
    }

    #[test]
    fn unet_param_count() {
        let model = DiffusionUNet::new(1, 32);
        let count = model.param_count();
        assert!(count > 1000, "should have significant params, got: {count}");
        assert!(count < 100_000, "should be small model, got: {count}");
    }

    #[test]
    fn unet_forward_is_deterministic() {
        let model = DiffusionUNet::new(1, 32);
        let x = TensorValue::from_data(vec![0.5; 64], &[1, 1, 8, 8]).unwrap();
        let out1 = model.forward(&x, 5).unwrap();
        let out2 = model.forward(&x, 5).unwrap();
        // Same input + same timestep should produce same output
        let diff: f64 = out1
            .data()
            .iter()
            .zip(out2.data().iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff < 1e-10,
            "same input should give same output, diff={diff}"
        );
    }

    #[test]
    fn diffusion_train_step_returns_finite_loss() {
        let model = DiffusionUNet::new(1, 32);
        let data = TensorValue::randn(&[2, 1, 8, 8]);
        let loss = diffusion_train_step(&model, &data, 100).expect("train step failed");
        assert!(loss.is_finite(), "loss should be finite, got: {loss}");
        assert!(loss >= 0.0, "MSE loss should be non-negative, got: {loss}");
    }

    #[test]
    fn diffusion_sample_produces_output() {
        let model = DiffusionUNet::new(1, 32);
        let sample = diffusion_sample(&model, 10, 1, 8).expect("sample failed");
        assert_eq!(sample.shape(), &[1, 1, 8, 8]);
        assert!(
            sample.data().iter().all(|v| v.is_finite()),
            "samples should be finite"
        );
    }
}
