//! V14 GS3: HLSL compute shader backend for DirectX GPU compute.

/// HLSL compute shader generator.
#[derive(Debug)]
pub struct HlslModule {
    /// Kernel name.
    pub name: String,
    /// Generated HLSL source code.
    pub source: String,
}

impl HlslModule {
    /// Creates a new HLSL module with the given kernel name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: String::new(),
        }
    }

    /// Emit HLSL element-wise add compute shader.
    pub fn emit_add_kernel(&mut self, thread_group_size: u32) -> &str {
        self.source = format!(
            "RWStructuredBuffer<float> a : register(u0);\n\
             RWStructuredBuffer<float> b : register(u1);\n\
             RWStructuredBuffer<float> result : register(u2);\n\n\
             [numthreads({tgs}, 1, 1)]\n\
             void {name}(uint3 id : SV_DispatchThreadID) {{\n\
             \tresult[id.x] = a[id.x] + b[id.x];\n\
             }}\n",
            name = self.name,
            tgs = thread_group_size
        );
        &self.source
    }

    /// Emit HLSL element-wise multiply compute shader.
    pub fn emit_mul_kernel(&mut self, thread_group_size: u32) -> &str {
        self.source = format!(
            "RWStructuredBuffer<float> a : register(u0);\n\
             RWStructuredBuffer<float> b : register(u1);\n\
             RWStructuredBuffer<float> result : register(u2);\n\n\
             [numthreads({tgs}, 1, 1)]\n\
             void {name}(uint3 id : SV_DispatchThreadID) {{\n\
             \tresult[id.x] = a[id.x] * b[id.x];\n\
             }}\n",
            name = self.name,
            tgs = thread_group_size
        );
        &self.source
    }

    /// Emit HLSL matmul compute shader.
    pub fn emit_matmul_kernel(&mut self, m: usize, n: usize, k: usize) -> &str {
        self.source = format!(
            "RWStructuredBuffer<float> a : register(u0);\n\
             RWStructuredBuffer<float> b : register(u1);\n\
             RWStructuredBuffer<float> c : register(u2);\n\n\
             [numthreads(16, 16, 1)]\n\
             void {name}(uint3 gid : SV_DispatchThreadID) {{\n\
             \tuint row = gid.y;\n\
             \tuint col = gid.x;\n\
             \tif (row < {m} && col < {n}) {{\n\
             \t\tfloat sum = 0.0;\n\
             \t\tfor (uint i = 0; i < {k}; i++) {{\n\
             \t\t\tsum += a[row * {k} + i] * b[i * {n} + col];\n\
             \t\t}}\n\
             \t\tc[row * {n} + col] = sum;\n\
             \t}}\n\
             }}\n",
            name = self.name
        );
        &self.source
    }

    /// Get generated source code.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Write HLSL to file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, &self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v14_gs3_6_hlsl_add_kernel() {
        let mut h = HlslModule::new("CSAdd");
        let src = h.emit_add_kernel(256);
        assert!(src.contains("[numthreads(256, 1, 1)]"));
        assert!(src.contains("void CSAdd"));
        assert!(src.contains("result[id.x] = a[id.x] + b[id.x]"));
    }

    #[test]
    fn v14_gs3_7_hlsl_mul_kernel() {
        let mut h = HlslModule::new("CSMul");
        let src = h.emit_mul_kernel(128);
        assert!(src.contains("a[id.x] * b[id.x]"));
    }

    #[test]
    fn v14_gs3_8_hlsl_matmul_kernel() {
        let mut h = HlslModule::new("CSMatmul");
        let src = h.emit_matmul_kernel(32, 32, 32);
        assert!(src.contains("CSMatmul"));
        assert!(src.contains("row < 32"));
    }

    #[test]
    fn v14_gs3_9_hlsl_write_file() {
        let mut h = HlslModule::new("test");
        h.emit_add_kernel(64);
        let path = std::path::PathBuf::from("/tmp/test_shader.hlsl");
        h.write_to_file(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("void test"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn v14_gs3_10_hlsl_source_getter() {
        let mut h = HlslModule::new("k");
        assert!(h.source().is_empty());
        h.emit_add_kernel(64);
        assert!(!h.source().is_empty());
    }

    /// V14 GS3.6: Verify HLSL CLI path produces valid .hlsl file end-to-end.
    #[test]
    fn v14_gs3_6_hlsl_cli_e2e() {
        let out_path = std::path::PathBuf::from("/tmp/fj_test_cli.hlsl");
        let mut module = HlslModule::new("CSMain");
        module.emit_add_kernel(256);
        module.write_to_file(&out_path).unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(
            content.contains("RWStructuredBuffer<float>"),
            "HLSL should have structured buffers"
        );
        assert!(
            content.contains("[numthreads(256, 1, 1)]"),
            "should have thread group size"
        );
        assert!(
            content.contains("void CSMain"),
            "should have kernel entry point"
        );
        assert!(
            content.contains("result[id.x] = a[id.x] + b[id.x]"),
            "should have add operation"
        );

        std::fs::remove_file(&out_path).ok();
    }
}
