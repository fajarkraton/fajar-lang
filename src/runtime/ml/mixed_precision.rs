//! Mixed precision training utilities — DType conversions and loss scaling.
//!
//! Provides dtype abstraction (F64/F32), conversion functions, and
//! dynamic loss scaling for stable mixed-precision training.

use ndarray::ArrayD;

use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// DType
// ═══════════════════════════════════════════════════════════════════════

/// Data type for tensor storage precision.
///
/// Supports floating point (F16, BF16, F32, F64), integer (I8, U8, I32, I64),
/// and boolean types. Inspired by HuggingFace Candle's DType system.
///
/// Internal storage remains f64 (ndarray). Reduced-precision dtypes simulate
/// precision loss via roundtrip conversion (e.g., f64 → f16 → f64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DType {
    /// 64-bit floating point (default, full precision).
    F64,
    /// 32-bit floating point (single precision).
    F32,
    /// 16-bit floating point (IEEE 754 half precision).
    F16,
    /// Brain floating point 16 (truncated mantissa, same exponent range as F32).
    BF16,
    /// 8-bit signed integer (for quantized inference, -128..127).
    I8,
    /// 8-bit unsigned integer (0..255).
    U8,
    /// 32-bit signed integer.
    I32,
    /// 64-bit signed integer.
    I64,
    /// Boolean (stored as u8 internally: 0 or 1).
    Bool,
}

impl DType {
    /// Returns the number of bytes per element for this dtype.
    pub fn size_bytes(&self) -> usize {
        match self {
            DType::F64 | DType::I64 => 8,
            DType::F32 | DType::I32 => 4,
            DType::F16 | DType::BF16 => 2,
            DType::I8 | DType::U8 | DType::Bool => 1,
        }
    }

    /// Returns a human-readable name for this dtype.
    pub fn name(&self) -> &'static str {
        match self {
            DType::F64 => "f64",
            DType::F32 => "f32",
            DType::F16 => "f16",
            DType::BF16 => "bf16",
            DType::I8 => "i8",
            DType::U8 => "u8",
            DType::I32 => "i32",
            DType::I64 => "i64",
            DType::Bool => "bool",
        }
    }

    /// Returns true if this is a floating point type.
    pub fn is_float(&self) -> bool {
        matches!(self, DType::F64 | DType::F32 | DType::F16 | DType::BF16)
    }

    /// Returns true if this is an integer type.
    pub fn is_int(&self) -> bool {
        matches!(self, DType::I8 | DType::U8 | DType::I32 | DType::I64)
    }

    /// Returns true if this is a quantized type (I8/U8).
    pub fn is_quantized(&self) -> bool {
        matches!(self, DType::I8 | DType::U8)
    }

    /// Parses a dtype from a string name.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "f64" => Some(DType::F64),
            "f32" => Some(DType::F32),
            "f16" => Some(DType::F16),
            "bf16" => Some(DType::BF16),
            "i8" => Some(DType::I8),
            "u8" => Some(DType::U8),
            "i32" => Some(DType::I32),
            "i64" => Some(DType::I64),
            "bool" => Some(DType::Bool),
            _ => None,
        }
    }

    /// Returns the minimum value representable by this dtype.
    pub fn min_value(&self) -> f64 {
        match self {
            DType::F64 => f64::MIN,
            DType::F32 => f32::MIN as f64,
            DType::F16 => -65504.0,
            DType::BF16 => -3.389e38,
            DType::I8 => -128.0,
            DType::U8 => 0.0,
            DType::I32 => i32::MIN as f64,
            DType::I64 => i64::MIN as f64,
            DType::Bool => 0.0,
        }
    }

    /// Returns the maximum value representable by this dtype.
    pub fn max_value(&self) -> f64 {
        match self {
            DType::F64 => f64::MAX,
            DType::F32 => f32::MAX as f64,
            DType::F16 => 65504.0,
            DType::BF16 => 3.389e38,
            DType::I8 => 127.0,
            DType::U8 => 255.0,
            DType::I32 => i32::MAX as f64,
            DType::I64 => i64::MAX as f64,
            DType::Bool => 1.0,
        }
    }
}

impl std::fmt::Display for DType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DType conversion
// ═══════════════════════════════════════════════════════════════════════

/// Converts a tensor to the specified dtype.
///
/// Since `TensorValue` internally stores `f64`, converting to F32
/// truncates precision (f64 -> f32 -> f64 roundtrip) and converting
/// back to F64 preserves the truncated values.
///
/// This simulates real mixed-precision behavior where F32 compute
/// loses precision compared to F64.
pub fn to_dtype(tensor: &TensorValue, dtype: DType) -> TensorValue {
    let converted = match dtype {
        DType::F64 => return tensor.clone(),
        DType::F32 => tensor.data().mapv(|v| v as f32 as f64),
        DType::F16 => {
            // Simulate f16: clamp to [-65504, 65504], reduce mantissa
            tensor.data().mapv(|v| {
                let clamped = v.clamp(-65504.0, 65504.0);
                // Simulate 10-bit mantissa by rounding
                let bits = (clamped as f32).to_bits();
                let truncated = bits & 0xFFFF_E000; // keep top 16 bits of f32
                f32::from_bits(truncated) as f64
            })
        }
        DType::BF16 => {
            // Simulate bf16: same range as f32, 7-bit mantissa
            tensor.data().mapv(|v| {
                let bits = (v as f32).to_bits();
                let truncated = bits & 0xFFFF_0000; // keep top 16 bits
                f32::from_bits(truncated) as f64
            })
        }
        DType::I8 => tensor
            .data()
            .mapv(|v| (v.clamp(-128.0, 127.0) as i8) as f64),
        DType::U8 => tensor.data().mapv(|v| (v.clamp(0.0, 255.0) as u8) as f64),
        DType::I32 => tensor.data().mapv(|v| (v as i32) as f64),
        DType::I64 => tensor.data().mapv(|v| (v as i64) as f64),
        DType::Bool => tensor.data().mapv(|v| if v != 0.0 { 1.0 } else { 0.0 }),
    };
    TensorValue::new(converted, tensor.requires_grad())
}

/// Converts raw f64 data to simulate f32 precision.
///
/// Useful for applying precision reduction to individual arrays.
pub fn to_f32_precision(data: &ArrayD<f64>) -> ArrayD<f64> {
    data.mapv(|v| v as f32 as f64)
}

// ═══════════════════════════════════════════════════════════════════════
// LossScaler
// ═══════════════════════════════════════════════════════════════════════

/// Dynamic loss scaler for mixed-precision training.
///
/// Scales the loss before backward pass to prevent gradient underflow
/// in lower-precision computation. Automatically adjusts the scale
/// factor based on whether overflow/NaN is detected.
///
/// Typical usage:
/// 1. Scale the loss: `scaled_loss = scaler.scale(loss)`
/// 2. Run backward pass on `scaled_loss`
/// 3. Unscale gradients: `scaler.unscale_grads(params)`
/// 4. Check for overflow: `if !scaler.check_overflow(params) { optimizer.step() }`
/// 5. Update scaler: `scaler.update()`
#[derive(Debug, Clone)]
pub struct LossScaler {
    /// Current scale factor.
    scale: f64,
    /// Factor to multiply scale by when no overflow detected for `growth_interval` steps.
    growth_factor: f64,
    /// Factor to multiply scale by when overflow is detected.
    backoff_factor: f64,
    /// Number of consecutive non-overflow steps before growing scale.
    growth_interval: usize,
    /// Counter of consecutive non-overflow steps.
    growth_counter: usize,
    /// Whether overflow was detected in the current step.
    found_overflow: bool,
    /// Whether scaling is enabled.
    enabled: bool,
}

impl LossScaler {
    /// Creates a new loss scaler with default settings.
    ///
    /// Default: scale=65536, growth_factor=2, backoff_factor=0.5, growth_interval=2000.
    pub fn new() -> Self {
        Self {
            scale: 65536.0,
            growth_factor: 2.0,
            backoff_factor: 0.5,
            growth_interval: 2000,
            growth_counter: 0,
            found_overflow: false,
            enabled: true,
        }
    }

    /// Creates a loss scaler with custom parameters.
    pub fn with_params(
        initial_scale: f64,
        growth_factor: f64,
        backoff_factor: f64,
        growth_interval: usize,
    ) -> Self {
        Self {
            scale: initial_scale,
            growth_factor,
            backoff_factor,
            growth_interval,
            growth_counter: 0,
            found_overflow: false,
            enabled: true,
        }
    }

    /// Returns the current scale factor.
    pub fn scale_factor(&self) -> f64 {
        self.scale
    }

    /// Returns whether the scaler is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables scaling.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Scales a loss tensor by the current scale factor.
    ///
    /// Returns a new tensor with values multiplied by the scale.
    pub fn scale_loss(&self, loss_val: &TensorValue) -> TensorValue {
        if !self.enabled {
            return loss_val.clone();
        }
        let scaled = loss_val.data() * self.scale;
        TensorValue::new(scaled, loss_val.requires_grad())
    }

    /// Unscales gradients by dividing by the scale factor.
    ///
    /// Call after backward pass and before optimizer step.
    pub fn unscale_grads(&self, params: &mut [TensorValue]) {
        if !self.enabled {
            return;
        }
        let inv_scale = 1.0 / self.scale;
        for param in params.iter_mut() {
            if let Some(grad) = param.grad().cloned() {
                param.set_grad(grad.mapv(|v| v * inv_scale));
            }
        }
    }

    /// Checks if any parameter gradient contains NaN or Inf.
    ///
    /// Returns `true` if overflow/NaN is detected (optimizer step should be skipped).
    pub fn check_overflow(&mut self, params: &[TensorValue]) -> bool {
        if !self.enabled {
            self.found_overflow = false;
            return false;
        }

        self.found_overflow = false;
        for param in params {
            if let Some(grad) = param.grad() {
                if grad.iter().any(|v| v.is_nan() || v.is_infinite()) {
                    self.found_overflow = true;
                    return true;
                }
            }
        }
        false
    }

    /// Updates the scale factor after a training step.
    ///
    /// - If overflow was detected: scale down by `backoff_factor`, reset counter
    /// - If no overflow: increment counter, scale up when counter reaches `growth_interval`
    pub fn update(&mut self) {
        if !self.enabled {
            return;
        }

        if self.found_overflow {
            self.scale *= self.backoff_factor;
            self.growth_counter = 0;
        } else {
            self.growth_counter += 1;
            if self.growth_counter >= self.growth_interval {
                self.scale *= self.growth_factor;
                self.growth_counter = 0;
            }
        }

        self.found_overflow = false;
    }
}

impl Default for LossScaler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// MixedPrecisionConfig
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for mixed-precision training.
///
/// Specifies which dtype to use for master weights (high precision)
/// and compute weights (lower precision for speed).
#[derive(Debug, Clone)]
pub struct MixedPrecisionConfig {
    /// Dtype for master weights (typically F64 for accuracy).
    pub master_dtype: DType,
    /// Dtype for forward/backward compute (typically F32 for speed).
    pub compute_dtype: DType,
    /// Whether to use loss scaling.
    pub use_loss_scaling: bool,
}

impl MixedPrecisionConfig {
    /// Creates a standard mixed-precision config: F64 master, F32 compute.
    pub fn standard() -> Self {
        Self {
            master_dtype: DType::F64,
            compute_dtype: DType::F32,
            use_loss_scaling: true,
        }
    }

    /// Creates a full-precision config: F64 everything, no scaling.
    pub fn full_precision() -> Self {
        Self {
            master_dtype: DType::F64,
            compute_dtype: DType::F64,
            use_loss_scaling: false,
        }
    }
}

impl Default for MixedPrecisionConfig {
    fn default() -> Self {
        Self::full_precision()
    }
}

/// Creates compute-precision copies of master weight parameters.
///
/// Returns tensors converted to `compute_dtype` for forward pass.
pub fn create_compute_weights(
    master_params: &[TensorValue],
    compute_dtype: DType,
) -> Vec<TensorValue> {
    master_params
        .iter()
        .map(|p| to_dtype(p, compute_dtype))
        .collect()
}

/// Copies gradients from compute weights back to master weights.
///
/// Master weight gradients are always stored in F64.
pub fn copy_grads_to_master(
    master_params: &mut [TensorValue],
    compute_params: &[TensorValue],
) -> Result<(), TensorError> {
    if master_params.len() != compute_params.len() {
        return Err(TensorError::InvalidData {
            reason: format!(
                "master params count ({}) != compute params count ({})",
                master_params.len(),
                compute_params.len()
            ),
        });
    }

    for (master, compute) in master_params.iter_mut().zip(compute_params.iter()) {
        if let Some(grad) = compute.grad() {
            master.set_grad(grad.clone());
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── DType ──

    #[test]
    fn dtype_size_bytes() {
        assert_eq!(DType::F64.size_bytes(), 8);
        assert_eq!(DType::F32.size_bytes(), 4);
    }

    #[test]
    fn dtype_name() {
        assert_eq!(DType::F64.name(), "f64");
        assert_eq!(DType::F32.name(), "f32");
    }

    #[test]
    fn dtype_display() {
        assert_eq!(format!("{}", DType::F64), "f64");
        assert_eq!(format!("{}", DType::F32), "f32");
    }

    #[test]
    fn dtype_equality() {
        assert_eq!(DType::F64, DType::F64);
        assert_ne!(DType::F64, DType::F32);
    }

    // ── to_dtype ──

    #[test]
    fn to_dtype_f64_is_identity() {
        let t = TensorValue::from_data(vec![1.0, 2.5, 3.7], &[3]).unwrap();
        let converted = to_dtype(&t, DType::F64);
        assert_eq!(t.to_vec(), converted.to_vec());
    }

    #[test]
    fn to_dtype_f32_loses_precision() {
        // Use a value that has different f64 vs f32 representation
        let precise_val = 1.0000001192092896_f64; // Not exactly representable in f32
        let t = TensorValue::from_data(vec![precise_val], &[1]).unwrap();
        let converted = to_dtype(&t, DType::F32);
        let result = converted.to_vec()[0];

        // The f32 roundtrip should lose some precision
        let f32_val = precise_val as f32 as f64;
        assert_eq!(result, f32_val);
    }

    #[test]
    fn to_dtype_preserves_requires_grad() {
        let mut t = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        t.set_requires_grad(true);
        let converted = to_dtype(&t, DType::F32);
        assert!(converted.requires_grad());
    }

    #[test]
    fn to_dtype_f32_preserves_shape() {
        let t = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let converted = to_dtype(&t, DType::F32);
        assert_eq!(converted.shape(), &[2, 2]);
    }

    // ── LossScaler ──

    #[test]
    fn loss_scaler_default_scale() {
        let scaler = LossScaler::new();
        assert_eq!(scaler.scale_factor(), 65536.0);
        assert!(scaler.is_enabled());
    }

    #[test]
    fn loss_scaler_scale_loss() {
        let scaler = LossScaler::with_params(100.0, 2.0, 0.5, 10);
        let loss_val = TensorValue::from_data(vec![0.5], &[1]).unwrap();
        let scaled = scaler.scale_loss(&loss_val);
        assert!((scaled.to_vec()[0] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn loss_scaler_unscale_grads() {
        let scaler = LossScaler::with_params(100.0, 2.0, 0.5, 10);
        let mut param = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        param.set_requires_grad(true);
        let grad = ArrayD::from_shape_vec(vec![1], vec![200.0]).unwrap();
        param.set_grad(grad);

        let mut params = vec![param];
        scaler.unscale_grads(&mut params);

        // 200.0 / 100.0 = 2.0
        let unscaled = params[0].grad().unwrap().iter().next().copied().unwrap();
        assert!((unscaled - 2.0).abs() < 1e-10);
    }

    #[test]
    fn loss_scaler_detects_nan_overflow() {
        let mut scaler = LossScaler::new();
        let mut param = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        param.set_requires_grad(true);
        let grad = ArrayD::from_shape_vec(vec![1], vec![f64::NAN]).unwrap();
        param.set_grad(grad);

        let params = vec![param];
        assert!(scaler.check_overflow(&params));
    }

    #[test]
    fn loss_scaler_detects_inf_overflow() {
        let mut scaler = LossScaler::new();
        let mut param = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        param.set_requires_grad(true);
        let grad = ArrayD::from_shape_vec(vec![1], vec![f64::INFINITY]).unwrap();
        param.set_grad(grad);

        let params = vec![param];
        assert!(scaler.check_overflow(&params));
    }

    #[test]
    fn loss_scaler_no_overflow_for_normal_grads() {
        let mut scaler = LossScaler::new();
        let mut param = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        param.set_requires_grad(true);
        let grad = ArrayD::from_shape_vec(vec![1], vec![0.5]).unwrap();
        param.set_grad(grad);

        let params = vec![param];
        assert!(!scaler.check_overflow(&params));
    }

    #[test]
    fn loss_scaler_update_backoff_on_overflow() {
        let mut scaler = LossScaler::with_params(100.0, 2.0, 0.5, 10);
        scaler.found_overflow = true;
        scaler.update();
        assert_eq!(scaler.scale_factor(), 50.0);
        assert_eq!(scaler.growth_counter, 0);
    }

    #[test]
    fn loss_scaler_update_growth_after_interval() {
        let mut scaler = LossScaler::with_params(100.0, 2.0, 0.5, 3);
        // 3 non-overflow steps
        scaler.update();
        scaler.update();
        scaler.update();
        // After 3 steps (growth_interval=3), scale should double
        assert_eq!(scaler.scale_factor(), 200.0);
    }

    #[test]
    fn loss_scaler_disabled_is_noop() {
        let mut scaler = LossScaler::new();
        scaler.set_enabled(false);
        assert!(!scaler.is_enabled());

        let loss_val = TensorValue::from_data(vec![0.5], &[1]).unwrap();
        let scaled = scaler.scale_loss(&loss_val);
        assert_eq!(scaled.to_vec()[0], 0.5); // no scaling
    }

    // ── MixedPrecisionConfig ──

    #[test]
    fn mixed_precision_standard_config() {
        let cfg = MixedPrecisionConfig::standard();
        assert_eq!(cfg.master_dtype, DType::F64);
        assert_eq!(cfg.compute_dtype, DType::F32);
        assert!(cfg.use_loss_scaling);
    }

    #[test]
    fn mixed_precision_full_precision_config() {
        let cfg = MixedPrecisionConfig::full_precision();
        assert_eq!(cfg.master_dtype, DType::F64);
        assert_eq!(cfg.compute_dtype, DType::F64);
        assert!(!cfg.use_loss_scaling);
    }

    // ── Compute weights ──

    #[test]
    fn create_compute_weights_converts_dtype() {
        let master = vec![
            TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap(),
            TensorValue::from_data(vec![3.0, 4.0], &[2]).unwrap(),
        ];
        let compute = create_compute_weights(&master, DType::F32);
        assert_eq!(compute.len(), 2);
        assert_eq!(compute[0].shape(), &[2]);
    }

    #[test]
    fn copy_grads_to_master_transfers_gradients() {
        let mut master = vec![TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap()];
        master[0].set_requires_grad(true);

        let mut compute = vec![TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap()];
        compute[0].set_requires_grad(true);
        let grad = ArrayD::from_shape_vec(vec![2], vec![0.5, 0.6]).unwrap();
        compute[0].set_grad(grad);

        copy_grads_to_master(&mut master, &compute).unwrap();

        let master_grad: Vec<f64> = master[0].grad().unwrap().iter().copied().collect();
        assert_eq!(master_grad, vec![0.5, 0.6]);
    }

    #[test]
    fn copy_grads_mismatched_count_error() {
        let mut master = vec![TensorValue::from_data(vec![1.0], &[1]).unwrap()];
        let compute = vec![
            TensorValue::from_data(vec![1.0], &[1]).unwrap(),
            TensorValue::from_data(vec![2.0], &[1]).unwrap(),
        ];
        let result = copy_grads_to_master(&mut master, &compute);
        assert!(result.is_err());
    }

    #[test]
    fn to_f32_precision_helper() {
        let data =
            ArrayD::from_shape_vec(vec![2], vec![1.1234567890123456, 2.9876543210987654]).unwrap();
        let converted = to_f32_precision(&data);
        // f32 roundtrip
        assert_eq!(converted[[0]], 1.1234567890123456_f64 as f32 as f64);
        assert_eq!(converted[[1]], 2.9876543210987654_f64 as f32 as f64);
    }
}
