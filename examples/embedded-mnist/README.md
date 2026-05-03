# embedded-mnist — `@device` tensor inference example

Runs a pre-trained 2-layer MLP for MNIST digit classification using
Fajar Lang's `@device` annotation and stack-allocated `[f32; N]` arrays
(no heap, no allocator — suitable for an embedded target with ≤4 KB
working memory once weights are mapped read-only).

## Layout

```
embedded-mnist/
├── fj.toml
├── README.md
└── src/
    ├── main.fj      # entry: load weights + image, call forward, print result
    └── model.fj     # @device fn forward (2-layer MLP) + argmax helper
```

## Architecture

```
input  : [f32; 784]            (28×28 grayscale, [0, 1] normalized)
hidden : [f32; 128]            (ReLU activation)
output : [f32; 10]             (logits; argmax = predicted digit)
```

Total weights: 784·128 + 128 + 128·10 + 10 = 101,770 f32 = ~398 KB.
Activation memory: 128 + 10 + 784 = 922 f32 = ~3.6 KB stack frame.

## Build & run

Train weights externally (PyTorch/TF/etc) and dump to a flat binary
`mnist_weights.bin` ordered `[w1 | b1 | w2 | b2]` little-endian f32.

```bash
cd examples/embedded-mnist
mkdir -p data
# place data/digit_3.bin (784 bytes, raw u8 pixel data)
# place data/mnist_weights.bin (101770 * 4 = 407080 bytes, f32 LE)
fj build
fj run -- data/digit_3.bin data/mnist_weights.bin
```

Expected output (assuming the input is a hand-written "3"):

```
prediction: 3
```

## What it demonstrates

- **`@device` context**: heap allocation forbidden (`KE001`), tensor
  ops allowed (`KE002` only fires in `@kernel`). Raw pointers are
  forbidden (`DE001`).
- **Stack-allocated tensors** via `[f32; N]` const-shape arrays. The
  compiler verifies size at parse-time; mismatches fail before
  inference even runs.
- **Pure forward pass**: no gradients, no autograd state. The
  inference function is a pure mathematical mapping.

## Extending

- **Quantize weights** via FajarQuant — replace `f32` with `Quantized<f32, 4>`
  and call `dequantize()` per tile. See `docs/tutorials/embedded_ml.md`.
- **Cross-context bridge**: wrap `forward()` in an `@safe fn` that
  validates input range, then crosses into `@device` via the syscall ABI.
- **Hardware acceleration**: change `@device` to call into a vendor-
  specific NPU op via `nn::accelerator::*` (see `docs/STDLIB_SPEC.md`).

## Related

- `examples/recipes/image_classifier.fj` — single-file ImageNet-style
  classifier scaffold.
- `docs/tutorials/mnist.md` — full training-to-inference walkthrough.
- `docs/TUTORIAL.md` Chapter 8 — Tensors and ML.
