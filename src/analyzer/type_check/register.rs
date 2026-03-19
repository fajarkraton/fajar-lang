//! Builtin registration and symbol table initialization for the type checker.
//!
//! Contains `register_builtins()` (OS/ML/HAL function signatures) and
//! `register_item()` (first-pass declaration registration).

use crate::parser::ast::*;

use super::*;

impl TypeChecker {
    pub(super) fn register_builtins(&mut self) {
        let builtins = vec![
            (
                "print",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "println",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "len",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::USize),
                },
            ),
            (
                "type_of",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Str),
                },
            ),
            (
                "to_string",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Str),
                },
            ),
            (
                "to_int",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::I64),
                },
            ),
            (
                "to_float",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::F64),
                },
            ),
            (
                "format",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Str),
                },
            ),
            (
                "assert",
                Type::Function {
                    params: vec![Type::Bool],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "assert_eq",
                Type::Function {
                    params: vec![Type::Unknown, Type::Unknown],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "push",
                Type::Function {
                    params: vec![Type::Unknown, Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            (
                "pop",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
        ];

        for (name, ty) in builtins {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty,
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // OS runtime builtins (all return Unknown for now — proper typing later)
        let os_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("mem_alloc", vec![Type::I64, Type::I64], Type::Unknown),
            ("mem_free", vec![Type::Unknown], Type::Void),
            ("mem_read_u8", vec![Type::Unknown], Type::I64),
            ("mem_read_u32", vec![Type::Unknown], Type::I64),
            ("mem_read_u64", vec![Type::Unknown], Type::I64),
            ("mem_write_u8", vec![Type::Unknown, Type::I64], Type::Void),
            ("mem_write_u32", vec![Type::Unknown, Type::I64], Type::Void),
            ("mem_write_u64", vec![Type::Unknown, Type::I64], Type::Void),
            (
                "page_map",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::Void,
            ),
            ("page_unmap", vec![Type::Unknown], Type::Void),
            ("irq_register", vec![Type::I64, Type::Str], Type::Void),
            ("irq_unregister", vec![Type::I64], Type::Void),
            ("irq_enable", vec![], Type::Void),
            ("irq_disable", vec![], Type::Void),
            ("port_read", vec![Type::I64], Type::I64),
            ("port_write", vec![Type::I64, Type::I64], Type::Void),
            (
                "syscall_define",
                vec![Type::I64, Type::Str, Type::I64],
                Type::Void,
            ),
            ("syscall_dispatch", vec![Type::Unknown], Type::Str),
            // GPIO builtins (v2.0 Q6A)
            ("gpio_open", vec![Type::I64], Type::I64),
            ("gpio_close", vec![Type::I64], Type::Void),
            ("gpio_set_direction", vec![Type::I64, Type::Str], Type::Void),
            ("gpio_write", vec![Type::I64, Type::I64], Type::Void),
            ("gpio_read", vec![Type::I64], Type::I64),
            ("gpio_toggle", vec![Type::I64], Type::Void),
            // UART builtins (v2.0 Q6A)
            ("uart_open", vec![Type::I64, Type::I64], Type::I64),
            ("uart_close", vec![Type::I64], Type::Void),
            ("uart_write_byte", vec![Type::I64, Type::I64], Type::Void),
            ("uart_read_byte", vec![Type::I64], Type::I64),
            ("uart_write_str", vec![Type::I64, Type::Str], Type::Void),
            // PWM builtins (v2.0 Q6A)
            ("pwm_open", vec![Type::I64], Type::I64),
            ("pwm_close", vec![Type::I64], Type::Void),
            ("pwm_set_frequency", vec![Type::I64, Type::I64], Type::Void),
            ("pwm_set_duty", vec![Type::I64, Type::I64], Type::Void),
            ("pwm_enable", vec![Type::I64], Type::Void),
            ("pwm_disable", vec![Type::I64], Type::Void),
            // SPI builtins (v2.0 Q6A)
            ("spi_open", vec![Type::I64, Type::I64], Type::I64),
            ("spi_close", vec![Type::I64], Type::Void),
            ("spi_transfer", vec![Type::I64, Type::I64], Type::I64),
            ("spi_write", vec![Type::I64, Type::Str], Type::Void),
            // NPU builtins (v2.0 Q6A)
            ("npu_available", vec![], Type::Bool),
            ("npu_info", vec![], Type::Str),
            ("npu_load", vec![Type::Str], Type::I64),
            ("npu_infer", vec![Type::I64, Type::I64], Type::I64),
            (
                "qnn_quantize",
                vec![
                    Type::Tensor {
                        element: Box::new(Type::F64),
                        dims: vec![],
                    },
                    Type::Str,
                ],
                Type::I64,
            ),
            (
                "qnn_dequantize",
                vec![Type::I64],
                Type::Tensor {
                    element: Box::new(Type::F64),
                    dims: vec![],
                },
            ),
            ("qnn_version", vec![], Type::Str),
            // Timing builtins (v2.0)
            ("delay_ms", vec![Type::I64], Type::Void),
            ("delay_us", vec![Type::I64], Type::Void),
            // GPU/OpenCL builtins — detection (v2.0 Q6A)
            ("gpu_available", vec![], Type::Bool),
            ("gpu_info", vec![], Type::Str),
            // Edge AI / production builtins (v2.0 Q6A)
            ("cpu_temp", vec![], Type::I64),
            ("cpu_freq", vec![], Type::I64),
            ("mem_usage", vec![], Type::I64),
            ("sys_uptime", vec![], Type::I64),
            ("log_to_file", vec![Type::Str, Type::Str], Type::Bool),
            // Watchdog / deployment builtins (v2.0 Q6A)
            ("watchdog_start", vec![Type::I64], Type::I64),
            ("watchdog_kick", vec![Type::I64], Type::Bool),
            ("watchdog_stop", vec![Type::I64], Type::Bool),
            ("process_id", vec![], Type::I64),
            ("sleep_ms", vec![Type::I64], Type::Void),
            // Cache / file utilities (v2.0 Q6A)
            ("cache_set", vec![Type::Str, Type::Str], Type::Bool),
            ("cache_get", vec![Type::Str], Type::Str),
            ("cache_clear", vec![], Type::Void),
            ("file_size", vec![Type::Str], Type::I64),
            (
                "dir_list",
                vec![Type::Str],
                Type::Array(Box::new(Type::Str)),
            ),
            ("env_var", vec![Type::Str], Type::Str),
            // x86_64 port I/O builtins (FajarOS Nova)
            ("port_outb", vec![Type::I64, Type::I64], Type::I64),
            ("port_inb", vec![Type::I64], Type::I64),
            ("x86_serial_init", vec![Type::I64, Type::I64], Type::I64),
            ("set_uart_mode_x86", vec![Type::I64], Type::Void),
            // x86_64 CPUID + SSE builtins
            ("cpuid_eax", vec![Type::I64], Type::I64),
            ("cpuid_ebx", vec![Type::I64], Type::I64),
            ("cpuid_ecx", vec![Type::I64], Type::I64),
            ("cpuid_edx", vec![Type::I64], Type::I64),
            ("sse_enable", vec![], Type::Void),
            ("read_cr0", vec![], Type::I64),
            ("read_cr4", vec![], Type::I64),
            // x86_64 IDT + PIC + PIT builtins (Phase 3)
            ("idt_init", vec![], Type::Void),
            ("pic_remap", vec![], Type::Void),
            ("pic_eoi", vec![Type::I64], Type::Void),
            ("pit_init", vec![Type::I64], Type::Void),
            ("read_timer_ticks", vec![], Type::I64),
            // String byte access (no_std VGA support)
            // Take i64 pointer (not str) to avoid heap string issues in no_std
            ("str_byte_at", vec![Type::I64, Type::I64], Type::I64),
            ("str_len", vec![Type::I64], Type::I64),
            // Process scheduler builtins (Phase 4)
            ("proc_table_addr", vec![], Type::I64),
            ("get_current_pid", vec![], Type::I64),
            ("set_current_pid", vec![Type::I64], Type::Void),
            ("get_proc_count", vec![], Type::I64),
            ("proc_create", vec![Type::I64], Type::I64),
            ("yield_proc", vec![], Type::Void),
            // Phase 3 bare-metal HAL builtins (v3.0 FajarOS)
            // GPIO
            (
                "gpio_config",
                vec![Type::I64, Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            ("gpio_set_output", vec![Type::I64], Type::I64),
            ("gpio_set_input", vec![Type::I64], Type::I64),
            ("gpio_set_pull", vec![Type::I64, Type::I64], Type::I64),
            ("gpio_set_irq", vec![Type::I64, Type::I64], Type::I64),
            // UART
            ("uart_init", vec![Type::I64, Type::I64], Type::I64),
            ("uart_available", vec![Type::I64], Type::I64),
            // SPI
            ("spi_init", vec![Type::I64, Type::I64], Type::I64),
            (
                "spi_cs_set",
                vec![Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            // I2C
            ("i2c_init", vec![Type::I64, Type::I64], Type::I64),
            // Timer
            ("timer_get_ticks", vec![], Type::I64),
            ("timer_get_freq", vec![], Type::I64),
            ("timer_set_deadline", vec![Type::I64], Type::Void),
            ("timer_enable_virtual", vec![], Type::Void),
            ("timer_disable_virtual", vec![], Type::Void),
            ("sleep_us", vec![Type::I64], Type::Void),
            ("time_since_boot", vec![], Type::I64),
            ("timer_mark_boot", vec![], Type::Void),
            // DMA
            ("dma_alloc", vec![Type::I64], Type::I64),
            ("dma_free", vec![Type::I64, Type::I64], Type::Void),
            (
                "dma_config",
                vec![Type::I64, Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            ("dma_start", vec![Type::I64], Type::I64),
            ("dma_wait", vec![Type::I64], Type::I64),
            ("dma_status", vec![Type::I64], Type::I64),
            ("dma_barrier", vec![], Type::Void),
            // Phase 4: Storage builtins (v3.0 FajarOS)
            ("nvme_init", vec![], Type::I64),
            (
                "nvme_read",
                vec![Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            (
                "nvme_write",
                vec![Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            ("sd_init", vec![], Type::I64),
            ("sd_read_block", vec![Type::I64, Type::I64], Type::I64),
            ("sd_write_block", vec![Type::I64, Type::I64], Type::I64),
            ("vfs_mount", vec![Type::Str, Type::I64], Type::I64),
            ("vfs_open", vec![Type::Str, Type::I64], Type::I64),
            ("vfs_read", vec![Type::I64, Type::I64, Type::I64], Type::I64),
            (
                "vfs_write",
                vec![Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            ("vfs_close", vec![Type::I64], Type::I64),
            ("vfs_stat", vec![Type::Str], Type::I64),
            // Phase 5: Network builtins (v3.0 FajarOS)
            ("eth_init", vec![], Type::I64),
            ("net_socket", vec![Type::I64], Type::I64),
            ("net_bind", vec![Type::I64, Type::I64], Type::I64),
            ("net_listen", vec![Type::I64], Type::I64),
            ("net_accept", vec![Type::I64], Type::I64),
            (
                "net_connect",
                vec![Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            ("net_send", vec![Type::I64, Type::I64, Type::I64], Type::I64),
            ("net_recv", vec![Type::I64, Type::I64, Type::I64], Type::I64),
            ("net_close", vec![Type::I64], Type::I64),
            // Phase 6: Display & Input (v3.0 FajarOS)
            ("fb_init", vec![Type::I64, Type::I64], Type::I64),
            (
                "fb_write_pixel",
                vec![Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            (
                "fb_fill_rect",
                vec![Type::I64, Type::I64, Type::I64, Type::I64, Type::I64],
                Type::I64,
            ),
            ("fb_width", vec![], Type::I64),
            ("fb_height", vec![], Type::I64),
            ("kb_init", vec![], Type::I64),
            ("kb_read", vec![], Type::I64),
            ("kb_available", vec![], Type::I64),
            // Phase 8: OS Services (v3.0 FajarOS)
            ("proc_spawn", vec![Type::I64], Type::I64),
            ("proc_wait", vec![Type::I64], Type::I64),
            ("proc_kill", vec![Type::I64], Type::I64),
            ("proc_self", vec![], Type::I64),
            ("proc_yield", vec![], Type::Void),
            ("sys_poweroff", vec![], Type::Void),
            ("sys_reboot", vec![], Type::Void),
            ("sys_cpu_temp", vec![], Type::I64),
            ("sys_ram_total", vec![], Type::I64),
            ("sys_ram_free", vec![], Type::I64),
            // Context switch builtins
            ("sched_get_saved_sp", vec![], Type::I64),
            ("sched_set_next_sp", vec![Type::I64], Type::Void),
            ("sched_read_proc", vec![Type::I64], Type::I64),
            ("sched_write_proc", vec![Type::I64, Type::I64], Type::Void),
            // Syscall builtins
            ("syscall_arg0", vec![], Type::I64),
            ("syscall_arg1", vec![], Type::I64),
            ("syscall_arg2", vec![], Type::I64),
            ("syscall_set_return", vec![Type::I64], Type::Void),
            ("svc", vec![Type::I64, Type::I64, Type::I64], Type::I64),
            ("switch_ttbr0", vec![Type::I64], Type::Void),
            ("read_ttbr0", vec![], Type::I64),
            ("tlbi_va", vec![Type::I64], Type::Void),
        ];
        for (name, params, ret) in os_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // ML runtime builtins (tensor operations)
        // Dynamic tensor type: Tensor<f64>[] — unknown shape, compatible with all tensors
        let dyn_t = Type::dynamic_tensor();
        let ml_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            // Creation functions → return dynamic tensor
            ("tensor_zeros", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_ones", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_randn", vec![Type::Unknown], dyn_t.clone()),
            ("zeros", vec![Type::Unknown], dyn_t.clone()),
            ("ones", vec![Type::Unknown], dyn_t.clone()),
            ("randn", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_rand", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_eye", vec![Type::I64], dyn_t.clone()),
            ("tensor_full", vec![Type::Unknown, Type::F64], dyn_t.clone()),
            (
                "tensor_from_data",
                vec![Type::Unknown, Type::Unknown],
                dyn_t.clone(),
            ),
            // Shape query
            ("tensor_shape", vec![dyn_t.clone()], Type::Unknown), // returns array
            (
                "tensor_reshape",
                vec![dyn_t.clone(), Type::Unknown],
                dyn_t.clone(),
            ),
            ("tensor_numel", vec![dyn_t.clone()], Type::I64),
            // Tensor arithmetic → return dynamic tensor
            (
                "tensor_add",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_sub",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_mul",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_div",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            ("tensor_neg", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_matmul",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            ("tensor_transpose", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_sum", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_mean", vec![dyn_t.clone()], dyn_t.clone()),
            // Activation functions → return dynamic tensor (same shape as input)
            ("tensor_relu", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_sigmoid", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_tanh", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_softmax", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_gelu", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_leaky_relu",
                vec![dyn_t.clone(), Type::F64],
                dyn_t.clone(),
            ),
            // Loss functions → return dynamic tensor
            (
                "tensor_mse_loss",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_cross_entropy",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_bce_loss",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            // Shape manipulation → return dynamic tensor
            ("tensor_flatten", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_squeeze",
                vec![dyn_t.clone(), Type::I64],
                dyn_t.clone(),
            ),
            (
                "tensor_unsqueeze",
                vec![dyn_t.clone(), Type::I64],
                dyn_t.clone(),
            ),
            // Reductions → return dynamic tensor
            ("tensor_max", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_min", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_argmax", vec![dyn_t.clone()], Type::I64),
            // Creation
            (
                "tensor_arange",
                vec![Type::F64, Type::F64, Type::F64],
                dyn_t.clone(),
            ),
            (
                "tensor_linspace",
                vec![Type::F64, Type::F64, Type::I64],
                dyn_t.clone(),
            ),
            ("tensor_xavier", vec![Type::I64, Type::I64], dyn_t.clone()),
            // Loss
            (
                "tensor_l1_loss",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            // Memory management (no-op in interpreter, meaningful in native codegen)
            ("tensor_free", vec![dyn_t.clone()], Type::Void),
            // Shape query → return scalar integers
            ("tensor_rows", vec![dyn_t.clone()], Type::I64),
            ("tensor_cols", vec![dyn_t.clone()], Type::I64),
            // Element access
            ("tensor_row", vec![dyn_t.clone(), Type::I64], dyn_t.clone()),
            (
                "tensor_set",
                vec![dyn_t.clone(), Type::I64, Type::I64, Type::I64],
                Type::Void,
            ),
            // Normalization and scaling
            ("tensor_normalize", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_scale",
                vec![dyn_t.clone(), Type::I64],
                dyn_t.clone(),
            ),
            // GPU/OpenCL tensor builtins — CPU fallback (v2.0 Q6A)
            (
                "gpu_matmul",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            ("gpu_add", vec![dyn_t.clone(), dyn_t.clone()], dyn_t.clone()),
            ("gpu_relu", vec![dyn_t.clone()], dyn_t.clone()),
            ("gpu_sigmoid", vec![dyn_t.clone()], dyn_t.clone()),
            ("gpu_mul", vec![dyn_t.clone(), dyn_t.clone()], dyn_t.clone()),
            ("gpu_transpose", vec![dyn_t.clone()], dyn_t.clone()),
            ("gpu_sum", vec![dyn_t.clone()], Type::F64),
        ];
        for (name, params, ret) in ml_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Built-in enum constructors (Some, None, Ok, Err, Ready, Pending)
        let enum_constructors: Vec<(&str, Type)> = vec![
            (
                "Some",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            ("None", Type::Unknown),
            (
                "Ok",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            (
                "Err",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            (
                "Ready",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Enum {
                        name: "Poll".to_string(),
                    }),
                },
            ),
            (
                "Pending",
                Type::Enum {
                    name: "Poll".to_string(),
                },
            ),
        ];
        for (name, ty) in enum_constructors {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty,
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Built-in constants (PI, E)
        self.symbols.define(Symbol {
            name: "PI".to_string(),
            ty: Type::F64,
            mutable: false,
            span: Span::new(0, 0),
            used: false,
        });
        self.symbols.define(Symbol {
            name: "E".to_string(),
            ty: Type::F64,
            mutable: false,
            span: Span::new(0, 0),
            used: false,
        });

        // Error/debug builtins
        let debug_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("panic", vec![Type::Unknown], Type::Never),
            ("todo", vec![], Type::Never),
            ("dbg", vec![Type::Unknown], Type::Unknown),
            ("eprint", vec![Type::Unknown], Type::Void),
            ("eprintln", vec![Type::Unknown], Type::Void),
        ];
        for (name, params, ret) in debug_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Math builtins (accept Unknown to allow both int and float args)
        let math_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("abs", vec![Type::Unknown], Type::Unknown),
            ("sqrt", vec![Type::Unknown], Type::F64),
            ("pow", vec![Type::Unknown, Type::Unknown], Type::F64),
            ("log", vec![Type::Unknown], Type::F64),
            ("log2", vec![Type::Unknown], Type::F64),
            ("log10", vec![Type::Unknown], Type::F64),
            ("sin", vec![Type::Unknown], Type::F64),
            ("cos", vec![Type::Unknown], Type::F64),
            ("tan", vec![Type::Unknown], Type::F64),
            ("floor", vec![Type::Unknown], Type::F64),
            ("ceil", vec![Type::Unknown], Type::F64),
            ("round", vec![Type::Unknown], Type::F64),
            (
                "clamp",
                vec![Type::Unknown, Type::Unknown, Type::Unknown],
                Type::Unknown,
            ),
            ("min", vec![Type::Unknown, Type::Unknown], Type::Unknown),
            ("max", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ];
        for (name, params, ret) in math_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Collection builtins (HashMap)
        let collection_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("map_new", vec![], Type::Unknown),
            (
                "map_insert",
                vec![Type::Unknown, Type::Str, Type::Unknown],
                Type::Unknown,
            ),
            ("map_get", vec![Type::Unknown, Type::Str], Type::Unknown),
            ("map_remove", vec![Type::Unknown, Type::Str], Type::Unknown),
            (
                "map_contains_key",
                vec![Type::Unknown, Type::Str],
                Type::Bool,
            ),
            ("map_keys", vec![Type::Unknown], Type::Unknown),
            ("map_values", vec![Type::Unknown], Type::Unknown),
            ("map_len", vec![Type::Unknown], Type::I64),
        ];
        for (name, params, ret) in collection_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // File I/O builtins
        let io_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("read_file", vec![Type::Str], Type::Unknown),
            ("write_file", vec![Type::Str, Type::Str], Type::Unknown),
            ("append_file", vec![Type::Str, Type::Str], Type::Unknown),
            ("file_exists", vec![Type::Str], Type::Bool),
        ];
        for (name, params, ret) in io_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Metrics builtins
        let metrics_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            (
                "metric_accuracy",
                vec![Type::Unknown, Type::Unknown],
                Type::F64,
            ),
            (
                "metric_precision",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::F64,
            ),
            (
                "metric_recall",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::F64,
            ),
            (
                "metric_f1_score",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::F64,
            ),
        ];
        for (name, params, ret) in metrics_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Autograd builtins
        let autograd_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("tensor_backward", vec![dyn_t.clone()], Type::Void),
            ("tensor_grad", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_requires_grad", vec![dyn_t.clone()], Type::Bool),
            (
                "tensor_set_requires_grad",
                vec![dyn_t.clone(), Type::Bool],
                dyn_t.clone(),
            ),
            ("tensor_detach", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_clear_tape", vec![], Type::Void),
            ("tensor_no_grad_begin", vec![], Type::Void),
            ("tensor_no_grad_end", vec![], Type::Void),
        ];
        for (name, params, ret) in autograd_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Optimizer builtins
        let optim_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            (
                "optimizer_sgd",
                vec![Type::Unknown, Type::Unknown],
                Type::Unknown,
            ),
            ("optimizer_adam", vec![Type::F64], Type::Unknown),
            (
                "optimizer_step",
                vec![Type::Unknown, Type::Unknown],
                dyn_t.clone(),
            ),
            ("optimizer_zero_grad", vec![Type::Unknown], dyn_t.clone()),
        ];
        for (name, params, ret) in optim_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Model export builtins (variadic: path, name1, tensor1, ...)
        let export_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("model_save", vec![Type::Unknown], Type::Unknown),
            ("model_save_quantized", vec![Type::Unknown], Type::Unknown),
        ];
        for (name, params, ret) in export_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Layer builtins
        let layer_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("layer_dense", vec![Type::I64, Type::I64], Type::Unknown),
            (
                "layer_forward",
                vec![Type::Unknown, Type::Unknown],
                Type::Unknown,
            ),
            ("layer_params", vec![Type::Unknown], Type::Unknown),
        ];
        for (name, params, ret) in layer_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Hardware detection builtins (v1.1)
        let hw_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("hw_cpu_vendor", vec![], Type::Str),
            ("hw_cpu_arch", vec![], Type::Str),
            ("hw_has_avx2", vec![], Type::Bool),
            ("hw_has_avx512", vec![], Type::Bool),
            ("hw_has_amx", vec![], Type::Bool),
            ("hw_has_neon", vec![], Type::Bool),
            ("hw_has_sve", vec![], Type::Bool),
            ("hw_simd_width", vec![], Type::I64),
            // Accelerator registry builtins (v1.1 S4)
            ("hw_gpu_count", vec![], Type::I64),
            ("hw_npu_count", vec![], Type::I64),
            ("hw_best_accelerator", vec![], Type::Str),
        ];
        for (name, params, ret) in hw_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }
    }

    /// Registers built-in traits and their implementations for primitive types.
    pub(super) fn register_builtin_traits(&mut self) {
        // Built-in trait names (no methods needed for bound checking)
        let builtin_trait_names = [
            "Display",
            "Debug",
            "Clone",
            "Copy",
            "PartialEq",
            "Eq",
            "PartialOrd",
            "Ord",
            "Default",
            "Hash",
        ];
        for name in &builtin_trait_names {
            self.traits.entry(name.to_string()).or_default();
        }

        // Primitive types that implement all common traits
        let primitive_types = [
            "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "isize", "usize",
            "f32", "f64", "bool", "char",
        ];
        let all_traits = [
            "Display",
            "Debug",
            "Clone",
            "Copy",
            "PartialEq",
            "Eq",
            "PartialOrd",
            "Ord",
            "Default",
            "Hash",
        ];
        for ty in &primitive_types {
            for tr in &all_traits {
                self.trait_impls.insert((tr.to_string(), ty.to_string()));
            }
        }

        // String implements Display, Debug, Clone, PartialEq, Eq, Hash, Default
        for tr in &[
            "Display",
            "Debug",
            "Clone",
            "PartialEq",
            "Eq",
            "Hash",
            "Default",
        ] {
            self.trait_impls.insert((tr.to_string(), "str".to_string()));
            self.trait_impls
                .insert((tr.to_string(), "String".to_string()));
        }

        // Built-in Future<T> trait: fn poll(&mut self) -> Poll<T> (S4.2)
        self.traits.insert(
            "Future".to_string(),
            vec![TraitMethodSig {
                name: "poll".to_string(),
                param_types: vec![],
                ret_type: Type::Enum {
                    name: "Poll".to_string(),
                },
            }],
        );

        // Built-in Drop trait (already handled elsewhere, register name)
        self.traits.entry("Drop".to_string()).or_default();
    }

    /// Checks whether a concrete type satisfies a trait bound.
    /// Used by tests; will be integrated into trait constraint checking.
    #[allow(dead_code)]
    pub(super) fn type_satisfies_trait(&self, type_name: &str, trait_name: &str) -> bool {
        self.trait_impls
            .contains(&(trait_name.to_string(), type_name.to_string()))
    }

    /// Pre-registers additional known names as `Type::Unknown` symbols.
    ///
    /// Used by REPL / `eval_source()` to prevent false "undefined variable" errors
    /// for names defined in prior evaluation rounds.
    pub fn register_known_names(&mut self, names: &[String]) {
        for name in names {
            if self.symbols.lookup(name).is_none() {
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: Type::Unknown,
                    mutable: true,
                    span: Span::new(0, 0),
                    used: true, // don't warn about unused
                });
            }
        }
    }

    /// First pass: register top-level declarations.
    pub(super) fn register_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(fndef) => {
                // For generic functions, temporarily register type params so resolve_type works
                let generic_names: Vec<String> = fndef
                    .generic_params
                    .iter()
                    .map(|g| g.name.clone())
                    .collect();
                if !generic_names.is_empty() {
                    self.symbols
                        .push_scope_kind(crate::analyzer::scope::ScopeKind::Block);
                    for gp in &fndef.generic_params {
                        self.symbols.define(Symbol {
                            name: gp.name.clone(),
                            ty: Type::TypeVar(gp.name.clone()),
                            mutable: false,
                            span: gp.span,
                            used: true,
                        });
                    }
                }

                let param_types: Vec<Type> = fndef
                    .params
                    .iter()
                    .map(|p| self.resolve_type(&p.ty))
                    .collect();
                let ret_type = fndef
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Type::Void);
                // async fn wraps return type in Future<T>
                let effective_ret = if fndef.is_async {
                    Type::Future {
                        inner: Box::new(ret_type),
                    }
                } else {
                    ret_type
                };
                let fn_type = Type::Function {
                    params: param_types,
                    ret: Box::new(effective_ret),
                };

                if !generic_names.is_empty() {
                    let _ = self.symbols.pop_scope_unused();
                }

                self.symbols.define(Symbol {
                    name: fndef.name.clone(),
                    ty: fn_type,
                    mutable: false,
                    span: fndef.span,
                    used: false,
                });
                // Track annotation context
                if let Some(ann) = &fndef.annotation {
                    match ann.name.as_str() {
                        "kernel" => {
                            self.kernel_fns.insert(fndef.name.clone());
                        }
                        "device" => {
                            self.device_fns.insert(fndef.name.clone());
                        }
                        "npu" => {
                            self.npu_fns.insert(fndef.name.clone());
                        }
                        _ => {}
                    }
                }
            }
            Item::StructDef(sdef) => {
                let mut fields = HashMap::new();
                for field in &sdef.fields {
                    fields.insert(field.name.clone(), self.resolve_type(&field.ty));
                }
                self.symbols.define(Symbol {
                    name: sdef.name.clone(),
                    ty: Type::Struct {
                        name: sdef.name.clone(),
                        fields,
                    },
                    mutable: false,
                    span: sdef.span,
                    used: false,
                });
            }
            Item::UnionDef(udef) => {
                let mut fields = HashMap::new();
                for field in &udef.fields {
                    fields.insert(field.name.clone(), self.resolve_type(&field.ty));
                }
                self.symbols.define(Symbol {
                    name: udef.name.clone(),
                    ty: Type::Struct {
                        name: udef.name.clone(),
                        fields,
                    },
                    mutable: false,
                    span: udef.span,
                    used: false,
                });
            }
            Item::EnumDef(edef) => {
                // For generic enums, temporarily register type params as Unknown
                let has_generics = !edef.generic_params.is_empty();
                if has_generics {
                    self.symbols
                        .push_scope_kind(crate::analyzer::scope::ScopeKind::Block);
                    for gp in &edef.generic_params {
                        self.symbols.define(Symbol {
                            name: gp.name.clone(),
                            ty: Type::TypeVar(gp.name.clone()),
                            mutable: false,
                            span: gp.span,
                            used: true,
                        });
                    }
                }

                self.symbols.define(Symbol {
                    name: edef.name.clone(),
                    ty: Type::Enum {
                        name: edef.name.clone(),
                    },
                    mutable: false,
                    span: edef.span,
                    used: false,
                });
                // Track variant names for exhaustiveness checking
                let variant_names: Vec<String> =
                    edef.variants.iter().map(|v| v.name.clone()).collect();
                self.enum_variants.insert(edef.name.clone(), variant_names);
                // Register variants
                for variant in &edef.variants {
                    if variant.fields.is_empty() {
                        self.symbols.define(Symbol {
                            name: variant.name.clone(),
                            ty: Type::Enum {
                                name: edef.name.clone(),
                            },
                            mutable: false,
                            span: variant.span,
                            used: false,
                        });
                    } else {
                        let field_types: Vec<Type> = variant
                            .fields
                            .iter()
                            .map(|f| self.resolve_type(f))
                            .collect();
                        self.symbols.define(Symbol {
                            name: variant.name.clone(),
                            ty: Type::Function {
                                params: field_types,
                                ret: Box::new(Type::Enum {
                                    name: edef.name.clone(),
                                }),
                            },
                            mutable: false,
                            span: variant.span,
                            used: false,
                        });
                    }
                }

                if has_generics {
                    let _ = self.symbols.pop_scope_unused();
                }
            }
            Item::ConstDef(cdef) => {
                let ty = self.resolve_type(&cdef.ty);
                self.symbols.define(Symbol {
                    name: cdef.name.clone(),
                    ty,
                    mutable: false,
                    span: cdef.span,
                    used: false,
                });
            }
            Item::ImplBlock(impl_block) => {
                self.register_impl_block(impl_block);
            }
            Item::ModDecl(mod_decl) => {
                self.register_mod_decl(mod_decl);
            }
            Item::UseDecl(use_decl) => {
                self.register_use_decl(use_decl);
            }
            Item::TraitDef(tdef) => {
                self.register_trait_def(tdef);
            }
            Item::ExternFn(efn) => {
                self.register_extern_fn(efn);
            }
            Item::TypeAlias(ta) => {
                self.register_type_alias(ta);
            }
            _ => {}
        }
    }

    /// Registers a trait definition, storing its method signatures.
    fn register_trait_def(&mut self, tdef: &crate::parser::ast::TraitDef) {
        let mut method_sigs = Vec::new();
        let mut seen_methods = std::collections::HashSet::new();

        for method in &tdef.methods {
            if !seen_methods.insert(method.name.clone()) {
                self.errors.push(SemanticError::DuplicateDefinition {
                    name: method.name.clone(),
                    span: method.span,
                });
                continue;
            }

            let param_types: Vec<Type> = method
                .params
                .iter()
                .map(|p| self.resolve_type(&p.ty))
                .collect();
            let ret_type = method
                .return_type
                .as_ref()
                .map(|t| self.resolve_type(t))
                .unwrap_or(Type::Void);

            method_sigs.push(TraitMethodSig {
                name: method.name.clone(),
                param_types,
                ret_type,
            });
        }

        self.traits.insert(tdef.name.clone(), method_sigs);
    }

    /// Registers a type alias, resolving the target type.
    fn register_type_alias(&mut self, ta: &TypeAlias) {
        let resolved = self.resolve_type(&ta.ty);
        self.type_aliases.insert(ta.name.clone(), resolved);
    }

    /// Registers an extern function declaration in the symbol table.
    ///
    /// Validates that all parameter types and the return type are FFI-safe.
    /// FFI-safe types: bool, i8-i64, u8-u64, isize, usize, f32, f64, void.
    fn register_extern_fn(&mut self, efn: &ExternFn) {
        let param_types: Vec<Type> = efn
            .params
            .iter()
            .map(|p| self.resolve_type(&p.ty))
            .collect();
        let ret_type = efn
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Void);

        // Validate FFI-safe types
        for (i, ty) in param_types.iter().enumerate() {
            if !self.is_ffi_safe(ty) {
                self.errors.push(SemanticError::FfiUnsafeType {
                    ty: format!("{:?}", ty),
                    func: efn.name.clone(),
                    span: efn.params[i].span,
                });
            }
        }
        if !self.is_ffi_safe(&ret_type) {
            self.errors.push(SemanticError::FfiUnsafeType {
                ty: format!("{:?}", ret_type),
                func: efn.name.clone(),
                span: efn.span,
            });
        }

        let fn_type = Type::Function {
            params: param_types,
            ret: Box::new(ret_type),
        };
        self.symbols.define(Symbol {
            name: efn.name.clone(),
            ty: fn_type,
            mutable: false,
            span: efn.span,
            used: false,
        });
    }

    /// Returns `true` if the type is FFI-safe (can cross the C ABI boundary).
    fn is_ffi_safe(&self, ty: &Type) -> bool {
        matches!(
            ty,
            Type::Void
                | Type::Bool
                | Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::ISize
                | Type::USize
                | Type::F32
                | Type::F64
                | Type::IntLiteral
                | Type::FloatLiteral
        )
    }

    /// Registers impl block methods in the symbol table.
    fn register_impl_block(&mut self, impl_block: &ImplBlock) {
        let mut impl_method_names: Vec<String> = Vec::new();

        for method in &impl_block.methods {
            impl_method_names.push(method.name.clone());

            // Validate `self` parameter placement
            for (i, param) in method.params.iter().enumerate() {
                if param.name == "self" && i != 0 {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "self must be the first parameter".into(),
                        found: format!("self at position {}", i + 1),
                        span: param.span,
                        hint: None,
                    });
                }
            }

            let param_types: Vec<Type> = method
                .params
                .iter()
                .map(|p| {
                    if p.name == "self" {
                        let struct_ty = Type::Struct {
                            name: impl_block.target_type.clone(),
                            fields: HashMap::new(),
                        };
                        // Handle &self / &mut self sugar
                        match &p.ty {
                            crate::parser::ast::TypeExpr::Reference { mutable, .. } => {
                                if *mutable {
                                    Type::RefMut(Box::new(struct_ty))
                                } else {
                                    Type::Ref(Box::new(struct_ty))
                                }
                            }
                            _ => struct_ty,
                        }
                    } else {
                        self.resolve_type(&p.ty)
                    }
                })
                .collect();
            let ret_type = method
                .return_type
                .as_ref()
                .map(|t| self.resolve_type(t))
                .unwrap_or(Type::Void);

            // Register as qualified name: TypeName::method
            let qualified = format!("{}::{}", impl_block.target_type, method.name);
            self.symbols.define(Symbol {
                name: qualified,
                ty: Type::Function {
                    params: param_types,
                    ret: Box::new(ret_type),
                },
                mutable: false,
                span: method.span,
                used: false,
            });
        }

        // If this is a trait impl (`impl Trait for Type`), validate completeness + signatures
        if let Some(trait_name) = &impl_block.trait_name {
            if let Some(trait_methods) = self.traits.get(trait_name).cloned() {
                for tm in &trait_methods {
                    if !impl_method_names.contains(&tm.name) {
                        // Missing method
                        self.errors.push(SemanticError::MissingField {
                            struct_name: format!(
                                "impl {} for {}",
                                trait_name, impl_block.target_type
                            ),
                            field: tm.name.clone(),
                            span: impl_block.span,
                        });
                    } else {
                        // Method exists — verify signature matches
                        if let Some(impl_method) =
                            impl_block.methods.iter().find(|m| m.name == tm.name)
                        {
                            let impl_param_types: Vec<Type> = impl_method
                                .params
                                .iter()
                                .map(|p| {
                                    if p.name == "self" {
                                        Type::Struct {
                                            name: impl_block.target_type.clone(),
                                            fields: HashMap::new(),
                                        }
                                    } else {
                                        self.resolve_type(&p.ty)
                                    }
                                })
                                .collect();
                            let impl_ret = impl_method
                                .return_type
                                .as_ref()
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(Type::Void);

                            // Check parameter count (excluding self for comparison)
                            let trait_non_self: Vec<&Type> = tm
                                .param_types
                                .iter()
                                .filter(|t| !matches!(t, Type::Unknown))
                                .collect();
                            let impl_non_self: Vec<&Type> = impl_param_types
                                .iter()
                                .filter(|t| {
                                    !matches!(t, Type::Struct { name, .. } if name == &impl_block.target_type)
                                })
                                .collect();

                            if impl_method.params.len() != tm.param_types.len() {
                                self.errors
                                    .push(SemanticError::TraitMethodSignatureMismatch {
                                        method: tm.name.clone(),
                                        trait_name: trait_name.clone(),
                                        target_type: impl_block.target_type.clone(),
                                        detail: format!(
                                            "expected {} parameters, found {}",
                                            tm.param_types.len(),
                                            impl_method.params.len()
                                        ),
                                        span: impl_method.span,
                                    });
                            } else {
                                // Check return type
                                if !types_compatible(&tm.ret_type, &impl_ret) {
                                    self.errors
                                        .push(SemanticError::TraitMethodSignatureMismatch {
                                            method: tm.name.clone(),
                                            trait_name: trait_name.clone(),
                                            target_type: impl_block.target_type.clone(),
                                            detail: format!(
                                                "expected return type {:?}, found {:?}",
                                                tm.ret_type, impl_ret
                                            ),
                                            span: impl_method.span,
                                        });
                                }

                                // Check non-self parameter types
                                for (i, (trait_t, impl_t)) in
                                    trait_non_self.iter().zip(impl_non_self.iter()).enumerate()
                                {
                                    if !types_compatible(trait_t, impl_t) {
                                        self.errors.push(
                                            SemanticError::TraitMethodSignatureMismatch {
                                                method: tm.name.clone(),
                                                trait_name: trait_name.clone(),
                                                target_type: impl_block.target_type.clone(),
                                                detail: format!(
                                                    "parameter {} has type {:?}, expected {:?}",
                                                    i + 1,
                                                    impl_t,
                                                    trait_t
                                                ),
                                                span: impl_method.span,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                // Record that this type implements this trait
                self.trait_impls
                    .insert((trait_name.clone(), impl_block.target_type.clone()));
            }
            // If trait not found, it might just not be defined yet — no error
        }
    }

    /// Registers items inside a module declaration with qualified names.
    fn register_mod_decl(&mut self, mod_decl: &ModDecl) {
        self.register_mod_items(&mod_decl.name, &mod_decl.body);
    }

    /// Registers module items with a given prefix for qualified names.
    fn register_mod_items(&mut self, prefix: &str, body: &Option<Vec<Item>>) {
        if let Some(items) = body {
            for item in items {
                match item {
                    Item::FnDef(fndef) => {
                        let param_types: Vec<Type> = fndef
                            .params
                            .iter()
                            .map(|p| self.resolve_type(&p.ty))
                            .collect();
                        let ret_type = fndef
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Type::Void);
                        let qualified = format!("{}::{}", prefix, fndef.name);
                        self.symbols.define(Symbol {
                            name: qualified,
                            ty: Type::Function {
                                params: param_types,
                                ret: Box::new(ret_type),
                            },
                            mutable: false,
                            span: fndef.span,
                            used: false,
                        });
                    }
                    Item::ConstDef(cdef) => {
                        let ty = self.resolve_type(&cdef.ty);
                        let qualified = format!("{}::{}", prefix, cdef.name);
                        self.symbols.define(Symbol {
                            name: qualified,
                            ty,
                            mutable: false,
                            span: cdef.span,
                            used: false,
                        });
                    }
                    Item::ModDecl(inner) => {
                        // Nested module: register with outer::inner:: prefix
                        let nested_prefix = format!("{}::{}", prefix, inner.name);
                        self.register_mod_items(&nested_prefix, &inner.body);
                    }
                    _ => {
                        self.register_item(item);
                    }
                }
            }
        }
    }

    /// Registers use declarations by aliasing qualified names to short names.
    fn register_use_decl(&mut self, use_decl: &UseDecl) {
        let path = &use_decl.path;
        match &use_decl.kind {
            UseKind::Simple => {
                if path.len() >= 2 {
                    let mod_path = path[..path.len() - 1].join("::");
                    let item_name = &path[path.len() - 1];
                    let qualified = format!("{}::{}", mod_path, item_name);
                    if let Some(sym) = self.symbols.lookup(&qualified) {
                        self.symbols.define(Symbol {
                            name: item_name.clone(),
                            ty: sym.ty.clone(),
                            mutable: false,
                            span: use_decl.span,
                            used: false,
                        });
                    }
                    // Track for unused import detection
                    self.imports.push((path.join("::"), use_decl.span, false));
                }
            }
            UseKind::Glob => {
                // Glob import: find all symbols with the module prefix
                // and register them with their short names
                let mod_path = path.join("::");
                let symbols = self.symbols.find_with_prefix(&mod_path);
                let prefix_len = mod_path.len() + 2; // "mod::" prefix
                for sym in symbols {
                    if sym.name.len() > prefix_len {
                        let short_name = sym.name[prefix_len..].to_string();
                        // Only import direct children (no nested ::)
                        if !short_name.contains("::") {
                            self.symbols.define(Symbol {
                                name: short_name,
                                ty: sym.ty.clone(),
                                mutable: false,
                                span: use_decl.span,
                                used: false,
                            });
                        }
                    }
                }
            }
            UseKind::Group(names) => {
                let mod_path = path.join("::");
                for name in names {
                    let qualified = format!("{}::{}", mod_path, name);
                    if let Some(sym) = self.symbols.lookup(&qualified) {
                        self.symbols.define(Symbol {
                            name: name.clone(),
                            ty: sym.ty.clone(),
                            mutable: false,
                            span: use_decl.span,
                            used: false,
                        });
                    }
                }
            }
        }
    }
}
