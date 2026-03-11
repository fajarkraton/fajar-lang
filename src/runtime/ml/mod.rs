//! ML runtime — tensor operations and automatic differentiation.
//!
//! Only accessible from `@device` or `@unsafe` context.

pub mod autograd;
pub mod data;
pub mod dataloader;
pub mod export;
pub mod fixed_point;
pub mod layers;
pub mod metrics;
pub mod mixed_precision;
pub mod ops;
pub mod optim;
pub mod quantize;
pub mod rnn;
pub mod serialize;
pub mod stack_tensor;
pub mod tensor;

pub use autograd::{Tape, TensorId};
pub use ops as tensor_ops;
pub use tensor::{TensorError, TensorValue};
