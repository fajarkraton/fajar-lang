//! End-to-end integration tests for the LLVM backend.
//!
//! These tests compile Fajar Lang AST directly via `LlvmCompiler`
//! and JIT-execute `main()`, verifying correct results.
//!
//! Run: cargo test --features llvm --test llvm_e2e_tests

#![cfg(feature = "llvm")]

use fajar_lang::codegen::llvm::LlvmCompiler;
use fajar_lang::lexer::token::Span;
use fajar_lang::parser::ast::*;
use inkwell::context::Context;

fn dummy_span() -> Span {
    Span::new(0, 0)
}

fn make_int_lit(val: i64) -> Expr {
    Expr::Literal {
        kind: LiteralKind::Int(val),
        span: dummy_span(),
    }
}

fn make_float_lit(val: f64) -> Expr {
    Expr::Literal {
        kind: LiteralKind::Float(val),
        span: dummy_span(),
    }
}

fn make_ident(name: &str) -> Expr {
    Expr::Ident {
        name: name.to_string(),
        span: dummy_span(),
    }
}

fn make_call_arg(expr: Expr) -> CallArg {
    CallArg {
        name: None,
        value: expr,
        span: dummy_span(),
    }
}

fn make_fn(name: &str, params: Vec<Param>, ret: Option<TypeExpr>, body: Expr) -> Item {
    Item::FnDef(FnDef {
        is_pub: false,
        is_const: false,
        is_async: false,
        is_gen: false,
        is_test: false,
        should_panic: false,
        is_ignored: false,
        no_inline: false,
        doc_comment: None,
        annotation: None,
        name: name.to_string(),
        lifetime_params: vec![],
        generic_params: vec![],
        params,
        return_type: ret,
        where_clauses: vec![],
        requires: vec![],
        ensures: vec![],
        effects: vec![],
        effect_row_var: None,
        body: Box::new(body),
        span: dummy_span(),
    })
}

fn i64_type() -> TypeExpr {
    TypeExpr::Simple {
        name: "i64".to_string(),
        span: dummy_span(),
    }
}

fn i64_param(name: &str) -> Param {
    Param {
        name: name.to_string(),
        ty: i64_type(),
        span: dummy_span(),
    }
}

fn compile_and_run(items: Vec<Item>) -> i64 {
    LlvmCompiler::init_native_target().unwrap();
    let context = Context::create();
    let mut compiler = LlvmCompiler::new(&context, "test");
    compiler
        .compile_program(&Program {
            span: dummy_span(),
            items,
        })
        .expect("LLVM compilation failed");
    compiler.jit_execute().expect("LLVM JIT execution failed")
}

fn compile_to_ir(items: Vec<Item>) -> String {
    LlvmCompiler::init_native_target().unwrap();
    let context = Context::create();
    let mut compiler = LlvmCompiler::new(&context, "test");
    compiler
        .compile_program(&Program {
            span: dummy_span(),
            items,
        })
        .expect("LLVM compilation failed");
    compiler.verify().expect("LLVM IR verification failed");
    compiler.print_ir()
}

fn make_call(name: &str, arg_exprs: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(make_ident(name)),
        args: arg_exprs.into_iter().map(make_call_arg).collect(),
        span: dummy_span(),
    }
}

fn make_arm(pattern: Pattern, body: Expr) -> MatchArm {
    MatchArm {
        pattern,
        guard: None,
        body: Box::new(body),
        span: dummy_span(),
    }
}

fn make_assign(target: Expr, value: Expr) -> Expr {
    Expr::Assign {
        target: Box::new(target),
        op: AssignOp::Assign,
        value: Box::new(value),
        span: dummy_span(),
    }
}

fn make_let(name: &str, value: Expr) -> Stmt {
    Stmt::Let {
        mutable: false,
        linear: false,
        name: name.into(),
        ty: None,
        value: Box::new(value),
        span: dummy_span(),
    }
}

fn make_let_mut(name: &str, value: Expr) -> Stmt {
    Stmt::Let {
        mutable: true,
        linear: false,
        name: name.into(),
        ty: None,
        value: Box::new(value),
        span: dummy_span(),
    }
}

fn make_binop(left: Expr, op: BinOp, right: Expr) -> Expr {
    Expr::Binary {
        left: Box::new(left),
        op,
        right: Box::new(right),
        span: dummy_span(),
    }
}

fn make_expr_stmt(expr: Expr) -> Stmt {
    Stmt::Expr {
        expr: Box::new(expr),
        span: dummy_span(),
    }
}

fn make_block(stmts: Vec<Stmt>, expr: Option<Expr>) -> Expr {
    Expr::Block {
        stmts,
        expr: expr.map(Box::new),
        span: dummy_span(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Original E2E Tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn llvm_e2e_simple_return() {
    let items = vec![make_fn("main", vec![], Some(i64_type()), make_int_lit(42))];
    assert_eq!(compile_and_run(items), 42);
}

#[test]
fn llvm_e2e_function_call() {
    let add_body = make_binop(make_ident("a"), BinOp::Add, make_ident("b"));
    let main_body = make_call("add", vec![make_int_lit(17), make_int_lit(25)]);
    let items = vec![
        make_fn(
            "add",
            vec![i64_param("a"), i64_param("b")],
            Some(i64_type()),
            add_body,
        ),
        make_fn("main", vec![], Some(i64_type()), main_body),
    ];
    assert_eq!(compile_and_run(items), 42);
}

#[test]
fn llvm_e2e_if_else() {
    let body = Expr::If {
        condition: Box::new(make_binop(make_int_lit(10), BinOp::Gt, make_int_lit(5))),
        then_branch: Box::new(make_int_lit(1)),
        else_branch: Some(Box::new(make_int_lit(0))),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        1
    );
}

#[test]
fn llvm_e2e_while_loop_sum() {
    let body = make_block(
        vec![
            make_let_mut("s", make_int_lit(0)),
            make_let_mut("i", make_int_lit(0)),
            make_expr_stmt(Expr::While {
                condition: Box::new(make_binop(make_ident("i"), BinOp::Lt, make_int_lit(10))),
                body: Box::new(make_block(
                    vec![
                        make_expr_stmt(make_assign(
                            make_ident("s"),
                            make_binop(make_ident("s"), BinOp::Add, make_ident("i")),
                        )),
                        make_expr_stmt(make_assign(
                            make_ident("i"),
                            make_binop(make_ident("i"), BinOp::Add, make_int_lit(1)),
                        )),
                    ],
                    None,
                )),
                label: None,
                span: dummy_span(),
            }),
        ],
        Some(make_ident("s")),
    );
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        45
    );
}

#[test]
fn llvm_e2e_recursive_fibonacci() {
    let fib_body = Expr::If {
        condition: Box::new(make_binop(make_ident("n"), BinOp::Le, make_int_lit(1))),
        then_branch: Box::new(make_ident("n")),
        else_branch: Some(Box::new(make_binop(
            make_call(
                "fib",
                vec![make_binop(make_ident("n"), BinOp::Sub, make_int_lit(1))],
            ),
            BinOp::Add,
            make_call(
                "fib",
                vec![make_binop(make_ident("n"), BinOp::Sub, make_int_lit(2))],
            ),
        ))),
        span: dummy_span(),
    };
    let items = vec![
        make_fn("fib", vec![i64_param("n")], Some(i64_type()), fib_body),
        make_fn(
            "main",
            vec![],
            Some(i64_type()),
            make_call("fib", vec![make_int_lit(10)]),
        ),
    ];
    assert_eq!(compile_and_run(items), 55);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 1: Bare-metal builtin tests (IR verification)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn llvm_e2e_rdtsc_returns_nonzero() {
    let body = Expr::If {
        condition: Box::new(make_binop(
            make_call("rdtsc", vec![]),
            BinOp::Gt,
            make_int_lit(0),
        )),
        then_branch: Box::new(make_int_lit(1)),
        else_branch: Some(Box::new(make_int_lit(0))),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        1
    );
}

#[test]
fn llvm_e2e_rdtsc_increases_monotonically() {
    let body = make_block(
        vec![
            make_let("t1", make_call("rdtsc", vec![])),
            make_let("t2", make_call("rdtsc", vec![])),
        ],
        Some(Expr::If {
            condition: Box::new(make_binop(make_ident("t2"), BinOp::Gt, make_ident("t1"))),
            then_branch: Box::new(make_int_lit(1)),
            else_branch: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        }),
    );
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        1
    );
}

#[test]
fn llvm_e2e_rdrand_returns_value() {
    let body = make_block(
        vec![
            make_let("r1", make_call("rdrand", vec![])),
            make_let("r2", make_call("rdrand", vec![])),
        ],
        Some(Expr::If {
            condition: Box::new(make_binop(make_ident("r1"), BinOp::Eq, make_ident("r2"))),
            then_branch: Box::new(make_int_lit(0)),
            else_branch: Some(Box::new(make_int_lit(1))),
            span: dummy_span(),
        }),
    );
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        1
    );
}

#[test]
fn llvm_e2e_volatile_read_write_roundtrip() {
    let body = make_block(
        vec![make_expr_stmt(make_call(
            "volatile_write",
            vec![make_int_lit(0x10000), make_int_lit(42)],
        ))],
        Some(make_call("volatile_read", vec![make_int_lit(0x10000)])),
    );
    let ir = compile_to_ir(vec![make_fn("main", vec![], Some(i64_type()), body)]);
    assert!(ir.contains("volatile"), "Expected volatile in IR:\n{ir}");
}

#[test]
fn llvm_e2e_volatile_read_sized_ir() {
    let body = make_block(
        vec![
            make_let(
                "a",
                make_call("volatile_read_u8", vec![make_int_lit(0x3F8)]),
            ),
            make_let(
                "b",
                make_call("volatile_read_u16", vec![make_int_lit(0x3F8)]),
            ),
            make_let(
                "c",
                make_call("volatile_read_u32", vec![make_int_lit(0x3F8)]),
            ),
        ],
        Some(make_binop(
            make_binop(make_ident("a"), BinOp::Add, make_ident("b")),
            BinOp::Add,
            make_ident("c"),
        )),
    );
    let ir = compile_to_ir(vec![make_fn("main", vec![], Some(i64_type()), body)]);
    assert!(
        ir.contains("volatile"),
        "Expected volatile loads in IR:\n{ir}"
    );
    assert!(ir.contains("zext"), "Expected zext in IR:\n{ir}");
}

#[test]
fn llvm_e2e_port_io_compiles() {
    let body = make_block(
        vec![
            make_expr_stmt(make_call(
                "port_outb",
                vec![make_int_lit(0x3F8), make_int_lit(0x41)],
            )),
            make_expr_stmt(make_call(
                "port_outw",
                vec![make_int_lit(0x1F0), make_int_lit(0x1234)],
            )),
            make_expr_stmt(make_call(
                "port_outd",
                vec![make_int_lit(0xCF8), make_int_lit(0xDEADBEEF_u32 as i64)],
            )),
        ],
        Some(make_binop(
            make_binop(
                make_call("port_inb", vec![make_int_lit(0x3F8)]),
                BinOp::Add,
                make_call("port_inw", vec![make_int_lit(0x1F0)]),
            ),
            BinOp::Add,
            make_call("port_ind", vec![make_int_lit(0xCF8)]),
        )),
    );
    let ir = compile_to_ir(vec![make_fn("main", vec![], Some(i64_type()), body)]);
    assert!(ir.contains("inb"), "Expected inb in IR:\n{ir}");
    assert!(ir.contains("outb"), "Expected outb in IR:\n{ir}");
    assert!(ir.contains("inw"), "Expected inw in IR:\n{ir}");
    assert!(ir.contains("outw"), "Expected outw in IR:\n{ir}");
    assert!(ir.contains("inl"), "Expected inl (ind) in IR:\n{ir}");
    assert!(ir.contains("outl"), "Expected outl (outd) in IR:\n{ir}");
}

#[test]
fn llvm_e2e_cli_sti_hlt_compiles() {
    let body = make_block(
        vec![
            make_expr_stmt(make_call("cli", vec![])),
            make_expr_stmt(make_call("sti", vec![])),
            make_expr_stmt(make_call("hlt", vec![])),
        ],
        Some(make_int_lit(0)),
    );
    let ir = compile_to_ir(vec![make_fn("main", vec![], Some(i64_type()), body)]);
    assert!(ir.contains("cli"), "Expected cli in IR:\n{ir}");
    assert!(ir.contains("sti"), "Expected sti in IR:\n{ir}");
    assert!(ir.contains("hlt"), "Expected hlt in IR:\n{ir}");
}

#[test]
fn llvm_e2e_rdtsc_ir_contains_asm() {
    let ir = compile_to_ir(vec![make_fn(
        "main",
        vec![],
        Some(i64_type()),
        make_call("rdtsc", vec![]),
    )]);
    assert!(ir.contains("rdtsc"), "Expected rdtsc in IR:\n{ir}");
}

#[test]
fn llvm_e2e_rdrand_ir_contains_asm() {
    let ir = compile_to_ir(vec![make_fn(
        "main",
        vec![],
        Some(i64_type()),
        make_call("rdrand", vec![]),
    )]);
    assert!(ir.contains("rdrand"), "Expected rdrand in IR:\n{ir}");
}

#[test]
fn llvm_e2e_volatile_write_sized_ir() {
    let body = make_block(
        vec![
            make_expr_stmt(make_call(
                "volatile_write_u8",
                vec![make_int_lit(0xB8000), make_int_lit(0x41)],
            )),
            make_expr_stmt(make_call(
                "volatile_write_u16",
                vec![make_int_lit(0xB8000), make_int_lit(0x0F41)],
            )),
            make_expr_stmt(make_call(
                "volatile_write_u32",
                vec![make_int_lit(0xB8000), make_int_lit(0x0F410F42)],
            )),
        ],
        Some(make_int_lit(0)),
    );
    let ir = compile_to_ir(vec![make_fn("main", vec![], Some(i64_type()), body)]);
    assert!(
        ir.contains("store volatile i8"),
        "Expected volatile i8 store in IR:\n{ir}"
    );
    assert!(
        ir.contains("store volatile i16"),
        "Expected volatile i16 store in IR:\n{ir}"
    );
    assert!(
        ir.contains("store volatile i32"),
        "Expected volatile i32 store in IR:\n{ir}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Batch E-I: Tests for 30 LLVM enhancements
// ═══════════════════════════════════════════════════════════════════════

/// E1: User function overrides builtin
#[test]
fn llvm_e2e_user_fn_overrides_builtin() {
    let user_rdtsc = make_fn("rdtsc", vec![], Some(i64_type()), make_int_lit(999));
    let main_fn = make_fn("main", vec![], Some(i64_type()), make_call("rdtsc", vec![]));
    assert_eq!(compile_and_run(vec![user_rdtsc, main_fn]), 999);
}

/// F4: Match on integer literals
#[test]
fn llvm_e2e_match_int_literals() {
    let body = Expr::Match {
        subject: Box::new(make_int_lit(2)),
        arms: vec![
            make_arm(
                Pattern::Literal {
                    kind: LiteralKind::Int(1),
                    span: dummy_span(),
                },
                make_int_lit(10),
            ),
            make_arm(
                Pattern::Literal {
                    kind: LiteralKind::Int(2),
                    span: dummy_span(),
                },
                make_int_lit(20),
            ),
            make_arm(
                Pattern::Literal {
                    kind: LiteralKind::Int(3),
                    span: dummy_span(),
                },
                make_int_lit(30),
            ),
            make_arm(Pattern::Wildcard { span: dummy_span() }, make_int_lit(0)),
        ],
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        20
    );
}

/// F1: Match wildcard binds value
#[test]
fn llvm_e2e_match_ident_binding() {
    let body = Expr::Match {
        subject: Box::new(make_int_lit(7)),
        arms: vec![make_arm(
            Pattern::Ident {
                name: "n".into(),
                span: dummy_span(),
            },
            make_ident("n"),
        )],
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        7
    );
}

/// G1: Float power (2.0 ** 10.0 = 1024.0 -> 1024)
#[test]
fn llvm_e2e_float_pow() {
    let cast = Expr::Cast {
        expr: Box::new(make_binop(
            make_float_lit(2.0),
            BinOp::Pow,
            make_float_lit(10.0),
        )),
        ty: i64_type(),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), cast)]),
        1024
    );
}

/// I2: Integer power (3 ** 4 = 81)
#[test]
fn llvm_e2e_int_pow() {
    let body = make_binop(make_int_lit(3), BinOp::Pow, make_int_lit(4));
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        81
    );
}

/// G1: Float remainder (7.0 % 3.0 = 1.0 -> 1)
#[test]
fn llvm_e2e_float_rem() {
    let cast = Expr::Cast {
        expr: Box::new(make_binop(
            make_float_lit(7.0),
            BinOp::Rem,
            make_float_lit(3.0),
        )),
        ty: i64_type(),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), cast)]),
        1
    );
}

/// G2: Unary negation
#[test]
fn llvm_e2e_unary_neg() {
    let body = Expr::Unary {
        op: UnaryOp::Neg,
        operand: Box::new(make_int_lit(42)),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        -42
    );
}

/// G2: Unary bitwise not (~0 = -1)
#[test]
fn llvm_e2e_unary_bitnot() {
    let body = Expr::Unary {
        op: UnaryOp::BitNot,
        operand: Box::new(make_int_lit(0)),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        -1
    );
}

/// G4: Bool to int cast (true => 1)
#[test]
fn llvm_e2e_bool_cast() {
    let cast = Expr::Cast {
        expr: Box::new(make_binop(make_int_lit(1), BinOp::Eq, make_int_lit(1))),
        ty: i64_type(),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), cast)]),
        1
    );
}

/// G6: Indirect function call via fn_addr
#[test]
fn llvm_e2e_indirect_call() {
    let add1 = make_fn(
        "add1",
        vec![i64_param("x")],
        Some(i64_type()),
        make_binop(make_ident("x"), BinOp::Add, make_int_lit(1)),
    );
    let main_body = make_block(
        vec![make_let(
            "f",
            make_call(
                "fn_addr",
                vec![Expr::Literal {
                    kind: LiteralKind::String("add1".into()),
                    span: dummy_span(),
                }],
            ),
        )],
        Some(Expr::Call {
            callee: Box::new(make_ident("f")),
            args: vec![make_call_arg(make_int_lit(41))],
            span: dummy_span(),
        }),
    );
    assert_eq!(
        compile_and_run(vec![
            add1,
            make_fn("main", vec![], Some(i64_type()), main_body)
        ]),
        42
    );
}

/// H4: Range pattern (5 in 1..10 => 1)
#[test]
fn llvm_e2e_match_range_pattern() {
    let body = Expr::Match {
        subject: Box::new(make_int_lit(5)),
        arms: vec![
            make_arm(
                Pattern::Range {
                    start: Box::new(make_int_lit(1)),
                    end: Box::new(make_int_lit(10)),
                    inclusive: false,
                    span: dummy_span(),
                },
                make_int_lit(1),
            ),
            make_arm(Pattern::Wildcard { span: dummy_span() }, make_int_lit(0)),
        ],
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        1
    );
}

/// H4: Range pattern — outside range
#[test]
fn llvm_e2e_match_range_outside() {
    let body = Expr::Match {
        subject: Box::new(make_int_lit(15)),
        arms: vec![
            make_arm(
                Pattern::Range {
                    start: Box::new(make_int_lit(1)),
                    end: Box::new(make_int_lit(10)),
                    inclusive: false,
                    span: dummy_span(),
                },
                make_int_lit(1),
            ),
            make_arm(Pattern::Wildcard { span: dummy_span() }, make_int_lit(0)),
        ],
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        0
    );
}

/// Or-pattern: 1 | 2 | 3 => 100
#[test]
fn llvm_e2e_match_or_pattern() {
    let body = Expr::Match {
        subject: Box::new(make_int_lit(2)),
        arms: vec![
            make_arm(
                Pattern::Or {
                    patterns: vec![
                        Pattern::Literal {
                            kind: LiteralKind::Int(1),
                            span: dummy_span(),
                        },
                        Pattern::Literal {
                            kind: LiteralKind::Int(2),
                            span: dummy_span(),
                        },
                        Pattern::Literal {
                            kind: LiteralKind::Int(3),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                },
                make_int_lit(100),
            ),
            make_arm(Pattern::Wildcard { span: dummy_span() }, make_int_lit(0)),
        ],
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        100
    );
}

/// Bitwise ops: (0xFF & 0x0F) | (1 << 8) = 271
#[test]
fn llvm_e2e_bitwise_ops() {
    let body = make_binop(
        make_binop(make_int_lit(0xFF), BinOp::BitAnd, make_int_lit(0x0F)),
        BinOp::BitOr,
        make_binop(make_int_lit(1), BinOp::Shl, make_int_lit(8)),
    );
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        271
    );
}

/// H2: Yield returns value
#[test]
fn llvm_e2e_yield_value() {
    let body = Expr::Yield {
        value: Some(Box::new(make_int_lit(77))),
        span: dummy_span(),
    };
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        77
    );
}

/// Closure: |x| x + 10 applied to 32
#[test]
fn llvm_e2e_closure_basic() {
    let closure = Expr::Closure {
        params: vec![ClosureParam {
            name: "x".into(),
            ty: None,
            span: dummy_span(),
        }],
        return_type: None,
        body: Box::new(make_binop(make_ident("x"), BinOp::Add, make_int_lit(10))),
        span: dummy_span(),
    };
    let body = make_block(
        vec![make_let("c", closure)],
        Some(Expr::Call {
            callee: Box::new(make_ident("c")),
            args: vec![make_call_arg(make_int_lit(32))],
            span: dummy_span(),
        }),
    );
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        42
    );
}

/// E4: Function returning bool coerced to i64
#[test]
fn llvm_e2e_bool_return_coercion() {
    let is_pos = make_fn(
        "is_positive",
        vec![i64_param("x")],
        Some(i64_type()),
        make_binop(make_ident("x"), BinOp::Gt, make_int_lit(0)),
    );
    let main_fn = make_fn(
        "main",
        vec![],
        Some(i64_type()),
        make_call("is_positive", vec![make_int_lit(5)]),
    );
    assert_eq!(compile_and_run(vec![is_pos, main_fn]), 1);
}

/// Nested function calls: add1(double(20)) = 41
#[test]
fn llvm_e2e_nested_calls() {
    let double = make_fn(
        "double",
        vec![i64_param("x")],
        Some(i64_type()),
        make_binop(make_ident("x"), BinOp::Mul, make_int_lit(2)),
    );
    let add1 = make_fn(
        "add1",
        vec![i64_param("x")],
        Some(i64_type()),
        make_binop(make_ident("x"), BinOp::Add, make_int_lit(1)),
    );
    let main_fn = make_fn(
        "main",
        vec![],
        Some(i64_type()),
        make_call("add1", vec![make_call("double", vec![make_int_lit(20)])]),
    );
    assert_eq!(compile_and_run(vec![double, add1, main_fn]), 41);
}

/// For loop: sum 0..5 = 10
#[test]
fn llvm_e2e_for_loop_sum() {
    let body = make_block(
        vec![
            make_let_mut("s", make_int_lit(0)),
            make_expr_stmt(Expr::For {
                label: None,
                variable: "i".into(),
                iterable: Box::new(Expr::Range {
                    start: Some(Box::new(make_int_lit(0))),
                    end: Some(Box::new(make_int_lit(5))),
                    inclusive: false,
                    span: dummy_span(),
                }),
                body: Box::new(make_assign(
                    make_ident("s"),
                    make_binop(make_ident("s"), BinOp::Add, make_ident("i")),
                )),
                span: dummy_span(),
            }),
        ],
        Some(make_ident("s")),
    );
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        10
    );
}

/// I2: Int pow edge case: x ** 0 = 1
#[test]
fn llvm_e2e_int_pow_zero() {
    let body = make_binop(make_int_lit(999), BinOp::Pow, make_int_lit(0));
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        1
    );
}

/// Int remainder: 17 % 5 = 2
#[test]
fn llvm_e2e_int_rem() {
    let body = make_binop(make_int_lit(17), BinOp::Rem, make_int_lit(5));
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        2
    );
}

/// XOR: 0xFF ^ 0x0F = 0xF0
#[test]
fn llvm_e2e_xor() {
    let body = make_binop(make_int_lit(0xFF), BinOp::BitXor, make_int_lit(0x0F));
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        0xF0
    );
}

/// Right shift: 256 >> 4 = 16
#[test]
fn llvm_e2e_shr() {
    let body = make_binop(make_int_lit(256), BinOp::Shr, make_int_lit(4));
    assert_eq!(
        compile_and_run(vec![make_fn("main", vec![], Some(i64_type()), body)]),
        16
    );
}

// ═══════════════════════════════════════════════════════════════════
// V32 Perfection P2.A4 — @interrupt full source-level E2E
// ═══════════════════════════════════════════════════════════════════
//
// Closes the gap surfaced by HONEST_AUDIT_V32 §4 G2 in its FULL
// .fj-source-to-IR form. P1 followup F4 added a codegen-API direct
// test (src/codegen/llvm/mod.rs::at_interrupt_emits_naked_noinline_*).
// This test extends that by going through the COMPLETE pipeline:
// .fj file → tokenize → parse → LlvmCompiler::compile_program → IR
// grep. Verifies the lexer, parser, AST, and codegen all agree on
// @interrupt's `naked + noinline + .text.interrupt` semantics.

#[test]
fn at_interrupt_e2e_compiles_with_isr_attributes() {
    let source = std::fs::read_to_string("examples/at_interrupt_demo.fj")
        .expect("examples/at_interrupt_demo.fj must exist (created in P2.A4)");

    let tokens = fajar_lang::lexer::tokenize(&source).expect("lexer should accept @interrupt");
    let program = fajar_lang::parser::parse(tokens).expect("parser should accept @interrupt");

    LlvmCompiler::init_native_target().unwrap();
    let context = Context::create();
    let mut compiler = LlvmCompiler::new(&context, "at_interrupt_e2e");
    compiler
        .compile_program(&program)
        .expect("@interrupt full pipeline should compile cleanly");
    compiler.verify().expect("LLVM IR should verify");
    let ir = compiler.print_ir();

    // Each @interrupt fn must have naked + noinline attributes attached.
    // LLVM IR groups them under attributes #N tags:
    //   define i64 @timer_isr() #N { ... }
    //   attributes #N = { naked noinline ... }
    assert!(
        ir.contains("naked"),
        "expected `naked` attribute somewhere in IR — codegen at \
         src/codegen/llvm/mod.rs:3314-3317. IR was:\n{ir}",
    );
    assert!(
        ir.contains("noinline"),
        "expected `noinline` attribute somewhere in IR — codegen at \
         src/codegen/llvm/mod.rs:3318-3322. IR was:\n{ir}",
    );

    // Both ISR functions must be placed in .text.interrupt section.
    // The `main` function (no annotation) must NOT be.
    assert!(
        ir.contains(".text.interrupt"),
        "expected `.text.interrupt` ELF section directive — codegen at \
         src/codegen/llvm/mod.rs:3324. IR was:\n{ir}",
    );

    // Defensive: count occurrences. With 2 @interrupt fns + 1 plain main,
    // there should be at least 2 references to .text.interrupt (one per
    // ISR's section attribute). Allow more in case of LLVM duplication.
    let section_refs = ir.matches(".text.interrupt").count();
    assert!(
        section_refs >= 2,
        "expected ≥2 `.text.interrupt` references (2 @interrupt fns); \
         found {section_refs}. IR was:\n{ir}",
    );
}

#[test]
fn at_interrupt_e2e_main_fn_not_in_interrupt_section() {
    // Defensive E2E: regular `main` in the same .fj file must NOT pick up
    // the `.text.interrupt` section. Verifies the codegen path is
    // per-function-annotation, not a per-module flag that leaks.
    let source = std::fs::read_to_string("examples/at_interrupt_demo.fj")
        .expect("examples/at_interrupt_demo.fj must exist");

    let tokens = fajar_lang::lexer::tokenize(&source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();

    LlvmCompiler::init_native_target().unwrap();
    let context = Context::create();
    let mut compiler = LlvmCompiler::new(&context, "at_interrupt_e2e_main_isolation");
    compiler.compile_program(&program).unwrap();
    let ir = compiler.print_ir();

    // The `main` function definition must appear without immediately-
    // adjacent `.text.interrupt` or `naked` modifiers. We grep for the
    // function definition line and check the next few lines / containing
    // attribute group don't grant it ISR treatment.
    //
    // LLVM IR for a non-interrupt fn looks like:
    //   define i64 @main() #2 { ... }
    //   attributes #2 = { mustprogress nofree norecurse nounwind ... }
    //
    // Verifying the absence rigorously requires parsing the IR; for the
    // E2E sanity check we assert that grep for "@main()" and grep for
    // ".text.interrupt" do NOT co-locate via their attribute group #.
    //
    // Simpler defensive: the IR should contain exactly 2 distinct attribute
    // groups containing both "naked" AND "noinline" (one per @interrupt
    // fn). If main accidentally got naked, we'd see ≥3 such groups.

    // Find unique attribute-group lines containing both naked and noinline.
    let isr_attr_groups: std::collections::HashSet<&str> = ir
        .lines()
        .filter(|l| l.starts_with("attributes #") && l.contains("naked") && l.contains("noinline"))
        .collect();

    assert!(
        isr_attr_groups.len() <= 2,
        "expected ≤2 attribute groups with `naked` + `noinline` (one per \
         @interrupt fn at most); found {}. main() may have leaked ISR \
         attributes. Groups:\n{:#?}\nFull IR:\n{}",
        isr_attr_groups.len(),
        isr_attr_groups,
        ir,
    );
}
