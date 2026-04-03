//! Native GPU Codegen — PTX/SPIR-V/Metal/HLSL backend, kernel fusion,
//! GPU memory management, and AST-driven GPU IR lowering.

pub mod fusion;
pub mod gpu_memory;
pub mod hlsl;
pub mod metal;
pub mod ptx;
pub mod spirv;

use crate::parser::ast::{Annotation, Expr, Item, Program, Stmt};

// ═══════════════════════════════════════════════════════════════════════
// GPU Intermediate Representation (GpuIr)
// ═══════════════════════════════════════════════════════════════════════

/// A complete GPU IR program containing one or more kernels.
#[derive(Debug, Clone)]
pub struct GpuIr {
    /// Kernel functions lowered from `@gpu fn`.
    pub kernels: Vec<GpuKernel>,
}

/// A single GPU compute kernel.
#[derive(Debug, Clone)]
pub struct GpuKernel {
    /// Kernel name.
    pub name: String,
    /// Buffer parameters (each is a `buffer<f32>` binding).
    pub buffers: Vec<String>,
    /// Shared memory declarations: (name, element_count).
    pub shared_memory: Vec<(String, usize)>,
    /// Workgroup size (x, y, z).
    pub workgroup_size: (u32, u32, u32),
    /// Kernel body statements.
    pub body: Vec<GpuStmt>,
}

/// A GPU statement.
#[derive(Debug, Clone)]
pub enum GpuStmt {
    /// Store result of expression into buffer at thread index:
    /// `buffers[target][gid] = expr`
    Store {
        /// Target buffer name.
        target: String,
        /// Expression to compute.
        value: GpuExpr,
    },
    /// Return / end kernel.
    Return,
}

/// A GPU expression.
#[derive(Debug, Clone)]
pub enum GpuExpr {
    /// Load from buffer at thread index: `buf[gid]`
    BufferLoad(String),
    /// Float literal.
    FloatLit(f64),
    /// Integer literal.
    IntLit(i64),
    /// Binary operation.
    BinOp {
        /// Left operand.
        left: Box<GpuExpr>,
        /// Operator.
        op: GpuBinOp,
        /// Right operand.
        right: Box<GpuExpr>,
    },
}

/// Binary operations supported in GPU kernels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBinOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
}

/// Error during GPU IR lowering.
#[derive(Debug, Clone)]
pub struct GpuLowerError {
    /// Error message.
    pub message: String,
}

impl std::fmt::Display for GpuLowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GPU lowering error: {}", self.message)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AST → GpuIr lowering
// ═══════════════════════════════════════════════════════════════════════

/// Lower a Fajar Lang program to GPU IR.
///
/// Finds all `@gpu fn` annotated functions and converts them to `GpuKernel`s.
/// Function parameters become buffer bindings, and the body is lowered to
/// element-wise operations on those buffers.
pub fn lower_to_gpu_ir(program: &Program) -> Result<GpuIr, GpuLowerError> {
    let mut kernels = Vec::new();

    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            let is_gpu =
                matches!(&fndef.annotation, Some(Annotation { name, .. }) if name == "gpu");
            if is_gpu {
                let buffers: Vec<String> = fndef.params.iter().map(|p| p.name.clone()).collect();
                let body = lower_expr_to_gpu_stmts(&fndef.body, &buffers)?;
                // Parse workgroup size from annotation params: @gpu(workgroup=256)
                let workgroup_x = fndef
                    .annotation
                    .as_ref()
                    .and_then(|a| {
                        a.params
                            .iter()
                            .find(|p| p.starts_with("workgroup="))
                            .and_then(|p| p.strip_prefix("workgroup="))
                            .and_then(|v| v.parse::<u32>().ok())
                    })
                    .unwrap_or(64);
                kernels.push(GpuKernel {
                    name: fndef.name.clone(),
                    buffers,
                    shared_memory: Vec::new(),
                    workgroup_size: (workgroup_x, 1, 1),
                    body,
                });
            }
        }
    }

    if kernels.is_empty() {
        return Err(GpuLowerError {
            message: "no @gpu functions found in source".into(),
        });
    }

    Ok(GpuIr { kernels })
}

/// Lower a block expression to GPU statements.
fn lower_expr_to_gpu_stmts(expr: &Expr, buffers: &[String]) -> Result<Vec<GpuStmt>, GpuLowerError> {
    let mut stmts = Vec::new();

    match expr {
        Expr::Block {
            stmts: block_stmts,
            expr: tail,
            ..
        } => {
            for stmt in block_stmts {
                match stmt {
                    Stmt::Let { name, value, .. } => {
                        // `let result = a + b` → Store { target: "result", value: lower(a + b) }
                        let gpu_expr = lower_expr(value, buffers)?;
                        stmts.push(GpuStmt::Store {
                            target: name.clone(),
                            value: gpu_expr,
                        });
                    }
                    Stmt::Expr { expr: e, .. } => {
                        // If the expression is an assignment-like pattern, try to lower it.
                        if let Some(gpu_stmt) = try_lower_assign(e, buffers)? {
                            stmts.push(gpu_stmt);
                        }
                    }
                    _ => {}
                }
            }
            // Tail expression: if it references a buffer, store to first unused buffer.
            if let Some(tail_expr) = tail {
                let gpu_expr = lower_expr(tail_expr, buffers)?;
                // Convention: last buffer is the output.
                if let Some(out) = buffers.last() {
                    stmts.push(GpuStmt::Store {
                        target: out.clone(),
                        value: gpu_expr,
                    });
                }
            }
        }
        _ => {
            // Single expression body → store to last buffer.
            let gpu_expr = lower_expr(expr, buffers)?;
            if let Some(out) = buffers.last() {
                stmts.push(GpuStmt::Store {
                    target: out.clone(),
                    value: gpu_expr,
                });
            }
        }
    }

    stmts.push(GpuStmt::Return);
    Ok(stmts)
}

/// Try to lower an expression as an assignment statement.
fn try_lower_assign(expr: &Expr, buffers: &[String]) -> Result<Option<GpuStmt>, GpuLowerError> {
    // Look for patterns like `result[i] = a[i] + b[i]` (simplified to buffer-level)
    if let Expr::Assign { target, value, .. } = expr {
        if let Expr::Ident { name, .. } = target.as_ref() {
            let gpu_expr = lower_expr(value, buffers)?;
            return Ok(Some(GpuStmt::Store {
                target: name.clone(),
                value: gpu_expr,
            }));
        }
    }
    Ok(None)
}

/// Lower a single expression to a GpuExpr.
#[allow(clippy::only_used_in_recursion)]
fn lower_expr(expr: &Expr, buffers: &[String]) -> Result<GpuExpr, GpuLowerError> {
    match expr {
        Expr::Ident { name, .. } => Ok(GpuExpr::BufferLoad(name.clone())),
        Expr::Literal { kind, .. } => match kind {
            crate::parser::ast::LiteralKind::Int(v) => Ok(GpuExpr::IntLit(*v)),
            crate::parser::ast::LiteralKind::Float(v) => Ok(GpuExpr::FloatLit(*v)),
            _ => Err(GpuLowerError {
                message: "unsupported literal type in @gpu kernel".into(),
            }),
        },
        Expr::Binary {
            left, op, right, ..
        } => {
            let gpu_op = match op {
                crate::parser::ast::BinOp::Add => GpuBinOp::Add,
                crate::parser::ast::BinOp::Sub => GpuBinOp::Sub,
                crate::parser::ast::BinOp::Mul => GpuBinOp::Mul,
                crate::parser::ast::BinOp::Div => GpuBinOp::Div,
                _ => {
                    return Err(GpuLowerError {
                        message: format!("unsupported GPU binary operator: {op:?}"),
                    });
                }
            };
            let l = lower_expr(left, buffers)?;
            let r = lower_expr(right, buffers)?;
            Ok(GpuExpr::BinOp {
                left: Box::new(l),
                op: gpu_op,
                right: Box::new(r),
            })
        }
        Expr::Block { expr: Some(e), .. } => lower_expr(e, buffers),
        _ => Err(GpuLowerError {
            message: format!("unsupported expression in @gpu kernel: {expr:?}"),
        }),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GpuIr → Backend code generation
// ═══════════════════════════════════════════════════════════════════════

impl GpuKernel {
    /// Generate SPIR-V binary from this kernel.
    pub fn to_spirv(&self) -> Vec<u8> {
        let mut module = spirv::SpirVModule::new_compute();
        // Use the existing elementwise add shader as a base, parametrized by kernel.
        // For now, all kernels generate an element-wise operation on buffer 0.
        module.emit_elementwise_add_shader(&self.name)
    }

    /// Generate PTX assembly from this kernel.
    pub fn to_ptx(&self) -> String {
        let mut lines = Vec::new();
        lines.push(".version 7.5".into());
        lines.push(".target sm_80".into());
        lines.push(".address_size 64".into());
        lines.push(String::new());
        lines.push(format!(".visible .entry {}(", self.name));
        for (i, buf) in self.buffers.iter().enumerate() {
            let comma = if i + 1 < self.buffers.len() { "," } else { "" };
            lines.push(format!(
                "\t.param .u64 .ptr .global .align 4 param_{buf}{comma}"
            ));
        }
        lines.push(")".into());
        lines.push("{{".into());
        // Registers
        lines.push("\t.reg .u32 %tid;".into());
        lines.push("\t.reg .u64 %addr;".into());
        lines.push(format!("\t.reg .f32 %f<{}>;", self.buffers.len() + 4));
        lines.push(String::new());
        lines.push("\tmov.u32 %tid, %ctaid.x;".into());
        // Load params
        for (i, buf) in self.buffers.iter().enumerate() {
            lines.push(format!("\tld.param.u64 %addr, [param_{buf}];"));
            if i < self.buffers.len() - 1 {
                lines.push(format!("\tld.global.f32 %f{i}, [%addr];"));
            }
        }
        // Body operations
        for stmt in &self.body {
            match stmt {
                GpuStmt::Store { value, .. } => {
                    let op_str = gpu_expr_to_ptx(value, self.buffers.len());
                    lines.push(format!("\t{op_str}"));
                    // Store result
                    let out_idx = self.buffers.len() - 1;
                    lines.push(format!(
                        "\tld.param.u64 %addr, [param_{}];",
                        self.buffers[out_idx]
                    ));
                    lines.push(format!(
                        "\tst.global.f32 [%addr], %f{};",
                        self.buffers.len()
                    ));
                }
                GpuStmt::Return => {
                    lines.push("\tret;".into());
                }
            }
        }
        lines.push("}}".into());
        lines.join("\n")
    }

    /// Generate Metal Shading Language from this kernel.
    pub fn to_metal(&self) -> String {
        let mut lines = Vec::new();
        lines.push("#include <metal_stdlib>".into());
        lines.push("using namespace metal;".into());
        lines.push(String::new());
        // Shared memory (threadgroup)
        for (name, count) in &self.shared_memory {
            lines.push(format!("threadgroup float {name}[{count}];"));
        }
        if !self.shared_memory.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("kernel void {}(", self.name));
        for (i, buf) in self.buffers.iter().enumerate() {
            let comma = if i + 1 < self.buffers.len() { "," } else { "" };
            lines.push(format!("\tdevice float* {buf} [[buffer({i})]]{comma}"));
        }
        lines.push("\tuint id [[thread_position_in_grid]]".into());
        lines.push(") {".into());
        // Body
        for stmt in &self.body {
            match stmt {
                GpuStmt::Store { target, value } => {
                    let expr_str = gpu_expr_to_metal(value);
                    lines.push(format!("\t{target}[id] = {expr_str};"));
                }
                GpuStmt::Return => {}
            }
        }
        lines.push("}".into());
        lines.join("\n")
    }

    /// Generate HLSL compute shader from this kernel.
    pub fn to_hlsl(&self, thread_group_size: u32) -> String {
        let mut lines = Vec::new();
        for (i, buf) in self.buffers.iter().enumerate() {
            lines.push(format!("RWStructuredBuffer<float> {buf} : register(u{i});"));
        }
        // Shared memory (groupshared)
        for (name, count) in &self.shared_memory {
            lines.push(format!("groupshared float {name}[{count}];"));
        }
        lines.push(String::new());
        // Use kernel's workgroup size if explicitly set (non-default), else use caller's.
        let tgs = if self.workgroup_size != (64, 1, 1) {
            self.workgroup_size.0
        } else {
            thread_group_size
        };
        lines.push(format!("[numthreads({tgs}, 1, 1)]"));
        lines.push(format!(
            "void {}(uint3 id : SV_DispatchThreadID) {{",
            self.name
        ));
        for stmt in &self.body {
            match stmt {
                GpuStmt::Store { target, value } => {
                    let expr_str = gpu_expr_to_hlsl(value);
                    lines.push(format!("\t{target}[id.x] = {expr_str};"));
                }
                GpuStmt::Return => {}
            }
        }
        lines.push("}".into());
        lines.join("\n")
    }
}

/// Convert GpuExpr to Metal expression string.
fn gpu_expr_to_metal(expr: &GpuExpr) -> String {
    match expr {
        GpuExpr::BufferLoad(name) => format!("{name}[id]"),
        GpuExpr::FloatLit(v) => format!("{v}"),
        GpuExpr::IntLit(v) => format!("{v}"),
        GpuExpr::BinOp { left, op, right } => {
            let l = gpu_expr_to_metal(left);
            let r = gpu_expr_to_metal(right);
            let op_str = match op {
                GpuBinOp::Add => "+",
                GpuBinOp::Sub => "-",
                GpuBinOp::Mul => "*",
                GpuBinOp::Div => "/",
            };
            format!("({l} {op_str} {r})")
        }
    }
}

/// Convert GpuExpr to HLSL expression string.
fn gpu_expr_to_hlsl(expr: &GpuExpr) -> String {
    match expr {
        GpuExpr::BufferLoad(name) => format!("{name}[id.x]"),
        GpuExpr::FloatLit(v) => format!("{v}"),
        GpuExpr::IntLit(v) => format!("{v}"),
        GpuExpr::BinOp { left, op, right } => {
            let l = gpu_expr_to_hlsl(left);
            let r = gpu_expr_to_hlsl(right);
            let op_str = match op {
                GpuBinOp::Add => "+",
                GpuBinOp::Sub => "-",
                GpuBinOp::Mul => "*",
                GpuBinOp::Div => "/",
            };
            format!("({l} {op_str} {r})")
        }
    }
}

/// Convert GpuExpr to PTX instruction string.
fn gpu_expr_to_ptx(expr: &GpuExpr, buf_count: usize) -> String {
    match expr {
        GpuExpr::BinOp { op, .. } => {
            let ptx_op = match op {
                GpuBinOp::Add => "add.f32",
                GpuBinOp::Sub => "sub.f32",
                GpuBinOp::Mul => "mul.f32",
                GpuBinOp::Div => "div.approx.f32",
            };
            format!("{ptx_op} %f{buf_count}, %f0, %f1;")
        }
        _ => format!("mov.f32 %f{buf_count}, %f0;"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v14_gpu_ir_lower_add_kernel() {
        let source = r#"
            @gpu fn add_kernel(a: f32, b: f32, result: f32) {
                let result = a + b
            }
            fn main() { }
        "#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let ir = lower_to_gpu_ir(&program).unwrap();
        assert_eq!(ir.kernels.len(), 1);
        assert_eq!(ir.kernels[0].name, "add_kernel");
        assert_eq!(ir.kernels[0].buffers.len(), 3);
    }

    #[test]
    fn v14_gpu_ir_to_metal() {
        let source = r#"
            @gpu fn add(a: f32, b: f32, out: f32) {
                let out = a + b
            }
            fn main() { }
        "#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let ir = lower_to_gpu_ir(&program).unwrap();
        let metal = ir.kernels[0].to_metal();
        assert!(metal.contains("kernel void add("));
        assert!(metal.contains("a[id]"));
        assert!(metal.contains("+"));
    }

    #[test]
    fn v14_gpu_ir_to_hlsl() {
        let source = r#"
            @gpu fn mul(a: f32, b: f32, out: f32) {
                let out = a * b
            }
            fn main() { }
        "#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let ir = lower_to_gpu_ir(&program).unwrap();
        let hlsl = ir.kernels[0].to_hlsl(256);
        assert!(hlsl.contains("[numthreads(256, 1, 1)]"));
        assert!(hlsl.contains("void mul("));
        assert!(hlsl.contains("*"));
    }

    #[test]
    fn v14_gpu_ir_to_ptx() {
        let source = r#"
            @gpu fn sub(a: f32, b: f32, out: f32) {
                let out = a - b
            }
            fn main() { }
        "#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let ir = lower_to_gpu_ir(&program).unwrap();
        let ptx = ir.kernels[0].to_ptx();
        assert!(ptx.contains(".entry sub("));
        assert!(ptx.contains("sub.f32"));
    }

    #[test]
    fn v14_gpu_ir_to_spirv() {
        let source = r#"
            @gpu fn compute(a: f32, b: f32, out: f32) {
                let out = a + b
            }
            fn main() { }
        "#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let ir = lower_to_gpu_ir(&program).unwrap();
        let spirv = ir.kernels[0].to_spirv();
        // SPIR-V magic number
        assert_eq!(spirv[0], 0x03);
        assert_eq!(spirv[1], 0x02);
        assert_eq!(spirv[2], 0x23);
        assert_eq!(spirv[3], 0x07);
    }

    #[test]
    fn v14_gpu_ir_no_gpu_fn_returns_error() {
        let source = r#"
            fn main() { }
        "#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let result = lower_to_gpu_ir(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no @gpu functions"));
    }
}
