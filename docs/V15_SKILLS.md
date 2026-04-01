# V15 "Delivery" — Implementation Skills & Patterns

> Code recipes for V15 tasks. Reference this before implementing each sprint.
> **Previous:** `docs/V1_SKILLS.md` (Cranelift), `docs/V03_SKILLS.md` (concurrency/GPU)

---

## Skill 1: Registering a New Builtin Function

**Used by:** B2.1-B2.9 (tanh, gelu, leaky_relu, flatten, concat, cross_entropy, accuracy)

### Step 1: Type Checker Registration

File: `src/analyzer/type_check/register.rs`

```rust
// Add to the register_builtins() method:
self.register_builtin("tanh", Type::Function {
    params: vec![Type::Tensor],
    ret: Box::new(Type::Tensor),
});
```

### Step 2: Interpreter Dispatch

File: `src/interpreter/eval/builtins.rs`

```rust
// Add to call_builtin() match:
"tanh" => {
    let t = args.into_iter().next()
        .ok_or(RuntimeError::Unsupported("tanh requires 1 argument".into()))?;
    match t {
        Value::Tensor(tv) => {
            let result = tv.data.mapv(|x| x.tanh());
            Ok(Value::Tensor(TensorValue { data: result, requires_grad: tv.requires_grad }))
        }
        _ => Err(RuntimeError::TypeMismatch("tanh expects tensor".into()).into()),
    }
}
```

### Step 3: Test

```rust
#[test]
fn tanh_builtin_works() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = from_data([[0.0, 1.0]])\ntanh(t)");
    assert!(result.is_ok());
}
```

### Step 4: End-to-End (.fj file)

```fajar
let t = from_data([[0.0, 1.0, -1.0]])
let r = tanh(t)
println(r)
// Expected: tensor values between -1 and 1
```

Run: `fj run test_tanh.fj`

---

## Skill 2: Fixing Method Dispatch on Value Types

**Used by:** B2.4-B2.5 (Dense.forward(), Conv2d.forward())

### Problem

`Dense(4, 2)` creates a `Value::Layer(LayerValue)`, but `.forward(x)` is not dispatched.

### Solution

File: `src/interpreter/eval/methods.rs` (or wherever method calls are resolved)

Find method dispatch for `Value::Layer` and add:

```rust
Value::Layer(layer) => {
    match method_name {
        "forward" => {
            let input = args.into_iter().next()
                .ok_or(RuntimeError::Unsupported("forward requires 1 argument".into()))?;
            self.builtin_layer_forward(layer, input)
        }
        _ => Err(RuntimeError::Unsupported(
            format!("no method '{}' on Layer", method_name)
        ).into()),
    }
}
```

### Verify

```fajar
let layer = Dense(4, 2)
let x = ones(1, 4)
let y = layer.forward(x)
println(shape(y))   // [1, 2]
```

---

## Skill 3: Fixing Effect Multi-Step Continuation

**Used by:** B1.1-B1.3

### Problem

When a handle body performs two effect operations:
```fajar
handle {
    Console::log("first")    // works
    Console::log("second")   // NOT reached — body exits after first resume
} with {
    Console::log(msg) => { println(msg); resume(null) }
}
```

### Root Cause

In `src/interpreter/eval/mod.rs`, the `HandleEffect` expression evaluates the body.
When `ControlFlow::EffectPerformed` is caught, the handler runs and returns the resumed value.
But then the handle expression returns that value instead of continuing the body.

### Solution Pattern

The body needs to be wrapped in a loop/continuation that re-enters after each effect:
1. Save the body's execution state (which statement we're at)
2. On EffectPerformed, run the handler, get resumed value
3. Continue body from where it left off

This is the "one-shot delimited continuation" pattern. Implementation approach:

```rust
// In eval of HandleEffect:
loop {
    match self.eval_block_from(body, resume_point) {
        Ok(val) => return Ok(val),  // body completed normally
        Err(e) if let Some(effect) = e.as_effect_performed() => {
            // Find matching handler
            let handler = find_handler(&handlers, &effect);
            // Run handler body, get resumed value
            let resumed_val = self.eval_handler(handler, effect.args)?;
            // Set resumed value for next iteration
            resume_point = effect.continuation_point;
            last_resumed = resumed_val;
        }
        Err(e) => return Err(e),  // propagate other errors
    }
}
```

### Alternative (Simpler)

If full continuations are too complex, use the "replay" approach:
- Wrap the body in a stateful closure that remembers which effects have been handled
- On each effect, save the effect's result and replay the body from the start
- Skip already-handled effects using the saved results

---

## Skill 4: Adding a CLI Subcommand

**Used by:** B3.3, B3.5, B3.8, B3.9

### Pattern

File: `src/main.rs`

1. Add to clap enum:
```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands
    
    /// Initialize local package registry
    #[command(name = "registry")]
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
}

#[derive(Subcommand)]
enum RegistryAction {
    /// Initialize local registry directory
    Init {
        /// Path for the registry
        path: String,
    },
}
```

2. Add handler function:
```rust
fn cmd_registry_init(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Create directory structure
    // Write initial metadata
    // Print success message
    Ok(())
}
```

3. Wire in match:
```rust
Commands::Registry { action } => match action {
    RegistryAction::Init { path } => cmd_registry_init(&path)?,
},
```

---

## Skill 5: Creating Integration Test (.fj program)

**Used by:** I1.1-I3.10

### Pattern

```fajar
// examples/v15_mnist_train.fj
// V15 Sprint I1: Real MNIST Training

// Step 1: Load data
let train_data = read_file("data/mnist/train-images-idx3-ubyte")
let train_labels = read_file("data/mnist/train-labels-idx1-ubyte")

// Step 2: Define model
let layer1 = Dense(784, 128)
let layer2 = Dense(128, 10)

// Step 3: Training loop
let lr = 0.01
let epochs = 5
for epoch in 0..epochs {
    // forward
    let h = relu(forward(layer1, batch))
    let out = softmax(forward(layer2, h))
    
    // loss
    let loss_val = cross_entropy(out, labels)
    
    // backward
    backward(loss_val)
    
    // update
    // SGD step
    
    println(f"Epoch {epoch}: loss = {loss_val}")
}
```

### Verification

```bash
# Must produce output, no errors
fj run examples/v15_mnist_train.fj

# Expected: loss decreasing per epoch
# Epoch 0: loss = 2.3
# Epoch 1: loss = 1.8
# ...
```

---

## Skill 6: Writing Fuzz Harness

**Used by:** P1.1-P1.9

### Pattern

File: `fuzz/fuzz_targets/lexer_fuzz.rs`

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Must not panic — only return errors
        let _ = fajar_lang::lexer::tokenize(s);
    }
});
```

### Setup

```bash
cargo install cargo-fuzz
cargo fuzz init  # if not already done
cargo fuzz add lexer_fuzz
# Edit fuzz/fuzz_targets/lexer_fuzz.rs
cargo fuzz run lexer_fuzz -- -runs=100000
```

---

## Skill 7: Context Isolation Deepening

**Used by:** B3.2

### Problem

`fj verify` doesn't catch `zeros()` or `relu()` inside @kernel functions.

### Solution

File: `src/verify/pipeline.rs` or `src/analyzer/type_check/check.rs`

Add a tensor builtin blacklist:

```rust
const TENSOR_BUILTINS: &[&str] = &[
    "zeros", "ones", "randn", "from_data", "matmul", "transpose",
    "relu", "sigmoid", "tanh", "softmax", "flatten",
    "Dense", "Conv2d", "backward", "grad", "forward",
    "mse_loss", "cross_entropy", "SGD",
];

// In check_context_call or verify pipeline:
if self.current_context == Context::Kernel {
    if TENSOR_BUILTINS.contains(&fn_name) {
        self.errors.push(SemanticError::KernelError {
            kind: KE002,  // TensorInKernel
            span,
        });
    }
}
```

---

## Skill 8: Writing End-to-End Test Suites

**Used by:** B1.10, B3.10, I3.10

### Pattern

Create a test runner script:

```bash
#!/bin/bash
# tests/v15_effects_e2e.sh
PASS=0
FAIL=0

run_test() {
    echo -n "Testing $1... "
    if cargo run -- run "tests/v15/$1.fj" 2>&1 | grep -q "$2"; then
        echo "PASS"
        ((PASS++))
    else
        echo "FAIL"
        ((FAIL++))
    fi
}

run_test "effect_basic" "Hello from effects"
run_test "effect_resume" "42"
run_test "effect_nested" "inner then outer"
# ... more tests

echo "Results: $PASS passed, $FAIL failed"
exit $FAIL
```

Or as Rust integration tests:

```rust
#[test]
fn v15_effect_basic() {
    let output = Command::new("cargo")
        .args(["run", "--", "run", "tests/v15/effect_basic.fj"])
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Hello from effects"));
}
```

---

*V15 Skills — Version 1.0 | 8 skills | 2026-04-01*
