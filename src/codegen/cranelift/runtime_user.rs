//! User-mode runtime library for FajarOS microkernel.
//!
//! Provides syscall wrappers for @safe and @device services.
//! These functions are linked into user-mode ELFs built with `--target x86_64-user`.
//!
//! Syscall convention (x86_64):
//!   RAX = syscall number
//!   RDI = arg0, RSI = arg1, RDX = arg2
//!   Return value in RAX

/// Raw syscall with 0 args.
#[inline(always)]
fn syscall0(num: i64) -> i64 {
    let ret: i64;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") num,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ret = -1;
    }
    ret
}

/// Raw syscall with 1 arg.
#[inline(always)]
fn syscall1(num: i64, arg0: i64) -> i64 {
    let ret: i64;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") num,
            in("rdi") arg0,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ret = -1;
    }
    ret
}

/// Raw syscall with 3 args.
#[inline(always)]
fn syscall3(num: i64, arg0: i64, arg1: i64, arg2: i64) -> i64 {
    let ret: i64;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") num,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ret = -1;
    }
    ret
}

// ── Syscall Constants (must match kernel/linker) ──

const SYS_EXIT: i64 = 0;
const SYS_WRITE: i64 = 1;
const SYS_READ: i64 = 2;
const SYS_GETPID: i64 = 4;
const SYS_YIELD: i64 = 5;
const SYS_IPC_SEND: i64 = 10;
const SYS_IPC_RECV: i64 = 11;
const SYS_IPC_CALL: i64 = 12;
const SYS_IPC_REPLY: i64 = 13;
const SYS_MMAP: i64 = 20;

// ── User-mode Runtime Functions ──
// These are called from Fajar Lang @safe code via the name mapping
// in runtime_fns.rs (fj_rt_user_* prefix)

/// Print a string to stdout (fd=1) via SYS_WRITE.
/// Called by @safe code via `println("hello")`.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_print(buf: *const u8, len: i64) -> i64 {
    syscall3(SYS_WRITE, 1, buf as i64, len)
}

/// Print an i64 value to stdout.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_print_i64(val: i64) {
    // Convert to decimal string
    if val == 0 {
        let zero = b"0";
        syscall3(SYS_WRITE, 1, zero.as_ptr() as i64, 1);
        return;
    }
    let mut buf = [0u8; 20];
    let mut n = if val < 0 { -val } else { val } as u64;
    let mut i = 19usize;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if i == 0 {
            break;
        }
        i -= 1;
    }
    if val < 0 {
        buf[i] = b'-';
    } else {
        i += 1;
    }
    let start = &buf[i..];
    syscall3(SYS_WRITE, 1, start.as_ptr() as i64, start.len() as i64);
}

/// Print a newline.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_println() {
    let nl = b"\n";
    syscall3(SYS_WRITE, 1, nl.as_ptr() as i64, 1);
}

/// Exit the process with given exit code.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_exit(code: i64) -> ! {
    syscall1(SYS_EXIT, code);
    // Should never reach here
    loop {}
}

/// Get current process ID.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_getpid() -> i64 {
    syscall0(SYS_GETPID)
}

/// Yield CPU to scheduler.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_yield() {
    syscall0(SYS_YIELD);
}

/// Read from stdin (fd=0).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_read(buf: *mut u8, len: i64) -> i64 {
    syscall3(SYS_READ, 0, buf as i64, len)
}

// ── IPC Syscall Wrappers ──

/// Send IPC message (64 bytes) to destination PID.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_ipc_send(dst_pid: i64, msg_ptr: i64) -> i64 {
    syscall3(SYS_IPC_SEND, dst_pid, msg_ptr, 64) // 64 bytes per message
}

/// Receive IPC message. Blocks until message arrives.
/// Returns sender PID, message written to buf_ptr.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_ipc_recv(src_filter: i64, buf_ptr: i64) -> i64 {
    syscall3(SYS_IPC_RECV, src_filter, buf_ptr, 64)
}

/// IPC call: send message and wait for reply (RPC pattern).
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_ipc_call(dst_pid: i64, msg_ptr: i64, reply_ptr: i64) -> i64 {
    syscall3(SYS_IPC_CALL, dst_pid, msg_ptr, reply_ptr)
}

/// Reply to a received IPC message.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_ipc_reply(dst_pid: i64, msg_ptr: i64) -> i64 {
    syscall3(SYS_IPC_REPLY, dst_pid, msg_ptr, 0)
}

/// Allocate memory pages.
#[unsafe(no_mangle)]
pub extern "C" fn fj_rt_user_mmap(addr: i64, len: i64, prot: i64) -> i64 {
    syscall3(SYS_MMAP, addr, len, prot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syscall_constants_match() {
        assert_eq!(SYS_EXIT, 0);
        assert_eq!(SYS_WRITE, 1);
        assert_eq!(SYS_IPC_SEND, 10);
    }
}
