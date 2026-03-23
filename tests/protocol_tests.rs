//! Protocol and Service syntax tests for Fajar Lang.
//!
//! Verifies protocol definitions, service implements clause,
//! completeness checking, and cross-feature interaction.
//! Sprint 9 of Master Implementation Plan v7.0.

fn expect_error(source: &str, error_substr: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let errors = fajar_lang::analyzer::analyze(&program).unwrap_err();
    let found = errors.iter().any(|e| format!("{e}").contains(error_substr));
    assert!(
        found,
        "expected error containing '{error_substr}', got: {:?}",
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
// 1. Protocol parsing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_protocol_empty() {
    parse_ok("protocol Empty {}");
}

#[test]
fn parse_protocol_one_method() {
    parse_ok("protocol Ping { fn ping() -> i64 { 0 } }");
}

#[test]
fn parse_protocol_multiple_methods() {
    parse_ok(
        r#"
protocol VfsProtocol {
    fn open(path: str, flags: i64) -> i64 { 0 }
    fn read(fd: i64, len: i64) -> i64 { 0 }
    fn write(fd: i64, data: i64) -> i64 { 0 }
    fn close(fd: i64) -> i64 { 0 }
}
"#,
    );
}

#[test]
fn parse_protocol_with_return_types() {
    parse_ok(
        r#"
protocol BlkProtocol {
    fn read_sectors(lba: i64, count: i64) -> i64 { 0 }
    fn write_sectors(lba: i64, count: i64) -> i64 { 0 }
    fn get_size() -> i64 { 0 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 2. Service parsing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_service_simple() {
    parse_ok(
        r#"
service echo {
    fn handle(msg: i64) -> i64 { msg }
}
"#,
    );
}

#[test]
fn parse_service_with_implements() {
    parse_ok(
        r#"
protocol PingProto {
    fn ping() -> i64 { 0 }
}
service pinger implements PingProto {
    fn ping() -> i64 { 42 }
}
"#,
    );
}

#[test]
fn parse_service_multiple_handlers() {
    parse_ok(
        r#"
service calculator {
    fn add(a: i64, b: i64) -> i64 { a + b }
    fn mul(a: i64, b: i64) -> i64 { a * b }
    fn neg(a: i64) -> i64 { 0 - a }
}
"#,
    );
}

#[test]
fn parse_service_with_annotation() {
    parse_ok(
        r#"
@safe service user_svc {
    fn handle(x: i64) -> i64 { x }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 3. Protocol completeness checking
// ════════════════════════════════════════════════════════════════════════

#[test]
fn service_implements_complete() {
    expect_ok(
        r#"
protocol Greeter {
    fn greet() -> i64 { 0 }
}
service hello implements Greeter {
    fn greet() -> i64 { 42 }
}
"#,
    );
}

#[test]
fn service_missing_method_error() {
    expect_error(
        r#"
protocol TwoMethods {
    fn method_a() -> i64 { 0 }
    fn method_b() -> i64 { 0 }
}
service incomplete implements TwoMethods {
    fn method_a() -> i64 { 1 }
}
"#,
        "method_b",
    );
}

#[test]
fn service_missing_all_methods() {
    expect_error(
        r#"
protocol Required {
    fn must_have() -> i64 { 0 }
}
service empty_svc implements Required {
}
"#,
        "must_have",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. Service without implements (no checking)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn service_no_implements_ok() {
    expect_ok(
        r#"
service standalone {
    fn process(x: i64) -> i64 { x * 2 }
}
"#,
    );
}

#[test]
fn service_no_implements_any_methods() {
    expect_ok(
        r#"
service flexible {
    fn alpha() -> i64 { 1 }
    fn beta() -> i64 { 2 }
    fn gamma() -> i64 { 3 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5. Protocol + @message interaction
// ════════════════════════════════════════════════════════════════════════

#[test]
fn protocol_with_message_types() {
    parse_ok(
        r#"
@message struct VfsOpenReq { path_len: i64, flags: i64 }
@message struct VfsOpenReply { fd: i64, status: i64 }

protocol VfsProtocol {
    fn open(req: i64) -> i64 { 0 }
    fn close(fd: i64) -> i64 { 0 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 6. Protocol + context annotations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_service_implements_protocol() {
    expect_ok(
        r#"
protocol EchoProto {
    fn echo(x: i64) -> i64 { 0 }
}
@safe service safe_echo implements EchoProto {
    fn echo(x: i64) -> i64 { x }
}
"#,
    );
}

#[test]
fn service_handler_uses_safe_ops() {
    expect_ok(
        r#"
service math_svc {
    fn add(a: i64, b: i64) -> i64 { a + b }
    fn factorial(n: i64) -> i64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 7. FajarOS-realistic protocols
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajaros_vfs_protocol() {
    expect_ok(
        r#"
protocol VfsProtocol {
    fn open(path_len: i64, flags: i64) -> i64 { 0 }
    fn read(fd: i64, len: i64) -> i64 { 0 }
    fn write(fd: i64, data_len: i64) -> i64 { 0 }
    fn close(fd: i64) -> i64 { 0 }
    fn stat(path_len: i64) -> i64 { 0 }
}

service vfs implements VfsProtocol {
    fn open(path_len: i64, flags: i64) -> i64 { 42 }
    fn read(fd: i64, len: i64) -> i64 { 0 }
    fn write(fd: i64, data_len: i64) -> i64 { 0 }
    fn close(fd: i64) -> i64 { 0 }
    fn stat(path_len: i64) -> i64 { 0 }
}
"#,
    );
}

#[test]
fn fajaros_blk_protocol() {
    expect_ok(
        r#"
protocol BlkProtocol {
    fn read_sector(lba: i64, buf: i64) -> i64 { 0 }
    fn write_sector(lba: i64, buf: i64) -> i64 { 0 }
}

service nvme implements BlkProtocol {
    fn read_sector(lba: i64, buf: i64) -> i64 { 0 }
    fn write_sector(lba: i64, buf: i64) -> i64 { 0 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 8. Multiple protocols and services
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_protocols_multiple_services() {
    expect_ok(
        r#"
protocol EchoProto {
    fn echo(x: i64) -> i64 { 0 }
}

protocol MathProto {
    fn double(x: i64) -> i64 { 0 }
}

service echo_svc implements EchoProto {
    fn echo(x: i64) -> i64 { x }
}

service math_svc implements MathProto {
    fn double(x: i64) -> i64 { x * 2 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 9. Protocol + effects
// ════════════════════════════════════════════════════════════════════════

#[test]
fn protocol_methods_with_effects() {
    parse_ok(
        r#"
protocol IoProto {
    fn read_data() -> i64 { 0 }
    fn write_data(val: i64) -> i64 { 0 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 10. Edge cases
// ════════════════════════════════════════════════════════════════════════

#[test]
fn service_extra_methods_ok() {
    // Service can have MORE methods than protocol requires
    expect_ok(
        r#"
protocol MinProto {
    fn required() -> i64 { 0 }
}
service extended implements MinProto {
    fn required() -> i64 { 1 }
    fn bonus() -> i64 { 2 }
    fn extra() -> i64 { 3 }
}
"#,
    );
}

#[test]
fn protocol_used_as_trait() {
    // protocol is stored as TraitDef internally
    expect_ok(
        r#"
protocol Hashable {
    fn hash() -> i64 { 0 }
}
"#,
    );
}
