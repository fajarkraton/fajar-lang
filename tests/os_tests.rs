//! Integration tests for the OS runtime builtins.
//!
//! Tests that Fajar Lang code can call OS primitives (mem_alloc, mem_free,
//! mem_read/write, page_map, irq_register, port_read/write).

use fajar_lang::interpreter::Interpreter;

/// Helper: evaluates source and returns captured output.
fn eval_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).expect("eval_source failed");
    interp.get_output().to_vec()
}

// ── Memory allocation ──

#[test]
fn os_mem_alloc_returns_pointer() {
    let source = r#"
        let p = mem_alloc(64, 8)
        println(type_of(p))
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["pointer"]);
}

#[test]
fn os_mem_alloc_and_free() {
    let source = r#"
        let p = mem_alloc(128, 1)
        mem_free(p)
        println("freed")
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["freed"]);
}

#[test]
fn os_mem_write_and_read_u32() {
    let source = r#"
        let p = mem_alloc(64, 4)
        mem_write_u32(p, 42)
        let val = mem_read_u32(p)
        println(val)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn os_mem_write_and_read_u8() {
    let source = r#"
        let p = mem_alloc(16, 1)
        mem_write_u8(p, 255)
        let val = mem_read_u8(p)
        println(val)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["255"]);
}

#[test]
fn os_mem_write_and_read_u64() {
    let source = r#"
        let p = mem_alloc(16, 8)
        mem_write_u64(p, 123456789)
        let val = mem_read_u64(p)
        println(val)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["123456789"]);
}

// ── Page table ──

#[test]
fn os_page_map_and_unmap() {
    // page_map(virt_addr, phys_addr, flags)
    // flags: READ=1, WRITE=2, RW=3
    let source = r#"
        page_map(4096, 8192, 3)
        page_unmap(4096)
        println("ok")
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["ok"]);
}

// ── IRQ ──

#[test]
fn os_irq_register_enable_disable() {
    let source = r#"
        irq_register(32, "timer_handler")
        irq_enable()
        irq_disable()
        println("irq ok")
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["irq ok"]);
}

// ── Port I/O ──

#[test]
fn os_port_write_and_read() {
    let source = r#"
        port_write(128, 66)
        let val = port_read(128)
        println(val)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["66"]);
}

// ── Alloc + Write + Read + Free cycle ──

#[test]
fn os_full_memory_cycle() {
    let source = r#"
        let p = mem_alloc(256, 8)
        mem_write_u32(p, 100)
        mem_write_u32(p, 200)
        let val = mem_read_u32(p)
        println(val)
        mem_free(p)
        println("done")
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["200", "done"]);
}

#[test]
fn os_pointer_display_format() {
    let source = r#"
        let p = mem_alloc(16, 1)
        println(p)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["0x00000000"]);
}

// ── Syscall ──

#[test]
fn os_syscall_define_and_dispatch() {
    let source = r#"
        syscall_define(1, "sys_write", 3)
        let handler = syscall_dispatch(1, "fd", "buf", "len")
        println(handler)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["sys_write"]);
}

#[test]
fn os_syscall_dispatch_wrong_arg_count() {
    let source = r#"
        syscall_define(60, "sys_exit", 1)
        syscall_dispatch(60, "code", "extra")
    "#;
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(source);
    assert!(result.is_err());
}

#[test]
fn os_syscall_dispatch_no_handler() {
    let source = "syscall_dispatch(99)";
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(source);
    assert!(result.is_err());
}

// ── Kernel init sequence ──

#[test]
fn os_kernel_init_sequence() {
    let source = r#"
        let p = mem_alloc(256, 8)
        mem_write_u32(p, 0xDEAD)
        let val = mem_read_u32(p)
        println(val)
        page_map(4096, 8192, 3)
        println("mapped")
        page_unmap(4096)
        println("unmapped")
        mem_free(p)
        println("freed")
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["57005", "mapped", "unmapped", "freed"]);
}

// ── IRQ register + dispatch ──

#[test]
fn os_irq_register_and_dispatch_log() {
    let source = r#"
        irq_register(32, "timer_handler")
        irq_register(33, "keyboard_handler")
        irq_enable()
        irq_disable()
        irq_unregister(32)
        println("irq lifecycle ok")
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["irq lifecycle ok"]);
}

// ── Syscall define + dispatch from .fj ──

#[test]
fn os_syscall_define_dispatch_from_fj() {
    let source = r#"
        syscall_define(1, "sys_write", 3)
        syscall_define(60, "sys_exit", 1)
        let h1 = syscall_dispatch(1, "fd", "buf", "len")
        let h2 = syscall_dispatch(60, "code")
        println(h1)
        println(h2)
    "#;
    let output = eval_output(source);
    assert_eq!(output, vec!["sys_write", "sys_exit"]);
}
