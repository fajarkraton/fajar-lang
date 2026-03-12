# Skills — Fajar Lang v0.6 "Horizon"

> Implementation patterns and technical recipes for v0.6 features.
> Read this BEFORE implementing complex tasks.
> Reference: `V06_PLAN.md`, `V06_WORKFLOW.md`
> Created: 2026-03-11

---

## 1. LLVM Backend Patterns (Phase 1)

### 1.1 inkwell Context Setup

```rust
// src/codegen/llvm/mod.rs
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::targets::{
    InitializationConfig, Target, TargetMachine, RelocMode, CodeModel, FileType,
};
use inkwell::OptimizationLevel;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, FloatValue, PointerValue};
use inkwell::types::{BasicTypeEnum, FunctionType};

pub struct LlvmCompiler<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    target_machine: Option<TargetMachine>,
    /// Maps function name → LLVM function value
    functions: HashMap<String, FunctionValue<'ctx>>,
    /// Maps variable name → alloca pointer
    variables: HashMap<String, PointerValue<'ctx>>,
    /// Maps variable name → type info for codegen
    var_types: HashMap<String, FjType>,
}

impl<'ctx> LlvmCompiler<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            target_machine: None,
            functions: HashMap::new(),
            variables: HashMap::new(),
            var_types: HashMap::new(),
        }
    }
}
```

### 1.2 Type Mapping (Fajar → LLVM)

```rust
// src/codegen/llvm/types.rs
use inkwell::context::Context;
use inkwell::types::BasicTypeEnum;

pub fn fj_type_to_llvm<'ctx>(ctx: &'ctx Context, ty: &FjType) -> BasicTypeEnum<'ctx> {
    match ty {
        FjType::Bool      => ctx.bool_type().into(),
        FjType::I8        => ctx.i8_type().into(),
        FjType::I16       => ctx.i16_type().into(),
        FjType::I32       => ctx.i32_type().into(),
        FjType::I64       => ctx.i64_type().into(),
        FjType::I128      => ctx.i128_type().into(),
        FjType::F32       => ctx.f32_type().into(),
        FjType::F64       => ctx.f64_type().into(),
        FjType::Str       => {
            // String: { ptr: *i8, len: i64 }
            ctx.struct_type(&[
                ctx.ptr_type(inkwell::AddressSpace::default()).into(),
                ctx.i64_type().into(),
            ], false).into()
        }
        FjType::Void      => ctx.i64_type().into(), // void → i64(0) sentinel
        FjType::Array(_)  => ctx.ptr_type(inkwell::AddressSpace::default()).into(),
        FjType::Struct(_) => ctx.ptr_type(inkwell::AddressSpace::default()).into(),
        _ => ctx.i64_type().into(), // default opaque pointer
    }
}
```

### 1.3 Expression Compilation

```rust
// src/codegen/llvm/compile/expr.rs
impl<'ctx> LlvmCompiler<'ctx> {
    fn compile_expr(&mut self, expr: &Expr) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match expr {
            Expr::IntLit(v) => {
                Ok(self.context.i64_type().const_int(*v as u64, true).into())
            }
            Expr::FloatLit(v) => {
                Ok(self.context.f64_type().const_float(*v).into())
            }
            Expr::BoolLit(v) => {
                Ok(self.context.bool_type().const_int(*v as u64, false).into())
            }
            Expr::BinOp { op, left, right } => {
                let lhs = self.compile_expr(left)?;
                let rhs = self.compile_expr(right)?;
                self.compile_binop(op, lhs, rhs)
            }
            Expr::Ident(name) => {
                let ptr = self.variables.get(name)
                    .ok_or(CodegenError::UndefinedVariable(name.clone()))?;
                Ok(self.builder.build_load(self.context.i64_type(), *ptr, name)
                    .map_err(|e| CodegenError::LlvmError(e.to_string()))?)
            }
            // ... more expression types
            _ => Err(CodegenError::UnsupportedExpr(format!("{:?}", expr)))
        }
    }

    fn compile_binop(
        &self,
        op: &BinOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let lhs_int = lhs.into_int_value();
        let rhs_int = rhs.into_int_value();
        let result = match op {
            BinOp::Add => self.builder.build_int_add(lhs_int, rhs_int, "add"),
            BinOp::Sub => self.builder.build_int_sub(lhs_int, rhs_int, "sub"),
            BinOp::Mul => self.builder.build_int_mul(lhs_int, rhs_int, "mul"),
            BinOp::Div => self.builder.build_int_signed_div(lhs_int, rhs_int, "div"),
            BinOp::Mod => self.builder.build_int_signed_rem(lhs_int, rhs_int, "rem"),
            // Comparison → i1 result
            BinOp::Eq  => self.builder.build_int_compare(
                inkwell::IntPredicate::EQ, lhs_int, rhs_int, "eq"
            ),
            BinOp::Lt  => self.builder.build_int_compare(
                inkwell::IntPredicate::SLT, lhs_int, rhs_int, "lt"
            ),
            // ... more ops
            _ => return Err(CodegenError::UnsupportedOp(format!("{:?}", op)))
        };
        Ok(result.map_err(|e| CodegenError::LlvmError(e.to_string()))?.into())
    }
}
```

### 1.4 Control Flow (If/Else with Phi Nodes)

```rust
fn compile_if(
    &mut self,
    cond: &Expr,
    then_body: &[Stmt],
    else_body: &Option<Vec<Stmt>>,
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    let cond_val = self.compile_expr(cond)?.into_int_value();
    let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();

    let then_bb = self.context.append_basic_block(function, "then");
    let else_bb = self.context.append_basic_block(function, "else");
    let merge_bb = self.context.append_basic_block(function, "merge");

    self.builder.build_conditional_branch(cond_val, then_bb, else_bb)
        .map_err(|e| CodegenError::LlvmError(e.to_string()))?;

    // Then block
    self.builder.position_at_end(then_bb);
    let then_val = self.compile_block(then_body)?;
    let then_exit_bb = self.builder.get_insert_block().unwrap();
    self.builder.build_unconditional_branch(merge_bb)
        .map_err(|e| CodegenError::LlvmError(e.to_string()))?;

    // Else block
    self.builder.position_at_end(else_bb);
    let else_val = if let Some(body) = else_body {
        self.compile_block(body)?
    } else {
        self.context.i64_type().const_int(0, false).into()
    };
    let else_exit_bb = self.builder.get_insert_block().unwrap();
    self.builder.build_unconditional_branch(merge_bb)
        .map_err(|e| CodegenError::LlvmError(e.to_string()))?;

    // Merge with phi
    self.builder.position_at_end(merge_bb);
    let phi = self.builder.build_phi(self.context.i64_type(), "if_result")
        .map_err(|e| CodegenError::LlvmError(e.to_string()))?;
    phi.add_incoming(&[(&then_val, then_exit_bb), (&else_val, else_exit_bb)]);
    Ok(phi.as_basic_value())
}
```

### 1.5 Optimization Passes (New Pass Manager)

```rust
// src/codegen/llvm/optimize.rs
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;

pub fn optimize_module(
    module: &Module,
    target_machine: &TargetMachine,
    opt_level: OptLevel,
) -> Result<(), String> {
    let pass_string = match opt_level {
        OptLevel::O0 => "default<O0>",
        OptLevel::O1 => "default<O1>",
        OptLevel::O2 => "default<O2>",
        OptLevel::O3 => "default<O3>",
        OptLevel::Os => "default<Os>",
        OptLevel::Oz => "default<Oz>",
    };

    let opts = PassBuilderOptions::create();
    // Enable key passes explicitly if needed:
    // opts.set_loop_unrolling(true);
    // opts.set_loop_vectorization(true);

    module.run_passes(pass_string, target_machine, opts)
        .map_err(|e| format!("LLVM pass error: {:?}", e))
}
```

### 1.6 JIT Execution

```rust
// JIT: Execute immediately via ExecutionEngine
use inkwell::execution_engine::JitFunction;

impl<'ctx> LlvmCompiler<'ctx> {
    pub fn jit_execute(&self) -> Result<i64, CodegenError> {
        let ee = self.module.create_jit_execution_engine(OptimizationLevel::Default)
            .map_err(|e| CodegenError::LlvmError(e.to_string()))?;

        // Register runtime functions
        ee.add_global_mapping(
            &self.functions["fj_rt_print_int"],
            fj_rt_print_int as usize,
        );

        // Get and call main
        unsafe {
            let main_fn: JitFunction<unsafe extern "C" fn() -> i64> =
                ee.get_function("main")
                    .map_err(|e| CodegenError::LlvmError(e.to_string()))?;
            Ok(main_fn.call())
        }
    }
}
```

### 1.7 AOT Compilation

```rust
// AOT: Write object file
pub fn emit_object(
    module: &Module,
    target_machine: &TargetMachine,
    output_path: &Path,
) -> Result<(), CodegenError> {
    target_machine.write_to_file(module, FileType::Object, output_path)
        .map_err(|e| CodegenError::LlvmError(e.to_string()))
}

pub fn emit_assembly(
    module: &Module,
    target_machine: &TargetMachine,
    output_path: &Path,
) -> Result<(), CodegenError> {
    target_machine.write_to_file(module, FileType::Assembly, output_path)
        .map_err(|e| CodegenError::LlvmError(e.to_string()))
}
```

### 1.8 CodegenBackend Trait

```rust
// src/codegen/mod.rs — Shared trait for Cranelift and LLVM backends
pub trait CodegenBackend {
    fn compile_program(&mut self, program: &Program) -> Result<(), CodegenError>;
    fn jit_execute(&self) -> Result<i64, CodegenError>;
    fn emit_object(&self, path: &Path) -> Result<(), CodegenError>;
    fn emit_assembly(&self, path: &Path) -> Result<(), CodegenError>;
    fn emit_ir(&self) -> String;
}
```

---

## 2. Debugger / DAP Patterns (Phase 2)

### 2.1 Debug State Architecture

```rust
// src/debugger/mod.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};

pub mod dap_server;

#[derive(Clone, Debug)]
pub struct Breakpoint {
    pub id: i64,
    pub file: String,
    pub line: usize,
    pub condition: Option<String>,
    pub hit_count: usize,
    pub log_message: Option<String>,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StepMode {
    Continue,
    StepIn,
    StepOver { call_depth: usize },
    StepOut  { target_depth: usize },
    Paused,
}

pub struct DebugState {
    pub breakpoints: HashMap<(String, usize), Breakpoint>,
    pub step_mode: StepMode,
    pub call_depth: usize,
    pub current_file: String,
    pub current_line: usize,
    pub next_breakpoint_id: i64,
}

impl DebugState {
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            step_mode: StepMode::Continue,
            call_depth: 0,
            current_file: String::new(),
            current_line: 0,
            next_breakpoint_id: 1,
        }
    }

    /// Returns true if execution should pause at this location
    pub fn should_stop(&mut self, file: &str, line: usize) -> bool {
        self.current_file = file.to_string();
        self.current_line = line;

        // Check breakpoints
        if let Some(bp) = self.breakpoints.get_mut(&(file.to_string(), line)) {
            bp.hit_count += 1;
            return true;
        }

        // Check step mode
        match &self.step_mode {
            StepMode::Continue => false,
            StepMode::StepIn => true,
            StepMode::StepOver { call_depth } => self.call_depth <= *call_depth,
            StepMode::StepOut { target_depth } => self.call_depth < *target_depth,
            StepMode::Paused => true,
        }
    }
}
```

### 2.2 Interpreter Debug Hook

```rust
// Pattern: Insert debug_hook calls in eval_stmt/eval_expr
// src/interpreter/eval.rs

impl Interpreter {
    fn eval_stmt_with_debug(&mut self, stmt: &Stmt) -> Result<Value, RuntimeError> {
        // Debug hook: check before every statement
        if let Some(debug) = &self.debug_state {
            let line = stmt.span().start_line;
            let file = &self.current_file;
            let mut state = debug.lock().map_err(|_| RuntimeError::InternalError)?;
            if state.should_stop(file, line) {
                state.step_mode = StepMode::Paused;
                // Signal debugger that we've stopped
                if let Some(tx) = &self.debug_event_tx {
                    tx.send(DebugEvent::Stopped {
                        reason: StopReason::Breakpoint,
                        line,
                        file: file.clone(),
                    }).ok();
                }
                // Wait for debugger command
                if let Some(rx) = &self.debug_cmd_rx {
                    match rx.recv() {
                        Ok(DebugCommand::Continue) => state.step_mode = StepMode::Continue,
                        Ok(DebugCommand::StepIn) => state.step_mode = StepMode::StepIn,
                        Ok(DebugCommand::StepOver) => {
                            state.step_mode = StepMode::StepOver {
                                call_depth: state.call_depth,
                            };
                        }
                        Ok(DebugCommand::StepOut) => {
                            state.step_mode = StepMode::StepOut {
                                target_depth: state.call_depth,
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
        self.eval_stmt(stmt)
    }
}
```

### 2.3 DAP Server Pattern

```rust
// src/debugger/dap_server.rs
use dap::prelude::*;
use std::io::{BufRead, BufReader, Write};

pub struct FajarDebugServer {
    server: Server<BufReader<std::io::Stdin>, std::io::Stdout>,
    debug_state: Arc<Mutex<DebugState>>,
    cmd_tx: mpsc::Sender<DebugCommand>,
    event_rx: mpsc::Receiver<DebugEvent>,
}

impl FajarDebugServer {
    pub fn new(
        debug_state: Arc<Mutex<DebugState>>,
        cmd_tx: mpsc::Sender<DebugCommand>,
        event_rx: mpsc::Receiver<DebugEvent>,
    ) -> Self {
        let reader = BufReader::new(std::io::stdin());
        let writer = std::io::stdout();
        let server = Server::new(reader, writer);
        Self { server, debug_state, cmd_tx, event_rx }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match self.server.poll_request()? {
                Some(req) => {
                    match &req.command {
                        Command::Initialize(args) => {
                            self.handle_initialize(&req, args)?;
                        }
                        Command::Launch(args) => {
                            self.handle_launch(&req, args)?;
                        }
                        Command::SetBreakpoints(args) => {
                            self.handle_set_breakpoints(&req, args)?;
                        }
                        Command::Continue(_) => {
                            self.cmd_tx.send(DebugCommand::Continue)?;
                            let resp = req.success(ResponseBody::Continue(
                                ContinueResponse { all_threads_continued: Some(true) }
                            ));
                            self.server.respond(resp)?;
                        }
                        Command::Next(_) => {
                            self.cmd_tx.send(DebugCommand::StepOver)?;
                            self.server.respond(req.success(ResponseBody::Next))?;
                        }
                        Command::StepIn(_) => {
                            self.cmd_tx.send(DebugCommand::StepIn)?;
                            self.server.respond(req.success(ResponseBody::StepIn))?;
                        }
                        Command::StepOut(_) => {
                            self.cmd_tx.send(DebugCommand::StepOut)?;
                            self.server.respond(req.success(ResponseBody::StepOut))?;
                        }
                        Command::Threads => {
                            self.handle_threads(&req)?;
                        }
                        Command::StackTrace(args) => {
                            self.handle_stack_trace(&req, args)?;
                        }
                        Command::Scopes(args) => {
                            self.handle_scopes(&req, args)?;
                        }
                        Command::Variables(args) => {
                            self.handle_variables(&req, args)?;
                        }
                        Command::Evaluate(args) => {
                            self.handle_evaluate(&req, args)?;
                        }
                        Command::Disconnect(_) => break,
                        _ => {
                            self.server.respond(req.error("Unsupported command"))?;
                        }
                    }
                }
                None => break,
            }

            // Check for events from interpreter thread
            while let Ok(event) = self.event_rx.try_recv() {
                self.send_debug_event(event)?;
            }
        }
        Ok(())
    }
}
```

### 2.4 DWARF Debug Info Generation

```rust
// src/codegen/dwarf.rs
use gimli::write::{
    DwarfUnit, UnitEntryId, AttributeValue, LineProgram, LineString,
    Address, EndianVec, Sections,
};
use gimli::{Encoding, Format, RunTimeEndian, LittleEndian};

pub struct DwarfGenerator {
    dwarf: DwarfUnit,
    root_id: UnitEntryId,
    /// Source line → instruction offset mapping
    source_map: Vec<(usize, u64)>, // (line_number, instruction_offset)
}

impl DwarfGenerator {
    pub fn new(file_name: &str) -> Self {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 5,
            address_size: 8,
        };

        let mut dwarf = DwarfUnit::new(encoding);
        let root_id = dwarf.unit.root();

        // Set compile unit attributes
        let root = dwarf.unit.get_mut(root_id);
        root.set(
            gimli::DW_AT_producer,
            AttributeValue::String(b"Fajar Lang v0.6".to_vec()),
        );
        root.set(
            gimli::DW_AT_name,
            AttributeValue::String(file_name.as_bytes().to_vec()),
        );

        Self {
            dwarf,
            root_id,
            source_map: Vec::new(),
        }
    }

    pub fn add_function(
        &mut self,
        name: &str,
        low_pc: u64,
        high_pc: u64,
        params: &[(String, FjType)],
    ) {
        let fn_id = self.dwarf.unit.add(self.root_id, gimli::DW_TAG_subprogram);
        let fn_entry = self.dwarf.unit.get_mut(fn_id);
        fn_entry.set(
            gimli::DW_AT_name,
            AttributeValue::String(name.as_bytes().to_vec()),
        );
        fn_entry.set(
            gimli::DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(low_pc)),
        );
        fn_entry.set(
            gimli::DW_AT_high_pc,
            AttributeValue::Udata(high_pc - low_pc),
        );

        // Add parameters
        for (pname, _ptype) in params {
            let param_id = self.dwarf.unit.add(fn_id, gimli::DW_TAG_formal_parameter);
            let param = self.dwarf.unit.get_mut(param_id);
            param.set(
                gimli::DW_AT_name,
                AttributeValue::String(pname.as_bytes().to_vec()),
            );
        }
    }

    pub fn add_source_line(&mut self, line: usize, offset: u64) {
        self.source_map.push((line, offset));
    }

    pub fn write_to_sections(&self) -> Result<Vec<u8>, gimli::write::Error> {
        let mut sections = Sections::new(EndianVec::new(LittleEndian));
        self.dwarf.write(&mut sections)?;
        // Collect sections into bytes for embedding in object file
        // ...
        Ok(Vec::new()) // simplified
    }
}
```

### 2.5 Cranelift Source Location Mapping

```rust
// Pattern: Set source locations during codegen for DWARF
// In compile_stmt(), before generating IR for each statement:

fn compile_stmt_with_debug<M: Module>(
    cx: &mut CodegenCtx<'_, M>,
    builder: &mut FunctionBuilder,
    stmt: &Stmt,
) -> Result<(), CodegenError> {
    // Set source location for debug info
    if let Some(span) = stmt.span() {
        let srcloc = cranelift_codegen::ir::SourceLoc::new(span.start_line as u32);
        builder.set_srcloc(srcloc);
    }

    // Normal statement compilation...
    compile_stmt(cx, builder, stmt)
}
```

---

## 3. Board Support Package Patterns (Phase 3)

### 3.1 Board Trait

```rust
// src/bsp/mod.rs
pub mod stm32f407;
pub mod esp32;
pub mod rp2040;

pub trait Board {
    fn name(&self) -> &str;
    fn arch(&self) -> Arch;
    fn memory_regions(&self) -> Vec<MemoryRegion>;
    fn peripherals(&self) -> Vec<Peripheral>;
    fn vector_table_size(&self) -> usize;
    fn clock_speed_hz(&self) -> u32;
    fn linker_script(&self) -> String;
    fn startup_code(&self) -> String;
}

#[derive(Clone, Debug)]
pub struct MemoryRegion {
    pub name: String,
    pub origin: u64,
    pub length: u64,
    pub attributes: MemoryAttributes,
}

#[derive(Clone, Debug)]
pub enum MemoryAttributes {
    Rx,     // Read-execute (Flash)
    Rwx,    // Read-write-execute (SRAM)
    Rw,     // Read-write (peripherals)
}

#[derive(Clone, Debug)]
pub struct Peripheral {
    pub name: String,
    pub base_address: u64,
    pub registers: Vec<Register>,
}

#[derive(Clone, Debug)]
pub struct Register {
    pub name: String,
    pub offset: u32,
    pub size: RegisterSize,
    pub access: Access,
}

#[derive(Clone, Copy, Debug)]
pub enum Arch {
    X86_64,
    Aarch64,
    Riscv64,
    Riscv32,
    Thumbv7em,   // Cortex-M4/M7 (STM32F4)
    Thumbv6m,    // Cortex-M0/M0+  (RP2040)
    Xtensa,      // ESP32
}
```

### 3.2 STM32F407 Memory Map

```rust
// src/bsp/stm32f407.rs
pub struct Stm32f407;

impl Board for Stm32f407 {
    fn name(&self) -> &str { "stm32f407vg" }
    fn arch(&self) -> Arch { Arch::Thumbv7em }
    fn clock_speed_hz(&self) -> u32 { 168_000_000 } // 168 MHz

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                name: "FLASH".into(),
                origin: 0x0800_0000,
                length: 1024 * 1024, // 1MB
                attributes: MemoryAttributes::Rx,
            },
            MemoryRegion {
                name: "SRAM1".into(),
                origin: 0x2000_0000,
                length: 112 * 1024, // 112KB
                attributes: MemoryAttributes::Rwx,
            },
            MemoryRegion {
                name: "SRAM2".into(),
                origin: 0x2001_C000,
                length: 16 * 1024, // 16KB
                attributes: MemoryAttributes::Rwx,
            },
            MemoryRegion {
                name: "CCM".into(),
                origin: 0x1000_0000,
                length: 64 * 1024, // 64KB CCM SRAM
                attributes: MemoryAttributes::Rw,
            },
        ]
    }

    fn vector_table_size(&self) -> usize { 98 } // 16 system + 82 peripheral IRQs
}
```

### 3.3 Linker Script Generation

```rust
// Extend src/codegen/linker.rs
pub fn generate_linker_script(board: &dyn Board) -> String {
    let mut script = String::new();

    // Memory regions
    script.push_str("MEMORY {\n");
    for region in board.memory_regions() {
        script.push_str(&format!(
            "    {} ({}) : ORIGIN = {:#010X}, LENGTH = {}K\n",
            region.name,
            match region.attributes {
                MemoryAttributes::Rx  => "rx",
                MemoryAttributes::Rwx => "rwx",
                MemoryAttributes::Rw  => "rw",
            },
            region.origin,
            region.length / 1024,
        ));
    }
    script.push_str("}\n\n");

    // Sections
    script.push_str("SECTIONS {\n");
    script.push_str("    .isr_vector : {\n");
    script.push_str("        . = ALIGN(4);\n");
    script.push_str("        KEEP(*(.isr_vector))\n");
    script.push_str("        . = ALIGN(4);\n");
    script.push_str("    } > FLASH\n\n");

    script.push_str("    .text : {\n");
    script.push_str("        . = ALIGN(4);\n");
    script.push_str("        *(.text .text.*)\n");
    script.push_str("        . = ALIGN(4);\n");
    script.push_str("    } > FLASH\n\n");

    script.push_str("    .data : {\n");
    script.push_str("        . = ALIGN(4);\n");
    script.push_str("        _sdata = .;\n");
    script.push_str("        *(.data .data.*)\n");
    script.push_str("        _edata = .;\n");
    script.push_str("    } > SRAM1 AT> FLASH\n\n");

    script.push_str("    .bss : {\n");
    script.push_str("        . = ALIGN(4);\n");
    script.push_str("        _sbss = .;\n");
    script.push_str("        *(.bss .bss.*)\n");
    script.push_str("        *(COMMON)\n");
    script.push_str("        _ebss = .;\n");
    script.push_str("    } > SRAM1\n\n");

    script.push_str("    _stack_top = ORIGIN(SRAM1) + LENGTH(SRAM1);\n");
    script.push_str("}\n");

    script
}
```

### 3.4 Cortex-M Startup Sequence

```rust
// Pattern: Generated Reset_Handler for ARM Cortex-M
// 1. Set MSP (Main Stack Pointer)
// 2. Copy .data from Flash to SRAM
// 3. Zero .bss section
// 4. Enable FPU (if Cortex-M4F)
// 5. Call SystemInit (clock config)
// 6. Call main()
// 7. Infinite loop (if main returns)

pub fn generate_startup_asm(board: &dyn Board) -> String {
    let mut asm = String::new();

    asm.push_str(".syntax unified\n");
    asm.push_str(".cpu cortex-m4\n");
    asm.push_str(".fpu fpv4-sp-d16\n");
    asm.push_str(".thumb\n\n");

    // Vector table
    asm.push_str(".section .isr_vector, \"a\", %progbits\n");
    asm.push_str(".word _stack_top\n");
    asm.push_str(".word Reset_Handler\n");
    // NMI, HardFault, etc...
    for i in 2..board.vector_table_size() {
        asm.push_str(&format!(".word Default_Handler  @ IRQ {}\n", i));
    }

    // Reset handler
    asm.push_str("\n.section .text\n");
    asm.push_str(".global Reset_Handler\n");
    asm.push_str(".type Reset_Handler, %function\n");
    asm.push_str("Reset_Handler:\n");

    // Copy .data
    asm.push_str("    ldr r0, =_sdata\n");
    asm.push_str("    ldr r1, =_edata\n");
    asm.push_str("    ldr r2, =_sidata\n");
    asm.push_str("    b copy_data_check\n");
    asm.push_str("copy_data_loop:\n");
    asm.push_str("    ldr r3, [r2], #4\n");
    asm.push_str("    str r3, [r0], #4\n");
    asm.push_str("copy_data_check:\n");
    asm.push_str("    cmp r0, r1\n");
    asm.push_str("    blt copy_data_loop\n");

    // Zero .bss
    asm.push_str("    ldr r0, =_sbss\n");
    asm.push_str("    ldr r1, =_ebss\n");
    asm.push_str("    movs r2, #0\n");
    asm.push_str("    b zero_bss_check\n");
    asm.push_str("zero_bss_loop:\n");
    asm.push_str("    str r2, [r0], #4\n");
    asm.push_str("zero_bss_check:\n");
    asm.push_str("    cmp r0, r1\n");
    asm.push_str("    blt zero_bss_loop\n");

    // Enable FPU (Cortex-M4F)
    if board.arch() == Arch::Thumbv7em {
        asm.push_str("    ldr r0, =0xE000ED88\n");
        asm.push_str("    ldr r1, [r0]\n");
        asm.push_str("    orr r1, r1, #(0xF << 20)\n");
        asm.push_str("    str r1, [r0]\n");
        asm.push_str("    dsb\n");
        asm.push_str("    isb\n");
    }

    // Call main
    asm.push_str("    bl main\n");
    asm.push_str("    b .\n");

    asm
}
```

### 3.5 RP2040 UF2 Format

```rust
// UF2 block format for drag-and-drop flashing
// Each block: 512 bytes (32-byte header + 476 data + 4-byte magic)
const UF2_MAGIC_START0: u32 = 0x0A324655;
const UF2_MAGIC_START1: u32 = 0x9E5D5157;
const UF2_MAGIC_END:    u32 = 0x0AB16F30;
const UF2_FLAG_FAMILY: u32 = 0x00002000;
const RP2040_FAMILY_ID: u32 = 0xE48BFF56;

pub fn elf_to_uf2(elf_data: &[u8], flash_base: u32) -> Vec<u8> {
    let payload_size = 256; // bytes per UF2 block
    let num_blocks = (elf_data.len() + payload_size - 1) / payload_size;
    let mut uf2 = Vec::with_capacity(num_blocks * 512);

    for (i, chunk) in elf_data.chunks(payload_size).enumerate() {
        let mut block = [0u8; 512];
        // Header
        block[0..4].copy_from_slice(&UF2_MAGIC_START0.to_le_bytes());
        block[4..8].copy_from_slice(&UF2_MAGIC_START1.to_le_bytes());
        block[8..12].copy_from_slice(&UF2_FLAG_FAMILY.to_le_bytes());
        let addr = flash_base + (i * payload_size) as u32;
        block[12..16].copy_from_slice(&addr.to_le_bytes());
        block[16..20].copy_from_slice(&(chunk.len() as u32).to_le_bytes());
        block[20..24].copy_from_slice(&(i as u32).to_le_bytes());
        block[24..28].copy_from_slice(&(num_blocks as u32).to_le_bytes());
        block[28..32].copy_from_slice(&RP2040_FAMILY_ID.to_le_bytes());
        // Payload
        block[32..32 + chunk.len()].copy_from_slice(chunk);
        // End magic
        block[508..512].copy_from_slice(&UF2_MAGIC_END.to_le_bytes());
        uf2.extend_from_slice(&block);
    }
    uf2
}
```

---

## 4. Package Registry Patterns (Phase 4)

### 4.1 PubGrub Dependency Provider

```rust
// src/package/resolver.rs
use pubgrub::{
    DependencyProvider, Dependencies, Package, Version,
    Range, Map, Set,
};

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct FjPackage(pub String);

impl Package for FjPackage {}

pub struct FjDependencyProvider {
    /// Known packages: name → sorted versions
    packages: HashMap<String, Vec<semver::Version>>,
    /// Dependencies: (name, version) → deps
    dependencies: HashMap<(String, semver::Version), Vec<(String, Range<semver::Version>)>>,
}

impl DependencyProvider for FjDependencyProvider {
    type P = FjPackage;
    type V = semver::Version;
    type VS = Range<semver::Version>;
    type M = String;

    fn choose_version(
        &self,
        package: &FjPackage,
        range: &Range<semver::Version>,
    ) -> Result<Option<semver::Version>, Self::M> {
        // Return highest version matching range
        if let Some(versions) = self.packages.get(&package.0) {
            for v in versions.iter().rev() {
                if range.contains(v) {
                    return Ok(Some(v.clone()));
                }
            }
        }
        Ok(None)
    }

    fn get_dependencies(
        &self,
        package: &FjPackage,
        version: &semver::Version,
    ) -> Result<Dependencies<Self::P, Self::VS, Self::M>, Self::M> {
        let key = (package.0.clone(), version.clone());
        if let Some(deps) = self.dependencies.get(&key) {
            let map: Map<FjPackage, Range<semver::Version>> = deps
                .iter()
                .map(|(name, range)| (FjPackage(name.clone()), range.clone()))
                .collect();
            Ok(Dependencies::Available(map))
        } else {
            Err(format!("Package {}@{} not found", package.0, version))
        }
    }

    fn prioritize(
        &self,
        package: &FjPackage,
        _range: &Range<semver::Version>,
    ) -> Self::Priority {
        // Fewer versions = higher priority (more constrained)
        let count = self.packages.get(&package.0).map(|v| v.len()).unwrap_or(0);
        std::cmp::Reverse(count)
    }
}
```

### 4.2 Registry Server (Axum)

```rust
// packages/fj-registry/src/main.rs
use axum::{
    routing::{get, put, delete},
    Router, Json, extract::{Path, Query, State},
    body::Bytes,
    http::StatusCode,
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub storage_path: PathBuf,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/crates/new", put(publish_crate))
        .route("/api/v1/crates/:name/:version/download", get(download_crate))
        .route("/api/v1/crates", get(search_crates))
        .route("/api/v1/crates/:name/:version/yank", delete(yank_crate))
        .route("/api/v1/crates/:name", get(crate_info))
        .with_state(state)
}

async fn publish_crate(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<PublishResponse>, StatusCode> {
    // Binary format: [4-byte json_len][json_metadata][4-byte tarball_len][tarball]
    let json_len = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let json_bytes = &body[4..4 + json_len];
    let metadata: PackageMetadata = serde_json::from_slice(json_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let tarball_offset = 4 + json_len;
    let tarball_len = u32::from_le_bytes(
        body[tarball_offset..tarball_offset + 4].try_into().unwrap()
    ) as usize;
    let tarball = &body[tarball_offset + 4..tarball_offset + 4 + tarball_len];

    // Compute SHA256
    use sha2::{Sha256, Digest};
    let checksum = hex::encode(Sha256::digest(tarball));

    // Store in database + filesystem
    // ...

    Ok(Json(PublishResponse { ok: true, checksum }))
}
```

### 4.3 Lock File Format

```toml
# fj.lock — generated by `fj build`, do not edit manually
# This file ensures reproducible builds.

[[package]]
name = "fj-math"
version = "0.1.0"
checksum = "sha256:abcdef1234567890..."
dependencies = []

[[package]]
name = "fj-nn"
version = "0.2.0"
checksum = "sha256:fedcba0987654321..."
dependencies = [
    { name = "fj-math", version = "0.1.0" },
]
```

---

## 5. Lifetime Annotation Patterns (Phase 5)

### 5.1 AST Representation

```rust
// src/parser/ast.rs — extensions

#[derive(Clone, Debug, PartialEq)]
pub struct LifetimeParam {
    pub name: String,    // e.g., "a" from 'a
    pub span: Span,
}

// Extended FnDef
pub struct FnDef {
    pub name: String,
    pub lifetime_params: Vec<LifetimeParam>,  // NEW
    pub generic_params: Vec<GenericParam>,
    pub params: Vec<(String, TypeExpr)>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    // ...
}

// Extended TypeExpr
pub enum TypeExpr {
    // Existing...
    Ref(Option<String>, Box<TypeExpr>),        // &'a T — lifetime is optional
    RefMut(Option<String>, Box<TypeExpr>),      // &'a mut T
    // ...
}
```

### 5.2 Lifetime Elision Rules

```rust
// src/analyzer/lifetimes.rs
// Three elision rules (same as Rust):
//
// Rule 1: Each input reference gets a distinct lifetime
//   fn foo(x: &T, y: &U) → fn foo<'a, 'b>(x: &'a T, y: &'b U)
//
// Rule 2: If exactly one input lifetime, output gets that lifetime
//   fn foo(x: &T) -> &T → fn foo<'a>(x: &'a T) -> &'a T
//
// Rule 3: If &self or &mut self, output gets self's lifetime
//   fn foo(&self, x: &T) -> &U → fn foo<'a, 'b>(&'a self, x: &'b T) -> &'a U

pub fn apply_elision(fn_def: &mut FnDef) {
    // Count input references without explicit lifetimes
    let input_refs: Vec<usize> = fn_def.params.iter()
        .enumerate()
        .filter(|(_, (_, ty))| matches!(ty, TypeExpr::Ref(None, _) | TypeExpr::RefMut(None, _)))
        .map(|(i, _)| i)
        .collect();

    if input_refs.is_empty() { return; }

    // Rule 1: Assign distinct lifetimes
    let mut lifetime_counter = 0u8;
    for &idx in &input_refs {
        let lt = format!("'{}", (b'a' + lifetime_counter) as char);
        lifetime_counter += 1;
        assign_lifetime(&mut fn_def.params[idx].1, &lt);
        fn_def.lifetime_params.push(LifetimeParam {
            name: lt,
            span: fn_def.params[idx].1.span(),
        });
    }

    // Rule 2 or 3: Assign output lifetime
    if let Some(ret_ty) = &mut fn_def.return_type {
        if is_reference(ret_ty) {
            let output_lt = if has_self_param(&fn_def.params) {
                // Rule 3: use self's lifetime
                "'a".to_string()
            } else if input_refs.len() == 1 {
                // Rule 2: use the single input lifetime
                "'a".to_string()
            } else {
                // Ambiguous — require explicit annotation
                return; // Will trigger error in analyzer
            };
            assign_lifetime(ret_ty, &output_lt);
        }
    }
}
```

### 5.3 CFG-Based Region Inference

```rust
// src/analyzer/regions.rs
use std::collections::{HashMap, HashSet, BTreeSet};

/// A region variable representing the set of program points where a borrow is live
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Region(pub usize);

/// A program point in the CFG
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Point {
    pub block: usize,
    pub statement: usize,
}

/// Constraint: region R must include point P
pub struct LivenessConstraint {
    pub region: Region,
    pub point: Point,
}

/// Constraint: region A outlives region B (A : B)
pub struct OutlivesConstraint {
    pub longer: Region,  // must live at least as long as...
    pub shorter: Region, // ...this region
    pub point: Point,    // at this program point
}

pub struct RegionInference {
    /// Each region maps to its set of live points
    pub regions: HashMap<Region, BTreeSet<Point>>,
    pub liveness_constraints: Vec<LivenessConstraint>,
    pub outlives_constraints: Vec<OutlivesConstraint>,
}

impl RegionInference {
    /// Fixed-point iteration: propagate outlives constraints
    pub fn solve(&mut self) -> Result<(), Vec<LifetimeError>> {
        let mut changed = true;
        while changed {
            changed = false;
            for constraint in &self.outlives_constraints {
                let shorter_points = self.regions.get(&constraint.shorter)
                    .cloned()
                    .unwrap_or_default();
                let longer = self.regions.entry(constraint.longer.clone())
                    .or_default();
                for point in &shorter_points {
                    if longer.insert(point.clone()) {
                        changed = true;
                    }
                }
            }
        }

        // Check for errors: borrow outlives data
        self.check_errors()
    }

    fn check_errors(&self) -> Result<(), Vec<LifetimeError>> {
        // If region R includes a point after the borrowed data's scope ends,
        // that's a dangling reference error
        // ...
        Ok(())
    }
}
```

---

## 6. FreeRTOS Integration Patterns (Phase 6)

### 6.1 FFI Wrapper Pattern

```rust
// src/rtos/freertos.rs
// FreeRTOS C API → Rust FFI declarations
// Note: FreeRTOS uses macros (xTaskCreate, etc.) which cannot be FFI'd directly.
// Solution: thin C shim that wraps macros into real functions.

// C shim (compiled separately, linked):
// void fj_freertos_task_create(
//     TaskFunction_t fn, const char* name,
//     uint16_t stack, void* param, UBaseType_t priority, TaskHandle_t* handle
// ) {
//     xTaskCreate(fn, name, stack, param, priority, handle);
// }

extern "C" {
    fn fj_freertos_task_create(
        func: extern "C" fn(*mut core::ffi::c_void),
        name: *const u8,
        stack_depth: u16,
        param: *mut core::ffi::c_void,
        priority: u32,
        handle: *mut *mut core::ffi::c_void,
    ) -> i32;

    fn fj_freertos_task_delay(ticks: u32);
    fn fj_freertos_task_delay_until(prev_wake: *mut u32, increment: u32);

    fn fj_freertos_queue_create(length: u32, item_size: u32) -> *mut core::ffi::c_void;
    fn fj_freertos_queue_send(
        queue: *mut core::ffi::c_void,
        item: *const core::ffi::c_void,
        timeout: u32,
    ) -> i32;
    fn fj_freertos_queue_receive(
        queue: *mut core::ffi::c_void,
        buffer: *mut core::ffi::c_void,
        timeout: u32,
    ) -> i32;

    fn fj_freertos_mutex_create() -> *mut core::ffi::c_void;
    fn fj_freertos_mutex_lock(mutex: *mut core::ffi::c_void, timeout: u32) -> i32;
    fn fj_freertos_mutex_unlock(mutex: *mut core::ffi::c_void) -> i32;

    fn vTaskStartScheduler();
}
```

### 6.2 Runtime Functions for Fajar Lang

```rust
// src/rtos/runtime.rs — Called from Fajar native codegen
// Pattern: Same as other fj_rt_* functions in runtime_fns.rs

#[no_mangle]
pub extern "C" fn fj_rt_task_create(
    fn_ptr: extern "C" fn(*mut core::ffi::c_void),
    name_ptr: *const u8,
    name_len: i64,
    stack_size: i64,
    priority: i64,
) -> i64 {
    unsafe {
        let mut handle: *mut core::ffi::c_void = std::ptr::null_mut();
        let result = fj_freertos_task_create(
            fn_ptr,
            name_ptr,
            stack_size as u16,
            std::ptr::null_mut(),
            priority as u32,
            &mut handle,
        );
        if result == 1 { // pdPASS
            handle as i64
        } else {
            0 // null handle = failure
        }
    }
}

#[no_mangle]
pub extern "C" fn fj_rt_task_delay_ms(ms: i64) {
    unsafe {
        // configTICK_RATE_HZ is typically 1000
        fj_freertos_task_delay(ms as u32);
    }
}

#[no_mangle]
pub extern "C" fn fj_rt_scheduler_start() {
    unsafe {
        vTaskStartScheduler();
    }
}
```

### 6.3 @periodic Annotation Pattern

```rust
// @periodic(period: 10ms) generates:
//
// void task_wrapper(void* param) {
//     TickType_t last_wake = xTaskGetTickCount();
//     for(;;) {
//         user_function();
//         vTaskDelayUntil(&last_wake, pdMS_TO_TICKS(10));
//     }
// }
//
// Then creates task with xTaskCreate(task_wrapper, ...)

// In analyzer, @periodic on a function:
// 1. Verify the function takes no parameters and returns void
// 2. Store annotation metadata (period, priority)
// 3. In codegen, generate wrapper function with delay loop
```

### 6.4 FreeRTOSConfig.h Generation

```rust
// Generate board-specific FreeRTOS configuration
pub fn generate_freertos_config(board: &dyn Board) -> String {
    format!(r#"
#ifndef FREERTOS_CONFIG_H
#define FREERTOS_CONFIG_H

#define configUSE_PREEMPTION                1
#define configCPU_CLOCK_HZ                  ({clock})
#define configTICK_RATE_HZ                  1000
#define configMAX_PRIORITIES                5
#define configMINIMAL_STACK_SIZE            128
#define configTOTAL_HEAP_SIZE               ({heap_size})
#define configMAX_TASK_NAME_LEN             16
#define configUSE_16_BIT_TICKS              0
#define configIDLE_SHOULD_YIELD             1
#define configUSE_MUTEXES                   1
#define configUSE_RECURSIVE_MUTEXES         1
#define configUSE_COUNTING_SEMAPHORES       1
#define configUSE_QUEUE_SETS                1
#define configUSE_TASK_NOTIFICATIONS        1
#define configSUPPORT_DYNAMIC_ALLOCATION    1

/* Cortex-M specific */
#define configPRIO_BITS                     4
#define configLIBRARY_LOWEST_INTERRUPT_PRIORITY     15
#define configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY 5
#define configKERNEL_INTERRUPT_PRIORITY     (configLIBRARY_LOWEST_INTERRUPT_PRIORITY << (8 - configPRIO_BITS))
#define configMAX_SYSCALL_INTERRUPT_PRIORITY (configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY << (8 - configPRIO_BITS))

#define configUSE_IDLE_HOOK                 0
#define configUSE_TICK_HOOK                 0

/* Memory: use heap_4 by default */
#define configAPPLICATION_ALLOCATED_HEAP    0

#endif
"#,
        clock = board.clock_speed_hz(),
        heap_size = board.memory_regions()
            .iter()
            .filter(|r| matches!(r.attributes, MemoryAttributes::Rwx))
            .map(|r| r.length)
            .sum::<u64>() / 2, // Use half of SRAM for heap
    )
}
```

---

## 7. Advanced ML Patterns (Phase 7)

### 7.1 LSTM Cell Forward

```rust
// src/runtime/ml/rnn.rs
use ndarray::{Array2, Axis, concatenate};

pub struct LSTMCell {
    pub w_ih: Array2<f64>,  // [4*hidden_size, input_size]  — input→hidden
    pub w_hh: Array2<f64>,  // [4*hidden_size, hidden_size] — hidden→hidden
    pub b_ih: Array2<f64>,  // [1, 4*hidden_size]
    pub b_hh: Array2<f64>,  // [1, 4*hidden_size]
    pub hidden_size: usize,
}

impl LSTMCell {
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        // Xavier initialization for gates
        let scale_ih = (2.0 / (input_size + hidden_size) as f64).sqrt();
        let scale_hh = (2.0 / (hidden_size + hidden_size) as f64).sqrt();
        Self {
            w_ih: Array2::from_shape_fn((4 * hidden_size, input_size), |_| {
                rand::random::<f64>() * 2.0 * scale_ih - scale_ih
            }),
            w_hh: Array2::from_shape_fn((4 * hidden_size, hidden_size), |_| {
                rand::random::<f64>() * 2.0 * scale_hh - scale_hh
            }),
            b_ih: Array2::zeros((1, 4 * hidden_size)),
            b_hh: Array2::zeros((1, 4 * hidden_size)),
            hidden_size,
        }
    }

    /// Forward pass for single timestep
    /// Returns (h_t, c_t)
    pub fn forward(
        &self,
        x_t: &Array2<f64>,   // [batch, input_size]
        h_prev: &Array2<f64>, // [batch, hidden_size]
        c_prev: &Array2<f64>, // [batch, hidden_size]
    ) -> (Array2<f64>, Array2<f64>) {
        let h = self.hidden_size;

        // gates = x_t @ W_ih^T + h_prev @ W_hh^T + b_ih + b_hh
        let gates = x_t.dot(&self.w_ih.t()) + h_prev.dot(&self.w_hh.t())
            + &self.b_ih + &self.b_hh;

        // Split into 4 gates: [forget, input, candidate, output]
        let f_gate = gates.slice(s![.., 0..h]).mapv(sigmoid);      // forget gate
        let i_gate = gates.slice(s![.., h..2*h]).mapv(sigmoid);     // input gate
        let c_tilde = gates.slice(s![.., 2*h..3*h]).mapv(f64::tanh); // candidate
        let o_gate = gates.slice(s![.., 3*h..4*h]).mapv(sigmoid);   // output gate

        // Cell state update: c_t = f * c_prev + i * c~
        let c_t = &f_gate * c_prev + &i_gate * &c_tilde;

        // Hidden state: h_t = o * tanh(c_t)
        let h_t = &o_gate * &c_t.mapv(f64::tanh);

        (h_t.to_owned(), c_t.to_owned())
    }
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}
```

### 7.2 GRU Cell Forward

```rust
pub struct GRUCell {
    pub w_ih: Array2<f64>,  // [3*hidden_size, input_size]
    pub w_hh: Array2<f64>,  // [3*hidden_size, hidden_size]
    pub b_ih: Array2<f64>,  // [1, 3*hidden_size]
    pub b_hh: Array2<f64>,  // [1, 3*hidden_size]
    pub hidden_size: usize,
}

impl GRUCell {
    /// Forward pass for single timestep
    /// Returns h_t
    pub fn forward(
        &self,
        x_t: &Array2<f64>,
        h_prev: &Array2<f64>,
    ) -> Array2<f64> {
        let h = self.hidden_size;

        let gi = x_t.dot(&self.w_ih.t()) + &self.b_ih;
        let gh = h_prev.dot(&self.w_hh.t()) + &self.b_hh;

        // Reset and update gates
        let r_gate = (&gi.slice(s![.., 0..h]) + &gh.slice(s![.., 0..h])).mapv(sigmoid);
        let z_gate = (&gi.slice(s![.., h..2*h]) + &gh.slice(s![.., h..2*h])).mapv(sigmoid);

        // Candidate hidden state
        let n_hat = (&gi.slice(s![.., 2*h..3*h])
            + &r_gate * &gh.slice(s![.., 2*h..3*h])).mapv(f64::tanh);

        // Interpolation: h_t = (1 - z) * n_hat + z * h_prev
        let ones = Array2::ones(z_gate.raw_dim());
        let h_t = (&ones - &z_gate) * &n_hat + &z_gate * h_prev;

        h_t.to_owned()
    }
}
```

### 7.3 AdamW Optimizer

```rust
// src/runtime/ml/optim.rs — extension
pub struct AdamW {
    pub lr: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub epsilon: f64,
    pub weight_decay: f64,  // Decoupled weight decay
    pub step_count: usize,
    pub m: Vec<Array2<f64>>,  // First moment
    pub v: Vec<Array2<f64>>,  // Second moment
}

impl AdamW {
    pub fn step(&mut self, params: &mut [Array2<f64>], grads: &[Array2<f64>]) {
        self.step_count += 1;
        let t = self.step_count as f64;

        for (i, (param, grad)) in params.iter_mut().zip(grads.iter()).enumerate() {
            // Moment updates (same as Adam)
            self.m[i] = &self.m[i] * self.beta1 + grad * (1.0 - self.beta1);
            self.v[i] = &self.v[i] * self.beta2 + &(grad * grad) * (1.0 - self.beta2);

            // Bias correction
            let m_hat = &self.m[i] / (1.0 - self.beta1.powf(t));
            let v_hat = &self.v[i] / (1.0 - self.beta2.powf(t));

            // AdamW: decoupled weight decay (applied to param, not gradient)
            *param = &*param - &(&m_hat / &(v_hat.mapv(f64::sqrt) + self.epsilon)) * self.lr;
            *param = &*param - &*param * (self.lr * self.weight_decay);
        }
    }
}
```

### 7.4 Learning Rate Schedulers

```rust
pub trait LRScheduler {
    fn get_lr(&self, step: usize) -> f64;
}

/// Linear warmup from 0 → base_lr over warmup_steps
pub struct WarmupLR {
    pub base_lr: f64,
    pub warmup_steps: usize,
}

impl LRScheduler for WarmupLR {
    fn get_lr(&self, step: usize) -> f64 {
        if step < self.warmup_steps {
            self.base_lr * (step as f64 / self.warmup_steps as f64)
        } else {
            self.base_lr
        }
    }
}

/// Cosine annealing: lr = min_lr + 0.5*(max_lr - min_lr)*(1 + cos(pi * T_cur / T_max))
pub struct CosineAnnealingLR {
    pub max_lr: f64,
    pub min_lr: f64,
    pub t_max: usize,
}

impl LRScheduler for CosineAnnealingLR {
    fn get_lr(&self, step: usize) -> f64 {
        let t = (step % self.t_max) as f64;
        self.min_lr + 0.5 * (self.max_lr - self.min_lr)
            * (1.0 + (std::f64::consts::PI * t / self.t_max as f64).cos())
    }
}

/// ReduceOnPlateau: reduce LR when metric stops improving
pub struct ReduceOnPlateau {
    pub factor: f64,         // Multiply LR by this (e.g., 0.1)
    pub patience: usize,     // Wait this many steps before reducing
    pub min_lr: f64,
    pub current_lr: f64,
    pub best_metric: f64,
    pub wait: usize,
}

impl ReduceOnPlateau {
    pub fn step_with_metric(&mut self, metric: f64) -> f64 {
        if metric < self.best_metric {
            self.best_metric = metric;
            self.wait = 0;
        } else {
            self.wait += 1;
            if self.wait >= self.patience {
                self.current_lr = (self.current_lr * self.factor).max(self.min_lr);
                self.wait = 0;
            }
        }
        self.current_lr
    }
}
```

### 7.5 ThreadedDataLoader

```rust
// src/runtime/ml/dataloader.rs
use std::sync::{mpsc, Arc};
use std::thread;

pub trait Dataset: Send + Sync {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> (Array2<f64>, Array2<f64>); // (features, labels)
}

pub struct ThreadedDataLoader<D: Dataset> {
    dataset: Arc<D>,
    batch_size: usize,
    num_workers: usize,
    shuffle: bool,
}

impl<D: Dataset + 'static> ThreadedDataLoader<D> {
    pub fn iter(&self) -> DataLoaderIter {
        let indices = if self.shuffle {
            let mut idx: Vec<usize> = (0..self.dataset.len()).collect();
            // Fisher-Yates shuffle
            for i in (1..idx.len()).rev() {
                let j = rand::random::<usize>() % (i + 1);
                idx.swap(i, j);
            }
            idx
        } else {
            (0..self.dataset.len()).collect()
        };

        let (tx, rx) = mpsc::sync_channel(self.num_workers * 2);
        let batches: Vec<Vec<usize>> = indices.chunks(self.batch_size)
            .map(|c| c.to_vec())
            .collect();

        let dataset = self.dataset.clone();
        let num_workers = self.num_workers;

        // Spawn worker threads
        thread::spawn(move || {
            let pool: Vec<_> = (0..num_workers).map(|_| {
                let (batch_tx, batch_rx) = mpsc::channel::<Vec<usize>>();
                let ds = dataset.clone();
                let result_tx = tx.clone();
                thread::spawn(move || {
                    while let Ok(batch_indices) = batch_rx.recv() {
                        let samples: Vec<_> = batch_indices.iter()
                            .map(|&i| ds.get(i))
                            .collect();
                        // Collate: stack into batch tensors
                        let features = collate_features(&samples);
                        let labels = collate_labels(&samples);
                        result_tx.send((features, labels)).ok();
                    }
                });
                batch_tx
            }).collect();

            for (i, batch) in batches.into_iter().enumerate() {
                pool[i % num_workers].send(batch).ok();
            }
        });

        DataLoaderIter { rx }
    }
}
```

### 7.6 Early Stopping

```rust
pub struct EarlyStopping {
    pub patience: usize,
    pub min_delta: f64,
    pub best_metric: f64,
    pub wait: usize,
    pub stopped: bool,
}

impl EarlyStopping {
    pub fn new(patience: usize, min_delta: f64) -> Self {
        Self {
            patience,
            min_delta,
            best_metric: f64::INFINITY,
            wait: 0,
            stopped: false,
        }
    }

    /// Returns true if training should stop
    pub fn check(&mut self, metric: f64) -> bool {
        if metric < self.best_metric - self.min_delta {
            self.best_metric = metric;
            self.wait = 0;
        } else {
            self.wait += 1;
            if self.wait >= self.patience {
                self.stopped = true;
                return true;
            }
        }
        false
    }
}
```

---

## 8. Cross-Cutting Patterns

### 8.1 Feature Gating

```toml
# Cargo.toml feature gates
[features]
default = []
native = ["cranelift-codegen", "cranelift-frontend", ...]  # Existing
llvm = ["inkwell"]                                          # New LLVM backend
rtos = []                                                   # RTOS FFI bindings
bsp = ["llvm"]                                              # BSP requires LLVM
```

### 8.2 Backend Selection at CLI

```rust
// src/main.rs
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Backend {
    Interpreter,
    Vm,
    Cranelift,
    #[cfg(feature = "llvm")]
    Llvm,
}

// Usage: fj run --backend llvm --opt-level 3 examples/fibonacci.fj
```

### 8.3 Test Pattern for Feature-Gated Code

```rust
#[cfg(feature = "llvm")]
#[test]
fn llvm_fibonacci_produces_correct_result() {
    let context = Context::create();
    let mut compiler = LlvmCompiler::new(&context, "test");
    let program = parse_source("fn main() -> i64 { fibonacci(10) } ...");
    compiler.compile_program(&program).unwrap();
    let result = compiler.jit_execute().unwrap();
    assert_eq!(result, 55);
}
```

---

*V06_SKILLS.md v1.0 | Implementation patterns for v0.6 "Horizon" | Created 2026-03-11*
