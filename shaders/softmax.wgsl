// Fajar Lang GPU Compute — Softmax
// output[i] = exp(input[i]) / sum(exp(input))
// Two-pass: find max (numerical stability), then compute exp and normalize

struct Params {
    n: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = params.n;

    // Find max for numerical stability
    var max_val: f32 = input[0];
    for (var i: u32 = 1u; i < n; i = i + 1u) {
        max_val = max(max_val, input[i]);
    }

    // Compute exp(x - max) and sum
    var sum: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let e = exp(input[i] - max_val);
        output[i] = e;
        sum = sum + e;
    }

    // Normalize
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        output[i] = output[i] / sum;
    }
}
