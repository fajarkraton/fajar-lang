//! Context safety enforcement tests for Fajar Lang.
//!
//! Verifies that @safe/@kernel/@device context isolation works correctly.
//! Target: 80+ tests covering all blocked builtins and cross-context calls.
//! Sprint 1 of Master Implementation Plan v7.0.

/// Check that source produces a semantic error containing the given code.
fn expect_error(source: &str, error_code: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let errors = fajar_lang::analyzer::analyze(&program).unwrap_err();
    let found = errors.iter().any(|e| format!("{e}").contains(error_code));
    assert!(
        found,
        "expected error '{error_code}', got: {:?}",
        errors.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

/// Check that source analyzes without hard errors.
fn expect_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    match fajar_lang::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard = errors.iter().filter(|e| !e.is_warning()).count();
            assert!(hard == 0, "unexpected errors: {errors:?}");
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
// 1. SE020: @safe cannot access hardware builtins
// ════════════════════════════════════════════════════════════════════════

#[test]
fn se020_port_outb() {
    expect_error("@safe fn f() { port_outb(0x3F8, 65) }", "SE020");
}
#[test]
fn se020_port_inb() {
    expect_error("@safe fn f() { port_inb(0x3F8) }", "SE020");
}
#[test]
fn se020_port_outw() {
    expect_error("@safe fn f() { port_outw(0x3F8, 0) }", "SE020");
}
#[test]
fn se020_port_inw() {
    expect_error("@safe fn f() { port_inw(0x3F8) }", "SE020");
}
#[test]
fn se020_port_outd() {
    expect_error("@safe fn f() { port_outd(0x3F8, 0) }", "SE020");
}
#[test]
fn se020_port_ind() {
    expect_error("@safe fn f() { port_ind(0x3F8) }", "SE020");
}
#[test]
fn se020_volatile_read() {
    expect_error("@safe fn f() { volatile_read(0x1000) }", "SE020");
}
#[test]
fn se020_volatile_write() {
    expect_error("@safe fn f() { volatile_write(0x1000, 0) }", "SE020");
}
#[test]
fn se020_volatile_read_u8() {
    expect_error("@safe fn f() { volatile_read_u8(0x1000) }", "SE020");
}
#[test]
fn se020_volatile_write_u8() {
    expect_error("@safe fn f() { volatile_write_u8(0x1000, 0) }", "SE020");
}
#[test]
fn se020_volatile_read_u16() {
    expect_error("@safe fn f() { volatile_read_u16(0x1000) }", "SE020");
}
#[test]
fn se020_volatile_write_u16() {
    expect_error("@safe fn f() { volatile_write_u16(0x1000, 0) }", "SE020");
}
#[test]
fn se020_volatile_read_u32() {
    expect_error("@safe fn f() { volatile_read_u32(0x1000) }", "SE020");
}
#[test]
fn se020_volatile_write_u32() {
    expect_error("@safe fn f() { volatile_write_u32(0x1000, 0) }", "SE020");
}
#[test]
fn se020_volatile_read_u64() {
    expect_error("@safe fn f() { volatile_read_u64(0x1000) }", "SE020");
}
#[test]
fn se020_volatile_write_u64() {
    expect_error("@safe fn f() { volatile_write_u64(0x1000, 0) }", "SE020");
}
#[test]
fn se020_read_cr3() {
    expect_error("@safe fn f() { read_cr3() }", "SE020");
}
#[test]
fn se020_write_cr3() {
    expect_error("@safe fn f() { write_cr3(0) }", "SE020");
}
#[test]
fn se020_read_msr() {
    expect_error("@safe fn f() { read_msr(0) }", "SE020");
}
#[test]
fn se020_write_msr() {
    expect_error("@safe fn f() { write_msr(0, 0) }", "SE020");
}
#[test]
fn se020_rdtsc() {
    expect_error("@safe fn f() { rdtsc() }", "SE020");
}
#[test]
fn se020_mem_alloc() {
    expect_error("@safe fn f() { mem_alloc(4096) }", "SE020");
}
#[test]
fn se020_mem_free() {
    expect_error("@safe fn f() { mem_free(0x1000) }", "SE020");
}
#[test]
fn se020_page_map() {
    expect_error("@safe fn f() { page_map(0, 0, 0) }", "SE020");
}
#[test]
fn se020_page_unmap() {
    expect_error("@safe fn f() { page_unmap(0) }", "SE020");
}
#[test]
fn se020_irq_register() {
    expect_error("@safe fn f() { irq_register(0, 0) }", "SE020");
}
#[test]
fn se020_irq_enable() {
    expect_error("@safe fn f() { irq_enable() }", "SE020");
}
#[test]
fn se020_irq_disable() {
    expect_error("@safe fn f() { irq_disable() }", "SE020");
}
#[test]
fn se020_sleep_ms() {
    expect_error("@safe fn f() { sleep_ms(100) }", "SE020");
}
#[test]
fn se020_memory_fence() {
    expect_error("@safe fn f() { memory_fence() }", "SE020");
}
#[test]
fn se020_pci_read32() {
    expect_error("@safe fn f() { pci_read32(0, 0, 0, 0) }", "SE020");
}
#[test]
fn se020_pci_write32() {
    expect_error("@safe fn f() { pci_write32(0, 0, 0, 0, 0) }", "SE020");
}
#[test]
fn se020_dma_alloc() {
    expect_error("@safe fn f() { dma_alloc(4096) }", "SE020");
}
#[test]
fn se020_nvme_read() {
    expect_error("@safe fn f() { nvme_read(0, 0, 0) }", "SE020");
}
#[test]
fn se020_nvme_write() {
    expect_error("@safe fn f() { nvme_write(0, 0, 0) }", "SE020");
}
// frame_alloc/free use mem_alloc/mem_free internally:
#[test]
fn se020_pit_init() {
    expect_error("@safe fn f() { pit_init(100) }", "SE020");
}
#[test]
fn se020_pic_eoi() {
    expect_error("@safe fn f() { pic_eoi(0) }", "SE020");
}
#[test]
fn se020_set_current_pid() {
    expect_error("@safe fn f() { set_current_pid(0) }", "SE020");
}
#[test]
fn se020_idt_init() {
    expect_error("@safe fn f() { idt_init() }", "SE020");
}
#[test]
fn se020_pic_remap() {
    expect_error("@safe fn f() { pic_remap() }", "SE020");
}
#[test]
fn se020_tss_init() {
    expect_error("@safe fn f() { tss_init() }", "SE020");
}
#[test]
fn se020_syscall_init() {
    expect_error("@safe fn f() { syscall_init() }", "SE020");
}
#[test]
fn se020_acpi_shutdown() {
    expect_error("@safe fn f() { acpi_shutdown() }", "SE020");
}
// GPIO/SPI/I2C/UART builtins use full runtime names (fj_rt_bare_*)
// These test the subset that's registered with short names:
#[test]
fn se020_eth_init() {
    expect_error("@safe fn f() { eth_init() }", "SE020");
}
#[test]
fn se020_sse_enable() {
    expect_error("@safe fn f() { sse_enable() }", "SE020");
}
#[test]
fn se020_invlpg() {
    expect_error("@safe fn f() { invlpg(0) }", "SE020");
}
#[test]
fn se020_read_cr2() {
    expect_error("@safe fn f() { read_cr2() }", "SE020");
}
#[test]
fn se020_read_cr4() {
    expect_error("@safe fn f() { read_cr4() }", "SE020");
}
#[test]
fn se020_write_cr4() {
    expect_error("@safe fn f() { write_cr4(0) }", "SE020");
}
#[test]
fn se020_fn_addr() {
    expect_error("@safe fn f() { fn_addr(0) }", "SE020");
}
#[test]
fn se020_proc_create() {
    expect_error("@safe fn f() { proc_create(0) }", "SE020");
}

// ════════════════════════════════════════════════════════════════════════
// 2. SE021: @safe cannot call @kernel functions
// ════════════════════════════════════════════════════════════════════════

#[test]
fn se021_safe_calls_kernel() {
    expect_error(
        "@kernel fn hw_read() -> i64 { 0 }\n@safe fn bad() { hw_read() }",
        "SE021",
    );
}

#[test]
fn se021_safe_calls_kernel_with_return() {
    expect_error(
        "@kernel fn get_cr3() -> i64 { 0 }\n@safe fn bad() -> i64 { get_cr3() }",
        "SE021",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 3. SE022: @safe cannot call @device functions
// ════════════════════════════════════════════════════════════════════════

#[test]
fn se022_safe_calls_device() {
    expect_error(
        "@device fn inference() -> i64 { 0 }\n@safe fn bad() { inference() }",
        "SE022",
    );
}

#[test]
fn se022_safe_calls_device_with_return() {
    expect_error(
        "@device fn classify() -> i64 { 42 }\n@safe fn bad() -> i64 { classify() }",
        "SE022",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. @safe CAN use safe builtins
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_println() {
    expect_ok("@safe fn f() { println(42) }");
}
#[test]
fn safe_len() {
    expect_ok("@safe fn f() { let a = [1, 2, 3]\n len(a) }");
}
#[test]
fn safe_type_of() {
    expect_ok("@safe fn f() { type_of(42) }");
}
#[test]
fn safe_assert() {
    expect_ok("@safe fn f() { assert(true) }");
}
#[test]
fn safe_assert_eq() {
    expect_ok("@safe fn f() { assert_eq(1, 1) }");
}
#[test]
fn safe_to_string() {
    expect_ok("@safe fn f() { to_string(42) }");
}
#[test]
fn safe_arithmetic() {
    expect_ok("@safe fn f() -> i64 { 2 + 3 * 4 }");
}
#[test]
fn safe_string_lit() {
    expect_ok(r#"@safe fn f() -> str { "hello" }"#);
}
#[test]
fn safe_if_else() {
    expect_ok("@safe fn f() -> i64 { if true { 1 } else { 0 } }");
}
#[test]
fn safe_fn_call() {
    expect_ok("@safe fn a() -> i64 { 42 }\n@safe fn b() -> i64 { a() }");
}

// ════════════════════════════════════════════════════════════════════════
// 5. @kernel CAN access hardware
// ════════════════════════════════════════════════════════════════════════

#[test]
fn kernel_calls_kernel() {
    expect_ok("@kernel fn a() -> i64 { 0 }\n@kernel fn b() { a() }");
}
#[test]
fn kernel_arithmetic() {
    expect_ok("@kernel fn f() -> i64 { 2 + 3 }");
}
#[test]
fn kernel_if_else() {
    expect_ok("@kernel fn f() -> i64 { if true { 1 } else { 0 } }");
}

// ════════════════════════════════════════════════════════════════════════
// 6. @kernel CANNOT use tensor ops — KE002 comprehensive
// ════��═══════════════════════════════════════════════════════════════════

// Full-name tensor builtins
#[test]
fn ke002_tensor_zeros() {
    expect_error("@kernel fn f() { tensor_zeros(3, 4) }", "KE002");
}
#[test]
fn ke002_tensor_ones() {
    expect_error("@kernel fn f() { tensor_ones(3, 4) }", "KE002");
}
#[test]
fn ke002_tensor_randn() {
    expect_error("@kernel fn f() { tensor_randn(3, 4) }", "KE002");
}
#[test]
fn ke002_tensor_matmul() {
    expect_error("@kernel fn f() { tensor_matmul(0, 0) }", "KE002");
}
#[test]
fn ke002_tensor_relu() {
    expect_error("@kernel fn f() { tensor_relu(0) }", "KE002");
}
#[test]
fn ke002_tensor_sigmoid() {
    expect_error("@kernel fn f() { tensor_sigmoid(0) }", "KE002");
}
#[test]
fn ke002_tensor_softmax() {
    expect_error("@kernel fn f() { tensor_softmax(0) }", "KE002");
}
#[test]
fn ke002_tensor_tanh() {
    expect_error("@kernel fn f() { tensor_tanh(0) }", "KE002");
}
#[test]
fn ke002_tensor_mse_loss() {
    expect_error("@kernel fn f() { tensor_mse_loss(0, 0) }", "KE002");
}
#[test]
fn ke002_tensor_cross_entropy() {
    expect_error("@kernel fn f() { tensor_cross_entropy(0, 0) }", "KE002");
}
#[test]
fn ke002_tensor_transpose() {
    expect_error("@kernel fn f() { tensor_transpose(0) }", "KE002");
}
#[test]
fn ke002_tensor_reshape() {
    expect_error("@kernel fn f() { tensor_reshape(0, 0) }", "KE002");
}

// Short aliases — must also be blocked
#[test]
fn ke002_zeros() {
    expect_error("@kernel fn f() { zeros(3, 4) }", "KE002");
}
#[test]
fn ke002_ones() {
    expect_error("@kernel fn f() { ones(3, 4) }", "KE002");
}
#[test]
fn ke002_randn() {
    expect_error("@kernel fn f() { randn(3, 4) }", "KE002");
}
#[test]
fn ke002_matmul() {
    expect_error("@kernel fn f() { matmul(0, 0) }", "KE002");
}
#[test]
fn ke002_relu() {
    expect_error("@kernel fn f() { relu(0) }", "KE002");
}
#[test]
fn ke002_sigmoid() {
    expect_error("@kernel fn f() { sigmoid(0) }", "KE002");
}
#[test]
fn ke002_softmax() {
    expect_error("@kernel fn f() { softmax(0) }", "KE002");
}
#[test]
fn ke002_backward() {
    expect_error("@kernel fn f() { backward(0) }", "KE002");
}
#[test]
fn ke002_grad() {
    expect_error("@kernel fn f() { grad(0) }", "KE002");
}
#[test]
fn ke002_dense() {
    expect_error("@kernel fn f() { Dense(784, 128) }", "KE002");
}
#[test]
fn ke002_adam() {
    expect_error("@kernel fn f() { Adam(0.001) }", "KE002");
}
#[test]
fn ke002_sgd() {
    expect_error("@kernel fn f() { SGD(0.01) }", "KE002");
}
#[test]
fn ke002_conv2d() {
    expect_error("@kernel fn f() { Conv2d(3, 16, 3) }", "KE002");
}
#[test]
fn ke002_batchnorm() {
    expect_error("@kernel fn f() { BatchNorm(16) }", "KE002");
}
#[test]
fn ke002_dropout() {
    expect_error("@kernel fn f() { Dropout(0.5) }", "KE002");
}

// ════════════════════════════════════════════════════════════════════════
// 6b. @kernel CANNOT use heap-allocating builtins — KE001
// ════════════���═══════════════════════════════════════════════════════════

#[test]
fn ke001_push() {
    expect_error("@kernel fn f() { let a = [1]\n push(a, 2) }", "KE001");
}
#[test]
fn ke001_to_string() {
    expect_error("@kernel fn f() { to_string(42) }", "KE001");
}
#[test]
fn ke001_map_insert() {
    expect_error(
        r#"@kernel fn f() { let m = {} map_insert(m, "k", 1) }"#,
        "KE001",
    );
}
#[test]
fn ke001_map_get() {
    expect_error(
        r#"@kernel fn f() { let m = {} map_get(m, "k") }"#,
        "KE001",
    );
}
#[test]
fn ke001_map_remove() {
    expect_error(
        r#"@kernel fn f() { let m = {} map_remove(m, "k") }"#,
        "KE001",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 7. @device CANNOT access hardware — DE001 comprehensive
// ════════════════════════════════════════════════════════════════════════

#[test]
fn de001_device_port_outb() {
    expect_error("@device fn f() { port_outb(0x3F8, 65) }", "DE001");
}
#[test]
fn de001_device_mem_alloc() {
    expect_error("@device fn f() { mem_alloc(4096) }", "DE001");
}
#[test]
fn de001_device_mem_free() {
    expect_error("@device fn f() { mem_free(0x1000) }", "DE001");
}
#[test]
fn de001_device_irq_register() {
    expect_error("@device fn f() { irq_register(0, 0) }", "DE001");
}
#[test]
fn de001_device_volatile_read() {
    expect_error("@device fn f() { volatile_read(0x1000) }", "DE001");
}
#[test]
fn de001_device_page_map() {
    expect_error("@device fn f() { page_map(0, 0, 0) }", "DE001");
}

// ════════════════════════════════════════════════════════════════════════
// 8. Cross-context call matrix
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_to_safe_ok() {
    expect_ok("@safe fn a() { 0 }\n@safe fn b() { a() }");
}
#[test]
fn kernel_to_kernel_ok() {
    expect_ok("@kernel fn a() { 0 }\n@kernel fn b() { a() }");
}
#[test]
fn device_to_device_ok() {
    expect_ok("@device fn a() { 0 }\n@device fn b() { a() }");
}

#[test]
fn kernel_to_device_blocked() {
    expect_error("@device fn d() { 0 }\n@kernel fn k() { d() }", "KE003");
}

#[test]
fn device_to_kernel_blocked() {
    expect_error("@kernel fn k() { 0 }\n@device fn d() { k() }", "DE002");
}

// ════════════════════════════════════════════════════════════════════════
// 9. Effect + context interaction
// ════════════════════════════════════════════════════════════════════════

#[test]
fn kernel_with_hardware_ok() {
    expect_ok("@kernel fn f() with Hardware { 0 }");
}

#[test]
fn kernel_with_tensor_blocked() {
    expect_error("@kernel fn f() with Tensor { 0 }", "EE006");
}

#[test]
fn device_with_tensor_ok() {
    expect_ok("@device fn f() with Tensor { 0 }");
}

#[test]
fn device_with_hardware_blocked() {
    expect_error("@device fn f() with Hardware { 0 }", "EE006");
}

#[test]
fn safe_with_io_blocked() {
    expect_error("@safe fn f() with IO { 0 }", "EE006");
}

#[test]
fn safe_with_alloc_blocked() {
    expect_error("@safe fn f() with Alloc { 0 }", "EE006");
}

#[test]
fn safe_with_hardware_blocked() {
    expect_error("@safe fn f() with Hardware { 0 }", "EE006");
}

#[test]
fn safe_with_tensor_blocked() {
    expect_error("@safe fn f() with Tensor { 0 }", "EE006");
}

// ════════════════════════════════════════════════════════════════════════
// 10. No annotation = default behavior (no blocking)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn no_annotation_println() {
    expect_ok("fn f() { println(42) }");
}
#[test]
fn no_annotation_arithmetic() {
    expect_ok("fn f() -> i64 { 2 + 3 }");
}

// ════════════════════════════════════════════════════════════════════════
// 11. V18 2.10: FFI blocked in @safe context
// ════════════════════════════════════════════════════════════════════════

#[test]
fn se020_ffi_load_library() {
    expect_error(r#"@safe fn f() { ffi_load_library("libc.so") }"#, "SE020");
}
#[test]
fn se020_ffi_call() {
    expect_error("@safe fn f() { ffi_call(0, 0, 0) }", "SE020");
}

// ════════════════════════════════════════════════════════════════════════
// 12. V18 2.11: Method calls respect context
// ════════════════════════════════════════════════════════════════════════

#[test]
fn ke002_method_reshape_in_kernel() {
    // t.reshape() is a tensor method — blocked in @kernel
    expect_error("@kernel fn f() { let t = 0\n t.reshape(0) }", "KE002");
}
#[test]
fn ke002_method_relu_in_kernel() {
    expect_error("@kernel fn f() { let t = 0\n t.relu() }", "KE002");
}
#[test]
fn ke002_method_softmax_in_kernel() {
    expect_error("@kernel fn f() { let t = 0\n t.softmax() }", "KE002");
}
#[test]
fn ke002_method_backward_in_kernel() {
    expect_error("@kernel fn f() { let t = 0\n t.backward() }", "KE002");
}
#[test]
fn de001_method_mem_alloc_in_device() {
    expect_error("@device fn f() { let x = 0\n x.mem_alloc() }", "DE001");
}
#[test]
fn method_len_ok_in_kernel() {
    // .len() is safe in @kernel
    expect_ok("@kernel fn f() { let a = [1, 2]\n a.len() }");
}

// ════════════════════════════════════════════════════════════════════════
// 13. V18 2.12: Transitive context enforcement
// ════════════════════════════════════════════════════════════════════════

#[test]
fn ke002_transitive_tensor_wrapper() {
    // fn wrapper calls tensor_zeros → @kernel calling wrapper should error
    expect_error(
        "fn wrap() { tensor_zeros(3, 4) }\n@kernel fn bad() { wrap() }",
        "KE002",
    );
}

#[test]
fn transitive_safe_fn_ok_in_kernel() {
    // fn that only does arithmetic → ok to call from @kernel
    expect_ok("fn add(a: i64, b: i64) -> i64 { a + b }\n@kernel fn k() { add(1, 2) }");
}

// ════════════════════════════════════════════════════════════════════════
// 14. V18 2.7: Generator tests
// ════════════════════════════════════════════════════════════════════════

/// Helper: run source and return captured output.
fn run_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program); // ignore warnings
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

#[test]
fn gen_fn_range_produces_array() {
    let out = run_capture(
        "gen fn range(n: i64) -> i64 {
            let mut i = 0
            while i < n { yield i\n i = i + 1 }
        }
        let v = range(4)
        println(v)",
    );
    assert!(out.contains("[0, 1, 2, 3]"), "got: {out}");
}

#[test]
fn gen_fn_yields_collected() {
    let out = run_capture(
        "gen fn triple() {
            yield 10
            yield 20
            yield 30
        }
        println(triple())",
    );
    assert!(out.contains("[10, 20, 30]"), "got: {out}");
}

#[test]
fn gen_fn_for_loop_iteration() {
    let out = run_capture(
        "gen fn nums() {
            yield 1
            yield 2
        }
        for x in nums() { println(x) }",
    );
    assert!(out.contains("1") && out.contains("2"), "got: {out}");
}
