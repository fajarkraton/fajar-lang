//! ML runtime — tensor operations and automatic differentiation.
//!
//! Only accessible from `@device` or `@unsafe` context.

pub mod autograd;
pub mod backend;
pub mod bf16;
pub mod compression;
pub mod custom_grad;
pub mod data;
pub mod dataloader;
pub mod distillation;
pub mod distributed;
pub mod export;
pub mod fajarquant;
pub mod fixed_point;
pub mod fp4;
pub mod fp8;
pub mod gpu;
pub mod layers;
pub mod metrics;
pub mod mixed_precision;
pub mod model_formats;
pub mod npu;
pub mod ops;
pub mod optim;
pub mod pruning;
pub mod quantize;
pub mod rnn;
pub mod serialize;
pub mod sparsity;
pub mod stack_tensor;
pub mod tensor;
pub mod tflite;
pub mod transformer;
pub mod turboquant;

pub use autograd::{Tape, TensorId};
pub use ops as tensor_ops;
pub use tensor::{TensorError, TensorValue};

// Re-export advanced ML modules (transformer inference, diffusion, RL, serving).
pub use crate::ml_advanced;

/// Returns the list of advanced ML subsystem names.
pub fn ml_advanced_subsystems() -> Vec<&'static str> {
    vec!["transformer", "diffusion", "reinforcement", "serving"]
}
