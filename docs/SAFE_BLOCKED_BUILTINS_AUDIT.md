# `safe_blocked_builtins` audit — pure-functional carve-outs

**Date:** 2026-05-10 (post-v35.6.0 ship)
**Status:** AUDIT COMPLETE — 4 additional names carved out beyond v35.6.0's `str_byte_at`/`str_len`
**Predecessor:** `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §3 micro-gap "audit recommendation"

---

## TL;DR

v35.6.0 carved `str_byte_at` and `str_len` out of `safe_blocked_builtins`
because they were mis-categorized as hardware access (they're pure
Rust-`str` byte ops). This audit extends that work: every name in the
~150-entry `os_builtins` set was classified, and **4 additional names**
qualify as pure-functional carve-outs.

| Carved out | Why |
|---|---|
| `tensor_workload_hint` | Pure FLOP-count estimator (`rows * cols * rows`); no hw, no state |
| `cap_new` | Wraps a value in a `Cap<T>` (language-level type op) |
| `cap_unwrap` | Consumes a `Cap<T>` (linear-type semantics; no hw) |
| `cap_is_valid` | Read-only check on a `Cap<T>` (no hw) |

Total `safe_blocked_builtins` carve-outs since v35.6.0: **6 names**.

---

## §1. Carve-out criteria

A name qualifies for carve-out from `safe_blocked_builtins` only if **all
three** of the following hold:

1. **No real-world side effect** — calling the function does not modify
   any global/kernel/hardware state. Pure or restricted to its own
   arguments' state (e.g. linear-type consumption).
2. **Language-managed values only** — operates on values the language
   runtime owns (Rust `str`, language `Cap<T>`, language `[T]`), NOT
   raw memory or kernel data structures.
3. **Native codegen also emits no hw instruction** — when compiled to
   Cranelift/LLVM, the generated assembly is pure data manipulation,
   not a hardware port write or syscall instruction.

If ANY of those three is false, the name stays blocked under `@safe`.

---

## §2. Audit table — every name in `os_builtins`, classified

> Total: ~150 entries. Source: `src/analyzer/type_check/mod.rs:1439`.
> Categories: **HW** = hardware access · **KERN** = privileged kernel
> instruction · **MEM** = raw memory access · **PRIV-READ** = read of
> privileged state · **CARVED** = pure-functional, exception granted.

### Memory & paging
| Name | Class | Notes |
|------|-------|-------|
| `mem_alloc`, `mem_free` | MEM | Heap allocation |
| `mem_read_u8/u32/u64`, `mem_write_u8/u32/u64` | MEM | Raw memory access |
| `page_map`, `page_unmap` | KERN | MMU manipulation |

### Interrupts & syscalls
| Name | Class | Notes |
|------|-------|-------|
| `irq_register/unregister/enable/disable` | KERN | IRQ ctrl |
| `syscall_define/dispatch/init`, `svc` | KERN | Syscall ABI |
| `int_n` | KERN | Software interrupt |

### I/O ports & serial
| Name | Class | Notes |
|------|-------|-------|
| `port_read/write/inb/outb/inw/outw/ind/outd` | HW | I/O instructions |
| `x86_serial_init`, `set_uart_mode_x86` | HW | Serial init |

### CPU control
| Name | Class | Notes |
|------|-------|-------|
| `cpuid_eax/ebx/ecx/edx`, `cpuid` | PRIV-READ | Returns CPU info; emits CPUID instruction → KEEP BLOCKED |
| `sse_enable` | KERN | CR4 write |
| `read_cr0/cr2/cr3/cr4`, `write_cr3/cr4` | KERN | Control register access |
| `idt_init`, `pic_remap`, `pic_eoi`, `pit_init` | KERN | Interrupt ctrl |
| `read_timer_ticks` | PRIV-READ | TSC/timer read → KEEP BLOCKED |
| `read_msr`, `rdmsr`, `write_msr`, `wrmsr` | KERN | MSR access |
| `rdtsc` | PRIV-READ | TSC instruction → KEEP BLOCKED |
| `fxsave`, `fxrstor`, `iretq_to_user`, `rdrand` | KERN | Privileged CPU |
| `hlt`, `cli`, `sti`, `swapgs`, `pause`, `stac`, `clac` | KERN | Privileged CPU |
| `ltr`, `lgdt_mem`, `lidt_mem` | KERN | Descriptor table loads |
| `read_ttbr0`, `switch_ttbr0`, `tlbi_va`, `invlpg`, `memory_fence` | KERN | MMU/TLB |

### Process scheduling
| Name | Class | Notes |
|------|-------|-------|
| `proc_table_addr` | KERN | Exposes kernel data ptr; info leak |
| `get_current_pid`, `get_proc_count`, `set_current_pid` | KERN | Kernel state read/write |
| `proc_create`, `proc_create_user`, `yield_proc`, `tss_init` | KERN | Process ctrl |
| `proc_spawn`, `proc_wait`, `proc_kill`, `proc_self`, `proc_yield` | KERN | Process ctrl |
| `sched_get_saved_sp`, `sched_set_next_sp`, `sched_read_proc`, `sched_write_proc` | KERN | Scheduler |
| `syscall_arg0/1/2`, `syscall_set_return` | KERN | Syscall ABI |
| **`schedule_ai_task`** | KERN | Modifies AI scheduler state — KEEP BLOCKED (despite simulation stub being pure) |

### String / buffer (mixed)
| Name | Class | Notes |
|------|-------|-------|
| **`str_byte_at`** | **CARVED (v35.6.0)** | Pure Rust-`str` byte read |
| **`str_len`** | **CARVED (v35.6.0)** | Pure Rust-`str` length |
| `buffer_read_u16/u32/u64_le/be`, `buffer_write_*_le/be` | MEM | Raw buffer offset r/w |
| `memcmp_buf`, `memcpy_buf`, `memset_buf` | MEM | Raw buffer ops |

### Bus & devices
| Name | Class | Notes |
|------|-------|-------|
| `pci_read32`, `pci_write32` | HW | PCI config space |
| `volatile_read/write*` (8/16/32/64) | MEM | Volatile memory access |
| `acpi_shutdown`, `acpi_find_rsdp`, `acpi_get_cpu_count` | HW | ACPI firmware |

### HAL peripherals (Phase 3-8)
| Name | Class | Notes |
|------|-------|-------|
| `gpio_*` | HW | GPIO pins |
| `uart_init`, `uart_available` | HW | Serial peripheral |
| `spi_init`, `spi_cs_set`, `i2c_init` | HW | Bus peripherals |
| `timer_*`, `sleep_us`, `sleep_ms`, `time_since_boot`, `timer_mark_boot` | HW | Timer peripherals |
| `dma_*` | HW | DMA controller |
| `kb_init`, `kb_read*`, `kb_has_data`, `kb_available` | HW | Keyboard |
| `nvme_*`, `sd_*`, `vfs_*` | HW | Storage |
| `eth_init`, `net_*`, `http_listen` | HW | Network |
| `fb_init`, `fb_*`, `fb_set_base`, `fb_scroll` | HW | Framebuffer |

### Power / system
| Name | Class | Notes |
|------|-------|-------|
| `sys_poweroff`, `sys_reboot` | HW | Power ctrl |
| `sys_cpu_temp`, `sys_ram_total`, `sys_ram_free` | PRIV-READ | Sensor/info read → KEEP BLOCKED (info leak + likely native syscall) |

### V27.5 AI / Capability builtins
| Name | Class | Notes |
|------|-------|-------|
| **`tensor_workload_hint`** | **CARVED** | Pure math: `rows * cols * rows` (FLOP estimator). No state, no hw. |
| `schedule_ai_task` | KERN | Affects scheduler state |
| **`cap_new`** | **CARVED** | Wraps value in language-level `Cap<T>` |
| **`cap_unwrap`** | **CARVED** | Consumes `Cap<T>` (linear-type op; no hw) |
| **`cap_is_valid`** | **CARVED** | Read-only check on `Cap<T>` |
| `fn_addr` | PRIV-READ | Returns fn pointer; risk: enables raw-ptr ops downstream → KEEP BLOCKED |

---

## §3. Reproduction (gates after carve-out batch 2)

```bash
cd "/home/primecore/Documents/Fajar Lang"
cargo test --lib                                                   # 7,633 / 7,633
cargo test --release --test selfhost_stage1_full -- --test-threads=1   # 86 / 86
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1  # 4 / 4 (Stage 2 byte-equality preserved)
cargo clippy --lib -- -D warnings                                   # clean
cargo fmt -- --check                                                # clean
```

---

## §4. Why these specifically

### `tensor_workload_hint`

```rust
// src/interpreter/eval/builtins.rs:3027
"tensor_workload_hint" => {
    let rows = match args.first() { Some(Value::Int(n)) => *n, _ => 0 };
    let cols = match args.get(1) { Some(Value::Int(n)) => *n, _ => 0 };
    Ok(Value::Int(rows * cols * rows))
}
```

Pure math. The function exists to give @safe ML code a way to estimate
compute cost before calling into `@kernel` AI scheduling primitives.
Blocking it from @safe defeats the purpose.

### `cap_new` / `cap_unwrap` / `cap_is_valid`

```rust
// src/interpreter/eval/builtins.rs:2990
"cap_new" => Ok(Value::Cap { inner: Arc::new(Mutex::new(Some(val))) })
"cap_unwrap" => match args { Some(Value::Cap { inner }) => guard.take(), … }
"cap_is_valid" => match args { Some(Value::Cap { inner }) => …read… }
```

These are language-level type operations on the `Cap<T>` type — exactly
analogous to `Option::Some`, `Option::take`, `Option::is_some`. Treating
them as hw access blocks legitimate @safe code that wants to use
linear-type capabilities for resource management (which is what the
type was designed for).

### Why NOT `schedule_ai_task`

Despite the interpreter stub being pure math (`1000 - priority * 100 +
task_id`), the *intent* of `schedule_ai_task` is to enqueue an AI task
into the kernel scheduler. The simulation stub doesn't enqueue anything
because there's no scheduler in the interpreter, but the language-level
semantic is "schedule a task" — that's a side effect on kernel state.
Native codegen would call into the scheduler. KEEP BLOCKED.

### Why NOT `cpuid_*` / `rdtsc` / `read_msr`

Native codegen emits the literal CPU instruction (CPUID, RDTSC, RDMSR).
While the result is deterministic-readonly of CPU state, the *act* of
executing a privileged CPU instruction inside @safe code violates the
microkernel-isolation contract. KEEP BLOCKED.

### Why NOT `sys_cpu_temp` / `sys_ram_total` / `sys_ram_free`

Sensor/info reads. Pure-read in language semantics, but native
implementations call into kernel/firmware (ACPI, sysfs, /proc/meminfo,
etc.). The `@safe → @kernel` bridge per D-α is the right path: write a
trivial `@kernel fn cpu_temp_safe()` wrapper if you need this from
@safe code. KEEP BLOCKED.

---

## §5. Migration impact

**Zero.** All 4 newly-carved names already pass through @safe code
without firing SE020 — but only because the test sources annotated
`fn main` as `@kernel` to bypass the issue (e.g.,
`p_all_features_coexist_in_one_program`'s `@kernel fn main()` wraps
both `tensor_workload_hint` and `cap_*` calls). Those `@kernel`
annotations remain valid (more strict ≠ wrong); they're just no longer
*required* for these specific builtins.

A follow-up "remove unnecessary @kernel" pass on test sources is
optional and low-priority — leaving the explicit annotations in place
documents intent and remains correct.

---

## §6. Future audit notes

The audit pattern locked in for future analyzer-rule additions:

1. When adding a builtin to `os_builtins`, check whether it satisfies
   the 3-criteria carve-out test (§1). If yes, add to the carve-out
   list with a one-line justification.
2. Periodically re-audit (suggested: every minor version bump that
   touches `os_builtins` definition). The set drifts as new
   peripherals/AI/cap features land.
3. Keep the carve-out list in `mod.rs` next to the `os_builtins`
   definition with explanatory comments — close to where surface bugs
   would be introduced.

---

## §7. Disposition

- ✅ Code change committed: 4 additional `safe_blocked_builtins.remove()` calls.
- ✅ Comment block in `mod.rs` documents the carve-out criteria + list.
- ✅ Engineering gates: 7,633 lib + 86/86 stage1_full + 4/4 phase17 byte-equality + clippy/fmt clean.
- ⏸️ Optional follow-up: revert `@kernel fn main()` → bare `fn main()` on test sources where the `@kernel` was added solely to bypass the now-carved builtins. Low-priority.
