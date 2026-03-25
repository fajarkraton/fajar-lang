# FajarOS Nova v0.9 — Security Audit Report

> **Date:** 2026-03-25
> **Auditor:** Claude Opus 4.6 + Fajar (PrimeCore.id)
> **Scope:** All 34 syscalls + kernel interfaces in fajaros_nova_kernel.fj (20,176 LOC)
> **Method:** Static analysis of volatile memory access patterns, bounds checking, permission enforcement

---

## Executive Summary

15 vulnerabilities found across 3 severity levels:
- **HIGH (7)**: Buffer overflow, integer overflow, out-of-bounds array access
- **MEDIUM (5)**: Permission bypass, race conditions, incomplete validation
- **LOW (3)**: Truncation, unchecked indices, implicit assumptions

Root causes: missing bounds validation on table accessors, unchecked PID/FD reads from fixed memory, incomplete permission framework.

---

## HIGH Severity (7)

### H1: Buffer overflow in sys_write (RAMFS)
- **Location:** sys_write → FD_RAMFS path
- **Issue:** No check that `offset + len <= file_buffer_size`. Writing past file data corrupts adjacent kernel memory.
- **Fix:** Add `if offset + len > RAMFS_DATA_SIZE { len = RAMFS_DATA_SIZE - offset }`

### H2: Out-of-bounds ramfs_entry_addr
- **Location:** ramfs_entry_addr(idx) used in sys_write, sys_read, sys_stat, sys_open, sys_unlink
- **Issue:** No bounds check on `idx`. If `idx >= 64`, reads/writes outside FS_BASE region.
- **Fix:** All callers must check `if idx < 0 || idx >= FS_MAX_FILES { return -1 }`

### H3: Integer overflow in sys_brk / sys_mmap
- **Location:** sys_brk, sys_mmap
- **Issue:** `new_brk` and page arithmetic could overflow, mapping pages into kernel region.
- **Fix:** Add explicit upper bound: `if new_brk > 0x7FFFFFFF { return -1 }`

### H4: Unchecked PID in fd_v2_addr
- **Location:** fd_v2_addr(pid, fd) — called from most syscalls
- **Issue:** PID from volatile_read_u64(0x6FE00) never validated < PROC_MAX before use as table index.
- **Fix:** Add `if pid >= PROC_MAX { return -1 }` at syscall entry

### H5: Unbounded file offset in sys_read/sys_write
- **Location:** FD data field stores packed (offset << 32 | idx)
- **Issue:** Negative offsets via lseek could read before file data.
- **Fix:** Validate `if offset < 0 { return -1 }`

### H6: Unchecked socket slot
- **Location:** socket_read/socket_write
- **Issue:** `slot` from fd_v2_get_data used without `slot < SOCKET_MAX` check.
- **Fix:** Add bounds check at function entry

### H7: Unchecked pipe slot
- **Location:** pipe_read_circular/pipe_write_circular
- **Issue:** `slot` used without `slot < PIPE_MAX` check.
- **Fix:** Add `if slot < 0 || slot >= PIPE_MAX { return -1 }`

---

## MEDIUM Severity (5)

### M1: Missing permission check in sys_chdir
- **Issue:** Any process can chdir to any directory. Should check directory execute permission.
- **Fix:** Add `fs_check_perm(entry_addr, 1)` (execute = traverse)

### M2: Wrong permission target in sys_unlink
- **Issue:** Checks write permission on FILE, should check write permission on DIRECTORY.
- **Fix:** Check parent directory write permission

### M3: Missing ownership check in sys_kill
- **Issue:** Any process can kill any other process. No UID-based authorization.
- **Fix:** Add `if sender_uid != 0 && sender_uid != target_uid { return -1 }`

### M4: Signal delivery race condition (SMP)
- **Issue:** Read-modify-write of signal pending bitmask not atomic. Under SMP, signals can be lost.
- **Fix:** Use atomic OR operation or spinlock around signal delivery

### M5: Silent CWD truncation in sys_getcwd
- **Issue:** Path silently truncated at 31 chars. Should return error if buffer too small.
- **Fix:** Return -ERANGE if path_len > size

---

## LOW Severity (3)

### L1: Command buffer overflow
- **Issue:** cmdbuf is 64 bytes but copy loops allow overwrite. Partially mitigated by `if len < 63` in shell.
- **Fix:** Enforce `len < 63` in all dispatch_command callers

### L2: Unchecked user table index
- **Issue:** user_find() return value used without `< USER_MAX` check.
- **Fix:** Add bounds check after user_find()

### L3: FD data field -1 corruption
- **Issue:** Closed FD returns data=-1, extracted as idx=0xFFFFFFFF, used as array index.
- **Fix:** Check `if ftype == FD_CLOSED { return -1 }` before extracting data fields

---

## Hardening Recommendations

### Sprint SA2: Immediate Hardening (10 tasks)

| # | Task | Status |
|---|------|--------|
| SA2.1 | Add bounds check to ramfs_entry_addr callers (H2) | [x] |
| SA2.2 | Add pipe slot bounds check (H7) | [x] |
| SA2.3 | Add socket slot bounds check (H6) | [x] |
| SA2.4 | Validate PID at syscall entry (H4) | [x] |
| SA2.5 | Add sys_brk upper bound (H3) | [x] |
| SA2.6 | Add file offset validation (H5) | [x] |
| SA2.7 | Add sys_write RAMFS length cap (H1) | [x] |
| SA2.8 | Add permission check to sys_chdir (M1) | [x] |
| SA2.9 | Add ownership check to sys_kill (M3) | [x] |
| SA2.10 | Enforce cmdbuf 63-byte limit (L1) | [x] |

### Future Hardening (v1.0)

| Feature | Description |
|---------|-------------|
| Stack canaries | Guard value at stack bottom per process |
| NX enforcement | Ensure data pages not executable |
| Kernel stack guard | Unmapped page between kernel stacks |
| Syscall number range check | Reject numbers > max |
| User pointer validation | Verify user pointers are in user space |
| Rate limiting | Limit fork/exec rate per user |
| Audit log | Log all privilege changes |
| Per-process capabilities | Capability bitmask |
| Seccomp-like filter | Per-process syscall whitelist |

---

## Risk Matrix

| Vulnerability | Exploitability | Impact | Risk |
|---------------|----------------|--------|------|
| H1: RAMFS write overflow | Easy | Kernel corruption | **CRITICAL** |
| H2: ramfs_entry OOB | Easy | Arbitrary R/W | **CRITICAL** |
| H3: brk integer overflow | Moderate | Page table corruption | **HIGH** |
| H4: PID unchecked | Easy | FD table corruption | **HIGH** |
| H5: Negative offset | Moderate | Info leak | **HIGH** |
| H6: Socket OOB | Moderate | Memory corruption | **HIGH** |
| H7: Pipe OOB | Moderate | Memory corruption | **HIGH** |
| M1: chdir no perms | Easy | Directory traversal | **MEDIUM** |
| M2: unlink wrong target | Easy | Unauthorized deletion | **MEDIUM** |
| M3: kill no ownership | Easy | DoS | **MEDIUM** |
| M4: Signal race | Hard (SMP only) | Missed signals | **LOW** |
| M5: getcwd truncation | Easy | Silent data loss | **LOW** |

---

## Conclusion

The Nova kernel has a solid foundation with @kernel context enforcement preventing the most dangerous class of bugs (heap in kernel, tensor in kernel). The vulnerabilities found are typical of early OS development: missing bounds checks and incomplete permission enforcement.

All HIGH severity issues have straightforward fixes (bounds checks at function entry). The MEDIUM issues require design decisions (permission framework, atomic operations). None require architectural changes.

**Recommendation:** Apply all SA2 hardening tasks before v1.0 release.

---

*Security Audit v0.9 — FajarOS Nova*
*Audited by Claude Opus 4.6 on 2026-03-25*
