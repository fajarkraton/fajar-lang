// Fajar Lang GPU Compute — ReLU Activation
// output[i] = max(0, input[i])

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx < arrayLength(&input) {
        output[idx] = max(0.0, input[idx]);
    }
}
