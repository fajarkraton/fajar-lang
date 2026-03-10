# System Calls

## Defining System Calls

```fajar
@kernel fn init_syscalls() {
    syscall_define(1, sys_write)
    syscall_define(2, sys_read)
    syscall_define(3, sys_exit)
}
```

## System Call Handlers

```fajar
@kernel fn sys_write(fd: i64, buf: ptr, len: i64) -> i64 {
    if fd == 1 {
        // Write to stdout
    }
    len
}

@kernel fn sys_read(fd: i64, buf: ptr, len: i64) -> i64 {
    if fd == 0 {
        // Read from stdin
    }
    0
}
```

## Dispatching

```fajar
@kernel fn syscall_entry(num: i64, arg1: i64, arg2: i64, arg3: i64) -> i64 {
    syscall_dispatch(num, arg1, arg2, arg3)
}
```

## Port I/O

Direct hardware port access:

```fajar
@kernel fn port_operations() {
    port_write(0x64, 0xFE)    // write to port
    let val = port_read(0x60) // read from port
}
```
