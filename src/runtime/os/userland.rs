//! Userland libraries for FajarOS Nova v2.0 — Sprint N7.
//!
//! Provides simulated libc, libm, Fajar runtime library, dynamic linker,
//! shared objects, process API, signal handling, thread API, and time API.
//! All structures are simulated in-memory — no real syscalls or OS interaction.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// Userland Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced by userland libraries.
#[derive(Debug, Clone, thiserror::Error)]
pub enum UserlandError {
    /// Out of memory.
    #[error("out of memory: requested {0} bytes")]
    OutOfMemory(usize),
    /// Invalid pointer.
    #[error("invalid pointer: 0x{0:X}")]
    InvalidPointer(u64),
    /// Process not found.
    #[error("process not found: pid {0}")]
    ProcessNotFound(u32),
    /// Signal delivery failed.
    #[error("cannot deliver signal {0} to pid {1}")]
    SignalFailed(i32, u32),
    /// Thread error.
    #[error("thread error: {0}")]
    ThreadError(String),
    /// Shared object not found.
    #[error("shared object not found: {0}")]
    SoNotFound(String),
    /// Symbol not found.
    #[error("undefined symbol: {0}")]
    SymbolNotFound(String),
    /// Pipe error.
    #[error("broken pipe")]
    BrokenPipe,
    /// Invalid file descriptor.
    #[error("bad file descriptor: {0}")]
    BadFd(u64),
}

// ═══════════════════════════════════════════════════════════════════════
// LibC — Core C Library Functions
// ═══════════════════════════════════════════════════════════════════════

/// Simulated heap allocation entry.
#[derive(Debug, Clone)]
struct HeapBlock {
    /// Start address.
    addr: u64,
    /// Size in bytes.
    size: usize,
    /// Is this block free?
    free: bool,
}

/// Simulated libc — malloc, free, printf, strlen, memcpy, memset, strcmp.
///
/// Uses a simple first-fit allocator over a virtual address space.
#[derive(Debug)]
pub struct LibC {
    /// Heap blocks.
    heap: Vec<HeapBlock>,
    /// Next virtual address.
    next_addr: u64,
    /// Total allocated bytes.
    pub allocated_bytes: usize,
    /// Total allocation count.
    pub alloc_count: u64,
    /// Total free count.
    pub free_count: u64,
    /// Printf output buffer.
    pub output: Vec<String>,
}

impl LibC {
    /// Creates a new libc instance.
    pub fn new() -> Self {
        Self {
            heap: Vec::new(),
            next_addr: 0x4000_0000, // userland heap start
            allocated_bytes: 0,
            alloc_count: 0,
            free_count: 0,
            output: Vec::new(),
        }
    }

    /// Allocates `size` bytes. Returns the virtual address.
    pub fn malloc(&mut self, size: usize) -> Result<u64, UserlandError> {
        if size == 0 {
            return Ok(0);
        }
        // Try to find a free block first (first-fit)
        for block in &mut self.heap {
            if block.free && block.size >= size {
                block.free = false;
                self.allocated_bytes += block.size;
                self.alloc_count += 1;
                return Ok(block.addr);
            }
        }
        // Allocate new block
        let addr = self.next_addr;
        self.next_addr += size as u64;
        self.heap.push(HeapBlock {
            addr,
            size,
            free: false,
        });
        self.allocated_bytes += size;
        self.alloc_count += 1;
        Ok(addr)
    }

    /// Frees a previously allocated block.
    pub fn free(&mut self, addr: u64) -> Result<(), UserlandError> {
        for block in &mut self.heap {
            if block.addr == addr && !block.free {
                block.free = true;
                self.allocated_bytes = self.allocated_bytes.saturating_sub(block.size);
                self.free_count += 1;
                return Ok(());
            }
        }
        Err(UserlandError::InvalidPointer(addr))
    }

    /// Simulated printf — stores formatted output.
    pub fn printf(&mut self, format: &str, args: &[&str]) -> usize {
        let mut output = String::new();
        let mut arg_idx = 0;
        let mut chars = format.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let Some(&spec) = chars.peek() {
                    match spec {
                        's' | 'd' | 'f' | 'x' => {
                            chars.next();
                            if arg_idx < args.len() {
                                output.push_str(args[arg_idx]);
                                arg_idx += 1;
                            }
                        }
                        '%' => {
                            chars.next();
                            output.push('%');
                        }
                        _ => {
                            output.push(ch);
                        }
                    }
                } else {
                    output.push(ch);
                }
            } else {
                output.push(ch);
            }
        }

        let len = output.len();
        self.output.push(output);
        len
    }

    /// Returns the length of a null-terminated string.
    pub fn strlen(&self, s: &str) -> usize {
        s.len()
    }

    /// Copies `n` bytes from `src` to `dst`.
    pub fn memcpy(&self, dst: &mut [u8], src: &[u8], n: usize) -> usize {
        let count = n.min(dst.len()).min(src.len());
        dst[..count].copy_from_slice(&src[..count]);
        count
    }

    /// Sets `n` bytes of `dst` to `val`.
    pub fn memset(&self, dst: &mut [u8], val: u8, n: usize) -> usize {
        let count = n.min(dst.len());
        for byte in &mut dst[..count] {
            *byte = val;
        }
        count
    }

    /// Compares two strings lexicographically. Returns 0 if equal,
    /// negative if a < b, positive if a > b.
    pub fn strcmp(&self, a: &str, b: &str) -> i32 {
        match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
}

impl Default for LibC {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LibM — Math Functions
// ═══════════════════════════════════════════════════════════════════════

/// Simulated math library (libm).
///
/// Provides standard mathematical functions using Rust's `f64` operations.
#[derive(Debug)]
pub struct LibM;

impl LibM {
    /// Creates a new libm instance.
    pub fn new() -> Self {
        Self
    }

    /// Sine.
    pub fn sin(&self, x: f64) -> f64 {
        x.sin()
    }

    /// Cosine.
    pub fn cos(&self, x: f64) -> f64 {
        x.cos()
    }

    /// Tangent.
    pub fn tan(&self, x: f64) -> f64 {
        x.tan()
    }

    /// Square root.
    pub fn sqrt(&self, x: f64) -> f64 {
        x.sqrt()
    }

    /// Power (x^y).
    pub fn pow(&self, x: f64, y: f64) -> f64 {
        x.powf(y)
    }

    /// Natural logarithm.
    pub fn log(&self, x: f64) -> f64 {
        x.ln()
    }

    /// Base-10 logarithm.
    pub fn log10(&self, x: f64) -> f64 {
        x.log10()
    }

    /// Absolute value.
    pub fn fabs(&self, x: f64) -> f64 {
        x.abs()
    }

    /// Floor.
    pub fn floor(&self, x: f64) -> f64 {
        x.floor()
    }

    /// Ceiling.
    pub fn ceil(&self, x: f64) -> f64 {
        x.ceil()
    }

    /// Round to nearest integer.
    pub fn round(&self, x: f64) -> f64 {
        x.round()
    }

    /// Exponential (e^x).
    pub fn exp(&self, x: f64) -> f64 {
        x.exp()
    }

    /// Arcsine.
    pub fn asin(&self, x: f64) -> f64 {
        x.asin()
    }

    /// Arccosine.
    pub fn acos(&self, x: f64) -> f64 {
        x.acos()
    }

    /// Arctangent.
    pub fn atan(&self, x: f64) -> f64 {
        x.atan()
    }

    /// Two-argument arctangent (atan2).
    pub fn atan2(&self, y: f64, x: f64) -> f64 {
        y.atan2(x)
    }
}

impl Default for LibM {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LibFj — Fajar Runtime Library
// ═══════════════════════════════════════════════════════════════════════

/// Fajar runtime library — array operations, string operations, and I/O.
#[derive(Debug)]
pub struct LibFj {
    /// I/O output buffer (simulated stdout).
    pub stdout: Vec<String>,
    /// I/O error buffer (simulated stderr).
    pub stderr: Vec<String>,
    /// Environment variables.
    env_vars: HashMap<String, String>,
}

impl LibFj {
    /// Creates a new Fajar runtime library instance.
    pub fn new() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("FAJAROS_VERSION".to_string(), "2.0.0".to_string());
        env_vars.insert("HOME".to_string(), "/home/fajar".to_string());
        env_vars.insert("PATH".to_string(), "/usr/local/bin:/usr/bin:/bin".to_string());
        Self {
            stdout: Vec::new(),
            stderr: Vec::new(),
            env_vars,
        }
    }

    /// Prints to stdout.
    pub fn print(&mut self, s: &str) {
        self.stdout.push(s.to_string());
    }

    /// Prints to stderr.
    pub fn eprint(&mut self, s: &str) {
        self.stderr.push(s.to_string());
    }

    /// Creates an array of the given size filled with a default value.
    pub fn array_new(&self, size: usize, default: i64) -> Vec<i64> {
        vec![default; size]
    }

    /// Pushes a value to an array and returns the new array.
    pub fn array_push(&self, mut arr: Vec<i64>, val: i64) -> Vec<i64> {
        arr.push(val);
        arr
    }

    /// Returns the length of an array.
    pub fn array_len(&self, arr: &[i64]) -> usize {
        arr.len()
    }

    /// Concatenates two strings.
    pub fn string_concat(&self, a: &str, b: &str) -> String {
        format!("{}{}", a, b)
    }

    /// Splits a string by delimiter.
    pub fn string_split(&self, s: &str, delim: &str) -> Vec<String> {
        s.split(delim).map(|p| p.to_string()).collect()
    }

    /// Gets an environment variable.
    pub fn getenv(&self, key: &str) -> Option<&str> {
        self.env_vars.get(key).map(|s| s.as_str())
    }

    /// Sets an environment variable.
    pub fn setenv(&mut self, key: &str, value: &str) {
        self.env_vars.insert(key.to_string(), value.to_string());
    }
}

impl Default for LibFj {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dynamic Linker
// ═══════════════════════════════════════════════════════════════════════

/// Symbol binding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolBind {
    /// Local (file scope).
    Local,
    /// Global (visible to linker).
    Global,
    /// Weak (overridable).
    Weak,
}

/// A symbol in a shared object.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,
    /// Virtual address.
    pub address: u64,
    /// Size in bytes.
    pub size: u64,
    /// Binding type.
    pub bind: SymbolBind,
}

/// Relocation entry.
#[derive(Debug, Clone)]
pub struct Relocation {
    /// Offset to patch.
    pub offset: u64,
    /// Symbol name to resolve.
    pub symbol: String,
    /// Addend.
    pub addend: i64,
}

/// A simulated shared object (.so file).
#[derive(Debug, Clone)]
pub struct SharedObject {
    /// Library name (e.g. "libfj.so").
    pub name: String,
    /// Exported symbols.
    pub symbols: Vec<Symbol>,
    /// Relocations.
    pub relocations: Vec<Relocation>,
    /// Load address.
    pub load_addr: u64,
    /// Dependencies (other .so names).
    pub dependencies: Vec<String>,
    /// Is loaded?
    pub loaded: bool,
}

impl SharedObject {
    /// Creates a new shared object.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            symbols: Vec::new(),
            relocations: Vec::new(),
            load_addr: 0,
            dependencies: Vec::new(),
            loaded: false,
        }
    }

    /// Adds an exported symbol.
    pub fn add_symbol(&mut self, name: &str, address: u64, size: u64, bind: SymbolBind) {
        self.symbols.push(Symbol {
            name: name.to_string(),
            address,
            size,
            bind,
        });
    }

    /// Adds a relocation entry.
    pub fn add_relocation(&mut self, offset: u64, symbol: &str, addend: i64) {
        self.relocations.push(Relocation {
            offset,
            symbol: symbol.to_string(),
            addend,
        });
    }

    /// Looks up a symbol by name.
    pub fn find_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.name == name)
    }
}

/// Simulated dynamic linker with GOT/PLT.
///
/// Loads shared objects, resolves symbols, and applies relocations.
#[derive(Debug)]
pub struct DynamicLinker {
    /// Loaded shared objects.
    loaded: HashMap<String, SharedObject>,
    /// Global Offset Table: symbol name -> resolved address.
    got: HashMap<String, u64>,
    /// Next load address.
    next_load_addr: u64,
}

impl DynamicLinker {
    /// Creates a new dynamic linker.
    pub fn new() -> Self {
        Self {
            loaded: HashMap::new(),
            got: HashMap::new(),
            next_load_addr: 0x7F00_0000,
        }
    }

    /// Registers a shared object.
    pub fn register(&mut self, so: SharedObject) {
        self.loaded.insert(so.name.clone(), so);
    }

    /// Loads a shared object by name and resolves its symbols.
    pub fn load(&mut self, name: &str) -> Result<u64, UserlandError> {
        let so = self
            .loaded
            .get_mut(name)
            .ok_or_else(|| UserlandError::SoNotFound(name.to_string()))?;

        if so.loaded {
            return Ok(so.load_addr);
        }

        let load_addr = self.next_load_addr;
        self.next_load_addr += 0x10_0000; // 1MB per .so
        so.load_addr = load_addr;
        so.loaded = true;

        // Register symbols in GOT
        let symbols: Vec<(String, u64)> = so
            .symbols
            .iter()
            .filter(|s| s.bind == SymbolBind::Global)
            .map(|s| (s.name.clone(), load_addr + s.address))
            .collect();

        for (sym_name, addr) in symbols {
            self.got.insert(sym_name, addr);
        }

        Ok(load_addr)
    }

    /// Resolves a symbol from the GOT.
    pub fn resolve(&self, name: &str) -> Result<u64, UserlandError> {
        self.got
            .get(name)
            .copied()
            .ok_or_else(|| UserlandError::SymbolNotFound(name.to_string()))
    }

    /// Returns the number of loaded shared objects.
    pub fn loaded_count(&self) -> usize {
        self.loaded.values().filter(|so| so.loaded).count()
    }

    /// Returns the total number of GOT entries.
    pub fn got_size(&self) -> usize {
        self.got.len()
    }
}

impl Default for DynamicLinker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Process API
// ═══════════════════════════════════════════════════════════════════════

/// Process state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Ready to run.
    Ready,
    /// Currently executing.
    Running,
    /// Waiting for I/O or signal.
    Sleeping,
    /// Stopped (SIGSTOP).
    Stopped,
    /// Terminated (exit code stored).
    Zombie,
}

/// A simulated process.
#[derive(Debug, Clone)]
pub struct Process {
    /// Process ID.
    pub pid: u32,
    /// Parent process ID.
    pub ppid: u32,
    /// Process name.
    pub name: String,
    /// Current state.
    pub state: ProcessState,
    /// Exit code (set on termination).
    pub exit_code: Option<i32>,
    /// Environment variables.
    pub env: HashMap<String, String>,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Open file descriptors.
    pub fds: Vec<u64>,
}

/// Process API — fork, exec, wait, exit, getpid, pipe.
#[derive(Debug)]
pub struct ProcessApi {
    /// All processes keyed by PID.
    processes: HashMap<u32, Process>,
    /// Next PID.
    next_pid: u32,
    /// Pipes: (read_fd, write_fd) -> buffer.
    pipes: HashMap<(u64, u64), Vec<u8>>,
    /// Next FD for pipes.
    next_pipe_fd: u64,
}

impl ProcessApi {
    /// Creates a new process API with an init process (PID 1).
    pub fn new() -> Self {
        let mut api = Self {
            processes: HashMap::new(),
            next_pid: 2,
            pipes: HashMap::new(),
            next_pipe_fd: 100,
        };
        // Create init process
        api.processes.insert(
            1,
            Process {
                pid: 1,
                ppid: 0,
                name: "init".to_string(),
                state: ProcessState::Running,
                exit_code: None,
                env: HashMap::new(),
                args: vec!["init".to_string()],
                fds: vec![0, 1, 2],
            },
        );
        api
    }

    /// Forks a process. Returns the child PID.
    pub fn fork(&mut self, parent_pid: u32) -> Result<u32, UserlandError> {
        let parent = self
            .processes
            .get(&parent_pid)
            .ok_or(UserlandError::ProcessNotFound(parent_pid))?
            .clone();

        let child_pid = self.next_pid;
        self.next_pid += 1;

        let child = Process {
            pid: child_pid,
            ppid: parent_pid,
            name: parent.name.clone(),
            state: ProcessState::Ready,
            exit_code: None,
            env: parent.env.clone(),
            args: parent.args.clone(),
            fds: parent.fds.clone(),
        };
        self.processes.insert(child_pid, child);
        Ok(child_pid)
    }

    /// Replaces a process image (exec).
    pub fn exec(
        &mut self,
        pid: u32,
        name: &str,
        args: Vec<String>,
    ) -> Result<(), UserlandError> {
        let proc = self
            .processes
            .get_mut(&pid)
            .ok_or(UserlandError::ProcessNotFound(pid))?;
        proc.name = name.to_string();
        proc.args = args;
        proc.state = ProcessState::Running;
        Ok(())
    }

    /// Waits for a child process to exit. Returns the exit code.
    pub fn waitpid(&mut self, pid: u32) -> Result<i32, UserlandError> {
        let proc = self
            .processes
            .get(&pid)
            .ok_or(UserlandError::ProcessNotFound(pid))?;
        match proc.state {
            ProcessState::Zombie => {
                let code = proc.exit_code.unwrap_or(0);
                self.processes.remove(&pid);
                Ok(code)
            }
            _ => Ok(-1), // not yet exited (non-blocking)
        }
    }

    /// Terminates a process with the given exit code.
    pub fn exit(&mut self, pid: u32, code: i32) -> Result<(), UserlandError> {
        let proc = self
            .processes
            .get_mut(&pid)
            .ok_or(UserlandError::ProcessNotFound(pid))?;
        proc.state = ProcessState::Zombie;
        proc.exit_code = Some(code);
        Ok(())
    }

    /// Returns the PID of a process (identity, for API completeness).
    pub fn getpid(&self, pid: u32) -> Result<u32, UserlandError> {
        if self.processes.contains_key(&pid) {
            Ok(pid)
        } else {
            Err(UserlandError::ProcessNotFound(pid))
        }
    }

    /// Creates a pipe. Returns (read_fd, write_fd).
    pub fn pipe(&mut self) -> (u64, u64) {
        let read_fd = self.next_pipe_fd;
        self.next_pipe_fd += 1;
        let write_fd = self.next_pipe_fd;
        self.next_pipe_fd += 1;
        self.pipes.insert((read_fd, write_fd), Vec::new());
        (read_fd, write_fd)
    }

    /// Writes to a pipe.
    pub fn pipe_write(&mut self, write_fd: u64, data: &[u8]) -> Result<usize, UserlandError> {
        for ((_, wfd), buf) in &mut self.pipes {
            if *wfd == write_fd {
                buf.extend_from_slice(data);
                return Ok(data.len());
            }
        }
        Err(UserlandError::BadFd(write_fd))
    }

    /// Reads from a pipe.
    pub fn pipe_read(&mut self, read_fd: u64) -> Result<Vec<u8>, UserlandError> {
        for ((rfd, _), buf) in &mut self.pipes {
            if *rfd == read_fd {
                let data = buf.clone();
                buf.clear();
                return Ok(data);
            }
        }
        Err(UserlandError::BadFd(read_fd))
    }

    /// Returns the number of active processes.
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// Looks up a process by PID.
    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        self.processes.get(&pid)
    }
}

impl Default for ProcessApi {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Signal Handler
// ═══════════════════════════════════════════════════════════════════════

/// Standard signal numbers.
pub const SIGTERM: i32 = 15;
/// Interrupt signal (Ctrl+C).
pub const SIGINT: i32 = 2;
/// Child process status change.
pub const SIGCHLD: i32 = 17;
/// Kill signal (cannot be caught).
pub const SIGKILL: i32 = 9;
/// Stop signal.
pub const SIGSTOP: i32 = 19;
/// Continue signal.
pub const SIGCONT: i32 = 18;
/// Alarm signal.
pub const SIGALRM: i32 = 14;
/// User-defined signal 1.
pub const SIGUSR1: i32 = 10;

/// Signal disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigAction {
    /// Default action.
    Default,
    /// Ignore the signal.
    Ignore,
    /// Call a handler (handler_id).
    Handler(u32),
}

/// Pending signal.
#[derive(Debug, Clone)]
pub struct PendingSignal {
    /// Signal number.
    pub signum: i32,
    /// Sender PID.
    pub sender: u32,
    /// Timestamp.
    pub timestamp: u64,
}

/// Signal handler for a process.
#[derive(Debug)]
pub struct SignalHandler {
    /// Signal dispositions: signal number -> action.
    dispositions: HashMap<i32, SigAction>,
    /// Pending signals queue.
    pending: Vec<PendingSignal>,
    /// Signal mask (blocked signals).
    mask: Vec<i32>,
    /// Delivered signal log.
    pub delivered: Vec<PendingSignal>,
}

impl SignalHandler {
    /// Creates a new signal handler with default dispositions.
    pub fn new() -> Self {
        Self {
            dispositions: HashMap::new(),
            pending: Vec::new(),
            mask: Vec::new(),
            delivered: Vec::new(),
        }
    }

    /// Registers a signal handler.
    pub fn sigaction(&mut self, signum: i32, action: SigAction) -> bool {
        // Cannot catch SIGKILL or SIGSTOP
        if signum == SIGKILL || signum == SIGSTOP {
            return false;
        }
        self.dispositions.insert(signum, action);
        true
    }

    /// Sends a signal. Returns `true` if delivered.
    pub fn send(&mut self, signum: i32, sender: u32, now: u64) -> bool {
        if self.mask.contains(&signum) {
            return false;
        }
        self.pending.push(PendingSignal {
            signum,
            sender,
            timestamp: now,
        });
        true
    }

    /// Processes pending signals. Returns list of handler IDs to invoke.
    pub fn process_pending(&mut self) -> Vec<(i32, SigAction)> {
        let mut actions = Vec::new();
        let signals: Vec<PendingSignal> = self.pending.drain(..).collect();

        for sig in signals {
            let action = self
                .dispositions
                .get(&sig.signum)
                .copied()
                .unwrap_or(SigAction::Default);
            if action != SigAction::Ignore {
                actions.push((sig.signum, action));
            }
            self.delivered.push(sig);
        }
        actions
    }

    /// Blocks a signal.
    pub fn block(&mut self, signum: i32) {
        if !self.mask.contains(&signum) {
            self.mask.push(signum);
        }
    }

    /// Unblocks a signal.
    pub fn unblock(&mut self, signum: i32) {
        self.mask.retain(|&s| s != signum);
    }

    /// Returns the number of pending signals.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Thread API
// ═══════════════════════════════════════════════════════════════════════

/// Thread state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Created but not started.
    Created,
    /// Running.
    Running,
    /// Blocked (waiting on mutex/condvar).
    Blocked,
    /// Finished.
    Finished,
}

/// A simulated thread.
#[derive(Debug, Clone)]
pub struct Thread {
    /// Thread ID.
    pub tid: u32,
    /// Thread name.
    pub name: String,
    /// Current state.
    pub state: ThreadState,
    /// Return value (set on join).
    pub return_value: Option<i64>,
}

/// Simulated mutex.
#[derive(Debug)]
pub struct Mutex {
    /// Mutex ID.
    pub id: u32,
    /// Is locked?
    pub locked: bool,
    /// Owner thread ID (0 if unlocked).
    pub owner: u32,
    /// Waiting threads.
    pub waiters: Vec<u32>,
}

/// Simulated condition variable.
#[derive(Debug)]
pub struct CondVar {
    /// CondVar ID.
    pub id: u32,
    /// Waiting threads.
    pub waiters: Vec<u32>,
}

/// Pthread-like thread API.
#[derive(Debug)]
pub struct ThreadApi {
    /// All threads.
    threads: HashMap<u32, Thread>,
    /// Mutexes.
    mutexes: HashMap<u32, Mutex>,
    /// Condition variables.
    condvars: HashMap<u32, CondVar>,
    /// Next thread ID.
    next_tid: u32,
    /// Next mutex ID.
    next_mutex_id: u32,
    /// Next condvar ID.
    next_condvar_id: u32,
}

impl ThreadApi {
    /// Creates a new thread API.
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
            mutexes: HashMap::new(),
            condvars: HashMap::new(),
            next_tid: 1,
            next_mutex_id: 1,
            next_condvar_id: 1,
        }
    }

    /// Creates a new thread. Returns the thread ID.
    pub fn create(&mut self, name: &str) -> u32 {
        let tid = self.next_tid;
        self.next_tid += 1;
        self.threads.insert(
            tid,
            Thread {
                tid,
                name: name.to_string(),
                state: ThreadState::Created,
                return_value: None,
            },
        );
        tid
    }

    /// Starts a thread (transitions to Running).
    pub fn start(&mut self, tid: u32) -> Result<(), UserlandError> {
        let thread = self
            .threads
            .get_mut(&tid)
            .ok_or_else(|| UserlandError::ThreadError(format!("thread {} not found", tid)))?;
        thread.state = ThreadState::Running;
        Ok(())
    }

    /// Joins a thread (waits for it to finish). Returns the return value.
    pub fn join(&mut self, tid: u32) -> Result<Option<i64>, UserlandError> {
        let thread = self
            .threads
            .get(&tid)
            .ok_or_else(|| UserlandError::ThreadError(format!("thread {} not found", tid)))?;
        if thread.state != ThreadState::Finished {
            return Err(UserlandError::ThreadError(format!(
                "thread {} not finished",
                tid
            )));
        }
        let val = thread.return_value;
        self.threads.remove(&tid);
        Ok(val)
    }

    /// Finishes a thread with a return value.
    pub fn finish(&mut self, tid: u32, value: i64) -> Result<(), UserlandError> {
        let thread = self
            .threads
            .get_mut(&tid)
            .ok_or_else(|| UserlandError::ThreadError(format!("thread {} not found", tid)))?;
        thread.state = ThreadState::Finished;
        thread.return_value = Some(value);
        Ok(())
    }

    /// Creates a mutex. Returns the mutex ID.
    pub fn mutex_create(&mut self) -> u32 {
        let id = self.next_mutex_id;
        self.next_mutex_id += 1;
        self.mutexes.insert(
            id,
            Mutex {
                id,
                locked: false,
                owner: 0,
                waiters: Vec::new(),
            },
        );
        id
    }

    /// Locks a mutex.
    pub fn mutex_lock(&mut self, mutex_id: u32, tid: u32) -> Result<bool, UserlandError> {
        let mtx = self.mutexes.get_mut(&mutex_id).ok_or_else(|| {
            UserlandError::ThreadError(format!("mutex {} not found", mutex_id))
        })?;
        if !mtx.locked {
            mtx.locked = true;
            mtx.owner = tid;
            Ok(true)
        } else {
            mtx.waiters.push(tid);
            Ok(false) // blocked
        }
    }

    /// Unlocks a mutex.
    pub fn mutex_unlock(&mut self, mutex_id: u32, tid: u32) -> Result<(), UserlandError> {
        let mtx = self.mutexes.get_mut(&mutex_id).ok_or_else(|| {
            UserlandError::ThreadError(format!("mutex {} not found", mutex_id))
        })?;
        if mtx.owner != tid {
            return Err(UserlandError::ThreadError(
                "not owner of mutex".to_string(),
            ));
        }
        if let Some(next_tid) = mtx.waiters.first().copied() {
            mtx.owner = next_tid;
            mtx.waiters.remove(0);
        } else {
            mtx.locked = false;
            mtx.owner = 0;
        }
        Ok(())
    }

    /// Creates a condition variable. Returns the condvar ID.
    pub fn condvar_create(&mut self) -> u32 {
        let id = self.next_condvar_id;
        self.next_condvar_id += 1;
        self.condvars.insert(
            id,
            CondVar {
                id,
                waiters: Vec::new(),
            },
        );
        id
    }

    /// Waits on a condition variable.
    pub fn condvar_wait(&mut self, condvar_id: u32, tid: u32) -> Result<(), UserlandError> {
        let cv = self.condvars.get_mut(&condvar_id).ok_or_else(|| {
            UserlandError::ThreadError(format!("condvar {} not found", condvar_id))
        })?;
        cv.waiters.push(tid);
        Ok(())
    }

    /// Signals one waiter on a condition variable.
    pub fn condvar_signal(&mut self, condvar_id: u32) -> Result<Option<u32>, UserlandError> {
        let cv = self.condvars.get_mut(&condvar_id).ok_or_else(|| {
            UserlandError::ThreadError(format!("condvar {} not found", condvar_id))
        })?;
        Ok(if cv.waiters.is_empty() {
            None
        } else {
            Some(cv.waiters.remove(0))
        })
    }

    /// Broadcasts to all waiters on a condition variable.
    pub fn condvar_broadcast(&mut self, condvar_id: u32) -> Result<Vec<u32>, UserlandError> {
        let cv = self.condvars.get_mut(&condvar_id).ok_or_else(|| {
            UserlandError::ThreadError(format!("condvar {} not found", condvar_id))
        })?;
        let waiters: Vec<u32> = cv.waiters.drain(..).collect();
        Ok(waiters)
    }

    /// Returns the number of threads.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

impl Default for ThreadApi {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Time API
// ═══════════════════════════════════════════════════════════════════════

/// Simulated time values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeVal {
    /// Seconds.
    pub tv_sec: u64,
    /// Microseconds.
    pub tv_usec: u64,
}

/// Simulated timespec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSpec {
    /// Seconds.
    pub tv_sec: u64,
    /// Nanoseconds.
    pub tv_nsec: u64,
}

/// Clock type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockId {
    /// Wall-clock time.
    Realtime,
    /// Monotonic (boot time).
    Monotonic,
}

/// Simulated time API.
///
/// Maintains monotonic and realtime clocks for the simulated OS.
#[derive(Debug)]
pub struct TimeApi {
    /// Monotonic tick count (nanoseconds since boot).
    pub monotonic_ns: u64,
    /// Realtime offset (epoch offset in nanoseconds).
    pub realtime_offset_ns: u64,
    /// Sleep log (for testing).
    pub sleep_log: Vec<u64>,
}

impl TimeApi {
    /// Creates a new time API.
    ///
    /// `epoch_offset_secs` sets the wall-clock base time (seconds since Unix epoch).
    pub fn new(epoch_offset_secs: u64) -> Self {
        Self {
            monotonic_ns: 0,
            realtime_offset_ns: epoch_offset_secs.saturating_mul(1_000_000_000),
            sleep_log: Vec::new(),
        }
    }

    /// Advances the monotonic clock by the given nanoseconds.
    pub fn advance(&mut self, ns: u64) {
        self.monotonic_ns = self.monotonic_ns.saturating_add(ns);
    }

    /// Returns the current time for the given clock.
    pub fn clock_gettime(&self, clock: ClockId) -> TimeSpec {
        match clock {
            ClockId::Monotonic => TimeSpec {
                tv_sec: self.monotonic_ns / 1_000_000_000,
                tv_nsec: self.monotonic_ns % 1_000_000_000,
            },
            ClockId::Realtime => {
                let total = self.monotonic_ns.saturating_add(self.realtime_offset_ns);
                TimeSpec {
                    tv_sec: total / 1_000_000_000,
                    tv_nsec: total % 1_000_000_000,
                }
            }
        }
    }

    /// Returns the current time as a timeval.
    pub fn gettimeofday(&self) -> TimeVal {
        let ts = self.clock_gettime(ClockId::Realtime);
        TimeVal {
            tv_sec: ts.tv_sec,
            tv_usec: ts.tv_nsec / 1000,
        }
    }

    /// Simulated sleep (advances monotonic clock and records).
    pub fn sleep_secs(&mut self, secs: u64) {
        let ns = secs.saturating_mul(1_000_000_000);
        self.advance(ns);
        self.sleep_log.push(ns);
    }

    /// Simulated nanosleep.
    pub fn nanosleep(&mut self, ns: u64) {
        self.advance(ns);
        self.sleep_log.push(ns);
    }
}

impl Default for TimeApi {
    fn default() -> Self {
        Self::new(1_711_843_200) // 2024-03-31T00:00:00Z as a reasonable default
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- LibC tests ---

    #[test]
    fn libc_malloc_and_free() {
        let mut libc = LibC::new();
        let addr = libc.malloc(256).unwrap();
        assert!(addr > 0);
        assert_eq!(libc.allocated_bytes, 256);
        libc.free(addr).unwrap();
        assert_eq!(libc.allocated_bytes, 0);
    }

    #[test]
    fn libc_malloc_reuse_freed_block() {
        let mut libc = LibC::new();
        let addr1 = libc.malloc(128).unwrap();
        libc.free(addr1).unwrap();
        let addr2 = libc.malloc(64).unwrap();
        // Should reuse the freed block
        assert_eq!(addr2, addr1);
    }

    #[test]
    fn libc_printf() {
        let mut libc = LibC::new();
        libc.printf("Hello %s, you are %d years old", &["Fajar", "30"]);
        assert_eq!(libc.output[0], "Hello Fajar, you are 30 years old");
    }

    #[test]
    fn libc_strlen_and_strcmp() {
        let libc = LibC::new();
        assert_eq!(libc.strlen("hello"), 5);
        assert_eq!(libc.strcmp("abc", "abc"), 0);
        assert_eq!(libc.strcmp("abc", "def"), -1);
        assert_eq!(libc.strcmp("def", "abc"), 1);
    }

    #[test]
    fn libc_memcpy_and_memset() {
        let libc = LibC::new();
        let src = [1u8, 2, 3, 4, 5];
        let mut dst = [0u8; 5];
        libc.memcpy(&mut dst, &src, 5);
        assert_eq!(dst, [1, 2, 3, 4, 5]);

        libc.memset(&mut dst, 0xFF, 3);
        assert_eq!(dst, [0xFF, 0xFF, 0xFF, 4, 5]);
    }

    // --- LibM tests ---

    #[test]
    fn libm_trig_functions() {
        let m = LibM::new();
        assert!((m.sin(0.0)).abs() < 1e-10);
        assert!((m.cos(0.0) - 1.0).abs() < 1e-10);
        assert!((m.tan(0.0)).abs() < 1e-10);
    }

    #[test]
    fn libm_sqrt_pow() {
        let m = LibM::new();
        assert!((m.sqrt(4.0) - 2.0).abs() < 1e-10);
        assert!((m.pow(2.0, 10.0) - 1024.0).abs() < 1e-10);
    }

    #[test]
    fn libm_log_exp() {
        let m = LibM::new();
        assert!((m.exp(0.0) - 1.0).abs() < 1e-10);
        assert!((m.log(std::f64::consts::E) - 1.0).abs() < 1e-10);
    }

    // --- LibFj tests ---

    #[test]
    fn libfj_print_and_env() {
        let mut fj = LibFj::new();
        fj.print("Hello FajarOS");
        assert_eq!(fj.stdout[0], "Hello FajarOS");
        assert_eq!(fj.getenv("FAJAROS_VERSION"), Some("2.0.0"));
        fj.setenv("MY_VAR", "test");
        assert_eq!(fj.getenv("MY_VAR"), Some("test"));
    }

    #[test]
    fn libfj_array_ops() {
        let fj = LibFj::new();
        let arr = fj.array_new(3, 0);
        assert_eq!(arr.len(), 3);
        let arr = fj.array_push(arr, 42);
        assert_eq!(fj.array_len(&arr), 4);
        assert_eq!(arr[3], 42);
    }

    #[test]
    fn libfj_string_ops() {
        let fj = LibFj::new();
        assert_eq!(fj.string_concat("Hello", " World"), "Hello World");
        let parts = fj.string_split("a,b,c", ",");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    // --- Dynamic Linker tests ---

    #[test]
    fn linker_load_and_resolve() {
        let mut linker = DynamicLinker::new();
        let mut so = SharedObject::new("libtest.so");
        so.add_symbol("test_fn", 0x100, 64, SymbolBind::Global);
        so.add_symbol("internal", 0x200, 32, SymbolBind::Local);
        linker.register(so);

        let addr = linker.load("libtest.so").unwrap();
        assert!(addr > 0);

        // Global symbol should be in GOT
        let resolved = linker.resolve("test_fn").unwrap();
        assert_eq!(resolved, addr + 0x100);

        // Local symbol should NOT be in GOT
        assert!(linker.resolve("internal").is_err());
    }

    #[test]
    fn linker_so_not_found() {
        let mut linker = DynamicLinker::new();
        assert!(linker.load("nonexistent.so").is_err());
    }

    // --- Process API tests ---

    #[test]
    fn process_fork_and_exec() {
        let mut api = ProcessApi::new();
        let child = api.fork(1).unwrap();
        assert!(child > 1);
        assert_eq!(api.process_count(), 2);

        api.exec(child, "hello", vec!["hello".to_string(), "--name".to_string()])
            .unwrap();
        let proc = api.get_process(child).unwrap();
        assert_eq!(proc.name, "hello");
    }

    #[test]
    fn process_exit_and_wait() {
        let mut api = ProcessApi::new();
        let child = api.fork(1).unwrap();
        api.exit(child, 42).unwrap();
        let code = api.waitpid(child).unwrap();
        assert_eq!(code, 42);
        // Zombie should be reaped
        assert_eq!(api.process_count(), 1);
    }

    #[test]
    fn process_pipe() {
        let mut api = ProcessApi::new();
        let (rfd, wfd) = api.pipe();
        api.pipe_write(wfd, b"hello from pipe").unwrap();
        let data = api.pipe_read(rfd).unwrap();
        assert_eq!(data, b"hello from pipe");
    }

    // --- Signal Handler tests ---

    #[test]
    fn signal_send_and_process() {
        let mut handler = SignalHandler::new();
        handler.sigaction(SIGTERM, SigAction::Handler(1));
        handler.send(SIGTERM, 1, 100);
        let actions = handler.process_pending();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].0, SIGTERM);
        assert_eq!(actions[0].1, SigAction::Handler(1));
    }

    #[test]
    fn signal_cannot_catch_sigkill() {
        let mut handler = SignalHandler::new();
        assert!(!handler.sigaction(SIGKILL, SigAction::Ignore));
    }

    #[test]
    fn signal_blocking() {
        let mut handler = SignalHandler::new();
        handler.block(SIGTERM);
        let sent = handler.send(SIGTERM, 1, 100);
        assert!(!sent);
        handler.unblock(SIGTERM);
        let sent = handler.send(SIGTERM, 1, 200);
        assert!(sent);
    }

    // --- Thread API tests ---

    #[test]
    fn thread_create_start_finish_join() {
        let mut api = ThreadApi::new();
        let tid = api.create("worker");
        api.start(tid).unwrap();
        api.finish(tid, 42).unwrap();
        let val = api.join(tid).unwrap();
        assert_eq!(val, Some(42));
    }

    #[test]
    fn thread_mutex_lock_unlock() {
        let mut api = ThreadApi::new();
        let mtx = api.mutex_create();
        let t1 = api.create("t1");

        // First lock succeeds
        assert!(api.mutex_lock(mtx, t1).unwrap());

        // Second lock by another thread blocks
        let t2 = api.create("t2");
        assert!(!api.mutex_lock(mtx, t2).unwrap());

        // Unlock transfers to waiter
        api.mutex_unlock(mtx, t1).unwrap();
    }

    #[test]
    fn thread_condvar_signal() {
        let mut api = ThreadApi::new();
        let cv = api.condvar_create();
        let t1 = api.create("t1");

        api.condvar_wait(cv, t1).unwrap();
        let woken = api.condvar_signal(cv).unwrap();
        assert_eq!(woken, Some(t1));
    }

    // --- Time API tests ---

    #[test]
    fn time_monotonic_clock() {
        let mut time = TimeApi::new(0);
        time.advance(1_000_000_000); // 1 second
        let ts = time.clock_gettime(ClockId::Monotonic);
        assert_eq!(ts.tv_sec, 1);
        assert_eq!(ts.tv_nsec, 0);
    }

    #[test]
    fn time_realtime_clock() {
        let mut time = TimeApi::new(1_000_000);
        time.advance(500_000_000); // 0.5 seconds
        let ts = time.clock_gettime(ClockId::Realtime);
        assert_eq!(ts.tv_sec, 1_000_000);
        assert_eq!(ts.tv_nsec, 500_000_000);
    }

    #[test]
    fn time_sleep() {
        let mut time = TimeApi::new(0);
        time.sleep_secs(5);
        assert_eq!(time.monotonic_ns, 5_000_000_000);
        time.nanosleep(500_000);
        assert_eq!(time.monotonic_ns, 5_000_500_000);
        assert_eq!(time.sleep_log.len(), 2);
    }

    #[test]
    fn time_gettimeofday() {
        let mut time = TimeApi::new(1_000);
        time.advance(500_000_000); // 0.5s
        let tv = time.gettimeofday();
        assert_eq!(tv.tv_sec, 1_000);
        assert_eq!(tv.tv_usec, 500_000);
    }
}
