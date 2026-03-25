// Fajar Lang GPU Compute — Matrix Multiplication (Tiled)
// C[M x N] = A[M x K] * B[K x N]

struct Dimensions {
    M: u32,
    K: u32,
    N: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> dims: Dimensions;
@group(0) @binding(1) var<storage, read> a: array<f32>;
@group(0) @binding(2) var<storage, read> b: array<f32>;
@group(0) @binding(3) var<storage, read_write> c: array<f32>;

const TILE_SIZE: u32 = 16u;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.y;
    let col = gid.x;

    if row >= dims.M || col >= dims.N {
        return;
    }

    var sum: f32 = 0.0;
    for (var k: u32 = 0u; k < dims.K; k = k + 1u) {
        sum = sum + a[row * dims.K + k] * b[k * dims.N + col];
    }

    c[row * dims.N + col] = sum;
}
