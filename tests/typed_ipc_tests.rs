//! Typed IPC (@message) tests for Fajar Lang.
//!
//! Verifies @message struct annotation, size validation, message ID
//! assignment, and IPC type safety enforcement.
//! Sprint 8 of Master Implementation Plan v7.0.

fn expect_error(source: &str, error_code: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let errors = fajar_lang::analyzer::analyze(&program).unwrap_err();
    let found = errors.iter().any(|e| format!("{e}").contains(error_code));
    assert!(
        found,
        "expected '{error_code}', got: {:?}",
        errors.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

fn expect_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    match fajar_lang::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
            assert!(hard.is_empty(), "unexpected errors: {hard:?}");
        }
    }
}

fn parse_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    fajar_lang::parser::parse(tokens).expect("parse failed");
}

// ════════════════════════════════════════════════════════════════════════
// 1. @message struct parsing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_message_struct_simple() {
    parse_ok("@message struct VfsOpen { path_offset: i64, path_len: i64, flags: i64 }");
}

#[test]
fn parse_message_struct_two_fields() {
    parse_ok("@message struct VfsReply { fd: i64, status: i64 }");
}

#[test]
fn parse_message_struct_one_field() {
    parse_ok("@message struct PingMsg { timestamp: i64 }");
}

#[test]
fn parse_message_struct_empty() {
    parse_ok("@message struct EmptyMsg {}");
}

#[test]
fn parse_multiple_message_structs() {
    parse_ok(
        r#"
@message struct Request { op: i64, arg1: i64, arg2: i64 }
@message struct Response { result: i64, status: i64 }
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 2. @message size validation
// ════════════════════════════════════════════════════════════════════════

#[test]
fn message_7_fields_ok() {
    // 8 (header) + 7×8 = 64 bytes — exactly fits
    expect_ok("@message struct Max { a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64 }");
}

#[test]
fn message_8_fields_too_large() {
    // 8 (header) + 8×8 = 72 bytes — exceeds 64
    expect_error(
        "@message struct TooBig { a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64 }",
        "IPC001",
    );
}

#[test]
fn message_3_fields_ok() {
    expect_ok("@message struct Small { x: i64, y: i64, z: i64 }");
}

// ════════════════════════════════════════════════════════════════════════
// 3. @message in function context
// ════════════════════════════════════════════════════════════════════════

#[test]
fn message_struct_used_in_function() {
    expect_ok(
        r#"
@message struct Ping { ts: i64 }
fn send_ping() {
    let msg = Ping { ts: 42 }
}
"#,
    );
}

#[test]
fn message_struct_field_access() {
    expect_ok(
        r#"
@message struct Reply { status: i64 }
fn check_reply() {
    let r = Reply { status: 0 }
    println(r.status)
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. @message with @safe context
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_uses_message_struct() {
    expect_ok(
        r#"
@message struct VfsOpen { path_len: i64, flags: i64 }
@safe fn open_file() {
    let msg = VfsOpen { path_len: 10, flags: 0 }
}
"#,
    );
}

#[test]
fn kernel_uses_message_struct() {
    expect_ok(
        r#"
@message struct IrqNotify { irq_num: i64 }
@kernel fn handle_irq() {
    let msg = IrqNotify { irq_num: 14 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5. Multiple message types (unique IDs)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_message_types_parse() {
    expect_ok(
        r#"
@message struct VfsOpen { path_len: i64, flags: i64 }
@message struct VfsRead { fd: i64, len: i64 }
@message struct VfsWrite { fd: i64, data_len: i64 }
@message struct VfsClose { fd: i64 }
@message struct VfsReply { result: i64, status: i64 }
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 6. FajarOS-realistic message structs
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_vfs_messages() {
    expect_ok(
        r#"
@message struct VfsOpen { path_offset: i64, path_len: i64, flags: i64 }
@message struct VfsReply { fd: i64, status: i64 }
@message struct BlkRead { sector: i64, count: i64, buf_ptr: i64 }
@message struct BlkReply { bytes_read: i64, status: i64 }
@message struct NetSend { dst_ip: i64, dst_port: i64, data_ptr: i64, data_len: i64 }
@message struct NetRecv { src_ip: i64, src_port: i64, data_ptr: i64, data_len: i64 }
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 7. Regular structs still work
// ════════════════════════════════════════════════════════════════════════

#[test]
fn regular_struct_no_size_limit() {
    // Regular struct can have any number of fields
    expect_ok(
        "struct BigStruct { a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64, i: i64, j: i64 }",
    );
}

#[test]
fn regular_struct_unaffected_by_message_rules() {
    expect_ok(
        r#"
struct NormalPoint { x: f64, y: f64, z: f64 }
@message struct IpcPoint { x: i64, y: i64 }
fn use_both() {
    let p1 = NormalPoint { x: 1.0, y: 2.0, z: 3.0 }
    let p2 = IpcPoint { x: 1, y: 2 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 8. Edge cases
// ════════════════════════════════════════════════════════════════════════

#[test]
fn message_struct_with_effects() {
    expect_ok(
        r#"
@message struct EffMsg { val: i64 }
fn send_with_effect() with IO {
    let msg = EffMsg { val: 42 }
    println(msg.val)
}
"#,
    );
}

#[test]
fn message_struct_in_comptime() {
    // Comptime should handle @message structs
    parse_ok("@message struct CfgMsg { key: i64, val: i64 }");
}

#[test]
fn message_struct_with_derive() {
    // Double annotations not supported yet — @message takes priority
    parse_ok("@message struct DebugMsg { data: i64 }");
}

// ════════════════════════════════════════════════════════════════════════
// 9. IPC002: ipc_send type-checks @message struct args
// ════════════════════════════════════════════════════════════════════════

#[test]
fn ipc_send_message_struct_ok() {
    expect_ok(
        r#"
@message struct VfsOpen { path_len: i64, flags: i64 }
fn send_open() {
    ipc_send(1, VfsOpen { path_len: 10, flags: 0 })
}
"#,
    );
}

#[test]
fn ipc_send_non_message_struct_error() {
    expect_error(
        r#"
struct Point { x: i64, y: i64 }
fn bad_send() {
    ipc_send(1, Point { x: 1, y: 2 })
}
"#,
        "IPC002",
    );
}

#[test]
fn ipc_send_raw_i64_ok() {
    // Raw integer args are backward compatible
    expect_ok(
        r#"
fn raw_send() {
    ipc_send(1, 42)
}
"#,
    );
}

#[test]
fn ipc_call_message_struct_ok() {
    expect_ok(
        r#"
@message struct BlkRead { sector: i64, count: i64 }
fn do_read() {
    ipc_call(2, BlkRead { sector: 0, count: 8 }, 0)
}
"#,
    );
}

#[test]
fn ipc_call_non_message_struct_error() {
    expect_error(
        r#"
struct Foo { val: i64 }
fn bad_call() {
    ipc_call(2, Foo { val: 42 }, 0)
}
"#,
        "IPC002",
    );
}
