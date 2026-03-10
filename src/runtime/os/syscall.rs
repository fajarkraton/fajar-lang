//! Syscall table — definition, dispatch, standard syscall numbers.
//!
//! Provides a simulated system call interface for OS-level programming.
//! Syscall handlers are stored by number and dispatched with arguments.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Standard syscall numbers
// ═══════════════════════════════════════════════════════════════════════

/// Read from a file descriptor.
pub const SYS_READ: u64 = 0;
/// Write to a file descriptor.
pub const SYS_WRITE: u64 = 1;
/// Open a file.
pub const SYS_OPEN: u64 = 2;
/// Close a file descriptor.
pub const SYS_CLOSE: u64 = 3;
/// Get memory mapping.
pub const SYS_MMAP: u64 = 9;
/// Change memory protection.
pub const SYS_MPROTECT: u64 = 10;
/// Unmap memory.
pub const SYS_MUNMAP: u64 = 11;
/// Exit the process.
pub const SYS_EXIT: u64 = 60;
/// Get process ID.
pub const SYS_GETPID: u64 = 39;

// ═══════════════════════════════════════════════════════════════════════
// Syscall errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from syscall operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SyscallError {
    /// No handler registered for this syscall number.
    #[error("no handler for syscall {num}")]
    NoHandler { num: u64 },

    /// Syscall number already has a handler.
    #[error("syscall {num} already defined")]
    AlreadyDefined { num: u64 },

    /// Invalid argument to syscall.
    #[error("invalid argument for syscall {num}: {reason}")]
    InvalidArg { num: u64, reason: String },
}

// ═══════════════════════════════════════════════════════════════════════
// Syscall table
// ═══════════════════════════════════════════════════════════════════════

/// A registered syscall handler.
#[derive(Debug, Clone)]
pub struct SyscallHandler {
    /// Handler function name (looked up in interpreter).
    pub name: String,
    /// Expected number of arguments.
    pub arg_count: usize,
}

/// Simulated system call table.
///
/// Maps syscall numbers to handler definitions. The interpreter
/// resolves the handler name and calls it with the provided arguments.
#[derive(Debug)]
pub struct SyscallTable {
    /// Syscall number → handler definition.
    handlers: HashMap<u64, SyscallHandler>,
    /// Log of dispatched syscalls (for testing).
    dispatch_log: Vec<u64>,
}

impl SyscallTable {
    /// Creates a new empty syscall table.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            dispatch_log: Vec::new(),
        }
    }

    /// Defines a syscall handler for the given syscall number.
    pub fn define(
        &mut self,
        num: u64,
        handler_name: String,
        arg_count: usize,
    ) -> Result<(), SyscallError> {
        if self.handlers.contains_key(&num) {
            return Err(SyscallError::AlreadyDefined { num });
        }
        self.handlers.insert(
            num,
            SyscallHandler {
                name: handler_name,
                arg_count,
            },
        );
        Ok(())
    }

    /// Removes a syscall handler.
    pub fn undefine(&mut self, num: u64) -> Result<(), SyscallError> {
        if self.handlers.remove(&num).is_none() {
            return Err(SyscallError::NoHandler { num });
        }
        Ok(())
    }

    /// Dispatches a syscall, returning the handler info if registered.
    ///
    /// Validates argument count before returning the handler.
    pub fn dispatch(
        &mut self,
        num: u64,
        arg_count: usize,
    ) -> Result<&SyscallHandler, SyscallError> {
        match self.handlers.get(&num) {
            Some(handler) => {
                if arg_count != handler.arg_count {
                    return Err(SyscallError::InvalidArg {
                        num,
                        reason: format!("expected {} args, got {}", handler.arg_count, arg_count),
                    });
                }
                self.dispatch_log.push(num);
                Ok(handler)
            }
            None => Err(SyscallError::NoHandler { num }),
        }
    }

    /// Returns the handler for the given syscall, if defined.
    pub fn handler_for(&self, num: u64) -> Option<&SyscallHandler> {
        self.handlers.get(&num)
    }

    /// Returns the number of defined syscalls.
    pub fn syscall_count(&self) -> usize {
        self.handlers.len()
    }

    /// Returns the dispatch log (for testing).
    pub fn dispatch_log(&self) -> &[u64] {
        &self.dispatch_log
    }
}

impl Default for SyscallTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_table_is_empty() {
        let table = SyscallTable::new();
        assert_eq!(table.syscall_count(), 0);
    }

    #[test]
    fn define_and_lookup() {
        let mut table = SyscallTable::new();
        table.define(SYS_WRITE, "sys_write".into(), 3).unwrap();
        let handler = table.handler_for(SYS_WRITE).unwrap();
        assert_eq!(handler.name, "sys_write");
        assert_eq!(handler.arg_count, 3);
    }

    #[test]
    fn define_multiple() {
        let mut table = SyscallTable::new();
        table.define(SYS_READ, "sys_read".into(), 3).unwrap();
        table.define(SYS_WRITE, "sys_write".into(), 3).unwrap();
        table.define(SYS_EXIT, "sys_exit".into(), 1).unwrap();
        assert_eq!(table.syscall_count(), 3);
    }

    #[test]
    fn define_duplicate_fails() {
        let mut table = SyscallTable::new();
        table.define(SYS_WRITE, "sys_write".into(), 3).unwrap();
        assert!(matches!(
            table.define(SYS_WRITE, "other".into(), 3),
            Err(SyscallError::AlreadyDefined { num: SYS_WRITE })
        ));
    }

    #[test]
    fn undefine() {
        let mut table = SyscallTable::new();
        table.define(SYS_EXIT, "sys_exit".into(), 1).unwrap();
        table.undefine(SYS_EXIT).unwrap();
        assert_eq!(table.syscall_count(), 0);
    }

    #[test]
    fn undefine_nonexistent_fails() {
        let mut table = SyscallTable::new();
        assert!(matches!(
            table.undefine(SYS_READ),
            Err(SyscallError::NoHandler { .. })
        ));
    }

    #[test]
    fn dispatch_success() {
        let mut table = SyscallTable::new();
        table.define(SYS_WRITE, "sys_write".into(), 3).unwrap();
        let handler = table.dispatch(SYS_WRITE, 3).unwrap();
        assert_eq!(handler.name, "sys_write");
    }

    #[test]
    fn dispatch_wrong_arg_count() {
        let mut table = SyscallTable::new();
        table.define(SYS_WRITE, "sys_write".into(), 3).unwrap();
        assert!(matches!(
            table.dispatch(SYS_WRITE, 2),
            Err(SyscallError::InvalidArg { .. })
        ));
    }

    #[test]
    fn dispatch_no_handler() {
        let mut table = SyscallTable::new();
        assert!(matches!(
            table.dispatch(SYS_READ, 3),
            Err(SyscallError::NoHandler { .. })
        ));
    }

    #[test]
    fn dispatch_log_tracks_calls() {
        let mut table = SyscallTable::new();
        table.define(SYS_READ, "sys_read".into(), 3).unwrap();
        table.define(SYS_WRITE, "sys_write".into(), 3).unwrap();
        table.dispatch(SYS_WRITE, 3).unwrap();
        table.dispatch(SYS_READ, 3).unwrap();
        table.dispatch(SYS_WRITE, 3).unwrap();
        assert_eq!(table.dispatch_log(), &[SYS_WRITE, SYS_READ, SYS_WRITE]);
    }

    #[test]
    fn standard_syscall_numbers() {
        assert_eq!(SYS_READ, 0);
        assert_eq!(SYS_WRITE, 1);
        assert_eq!(SYS_OPEN, 2);
        assert_eq!(SYS_CLOSE, 3);
        assert_eq!(SYS_EXIT, 60);
        assert_eq!(SYS_GETPID, 39);
    }
}
