//! Integration tests for the ML runtime builtins.
//!
//! Tests that Fajar Lang code can create and manipulate tensors.

use fajar_lang::interpreter::Interpreter;

/// Helper: evaluates source and returns captured output.
fn eval_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).expect("eval_source failed");
    interp.get_output().to_vec()
}

// ── Tensor creation ──

#[test]
fn ml_tensor_zeros() {
    let source = r#"
        let t = tensor_zeros([2, 3])
        println(type_of(t))
        println(tensor_numel(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor", "6"]);
}

#[test]
fn ml_tensor_ones() {
    let source = r#"
        let t = tensor_ones([3])
        println(t)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor([1.0000, 1.0000, 1.0000])"]);
}

#[test]
fn ml_tensor_eye() {
    let source = r#"
        let t = tensor_eye(2)
        println(tensor_numel(t))
        let s = tensor_shape(t)
        println(s)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["4", "[2, 2]"]);
}

#[test]
fn ml_tensor_full() {
    let source = r#"
        let t = tensor_full([2, 2], 42.0)
        println(tensor_numel(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["4"]);
}

#[test]
fn ml_tensor_from_data() {
    let source = r#"
        let t = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        println(tensor_shape(t))
        println(tensor_numel(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[2, 3]", "6"]);
}

#[test]
fn ml_tensor_randn() {
    let source = r#"
        let t = tensor_randn([5, 5])
        println(tensor_numel(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["25"]);
}

// ── Tensor operations ──

#[test]
fn ml_tensor_reshape() {
    let source = r#"
        let t = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let t2 = tensor_reshape(t, [3, 2])
        println(tensor_shape(t2))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[3, 2]"]);
}

#[test]
fn ml_tensor_reshape_invalid() {
    let source = r#"
        let t = tensor_from_data([1.0, 2.0, 3.0], [3])
        tensor_reshape(t, [2, 2])
    "#;
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(source);
    assert!(result.is_err());
}

// ── Tensor arithmetic ──

#[test]
fn ml_tensor_add() {
    let source = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_from_data([4.0, 5.0, 6.0], [3])
        let c = tensor_add(a, b)
        println(c)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor([5.0000, 7.0000, 9.0000])"]);
}

#[test]
fn ml_tensor_matmul() {
    let source = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        let b = tensor_eye(2)
        let c = tensor_matmul(a, b)
        println(tensor_shape(c))
        println(tensor_numel(c))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[2, 2]", "4"]);
}

#[test]
fn ml_tensor_transpose() {
    let source = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let b = tensor_transpose(a)
        println(tensor_shape(b))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[3, 2]"]);
}

// ── Activation functions ──

#[test]
fn ml_tensor_relu() {
    let source = r#"
        let a = tensor_from_data([-2.0, -1.0, 0.0, 1.0, 2.0], [5])
        let b = tensor_relu(a)
        println(b)
    "#;
    let output = eval_output(source);
    assert_eq!(
        output,
        vec!["tensor([0.0000, 0.0000, 0.0000, 1.0000, 2.0000])"]
    );
}

#[test]
fn ml_tensor_sigmoid() {
    let source = r#"
        let a = tensor_from_data([0.0], [1])
        let b = tensor_sigmoid(a)
        println(b)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor([0.5000])"]);
}

#[test]
fn ml_tensor_softmax() {
    let source = r#"
        let a = tensor_from_data([1.0, 1.0, 1.0], [3])
        let b = tensor_softmax(a)
        println(tensor_numel(b))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["3"]);
}

// ── Loss functions ──

#[test]
fn ml_tensor_mse_loss() {
    let source = r#"
        let pred = tensor_from_data([1.0, 2.0], [2])
        let target = tensor_from_data([1.0, 2.0], [2])
        let result = tensor_mse_loss(pred, target)
        println(result)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor(0)"]);
}

#[test]
fn ml_tensor_bce_loss() {
    let source = r#"
        let pred = tensor_from_data([0.5], [1])
        let target = tensor_from_data([1.0], [1])
        let result = tensor_bce_loss(pred, target)
        println(type_of(result))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor"]);
}

// ── Reductions ──

#[test]
fn ml_tensor_sum_mean() {
    let source = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0], [4])
        let s = tensor_sum(a)
        println(s)
        let m = tensor_mean(a)
        println(m)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor(10)", "tensor(2.5)"]);
}

// ── MNIST-style forward pass (784→128→10) ──

#[test]
fn ml_mnist_forward_pass() {
    // Simulated MNIST forward pass:
    // Input: [1, 784] (batch=1, 28x28 image flattened)
    // Layer 1: [784, 128] weights → relu → [1, 128]
    // Layer 2: [128, 10] weights → softmax → [1, 10]
    use fajar_lang::runtime::ml::{tensor_ops, TensorValue};

    let input = TensorValue::randn(&[1, 784]);
    let w1 = TensorValue::randn(&[784, 128]);
    let w2 = TensorValue::randn(&[128, 10]);

    // Layer 1: relu(input @ w1)
    let h1 = tensor_ops::matmul(&input, &w1).unwrap();
    let h1_act = tensor_ops::relu(&h1);
    assert_eq!(h1_act.shape(), &[1, 128]);

    // Layer 2: softmax(h1 @ w2)
    let h2 = tensor_ops::matmul(&h1_act, &w2).unwrap();
    let output = tensor_ops::softmax(&h2);
    assert_eq!(output.shape(), &[1, 10]);

    // Softmax output should sum to 1
    let sum: f64 = output.to_vec().iter().sum();
    assert!((sum - 1.0).abs() < 1e-6, "softmax output sum: {sum}");

    // All values should be non-negative (some may underflow to 0 with large random weights)
    assert!(output.to_vec().iter().all(|&v| v >= 0.0 && v.is_finite()));
}

// ── XOR training (convergence test) ──

#[test]
fn ml_xor_gradient_flow() {
    // Test that gradients flow correctly through a simple computation
    use fajar_lang::runtime::ml::{tensor_ops, Tape, TensorValue};

    let mut tape = Tape::new();
    let data = vec![0.5, -0.3, 1.2];
    let mut x = TensorValue::from_data(data.clone(), &[3]).unwrap();
    x.set_requires_grad(true);
    x.set_id(tape.fresh_id());

    // Compute: loss = sum(relu(x))
    let y = tensor_ops::relu_tracked(&x, &mut tape);
    let loss = tensor_ops::sum_tracked(&y, &mut tape);

    let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
    let grad_x: Vec<f64> = grads[&x.id().unwrap()].iter().copied().collect();

    // relu grad: 0 for negative, 1 for positive
    // x = [0.5, -0.3, 1.2] → relu grad = [1, 0, 1]
    assert_eq!(grad_x, vec![1.0, 0.0, 1.0]);
}

// ── Gradient correctness (numerical vs analytical) ──

#[test]
fn ml_gradient_correctness_mul_chain() {
    use fajar_lang::runtime::ml::{tensor_ops, Tape, TensorValue};

    let mut tape = Tape::new();
    let data_x = vec![2.0, 3.0];
    let data_y = vec![4.0, 5.0];

    let mut x = TensorValue::from_data(data_x.clone(), &[2]).unwrap();
    let y = TensorValue::from_data(data_y.clone(), &[2]).unwrap();
    x.set_requires_grad(true);
    x.set_id(tape.fresh_id());

    // loss = sum(x * y) = 2*4 + 3*5 = 23
    let prod = tensor_ops::mul_tracked(&x, &y, &mut tape).unwrap();
    let loss = tensor_ops::sum_tracked(&prod, &mut tape);

    let grads = tape.backward(loss.id().unwrap(), loss.shape()).unwrap();
    let grad_x: Vec<f64> = grads[&x.id().unwrap()].iter().copied().collect();

    // d(sum(x*y))/dx = y = [4, 5]
    assert_eq!(grad_x, vec![4.0, 5.0]);

    // Numerical check
    let num = tensor_ops::numerical_gradient(
        |xv| {
            let xt = TensorValue::from_data(xv.to_vec(), &[2]).unwrap();
            let yt = TensorValue::from_data(data_y.clone(), &[2]).unwrap();
            let p = tensor_ops::mul(&xt, &yt).unwrap();
            tensor_ops::sum(&p).to_scalar().unwrap()
        },
        &data_x,
        1e-5,
    );
    for (a, n) in grad_x.iter().zip(num.iter()) {
        assert!(
            (a - n).abs() < 1e-4,
            "grad mismatch: analytical={a}, numerical={n}"
        );
    }
}

// ── KE002: tensor builtins blocked in @kernel ──

#[test]
fn ml_kernel_cannot_use_tensor_zeros() {
    let source = r#"@kernel fn bad() { tensor_zeros([2, 3]) }"#;
    let errors = fajar_lang::analyzer::analyze(
        &fajar_lang::parser::parse(fajar_lang::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| {
        matches!(
            e,
            fajar_lang::analyzer::type_check::SemanticError::TensorInKernel { .. }
        )
    }));
}

#[test]
fn ml_kernel_cannot_use_tensor_relu() {
    let source = r#"@kernel fn bad() { let t = tensor_zeros([3]); tensor_relu(t) }"#;
    let errors = fajar_lang::analyzer::analyze(
        &fajar_lang::parser::parse(fajar_lang::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| {
        matches!(
            e,
            fajar_lang::analyzer::type_check::SemanticError::TensorInKernel { .. }
        )
    }));
}

#[test]
fn ml_kernel_cannot_use_loss_functions() {
    let source = r#"@kernel fn bad() { tensor_mse_loss(tensor_zeros([3]), tensor_zeros([3])) }"#;
    let errors = fajar_lang::analyzer::analyze(
        &fajar_lang::parser::parse(fajar_lang::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| {
        matches!(
            e,
            fajar_lang::analyzer::type_check::SemanticError::TensorInKernel { .. }
        )
    }));
}

// ── New gap-fix builtins ──

#[test]
fn ml_tensor_flatten() {
    let source = r#"
        let t = tensor_ones([2, 3])
        let flat = tensor_flatten(t)
        println(tensor_numel(flat))
        println(tensor_shape(flat))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["6", "[6]"]);
}

#[test]
fn ml_tensor_squeeze_unsqueeze() {
    let source = r#"
        let t = tensor_ones([3])
        let u = tensor_unsqueeze(t, 0)
        println(tensor_shape(u))
        let s = tensor_squeeze(u, 0)
        println(tensor_shape(s))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[1, 3]", "[3]"]);
}

#[test]
fn ml_tensor_max_min_argmax() {
    let source = r#"
        let t = tensor_from_data([3.0, 1.0, 4.0, 1.0, 5.0], [5])
        println(tensor_max(t))
        println(tensor_min(t))
        println(tensor_argmax(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output[0], "tensor(5)");
    assert_eq!(output[1], "tensor(1)");
    assert_eq!(output[2], "tensor(4)");
}

#[test]
fn ml_tensor_arange() {
    let source = r#"
        let t = tensor_arange(0.0, 5.0, 1.0)
        println(tensor_numel(t))
        println(tensor_shape(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["5", "[5]"]);
}

#[test]
fn ml_tensor_linspace() {
    let source = r#"
        let t = tensor_linspace(0.0, 1.0, 5)
        println(tensor_numel(t))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["5"]);
}

#[test]
fn ml_tensor_xavier() {
    let source = r#"
        let w = tensor_xavier(3, 4)
        println(tensor_shape(w))
        println(tensor_numel(w))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[3, 4]", "12"]);
}

#[test]
fn ml_tensor_l1_loss() {
    let source = r#"
        let pred = tensor_from_data([1.0, 2.0], [2])
        let target = tensor_from_data([3.0, 5.0], [2])
        let result = tensor_l1_loss(pred, target)
        println(result)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor(2.5)"]);
}

#[test]
fn ml_kernel_cannot_use_new_builtins() {
    let source = r#"@kernel fn bad() { tensor_flatten(tensor_zeros([2, 3])) }"#;
    let errors = fajar_lang::analyzer::analyze(
        &fajar_lang::parser::parse(fajar_lang::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| {
        matches!(
            e,
            fajar_lang::analyzer::type_check::SemanticError::TensorInKernel { .. }
        )
    }));
}

// ── Autograd builtins ──

#[test]
fn ml_tensor_requires_grad() {
    let source = r#"
        let t = tensor_ones([3])
        println(tensor_requires_grad(t))
        let t2 = tensor_set_requires_grad(t, true)
        println(tensor_requires_grad(t2))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["false", "true"]);
}

#[test]
fn ml_tensor_backward_and_grad() {
    // Simple: y = x * 2, backward gives grad(x) = 2
    let source = r#"
        let x = tensor_set_requires_grad(tensor_from_data([3.0], [1]), true)
        let y = tensor_mul(x, tensor_full([1], 2.0))
        tensor_backward(y)
        let g = tensor_grad(y)
        println(type_of(g))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor"]);
}

// ── Optimizer builtins ──

#[test]
fn ml_optimizer_sgd_create() {
    let source = r#"
        let opt = optimizer_sgd(0.01, 0.0)
        println(type_of(opt))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["optimizer"]);
}

#[test]
fn ml_optimizer_adam_create() {
    let source = r#"
        let opt = optimizer_adam(0.001)
        println(type_of(opt))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["optimizer"]);
}

#[test]
fn ml_optimizer_step_and_zero_grad() {
    let source = r#"
        let w = tensor_set_requires_grad(tensor_ones([2]), true)
        let l = tensor_sum(tensor_mul(w, w))
        tensor_backward(l)
        let opt = optimizer_sgd(0.1, 0.0)
        let w2 = optimizer_step(opt, w)
        println(type_of(w2))
        let w3 = optimizer_zero_grad(w2)
        println(type_of(w3))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["tensor", "tensor"]);
}

// ── Layer builtins ──

#[test]
fn ml_layer_dense_create() {
    let source = r#"
        let dense = layer_dense(4, 3)
        println(type_of(dense))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["layer"]);
}

#[test]
fn ml_layer_forward() {
    let source = r#"
        let dense = layer_dense(3, 2)
        let x = tensor_ones([1, 3])
        let y = layer_forward(dense, x)
        let s = tensor_shape(y)
        println(s)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["[1, 2]"]);
}

#[test]
fn ml_layer_params() {
    let source = r#"
        let dense = layer_dense(4, 3)
        let p = layer_params(dense)
        println(len(p))
    "#;
    let output = eval_output(source);
    // Dense layer has 2 params: weight + bias
    assert_eq!(output, vec!["2"]);
}
