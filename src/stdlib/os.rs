//! OS standard library bindings.
//!
//! Lists all OS builtin function names. These are registered as `BuiltinFn`
//! values in the interpreter and as typed symbols in the type checker.
//! The actual implementations live in `interpreter::eval` (builtin dispatch)
//! and `runtime::os` (OS subsystems).

/// All OS builtin function names.
///
/// Used for documentation and potential future dynamic registration.
pub const OS_BUILTINS: &[&str] = &[
    "mem_alloc",
    "mem_free",
    "mem_read_u8",
    "mem_read_u32",
    "mem_read_u64",
    "mem_write_u8",
    "mem_write_u32",
    "mem_write_u64",
    "page_map",
    "page_unmap",
    "irq_register",
    "irq_unregister",
    "irq_enable",
    "irq_disable",
    "port_read",
    "port_write",
    "syscall_define",
    "syscall_dispatch",
];
