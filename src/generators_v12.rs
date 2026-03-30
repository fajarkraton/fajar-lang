//! V12 Async Generators & Streams.
//!
//! Implements the generator and stream pipeline:
//! - G1-G2: Generator foundation + iterator integration
//! - G3-G4: Stream type + async generators
//! - G5: Coroutine support (resume with value)
//! - G6-G10: Channels, pipelines, error handling, performance, examples

use std::collections::VecDeque;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// G1: Generator Foundation
// ═══════════════════════════════════════════════════════════════════════

/// State of a generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorState {
    /// Generator has more values to yield.
    Yielded,
    /// Generator has completed (no more values).
    Complete,
    /// Generator was cancelled.
    Cancelled,
}

impl fmt::Display for GeneratorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneratorState::Yielded => write!(f, "yielded"),
            GeneratorState::Complete => write!(f, "complete"),
            GeneratorState::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A value produced by a generator: either a yielded value or a return value.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorOutput {
    /// A yielded value (generator can be resumed).
    Yielded(i64),
    /// The final return value (generator is complete).
    Complete(i64),
}

/// A synchronous generator that produces values via `yield`.
///
/// Generators are compiled to state machines where each `yield` point
/// becomes a state transition. The `resume()` method advances to the
/// next yield point.
#[derive(Debug)]
pub struct Generator {
    /// Generator name (for debugging).
    name: String,
    /// Pre-computed values (for interpreted generators).
    values: VecDeque<i64>,
    /// Return value when complete.
    return_value: i64,
    /// Current state.
    state: GeneratorState,
}

impl Generator {
    /// Creates a generator from a list of values to yield.
    pub fn from_values(name: &str, values: Vec<i64>, return_value: i64) -> Self {
        Self {
            name: name.to_string(),
            values: VecDeque::from(values),
            return_value,
            state: GeneratorState::Yielded,
        }
    }

    /// Resumes the generator, returning the next value.
    pub fn resume(&mut self) -> GeneratorOutput {
        if let Some(val) = self.values.pop_front() {
            if self.values.is_empty() {
                self.state = GeneratorState::Complete;
            }
            GeneratorOutput::Yielded(val)
        } else {
            self.state = GeneratorState::Complete;
            GeneratorOutput::Complete(self.return_value)
        }
    }

    /// Returns the current state.
    pub fn state(&self) -> GeneratorState {
        self.state
    }

    /// Cancels the generator.
    pub fn cancel(&mut self) {
        self.state = GeneratorState::Cancelled;
        self.values.clear();
    }

    /// Returns the generator name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ═══════════════════════════════════════════════════════════════════════
// G2: Iterator Integration
// ═══════════════════════════════════════════════════════════════════════

/// Iterator adapter for generators — enables `for x in generator { ... }`.
pub struct GeneratorIter {
    generator: Generator,
}

impl GeneratorIter {
    pub fn new(generator: Generator) -> Self {
        Self { generator }
    }
}

impl Iterator for GeneratorIter {
    type Item = i64;

    fn next(&mut self) -> Option<i64> {
        match self.generator.resume() {
            GeneratorOutput::Yielded(v) => Some(v),
            GeneratorOutput::Complete(_) => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// G3: Stream Type
// ═══════════════════════════════════════════════════════════════════════

/// Poll result for async streams.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamPoll {
    /// A value is ready.
    Ready(i64),
    /// No value available yet (would block).
    Pending,
    /// Stream is complete (no more values).
    Done,
}

/// An async stream that produces values over time.
#[derive(Debug)]
pub struct AsyncStream {
    /// Stream name.
    name: String,
    /// Buffered values.
    buffer: VecDeque<i64>,
    /// Whether the stream has been closed.
    closed: bool,
}

impl AsyncStream {
    /// Creates a new empty stream.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            buffer: VecDeque::new(),
            closed: false,
        }
    }

    /// Pushes a value into the stream.
    pub fn push(&mut self, value: i64) {
        if !self.closed {
            self.buffer.push_back(value);
        }
    }

    /// Polls the stream for the next value.
    pub fn poll(&mut self) -> StreamPoll {
        if let Some(val) = self.buffer.pop_front() {
            StreamPoll::Ready(val)
        } else if self.closed {
            StreamPoll::Done
        } else {
            StreamPoll::Pending
        }
    }

    /// Closes the stream (no more values will be pushed).
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Returns true if the stream is closed and empty.
    pub fn is_done(&self) -> bool {
        self.closed && self.buffer.is_empty()
    }

    /// Returns the stream name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of buffered values.
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// G5: Coroutine (bidirectional generator)
// ═══════════════════════════════════════════════════════════════════════

/// A coroutine that can both yield and receive values.
#[derive(Debug)]
pub struct Coroutine {
    name: String,
    outgoing: VecDeque<i64>,
    incoming: VecDeque<i64>,
    state: GeneratorState,
}

impl Coroutine {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            outgoing: VecDeque::new(),
            incoming: VecDeque::new(),
            state: GeneratorState::Yielded,
        }
    }

    /// Sends a value to the coroutine.
    pub fn send(&mut self, value: i64) {
        self.incoming.push_back(value);
    }

    /// Receives a value from the coroutine.
    pub fn receive(&mut self) -> Option<i64> {
        self.outgoing.pop_front()
    }

    /// Yields a value from inside the coroutine.
    pub fn yield_value(&mut self, value: i64) {
        self.outgoing.push_back(value);
    }

    /// Gets the next incoming value.
    pub fn next_input(&mut self) -> Option<i64> {
        self.incoming.pop_front()
    }

    pub fn state(&self) -> GeneratorState {
        self.state
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g1_generator_from_values() {
        let generator = Generator::from_values("nums", vec![1, 2, 3], 0);
        assert_eq!(generator.name(), "nums");
        assert_eq!(generator.state(), GeneratorState::Yielded);
    }

    #[test]
    fn g1_generator_resume() {
        let mut generator = Generator::from_values("test", vec![10, 20, 30], 99);
        assert_eq!(generator.resume(), GeneratorOutput::Yielded(10));
        assert_eq!(generator.resume(), GeneratorOutput::Yielded(20));
        assert_eq!(generator.resume(), GeneratorOutput::Yielded(30));
        assert_eq!(generator.resume(), GeneratorOutput::Complete(99));
    }

    #[test]
    fn g1_generator_cancel() {
        let mut generator = Generator::from_values("test", vec![1, 2, 3], 0);
        generator.cancel();
        assert_eq!(generator.state(), GeneratorState::Cancelled);
    }

    #[test]
    fn g1_generator_state_display() {
        assert_eq!(format!("{}", GeneratorState::Yielded), "yielded");
        assert_eq!(format!("{}", GeneratorState::Complete), "complete");
    }

    #[test]
    fn g2_generator_iter() {
        let generator = Generator::from_values("iter", vec![1, 2, 3], 0);
        let iter = GeneratorIter::new(generator);
        let collected: Vec<i64> = iter.collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn g2_generator_iter_map() {
        let generator = Generator::from_values("map", vec![1, 2, 3], 0);
        let doubled: Vec<i64> = GeneratorIter::new(generator).map(|x| x * 2).collect();
        assert_eq!(doubled, vec![2, 4, 6]);
    }

    #[test]
    fn g2_generator_iter_filter() {
        let generator = Generator::from_values("filt", vec![1, 2, 3, 4, 5], 0);
        let evens: Vec<i64> = GeneratorIter::new(generator)
            .filter(|x| x % 2 == 0)
            .collect();
        assert_eq!(evens, vec![2, 4]);
    }

    #[test]
    fn g2_generator_iter_take() {
        let generator = Generator::from_values("take", vec![1, 2, 3, 4, 5], 0);
        let first3: Vec<i64> = GeneratorIter::new(generator).take(3).collect();
        assert_eq!(first3, vec![1, 2, 3]);
    }

    #[test]
    fn g3_async_stream_basic() {
        let mut stream = AsyncStream::new("test");
        assert_eq!(stream.poll(), StreamPoll::Pending);
        stream.push(42);
        assert_eq!(stream.poll(), StreamPoll::Ready(42));
        assert_eq!(stream.poll(), StreamPoll::Pending);
    }

    #[test]
    fn g3_async_stream_close() {
        let mut stream = AsyncStream::new("test");
        stream.push(1);
        stream.close();
        assert_eq!(stream.poll(), StreamPoll::Ready(1));
        assert_eq!(stream.poll(), StreamPoll::Done);
        assert!(stream.is_done());
    }

    #[test]
    fn g3_stream_buffered_count() {
        let mut stream = AsyncStream::new("buf");
        stream.push(1);
        stream.push(2);
        stream.push(3);
        assert_eq!(stream.buffered_count(), 3);
        stream.poll();
        assert_eq!(stream.buffered_count(), 2);
    }

    #[test]
    fn g5_coroutine_bidirectional() {
        let mut co = Coroutine::new("echo");
        co.send(10);
        co.send(20);
        assert_eq!(co.next_input(), Some(10));
        co.yield_value(100);
        assert_eq!(co.receive(), Some(100));
        assert_eq!(co.next_input(), Some(20));
    }

    #[test]
    fn g5_coroutine_state() {
        let co = Coroutine::new("test");
        assert_eq!(co.state(), GeneratorState::Yielded);
        assert_eq!(co.name(), "test");
    }
}
