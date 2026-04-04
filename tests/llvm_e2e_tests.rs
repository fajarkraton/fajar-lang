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

/// Compile program to LLVM IR (no JIT execution). Returns the IR string.
/// Used for bare-metal builtins that are privileged and can't run in userspace.
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

/// Helper: build a call expression `name(arg1, arg2, ...)`
fn make_call(name: &str, arg_exprs: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(make_ident(name)),
        args: arg_exprs.into_iter().map(make_call_arg).collect(),
        span: dummy_span(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2E Tests
// ���═══════════════════════════════════════════════════���══════════════════

#[test]
fn llvm_e2e_simple_return() {
    let items = vec![make_fn("main", vec![], Some(i64_type()), make_int_lit(42))];
    assert_eq!(compile_and_run(items), 42);
}

#[test]
fn llvm_e2e_function_call() {
    // fn add(a: i64, b: i64) -> i64 { a + b }
    // fn main() -> i64 { add(17, 25) }
    let add_body = Expr::Binary {
        left: Box::new(make_ident("a")),
        op: BinOp::Add,
        right: Box::new(make_ident("b")),
        span: dummy_span(),
    };
    let main_body = Expr::Call {
        callee: Box::new(make_ident("add")),
        args: vec![
            make_call_arg(make_int_lit(17)),
            make_call_arg(make_int_lit(25)),
        ],
        span: dummy_span(),
    };
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
    // fn main() -> i64 { if 10 > 5 { 1 } else { 0 } }
    let body = Expr::If {
        condition: Box::new(Expr::Binary {
            left: Box::new(make_int_lit(10)),
            op: BinOp::Gt,
            right: Box::new(make_int_lit(5)),
            span: dummy_span(),
        }),
        then_branch: Box::new(make_int_lit(1)),
        else_branch: Some(Box::new(make_int_lit(0))),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    assert_eq!(compile_and_run(items), 1);
}

#[test]
fn llvm_e2e_while_loop_sum() {
    // fn main() -> i64 { let mut s = 0; let mut i = 0; while i < 10 { s = s + i; i = i + 1 } s }
    let body = Expr::Block {
        stmts: vec![
            Stmt::Let {
                name: "s".into(),
                ty: None,
                value: Box::new(make_int_lit(0)),
                mutable: true,
                linear: false,
                span: dummy_span(),
            },
            Stmt::Let {
                name: "i".into(),
                ty: None,
                value: Box::new(make_int_lit(0)),
                mutable: true,
                linear: false,
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(Expr::While {
                    condition: Box::new(Expr::Binary {
                        left: Box::new(make_ident("i")),
                        op: BinOp::Lt,
                        right: Box::new(make_int_lit(10)),
                        span: dummy_span(),
                    }),
                    body: Box::new(Expr::Block {
                        stmts: vec![
                            Stmt::Expr {
                                expr: Box::new(Expr::Assign {
                                    target: Box::new(make_ident("s")),
                                    value: Box::new(Expr::Binary {
                                        left: Box::new(make_ident("s")),
                                        op: BinOp::Add,
                                        right: Box::new(make_ident("i")),
                                        span: dummy_span(),
                                    }),
                                    op: AssignOp::Assign,
                                    span: dummy_span(),
                                }),
                                span: dummy_span(),
                            },
                            Stmt::Expr {
                                expr: Box::new(Expr::Assign {
                                    target: Box::new(make_ident("i")),
                                    value: Box::new(Expr::Binary {
                                        left: Box::new(make_ident("i")),
                                        op: BinOp::Add,
                                        right: Box::new(make_int_lit(1)),
                                        span: dummy_span(),
                                    }),
                                    op: AssignOp::Assign,
                                    span: dummy_span(),
                                }),
                                span: dummy_span(),
                            },
                        ],
                        expr: None,
                        span: dummy_span(),
                    }),
                    label: None,
                    span: dummy_span(),
                }),
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(make_ident("s"))),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    assert_eq!(compile_and_run(items), 45);
}

#[test]
fn llvm_e2e_recursive_fibonacci() {
    // fn fib(n: i64) -> i64 { if n <= 1 { n } else { fib(n-1) + fib(n-2) } }
    // fn main() -> i64 { fib(10) }
    let fib_body = Expr::If {
        condition: Box::new(Expr::Binary {
            left: Box::new(make_ident("n")),
            op: BinOp::Le,
            right: Box::new(make_int_lit(1)),
            span: dummy_span(),
        }),
        then_branch: Box::new(make_ident("n")),
        else_branch: Some(Box::new(Expr::Binary {
            left: Box::new(Expr::Call {
                callee: Box::new(make_ident("fib")),
                args: vec![make_call_arg(Expr::Binary {
                    left: Box::new(make_ident("n")),
                    op: BinOp::Sub,
                    right: Box::new(make_int_lit(1)),
                    span: dummy_span(),
                })],
                span: dummy_span(),
            }),
            op: BinOp::Add,
            right: Box::new(Expr::Call {
                callee: Box::new(make_ident("fib")),
                args: vec![make_call_arg(Expr::Binary {
                    left: Box::new(make_ident("n")),
                    op: BinOp::Sub,
                    right: Box::new(make_int_lit(2)),
                    span: dummy_span(),
                })],
                span: dummy_span(),
            }),
            span: dummy_span(),
        })),
        span: dummy_span(),
    };
    let main_body = Expr::Call {
        callee: Box::new(make_ident("fib")),
        args: vec![make_call_arg(make_int_lit(10))],
        span: dummy_span(),
    };
    let items = vec![
        make_fn("fib", vec![i64_param("n")], Some(i64_type()), fib_body),
        make_fn("main", vec![], Some(i64_type()), main_body),
    ];
    assert_eq!(compile_and_run(items), 55);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 1: Bare-metal builtin tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn llvm_e2e_rdtsc_returns_nonzero() {
    // fn main() -> i64 { if rdtsc() > 0 { 1 } else { 0 } }
    let body = Expr::If {
        condition: Box::new(Expr::Binary {
            left: Box::new(make_call("rdtsc", vec![])),
            op: BinOp::Gt,
            right: Box::new(make_int_lit(0)),
            span: dummy_span(),
        }),
        then_branch: Box::new(make_int_lit(1)),
        else_branch: Some(Box::new(make_int_lit(0))),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    assert_eq!(compile_and_run(items), 1);
}

#[test]
fn llvm_e2e_rdtsc_increases_monotonically() {
    // fn main() -> i64 {
    //   let t1 = rdtsc()
    //   let t2 = rdtsc()
    //   if t2 > t1 { 1 } else { 0 }
    // }
    let body = Expr::Block {
        stmts: vec![
            Stmt::Let {
                name: "t1".into(),
                ty: None,
                value: Box::new(make_call("rdtsc", vec![])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
            Stmt::Let {
                name: "t2".into(),
                ty: None,
                value: Box::new(make_call("rdtsc", vec![])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(Expr::If {
            condition: Box::new(Expr::Binary {
                left: Box::new(make_ident("t2")),
                op: BinOp::Gt,
                right: Box::new(make_ident("t1")),
                span: dummy_span(),
            }),
            then_branch: Box::new(make_int_lit(1)),
            else_branch: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        })),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    assert_eq!(compile_and_run(items), 1);
}

#[test]
fn llvm_e2e_rdrand_returns_value() {
    // fn main() -> i64 {
    //   let r1 = rdrand()
    //   let r2 = rdrand()
    //   // Extremely unlikely that two consecutive rdrand calls produce the same value
    //   // Just verify it compiles and runs without crashing. Return 1.
    //   if r1 == r2 { 0 } else { 1 }
    // }
    let body = Expr::Block {
        stmts: vec![
            Stmt::Let {
                name: "r1".into(),
                ty: None,
                value: Box::new(make_call("rdrand", vec![])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
            Stmt::Let {
                name: "r2".into(),
                ty: None,
                value: Box::new(make_call("rdrand", vec![])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(Expr::If {
            condition: Box::new(Expr::Binary {
                left: Box::new(make_ident("r1")),
                op: BinOp::Eq,
                right: Box::new(make_ident("r2")),
                span: dummy_span(),
            }),
            then_branch: Box::new(make_int_lit(0)),
            else_branch: Some(Box::new(make_int_lit(1))),
            span: dummy_span(),
        })),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    assert_eq!(compile_and_run(items), 1);
}

#[test]
fn llvm_e2e_volatile_read_write_roundtrip() {
    // Allocate a local buffer via a let binding, volatile write + read back.
    // fn main() -> i64 {
    //   let mut buf: i64 = 0
    //   volatile_write(addr_of(buf), 42)
    //   volatile_read(addr_of(buf))
    // }
    // Since we can't take address easily in the AST, test that volatile_write(0x7FFF0000, 99)
    // compiles correctly. For actual roundtrip, we'll verify IR.
    // Instead: verify that both volatile_read and volatile_write generate valid LLVM IR.
    let body = Expr::Block {
        stmts: vec![Stmt::Expr {
            expr: Box::new(make_call(
                "volatile_write",
                vec![make_int_lit(0x10000), make_int_lit(42)],
            )),
            span: dummy_span(),
        }],
        expr: Some(Box::new(make_call(
            "volatile_read",
            vec![make_int_lit(0x10000)],
        ))),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
    // Verify the IR contains volatile load and store.
    assert!(ir.contains("volatile"), "Expected volatile in IR:\n{ir}");
}

#[test]
fn llvm_e2e_volatile_read_sized_ir() {
    // volatile_read_u8, volatile_read_u16, volatile_read_u32 generate correct LLVM IR.
    let body = Expr::Block {
        stmts: vec![
            Stmt::Let {
                name: "a".into(),
                ty: None,
                value: Box::new(make_call("volatile_read_u8", vec![make_int_lit(0x3F8)])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
            Stmt::Let {
                name: "b".into(),
                ty: None,
                value: Box::new(make_call("volatile_read_u16", vec![make_int_lit(0x3F8)])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
            Stmt::Let {
                name: "c".into(),
                ty: None,
                value: Box::new(make_call("volatile_read_u32", vec![make_int_lit(0x3F8)])),
                mutable: false,
                linear: false,
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(make_ident("a")),
                op: BinOp::Add,
                right: Box::new(make_ident("b")),
                span: dummy_span(),
            }),
            op: BinOp::Add,
            right: Box::new(make_ident("c")),
            span: dummy_span(),
        })),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
    assert!(
        ir.contains("volatile"),
        "Expected volatile loads in IR:\n{ir}"
    );
    // Check for zero-extension (zext).
    assert!(ir.contains("zext"), "Expected zext in IR:\n{ir}");
}

#[test]
fn llvm_e2e_port_io_compiles() {
    // port_inb/outb/inw/outw/ind/outd produce valid LLVM IR with inline asm.
    let body = Expr::Block {
        stmts: vec![
            Stmt::Expr {
                expr: Box::new(make_call(
                    "port_outb",
                    vec![make_int_lit(0x3F8), make_int_lit(0x41)],
                )),
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(make_call(
                    "port_outw",
                    vec![make_int_lit(0x1F0), make_int_lit(0x1234)],
                )),
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(make_call(
                    "port_outd",
                    vec![make_int_lit(0xCF8), make_int_lit(0xDEADBEEF_u32 as i64)],
                )),
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(make_call("port_inb", vec![make_int_lit(0x3F8)])),
                op: BinOp::Add,
                right: Box::new(make_call("port_inw", vec![make_int_lit(0x1F0)])),
                span: dummy_span(),
            }),
            op: BinOp::Add,
            right: Box::new(make_call("port_ind", vec![make_int_lit(0xCF8)])),
            span: dummy_span(),
        })),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
    // Verify inline asm for port I/O is present.
    assert!(ir.contains("inb"), "Expected inb in IR:\n{ir}");
    assert!(ir.contains("outb"), "Expected outb in IR:\n{ir}");
    assert!(ir.contains("inw"), "Expected inw in IR:\n{ir}");
    assert!(ir.contains("outw"), "Expected outw in IR:\n{ir}");
    assert!(ir.contains("inl"), "Expected inl (ind) in IR:\n{ir}");
    assert!(ir.contains("outl"), "Expected outl (outd) in IR:\n{ir}");
}

#[test]
fn llvm_e2e_cli_sti_hlt_compiles() {
    // cli()/sti()/hlt() produce valid LLVM IR.
    let body = Expr::Block {
        stmts: vec![
            Stmt::Expr {
                expr: Box::new(make_call("cli", vec![])),
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(make_call("sti", vec![])),
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(make_call("hlt", vec![])),
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(make_int_lit(0))),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
    assert!(ir.contains("cli"), "Expected cli in IR:\n{ir}");
    assert!(ir.contains("sti"), "Expected sti in IR:\n{ir}");
    assert!(ir.contains("hlt"), "Expected hlt in IR:\n{ir}");
}

#[test]
fn llvm_e2e_rdtsc_ir_contains_asm() {
    // Verify rdtsc produces inline asm with rdtsc instruction.
    let body = make_call("rdtsc", vec![]);
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
    assert!(ir.contains("rdtsc"), "Expected rdtsc in IR:\n{ir}");
}

#[test]
fn llvm_e2e_rdrand_ir_contains_asm() {
    // Verify rdrand produces inline asm with rdrand instruction.
    let body = make_call("rdrand", vec![]);
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
    assert!(ir.contains("rdrand"), "Expected rdrand in IR:\n{ir}");
}

#[test]
fn llvm_e2e_volatile_write_sized_ir() {
    // volatile_write_u8/u16/u32 generate truncated volatile stores.
    let body = Expr::Block {
        stmts: vec![
            Stmt::Expr {
                expr: Box::new(make_call(
                    "volatile_write_u8",
                    vec![make_int_lit(0xB8000), make_int_lit(0x41)],
                )),
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(make_call(
                    "volatile_write_u16",
                    vec![make_int_lit(0xB8000), make_int_lit(0x0F41)],
                )),
                span: dummy_span(),
            },
            Stmt::Expr {
                expr: Box::new(make_call(
                    "volatile_write_u32",
                    vec![make_int_lit(0xB8000), make_int_lit(0x0F410F42)],
                )),
                span: dummy_span(),
            },
        ],
        expr: Some(Box::new(make_int_lit(0))),
        span: dummy_span(),
    };
    let items = vec![make_fn("main", vec![], Some(i64_type()), body)];
    let ir = compile_to_ir(items);
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
