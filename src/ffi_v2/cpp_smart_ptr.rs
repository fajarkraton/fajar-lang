//! Sprint E2: C++ Smart Pointer & RAII Bridge
//!
//! Maps C++ smart pointer semantics (`unique_ptr`, `shared_ptr`, `weak_ptr`)
//! to Fajar Lang ownership, with RAII guards, move/copy traits, custom deleters,
//! ref-qualified method dispatch, and exception safety.
//!
//! All types are simulated (no real C++ runtime). Values are stored via
//! `Box<dyn Any>` type erasure with string-based type names for introspection.

use std::any::Any;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Global ID generator for tracking pointer identity across moves.
static NEXT_PTR_ID: AtomicUsize = AtomicUsize::new(1);

fn next_id() -> usize {
    NEXT_PTR_ID.fetch_add(1, Ordering::Relaxed)
}

/// Type alias for a deleter callback.
type Deleter = Box<dyn Fn(&str) + Send>;

/// Type alias for a deleter factory.
type DeleterFactory = Box<dyn Fn(&str) -> Deleter + Send>;

/// Type alias for the clone function used in `CppCopyable`.
type CloneFn = Arc<dyn Fn(&dyn Any) -> Box<dyn Any> + Send + Sync>;

// ═══════════════════════════════════════════════════════════════════════
// E2.1: unique_ptr<T> — move-only ownership mapped to Fajar ownership
// ═══════════════════════════════════════════════════════════════════════

/// A simulated C++ `unique_ptr<T>` — move-only smart pointer.
///
/// After `take()`, the source becomes invalidated (holds `None`).
/// Maps to Fajar Lang's single-owner move semantics.
pub struct UniquePtr {
    /// The owned value (erased). `None` after a move.
    inner: Option<Box<dyn Any>>,
    /// C++ type name for diagnostics.
    type_name: String,
    /// Unique identity for tracking.
    id: usize,
    /// Optional custom deleter (E2.7).
    deleter: Option<Deleter>,
}

impl UniquePtr {
    /// Create a new `UniquePtr` wrapping a value.
    pub fn new<T: Any + 'static>(value: T, type_name: &str) -> Self {
        Self {
            inner: Some(Box::new(value)),
            type_name: type_name.to_string(),
            id: next_id(),
            deleter: None,
        }
    }

    /// Create a `UniquePtr` with a custom deleter (E2.7).
    pub fn with_deleter<T: Any + 'static>(value: T, type_name: &str, deleter: Deleter) -> Self {
        Self {
            inner: Some(Box::new(value)),
            type_name: type_name.to_string(),
            id: next_id(),
            deleter: Some(deleter),
        }
    }

    /// Take ownership of the inner value, invalidating this pointer.
    ///
    /// Returns `None` if already moved from.
    pub fn take(&mut self) -> Option<Box<dyn Any>> {
        self.inner.take()
    }

    /// Check whether this pointer still owns a value.
    pub fn is_valid(&self) -> bool {
        self.inner.is_some()
    }

    /// Borrow the inner value by reference, if still valid.
    pub fn get(&self) -> Option<&dyn Any> {
        self.inner.as_deref()
    }

    /// Borrow the inner value mutably, if still valid.
    pub fn get_mut(&mut self) -> Option<&mut dyn Any> {
        self.inner.as_deref_mut()
    }

    /// Return the C++ type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Return the unique pointer ID for tracking.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Release ownership without invoking the deleter.
    pub fn release(&mut self) -> Option<Box<dyn Any>> {
        // Detach the deleter so drop won't run it.
        self.deleter = None;
        self.inner.take()
    }
}

impl Drop for UniquePtr {
    fn drop(&mut self) {
        if self.inner.is_some() {
            if let Some(ref deleter) = self.deleter {
                deleter(&self.type_name);
            }
        }
    }
}

impl fmt::Debug for UniquePtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UniquePtr")
            .field("is_valid", &self.inner.is_some())
            .field("type_name", &self.type_name)
            .field("id", &self.id)
            .field("has_deleter", &self.deleter.is_some())
            .finish()
    }
}

impl fmt::Display for UniquePtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "unique_ptr<{}>(live, id={})", self.type_name, self.id)
        } else {
            write!(
                f,
                "unique_ptr<{}>(moved-from, id={})",
                self.type_name, self.id
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.2: shared_ptr<T> — reference-counted shared ownership
// ═══════════════════════════════════════════════════════════════════════

/// Internal control block shared among `SharedPtr` and `WeakPtr` instances.
///
/// Wrapped in `UnsafeCell` to allow interior mutability through raw pointers.
/// This is sound because the simulation is single-threaded.
struct ControlBlock {
    /// The owned value. Becomes `None` when strong count reaches 0.
    value: Option<Box<dyn Any>>,
    /// Number of `SharedPtr` instances referencing this block.
    strong_count: usize,
    /// Number of `WeakPtr` instances referencing this block.
    weak_count: usize,
    /// C++ type name for diagnostics.
    type_name: String,
    /// Unique ID for the control block.
    id: usize,
    /// Drop callbacks (E2.4).
    drop_callbacks: Vec<Deleter>,
}

impl fmt::Debug for ControlBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlBlock")
            .field("has_value", &self.value.is_some())
            .field("strong_count", &self.strong_count)
            .field("weak_count", &self.weak_count)
            .field("type_name", &self.type_name)
            .field("id", &self.id)
            .finish()
    }
}

/// Wrapper around `ControlBlock` using `UnsafeCell` for interior mutability.
///
/// Multiple `SharedPtr` and `WeakPtr` instances hold a raw pointer to this cell.
/// All access is single-threaded (simulated C++ smart pointers).
struct ControlBlockCell {
    cell: UnsafeCell<ControlBlock>,
}

impl ControlBlockCell {
    /// Create a new control block cell.
    fn new(block: ControlBlock) -> Self {
        Self {
            cell: UnsafeCell::new(block),
        }
    }
}

/// A simulated C++ `shared_ptr<T>` — reference-counted smart pointer.
///
/// `clone()` increments the strong count; dropping decrements it.
/// When the last strong reference is dropped, the value is destroyed.
pub struct SharedPtr {
    /// Pointer to the control block cell. Uses `*mut` because multiple
    /// `SharedPtr` and `WeakPtr` instances share the same block.
    // SAFETY: control is heap-allocated via Box::into_raw and freed only when
    // both strong_count and weak_count reach 0. All access is single-threaded
    // (simulated, not real multi-threaded shared_ptr).
    control: *mut ControlBlockCell,
}

impl SharedPtr {
    /// Create a new `SharedPtr` wrapping a value.
    pub fn new<T: Any + 'static>(value: T, type_name: &str) -> Self {
        let cell = Box::new(ControlBlockCell::new(ControlBlock {
            value: Some(Box::new(value)),
            strong_count: 1,
            weak_count: 0,
            type_name: type_name.to_string(),
            id: next_id(),
            drop_callbacks: Vec::new(),
        }));
        Self {
            control: Box::into_raw(cell),
        }
    }

    /// Register a drop callback invoked when the value is destroyed (E2.4).
    pub fn on_drop(&self, callback: Deleter) {
        self.with_block_mut(|block| {
            block.drop_callbacks.push(callback);
        });
    }

    /// Get the current strong reference count.
    pub fn strong_count(&self) -> usize {
        self.with_block(|b| b.strong_count)
    }

    /// Get the current weak reference count.
    pub fn weak_count(&self) -> usize {
        self.with_block(|b| b.weak_count)
    }

    /// Borrow the inner value by reference.
    pub fn get(&self) -> Option<&dyn Any> {
        // SAFETY: control is valid while any SharedPtr exists.
        // The returned reference borrows self, preventing concurrent mutation.
        let block = unsafe { &*(*self.control).cell.get() };
        block.value.as_deref()
    }

    /// Borrow the inner value mutably.
    pub fn get_mut(&mut self) -> Option<&mut dyn Any> {
        // SAFETY: control is valid, and &mut self prevents aliased access.
        let block = unsafe { &mut *(*self.control).cell.get() };
        block.value.as_deref_mut()
    }

    /// Return the C++ type name.
    pub fn type_name(&self) -> &str {
        // SAFETY: control is valid while any SharedPtr exists.
        let block = unsafe { &*(*self.control).cell.get() };
        &block.type_name
    }

    /// Return the control block unique ID.
    pub fn id(&self) -> usize {
        self.with_block(|b| b.id)
    }

    /// Create a `WeakPtr` from this `SharedPtr` (E2.3).
    pub fn downgrade(&self) -> WeakPtr {
        self.with_block_mut(|block| {
            block.weak_count += 1;
        });
        WeakPtr {
            control: self.control,
        }
    }

    /// Access the control block immutably via a closure.
    fn with_block<R>(&self, f: impl FnOnce(&ControlBlock) -> R) -> R {
        // SAFETY: control is valid while any SharedPtr exists (strong_count >= 1).
        // UnsafeCell provides interior mutability. Single-threaded access.
        let block = unsafe { &*(*self.control).cell.get() };
        f(block)
    }

    /// Access the control block mutably via a closure.
    fn with_block_mut<R>(&self, f: impl FnOnce(&mut ControlBlock) -> R) -> R {
        // SAFETY: control is valid while any SharedPtr exists (strong_count >= 1).
        // UnsafeCell provides interior mutability. Single-threaded access — no data races.
        let block = unsafe { &mut *(*self.control).cell.get() };
        f(block)
    }
}

impl Clone for SharedPtr {
    fn clone(&self) -> Self {
        self.with_block_mut(|block| {
            block.strong_count += 1;
        });
        Self {
            control: self.control,
        }
    }
}

impl Drop for SharedPtr {
    fn drop(&mut self) {
        // SAFETY: control is valid while any SharedPtr or WeakPtr exists.
        // UnsafeCell provides interior mutability.
        let block = unsafe { &mut *(*self.control).cell.get() };
        block.strong_count -= 1;
        if block.strong_count == 0 {
            // Fire drop callbacks before destroying value.
            let type_name = block.type_name.clone();
            for cb in &block.drop_callbacks {
                cb(&type_name);
            }
            block.drop_callbacks.clear();
            block.value = None;

            if block.weak_count == 0 {
                // SAFETY: No remaining SharedPtr or WeakPtr — free the block.
                let _ = unsafe { Box::from_raw(self.control) };
            }
        }
    }
}

impl fmt::Debug for SharedPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with_block(|block| {
            f.debug_struct("SharedPtr")
                .field("type_name", &block.type_name)
                .field("id", &block.id)
                .field("strong_count", &block.strong_count)
                .field("weak_count", &block.weak_count)
                .finish()
        })
    }
}

impl fmt::Display for SharedPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with_block(|block| {
            write!(
                f,
                "shared_ptr<{}>(strong={}, weak={}, id={})",
                block.type_name, block.strong_count, block.weak_count, block.id
            )
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.3: weak_ptr<T> — non-owning, upgrade to shared
// ═══════════════════════════════════════════════════════════════════════

/// A simulated C++ `weak_ptr<T>` — non-owning observer of a `SharedPtr`.
///
/// `upgrade()` returns `Some(SharedPtr)` if the value is still alive,
/// `None` if all strong references have been dropped.
pub struct WeakPtr {
    /// Pointer to the shared control block cell.
    // SAFETY: control is heap-allocated and freed only when both strong_count
    // and weak_count reach 0. See SharedPtr::drop and WeakPtr::drop.
    control: *mut ControlBlockCell,
}

impl WeakPtr {
    /// Attempt to upgrade to a `SharedPtr`.
    ///
    /// Returns `Some` if strong references still exist, `None` otherwise.
    pub fn upgrade(&self) -> Option<SharedPtr> {
        // SAFETY: control is valid while any WeakPtr exists.
        // UnsafeCell provides interior mutability.
        let block = unsafe { &mut *(*self.control).cell.get() };
        if block.strong_count > 0 {
            block.strong_count += 1;
            Some(SharedPtr {
                control: self.control,
            })
        } else {
            None
        }
    }

    /// Check if the referenced value is still alive (strong_count > 0).
    pub fn is_expired(&self) -> bool {
        self.with_block(|b| b.strong_count == 0)
    }

    /// Get the current strong reference count observed by this weak pointer.
    pub fn strong_count(&self) -> usize {
        self.with_block(|b| b.strong_count)
    }

    /// Access the control block immutably via a closure.
    fn with_block<R>(&self, f: impl FnOnce(&ControlBlock) -> R) -> R {
        // SAFETY: control is valid while any WeakPtr exists (weak_count >= 1).
        // UnsafeCell provides interior mutability. Single-threaded access.
        let block = unsafe { &*(*self.control).cell.get() };
        f(block)
    }
}

impl Clone for WeakPtr {
    fn clone(&self) -> Self {
        // SAFETY: control is valid while any WeakPtr exists.
        // UnsafeCell provides interior mutability.
        let block = unsafe { &mut *(*self.control).cell.get() };
        block.weak_count += 1;
        Self {
            control: self.control,
        }
    }
}

impl Drop for WeakPtr {
    fn drop(&mut self) {
        // SAFETY: control is valid while any SharedPtr or WeakPtr exists.
        // UnsafeCell provides interior mutability.
        let block = unsafe { &mut *(*self.control).cell.get() };
        block.weak_count -= 1;
        if block.weak_count == 0 && block.strong_count == 0 {
            // SAFETY: No remaining SharedPtr or WeakPtr — free the block.
            let _ = unsafe { Box::from_raw(self.control) };
        }
    }
}

impl fmt::Debug for WeakPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with_block(|block| {
            f.debug_struct("WeakPtr")
                .field("type_name", &block.type_name)
                .field("id", &block.id)
                .field("expired", &(block.strong_count == 0))
                .finish()
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.4: RAII Bridge — destructor tracking, drop callbacks
// ═══════════════════════════════════════════════════════════════════════

/// A general-purpose RAII guard that runs a callback on drop.
///
/// Models C++ destructor semantics for Fajar Lang's drop system.
pub struct RaiiGuard {
    /// Human-readable label for this guard.
    label: String,
    /// Callback invoked when the guard is dropped.
    on_drop: Option<Box<dyn FnOnce() + Send>>,
    /// Whether the guard has been disarmed (callback will NOT fire).
    disarmed: bool,
}

impl RaiiGuard {
    /// Create a new RAII guard with a label and drop callback.
    pub fn new(label: &str, on_drop: Box<dyn FnOnce() + Send>) -> Self {
        Self {
            label: label.to_string(),
            on_drop: Some(on_drop),
            disarmed: false,
        }
    }

    /// Disarm the guard so the callback will not fire on drop.
    pub fn disarm(&mut self) {
        self.disarmed = true;
    }

    /// Check whether this guard is still armed.
    pub fn is_armed(&self) -> bool {
        !self.disarmed
    }

    /// Return the guard label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Drop for RaiiGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            if let Some(cb) = self.on_drop.take() {
                cb();
            }
        }
    }
}

impl fmt::Debug for RaiiGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RaiiGuard")
            .field("label", &self.label)
            .field("armed", &(!self.disarmed))
            .finish()
    }
}

/// Tracks RAII destructor registrations across a C++ bridge session.
///
/// Each registered type has a list of destructor callbacks keyed by instance ID.
#[derive(Default)]
pub struct DestructorTracker {
    /// Map from type_name to list of (instance_id, was_destroyed).
    records: HashMap<String, Vec<DestructorRecord>>,
}

/// A single destructor lifecycle record.
#[derive(Debug, Clone)]
pub struct DestructorRecord {
    /// Instance identifier.
    pub instance_id: usize,
    /// Whether the destructor has been called.
    pub destroyed: bool,
}

impl DestructorTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that an instance of `type_name` was created.
    pub fn register(&mut self, type_name: &str, instance_id: usize) {
        self.records
            .entry(type_name.to_string())
            .or_default()
            .push(DestructorRecord {
                instance_id,
                destroyed: false,
            });
    }

    /// Mark an instance as destroyed.
    pub fn mark_destroyed(&mut self, type_name: &str, instance_id: usize) {
        if let Some(records) = self.records.get_mut(type_name) {
            for rec in records.iter_mut() {
                if rec.instance_id == instance_id {
                    rec.destroyed = true;
                    return;
                }
            }
        }
    }

    /// Check if an instance has been destroyed.
    pub fn is_destroyed(&self, type_name: &str, instance_id: usize) -> bool {
        self.records
            .get(type_name)
            .and_then(|recs| recs.iter().find(|r| r.instance_id == instance_id))
            .is_some_and(|r| r.destroyed)
    }

    /// Count how many instances of a type are still alive (not destroyed).
    pub fn alive_count(&self, type_name: &str) -> usize {
        self.records
            .get(type_name)
            .map_or(0, |recs| recs.iter().filter(|r| !r.destroyed).count())
    }

    /// Get all records for a given type.
    pub fn records_for(&self, type_name: &str) -> &[DestructorRecord] {
        self.records.get(type_name).map_or(&[], |v| v.as_slice())
    }
}

impl fmt::Debug for DestructorTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DestructorTracker")
            .field("types_tracked", &self.records.len())
            .finish()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.5: Move Semantics — CppMove trait, object invalidation after move
// ═══════════════════════════════════════════════════════════════════════

/// Trait modeling C++ move semantics for Fajar Lang bridge objects.
///
/// After `cpp_move()`, the source object should be in a "moved-from" state
/// (valid but unspecified — typically empty/null).
pub trait CppMove {
    /// Move the value out of `self`, leaving `self` in a moved-from state.
    ///
    /// Returns `Some(Self)` on success, `None` if already moved-from.
    fn cpp_move(&mut self) -> Option<Self>
    where
        Self: Sized;

    /// Check whether this object is in a moved-from (invalidated) state.
    fn is_moved_from(&self) -> bool;
}

impl CppMove for UniquePtr {
    fn cpp_move(&mut self) -> Option<Self> {
        self.inner.as_ref()?;
        let moved = UniquePtr {
            inner: self.inner.take(),
            type_name: self.type_name.clone(),
            id: next_id(),
            deleter: None, // deleter does NOT transfer on move (intentional)
        };
        Some(moved)
    }

    fn is_moved_from(&self) -> bool {
        self.inner.is_none()
    }
}

/// A generic movable C++ value wrapper.
///
/// Wraps an arbitrary value and tracks whether it has been moved from.
#[derive(Debug)]
pub struct CppMovable {
    /// The value (erased). `None` after a move.
    inner: Option<Box<dyn Any>>,
    /// C++ type name.
    type_name: String,
}

impl CppMovable {
    /// Create a new movable wrapper.
    pub fn new<T: Any + 'static>(value: T, type_name: &str) -> Self {
        Self {
            inner: Some(Box::new(value)),
            type_name: type_name.to_string(),
        }
    }

    /// Access the wrapped value.
    pub fn get(&self) -> Option<&dyn Any> {
        self.inner.as_deref()
    }

    /// Return the C++ type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }
}

impl CppMove for CppMovable {
    fn cpp_move(&mut self) -> Option<Self> {
        self.inner.as_ref()?;
        Some(CppMovable {
            inner: self.inner.take(),
            type_name: self.type_name.clone(),
        })
    }

    fn is_moved_from(&self) -> bool {
        self.inner.is_none()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.6: Copy Semantics — CppCopy trait, deep clone
// ═══════════════════════════════════════════════════════════════════════

/// Trait modeling C++ copy semantics for Fajar Lang bridge objects.
///
/// `cpp_copy()` performs a deep clone, producing an independent copy.
pub trait CppCopy {
    /// Produce a deep copy of this object.
    ///
    /// Returns `None` if the object is in a moved-from state and cannot be copied.
    fn cpp_copy(&self) -> Option<Self>
    where
        Self: Sized;
}

/// A copyable C++ value wrapper using a clone function.
///
/// Since `Box<dyn Any>` is not `Clone`, we store a shared clone function
/// (via `Arc`) that knows how to duplicate the inner value.
pub struct CppCopyable {
    /// The value (erased).
    inner: Option<Box<dyn Any>>,
    /// C++ type name.
    type_name: String,
    /// Clone function — shared via `Arc` so copies and moves can reuse it.
    clone_fn: CloneFn,
}

impl CppCopyable {
    /// Create a new copyable wrapper with a clone function.
    pub fn new<T: Any + Clone + 'static>(value: T, type_name: &str) -> Self {
        Self {
            inner: Some(Box::new(value.clone())),
            type_name: type_name.to_string(),
            clone_fn: Arc::new(|any: &dyn Any| {
                let val = any
                    .downcast_ref::<T>()
                    .expect("CppCopyable clone_fn: type mismatch");
                Box::new(val.clone()) as Box<dyn Any>
            }),
        }
    }

    /// Access the wrapped value.
    pub fn get(&self) -> Option<&dyn Any> {
        self.inner.as_deref()
    }

    /// Return the C++ type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }
}

impl CppCopy for CppCopyable {
    fn cpp_copy(&self) -> Option<Self> {
        let inner_ref = self.inner.as_deref()?;
        let cloned = (self.clone_fn)(inner_ref);
        Some(CppCopyable {
            inner: Some(cloned),
            type_name: self.type_name.clone(),
            clone_fn: Arc::clone(&self.clone_fn),
        })
    }
}

impl CppMove for CppCopyable {
    fn cpp_move(&mut self) -> Option<Self> {
        self.inner.as_ref()?;
        let value = self.inner.take()?;
        Some(CppCopyable {
            inner: Some(value),
            type_name: self.type_name.clone(),
            clone_fn: Arc::clone(&self.clone_fn),
        })
    }

    fn is_moved_from(&self) -> bool {
        self.inner.is_none()
    }
}

impl fmt::Debug for CppCopyable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CppCopyable")
            .field("has_value", &self.inner.is_some())
            .field("type_name", &self.type_name)
            .finish()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.7: Custom Deleters — unique_ptr<T, Deleter>
// ═══════════════════════════════════════════════════════════════════════

// Custom deleters are integrated into UniquePtr::with_deleter (E2.1).
// The deleter fires on Drop if the pointer still holds a value.
// UniquePtr::release() detaches the deleter for manual lifetime control.

/// Registry of named custom deleters for C++ types.
///
/// Allows registering reusable deleters by type name, which can then
/// be associated with `UniquePtr` instances.
#[derive(Default)]
pub struct DeleterRegistry {
    /// Map from type_name to a deleter factory.
    deleters: HashMap<String, DeleterFactory>,
}

impl DeleterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a deleter factory for a given type name.
    pub fn register(&mut self, type_name: &str, factory: DeleterFactory) {
        self.deleters.insert(type_name.to_string(), factory);
    }

    /// Create a deleter for the given type, if one is registered.
    pub fn create_deleter(&self, type_name: &str) -> Option<Deleter> {
        self.deleters
            .get(type_name)
            .map(|factory| factory(type_name))
    }

    /// Check if a deleter is registered for a type.
    pub fn has_deleter(&self, type_name: &str) -> bool {
        self.deleters.contains_key(type_name)
    }
}

impl fmt::Debug for DeleterRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeleterRegistry")
            .field(
                "registered_types",
                &self.deleters.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.8: Ref-Qualified Methods — & vs && overload selection
// ═══════════════════════════════════════════════════════════════════════

/// C++ reference qualifier for method overloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefQualifier {
    /// No ref-qualifier (default).
    None,
    /// Lvalue reference qualifier (`&`).
    LValue,
    /// Rvalue reference qualifier (`&&`).
    RValue,
}

impl fmt::Display for RefQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, ""),
            Self::LValue => write!(f, " &"),
            Self::RValue => write!(f, " &&"),
        }
    }
}

/// A ref-qualified method descriptor for dispatch.
#[derive(Debug, Clone)]
pub struct RefQualifiedMethod {
    /// Method name.
    pub name: String,
    /// Reference qualifier.
    pub qualifier: RefQualifier,
    /// Return type (as string for simulation).
    pub return_type: String,
    /// Whether this method is const.
    pub is_const: bool,
}

/// Selects the best method overload based on the value category.
///
/// In C++, calling a method on an lvalue selects the `&` overload,
/// while calling on an rvalue (moved-from temporary) selects `&&`.
pub fn select_overload<'a>(
    methods: &'a [RefQualifiedMethod],
    name: &str,
    is_rvalue: bool,
) -> Option<&'a RefQualifiedMethod> {
    let candidates: Vec<&RefQualifiedMethod> = methods.iter().filter(|m| m.name == name).collect();

    if candidates.is_empty() {
        return None;
    }

    let target_qual = if is_rvalue {
        RefQualifier::RValue
    } else {
        RefQualifier::LValue
    };

    // First try exact match.
    if let Some(exact) = candidates.iter().find(|m| m.qualifier == target_qual) {
        return Some(exact);
    }

    // Fall back to unqualified overload.
    if let Some(unqual) = candidates
        .iter()
        .find(|m| m.qualifier == RefQualifier::None)
    {
        return Some(unqual);
    }

    // Last resort: return the first candidate.
    candidates.into_iter().next()
}

// ═══════════════════════════════════════════════════════════════════════
// E2.9: Exception Safety — C++ exception -> Fajar Result<T, CppException>
// ═══════════════════════════════════════════════════════════════════════

/// A captured C++ exception mapped to Fajar Lang's error system.
#[derive(Debug, Clone)]
pub struct CppException {
    /// The C++ exception type name (e.g., `std::runtime_error`).
    pub type_name: String,
    /// The exception message (`what()`).
    pub message: String,
    /// Simulated backtrace frames.
    pub backtrace: Vec<String>,
}

impl CppException {
    /// Create a new `CppException`.
    pub fn new(type_name: &str, message: &str) -> Self {
        Self {
            type_name: type_name.to_string(),
            message: message.to_string(),
            backtrace: Vec::new(),
        }
    }

    /// Create a `CppException` with a backtrace.
    pub fn with_backtrace(type_name: &str, message: &str, backtrace: Vec<String>) -> Self {
        Self {
            type_name: type_name.to_string(),
            message: message.to_string(),
            backtrace,
        }
    }

    /// Create a `std::runtime_error`.
    pub fn runtime_error(message: &str) -> Self {
        Self::new("std::runtime_error", message)
    }

    /// Create a `std::logic_error`.
    pub fn logic_error(message: &str) -> Self {
        Self::new("std::logic_error", message)
    }

    /// Create a `std::out_of_range`.
    pub fn out_of_range(message: &str) -> Self {
        Self::new("std::out_of_range", message)
    }

    /// Create a `std::bad_alloc`.
    pub fn bad_alloc() -> Self {
        Self::new("std::bad_alloc", "memory allocation failed")
    }
}

impl fmt::Display for CppException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C++ exception [{}]: {}", self.type_name, self.message)?;
        if !self.backtrace.is_empty() {
            write!(f, "\n  backtrace:")?;
            for (i, frame) in self.backtrace.iter().enumerate() {
                write!(f, "\n    #{i}: {frame}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for CppException {}

/// Execute a simulated C++ call that may throw, returning a Fajar `Result`.
///
/// The callback returns `Ok(T)` on success or `Err(CppException)` if the
/// simulated C++ function "throws".
pub fn cpp_try<T, F>(f: F) -> Result<T, CppException>
where
    F: FnOnce() -> Result<T, CppException>,
{
    f()
}

/// Map a C++ exception type to a Fajar error code.
pub fn exception_to_error_code(exception: &CppException) -> &'static str {
    match exception.type_name.as_str() {
        "std::runtime_error" => "RE001",
        "std::logic_error" => "RE002",
        "std::out_of_range" => "RE003",
        "std::bad_alloc" => "ME001",
        "std::invalid_argument" => "RE004",
        "std::overflow_error" => "RE005",
        "std::underflow_error" => "RE006",
        _ => "RE008", // generic foreign exception
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E2.10: Tests (24 tests)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

    // --- E2.1: UniquePtr ---

    #[test]
    fn e2_1_unique_ptr_basic_ownership() {
        let ptr = UniquePtr::new(42_i32, "int");
        assert!(ptr.is_valid());
        assert_eq!(ptr.type_name(), "int");

        let val = ptr.get().unwrap();
        assert_eq!(*val.downcast_ref::<i32>().unwrap(), 42);
    }

    #[test]
    fn e2_1_unique_ptr_take_invalidates_source() {
        let mut ptr = UniquePtr::new("hello".to_string(), "std::string");
        assert!(ptr.is_valid());

        let taken = ptr.take();
        assert!(taken.is_some());
        assert!(!ptr.is_valid());

        // Second take returns None.
        assert!(ptr.take().is_none());
    }

    #[test]
    fn e2_1_unique_ptr_display() {
        let ptr = UniquePtr::new(100_u64, "uint64_t");
        let display = format!("{ptr}");
        assert!(display.contains("unique_ptr<uint64_t>"));
        assert!(display.contains("live"));
    }

    // --- E2.2: SharedPtr ---

    #[test]
    fn e2_2_shared_ptr_ref_counting() {
        let sp1 = SharedPtr::new(vec![1, 2, 3], "std::vector<int>");
        assert_eq!(sp1.strong_count(), 1);
        assert_eq!(sp1.weak_count(), 0);

        let sp2 = sp1.clone();
        assert_eq!(sp1.strong_count(), 2);
        assert_eq!(sp2.strong_count(), 2);

        drop(sp2);
        assert_eq!(sp1.strong_count(), 1);
    }

    #[test]
    fn e2_2_shared_ptr_value_access() {
        let sp = SharedPtr::new(1.25_f64, "double");
        let val = sp.get().unwrap();
        assert!((*val.downcast_ref::<f64>().unwrap() - 1.25).abs() < f64::EPSILON);
        assert_eq!(sp.type_name(), "double");
    }

    // --- E2.3: WeakPtr ---

    #[test]
    fn e2_3_weak_ptr_upgrade_while_alive() {
        let sp = SharedPtr::new(99_i32, "int");
        let wp = sp.downgrade();
        assert!(!wp.is_expired());
        assert_eq!(wp.strong_count(), 1);

        let upgraded = wp.upgrade();
        assert!(upgraded.is_some());
        assert_eq!(sp.strong_count(), 2);
    }

    #[test]
    fn e2_3_weak_ptr_expired_after_drop() {
        let sp = SharedPtr::new(42_i32, "int");
        let wp = sp.downgrade();
        assert!(!wp.is_expired());

        drop(sp);
        assert!(wp.is_expired());
        assert!(wp.upgrade().is_none());
    }

    // --- E2.4: RAII Guard ---

    #[test]
    fn e2_4_raii_guard_fires_on_drop() {
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        {
            let _guard = RaiiGuard::new(
                "test-guard",
                Box::new(move || {
                    fired_clone.store(true, AtomicOrdering::SeqCst);
                }),
            );
            assert!(!fired.load(AtomicOrdering::SeqCst));
        }
        assert!(fired.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn e2_4_raii_guard_disarm() {
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        {
            let mut guard = RaiiGuard::new(
                "disarmable",
                Box::new(move || {
                    fired_clone.store(true, AtomicOrdering::SeqCst);
                }),
            );
            assert!(guard.is_armed());
            guard.disarm();
            assert!(!guard.is_armed());
        }
        // Callback did NOT fire because we disarmed.
        assert!(!fired.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn e2_4_destructor_tracker() {
        let mut tracker = DestructorTracker::new();
        tracker.register("Widget", 1);
        tracker.register("Widget", 2);
        tracker.register("Gadget", 3);

        assert_eq!(tracker.alive_count("Widget"), 2);
        assert!(!tracker.is_destroyed("Widget", 1));

        tracker.mark_destroyed("Widget", 1);
        assert!(tracker.is_destroyed("Widget", 1));
        assert_eq!(tracker.alive_count("Widget"), 1);

        assert_eq!(tracker.records_for("Widget").len(), 2);
        assert_eq!(tracker.alive_count("Gadget"), 1);
    }

    // --- E2.5: CppMove ---

    #[test]
    fn e2_5_cpp_move_unique_ptr() {
        let mut src = UniquePtr::new(String::from("data"), "std::string");
        assert!(!src.is_moved_from());

        let dst = src.cpp_move();
        assert!(dst.is_some());
        assert!(src.is_moved_from());

        // Moving again returns None.
        assert!(src.cpp_move().is_none());
    }

    #[test]
    fn e2_5_cpp_movable_generic() {
        let mut val = CppMovable::new(vec![1, 2, 3], "std::vector<int>");
        assert!(!val.is_moved_from());
        assert!(val.get().is_some());

        let moved = val.cpp_move().unwrap();
        assert!(val.is_moved_from());
        assert!(moved.get().is_some());
        assert_eq!(moved.type_name(), "std::vector<int>");
    }

    // --- E2.6: CppCopy ---

    #[test]
    fn e2_6_cpp_copyable_deep_clone() {
        let original = CppCopyable::new(42_i32, "int");
        let copy = original.cpp_copy().unwrap();

        let orig_val = original.get().unwrap().downcast_ref::<i32>().unwrap();
        let copy_val = copy.get().unwrap().downcast_ref::<i32>().unwrap();
        assert_eq!(*orig_val, *copy_val);
        assert_eq!(copy.type_name(), "int");
    }

    // --- E2.7: Custom Deleters ---

    #[test]
    fn e2_7_custom_deleter_fires_on_drop() {
        let deleted = Arc::new(AtomicBool::new(false));
        let deleted_clone = deleted.clone();
        {
            let _ptr = UniquePtr::with_deleter(
                100_i32,
                "CustomResource",
                Box::new(move |_type_name| {
                    deleted_clone.store(true, AtomicOrdering::SeqCst);
                }),
            );
            assert!(!deleted.load(AtomicOrdering::SeqCst));
        }
        assert!(deleted.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn e2_7_release_suppresses_deleter() {
        let deleted = Arc::new(AtomicBool::new(false));
        let deleted_clone = deleted.clone();
        {
            let mut ptr = UniquePtr::with_deleter(
                200_i32,
                "ManualResource",
                Box::new(move |_type_name| {
                    deleted_clone.store(true, AtomicOrdering::SeqCst);
                }),
            );
            let _released = ptr.release();
        }
        // Deleter did NOT fire because we released.
        assert!(!deleted.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn e2_7_deleter_registry() {
        let mut registry = DeleterRegistry::new();
        registry.register(
            "FileHandle",
            Box::new(|type_name| {
                let tn = type_name.to_string();
                Box::new(move |_| {
                    let _ = &tn; // would close file handle
                })
            }),
        );
        assert!(registry.has_deleter("FileHandle"));
        assert!(!registry.has_deleter("Unknown"));
        assert!(registry.create_deleter("FileHandle").is_some());
    }

    // --- E2.8: Ref-Qualified Methods ---

    #[test]
    fn e2_8_ref_qualifier_overload_selection() {
        let methods = vec![
            RefQualifiedMethod {
                name: "value".to_string(),
                qualifier: RefQualifier::LValue,
                return_type: "const T&".to_string(),
                is_const: true,
            },
            RefQualifiedMethod {
                name: "value".to_string(),
                qualifier: RefQualifier::RValue,
                return_type: "T&&".to_string(),
                is_const: false,
            },
        ];

        let lvalue = select_overload(&methods, "value", false).unwrap();
        assert_eq!(lvalue.qualifier, RefQualifier::LValue);
        assert_eq!(lvalue.return_type, "const T&");

        let rvalue = select_overload(&methods, "value", true).unwrap();
        assert_eq!(rvalue.qualifier, RefQualifier::RValue);
        assert_eq!(rvalue.return_type, "T&&");
    }

    #[test]
    fn e2_8_ref_qualifier_fallback_to_none() {
        let methods = vec![RefQualifiedMethod {
            name: "data".to_string(),
            qualifier: RefQualifier::None,
            return_type: "T".to_string(),
            is_const: false,
        }];

        // Both lvalue and rvalue resolve to the unqualified overload.
        let lv = select_overload(&methods, "data", false).unwrap();
        assert_eq!(lv.qualifier, RefQualifier::None);

        let rv = select_overload(&methods, "data", true).unwrap();
        assert_eq!(rv.qualifier, RefQualifier::None);
    }

    #[test]
    fn e2_8_overload_not_found() {
        let methods: Vec<RefQualifiedMethod> = vec![];
        assert!(select_overload(&methods, "missing", false).is_none());
    }

    // --- E2.9: Exception Safety ---

    #[test]
    fn e2_9_cpp_try_success() {
        let result: Result<i32, CppException> = cpp_try(|| Ok(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn e2_9_cpp_try_exception() {
        let result: Result<i32, CppException> =
            cpp_try(|| Err(CppException::runtime_error("segfault")));
        let err = result.unwrap_err();
        assert_eq!(err.type_name, "std::runtime_error");
        assert_eq!(err.message, "segfault");
        assert_eq!(exception_to_error_code(&err), "RE001");
    }

    #[test]
    fn e2_9_exception_display_with_backtrace() {
        let exc = CppException::with_backtrace(
            "std::out_of_range",
            "index 5 out of bounds",
            vec!["vector::at(size_t)".to_string(), "main()".to_string()],
        );
        let display = format!("{exc}");
        assert!(display.contains("std::out_of_range"));
        assert!(display.contains("index 5 out of bounds"));
        assert!(display.contains("vector::at(size_t)"));
        assert_eq!(exception_to_error_code(&exc), "RE003");
    }

    #[test]
    fn e2_9_exception_factory_methods() {
        let bad_alloc = CppException::bad_alloc();
        assert_eq!(bad_alloc.type_name, "std::bad_alloc");
        assert_eq!(exception_to_error_code(&bad_alloc), "ME001");

        let logic = CppException::logic_error("precondition violated");
        assert_eq!(logic.type_name, "std::logic_error");
        assert_eq!(exception_to_error_code(&logic), "RE002");
    }

    // --- E2.4 (continued): SharedPtr drop callback ---

    #[test]
    fn e2_4_shared_ptr_drop_callback() {
        let destroyed = Arc::new(AtomicBool::new(false));
        let destroyed_clone = destroyed.clone();

        let sp1 = SharedPtr::new(String::from("resource"), "std::string");
        sp1.on_drop(Box::new(move |_type_name| {
            destroyed_clone.store(true, AtomicOrdering::SeqCst);
        }));

        let sp2 = sp1.clone();
        drop(sp1);
        // Still alive — sp2 holds a reference.
        assert!(!destroyed.load(AtomicOrdering::SeqCst));

        drop(sp2);
        // Now destroyed — last strong ref dropped.
        assert!(destroyed.load(AtomicOrdering::SeqCst));
    }
}
