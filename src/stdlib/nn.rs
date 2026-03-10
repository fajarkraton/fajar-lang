//! ML/Neural Network standard library bindings.
//!
//! Lists all ML builtin function names. These are registered as `BuiltinFn`
//! values in the interpreter and as typed symbols in the type checker.
//! The actual implementations live in `interpreter::eval` (builtin dispatch)
//! and `runtime::ml` (tensor, ops, autograd, optim, layers).

/// All ML builtin function names.
///
/// Used for documentation and potential future dynamic registration.
pub const ML_BUILTINS: &[&str] = &[
    // Tensor creation
    "tensor_zeros",
    "tensor_ones",
    "tensor_randn",
    "tensor_eye",
    "tensor_full",
    "tensor_from_data",
    // Tensor accessors
    "tensor_shape",
    "tensor_reshape",
    "tensor_numel",
    // Element-wise arithmetic
    "tensor_add",
    "tensor_sub",
    "tensor_mul",
    "tensor_div",
    "tensor_neg",
    // Matrix operations
    "tensor_matmul",
    "tensor_transpose",
    // Reductions
    "tensor_sum",
    "tensor_mean",
    // Activation functions
    "tensor_relu",
    "tensor_sigmoid",
    "tensor_tanh",
    "tensor_softmax",
    "tensor_gelu",
    "tensor_leaky_relu",
    // Loss functions
    "tensor_mse_loss",
    "tensor_cross_entropy",
    "tensor_bce_loss",
    "tensor_l1_loss",
    // Shape manipulation
    "tensor_flatten",
    "tensor_squeeze",
    "tensor_unsqueeze",
    // Additional reductions
    "tensor_max",
    "tensor_min",
    "tensor_argmax",
    // Additional creation
    "tensor_arange",
    "tensor_linspace",
    "tensor_xavier",
];
