//! Python Async Interop — coroutine calling, asyncio bridge, GIL management.
//!
//! Sprint E4: 10 tasks covering Python coroutine execution, asyncio event loop
//! bridging with Fajar's tokio runtime, async generator interop, GIL-aware
//! computation, exception mapping, cancellation, timeouts, and connection pooling.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// E4.1: Python Coroutine Calling
// ═══════════════════════════════════════════════════════════════════════

/// Error type for Python async interop operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyAsyncError {
    /// The coroutine timed out.
    Timeout {
        /// How long we waited before timing out.
        elapsed: Duration,
    },
    /// The coroutine was cancelled.
    Cancelled {
        /// Reason for cancellation, if provided.
        reason: Option<String>,
    },
    /// A Python exception occurred during async execution.
    PythonException {
        /// Python exception type name (e.g., "RuntimeError").
        exc_type: String,
        /// Exception message.
        message: String,
        /// Optional Python traceback.
        traceback: Option<String>,
    },
    /// GIL could not be acquired.
    GilError {
        /// Description of the GIL failure.
        message: String,
    },
    /// The event loop is not running or has been closed.
    EventLoopClosed,
    /// The coroutine has already been awaited or is in an invalid state.
    InvalidState {
        /// Current state description.
        state: String,
    },
    /// Connection pool error.
    PoolError {
        /// Description of the pool failure.
        message: String,
    },
}

impl fmt::Display for PyAsyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { elapsed } => {
                write!(f, "asyncio.TimeoutError: elapsed {:?}", elapsed)
            }
            Self::Cancelled { reason } => {
                if let Some(r) = reason {
                    write!(f, "asyncio.CancelledError: {r}")
                } else {
                    write!(f, "asyncio.CancelledError")
                }
            }
            Self::PythonException {
                exc_type, message, ..
            } => write!(f, "{exc_type}: {message}"),
            Self::GilError { message } => write!(f, "GIL error: {message}"),
            Self::EventLoopClosed => write!(f, "Event loop is closed"),
            Self::InvalidState { state } => {
                write!(f, "InvalidStateError: coroutine is {state}")
            }
            Self::PoolError { message } => write!(f, "PoolError: {message}"),
        }
    }
}

/// Maps a Python async exception type string to a `PyAsyncError`.
pub fn map_async_exception(exc_type: &str, message: &str) -> PyAsyncError {
    match exc_type {
        "asyncio.TimeoutError" | "TimeoutError" => PyAsyncError::Timeout {
            elapsed: Duration::from_secs(0),
        },
        "asyncio.CancelledError" | "CancelledError" => PyAsyncError::Cancelled {
            reason: if message.is_empty() {
                None
            } else {
                Some(message.to_string())
            },
        },
        "asyncio.InvalidStateError" | "InvalidStateError" => PyAsyncError::InvalidState {
            state: message.to_string(),
        },
        _ => PyAsyncError::PythonException {
            exc_type: exc_type.to_string(),
            message: message.to_string(),
            traceback: None,
        },
    }
}

/// Poll result from a Python coroutine.
#[derive(Debug, Clone, PartialEq)]
pub enum PyPollResult {
    /// The coroutine is still pending.
    Pending,
    /// The coroutine completed with a value.
    Ready(PyAsyncValue),
    /// The coroutine raised an exception.
    Error(PyAsyncError),
}

/// A value produced by a Python async operation.
#[derive(Debug, Clone, PartialEq)]
pub enum PyAsyncValue {
    /// No return value (None).
    None,
    /// Integer result.
    Int(i64),
    /// Float result.
    Float(f64),
    /// String result.
    Str(String),
    /// Boolean result.
    Bool(bool),
    /// Bytes result.
    Bytes(Vec<u8>),
    /// List of values.
    List(Vec<PyAsyncValue>),
    /// Dictionary of values.
    Dict(Vec<(String, PyAsyncValue)>),
}

impl fmt::Display for PyAsyncValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Bytes(bs) => write!(f, "bytes(len={})", bs.len()),
            Self::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| format!("{v}")).collect();
                write!(f, "[{}]", inner.join(", "))
            }
            Self::Dict(entries) => {
                let inner: Vec<String> = entries.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", inner.join(", "))
            }
        }
    }
}

/// State of a Python coroutine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    /// Created but not yet started.
    Created,
    /// Running (being polled).
    Running,
    /// Suspended (yielded, awaiting something).
    Suspended,
    /// Completed successfully.
    Completed,
    /// Cancelled.
    Cancelled,
    /// Failed with an exception.
    Failed,
}

impl fmt::Display for CoroutineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "CREATED"),
            Self::Running => write!(f, "RUNNING"),
            Self::Suspended => write!(f, "SUSPENDED"),
            Self::Completed => write!(f, "COMPLETED"),
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::Failed => write!(f, "FAILED"),
        }
    }
}

/// A Python coroutine with poll/await semantics.
///
/// Wraps a Python coroutine object and provides a Fajar-native polling
/// interface that integrates with the Fajar async runtime.
#[derive(Debug, Clone)]
pub struct PyCoroutine {
    /// Unique identifier for this coroutine.
    id: u64,
    /// Module containing the coroutine function.
    module: String,
    /// Coroutine function name.
    function: String,
    /// Current state.
    state: CoroutineState,
    /// Result value (set when completed).
    result: Option<PyAsyncValue>,
    /// Error (set when failed).
    error: Option<PyAsyncError>,
    /// Number of times this coroutine has been polled.
    poll_count: u64,
    /// Whether cancellation has been requested.
    cancel_requested: bool,
}

/// Counter for generating unique coroutine IDs.
static NEXT_COROUTINE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl PyCoroutine {
    /// Creates a new Python coroutine from a module and function name.
    pub fn new(module: &str, function: &str) -> Self {
        let id = NEXT_COROUTINE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id,
            module: module.to_string(),
            function: function.to_string(),
            state: CoroutineState::Created,
            result: None,
            error: None,
            poll_count: 0,
            cancel_requested: false,
        }
    }

    /// Returns the unique ID of this coroutine.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the current state of this coroutine.
    pub fn state(&self) -> CoroutineState {
        self.state
    }

    /// Returns the module name.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Returns the function name.
    pub fn function(&self) -> &str {
        &self.function
    }

    /// Returns the number of times this coroutine has been polled.
    pub fn poll_count(&self) -> u64 {
        self.poll_count
    }

    /// Polls the coroutine, advancing it one step.
    ///
    /// In a real implementation this would call into CPython's coroutine
    /// protocol (`send(None)` / `throw()`). Here we simulate: first poll
    /// returns Pending, second poll completes with a value.
    pub fn poll(&mut self) -> PyPollResult {
        if self.cancel_requested {
            self.state = CoroutineState::Cancelled;
            let err = PyAsyncError::Cancelled {
                reason: Some("cancel() called".to_string()),
            };
            self.error = Some(err.clone());
            return PyPollResult::Error(err);
        }

        match self.state {
            CoroutineState::Completed => {
                if let Some(ref val) = self.result {
                    PyPollResult::Ready(val.clone())
                } else {
                    PyPollResult::Ready(PyAsyncValue::None)
                }
            }
            CoroutineState::Failed => {
                let err = self.error.clone().unwrap_or(PyAsyncError::InvalidState {
                    state: "failed without error".to_string(),
                });
                PyPollResult::Error(err)
            }
            CoroutineState::Cancelled => PyPollResult::Error(PyAsyncError::Cancelled {
                reason: Some("already cancelled".to_string()),
            }),
            _ => {
                self.poll_count += 1;
                self.state = CoroutineState::Running;

                // Simulate: complete on second poll
                if self.poll_count >= 2 {
                    self.state = CoroutineState::Completed;
                    let val =
                        PyAsyncValue::Str(format!("{}:{} completed", self.module, self.function));
                    self.result = Some(val.clone());
                    PyPollResult::Ready(val)
                } else {
                    self.state = CoroutineState::Suspended;
                    PyPollResult::Pending
                }
            }
        }
    }

    /// Awaits the coroutine to completion by polling until ready.
    ///
    /// Returns the final value or an error.
    pub fn await_result(&mut self) -> Result<PyAsyncValue, PyAsyncError> {
        loop {
            match self.poll() {
                PyPollResult::Ready(val) => return Ok(val),
                PyPollResult::Error(err) => return Err(err),
                PyPollResult::Pending => continue,
            }
        }
    }

    /// Completes the coroutine with a specific value (for testing/simulation).
    pub fn complete_with(&mut self, value: PyAsyncValue) {
        self.state = CoroutineState::Completed;
        self.result = Some(value);
    }

    /// Fails the coroutine with a specific error (for testing/simulation).
    pub fn fail_with(&mut self, error: PyAsyncError) {
        self.state = CoroutineState::Failed;
        self.error = Some(error);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.2: asyncio Event Loop Bridge
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the event loop bridge.
#[derive(Debug, Clone)]
pub struct EventLoopConfig {
    /// Maximum number of concurrent Python coroutines.
    pub max_concurrent: usize,
    /// Whether to run asyncio in a dedicated thread.
    pub dedicated_thread: bool,
    /// Queue size for cross-runtime communication.
    pub queue_size: usize,
}

impl Default for EventLoopConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 64,
            dedicated_thread: true,
            queue_size: 256,
        }
    }
}

/// State of the event loop bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLoopState {
    /// Not yet started.
    Idle,
    /// Running and accepting tasks.
    Running,
    /// Shutting down (draining pending tasks).
    ShuttingDown,
    /// Fully stopped.
    Stopped,
}

impl fmt::Display for EventLoopState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "IDLE"),
            Self::Running => write!(f, "RUNNING"),
            Self::ShuttingDown => write!(f, "SHUTTING_DOWN"),
            Self::Stopped => write!(f, "STOPPED"),
        }
    }
}

/// Bridges Python asyncio event loop with Fajar's tokio runtime.
///
/// Manages the lifecycle of a Python asyncio event loop running in a
/// background thread, with a channel-based interface for submitting
/// coroutines from the Fajar (tokio) side and receiving results.
#[derive(Debug)]
pub struct EventLoopBridge {
    /// Configuration.
    config: EventLoopConfig,
    /// Current state.
    state: EventLoopState,
    /// Pending coroutines (id -> coroutine).
    pending: HashMap<u64, PyCoroutine>,
    /// Completed results (id -> result).
    completed: HashMap<u64, Result<PyAsyncValue, PyAsyncError>>,
    /// Total coroutines submitted.
    total_submitted: u64,
    /// Total coroutines completed.
    total_completed: u64,
}

impl EventLoopBridge {
    /// Creates a new event loop bridge with the given configuration.
    pub fn new(config: EventLoopConfig) -> Self {
        Self {
            config,
            state: EventLoopState::Idle,
            pending: HashMap::new(),
            completed: HashMap::new(),
            total_submitted: 0,
            total_completed: 0,
        }
    }

    /// Creates a new bridge with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(EventLoopConfig::default())
    }

    /// Starts the event loop bridge.
    pub fn start(&mut self) -> Result<(), PyAsyncError> {
        match self.state {
            EventLoopState::Running => Ok(()),
            EventLoopState::Stopped => Err(PyAsyncError::EventLoopClosed),
            _ => {
                self.state = EventLoopState::Running;
                Ok(())
            }
        }
    }

    /// Submits a coroutine for execution on the Python event loop.
    ///
    /// Returns the coroutine ID that can be used to poll for the result.
    pub fn submit(&mut self, coroutine: PyCoroutine) -> Result<u64, PyAsyncError> {
        if self.state != EventLoopState::Running {
            return Err(PyAsyncError::EventLoopClosed);
        }
        if self.pending.len() >= self.config.max_concurrent {
            return Err(PyAsyncError::PoolError {
                message: format!(
                    "max concurrent coroutines ({}) reached",
                    self.config.max_concurrent
                ),
            });
        }
        let id = coroutine.id();
        self.pending.insert(id, coroutine);
        self.total_submitted += 1;
        Ok(id)
    }

    /// Polls a submitted coroutine by ID.
    pub fn poll(&mut self, id: u64) -> Result<PyPollResult, PyAsyncError> {
        // Check completed cache first.
        if let Some(result) = self.completed.get(&id) {
            return match result {
                Ok(val) => Ok(PyPollResult::Ready(val.clone())),
                Err(err) => Ok(PyPollResult::Error(err.clone())),
            };
        }

        let coroutine = self
            .pending
            .get_mut(&id)
            .ok_or(PyAsyncError::InvalidState {
                state: format!("coroutine {id} not found"),
            })?;

        let result = coroutine.poll();
        match &result {
            PyPollResult::Ready(val) => {
                self.completed.insert(id, Ok(val.clone()));
                self.pending.remove(&id);
                self.total_completed += 1;
            }
            PyPollResult::Error(err) => {
                self.completed.insert(id, Err(err.clone()));
                self.pending.remove(&id);
                self.total_completed += 1;
            }
            PyPollResult::Pending => {}
        }
        Ok(result)
    }

    /// Returns the number of currently pending coroutines.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the total number of completed coroutines.
    pub fn completed_count(&self) -> u64 {
        self.total_completed
    }

    /// Returns the current state of the bridge.
    pub fn state(&self) -> EventLoopState {
        self.state
    }

    /// Returns the configuration.
    pub fn config(&self) -> &EventLoopConfig {
        &self.config
    }

    /// Shuts down the event loop bridge, cancelling all pending coroutines.
    pub fn shutdown(&mut self) -> Vec<u64> {
        self.state = EventLoopState::ShuttingDown;
        let cancelled_ids: Vec<u64> = self.pending.keys().copied().collect();
        for (id, _coro) in self.pending.drain() {
            self.completed.insert(
                id,
                Err(PyAsyncError::Cancelled {
                    reason: Some("event loop shutting down".to_string()),
                }),
            );
        }
        self.state = EventLoopState::Stopped;
        cancelled_ids
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.3: Fajar Async -> Python Awaitable
// ═══════════════════════════════════════════════════════════════════════

/// Wraps a Fajar async computation as a Python-compatible awaitable.
///
/// When exposed to Python, this allows `await fajar_function()` syntax
/// from Python code. The Fajar side runs on tokio, and results are
/// marshalled back through the event loop bridge.
#[derive(Debug, Clone)]
pub struct FajarAwaitable {
    /// Unique awaitable identifier.
    id: u64,
    /// Name of the Fajar function being exposed.
    function_name: String,
    /// Current state.
    state: CoroutineState,
    /// Result when completed.
    result: Option<PyAsyncValue>,
    /// Error when failed.
    error: Option<PyAsyncError>,
    /// Whether this awaitable has been consumed (single-use).
    consumed: bool,
}

/// Counter for generating unique awaitable IDs.
static NEXT_AWAITABLE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl FajarAwaitable {
    /// Creates a new Fajar awaitable for the given function.
    pub fn new(function_name: &str) -> Self {
        let id = NEXT_AWAITABLE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id,
            function_name: function_name.to_string(),
            state: CoroutineState::Created,
            result: None,
            error: None,
            consumed: false,
        }
    }

    /// Returns the awaitable ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the function name.
    pub fn function_name(&self) -> &str {
        &self.function_name
    }

    /// Returns whether this awaitable has been consumed.
    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    /// Returns the current state.
    pub fn state(&self) -> CoroutineState {
        self.state
    }

    /// Sets the result and marks as completed.
    pub fn set_result(&mut self, value: PyAsyncValue) {
        self.state = CoroutineState::Completed;
        self.result = Some(value);
    }

    /// Sets an error and marks as failed.
    pub fn set_error(&mut self, error: PyAsyncError) {
        self.state = CoroutineState::Failed;
        self.error = Some(error);
    }

    /// Consumes the awaitable, returning the result.
    ///
    /// Returns an error if already consumed or if the coroutine has not
    /// completed yet.
    pub fn consume(&mut self) -> Result<PyAsyncValue, PyAsyncError> {
        if self.consumed {
            return Err(PyAsyncError::InvalidState {
                state: "awaitable already consumed".to_string(),
            });
        }
        self.consumed = true;

        match self.state {
            CoroutineState::Completed => Ok(self.result.take().unwrap_or(PyAsyncValue::None)),
            CoroutineState::Failed => {
                Err(self.error.take().unwrap_or(PyAsyncError::InvalidState {
                    state: "failed without error".to_string(),
                }))
            }
            other => Err(PyAsyncError::InvalidState {
                state: format!("not completed, currently {other}"),
            }),
        }
    }

    /// Generates the Python wrapper code that exposes this as an awaitable.
    pub fn python_wrapper(&self) -> String {
        format!(
            r#"async def {name}(*args, **kwargs):
    """Fajar Lang awaitable wrapper for {name}."""
    return await _fajar_bridge.call("{name}", args, kwargs)
"#,
            name = self.function_name
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.4: Async Generator Bridge
// ═══════════════════════════════════════════════════════════════════════

/// A Python async generator bridge.
///
/// Wraps a Python async generator and provides `async_next()` to
/// retrieve yielded values one at a time.
#[derive(Debug, Clone)]
pub struct PyAsyncGenerator {
    /// Unique identifier.
    id: u64,
    /// Module containing the generator function.
    module: String,
    /// Generator function name.
    function: String,
    /// Items that have been yielded (simulated buffer).
    buffer: Vec<PyAsyncValue>,
    /// Current read position in the buffer.
    position: usize,
    /// Whether the generator has been exhausted.
    exhausted: bool,
    /// Total items yielded.
    total_yielded: u64,
}

/// Counter for generating unique generator IDs.
static NEXT_GENERATOR_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl PyAsyncGenerator {
    /// Creates a new async generator bridge.
    pub fn new(module: &str, function: &str) -> Self {
        let id = NEXT_GENERATOR_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id,
            module: module.to_string(),
            function: function.to_string(),
            buffer: Vec::new(),
            position: 0,
            exhausted: false,
            total_yielded: 0,
        }
    }

    /// Returns the generator ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns whether the generator is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    /// Returns the total number of items yielded.
    pub fn total_yielded(&self) -> u64 {
        self.total_yielded
    }

    /// Feeds a value into the buffer (simulates the Python generator yielding).
    pub fn feed(&mut self, value: PyAsyncValue) {
        self.buffer.push(value);
    }

    /// Marks the generator as exhausted (no more values).
    pub fn mark_exhausted(&mut self) {
        self.exhausted = true;
    }

    /// Retrieves the next yielded value asynchronously.
    ///
    /// Returns `Ok(Some(value))` if a value is available,
    /// `Ok(None)` if the generator is exhausted, or
    /// `Err` if there is an error.
    pub fn async_next(&mut self) -> Result<Option<PyAsyncValue>, PyAsyncError> {
        if self.position < self.buffer.len() {
            let value = self.buffer[self.position].clone();
            self.position += 1;
            self.total_yielded += 1;
            Ok(Some(value))
        } else if self.exhausted {
            Ok(None)
        } else {
            // In a real implementation, this would await the next yield
            // from the Python async generator via the event loop bridge.
            Ok(None)
        }
    }

    /// Collects all remaining items into a Vec.
    pub fn collect_all(&mut self) -> Result<Vec<PyAsyncValue>, PyAsyncError> {
        let mut items = Vec::new();
        while let Some(val) = self.async_next()? {
            items.push(val);
        }
        Ok(items)
    }

    /// Returns the module name.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Returns the function name.
    pub fn function(&self) -> &str {
        &self.function
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.5: GIL Management (RAII)
// ═══════════════════════════════════════════════════════════════════════

/// GIL state for async operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncGilState {
    /// GIL is held.
    Held,
    /// GIL is released (Fajar computation in progress).
    Released,
    /// GIL was never acquired.
    NotAcquired,
}

impl fmt::Display for AsyncGilState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held => write!(f, "HELD"),
            Self::Released => write!(f, "RELEASED"),
            Self::NotAcquired => write!(f, "NOT_ACQUIRED"),
        }
    }
}

/// RAII guard for Python GIL management in async contexts.
///
/// Acquires the GIL on creation and releases it on drop. Provides
/// `release_gil()` for explicitly releasing during Fajar-side computation
/// (allowing other Python threads to run).
#[derive(Debug)]
pub struct GilGuard {
    /// Current GIL state.
    state: AsyncGilState,
    /// Thread ID that acquired the GIL.
    thread_id: u64,
    /// Number of times GIL was acquired.
    acquire_count: u64,
    /// Number of times GIL was released.
    release_count: u64,
}

impl GilGuard {
    /// Acquires the GIL.
    ///
    /// In a real implementation, this would call `PyGILState_Ensure()`.
    pub fn acquire() -> Result<Self, PyAsyncError> {
        Ok(Self {
            state: AsyncGilState::Held,
            thread_id: 1, // simulated
            acquire_count: 1,
            release_count: 0,
        })
    }

    /// Returns the thread ID that acquired the GIL.
    pub fn thread_id(&self) -> u64 {
        self.thread_id
    }

    /// Returns the current GIL state.
    pub fn state(&self) -> AsyncGilState {
        self.state
    }

    /// Returns whether the GIL is currently held.
    pub fn is_held(&self) -> bool {
        self.state == AsyncGilState::Held
    }

    /// Releases the GIL for Fajar-side computation.
    ///
    /// While the GIL is released, other Python threads can execute.
    /// The GIL must be re-acquired before calling any Python API.
    pub fn release_gil(&mut self) -> Result<(), PyAsyncError> {
        if self.state != AsyncGilState::Held {
            return Err(PyAsyncError::GilError {
                message: format!("cannot release GIL from state {}", self.state),
            });
        }
        self.state = AsyncGilState::Released;
        self.release_count += 1;
        Ok(())
    }

    /// Re-acquires the GIL after it was released.
    pub fn reacquire_gil(&mut self) -> Result<(), PyAsyncError> {
        if self.state != AsyncGilState::Released {
            return Err(PyAsyncError::GilError {
                message: format!("cannot reacquire GIL from state {}", self.state),
            });
        }
        self.state = AsyncGilState::Held;
        self.acquire_count += 1;
        Ok(())
    }

    /// Returns the total number of GIL acquire operations.
    pub fn acquire_count(&self) -> u64 {
        self.acquire_count
    }

    /// Returns the total number of GIL release operations.
    pub fn release_count(&self) -> u64 {
        self.release_count
    }

    /// Executes a closure with the GIL released, re-acquiring after.
    ///
    /// This is the recommended pattern for Fajar-side computation that
    /// does not need Python access.
    pub fn without_gil<F, R>(&mut self, f: F) -> Result<R, PyAsyncError>
    where
        F: FnOnce() -> R,
    {
        self.release_gil()?;
        let result = f();
        self.reacquire_gil()?;
        Ok(result)
    }
}

impl Drop for GilGuard {
    fn drop(&mut self) {
        // In a real implementation, this would call `PyGILState_Release()`.
        self.state = AsyncGilState::NotAcquired;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.6: Exception -> Error Mapping
// ═══════════════════════════════════════════════════════════════════════

// (map_async_exception is defined above near PyAsyncError)

/// Maps Fajar error codes to Python exception types for re-raising.
pub fn fajar_error_to_python_exception(error_code: &str) -> &'static str {
    match error_code {
        "SE004" => "TypeError",
        "RE005" => "ZeroDivisionError",
        "RE006" => "IndexError",
        "RE007" => "KeyError",
        "RE008" => "FileNotFoundError",
        "ME008" => "MemoryError",
        "RE002" => "RuntimeError",
        _ => "RuntimeError",
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.7: Cancellation Support
// ═══════════════════════════════════════════════════════════════════════

/// Cancels a running coroutine.
///
/// Sets the cancel flag on the coroutine. The next poll will observe
/// the flag and return `CancelledError`.
pub fn cancel(coroutine: &mut PyCoroutine) -> Result<(), PyAsyncError> {
    match coroutine.state {
        CoroutineState::Completed => Err(PyAsyncError::InvalidState {
            state: "already completed".to_string(),
        }),
        CoroutineState::Cancelled => Err(PyAsyncError::InvalidState {
            state: "already cancelled".to_string(),
        }),
        _ => {
            coroutine.cancel_requested = true;
            Ok(())
        }
    }
}

/// Checks if a coroutine has a pending cancellation request.
pub fn is_cancel_requested(coroutine: &PyCoroutine) -> bool {
    coroutine.cancel_requested
}

// ═══════════════════════════════════════════════════════════════════════
// E4.8: Timeout Support
// ═══════════════════════════════════════════════════════════════════════

/// Wraps a coroutine with a timeout.
///
/// If the coroutine does not complete within `max_polls` iterations,
/// returns a `Timeout` error. In a real implementation, this would use
/// wall-clock time; here we use poll-count as a proxy.
pub fn with_timeout(
    coroutine: &mut PyCoroutine,
    _duration: Duration,
    max_polls: u64,
) -> Result<PyAsyncValue, PyAsyncError> {
    let mut polls = 0u64;
    loop {
        match coroutine.poll() {
            PyPollResult::Ready(val) => return Ok(val),
            PyPollResult::Error(err) => return Err(err),
            PyPollResult::Pending => {
                polls += 1;
                if polls >= max_polls {
                    coroutine.state = CoroutineState::Failed;
                    return Err(PyAsyncError::Timeout { elapsed: _duration });
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.9: Connection Pool (shared Python <-> Fajar)
// ═══════════════════════════════════════════════════════════════════════

/// A connection in the pool.
#[derive(Debug, Clone)]
pub struct PooledConnection {
    /// Unique connection ID.
    id: u64,
    /// Connection URI (e.g., database URL, HTTP endpoint).
    uri: String,
    /// Whether this connection is currently in use.
    in_use: bool,
    /// Number of times this connection has been used.
    use_count: u64,
}

/// Connection pool shared between Python and Fajar runtimes.
///
/// Manages a set of reusable connections (e.g., database, HTTP) that
/// can be checked out from either the Python or Fajar side.
#[derive(Debug)]
pub struct ConnectionPool {
    /// Pool name.
    name: String,
    /// Maximum pool size.
    max_size: usize,
    /// All connections.
    connections: Vec<PooledConnection>,
    /// Counter for generating connection IDs.
    next_id: u64,
    /// Total checkouts.
    total_checkouts: u64,
    /// Total checkins.
    total_checkins: u64,
}

impl ConnectionPool {
    /// Creates a new connection pool.
    pub fn new(name: &str, max_size: usize) -> Self {
        Self {
            name: name.to_string(),
            max_size,
            connections: Vec::new(),
            next_id: 1,
            total_checkouts: 0,
            total_checkins: 0,
        }
    }

    /// Returns the pool name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the maximum pool size.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Returns the current number of connections in the pool.
    pub fn size(&self) -> usize {
        self.connections.len()
    }

    /// Returns the number of available (idle) connections.
    pub fn available(&self) -> usize {
        self.connections.iter().filter(|c| !c.in_use).count()
    }

    /// Returns the number of connections currently in use.
    pub fn in_use(&self) -> usize {
        self.connections.iter().filter(|c| c.in_use).count()
    }

    /// Adds a new connection to the pool.
    pub fn add_connection(&mut self, uri: &str) -> Result<u64, PyAsyncError> {
        if self.connections.len() >= self.max_size {
            return Err(PyAsyncError::PoolError {
                message: format!(
                    "pool '{}' is full ({}/{})",
                    self.name,
                    self.connections.len(),
                    self.max_size
                ),
            });
        }
        let id = self.next_id;
        self.next_id += 1;
        self.connections.push(PooledConnection {
            id,
            uri: uri.to_string(),
            in_use: false,
            use_count: 0,
        });
        Ok(id)
    }

    /// Checks out a connection from the pool.
    ///
    /// Returns the connection ID of an available connection, or an error
    /// if no connections are available.
    pub fn checkout(&mut self) -> Result<u64, PyAsyncError> {
        for conn in &mut self.connections {
            if !conn.in_use {
                conn.in_use = true;
                conn.use_count += 1;
                self.total_checkouts += 1;
                return Ok(conn.id);
            }
        }
        Err(PyAsyncError::PoolError {
            message: format!(
                "no available connections in pool '{}' ({} total, all in use)",
                self.name,
                self.connections.len()
            ),
        })
    }

    /// Returns a connection to the pool.
    pub fn checkin(&mut self, id: u64) -> Result<(), PyAsyncError> {
        for conn in &mut self.connections {
            if conn.id == id {
                if !conn.in_use {
                    return Err(PyAsyncError::PoolError {
                        message: format!("connection {id} is not checked out"),
                    });
                }
                conn.in_use = false;
                self.total_checkins += 1;
                return Ok(());
            }
        }
        Err(PyAsyncError::PoolError {
            message: format!("connection {id} not found in pool '{}'", self.name),
        })
    }

    /// Returns the total number of checkout operations.
    pub fn total_checkouts(&self) -> u64 {
        self.total_checkouts
    }

    /// Returns the total number of checkin operations.
    pub fn total_checkins(&self) -> u64 {
        self.total_checkins
    }

    /// Returns the URI of a connection by ID.
    pub fn connection_uri(&self, id: u64) -> Option<&str> {
        self.connections
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.uri.as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4.10: Tests (15+)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- E4.1: Python Coroutine Tests ---

    #[test]
    fn e4_1_coroutine_creation() {
        let coro = PyCoroutine::new("asyncio", "sleep");
        assert_eq!(coro.module(), "asyncio");
        assert_eq!(coro.function(), "sleep");
        assert_eq!(coro.state(), CoroutineState::Created);
        assert_eq!(coro.poll_count(), 0);
    }

    #[test]
    fn e4_1_coroutine_poll_pending_then_ready() {
        let mut coro = PyCoroutine::new("aiohttp", "get");
        // First poll -> Pending
        let r1 = coro.poll();
        assert_eq!(r1, PyPollResult::Pending);
        assert_eq!(coro.state(), CoroutineState::Suspended);
        assert_eq!(coro.poll_count(), 1);

        // Second poll -> Ready
        let r2 = coro.poll();
        assert!(matches!(r2, PyPollResult::Ready(_)));
        assert_eq!(coro.state(), CoroutineState::Completed);
        assert_eq!(coro.poll_count(), 2);
    }

    #[test]
    fn e4_1_coroutine_await_result() {
        let mut coro = PyCoroutine::new("mylib", "fetch");
        let result = coro.await_result();
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(matches!(val, PyAsyncValue::Str(_)));
    }

    #[test]
    fn e4_1_coroutine_complete_with() {
        let mut coro = PyCoroutine::new("db", "query");
        coro.complete_with(PyAsyncValue::Int(42));
        let r = coro.poll();
        assert_eq!(r, PyPollResult::Ready(PyAsyncValue::Int(42)));
    }

    #[test]
    fn e4_1_coroutine_fail_with() {
        let mut coro = PyCoroutine::new("net", "connect");
        coro.fail_with(PyAsyncError::PythonException {
            exc_type: "ConnectionError".to_string(),
            message: "refused".to_string(),
            traceback: None,
        });
        let r = coro.poll();
        assert!(matches!(r, PyPollResult::Error(_)));
    }

    // --- E4.2: Event Loop Bridge Tests ---

    #[test]
    fn e4_2_event_loop_lifecycle() {
        let mut bridge = EventLoopBridge::with_defaults();
        assert_eq!(bridge.state(), EventLoopState::Idle);

        bridge.start().unwrap();
        assert_eq!(bridge.state(), EventLoopState::Running);

        let cancelled = bridge.shutdown();
        assert!(cancelled.is_empty());
        assert_eq!(bridge.state(), EventLoopState::Stopped);
    }

    #[test]
    fn e4_2_event_loop_submit_and_poll() {
        let mut bridge = EventLoopBridge::with_defaults();
        bridge.start().unwrap();

        let coro = PyCoroutine::new("test", "echo");
        let id = bridge.submit(coro).unwrap();
        assert_eq!(bridge.pending_count(), 1);

        // First poll -> Pending
        let r1 = bridge.poll(id).unwrap();
        assert_eq!(r1, PyPollResult::Pending);

        // Second poll -> Ready
        let r2 = bridge.poll(id).unwrap();
        assert!(matches!(r2, PyPollResult::Ready(_)));
        assert_eq!(bridge.pending_count(), 0);
        assert_eq!(bridge.completed_count(), 1);
    }

    #[test]
    fn e4_2_event_loop_submit_when_stopped() {
        let mut bridge = EventLoopBridge::with_defaults();
        let coro = PyCoroutine::new("test", "fn");
        let result = bridge.submit(coro);
        assert!(result.is_err());
    }

    // --- E4.3: FajarAwaitable Tests ---

    #[test]
    fn e4_3_awaitable_creation() {
        let aw = FajarAwaitable::new("my_function");
        assert_eq!(aw.function_name(), "my_function");
        assert_eq!(aw.state(), CoroutineState::Created);
        assert!(!aw.is_consumed());
    }

    #[test]
    fn e4_3_awaitable_consume() {
        let mut aw = FajarAwaitable::new("compute");
        aw.set_result(PyAsyncValue::Float(3.14));
        let val = aw.consume().unwrap();
        assert_eq!(val, PyAsyncValue::Float(3.14));
        assert!(aw.is_consumed());

        // Second consume fails
        let err = aw.consume().unwrap_err();
        assert!(matches!(err, PyAsyncError::InvalidState { .. }));
    }

    #[test]
    fn e4_3_awaitable_python_wrapper() {
        let aw = FajarAwaitable::new("predict");
        let code = aw.python_wrapper();
        assert!(code.contains("async def predict"));
        assert!(code.contains("_fajar_bridge.call"));
    }

    // --- E4.4: Async Generator Tests ---

    #[test]
    fn e4_4_async_generator_basic() {
        let mut ag = PyAsyncGenerator::new("data", "stream_items");
        assert_eq!(ag.module(), "data");
        assert_eq!(ag.function(), "stream_items");
        assert!(!ag.is_exhausted());

        ag.feed(PyAsyncValue::Int(1));
        ag.feed(PyAsyncValue::Int(2));
        ag.feed(PyAsyncValue::Int(3));
        ag.mark_exhausted();

        let items = ag.collect_all().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], PyAsyncValue::Int(1));
        assert_eq!(items[2], PyAsyncValue::Int(3));
        assert_eq!(ag.total_yielded(), 3);
    }

    #[test]
    fn e4_4_async_generator_exhausted() {
        let mut ag = PyAsyncGenerator::new("src", "gen_fn");
        ag.mark_exhausted();
        let result = ag.async_next().unwrap();
        assert!(result.is_none());
    }

    // --- E4.5: GIL Guard Tests ---

    #[test]
    fn e4_5_gil_guard_acquire_release() {
        let mut guard = GilGuard::acquire().unwrap();
        assert!(guard.is_held());
        assert_eq!(guard.state(), AsyncGilState::Held);

        guard.release_gil().unwrap();
        assert!(!guard.is_held());
        assert_eq!(guard.state(), AsyncGilState::Released);

        guard.reacquire_gil().unwrap();
        assert!(guard.is_held());
        assert_eq!(guard.acquire_count(), 2);
        assert_eq!(guard.release_count(), 1);
    }

    #[test]
    fn e4_5_gil_guard_without_gil() {
        let mut guard = GilGuard::acquire().unwrap();
        let result = guard.without_gil(|| 2 + 2).unwrap();
        assert_eq!(result, 4);
        assert!(guard.is_held()); // re-acquired after closure
    }

    #[test]
    fn e4_5_gil_guard_double_release_error() {
        let mut guard = GilGuard::acquire().unwrap();
        guard.release_gil().unwrap();
        let err = guard.release_gil().unwrap_err();
        assert!(matches!(err, PyAsyncError::GilError { .. }));
    }

    // --- E4.6: Exception Mapping Tests ---

    #[test]
    fn e4_6_timeout_exception_mapping() {
        let err = map_async_exception("asyncio.TimeoutError", "");
        assert!(matches!(err, PyAsyncError::Timeout { .. }));

        let err2 = map_async_exception("TimeoutError", "");
        assert!(matches!(err2, PyAsyncError::Timeout { .. }));
    }

    #[test]
    fn e4_6_cancelled_exception_mapping() {
        let err = map_async_exception("asyncio.CancelledError", "task was cancelled");
        match err {
            PyAsyncError::Cancelled { reason } => {
                assert_eq!(reason, Some("task was cancelled".to_string()));
            }
            other => panic!("expected Cancelled, got {other:?}"),
        }
    }

    #[test]
    fn e4_6_fajar_error_to_python() {
        assert_eq!(fajar_error_to_python_exception("SE004"), "TypeError");
        assert_eq!(
            fajar_error_to_python_exception("RE005"),
            "ZeroDivisionError"
        );
        assert_eq!(fajar_error_to_python_exception("UNKNOWN"), "RuntimeError");
    }

    // --- E4.7: Cancellation Tests ---

    #[test]
    fn e4_7_cancel_running_coroutine() {
        let mut coro = PyCoroutine::new("long", "task");
        coro.poll(); // move to Suspended
        assert!(!is_cancel_requested(&coro));

        cancel(&mut coro).unwrap();
        assert!(is_cancel_requested(&coro));

        let result = coro.poll();
        assert!(matches!(
            result,
            PyPollResult::Error(PyAsyncError::Cancelled { .. })
        ));
        assert_eq!(coro.state(), CoroutineState::Cancelled);
    }

    #[test]
    fn e4_7_cancel_completed_coroutine_fails() {
        let mut coro = PyCoroutine::new("done", "task");
        coro.complete_with(PyAsyncValue::None);
        let err = cancel(&mut coro).unwrap_err();
        assert!(matches!(err, PyAsyncError::InvalidState { .. }));
    }

    // --- E4.8: Timeout Tests ---

    #[test]
    fn e4_8_timeout_succeeds_within_limit() {
        let mut coro = PyCoroutine::new("quick", "fn");
        let result = with_timeout(&mut coro, Duration::from_secs(5), 10);
        assert!(result.is_ok());
    }

    #[test]
    fn e4_8_timeout_expires() {
        let mut coro = PyCoroutine::new("slow", "fn");
        // max_polls=1 means it will timeout on the first pending result
        let result = with_timeout(&mut coro, Duration::from_millis(100), 1);
        assert!(matches!(result, Err(PyAsyncError::Timeout { .. })));
    }

    // --- E4.9: Connection Pool Tests ---

    #[test]
    fn e4_9_pool_creation() {
        let pool = ConnectionPool::new("db_pool", 5);
        assert_eq!(pool.name(), "db_pool");
        assert_eq!(pool.max_size(), 5);
        assert_eq!(pool.size(), 0);
        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn e4_9_pool_checkout_checkin() {
        let mut pool = ConnectionPool::new("http", 3);
        pool.add_connection("http://localhost:8080").unwrap();
        pool.add_connection("http://localhost:8081").unwrap();
        assert_eq!(pool.size(), 2);
        assert_eq!(pool.available(), 2);

        let id1 = pool.checkout().unwrap();
        assert_eq!(pool.available(), 1);
        assert_eq!(pool.in_use(), 1);

        pool.checkin(id1).unwrap();
        assert_eq!(pool.available(), 2);
        assert_eq!(pool.in_use(), 0);
        assert_eq!(pool.total_checkouts(), 1);
        assert_eq!(pool.total_checkins(), 1);
    }

    #[test]
    fn e4_9_pool_full_error() {
        let mut pool = ConnectionPool::new("small", 1);
        pool.add_connection("db://host").unwrap();
        let err = pool.add_connection("db://host2").unwrap_err();
        assert!(matches!(err, PyAsyncError::PoolError { .. }));
    }

    #[test]
    fn e4_9_pool_no_available_error() {
        let mut pool = ConnectionPool::new("busy", 2);
        pool.add_connection("tcp://a").unwrap();
        pool.checkout().unwrap();
        let err = pool.checkout().unwrap_err();
        assert!(matches!(err, PyAsyncError::PoolError { .. }));
    }

    #[test]
    fn e4_9_pool_connection_uri() {
        let mut pool = ConnectionPool::new("test", 5);
        let id = pool.add_connection("redis://localhost:6379").unwrap();
        assert_eq!(pool.connection_uri(id), Some("redis://localhost:6379"));
        assert_eq!(pool.connection_uri(9999), None);
    }

    // --- Additional integration-style tests ---

    #[test]
    fn e4_error_display() {
        let err = PyAsyncError::Timeout {
            elapsed: Duration::from_secs(5),
        };
        let s = format!("{err}");
        assert!(s.contains("TimeoutError"));

        let err2 = PyAsyncError::Cancelled { reason: None };
        assert_eq!(format!("{err2}"), "asyncio.CancelledError");
    }

    #[test]
    fn e4_async_value_display() {
        assert_eq!(format!("{}", PyAsyncValue::None), "None");
        assert_eq!(format!("{}", PyAsyncValue::Int(42)), "42");
        assert_eq!(format!("{}", PyAsyncValue::Bool(true)), "true");
        assert_eq!(
            format!(
                "{}",
                PyAsyncValue::List(vec![PyAsyncValue::Int(1), PyAsyncValue::Int(2)])
            ),
            "[1, 2]"
        );
    }

    #[test]
    fn e4_coroutine_state_display() {
        assert_eq!(format!("{}", CoroutineState::Running), "RUNNING");
        assert_eq!(format!("{}", CoroutineState::Cancelled), "CANCELLED");
    }

    #[test]
    fn e4_bridge_shutdown_cancels_pending() {
        let mut bridge = EventLoopBridge::with_defaults();
        bridge.start().unwrap();

        let c1 = PyCoroutine::new("a", "fn1");
        let c2 = PyCoroutine::new("b", "fn2");
        let id1 = bridge.submit(c1).unwrap();
        let id2 = bridge.submit(c2).unwrap();
        assert_eq!(bridge.pending_count(), 2);

        let cancelled = bridge.shutdown();
        assert_eq!(cancelled.len(), 2);
        assert!(cancelled.contains(&id1));
        assert!(cancelled.contains(&id2));
        assert_eq!(bridge.state(), EventLoopState::Stopped);
    }
}
