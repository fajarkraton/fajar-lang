//! Diffusion Models — noise schedules, UNet, timestep embeddings,
//! forward/reverse process, DDIM, classifier-free guidance, latent diffusion.

use std::f64::consts::PI;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S14.1: Noise Schedule
// ═══════════════════════════════════════════════════════════════════════

/// Noise schedule type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleType {
    /// Linear schedule.
    Linear,
    /// Cosine schedule (improved DDPM).
    Cosine,
}

impl fmt::Display for ScheduleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleType::Linear => write!(f, "Linear"),
            ScheduleType::Cosine => write!(f, "Cosine"),
        }
    }
}

/// A noise schedule for the diffusion process.
#[derive(Debug, Clone)]
pub struct NoiseSchedule {
    /// Schedule type.
    pub schedule_type: ScheduleType,
    /// Total diffusion timesteps.
    pub num_steps: usize,
    /// Beta values (noise variance per step).
    pub betas: Vec<f64>,
    /// Cumulative alpha products (signal retention).
    pub alpha_cumprod: Vec<f64>,
}

/// Creates a linear noise schedule.
pub fn linear_schedule(num_steps: usize, beta_start: f64, beta_end: f64) -> NoiseSchedule {
    let betas: Vec<f64> = (0..num_steps)
        .map(|t| beta_start + (beta_end - beta_start) * t as f64 / (num_steps - 1) as f64)
        .collect();

    let mut alpha_cumprod = Vec::with_capacity(num_steps);
    let mut prod = 1.0;
    for &beta in &betas {
        prod *= 1.0 - beta;
        alpha_cumprod.push(prod);
    }

    NoiseSchedule {
        schedule_type: ScheduleType::Linear,
        num_steps,
        betas,
        alpha_cumprod,
    }
}

/// Creates a cosine noise schedule.
pub fn cosine_schedule(num_steps: usize) -> NoiseSchedule {
    let s = 0.008;
    let mut alpha_cumprod = Vec::with_capacity(num_steps);
    for t in 0..num_steps {
        let val = ((t as f64 / num_steps as f64 + s) / (1.0 + s) * PI / 2.0).cos();
        alpha_cumprod.push(val * val);
    }

    let mut betas = Vec::with_capacity(num_steps);
    for t in 0..num_steps {
        let prev = if t == 0 { 1.0 } else { alpha_cumprod[t - 1] };
        let beta = 1.0 - alpha_cumprod[t] / prev;
        betas.push(beta.clamp(0.0, 0.999));
    }

    NoiseSchedule {
        schedule_type: ScheduleType::Cosine,
        num_steps,
        betas,
        alpha_cumprod,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S14.2: UNet Architecture
// ═══════════════════════════════════════════════════════════════════════

/// A UNet block descriptor.
#[derive(Debug, Clone)]
pub struct UNetBlock {
    /// Block name.
    pub name: String,
    /// Input channels.
    pub in_channels: usize,
    /// Output channels.
    pub out_channels: usize,
    /// Block type.
    pub block_type: UNetBlockType,
}

/// Type of UNet block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UNetBlockType {
    /// Downsampling block.
    Down,
    /// Middle block.
    Mid,
    /// Upsampling block with skip connection.
    Up,
}

impl fmt::Display for UNetBlockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UNetBlockType::Down => write!(f, "Down"),
            UNetBlockType::Mid => write!(f, "Mid"),
            UNetBlockType::Up => write!(f, "Up"),
        }
    }
}

/// UNet configuration.
#[derive(Debug, Clone)]
pub struct UNetConfig {
    /// Channel multipliers per level.
    pub channel_mults: Vec<usize>,
    /// Base channels.
    pub base_channels: usize,
    /// Time embedding dimension.
    pub time_embed_dim: usize,
}

impl UNetConfig {
    /// Builds the list of blocks for this UNet.
    pub fn build_blocks(&self) -> Vec<UNetBlock> {
        let mut blocks = Vec::new();
        let mut ch = self.base_channels;

        // Downsampling path
        for (i, &mult) in self.channel_mults.iter().enumerate() {
            let out_ch = self.base_channels * mult;
            blocks.push(UNetBlock {
                name: format!("down_{i}"),
                in_channels: ch,
                out_channels: out_ch,
                block_type: UNetBlockType::Down,
            });
            ch = out_ch;
        }

        // Middle
        blocks.push(UNetBlock {
            name: "mid".into(),
            in_channels: ch,
            out_channels: ch,
            block_type: UNetBlockType::Mid,
        });

        // Upsampling path (reverse)
        for (i, &mult) in self.channel_mults.iter().rev().enumerate() {
            let out_ch = self.base_channels * mult;
            blocks.push(UNetBlock {
                name: format!("up_{i}"),
                in_channels: ch + out_ch, // skip connection doubles channels
                out_channels: out_ch,
                block_type: UNetBlockType::Up,
            });
            ch = out_ch;
        }

        blocks
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S14.3: Sinusoidal Timestep Embedding
// ═══════════════════════════════════════════════════════════════════════

/// Computes sinusoidal timestep embedding.
pub fn timestep_embedding(timestep: usize, dim: usize) -> Vec<f64> {
    let half = dim / 2;
    let mut embed = Vec::with_capacity(dim);
    for i in 0..half {
        let freq = 1.0 / 10000.0_f64.powf(2.0 * i as f64 / dim as f64);
        let angle = timestep as f64 * freq;
        embed.push(angle.sin());
        embed.push(angle.cos());
    }
    embed.truncate(dim);
    embed
}

// ═══════════════════════════════════════════════════════════════════════
// S14.4-S14.5: Forward & Reverse Process
// ═══════════════════════════════════════════════════════════════════════

/// Adds noise to data at a given timestep (forward process).
pub fn add_noise(data: &[f64], noise: &[f64], alpha_cumprod: f64) -> Vec<f64> {
    let signal_rate = alpha_cumprod.sqrt();
    let noise_rate = (1.0 - alpha_cumprod).sqrt();
    data.iter()
        .zip(noise.iter())
        .map(|(&d, &n)| signal_rate * d + noise_rate * n)
        .collect()
}

/// Predicts the original data from noisy data and predicted noise (reverse step).
pub fn predict_original(noisy: &[f64], predicted_noise: &[f64], alpha_cumprod: f64) -> Vec<f64> {
    let signal_rate = alpha_cumprod.sqrt();
    let noise_rate = (1.0 - alpha_cumprod).sqrt();
    noisy
        .iter()
        .zip(predicted_noise.iter())
        .map(|(&x, &eps)| (x - noise_rate * eps) / signal_rate)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S14.6: DDIM Sampling
// ═══════════════════════════════════════════════════════════════════════

/// Computes DDIM timestep schedule (fewer steps than DDPM).
pub fn ddim_timesteps(num_inference_steps: usize, num_train_steps: usize) -> Vec<usize> {
    let step_size = num_train_steps / num_inference_steps;
    (0..num_inference_steps).map(|i| i * step_size).collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S14.7: Classifier-Free Guidance
// ═══════════════════════════════════════════════════════════════════════

/// Applies classifier-free guidance to model predictions.
pub fn classifier_free_guidance(
    cond_pred: &[f64],
    uncond_pred: &[f64],
    guidance_scale: f64,
) -> Vec<f64> {
    cond_pred
        .iter()
        .zip(uncond_pred.iter())
        .map(|(&c, &u)| u + guidance_scale * (c - u))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S14.8: Latent Diffusion
// ═══════════════════════════════════════════════════════════════════════

/// Latent space configuration.
#[derive(Debug, Clone)]
pub struct LatentConfig {
    /// Latent spatial dimensions (e.g., 64 for 512px image with 8x downscale).
    pub latent_size: usize,
    /// Number of latent channels.
    pub latent_channels: usize,
    /// Downscale factor from pixel space.
    pub downscale_factor: usize,
}

impl LatentConfig {
    /// Computes the pixel-space image size.
    pub fn image_size(&self) -> usize {
        self.latent_size * self.downscale_factor
    }

    /// Computes the latent vector length.
    pub fn latent_length(&self) -> usize {
        self.latent_channels * self.latent_size * self.latent_size
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S14.9: Image Generation Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Pipeline stage for text-to-image generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStage {
    /// Text encoding.
    TextEncoding,
    /// Latent diffusion (UNet iterations).
    Diffusion,
    /// Latent-to-pixel decoding.
    Decoding,
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineStage::TextEncoding => write!(f, "TextEncoding"),
            PipelineStage::Diffusion => write!(f, "Diffusion"),
            PipelineStage::Decoding => write!(f, "Decoding"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S14.1 — Noise Schedule
    #[test]
    fn s14_1_linear_schedule() {
        let sched = linear_schedule(100, 1e-4, 0.02);
        assert_eq!(sched.betas.len(), 100);
        assert!(sched.betas[0] < sched.betas[99]);
        assert!(sched.alpha_cumprod[0] > sched.alpha_cumprod[99]);
    }

    #[test]
    fn s14_1_cosine_schedule() {
        let sched = cosine_schedule(100);
        assert_eq!(sched.betas.len(), 100);
        assert!(sched.alpha_cumprod[0] > sched.alpha_cumprod[99]);
    }

    // S14.2 — UNet Architecture
    #[test]
    fn s14_2_unet_blocks() {
        let config = UNetConfig {
            channel_mults: vec![1, 2, 4],
            base_channels: 64,
            time_embed_dim: 256,
        };
        let blocks = config.build_blocks();
        assert!(blocks.iter().any(|b| b.block_type == UNetBlockType::Down));
        assert!(blocks.iter().any(|b| b.block_type == UNetBlockType::Mid));
        assert!(blocks.iter().any(|b| b.block_type == UNetBlockType::Up));
    }

    // S14.3 — Timestep Embedding
    #[test]
    fn s14_3_timestep_embedding() {
        let embed = timestep_embedding(50, 128);
        assert_eq!(embed.len(), 128);
        // Different timesteps should produce different embeddings
        let embed2 = timestep_embedding(100, 128);
        assert_ne!(embed, embed2);
    }

    // S14.4 — Forward Process
    #[test]
    fn s14_4_add_noise() {
        let data = vec![1.0, 2.0, 3.0];
        let noise = vec![0.5, -0.5, 0.0];
        let noisy = add_noise(&data, &noise, 0.9);
        assert_eq!(noisy.len(), 3);
        // With high alpha_cumprod (0.9), signal dominates
        assert!((noisy[0] - 0.9_f64.sqrt() * 1.0 - 0.1_f64.sqrt() * 0.5).abs() < 1e-10);
    }

    // S14.5 — Reverse Process
    #[test]
    fn s14_5_predict_original() {
        let data = vec![1.0, 2.0, 3.0];
        let noise = vec![0.1, -0.1, 0.0];
        let alpha = 0.95;
        let noisy = add_noise(&data, &noise, alpha);
        let recovered = predict_original(&noisy, &noise, alpha);
        for (d, r) in data.iter().zip(recovered.iter()) {
            assert!((d - r).abs() < 1e-10);
        }
    }

    // S14.6 — DDIM Sampling
    #[test]
    fn s14_6_ddim_timesteps() {
        let steps = ddim_timesteps(20, 1000);
        assert_eq!(steps.len(), 20);
        assert_eq!(steps[0], 0);
        assert_eq!(steps[1], 50); // 1000/20 = 50
    }

    // S14.7 — Classifier-Free Guidance
    #[test]
    fn s14_7_cfg() {
        let cond = vec![2.0, 3.0];
        let uncond = vec![1.0, 1.0];
        let guided = classifier_free_guidance(&cond, &uncond, 7.5);
        // u + 7.5 * (c - u) = 1 + 7.5 * 1 = 8.5
        assert!((guided[0] - 8.5).abs() < 1e-10);
    }

    // S14.8 — Latent Diffusion
    #[test]
    fn s14_8_latent_config() {
        let cfg = LatentConfig {
            latent_size: 64,
            latent_channels: 4,
            downscale_factor: 8,
        };
        assert_eq!(cfg.image_size(), 512);
        assert_eq!(cfg.latent_length(), 4 * 64 * 64);
    }

    // S14.9 — Pipeline
    #[test]
    fn s14_9_pipeline_stages() {
        assert_eq!(PipelineStage::TextEncoding.to_string(), "TextEncoding");
        assert_eq!(PipelineStage::Diffusion.to_string(), "Diffusion");
        assert_eq!(PipelineStage::Decoding.to_string(), "Decoding");
    }

    // S14.10 — Integration
    #[test]
    fn s14_10_schedule_type_display() {
        assert_eq!(ScheduleType::Linear.to_string(), "Linear");
        assert_eq!(ScheduleType::Cosine.to_string(), "Cosine");
    }

    #[test]
    fn s14_10_unet_block_type_display() {
        assert_eq!(UNetBlockType::Down.to_string(), "Down");
        assert_eq!(UNetBlockType::Up.to_string(), "Up");
    }
}
