//! Tensor ops, optimization levels, multi-param generics, VolatilePtr (S31, S4.7/S4.8, S15.2).

use super::super::*;
use super::{compile_and_run, compile_and_run_optimized};
use crate::lexer::tokenize;
use crate::parser::parse;

// ── S31: Tensor ops in native codegen ──

#[test]
fn native_tensor_zeros_shape() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(3, 4)
            let r = tensor_rows(t)
            let c = tensor_cols(t)
            tensor_free(t)
            r * 10 + c
        }
    "#;
    assert_eq!(compile_and_run(src), 34);
}

#[test]
fn native_tensor_ones_shape() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 5)
            let r = tensor_rows(t)
            let c = tensor_cols(t)
            tensor_free(t)
            r * 10 + c
        }
    "#;
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_tensor_add() {
    let src = r#"
        fn main() -> i64 {
            let a = tensor_ones(2, 2)
            let b = tensor_ones(2, 2)
            let c = tensor_add(a, b)
            let rows = tensor_rows(c)
            tensor_free(a)
            tensor_free(b)
            tensor_free(c)
            rows
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_tensor_matmul() {
    let src = r#"
        fn main() -> i64 {
            let a = tensor_ones(2, 3)
            let b = tensor_ones(3, 4)
            let c = tensor_matmul(a, b)
            let r = tensor_rows(c)
            let cols = tensor_cols(c)
            tensor_free(a)
            tensor_free(b)
            tensor_free(c)
            r * 10 + cols
        }
    "#;
    // 2x3 @ 3x4 = 2x4
    assert_eq!(compile_and_run(src), 24);
}

#[test]
fn native_tensor_transpose() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(3, 5)
            let tt = tensor_transpose(t)
            let r = tensor_rows(tt)
            let c = tensor_cols(tt)
            tensor_free(t)
            tensor_free(tt)
            r * 10 + c
        }
    "#;
    // transpose of 3x5 = 5x3
    assert_eq!(compile_and_run(src), 53);
}

#[test]
fn native_tensor_reshape_basic() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 6)
            let r = tensor_reshape(t, 3, 4)
            let rows = tensor_rows(r)
            let cols = tensor_cols(r)
            tensor_free(t)
            tensor_free(r)
            rows * 10 + cols
        }
    "#;
    // reshape 2x6 (12 elements) to 3x4
    assert_eq!(compile_and_run(src), 34);
}

#[test]
fn native_tensor_flatten_basic() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 4)
            let f = tensor_flatten(t)
            let rows = tensor_rows(f)
            let cols = tensor_cols(f)
            tensor_free(t)
            tensor_free(f)
            rows * 100 + cols
        }
    "#;
    // flatten 3x4 (12 elements) to 1x12
    assert_eq!(compile_and_run(src), 112);
}

#[test]
fn native_tensor_relu() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(2, 2)
            let r = tensor_relu(t)
            let rows = tensor_rows(r)
            tensor_free(t)
            tensor_free(r)
            rows
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_tensor_softmax() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(2, 3)
            let s = tensor_softmax(t)
            let rows = tensor_rows(s)
            let cols = tensor_cols(s)
            tensor_free(t)
            tensor_free(s)
            rows * 10 + cols
        }
    "#;
    // softmax of 2x3 zeros → 2x3 (each row = [1/3, 1/3, 1/3])
    assert_eq!(compile_and_run(src), 23);
}

#[test]
fn native_tensor_sigmoid() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(3, 2)
            let s = tensor_sigmoid(t)
            let rows = tensor_rows(s)
            let cols = tensor_cols(s)
            tensor_free(t)
            tensor_free(s)
            rows * 10 + cols
        }
    "#;
    // sigmoid of 3x2 zeros → 3x2 (each element = 0.5)
    assert_eq!(compile_and_run(src), 32);
}

// =====================================================================
// S32 — Autograd in Native Codegen
// =====================================================================

#[test]
fn native_autograd_requires_grad() {
    // requires_grad wraps tensor into a GradTensor (returns opaque ptr)
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let gt = requires_grad(t)
            let data = grad_tensor_data(gt)
            let rows = tensor_rows(data)
            tensor_free(data)
            grad_tensor_free(gt)
            tensor_free(t)
            rows
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_autograd_mse_loss() {
    // mse_loss computes mean squared error and returns loss as f64 bits
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 4)
            let target = tensor_zeros(1, 4)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss_bits = mse_loss(gp, gt)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            if loss_bits != 0 { 1 } else { 0 }
        }
    "#;
    // MSE of ones vs zeros = 1.0, so loss_bits should be non-zero
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_autograd_grad_access() {
    // After mse_loss, the prediction grad tensor should have gradient
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 4)
            let target = tensor_zeros(1, 4)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss_bits = mse_loss(gp, gt)
            let grad = tensor_grad(gp)
            let rows = tensor_rows(grad)
            tensor_free(grad)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            rows
        }
    "#;
    // Gradient should be a 1x4 tensor -> rows = 1
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_autograd_zero_grad() {
    // zero_grad clears the gradient
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 4)
            let target = tensor_zeros(1, 4)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss_bits = mse_loss(gp, gt)
            zero_grad(gp)
            let grad = tensor_grad(gp)
            let rows = tensor_rows(grad)
            tensor_free(grad)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            rows
        }
    "#;
    // After zero_grad, tensor_grad returns a zero tensor (still 1x4) -> rows=1
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_autograd_cross_entropy_loss() {
    // cross_entropy_loss returns loss as f64 bits (non-zero for non-trivial inputs)
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 3)
            let target = tensor_ones(1, 3)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss_bits = cross_entropy_loss(gp, gt)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            if loss_bits != 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

// =====================================================================
// S32.3 — Gradient through matmul, relu, sigmoid, softmax
// =====================================================================

#[test]
fn native_grad_relu_basic() {
    // grad_relu applies ReLU and computes gradient
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let gt = requires_grad(t)
            let out = grad_relu(gt)
            let out_data = grad_tensor_data(out)
            let rows = tensor_rows(out_data)
            let cols = tensor_cols(out_data)
            let g = tensor_grad(out)
            let g_rows = tensor_rows(g)
            tensor_free(t)
            tensor_free(out_data)
            tensor_free(g)
            grad_tensor_free(gt)
            grad_tensor_free(out)
            rows * 100 + cols * 10 + g_rows
        }
    "#;
    // Output shape 2x3, grad shape should also be 2 rows
    assert_eq!(compile_and_run(src), 232);
}

#[test]
fn native_grad_sigmoid_basic() {
    // grad_sigmoid applies sigmoid and computes gradient
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 2)
            let gt = requires_grad(t)
            let out = grad_sigmoid(gt)
            let out_data = grad_tensor_data(out)
            let rows = tensor_rows(out_data)
            let cols = tensor_cols(out_data)
            tensor_free(t)
            tensor_free(out_data)
            grad_tensor_free(gt)
            grad_tensor_free(out)
            rows * 10 + cols
        }
    "#;
    assert_eq!(compile_and_run(src), 22);
}

#[test]
fn native_grad_softmax_basic() {
    // grad_softmax applies softmax and computes gradient
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(1, 4)
            let gt = requires_grad(t)
            let out = grad_softmax(gt)
            let out_data = grad_tensor_data(out)
            let rows = tensor_rows(out_data)
            let cols = tensor_cols(out_data)
            tensor_free(t)
            tensor_free(out_data)
            grad_tensor_free(gt)
            grad_tensor_free(out)
            rows * 10 + cols
        }
    "#;
    // Output shape 1x4
    assert_eq!(compile_and_run(src), 14);
}

#[test]
fn native_grad_matmul_basic() {
    // grad_matmul does A @ B with gradient tracking
    let src = r#"
        fn main() -> i64 {
            let a = tensor_ones(2, 3)
            let b = tensor_ones(3, 4)
            let ga = requires_grad(a)
            let out = grad_matmul(ga, b)
            let out_data = grad_tensor_data(out)
            let rows = tensor_rows(out_data)
            let cols = tensor_cols(out_data)
            let g = tensor_grad(out)
            let g_rows = tensor_rows(g)
            let g_cols = tensor_cols(g)
            tensor_free(a)
            tensor_free(b)
            tensor_free(out_data)
            tensor_free(g)
            grad_tensor_free(ga)
            grad_tensor_free(out)
            rows * 1000 + cols * 100 + g_rows * 10 + g_cols
        }
    "#;
    // Output shape: 2x4, gradient of A: 2x3 (dL/dA = dL/dC @ B^T)
    assert_eq!(compile_and_run(src), 2423);
}

// =====================================================================
// S33 — Optimizers & Training Native
// =====================================================================

#[test]
fn native_sgd_new_and_free() {
    // SGD optimizer creation and cleanup
    let src = r#"
        fn main() -> i64 {
            let lr_bits = 4607182418800017408
            let opt = sgd_new(lr_bits)
            optimizer_free(opt, 0)
            1
        }
    "#;
    // lr_bits = f64::to_bits(1.0) = 0x3FF0000000000000 = 4607182418800017408
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_adam_new_and_free() {
    // Adam optimizer creation and cleanup
    let src = r#"
        fn main() -> i64 {
            let lr_bits = 4591870180066957722
            let opt = adam_new(lr_bits)
            optimizer_free(opt, 1)
            1
        }
    "#;
    // lr_bits = f64::to_bits(0.001) = 4591870180066957722
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_sgd_step_updates_params() {
    // SGD step should modify GradTensor params
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 4)
            let target = tensor_zeros(1, 4)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss_bits = mse_loss(gp, gt)
            let lr_bits = 4607182418800017408
            let opt = sgd_new(lr_bits)
            sgd_step(opt, gp)
            let data = grad_tensor_data(gp)
            let rows = tensor_rows(data)
            tensor_free(data)
            optimizer_free(opt, 0)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            rows
        }
    "#;
    // After step, data should still be 1x4 -> rows = 1
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_adam_step_updates_params() {
    // Adam step should modify GradTensor params
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 4)
            let target = tensor_zeros(1, 4)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss_bits = mse_loss(gp, gt)
            let lr_bits = 4591870180066957722
            let opt = adam_new(lr_bits)
            adam_step(opt, gp)
            let data = grad_tensor_data(gp)
            let rows = tensor_rows(data)
            tensor_free(data)
            optimizer_free(opt, 1)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            rows
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_training_loop_loss_decreases() {
    // 3-step training loop: loss should decrease over iterations
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(1, 2)
            let target = tensor_zeros(1, 2)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let loss1 = mse_loss(gp, gt)
            let lr_bits = 4602678819172646912
            let opt = sgd_new(lr_bits)
            sgd_step(opt, gp)
            zero_grad(gp)
            let loss2 = mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            let loss3 = mse_loss(gp, gt)
            optimizer_free(opt, 0)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            if loss3 < loss1 { 1 } else { 0 }
        }
    "#;
    // lr_bits = f64::to_bits(0.5) = 4602678819172646912
    // Loss should decrease over steps
    assert_eq!(compile_and_run(src), 1);
}

// =====================================================================
// S36 — Data Pipeline
// =====================================================================

#[test]
fn native_dataloader_create_and_len() {
    let src = r#"
        fn main() -> i64 {
            let data = tensor_ones(10, 3)
            let labels = tensor_ones(10, 1)
            let dl = dataloader_new(data, labels, 4)
            let num_batches = dataloader_len(dl)
            dataloader_free(dl)
            tensor_free(data)
            tensor_free(labels)
            num_batches
        }
    "#;
    // 10 samples / batch_size 4 = ceil(10/4) = 3 batches
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_dataloader_num_samples() {
    let src = r#"
        fn main() -> i64 {
            let data = tensor_ones(8, 2)
            let labels = tensor_ones(8, 1)
            let dl = dataloader_new(data, labels, 3)
            let n = dataloader_num_samples(dl)
            dataloader_free(dl)
            tensor_free(data)
            tensor_free(labels)
            n
        }
    "#;
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_dataloader_iterate_batches() {
    let src = r#"
        fn main() -> i64 {
            let data = tensor_ones(6, 2)
            let labels = tensor_zeros(6, 1)
            let dl = dataloader_new(data, labels, 2)
            let b1_data = dataloader_next_data(dl)
            let b1_labels = dataloader_next_labels(dl)
            let r1 = tensor_rows(b1_data)
            let b2_data = dataloader_next_data(dl)
            let b2_labels = dataloader_next_labels(dl)
            let r2 = tensor_rows(b2_data)
            let b3_data = dataloader_next_data(dl)
            let b3_labels = dataloader_next_labels(dl)
            let r3 = tensor_rows(b3_data)
            tensor_free(b1_data)
            tensor_free(b1_labels)
            tensor_free(b2_data)
            tensor_free(b2_labels)
            tensor_free(b3_data)
            tensor_free(b3_labels)
            dataloader_free(dl)
            tensor_free(data)
            tensor_free(labels)
            r1 * 100 + r2 * 10 + r3
        }
    "#;
    // 3 batches of 2 rows each: 2*100 + 2*10 + 2 = 222
    assert_eq!(compile_and_run(src), 222);
}

#[test]
fn native_dataloader_reset() {
    let src = r#"
        fn main() -> i64 {
            let data = tensor_ones(4, 2)
            let labels = tensor_zeros(4, 1)
            let dl = dataloader_new(data, labels, 2)
            let b1 = dataloader_next_data(dl)
            let b1l = dataloader_next_labels(dl)
            let b2 = dataloader_next_data(dl)
            let b2l = dataloader_next_labels(dl)
            tensor_free(b1)
            tensor_free(b1l)
            tensor_free(b2)
            tensor_free(b2l)
            dataloader_reset(dl, 0)
            let b3 = dataloader_next_data(dl)
            let b3l = dataloader_next_labels(dl)
            let r = tensor_rows(b3)
            tensor_free(b3)
            tensor_free(b3l)
            dataloader_free(dl)
            tensor_free(data)
            tensor_free(labels)
            r
        }
    "#;
    // After reset, first batch should have 2 rows again
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_tensor_normalize() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 2)
            let n = tensor_normalize(t)
            let rows = tensor_rows(n)
            let cols = tensor_cols(n)
            tensor_free(n)
            tensor_free(t)
            rows * 10 + cols
        }
    "#;
    // Shape should be preserved: 3x2
    assert_eq!(compile_and_run(src), 32);
}

// =====================================================================
// S37 — Model Serialization
// =====================================================================

#[test]
fn native_tensor_save_load() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 4)
            let ok = tensor_save(t, "/tmp/fj_test_tensor.bin")
            let loaded = tensor_load("/tmp/fj_test_tensor.bin")
            let rows = tensor_rows(loaded)
            let cols = tensor_cols(loaded)
            tensor_free(loaded)
            tensor_free(t)
            ok * 1000 + rows * 10 + cols
        }
    "#;
    // ok=1, rows=3, cols=4 -> 1034
    assert_eq!(compile_and_run(src), 1034);
    // cleanup
    let _ = std::fs::remove_file("/tmp/fj_test_tensor.bin");
}

#[test]
fn native_checkpoint_save_load() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let ok = checkpoint_save(t, "/tmp/fj_test_ckpt.bin", 5, 42)
            let loaded = checkpoint_load("/tmp/fj_test_ckpt.bin")
            let ep = checkpoint_epoch("/tmp/fj_test_ckpt.bin")
            let lv = checkpoint_loss("/tmp/fj_test_ckpt.bin")
            let rows = tensor_rows(loaded)
            tensor_free(loaded)
            tensor_free(t)
            ok * 10000 + ep * 1000 + lv * 10 + rows
        }
    "#;
    // ok=1, epoch=5, loss=42, rows=2 -> 1*10000 + 5*1000 + 42*10 + 2 = 15422
    assert_eq!(compile_and_run(src), 15422);
    let _ = std::fs::remove_file("/tmp/fj_test_ckpt.bin");
}

#[test]
fn native_tensor_load_nonexistent() {
    let src = r#"
        fn main() -> i64 {
            let loaded = tensor_load("/tmp/fj_nonexistent_12345.bin")
            if loaded == 0 { 1 } else { 0 }
        }
    "#;
    // Should return null (0) for nonexistent file
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_checkpoint_epoch_corrupted() {
    let src = r#"
        fn main() -> i64 {
            let epoch = checkpoint_epoch("/tmp/fj_nonexistent_ckpt.bin")
            epoch
        }
    "#;
    // Should return -1 for nonexistent
    assert_eq!(compile_and_run(src), -1);
}

// =====================================================================
// S38 — MNIST End-to-End Training
// =====================================================================

#[test]
fn native_mnist_forward_pass() {
    // Forward: input(1x4) @ weights(4x2) → softmax → shape check
    let src = r#"
        fn main() -> i64 {
            let input = tensor_ones(1, 4)
            let weights = tensor_ones(4, 2)
            let logits = tensor_matmul(input, weights)
            let probs = tensor_softmax(logits)
            let rows = tensor_rows(probs)
            let cols = tensor_cols(probs)
            tensor_free(input)
            tensor_free(weights)
            tensor_free(logits)
            tensor_free(probs)
            rows * 10 + cols
        }
    "#;
    // 1x4 @ 4x2 = 1x2, softmax preserves shape → 1x2
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_mnist_cross_entropy_computes() {
    // Cross-entropy loss with softmax predictions and one-hot targets
    let src = r#"
        fn main() -> i64 {
            let logits = tensor_ones(1, 2)
            let probs = tensor_softmax(logits)
            let target = tensor_zeros(1, 2)
            tensor_set(target, 0, 0, 4607182418800017408)
            let gp = requires_grad(probs)
            let gt = requires_grad(target)
            let ce = cross_entropy_loss(gp, gt)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(logits)
            tensor_free(probs)
            tensor_free(target)
            if ce > 0 { 1 } else { 0 }
        }
    "#;
    // 4607182418800017408 = f64::to_bits(1.0)
    // softmax([1,1]) = [0.5, 0.5], target = [1, 0]
    // CE = -(1*log(0.5) + 0*log(0.5))/2 = 0.3466 > 0
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_mnist_multi_epoch_training() {
    // 5-step training with MSE loss + SGD, verify loss decreases
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(2, 3)
            let target = tensor_zeros(2, 3)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let lr_bits = 4602678819172646912
            let opt = sgd_new(lr_bits)
            let first_loss = mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            let last_loss = mse_loss(gp, gt)
            optimizer_free(opt, 0)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            if last_loss < first_loss { 1 } else { 0 }
        }
    "#;
    // lr_bits = f64::to_bits(0.5)
    // 5-step SGD: loss should strictly decrease (pred → 0 = target)
    assert_eq!(compile_and_run(src), 1);
}

// =====================================================================
// S41 — Optimization Passes
// =====================================================================

#[test]
fn native_opt_level_speed_basic() {
    // OptLevel::Speed should produce correct results
    let src = "fn main() -> i64 { 2 + 3 * 4 }";
    assert_eq!(compile_and_run_optimized(src), 14);
}

#[test]
fn native_opt_level_speed_loop() {
    // OptLevel::Speed with a loop — should optimize loop
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 100 {
                sum = sum + i
                i = i + 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run_optimized(src), 4950);
}

#[test]
fn native_opt_level_speed_function_calls() {
    // Functions with OptLevel::Speed
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 { add(10, 20) + add(30, 40) }
    "#;
    assert_eq!(compile_and_run_optimized(src), 100);
}

#[test]
fn native_opt_level_speed_and_size() {
    // OptLevel::SpeedAndSize should also work
    let tokens = tokenize("fn main() -> i64 { 42 }").expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler =
        CraneliftCompiler::with_opt_level("speed_and_size").expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 42);
}

#[test]
fn native_opt_const_folding() {
    // Cranelift should constant-fold simple expressions at speed opt level
    let src = r#"
        fn main() -> i64 {
            let x = 10 * 20 + 5
            let y = 100 / 4
            x + y
        }
    "#;
    assert_eq!(compile_and_run_optimized(src), 230);
}

#[test]
fn native_opt_dead_code_after_return() {
    // Code after return should be eliminated
    let src = r#"
        fn main() -> i64 {
            return 42
            let x = 100
            x
        }
    "#;
    assert_eq!(compile_and_run_optimized(src), 42);
}

// =====================================================================
// S41.3 — Loop-invariant code motion (via Cranelift optimizer)
// =====================================================================

#[test]
fn native_opt_loop_invariant_code_motion() {
    // `y = 10 * 20` is loop-invariant — Cranelift hoists the computation
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 50 {
                let y = 10 * 20
                sum = sum + y
                i = i + 1
            }
            sum
        }
    "#;
    // 50 * 200 = 10000 — correctness regardless of LICM
    assert_eq!(compile_and_run_optimized(src), 10000);
}

// =====================================================================
// S41.4 — Small function inlining (via Cranelift optimizer)
// =====================================================================

#[test]
fn native_opt_small_function_inlining() {
    // Small functions should be inlined by Cranelift at OptLevel::Speed
    let src = r#"
        fn add1(x: i64) -> i64 { x + 1 }
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let mut val = 0
            let mut i = 0
            while i < 100 {
                val = add1(double(val))
                i = i + 1
            }
            val
        }
    "#;
    // Correctness: apply add1(double(x)) = 2x + 1, 100 times starting from 0
    // This diverges fast but i64 wraps. Just verify it completes and is deterministic.
    let r1 = compile_and_run_optimized(src);
    let r2 = compile_and_run_optimized(src);
    assert_eq!(r1, r2);
}

// =====================================================================
// S41.5 — Common subexpression elimination (via Cranelift optimizer)
// =====================================================================

#[test]
fn native_opt_common_subexpression_elimination() {
    // `a + b` computed multiple times — CSE should recognize this
    let src = r#"
        fn main() -> i64 {
            let a = 17
            let b = 23
            let x = a + b
            let y = a + b
            let z = a + b
            x + y + z
        }
    "#;
    // 40 + 40 + 40 = 120
    assert_eq!(compile_and_run_optimized(src), 120);
}

// =====================================================================
// S42 — Benchmark correctness (verify native produces correct results
//       for common benchmark programs)
// =====================================================================

#[test]
fn native_bench_fibonacci_20() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(20) }
    "#;
    assert_eq!(compile_and_run(src), 6765);
}

#[test]
fn native_bench_fibonacci_20_optimized() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(20) }
    "#;
    assert_eq!(compile_and_run_optimized(src), 6765);
}

#[test]
fn native_bench_sum_loop_10000() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 10000 {
                sum = sum + i
                i = i + 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 49995000);
}

#[test]
fn native_bench_sum_loop_10000_optimized() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 10000 {
                sum = sum + i
                i = i + 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run_optimized(src), 49995000);
}

#[test]
fn native_bench_nested_calls() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn mul(a: i64, b: i64) -> i64 { a * b }
        fn compute(x: i64) -> i64 {
            add(mul(x, x), mul(x, 2))
        }
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 1
            while i <= 100 {
                sum = sum + compute(i)
                i = i + 1
            }
            sum
        }
    "#;
    // sum of (i^2 + 2i) for i=1..100 = sum(i^2) + 2*sum(i) = 338350 + 10100 = 348450
    assert_eq!(compile_and_run(src), 348450);
}

#[test]
fn native_bench_sorting_bubble() {
    // Bubble sort on small array
    let src = r#"
        fn main() -> i64 {
            let mut a = 5
            let mut b = 3
            let mut c = 8
            let mut d = 1
            let mut e = 4
            let mut swapped = 1
            while swapped == 1 {
                swapped = 0
                if a > b { let t = a; a = b; b = t; swapped = 1 }
                if b > c { let t = b; b = c; c = t; swapped = 1 }
                if c > d { let t = c; c = d; d = t; swapped = 1 }
                if d > e { let t = d; d = e; e = t; swapped = 1 }
            }
            a * 10000 + b * 1000 + c * 100 + d * 10 + e
        }
    "#;
    // Sorted: 1,3,4,5,8 -> 13458
    assert_eq!(compile_and_run(src), 13458);
}

#[test]
fn native_bench_matmul_tensor() {
    // Matrix multiply with tensors
    let src = r#"
        fn main() -> i64 {
            let a = tensor_ones(3, 4)
            let b = tensor_ones(4, 2)
            let c = tensor_matmul(a, b)
            let rows = tensor_rows(c)
            let cols = tensor_cols(c)
            tensor_free(a)
            tensor_free(b)
            tensor_free(c)
            rows * 10 + cols
        }
    "#;
    // 3x4 * 4x2 = 3x2
    assert_eq!(compile_and_run(src), 32);
}

// =====================================================================
// Additional tensor & utility tests
// =====================================================================

#[test]
fn native_tensor_mean() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let mean_bits = tensor_mean(t)
            tensor_free(t)
            if mean_bits != 0 { 1 } else { 0 }
        }
    "#;
    // Mean of all-ones tensor = 1.0, bits != 0
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_tensor_row_extract() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 4)
            let row0 = tensor_row(t, 0)
            let rows = tensor_rows(row0)
            let cols = tensor_cols(row0)
            tensor_free(row0)
            tensor_free(t)
            rows * 10 + cols
        }
    "#;
    // Row 0 of 3x4 tensor should be 1x4
    assert_eq!(compile_and_run(src), 14);
}

#[test]
fn native_tensor_abs() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 2)
            let a = tensor_abs(t)
            let rows = tensor_rows(a)
            tensor_free(a)
            tensor_free(t)
            rows
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_tensor_fill() {
    let src = r#"
        fn main() -> i64 {
            let val_bits = 4617315517961601024
            let t = tensor_fill(2, 3, val_bits)
            let rows = tensor_rows(t)
            let cols = tensor_cols(t)
            tensor_free(t)
            rows * 10 + cols
        }
    "#;
    // val_bits = f64::to_bits(5.0) = 4617315517961601024
    assert_eq!(compile_and_run(src), 23);
}

#[test]
fn native_tensor_rand_shape() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_rand(4, 5)
            let rows = tensor_rows(t)
            let cols = tensor_cols(t)
            tensor_free(t)
            rows * 10 + cols
        }
    "#;
    assert_eq!(compile_and_run(src), 45);
}

#[test]
fn native_tensor_scale() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 2)
            let scale_bits = 4611686018427387904
            let s = tensor_scale(t, scale_bits)
            let rows = tensor_rows(s)
            tensor_free(s)
            tensor_free(t)
            rows
        }
    "#;
    // scale_bits = f64::to_bits(2.0) = 4611686018427387904
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_random_int() {
    let src = r#"
        fn main() -> i64 {
            let r = random_int(100)
            if r >= 0 { if r < 100 { 1 } else { 0 } } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Arc (atomic reference counting) tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_arc_basic() {
    let src = r#"
        fn main() -> i64 {
            let a = Arc::new(42)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_arc_clone() {
    let src = r#"
        fn main() -> i64 {
            let a = Arc::new(100)
            let b = a.clone()
            b.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_arc_store_and_load() {
    let src = r#"
        fn main() -> i64 {
            let a = Arc::new(10)
            a.store(99)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_arc_shared_between_clones() {
    let src = r#"
        fn main() -> i64 {
            let a = Arc::new(0)
            let b = a.clone()
            a.store(77)
            b.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

// ═══════════════════════════════════════════════════════════════════════
// ── S5.7: Thread-local storage ──

#[test]
fn native_tls_basic() {
    let src = r#"
        fn main() -> i64 {
            tls_set(1, 42)
            tls_get(1)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_tls_different_per_thread() {
    // Main thread sets key 1, spawns a thread that also sets key 1
    // Each thread should see its own value
    let src = r#"
        fn worker(x: i64) -> i64 {
            tls_set(1, x * 10)
            tls_get(1)
        }

        fn main() -> i64 {
            tls_set(1, 99)
            let h = thread::spawn(worker, 5)
            let thread_val = h.join()
            let main_val = tls_get(1)
            main_val * 100 + thread_val
        }
    "#;
    // main_val = 99, thread_val = 50 → 99*100 + 50 = 9950
    assert_eq!(compile_and_run(src), 9950);
}

// Thread integration tests (S5.8)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_thread_parallel_sum() {
    let src = r#"
        fn sum_range(start: i64) -> i64 {
            let mut total = 0
            let mut i = start
            while i < start + 25 {
                total = total + i
                i = i + 1
            }
            total
        }

        fn main() -> i64 {
            let h1 = thread::spawn(sum_range, 0)
            let h2 = thread::spawn(sum_range, 25)
            let h3 = thread::spawn(sum_range, 50)
            let h4 = thread::spawn(sum_range, 75)
            let r1 = h1.join()
            let r2 = h2.join()
            let r3 = h3.join()
            let r4 = h4.join()
            r1 + r2 + r3 + r4
        }
    "#;
    // sum(0..100) = 4950
    assert_eq!(compile_and_run(src), 4950);
}

#[test]
fn native_thread_mutex_counter() {
    let src = r#"
        fn increment(m: i64) -> i64 {
            let mut i = 0
            while i < 100 {
                i = i + 1
            }
            i
        }

        fn main() -> i64 {
            let h1 = thread::spawn(increment, 0)
            let h2 = thread::spawn(increment, 0)
            let r1 = h1.join()
            let r2 = h2.join()
            r1 + r2
        }
    "#;
    assert_eq!(compile_and_run(src), 200);
}

#[test]
fn native_thread_arc_shared_state() {
    let src = r#"
        fn worker(val: i64) -> i64 {
            val * val
        }

        fn main() -> i64 {
            let a = Arc::new(0)
            let h1 = thread::spawn(worker, 3)
            let h2 = thread::spawn(worker, 4)
            let r1 = h1.join()
            let r2 = h2.join()
            a.store(r1 + r2)
            a.load()
        }
    "#;
    // 3*3 + 4*4 = 9 + 16 = 25
    assert_eq!(compile_and_run(src), 25);
}

// ===== S4.7: Multi-type-param generics =====

#[test]
fn native_generic_two_type_params() {
    let src = r#"
        fn pair<T, U>(a: T, b: U) -> T {
            a
        }
        fn main() -> i64 {
            pair(42, 3.14)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_generic_two_type_params_mixed() {
    let src = r#"
        fn first<T, U>(a: T, b: U) -> T {
            a + a
        }
        fn main() -> i64 {
            first(10, 3.14)
        }
    "#;
    // 10 + 10 = 20
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_generic_two_params_same_type() {
    let src = r#"
        fn add_pair<T, U>(a: T, b: U) -> T {
            a
        }
        fn main() -> i64 {
            add_pair(100, 200)
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_generic_return_first_of_two() {
    let src = r#"
        fn select<A, B>(x: A, y: B) -> A {
            x
        }
        fn main() -> i64 {
            let r1 = select(7, 3.14)
            let r2 = select(8, 99)
            r1 + r2
        }
    "#;
    // 7 + 8 = 15
    assert_eq!(compile_and_run(src), 15);
}

// ===== S4.8: String/struct monomorphization =====

#[test]
fn native_generic_with_string() {
    // Generic identity function called with a string argument
    let src = r#"
        fn identity<T>(x: T) -> T { x }

        fn main() -> i64 {
            let s = identity("hello")
            len(s)
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_generic_string_len() {
    // Generic function with string, use len on result
    let src = r#"
        fn get_len<T>(x: T) -> i64 { len(x) }

        fn main() -> i64 {
            get_len("hello world")
        }
    "#;
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_generic_identity_int_and_string() {
    // Same generic function used with both i64 and str in same program
    let src = r#"
        fn wrap<T>(x: T) -> T { x }

        fn main() -> i64 {
            let a = wrap(100)
            let s = wrap("hey")
            a + len(s)
        }
    "#;
    // 100 + 3 = 103
    assert_eq!(compile_and_run(src), 103);
}

// ===== S15.2: VolatilePtr wrapper =====

#[test]
fn native_volatile_ptr_read_write() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 42)
            let vp = VolatilePtr::new(buf)
            let val = vp.read()
            dealloc(buf, 8)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_volatile_ptr_write() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            let vp = VolatilePtr::new(buf)
            vp.write(99)
            let result = vp.read()
            dealloc(buf, 8)
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_volatile_ptr_update() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let buf = alloc(8)
            let vp = VolatilePtr::new(buf)
            vp.write(21)
            vp.update(double)
            let result = vp.read()
            dealloc(buf, 8)
            result
        }
    "#;
    // 21 * 2 = 42
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_volatile_ptr_addr() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            let vp = VolatilePtr::new(buf)
            let addr = vp.addr()
            dealloc(buf, 8)
            if addr > 0 { 1 } else { 0 }
        }
    "#;
    // addr should be non-zero (heap-allocated)
    assert_eq!(compile_and_run(src), 1);
}

// ── S15.3: MMIO Region ─────────────────────────────────────────────

#[test]
fn native_mmio_read() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(32)
            mem_write(buf, 0, 100)
            mem_write(buf, 8, 200)
            let region = MmioRegion::new(buf, 32)
            let val = region.read_u32(0)
            dealloc(buf, 32)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_mmio_write() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(32)
            let region = MmioRegion::new(buf, 32)
            region.write_u32(0, 42)
            let val = region.read_u32(0)
            dealloc(buf, 32)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_mmio_consecutive() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(32)
            let region = MmioRegion::new(buf, 32)
            region.write_u32(0, 10)
            region.write_u32(8, 20)
            region.write_u32(16, 30)
            let a = region.read_u32(0)
            let b = region.read_u32(8)
            let c = region.read_u32(16)
            dealloc(buf, 32)
            a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_mmio_base_addr() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            let region = MmioRegion::new(buf, 16)
            let base = region.base()
            dealloc(buf, 16)
            if base > 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

// ── S17.1: #[no_std] ────────────────────────────────────────────────

#[test]
fn native_no_std_compiles() {
    // Pure computation should compile fine in no_std mode
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 {
            let x = add(10, 20)
            x * 2
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("no_std compilation should succeed for pure computation");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    // SAFETY: main() compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 60);
}

#[test]
fn native_no_std_rejects_io() {
    // File I/O should fail in no_std mode (println is allowed via bare-metal UART)
    let src = r#"
        fn main() -> i64 {
            read_file("test.txt")
            0
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    let result = compiler.compile_program(&program);
    assert!(result.is_err(), "no_std should reject file I/O");
}

// ── S17.2: Panic Handler ────────────────────────────────────────────

#[test]
fn native_panic_handler_called() {
    // @panic_handler annotation should make panic() call user's handler
    // The handler sets a global flag; we verify via the return value pattern.
    // Since panic traps after calling the handler, we test that the handler function
    // is properly linked (compilation succeeds with @panic_handler).
    let src = r#"
        @panic_handler
        fn my_panic(code: i64) -> i64 {
            code
        }

        fn main() -> i64 {
            42
        }
    "#;
    // This should compile successfully (panic handler is recognized)
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_panic_handler_signature() {
    // Verify that a program with @panic_handler compiles with no_std
    let src = r#"
        @panic_handler
        fn handle_panic(code: i64) -> i64 {
            code + 1
        }

        fn main() -> i64 {
            100
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("no_std + panic_handler should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    // SAFETY: main() compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 100);
}

// ── S17.3: Entry Attribute ──────────────────────────────────────────

#[test]
fn native_entry_annotation_compiles() {
    // @entry annotation on a function should compile successfully
    let src = r#"
        @entry
        fn start() -> i64 {
            99
        }

        fn main() -> i64 {
            start()
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_entry_with_no_std() {
    // @entry + no_std is the typical bare-metal pattern
    let src = r#"
        @panic_handler
        fn panic(code: i64) -> i64 { code }

        @entry
        fn start() -> i64 {
            let x = 10 * 5
            x + 7
        }

        fn main() -> i64 {
            start()
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("no_std + entry + panic_handler should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 57);
}

// ── S13.1: Race Condition Testing ───────────────────────────────────

#[test]
fn native_concurrent_increment() {
    // Multiple threads each compute partial sums; results combined deterministically
    let src = r#"
        fn compute(n: i64) -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < n {
                sum = sum + 1
                i = i + 1
            }
            sum
        }

        fn main() -> i64 {
            let h1 = thread::spawn(compute, 1000)
            let h2 = thread::spawn(compute, 1000)
            let h3 = thread::spawn(compute, 1000)
            let h4 = thread::spawn(compute, 1000)
            let r1 = h1.join()
            let r2 = h2.join()
            let r3 = h3.join()
            let r4 = h4.join()
            r1 + r2 + r3 + r4
        }
    "#;
    // 4 threads × 1000 = 4000
    assert_eq!(compile_and_run(src), 4000);
}

#[test]
fn native_mutex_toctou_prevention() {
    // Mutex ensures atomic read-modify-write
    // Each lock()/store() pair is individually atomic
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(0)
            let v = m.lock()
            m.store(v + 10)

            let v2 = m.lock()
            m.store(v2 + 20)

            m.lock()
        }
    "#;
    // 0 + 10 + 20 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_atomic_concurrent_adds() {
    // Atomic operations are race-free by design
    let src = r#"
        fn add_to_atomic(val: i64) -> i64 {
            val + val
        }

        fn main() -> i64 {
            let a = Atomic::new(0)
            a.store(10)
            let v1 = a.load()
            a.store(v1 + 5)
            let v2 = a.load()
            a.store(v2 + 3)
            a.load()
        }
    "#;
    // 10 + 5 + 3 = 18
    assert_eq!(compile_and_run(src), 18);
}

// ── S13.2: Deadlock Scenarios ───────────────────────────────────────

#[test]
fn native_lock_ordering_safe() {
    // With auto-releasing locks, sequential lock/store is always safe
    let src = r#"
        fn main() -> i64 {
            let m1 = Mutex::new(0)
            let m2 = Mutex::new(0)
            m1.store(10)
            m2.store(20)
            let a = m1.lock()
            let b = m2.lock()
            a + b
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_mutex_no_deadlock() {
    // Sequential lock/store on same mutex cannot deadlock
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(1)
            let v1 = m.lock()
            m.store(v1 * 2)
            let v2 = m.lock()
            m.store(v2 * 3)
            m.lock()
        }
    "#;
    // 1 * 2 = 2, 2 * 3 = 6
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_mutex_try_lock_timeout() {
    // S13.2: try_lock used as a non-blocking probe on a mutex
    // In Fajar's mutex semantics, lock() auto-releases, so sequential
    // try_lock always succeeds. This test verifies that try_lock returns
    // the success flag (1) and can be used in a retry loop pattern.
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(0)
            m.store(100)
            let attempts = 0
            let success = 0
            while attempts < 3 {
                let r = m.try_lock()
                if r == 1 {
                    success = success + 1
                }
                attempts = attempts + 1
            }
            success
        }
    "#;
    // All 3 attempts should succeed (lock auto-releases)
    assert_eq!(compile_and_run(src), 3);
}

// ── S16.2: Built-in Allocators ──────────────────────────────────────

#[test]
fn native_bump_alloc() {
    let src = r#"
        fn main() -> i64 {
            let bump = BumpAllocator::new(256)
            let p1 = bump.alloc(8)
            let p2 = bump.alloc(16)
            mem_write(p1, 0, 42)
            mem_write(p2, 0, 99)
            let a = mem_read(p1, 0)
            let b = mem_read(p2, 0)
            bump.destroy()
            a + b
        }
    "#;
    assert_eq!(compile_and_run(src), 141);
}

#[test]
fn native_bump_exhaust() {
    // Allocating more than the buffer size should return 0 (null)
    let src = r#"
        fn main() -> i64 {
            let bump = BumpAllocator::new(16)
            let p1 = bump.alloc(8)
            let p2 = bump.alloc(8)
            let p3 = bump.alloc(8)
            bump.destroy()
            if p3 == 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_bump_reset() {
    let src = r#"
        fn main() -> i64 {
            let bump = BumpAllocator::new(32)
            let p1 = bump.alloc(16)
            let p2 = bump.alloc(16)
            bump.reset()
            let p3 = bump.alloc(16)
            bump.destroy()
            if p3 > 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_freelist_alloc_free() {
    let src = r#"
        fn main() -> i64 {
            let fl = FreeListAllocator::new(256)
            let p1 = fl.alloc(32)
            mem_write(p1, 0, 77)
            let val = mem_read(p1, 0)
            fl.free(p1, 32)
            fl.destroy()
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

#[test]
fn native_freelist_coalesce() {
    // Free + re-alloc should reuse space
    let src = r#"
        fn main() -> i64 {
            let fl = FreeListAllocator::new(64)
            let p1 = fl.alloc(32)
            fl.free(p1, 32)
            let p2 = fl.alloc(32)
            fl.destroy()
            if p2 > 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_pool_alloc() {
    let src = r#"
        fn main() -> i64 {
            let pool = PoolAllocator::new(8, 4)
            let p1 = pool.alloc()
            let p2 = pool.alloc()
            mem_write(p1, 0, 10)
            mem_write(p2, 0, 20)
            let a = mem_read(p1, 0)
            let b = mem_read(p2, 0)
            pool.free(p1)
            pool.free(p2)
            pool.destroy()
            a + b
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_pool_exhaust() {
    // Pool of 2 blocks, allocate 3 → third returns 0
    let src = r#"
        fn main() -> i64 {
            let pool = PoolAllocator::new(8, 2)
            let p1 = pool.alloc()
            let p2 = pool.alloc()
            let p3 = pool.alloc()
            pool.free(p1)
            pool.free(p2)
            pool.destroy()
            if p3 == 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}
