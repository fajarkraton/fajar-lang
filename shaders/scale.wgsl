// Fajar Lang GPU Compute — Scalar Multiplication
// output[i] = input[i] * scalar

struct Params {
    scalar: f32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx < arrayLength(&input) {
        output[idx] = input[idx] * params.scalar;
    }
}
