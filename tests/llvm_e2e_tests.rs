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
