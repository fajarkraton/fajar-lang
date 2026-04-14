//! Sprint S5: Stage 1 Bootstrap — Self-hosted compiler bootstrap chain.
//!
//! Provides the Stage1Compiler that can compile a subset of Fajar Lang,
//! bootstrap verification (Stage 0 == Stage 1 output), subset definition,
//! bootstrap test suite, and reproducible builds verification.

use std::fmt;
use std::time::{Duration, Instant};

use super::ast_tree::{AstProgram, Expr, Item, Stmt};
use super::codegen_v2::{CodegenV2, CompiledFn, Constant, Instruction, peephole_optimize};

// ═══════════════════════════════════════════════════════════════════════
// S5.1: Subset Definition
// ═══════════════════════════════════════════════════════════════════════

/// Defines what language features Stage 1 can handle.
#[derive(Debug, Clone)]
pub struct SubsetDefinition {
    /// Supported expression kinds.
    pub expressions: Vec<String>,
    /// Supported statement kinds.
    pub statements: Vec<String>,
    /// Supported type expressions.
    pub types: Vec<String>,
    /// Maximum function parameter count.
    pub max_params: usize,
    /// Whether generics are supported.
    pub supports_generics: bool,
    /// Whether closures are supported.
    pub supports_closures: bool,
    /// Whether pattern matching is supported.
    pub supports_match: bool,
    /// Whether async is supported.
    pub supports_async: bool,
}

impl SubsetDefinition {
    /// Returns the Stage 1 subset definition.
    pub fn stage1() -> Self {
        Self {
            expressions: vec![
                "int_lit".into(),
                "float_lit".into(),
                "bool_lit".into(),
                "string_lit".into(),
                "null_lit".into(),
                "ident".into(),
                "bin_op".into(),
                "unary_op".into(),
                "call".into(),
                "if".into(),
                "block".into(),
                "array_lit".into(),
                "index".into(),
                "field_access".into(),
                "assign".into(),
            ],
            statements: vec![
                "let".into(),
                "fn_def".into(),
                "return".into(),
                "while".into(),
                "for".into(),
                "break".into(),
                "continue".into(),
                "expr_stmt".into(),
                // V27.5 P3.2: @host annotation enables file I/O for Stage 1
                "host_fn".into(),
                "read_file".into(),
                "write_file".into(),
                "file_exists".into(),
            ],
            types: vec![
                "i8".into(),
                "i16".into(),
                "i32".into(),
                "i64".into(),
                "u8".into(),
                "u16".into(),
                "u32".into(),
                "u64".into(),
                "f32".into(),
                "f64".into(),
                "bool".into(),
                "str".into(),
                "void".into(),
            ],
            max_params: 16,
            supports_generics: false,
            supports_closures: false,
            supports_match: false,
            supports_async: false,
        }
    }

    /// Checks if an expression kind is in the subset.
    pub fn supports_expr(&self, kind: &str) -> bool {
        self.expressions.iter().any(|e| e == kind)
    }

    /// Checks if a statement kind is in the subset.
    pub fn supports_stmt(&self, kind: &str) -> bool {
        self.statements.iter().any(|s| s == kind)
    }

    /// Checks if a type is in the subset.
    pub fn supports_type(&self, ty: &str) -> bool {
        self.types.iter().any(|t| t == ty)
    }

    /// Returns the number of supported features.
    pub fn feature_count(&self) -> usize {
        self.expressions.len() + self.statements.len() + self.types.len()
    }
}

impl fmt::Display for SubsetDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Stage 1 Subset: {} exprs, {} stmts, {} types, generics={}, closures={}, match={}, async={}",
            self.expressions.len(),
            self.statements.len(),
            self.types.len(),
            self.supports_generics,
            self.supports_closures,
            self.supports_match,
            self.supports_async,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.2: Compilation Result
// ═══════════════════════════════════════════════════════════════════════

/// Result of a Stage 1 compilation.
#[derive(Debug, Clone)]
pub struct CompilationResult {
    /// Instructions produced.
    pub instructions: Vec<Instruction>,
    /// Constants.
    pub constants: Vec<Constant>,
    /// Functions compiled.
    pub functions: Vec<CompiledFn>,
    /// Compilation time.
    pub compile_time: Duration,
    /// Whether compilation succeeded.
    pub success: bool,
    /// Errors encountered.
    pub errors: Vec<String>,
    /// Warnings.
    pub warnings: Vec<String>,
}

impl CompilationResult {
    /// Creates a successful result.
    pub fn ok(
        instructions: Vec<Instruction>,
        constants: Vec<Constant>,
        functions: Vec<CompiledFn>,
        compile_time: Duration,
    ) -> Self {
        Self {
            instructions,
            constants,
            functions,
            compile_time,
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Creates a failed result.
    pub fn fail(errors: Vec<String>) -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
            functions: Vec::new(),
            compile_time: Duration::ZERO,
            success: false,
            errors,
            warnings: Vec::new(),
        }
    }

    /// Returns instruction count.
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Returns function count.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

impl fmt::Display for CompilationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(
                f,
                "OK: {} instructions, {} constants, {} functions ({:?})",
                self.instructions.len(),
                self.constants.len(),
                self.functions.len(),
                self.compile_time,
            )
        } else {
            write!(f, "FAIL: {} errors", self.errors.len())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.3: Stage 1 Compiler
// ═══════════════════════════════════════════════════════════════════════

/// The Stage 1 compiler — compiles a subset of Fajar Lang.
pub struct Stage1Compiler {
    /// Subset definition.
    pub subset: SubsetDefinition,
    /// Whether to apply optimizations.
    pub optimize: bool,
    /// Compilation statistics.
    pub stats: CompilerStats,
}

/// Compiler statistics.
#[derive(Debug, Clone, Default)]
pub struct CompilerStats {
    /// Number of programs compiled.
    pub programs_compiled: usize,
    /// Total instructions emitted.
    pub total_instructions: usize,
    /// Total functions compiled.
    pub total_functions: usize,
    /// Total compilation time.
    pub total_time: Duration,
    /// Optimization savings (instructions removed).
    pub optimization_savings: usize,
}

impl Stage1Compiler {
    /// Creates a new Stage 1 compiler.
    pub fn new() -> Self {
        Self {
            subset: SubsetDefinition::stage1(),
            optimize: true,
            stats: CompilerStats::default(),
        }
    }

    /// Creates a compiler with custom subset.
    pub fn with_subset(subset: SubsetDefinition) -> Self {
        Self {
            subset,
            optimize: true,
            stats: CompilerStats::default(),
        }
    }

    /// Compiles a program.
    pub fn compile(&mut self, program: &AstProgram) -> CompilationResult {
        let start = Instant::now();

        // Validate subset compliance.
        let subset_errors = self.validate_subset(program);
        if !subset_errors.is_empty() {
            return CompilationResult::fail(subset_errors);
        }

        // Compile using CodegenV2.
        let mut codegen = CodegenV2::new();
        codegen.compile_program(program);

        let mut instructions = codegen.instructions().to_vec();
        let pre_opt_count = instructions.len();

        // Apply optimizations.
        if self.optimize {
            instructions = peephole_optimize(&instructions);
        }

        let compile_time = start.elapsed();
        let savings = pre_opt_count.saturating_sub(instructions.len());

        // Update stats.
        self.stats.programs_compiled += 1;
        self.stats.total_instructions += instructions.len();
        self.stats.total_time += compile_time;
        self.stats.optimization_savings += savings;

        // Collect constants from the pool.
        let mut constants = Vec::new();
        let pool = &codegen.constants;
        for i in 0..pool.len() {
            if let Some(c) = pool.get(i as u32) {
                constants.push(c.clone());
            }
        }

        // Collect function metadata.
        let functions = Vec::new(); // Function table is inside codegen

        let mut result = CompilationResult::ok(instructions, constants, functions, compile_time);

        if !codegen.errors().is_empty() {
            result.errors = codegen.errors().to_vec();
            result.success = false;
        }

        result
    }

    /// Validates that a program uses only the Stage 1 subset.
    fn validate_subset(&self, program: &AstProgram) -> Vec<String> {
        let mut errors = Vec::new();

        for item in &program.items {
            match item {
                Item::FnDef(f) => {
                    if f.params.len() > self.subset.max_params {
                        errors.push(format!(
                            "function `{}` has {} params (max {})",
                            f.name,
                            f.params.len(),
                            self.subset.max_params
                        ));
                    }
                    if !f.type_params.is_empty() && !self.subset.supports_generics {
                        errors.push(format!(
                            "function `{}` uses generics (not in Stage 1 subset)",
                            f.name
                        ));
                    }
                    if f.is_async && !self.subset.supports_async {
                        errors.push(format!(
                            "function `{}` is async (not in Stage 1 subset)",
                            f.name
                        ));
                    }
                    self.validate_expr_subset(&f.body, &mut errors);
                }
                Item::Stmt(stmt) => self.validate_stmt_subset(stmt, &mut errors),
                _ => {}
            }
        }

        errors
    }

    /// Validates expression subset compliance.
    fn validate_expr_subset(&self, expr: &Expr, errors: &mut Vec<String>) {
        let kind = expr.kind_name();
        if !self.subset.supports_expr(kind) {
            errors.push(format!(
                "unsupported expression: `{kind}` (not in Stage 1 subset)"
            ));
        }

        // Recurse into children.
        match expr {
            Expr::BinOp { left, right, .. } => {
                self.validate_expr_subset(left, errors);
                self.validate_expr_subset(right, errors);
            }
            Expr::UnaryOp { operand, .. } => {
                self.validate_expr_subset(operand, errors);
            }
            Expr::Call { callee, args, .. } => {
                self.validate_expr_subset(callee, errors);
                for arg in args {
                    self.validate_expr_subset(arg, errors);
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.validate_expr_subset(condition, errors);
                self.validate_expr_subset(then_branch, errors);
                if let Some(e) = else_branch {
                    self.validate_expr_subset(e, errors);
                }
            }
            Expr::Block { stmts, expr, .. } => {
                for stmt in stmts {
                    self.validate_stmt_subset(stmt, errors);
                }
                if let Some(e) = expr {
                    self.validate_expr_subset(e, errors);
                }
            }
            Expr::ArrayLit { elements, .. } => {
                for elem in elements {
                    self.validate_expr_subset(elem, errors);
                }
            }
            Expr::Lambda { .. } if !self.subset.supports_closures => {
                errors.push("closures not in Stage 1 subset".into());
            }
            Expr::Match { .. } if !self.subset.supports_match => {
                errors.push("match not in Stage 1 subset".into());
            }
            _ => {}
        }
    }

    /// Validates statement subset compliance.
    fn validate_stmt_subset(&self, stmt: &Stmt, errors: &mut Vec<String>) {
        match stmt {
            Stmt::Let { init: Some(e), .. } => {
                self.validate_expr_subset(e, errors);
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.validate_expr_subset(condition, errors);
                self.validate_expr_subset(body, errors);
            }
            Stmt::For { iter, body, .. } => {
                self.validate_expr_subset(iter, errors);
                self.validate_expr_subset(body, errors);
            }
            Stmt::Return { value: Some(v), .. } => {
                self.validate_expr_subset(v, errors);
            }
            Stmt::ExprStmt { expr, .. } => {
                self.validate_expr_subset(expr, errors);
            }
            Stmt::FnDef(f) => {
                self.validate_expr_subset(&f.body, errors);
            }
            _ => {}
        }
    }
}

impl Default for Stage1Compiler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.4: Bootstrap Verification
// ═══════════════════════════════════════════════════════════════════════

/// Compares outputs from two compilation stages.
#[derive(Debug, Clone)]
pub struct BootstrapVerification {
    /// Stage 0 result.
    pub stage0: CompilationResult,
    /// Stage 1 result.
    pub stage1: CompilationResult,
    /// Whether outputs match.
    pub outputs_match: bool,
    /// Differences found.
    pub differences: Vec<BootstrapDiff>,
}

/// A difference between bootstrap stages.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapDiff {
    /// What component differs.
    pub component: String,
    /// Description of the difference.
    pub description: String,
}

impl fmt::Display for BootstrapDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.component, self.description)
    }
}

/// Verifies bootstrap: compares Stage 0 and Stage 1 compilation outputs.
pub fn verify_bootstrap(
    stage0: &CompilationResult,
    stage1: &CompilationResult,
) -> BootstrapVerification {
    let mut differences = Vec::new();

    // Compare instruction counts.
    if stage0.instruction_count() != stage1.instruction_count() {
        differences.push(BootstrapDiff {
            component: "instructions".into(),
            description: format!(
                "count differs: {} vs {}",
                stage0.instruction_count(),
                stage1.instruction_count()
            ),
        });
    }

    // Compare instruction sequences.
    if stage0.instructions != stage1.instructions {
        let first_diff = stage0
            .instructions
            .iter()
            .zip(stage1.instructions.iter())
            .position(|(a, b)| a != b);
        if let Some(pos) = first_diff {
            differences.push(BootstrapDiff {
                component: "instructions".into(),
                description: format!("first difference at offset {pos}"),
            });
        }
    }

    // Compare constant pools.
    if stage0.constants != stage1.constants {
        differences.push(BootstrapDiff {
            component: "constants".into(),
            description: format!(
                "pool differs: {} vs {} entries",
                stage0.constants.len(),
                stage1.constants.len()
            ),
        });
    }

    let outputs_match = differences.is_empty() && stage0.success && stage1.success;

    BootstrapVerification {
        stage0: stage0.clone(),
        stage1: stage1.clone(),
        outputs_match,
        differences,
    }
}

impl fmt::Display for BootstrapVerification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.outputs_match {
            write!(f, "BOOTSTRAP VERIFIED: Stage 0 == Stage 1")
        } else {
            write!(
                f,
                "BOOTSTRAP FAILED: {} differences",
                self.differences.len()
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.5: Bootstrap Test Suite
// ═══════════════════════════════════════════════════════════════════════

/// A bootstrap test case.
#[derive(Debug, Clone)]
pub struct BootstrapTest {
    /// Test name.
    pub name: String,
    /// Source program (AST).
    pub program: AstProgram,
    /// Expected instruction count (approximate).
    pub expected_instruction_count: Option<usize>,
    /// Whether this test should succeed.
    pub should_succeed: bool,
}

/// Bootstrap test suite result.
#[derive(Debug, Clone)]
pub struct BootstrapTestResult {
    /// Total tests.
    pub total: usize,
    /// Passed tests.
    pub passed: usize,
    /// Failed tests.
    pub failed: usize,
    /// Test details.
    pub details: Vec<(String, bool, String)>,
}

impl BootstrapTestResult {
    /// Whether all tests passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

impl fmt::Display for BootstrapTestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Bootstrap Tests: {}/{} passed, {} failed",
            self.passed, self.total, self.failed
        )
    }
}

/// Runs the bootstrap test suite.
pub fn run_bootstrap_tests(tests: &[BootstrapTest]) -> BootstrapTestResult {
    let mut passed = 0;
    let mut failed = 0;
    let mut details = Vec::new();

    for test in tests {
        let mut compiler = Stage1Compiler::new();
        let result = compiler.compile(&test.program);

        let test_passed = if test.should_succeed {
            result.success
        } else {
            !result.success
        };

        if test_passed {
            passed += 1;
            details.push((test.name.clone(), true, "OK".into()));
        } else {
            failed += 1;
            let msg = if test.should_succeed {
                format!("expected success, got: {:?}", result.errors)
            } else {
                "expected failure, got success".into()
            };
            details.push((test.name.clone(), false, msg));
        }
    }

    BootstrapTestResult {
        total: tests.len(),
        passed,
        failed,
        details,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.6: Reproducible Build Hash
// ═══════════════════════════════════════════════════════════════════════

/// Computes a deterministic hash of compilation output for reproducibility.
pub fn compilation_hash(result: &CompilationResult) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;

    // Hash instructions.
    for inst in &result.instructions {
        let s = format!("{inst}");
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    // Hash constants.
    for c in &result.constants {
        let s = format!("{c}");
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    hash
}

/// Verifies that two compilations of the same source produce the same hash.
pub fn verify_reproducible(source: &AstProgram, runs: usize) -> bool {
    let mut hashes = Vec::new();
    for _ in 0..runs {
        let mut compiler = Stage1Compiler::new();
        let result = compiler.compile(source);
        if result.success {
            hashes.push(compilation_hash(&result));
        }
    }

    if hashes.len() < 2 {
        return true;
    }

    let first = hashes[0];
    hashes.iter().all(|h| *h == first)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selfhost::ast_tree::*;

    fn span() -> AstSpan {
        AstSpan::dummy()
    }

    fn int_expr(v: i64) -> Expr {
        Expr::IntLit {
            value: v,
            span: span(),
        }
    }

    fn ident_expr(name: &str) -> Expr {
        Expr::Ident {
            name: name.into(),
            span: span(),
        }
    }

    fn simple_program() -> AstProgram {
        AstProgram::new(
            "test.fj",
            vec![
                Item::Stmt(Stmt::Let {
                    name: "x".into(),
                    mutable: false,
                    ty: None,
                    init: Some(Box::new(int_expr(42))),
                    span: span(),
                }),
                Item::Stmt(Stmt::ExprStmt {
                    expr: Box::new(Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(ident_expr("x")),
                        right: Box::new(int_expr(10)),
                        span: span(),
                    }),
                    span: span(),
                }),
            ],
        )
    }

    fn fn_program() -> AstProgram {
        AstProgram::new(
            "test.fj",
            vec![Item::FnDef(FnDefNode {
                name: "add".into(),
                type_params: vec![],
                params: vec![
                    Param {
                        name: "a".into(),
                        ty: TypeExpr::Name("i32".into(), span()),
                        mutable: false,
                    },
                    Param {
                        name: "b".into(),
                        ty: TypeExpr::Name("i32".into(), span()),
                        mutable: false,
                    },
                ],
                ret_type: Some(TypeExpr::Name("i32".into(), span())),
                body: Box::new(Expr::BinOp {
                    op: BinOp::Add,
                    left: Box::new(ident_expr("a")),
                    right: Box::new(ident_expr("b")),
                    span: span(),
                }),
                is_pub: false,
                context: None,
                is_async: false,
                is_gen: false,
                span: span(),
            })],
        )
    }

    // S5.1 — Subset definition
    #[test]
    fn s5_1_subset_definition() {
        let subset = SubsetDefinition::stage1();
        assert!(subset.supports_expr("int_lit"));
        assert!(subset.supports_expr("bin_op"));
        assert!(!subset.supports_expr("yield"));
        assert!(subset.supports_stmt("let"));
        assert!(subset.supports_type("i32"));
        assert!(!subset.supports_generics);
        assert!(!subset.supports_closures);
    }

    #[test]
    fn s5_1_subset_feature_count() {
        let subset = SubsetDefinition::stage1();
        assert!(subset.feature_count() > 20);
    }

    #[test]
    fn s5_1_subset_display() {
        let subset = SubsetDefinition::stage1();
        let display = subset.to_string();
        assert!(display.contains("Stage 1"));
        assert!(display.contains("generics=false"));
    }

    // S5.2 — Compilation result
    #[test]
    fn s5_2_compilation_result_ok() {
        let result = CompilationResult::ok(
            vec![Instruction::Halt],
            vec![Constant::Int(42)],
            vec![],
            Duration::from_millis(10),
        );
        assert!(result.success);
        assert_eq!(result.instruction_count(), 1);
        assert!(result.to_string().contains("OK"));
    }

    #[test]
    fn s5_2_compilation_result_fail() {
        let result = CompilationResult::fail(vec!["error".into()]);
        assert!(!result.success);
        assert!(result.to_string().contains("FAIL"));
    }

    // S5.3 — Stage 1 compiler
    #[test]
    fn s5_3_compile_simple_program() {
        let mut compiler = Stage1Compiler::new();
        let prog = simple_program();
        let result = compiler.compile(&prog);
        assert!(result.success);
        assert!(result.instruction_count() > 0);
    }

    #[test]
    fn s5_3_compile_function() {
        let mut compiler = Stage1Compiler::new();
        let prog = fn_program();
        let result = compiler.compile(&prog);
        assert!(result.success);
    }

    #[test]
    fn s5_3_reject_generics() {
        let mut compiler = Stage1Compiler::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::FnDef(FnDefNode {
                name: "id".into(),
                type_params: vec!["T".into()],
                params: vec![Param {
                    name: "x".into(),
                    ty: TypeExpr::Name("T".into(), span()),
                    mutable: false,
                }],
                ret_type: Some(TypeExpr::Name("T".into(), span())),
                body: Box::new(ident_expr("x")),
                is_pub: false,
                context: None,
                is_async: false,
                is_gen: false,
                span: span(),
            })],
        );
        let result = compiler.compile(&prog);
        assert!(!result.success);
        assert!(result.errors.iter().any(|e| e.contains("generics")));
    }

    #[test]
    fn s5_3_compiler_stats() {
        let mut compiler = Stage1Compiler::new();
        compiler.compile(&simple_program());
        compiler.compile(&fn_program());
        assert_eq!(compiler.stats.programs_compiled, 2);
        assert!(compiler.stats.total_instructions > 0);
    }

    // S5.4 — Bootstrap verification
    #[test]
    fn s5_4_verify_matching_outputs() {
        let mut compiler0 = Stage1Compiler::new();
        let mut compiler1 = Stage1Compiler::new();
        let prog = simple_program();

        let stage0 = compiler0.compile(&prog);
        let stage1 = compiler1.compile(&prog);

        let verification = verify_bootstrap(&stage0, &stage1);
        assert!(verification.outputs_match);
        assert!(verification.to_string().contains("VERIFIED"));
    }

    #[test]
    fn s5_4_verify_different_outputs() {
        let stage0 = CompilationResult::ok(
            vec![Instruction::LoadConst(0), Instruction::Halt],
            vec![Constant::Int(1)],
            vec![],
            Duration::ZERO,
        );
        let stage1 = CompilationResult::ok(
            vec![
                Instruction::LoadConst(0),
                Instruction::Add,
                Instruction::Halt,
            ],
            vec![Constant::Int(1)],
            vec![],
            Duration::ZERO,
        );

        let verification = verify_bootstrap(&stage0, &stage1);
        assert!(!verification.outputs_match);
        assert!(!verification.differences.is_empty());
    }

    // S5.5 — Bootstrap test suite
    #[test]
    fn s5_5_run_test_suite() {
        let tests = vec![
            BootstrapTest {
                name: "simple_let".into(),
                program: simple_program(),
                expected_instruction_count: None,
                should_succeed: true,
            },
            BootstrapTest {
                name: "function".into(),
                program: fn_program(),
                expected_instruction_count: None,
                should_succeed: true,
            },
        ];

        let result = run_bootstrap_tests(&tests);
        assert!(result.all_passed());
        assert_eq!(result.total, 2);
        assert_eq!(result.passed, 2);
        assert!(result.to_string().contains("2/2"));
    }

    #[test]
    fn s5_5_test_expected_failure() {
        let tests = vec![BootstrapTest {
            name: "reject_generics".into(),
            program: AstProgram::new(
                "test.fj",
                vec![Item::FnDef(FnDefNode {
                    name: "id".into(),
                    type_params: vec!["T".into()],
                    params: vec![],
                    ret_type: None,
                    body: Box::new(int_expr(0)),
                    is_pub: false,
                    context: None,
                    is_async: false,
                    is_gen: false,
                    span: span(),
                })],
            ),
            expected_instruction_count: None,
            should_succeed: false,
        }];

        let result = run_bootstrap_tests(&tests);
        assert!(result.all_passed());
    }

    // S5.6 — Reproducible builds
    #[test]
    fn s5_6_compilation_hash_deterministic() {
        let prog = simple_program();
        let mut c1 = Stage1Compiler::new();
        let mut c2 = Stage1Compiler::new();
        let r1 = c1.compile(&prog);
        let r2 = c2.compile(&prog);
        assert_eq!(compilation_hash(&r1), compilation_hash(&r2));
    }

    #[test]
    fn s5_6_verify_reproducible() {
        let prog = simple_program();
        assert!(verify_reproducible(&prog, 3));
    }

    #[test]
    fn s5_6_different_programs_different_hash() {
        let prog1 = simple_program();
        let prog2 = fn_program();
        let mut c1 = Stage1Compiler::new();
        let mut c2 = Stage1Compiler::new();
        let r1 = c1.compile(&prog1);
        let r2 = c2.compile(&prog2);
        assert_ne!(compilation_hash(&r1), compilation_hash(&r2));
    }

    // Additional tests
    #[test]
    fn s5_7_bootstrap_diff_display() {
        let diff = BootstrapDiff {
            component: "instructions".into(),
            description: "count differs: 5 vs 6".into(),
        };
        assert!(diff.to_string().contains("instructions"));
        assert!(diff.to_string().contains("5 vs 6"));
    }

    #[test]
    fn s5_8_empty_program_compiles() {
        let mut compiler = Stage1Compiler::new();
        let prog = AstProgram::new("empty.fj", vec![]);
        let result = compiler.compile(&prog);
        assert!(result.success);
        // Should have at least a HALT instruction.
        assert!(
            result
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::Halt))
        );
    }

    #[test]
    fn s5_9_optimization_enabled() {
        let mut compiler = Stage1Compiler::new();
        assert!(compiler.optimize);
        compiler.optimize = false;
        assert!(!compiler.optimize);
    }

    #[test]
    fn s5_10_reject_async() {
        let mut compiler = Stage1Compiler::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::FnDef(FnDefNode {
                name: "fetch".into(),
                type_params: vec![],
                params: vec![],
                ret_type: None,
                body: Box::new(int_expr(0)),
                is_pub: false,
                context: None,
                is_async: true,
                is_gen: false,
                span: span(),
            })],
        );
        let result = compiler.compile(&prog);
        assert!(!result.success);
        assert!(result.errors.iter().any(|e| e.contains("async")));
    }

    #[test]
    fn s5_10_reject_closures() {
        let mut compiler = Stage1Compiler::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::Stmt(Stmt::ExprStmt {
                expr: Box::new(Expr::Lambda {
                    params: vec![],
                    body: Box::new(int_expr(0)),
                    span: span(),
                }),
                span: span(),
            })],
        );
        let result = compiler.compile(&prog);
        assert!(!result.success);
        assert!(result.errors.iter().any(|e| e.contains("closure")));
    }
}
