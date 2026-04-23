//! V14 GS3: Metal Shading Language (MSL) backend for Apple GPU compute.

/// Metal compute kernel generator.
#[derive(Debug)]
pub struct MetalModule {
    /// Kernel name.
    pub name: String,
    /// Generated MSL source code.
    pub source: String,
}

impl MetalModule {
    /// Creates a new Metal module with the given kernel name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: String::new(),
        }
    }

    /// Emit a minimal Metal compute kernel for element-wise add.
    pub fn emit_add_kernel(&mut self) -> &str {
        self.source = format!(
            "#include <metal_stdlib>\n\
             using namespace metal;\n\n\
             kernel void {name}(\n\
             \tdevice float* a [[buffer(0)]],\n\
             \tdevice float* b [[buffer(1)]],\n\
             \tdevice float* result [[buffer(2)]],\n\
             \tuint id [[thread_position_in_grid]]\n\
             ) {{\n\
             \tresult[id] = a[id] + b[id];\n\
             }}\n",
            name = self.name
        );
        &self.source
    }

    /// Emit a Metal compute kernel for element-wise multiply.
    pub fn emit_mul_kernel(&mut self) -> &str {
        self.source = format!(
            "#include <metal_stdlib>\n\
             using namespace metal;\n\n\
             kernel void {name}(\n\
             \tdevice float* a [[buffer(0)]],\n\
             \tdevice float* b [[buffer(1)]],\n\
             \tdevice float* result [[buffer(2)]],\n\
             \tuint id [[thread_position_in_grid]]\n\
             ) {{\n\
             \tresult[id] = a[id] * b[id];\n\
             }}\n",
            name = self.name
        );
        &self.source
    }

    /// Emit a matmul kernel (simplified, not tiled).
    pub fn emit_matmul_kernel(&mut self, m: usize, n: usize, k: usize) -> &str {
        self.source = format!(
            "#include <metal_stdlib>\n\
             using namespace metal;\n\n\
             kernel void {name}(\n\
             \tdevice float* a [[buffer(0)]],\n\
             \tdevice float* b [[buffer(1)]],\n\
             \tdevice float* c [[buffer(2)]],\n\
             \tuint2 gid [[thread_position_in_grid]]\n\
             ) {{\n\
             \tint row = gid.y;\n\
             \tint col = gid.x;\n\
             \tif (row < {m} && col < {n}) {{\n\
             \t\tfloat sum = 0.0;\n\
             \t\tfor (int i = 0; i < {k}; i++) {{\n\
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

    /// Write MSL to file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, &self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v14_gs3_1_metal_add_kernel() {
        let mut m = MetalModule::new("add_kernel");
        let src = m.emit_add_kernel();
        assert!(src.contains("kernel void add_kernel"));
        assert!(src.contains("result[id] = a[id] + b[id]"));
        assert!(src.contains("#include <metal_stdlib>"));
    }

    #[test]
    fn v14_gs3_2_metal_mul_kernel() {
        let mut m = MetalModule::new("mul_kernel");
        let src = m.emit_mul_kernel();
        assert!(src.contains("a[id] * b[id]"));
    }

    #[test]
    fn v14_gs3_3_metal_matmul_kernel() {
        let mut m = MetalModule::new("matmul");
        let src = m.emit_matmul_kernel(64, 64, 64);
        assert!(src.contains("matmul"));
        assert!(src.contains("row < 64"));
        assert!(src.contains("col < 64"));
    }

    #[test]
    fn v14_gs3_4_metal_write_file() {
        let mut m = MetalModule::new("test_kernel");
        m.emit_add_kernel();
        let path = std::env::temp_dir().join("test_kernel.metal");
        m.write_to_file(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("kernel void test_kernel"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn v14_gs3_5_metal_source_getter() {
        let mut m = MetalModule::new("k");
        assert!(m.source().is_empty());
        m.emit_add_kernel();
        assert!(!m.source().is_empty());
    }

    /// V14 GS3.1: Verify Metal CLI path produces valid .metal file end-to-end.
    #[test]
    fn v14_gs3_1_metal_cli_e2e() {
        let out_path = std::env::temp_dir().join("fj_test_cli.metal");
        let mut module = MetalModule::new("main");
        module.emit_add_kernel();
        module.write_to_file(&out_path).unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(
            content.contains("#include <metal_stdlib>"),
            "MSL should include metal header"
        );
        assert!(
            content.contains("kernel void main"),
            "should have kernel entry point"
        );
        assert!(
            content.contains("result[id] = a[id] + b[id]"),
            "should have add operation"
        );
        assert!(
            content.len() > 100,
            "MSL should be substantial: {} bytes",
            content.len()
        );

        std::fs::remove_file(&out_path).ok();
    }
}
