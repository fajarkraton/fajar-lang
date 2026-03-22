# Fajar Lang Compiler Internals

## Two-Pass Compilation (Cranelift Backend)

### Pass 1: Declarations
1. Declare runtime functions (volatile_read/write, str_len, etc.)
2. Collect concrete functions from AST
3. Monomorphize generic specializations
4. **Dead Code Elimination**: compute reachable functions from entry points
   - JIT: entry = `main`, `@entry`, `@panic_handler`
   - AOT: entry = above + `kernel_main`, `fj_exception_*`
   - **Bare-metal (`no_std`): DCE DISABLED** — all functions declared
     (OS kernels use fn_addr, interrupt vectors invisible to static analysis)
5. Forward-declare all surviving functions in Cranelift module

### Pass 2: Definitions
1. Compile each function body to Cranelift IR
2. Cranelift optimizes + register-allocates
3. Emit machine code to JIT memory or object file

## Targets

| Target | Triple | Linker | Runtime |
|--------|--------|--------|---------|
| `x86_64-none` | x86_64-unknown-none | `ld` | Startup .o (inline asm) |
| `aarch64-none` | aarch64-unknown-none | `aarch64-linux-gnu-ld` | `libfj_runtime_bare.a` |
| `x86_64-user` | x86_64-unknown-none (user_mode) | `ld` | SYSCALL stubs |
| `host` | native | `cc` | Standard runtime |

## CLI Flags (Build)

| Flag | Description |
|------|-------------|
| `--target <triple>` | Cross-compile target |
| `--linker <path>` | Override linker binary |
| `--linker-script <path>` | Custom linker script |
| `--no-std` | Disable standard library |
| `--backend cranelift\|llvm` | Backend selection |
| `-v, --verbose` | Show compile time, source size, target |

## Runtime Libraries

### Bare-Metal (startup .o — x86_64)
Generated inline by compiler: volatile r/w, str_len, str_byte_at, print, println, memcpy, memset, timer, port I/O

### Bare-Metal (runtime .a — aarch64)
`runtime_bare/` crate: same functions as static library, plus ARM64 timer (CNTPCT_EL0), atomics, GIC stubs

### User-Mode (x86_64-user)
SYSCALL-based: println→SYS_WRITE, exit→SYS_EXIT, IPC send/recv/call/reply→SYS_SEND/RECV/CALL/REPLY

## Phase A+B Enhancements

| Fix | File | Effect |
|-----|------|--------|
| A.1: AOT DCE skip | mod.rs:11469 | FajarOS: zero CE006 |
| A.2: --linker flag | main.rs:1401 | ARM64 cross-compile |
| A.3: User-mode RT | mod.rs:7695 | Ring 3 service compile |
| A.4: Bare-metal atomics | mod.rs:7665 | SMP spinlocks |
| B.1: JIT DCE skip | mod.rs:4845 | Q6A JIT works |
| B.1: CE010 asm map | asm.rs:144 | pause→fence, hlt→trap |
| B.2: Runtime .a | runtime_bare/ | ARM64 bare-metal link |
| B.3: String builtins | mod.rs:7693 | str_concat, to_string |
| B.4: --verbose | main.rs:113 | Compile stats |
