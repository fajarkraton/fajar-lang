// Fajar Lang GPU Compute — 2D Convolution
// For neural network convolutional layers

struct ConvParams {
    input_h: u32,
    input_w: u32,
    kernel_h: u32,
    kernel_w: u32,
    output_h: u32,
    output_w: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<uniform> params: ConvParams;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read> kernel: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let out_y = gid.y;
    let out_x = gid.x;

    if out_y >= params.output_h || out_x >= params.output_w {
        return;
    }

    var sum: f32 = 0.0;
    for (var ky: u32 = 0u; ky < params.kernel_h; ky = ky + 1u) {
        for (var kx: u32 = 0u; kx < params.kernel_w; kx = kx + 1u) {
            let iy = out_y + ky;
            let ix = out_x + kx;
            sum = sum + input[iy * params.input_w + ix] * kernel[ky * params.kernel_w + kx];
        }
    }

    output[out_y * params.output_w + out_x] = sum;
}
