# FajarOS Nova Syscall Reference

> FajarOS Nova v1.4.0 "Zenith" -- x86_64 bare-metal OS written 100% in Fajar Lang
> 34 syscalls across 6 groups: Core, File I/O, Process, Pipe/Signal, IPC, GPU

---

## ABI Convention

Syscalls use the x86_64 `SYSCALL` instruction. The kernel configures MSRs
(`IA32_STAR`, `IA32_LSTAR`, `IA32_SFMASK`) at boot to route execution through
the entry stub at `0x8200`.

| Register | Purpose               |
|----------|-----------------------|
| `rax`    | Syscall number        |
| `rdi`    | Argument 0 (`arg0`)   |
| `rsi`    | Argument 1 (`arg1`)   |
| `rdx`    | Argument 2 (`arg2`)   |
| `rax`    | Return value          |
| `rcx`    | Clobbered (user RIP)  |
| `r11`    | Clobbered (user RFLAGS) |

On `SYSCALL`, the CPU saves the user RIP in `rcx` and RFLAGS in `r11`, clears
IF (interrupts disabled), and jumps to the stub. The stub performs `swapgs`,
saves the user stack pointer, switches to the kernel stack, dispatches via
`syscall_dispatch(num, arg0, arg1, arg2)`, then returns via `SYSRETQ`.

### Error Conventions

| Value | Meaning                                   |
|-------|-------------------------------------------|
| `0`   | Success (for syscalls that return status)  |
| `-1`  | Generic error / invalid argument / ENOMEM |
| `-2`  | Not found (IPC: queue full)               |
| `-3`  | IPC: no message available                 |
| `-4`  | IPC: target not in CALL state             |
| `-5`  | IPC: insufficient capability              |
| `-6`  | IPC: destination process is dead          |
| `-7`  | IPC: would block                          |

### File Descriptor Types

Each process has up to 16 file descriptors. FDs 0, 1, 2 are initialized to
`FD_CONSOLE` (stdin/stdout/stderr). The FD table lives at `0x8D0000` with
16-byte entries (8-byte type + 8-byte data).

| Type            | Value | Description                        |
|-----------------|-------|------------------------------------|
| `FD_CLOSED`     | 0     | Slot is unused                     |
| `FD_CONSOLE`    | 1     | VGA text console (serial fallback) |
| `FD_RAMFS`      | 2     | In-memory filesystem file          |
| `FD_PIPE_READ`  | 3     | Read end of a pipe                 |
| `FD_PIPE_WRITE` | 4     | Write end of a pipe                |
| `FD_FAT32`      | 5     | FAT32 filesystem file              |

---

## Core Syscalls (0-9)

### SYS_EXIT (0)

Terminate the current process.

```
Signature:  exit(status: i64) -> !
Registers:  rax=0, rdi=status
Returns:    Never returns
```

The kernel halts the CPU (`hlt` in a loop) for the calling process. In
multi-process mode, the process is marked `PROC_STATE_FREE` and the scheduler
selects the next runnable process.

**Errors:** None (always succeeds).

```fajar
@kernel fn user_main() {
    // ... program logic ...
    syscall(SYS_EXIT, 0, 0, 0)  // exit with status 0
}
```

---

### SYS_WRITE (1)

Write bytes to a file descriptor.

```
Signature:  write(fd: i64, buf: i64, len: i64) -> i64
Registers:  rax=1, rdi=fd, rsi=buf_addr, rdx=len
Returns:    Number of bytes written, or -1 on error
```

Behavior depends on the FD type:
- **FD_CONSOLE**: Writes each byte to the VGA text buffer via `console_putchar`.
- **FD_PIPE_WRITE**: Writes to the circular pipe buffer (4KB capacity).
- **FD_RAMFS**: Writes to the in-memory file at the current offset, extending
  the file size if the offset moves past the end.

**Errors:** Returns `-1` if `fd` is out of range (< 0 or >= 16) or the FD type
is `FD_CLOSED`, or the ramfs data pointer is null.

```fajar
let msg = "Hello, Nova!\n"
let written = syscall(SYS_WRITE, 1, msg as i64, 13)
```

---

### SYS_READ (2)

Read bytes from a file descriptor.

```
Signature:  read(fd: i64, buf: i64, len: i64) -> i64
Registers:  rax=2, rdi=fd, rsi=buf_addr, rdx=len
Returns:    Number of bytes read, or 0 on EOF/empty, or -1 on error
```

Behavior depends on the FD type:
- **FD_CONSOLE**: Reads one byte from the keyboard ring buffer (256-byte
  circular buffer at `0x6FB00`). Returns 0 if the buffer is empty.
- **FD_PIPE_READ**: Reads from the circular pipe buffer. Returns 0 on EOF
  (writer closed and buffer empty).
- **FD_RAMFS**: Reads up to `len` bytes from the current file offset. Returns
  0 when the offset reaches the file size.

**Errors:** Returns `-1` if `fd` is out of range or type is unsupported.

```fajar
let mut buf: [u8; 256] = [0; 256]
let n = syscall(SYS_READ, 0, buf as i64, 256)
```

---

### SYS_GETPID (3)

Return the PID of the calling process.

```
Signature:  getpid() -> i64
Registers:  rax=3
Returns:    Current process ID (0-15)
```

Reads the current PID from the global variable at `0x6FE00`.

**Errors:** None.

```fajar
let pid = syscall(SYS_GETPID, 0, 0, 0)
```

---

### SYS_YIELD (4)

Voluntarily yield the CPU to the scheduler.

```
Signature:  yield() -> i64
Registers:  rax=4
Returns:    0
```

Marks the calling process as `PROC_STATE_READY` so the scheduler can pick
another process. Always returns 0.

**Errors:** None.

```fajar
syscall(SYS_YIELD, 0, 0, 0)
```

---

### SYS_BRK (5)

Set or query the program break (heap boundary).

```
Signature:  brk(new_brk: i64) -> i64
Registers:  rax=5, rdi=new_brk
Returns:    New break address, or -1 on failure
```

If `new_brk` is 0, returns the current break (initializing to `0x2800000` if
unset). Otherwise, sets the break to `new_brk`, allocating and mapping new
pages as needed. The break must be between `USER_HEAP_BASE` (`0x2800000`) and
`ELF_STACK_TOP - 0x10000`.

**Errors:** Returns `-1` if `new_brk` is out of the valid range.

```fajar
let heap_start = syscall(SYS_BRK, 0, 0, 0)       // query current break
let new_end = syscall(SYS_BRK, heap_start + 4096, 0, 0)  // grow by 4KB
```

---

### SYS_MMAP (6)

Map anonymous pages into the process address space.

```
Signature:  mmap(addr: i64, len: i64, prot: i64) -> i64
Registers:  rax=6, rdi=addr, rsi=len, rdx=prot
Returns:    Virtual address of the mapped region
```

Allocates physical frames and maps them at `addr` (or `USER_HEAP_BASE +
0x100000` if `addr` is 0). Pages are mapped with `PAGE_PRESENT |
PAGE_WRITABLE | PAGE_USER`. The `prot` argument is accepted but currently all
mappings are read-write-user.

**Errors:** None currently (frame allocation failure is silent).

```fajar
let va = syscall(SYS_MMAP, 0, 8192, 0)  // map 2 pages
```

---

### SYS_CLOCK (7)

Return the system tick counter.

```
Signature:  clock() -> i64
Registers:  rax=7
Returns:    Current tick count (100 Hz timer, 10ms per tick)
```

Reads the 64-bit tick counter at `0x6FE08`, incremented by the PIT interrupt
handler.

**Errors:** None.

```fajar
let start = syscall(SYS_CLOCK, 0, 0, 0)
// ... work ...
let elapsed_ms = (syscall(SYS_CLOCK, 0, 0, 0) - start) * 10
```

---

### SYS_SLEEP (8)

Sleep for a specified number of milliseconds.

```
Signature:  sleep(ms: i64) -> i64
Registers:  rax=8, rdi=ms
Returns:    0
```

Computes the target tick, marks the process as `PROC_STATE_BLOCKED`, and
busy-waits until the tick counter reaches the target. The conversion is
`target = current_ticks + (ms + 9) / 10` (rounds up to the next 10ms tick).

**Errors:** None.

```fajar
syscall(SYS_SLEEP, 1000, 0, 0)  // sleep for ~1 second
```

---

### SYS_SBRK (9)

Increment the program break by a given amount.

```
Signature:  sbrk(increment: i64) -> i64
Registers:  rax=9, rdi=increment
Returns:    Previous break address (before the increment), or -1 on failure
```

This is a convenience wrapper around `SYS_BRK`. If `increment` is 0, returns
the current break without changing it. Otherwise calls `sys_brk(old +
increment)` and returns the old break address on success.

**Errors:** Returns `-1` if the underlying `sys_brk` fails.

```fajar
let ptr = syscall(SYS_SBRK, 1024, 0, 0)  // allocate 1024 bytes
```

---

## File I/O Syscalls (10-19)

### SYS_OPEN (10)

Open a file by path and return a file descriptor.

```
Signature:  open(path: i64, flags: i64) -> i64
Registers:  rax=10, rdi=path_addr, rsi=flags
Returns:    File descriptor (0-15), or -1 on error
```

Searches the ramfs for a file matching the null-terminated path string at
`path_addr`. The file must exist and must be a regular file (type 1, not a
directory). Allocates the lowest available FD and associates it with the ramfs
entry at offset 0.

**Errors:** Returns `-1` if the FD table is full, the file is not found, or
the path refers to a directory.

```fajar
let fd = syscall(SYS_OPEN, "/etc/config" as i64, 0)
```

---

### SYS_CLOSE (11)

Close a file descriptor.

```
Signature:  close(fd: i64) -> i64
Registers:  rax=11, rdi=fd
Returns:    0 on success, -1 on error
```

Marks the FD as `FD_CLOSED`. For pipe FDs, decrements the reference count and
frees the pipe slot if both ends are closed.

**Errors:** Returns `-1` if `fd` is out of range or already closed.

```fajar
syscall(SYS_CLOSE, fd, 0, 0)
```

---

### SYS_STAT (12)

Get file status by path.

```
Signature:  stat(path: i64, buf: i64) -> i64
Registers:  rax=12, rdi=path_addr, rsi=buf_addr
Returns:    0 on success, -1 if not found
```

Writes a 16-byte stat buffer: `[size: i64, type: i64]`. The type field is 1
for regular files and 2 for directories.

**Errors:** Returns `-1` if the path is not found in ramfs.

```fajar
let mut stat_buf: [i64; 2] = [0, 0]
syscall(SYS_STAT, "/hello.txt" as i64, stat_buf as i64)
let size = stat_buf[0]
let is_dir = stat_buf[1] == 2
```

---

### SYS_FSTAT (13)

Get file status by file descriptor.

```
Signature:  fstat(fd: i64, buf: i64) -> i64
Registers:  rax=13, rdi=fd, rsi=buf_addr
Returns:    0 on success, -1 on error
```

Same output format as `SYS_STAT`. For `FD_CONSOLE`, writes `[0, 0]`. For
`FD_RAMFS`, writes the file size and type from the ramfs entry.

**Errors:** Returns `-1` if `fd` is out of range or the FD type is not
`FD_CONSOLE` or `FD_RAMFS`.

---

### SYS_LSEEK (14)

Reposition the file offset for an open file descriptor.

```
Signature:  lseek(fd: i64, offset: i64, whence: i64) -> i64
Registers:  rax=14, rdi=fd, rsi=offset, rdx=whence
Returns:    New file offset, or -1 on error
```

| Whence     | Value | Meaning                    |
|------------|-------|----------------------------|
| `SEEK_SET` | 0     | Offset from start of file  |
| `SEEK_CUR` | 1     | Offset from current position |
| `SEEK_END` | 2     | Offset from end of file    |

Only valid for `FD_RAMFS` and `FD_FAT32` descriptors.

**Errors:** Returns `-1` if the FD type is not seekable, the resulting offset
is negative, or the FD is out of range.

```fajar
syscall(SYS_LSEEK, fd, 0, SEEK_SET)   // rewind to start
syscall(SYS_LSEEK, fd, -10, SEEK_END) // 10 bytes before EOF
```

---

### SYS_DUP (15)

Duplicate a file descriptor.

```
Signature:  dup(old_fd: i64) -> i64
Registers:  rax=15, rdi=old_fd
Returns:    New FD (lowest available), or -1 on error
```

Copies the FD type and data from `old_fd` to the lowest available FD slot.

**Errors:** Returns `-1` if `old_fd` is out of range, closed, or the FD table
is full.

```fajar
let fd2 = syscall(SYS_DUP, fd, 0, 0)
```

---

### SYS_DUP2 (16)

Duplicate a file descriptor to a specific slot.

```
Signature:  dup2(old_fd: i64, new_fd: i64) -> i64
Registers:  rax=16, rdi=old_fd, rsi=new_fd
Returns:    new_fd on success, or -1 on error
```

If `new_fd` is already open, it is closed first. If `old_fd == new_fd`,
returns `new_fd` without doing anything. Used for I/O redirection (e.g.,
redirecting stdout to a file).

**Errors:** Returns `-1` if either FD is out of range or `old_fd` is closed.

```fajar
syscall(SYS_DUP2, file_fd, 1, 0)  // redirect stdout to file
```

---

### SYS_GETCWD (17)

Get the current working directory.

```
Signature:  getcwd(buf: i64, size: i64) -> i64
Registers:  rax=17, rdi=buf_addr, rsi=size
Returns:    Length of the path string (excluding null), or 1 for "/"
```

Copies the process CWD (up to 31 characters) into the provided buffer. If the
CWD is empty or unset, writes "/" as the default.

**Errors:** None (always writes at least "/").

---

### SYS_CHDIR (18)

Change the current working directory.

```
Signature:  chdir(path: i64) -> i64
Registers:  rax=18, rdi=path_addr
Returns:    0
```

Copies the null-terminated path (up to 31 characters) into the process CWD
field. No validation is performed on whether the path exists.

**Errors:** None (always returns 0).

```fajar
syscall(SYS_CHDIR, "/home" as i64, 0, 0)
```

---

### SYS_UNLINK (19)

Delete a file from the ramfs.

```
Signature:  unlink(path: i64) -> i64
Registers:  rax=19, rdi=path_addr
Returns:    0 on success, -1 on error
```

Clears the ramfs entry (name, size, type) for the given path. Directories
(type 2) cannot be unlinked.

**Errors:** Returns `-1` if the path is not found or refers to a directory.

```fajar
syscall(SYS_UNLINK, "/tmp/scratch" as i64, 0, 0)
```

---

## Process Syscalls (20-22)

### SYS_FORK (20)

Create a child process by duplicating the calling process.

```
Signature:  fork() -> i64
Registers:  rax=20
Returns:    Child PID in parent, 0 in child, -1 on error
```

Creates a deep copy of the parent's page tables (Copy-on-Write), FD table,
and process fields (break, CWD, process group). The child's context frame is
a copy of the parent's with `rax` set to 0. The child gets a new kernel stack
and is placed in the `PROC_STATE_READY` queue.

Maximum 16 processes (`PROC_MAX`). PIDs are allocated from 1 upward.

**Errors:** Returns `-1` if no free PID is available or page table cloning
fails.

```fajar
let pid = syscall(SYS_FORK, 0, 0, 0)
if pid == 0 {
    // child process
    syscall(SYS_EXEC, "/bin/hello" as i64, 0)
} else {
    // parent process, pid = child's PID
    syscall(SYS_WAITPID, pid, 0, 0)
}
```

---

### SYS_EXEC (21)

Replace the current process image with a new ELF binary.

```
Signature:  exec(path: i64, argv: i64) -> i64
Registers:  rax=21, rdi=path_addr, rsi=argv_addr
Returns:    Does not return on success; -1 on error
```

Loads an ELF64 binary from ramfs or FAT32, parses PT_LOAD segments, maps them
into the process address space, and jumps to the ELF entry point in Ring 3.
Up to 16 arguments (`ARGV_MAX`) are copied from `argv_addr` (array of
null-terminated string pointers).

**Errors:** Returns `-1` if the file is not found, is empty, or ELF loading
fails.

---

### SYS_WAITPID (22)

Wait for a child process to change state.

```
Signature:  waitpid(pid: i64, status: i64, options: i64) -> i64
Registers:  rax=22, rdi=pid, rsi=status_addr, rdx=options
Returns:    PID of the child that changed state, or -1 on error
```

If `pid` is -1, waits for any child. If `pid` >= 0, waits for that specific
child. On return, writes the child's exit status to `status_addr` (if
non-null). The parent blocks (`PROC_STATE_BLOCKED`) until a child exits.

**Errors:** Returns `-1` if no matching child exists.

```fajar
let mut status: i64 = 0
let child = syscall(SYS_WAITPID, -1, &status as i64, 0)
```

---

## Pipe and Signal Syscalls (23-26)

### SYS_PIPE (23)

Create an anonymous pipe.

```
Signature:  pipe(fds_buf: i64) -> i64
Registers:  rax=23, rdi=fds_buf_addr
Returns:    0 on success, -1 on error
```

Creates a pipe (4KB circular buffer with refcounting) and allocates two FDs:
a read end (`FD_PIPE_READ`) and a write end (`FD_PIPE_WRITE`). Writes the
two FDs as `[read_fd: i64, write_fd: i64]` at `fds_buf_addr`.

**Errors:** Returns `-1` if pipe creation fails or no FD slots are available.

```fajar
let mut fds: [i64; 2] = [0, 0]
syscall(SYS_PIPE, fds as i64, 0, 0)
// fds[0] = read end, fds[1] = write end
```

---

### SYS_KILL (24)

Send a signal to a process.

```
Signature:  kill(pid: i64, signum: i64) -> i64
Registers:  rax=24, rdi=pid, rsi=signum
Returns:    0 on success, -1 on error
```

Delivers the specified signal to the target process. Signal numbers follow
POSIX conventions:

| Signal    | Value | Default Action       |
|-----------|-------|----------------------|
| `SIGHUP`  | 1     | Terminate            |
| `SIGINT`  | 2     | Terminate            |
| `SIGKILL` | 9     | Terminate (uncatchable) |
| `SIGSEGV` | 11    | Terminate            |
| `SIGTERM` | 15    | Terminate            |
| `SIGCHLD` | 17    | Ignore               |
| `SIGCONT` | 18    | Continue             |
| `SIGSTOP` | 19    | Stop (uncatchable)   |
| `SIGTSTP` | 20    | Stop                 |

**Errors:** Returns `-1` if `pid` is out of range or the process does not
exist.

```fajar
syscall(SYS_KILL, child_pid, SIGTERM, 0)
```

---

### SYS_SIGNAL (25)

Register a signal handler.

```
Signature:  signal(signum: i64, handler: i64) -> i64
Registers:  rax=25, rdi=signum, rsi=handler
Returns:    Previous handler address, or -1 on error
```

Sets the handler function pointer for the given signal. The special values
`SIG_DFL` (0) and `SIG_IGN` (1) restore the default action or ignore the
signal, respectively. `SIGKILL` and `SIGSTOP` cannot have custom handlers.

**Errors:** Returns `-1` if `signum` is `SIGKILL`, `SIGSTOP`, or unrecognized.

```fajar
let old = syscall(SYS_SIGNAL, SIGINT, my_handler as i64)
```

---

### SYS_SETPGID (26)

Set the process group ID.

```
Signature:  setpgid(pid: i64, pgid: i64) -> i64
Registers:  rax=26, rdi=pid, rsi=pgid
Returns:    0 on success, -1 on error
```

If `pid` is 0, operates on the calling process. Sets the process group ID in
the process table entry at offset `PROC_OFF_PGID`. Used for job control in
the shell.

**Errors:** Returns `-1` if the target PID is out of range.

```fajar
syscall(SYS_SETPGID, 0, child_pid, 0)  // set own pgid to child_pid
```

---

## IPC Syscalls (30-34)

FajarOS Nova uses synchronous rendezvous IPC inspired by L4/seL4. Each process
has an IPC endpoint (576 bytes at `0x8A0000`). Messages are 64 bytes:
`src_pid(8) + msg_type(4) + msg_id(4) + payload(40) + reserved(8)`.

All IPC syscalls are capability-checked: the calling process must hold the
appropriate capability bit (`CAP_IPC_SEND` or `CAP_IPC_RECV`).

### SYS_SEND (30)

Send a message to another process.

```
Signature:  send(dst_pid: i64, msg_addr: i64) -> i64
Registers:  rax=30, rdi=dst_pid, rsi=msg_addr
Returns:    0 on success, or negative error code
```

If the receiver is already blocked in `ipc2_recv()`, the message is
transferred directly (zero-copy rendezvous). Otherwise the message is queued
(up to 4 entries) and the sender blocks until the receiver picks it up.

**Errors:** `-1` invalid PID, `-2` queue full, `-5` no capability, `-6` dead
process.

---

### SYS_RECV (31)

Receive a message from another process.

```
Signature:  recv(from_pid: i64, buf_addr: i64) -> i64
Registers:  rax=31, rdi=from_pid, rsi=buf_addr
Returns:    Sender PID on success, or negative error code
```

If `from_pid` is -1, receives from any sender. If >= 0, receives only from
that specific process. Checks the message queue first, then checks if the
specified sender is blocked sending to us. If no message is available, the
receiver blocks.

**Errors:** `-1` invalid PID, `-3` no message (non-blocking path), `-5` no
capability.

---

### SYS_CALL_IPC (32)

Atomic send-then-receive (RPC call).

```
Signature:  call_ipc(dst_pid: i64, msg_addr: i64, reply_addr: i64) -> i64
Registers:  rax=32, rdi=dst_pid, rsi=msg_addr, rdx=reply_addr
Returns:    0 on success, or negative error code
```

Sends a message to `dst_pid` and immediately blocks waiting for a reply.
Combines `SYS_SEND` + `SYS_RECV` atomically, preventing race conditions in
client-server communication. The caller transitions to `IPC_CALL_BLOCKED`
state until the server calls `SYS_REPLY`.

**Errors:** `-1` invalid PID, `-5` no capability (requires both
`CAP_IPC_SEND` and `CAP_IPC_RECV`), `-6` dead process.

---

### SYS_REPLY (33)

Reply to a process that used SYS_CALL_IPC.

```
Signature:  reply(caller_pid: i64, reply_addr: i64) -> i64
Registers:  rax=33, rdi=caller_pid, rsi=reply_addr
Returns:    0 on success, or negative error code
```

Copies the 64-byte reply message to the caller's reply buffer and unblocks
the caller. The target process must be in `IPC_CALL_BLOCKED` state.

**Errors:** `-1` invalid PID, `-4` target not in CALL state, `-5` no
capability.

---

### SYS_NOTIFY (34)

Send an asynchronous notification (non-blocking).

```
Signature:  notify(dst_pid: i64, bits: i64) -> i64
Registers:  rax=34, rdi=dst_pid, rsi=bits
Returns:    0 on success, or negative error code
```

OR's the notification `bits` into the destination's `EP_NOTIFY_PENDING` field.
If the destination is blocked in `ipc2_recv()` and its notification mask
matches, the process is unblocked. Unlike `SYS_SEND`, notifications never
block the sender.

**Errors:** `-1` invalid PID, `-5` no capability, `-6` dead process.

```fajar
syscall(SYS_NOTIFY, server_pid, 0x01, 0)  // notify bit 0
```

---

## GPU Compute Syscalls (35-36)

FajarOS Nova provides in-kernel compute dispatch for matrix operations. Buffer
metadata is stored in the compute buffer table, and operations execute on the
CPU (GPU passthrough planned for VirtIO-GPU compute).

### SYS_GPU_ALLOC (35)

Allocate a compute buffer.

```
Signature:  gpu_alloc(rows: i64, cols: i64, dtype: i64) -> i64
Registers:  rax=35, rdi=rows, rsi=cols, rdx=dtype
Returns:    Buffer slot index (>= 0), or -1 on error
```

Allocates a buffer of `rows x cols` elements in the kernel compute region.
The `dtype` parameter is stored in metadata (currently integer i64 operations
only).

**Errors:** Returns `-1` if no buffer slots are available.

```fajar
let a = syscall(SYS_GPU_ALLOC, 4, 4, 0)   // 4x4 matrix
let b = syscall(SYS_GPU_ALLOC, 4, 4, 0)
let c = syscall(SYS_GPU_ALLOC, 4, 4, 0)   // result buffer
```

---

### SYS_GPU_DISPATCH (36)

Execute a compute kernel on allocated buffers.

```
Signature:  gpu_dispatch(kernel_id: i64, a: i64, b: i64, c: i64) -> i64
Registers:  rax=36, rdi=kernel_id, rsi=a_slot, rdx=b_slot
            (c_slot passed via stack or extended convention)
Returns:    0 on success, -1 on error
```

| Kernel ID       | Value | Operation                     |
|-----------------|-------|-------------------------------|
| `KERNEL_MATMUL` | 1     | C = A x B (matrix multiply)   |
| `KERNEL_VECADD` | 2     | C = A + B (element-wise add)  |
| `KERNEL_SCALE`  | 3     | Reserved                      |
| `KERNEL_RELU`   | 4     | Reserved                      |

For `KERNEL_MATMUL`: A must be `[m x n]`, B must be `[n x p]`, C receives
`[m x p]`. Returns `-1` on dimension mismatch.

**Errors:** Returns `-1` if `kernel_id` is unrecognized or dimensions are
incompatible.

```fajar
// Matrix multiply: C = A * B
syscall(SYS_GPU_DISPATCH, KERNEL_MATMUL, a, b)  // result in c
```

---

## Syscall Number Summary

| Number | Name           | Arguments                        | Returns                |
|--------|----------------|----------------------------------|------------------------|
| 0      | `SYS_EXIT`     | status                           | never                  |
| 1      | `SYS_WRITE`    | fd, buf, len                     | bytes written / -1     |
| 2      | `SYS_READ`     | fd, buf, len                     | bytes read / 0 / -1    |
| 3      | `SYS_GETPID`   | (none)                           | pid                    |
| 4      | `SYS_YIELD`    | (none)                           | 0                      |
| 5      | `SYS_BRK`      | new_brk                          | new_brk / -1           |
| 6      | `SYS_MMAP`     | addr, len, prot                  | virtual address        |
| 7      | `SYS_CLOCK`    | (none)                           | ticks (100 Hz)         |
| 8      | `SYS_SLEEP`    | ms                               | 0                      |
| 9      | `SYS_SBRK`     | increment                        | old_brk / -1           |
| 10     | `SYS_OPEN`     | path, flags                      | fd / -1                |
| 11     | `SYS_CLOSE`    | fd                               | 0 / -1                 |
| 12     | `SYS_STAT`     | path, buf                        | 0 / -1                 |
| 13     | `SYS_FSTAT`    | fd, buf                          | 0 / -1                 |
| 14     | `SYS_LSEEK`    | fd, offset, whence               | new_offset / -1        |
| 15     | `SYS_DUP`      | old_fd                           | new_fd / -1            |
| 16     | `SYS_DUP2`     | old_fd, new_fd                   | new_fd / -1            |
| 17     | `SYS_GETCWD`   | buf, size                        | length                 |
| 18     | `SYS_CHDIR`    | path                             | 0                      |
| 19     | `SYS_UNLINK`   | path                             | 0 / -1                 |
| 20     | `SYS_FORK`     | (none)                           | child_pid / 0 / -1     |
| 21     | `SYS_EXEC`     | path, argv                       | no return / -1         |
| 22     | `SYS_WAITPID`  | pid, status, options             | child_pid / -1         |
| 23     | `SYS_PIPE`     | fds_buf                          | 0 / -1                 |
| 24     | `SYS_KILL`     | pid, signum                      | 0 / -1                 |
| 25     | `SYS_SIGNAL`   | signum, handler                  | old_handler / -1       |
| 26     | `SYS_SETPGID`  | pid, pgid                        | 0 / -1                 |
| 27-29  | *(reserved)*   | --                               | --                     |
| 30     | `SYS_SEND`     | dst_pid, msg_addr                | 0 / error code         |
| 31     | `SYS_RECV`     | from_pid, buf_addr               | sender_pid / error     |
| 32     | `SYS_CALL_IPC` | dst_pid, msg_addr, reply_addr    | 0 / error code         |
| 33     | `SYS_REPLY`    | caller_pid, reply_addr           | 0 / error code         |
| 34     | `SYS_NOTIFY`   | dst_pid, bits                    | 0 / error code         |
| 35     | `SYS_GPU_ALLOC`| rows, cols, dtype                | slot / -1              |
| 36     | `SYS_GPU_DISPATCH` | kernel_id, a_slot, b_slot    | 0 / -1                 |

---

## Implementation Notes

- **Dispatch**: All syscalls route through `syscall_dispatch()` in
  `kernel/syscall/dispatch.fj`, which uses a chain of `if num == SYS_*`
  comparisons. Unrecognized numbers return `-1`.
- **Syscall table**: Located at `0x884000` (32 entries x 8 bytes). The
  dispatch function pointer is stored at `0x884008` for linker indirect calls.
- **Process table**: 16 processes at `PROC_TABLE`, each `PROC_ENTRY_SIZE`
  bytes. Fields include state, CR3, RSP, break, CWD, and PGID.
- **FD table V2**: At `0x8D0000`, 16 processes x 16 FDs x 16 bytes. FDs 0-2
  default to `FD_CONSOLE`.
- **IPC endpoints**: At `0x8A0000`, 16 endpoints x 576 bytes. Each has a
  4-entry message queue (72 bytes per entry: 8-byte PID + 64-byte message).
- **Pipe buffers**: 4KB circular buffers with separate read/write indices and
  reference counting at `0x8D4000`.
- **Signal table**: At `0x8D1000`, 16 processes x 64 bytes. Supports 8 signal
  slots with pending bits, mask, and per-signal handler pointers.
- **Compute buffers**: Kernel-managed buffer pool with metadata (rows, cols,
  dtype) and data regions for GPU/CPU compute dispatch.

---

*FajarOS Nova v1.4.0 "Zenith" -- 34 syscalls, 757 @kernel functions, 20,176 LOC*
*Source: kernel/syscall/dispatch.fj, kernel/syscall/entry.fj*
