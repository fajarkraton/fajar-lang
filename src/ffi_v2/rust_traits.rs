//! Rust Trait Object Marshalling — Sprint E6.
//!
//! Extends `rust_bridge.rs` with trait-level interop: `dyn Trait` across FFI,
//! generic function bridging, lifetime mapping, error/iterator/closure/async bridges.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Common FFI value type
// ═══════════════════════════════════════════════════════════════════════

/// A value that can cross the Fajar↔Rust FFI boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum FfiValue {
    /// Null / unit.
    Null,
    /// Signed 64-bit integer.
    Int(i64),
    /// 64-bit float.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// UTF-8 string.
    Str(String),
    /// Homogeneous array.
    Array(Vec<FfiValue>),
    /// Struct/record with named fields.
    Record(HashMap<String, FfiValue>),
    /// Opaque handle (simulated pointer).
    Handle(u64),
}

impl fmt::Display for FfiValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Str(v) => write!(f, "\"{v}\""),
            Self::Array(vs) => {
                write!(f, "[")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Self::Record(fields) => {
                write!(f, "{{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Self::Handle(h) => write!(f, "handle(0x{h:x})"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FFI Error type
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during FFI trait marshalling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiTraitError {
    /// Method not found in vtable.
    MethodNotFound(String),
    /// Type mismatch during conversion.
    TypeMismatch { expected: String, got: String },
    /// Lifetime violation (borrow expired).
    LifetimeViolation { scope: String, message: String },
    /// Iterator exhausted.
    IteratorExhausted,
    /// Monomorphization not found for given type arguments.
    MonomorphNotFound {
        base_fn: String,
        type_args: Vec<String>,
    },
    /// Future already completed.
    FutureAlreadyCompleted,
    /// Generic FFI error.
    Other(String),
}

impl fmt::Display for FfiTraitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodNotFound(name) => write!(f, "method not found: {name}"),
            Self::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            Self::LifetimeViolation { scope, message } => {
                write!(f, "lifetime violation in scope '{scope}': {message}")
            }
            Self::IteratorExhausted => write!(f, "iterator exhausted"),
            Self::MonomorphNotFound { base_fn, type_args } => {
                write!(
                    f,
                    "no monomorphization for {base_fn}<{}>",
                    type_args.join(", ")
                )
            }
            Self::FutureAlreadyCompleted => write!(f, "future already completed"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.1: Rust trait → Fajar trait
// ═══════════════════════════════════════════════════════════════════════

/// A method signature within a Rust trait definition.
#[derive(Debug, Clone)]
pub struct TraitMethodDef {
    /// Method name.
    pub name: String,
    /// Parameter names and type names (excluding `self`).
    pub params: Vec<(String, String)>,
    /// Return type name.
    pub return_type: String,
    /// Whether the method takes `&self`.
    pub has_self: bool,
    /// Whether the method takes `&mut self`.
    pub has_mut_self: bool,
    /// Whether the method is async.
    pub is_async: bool,
    /// Whether the method has a default implementation.
    pub has_default: bool,
}

/// A Rust trait definition to be marshalled into Fajar Lang.
#[derive(Debug, Clone)]
pub struct RustTraitDef {
    /// Trait name (e.g., `Iterator`, `Display`).
    pub name: String,
    /// Methods defined in this trait.
    pub methods: Vec<TraitMethodDef>,
    /// Super-trait names (e.g., `["Clone", "Debug"]`).
    pub super_traits: Vec<String>,
    /// Type parameters (e.g., `["T", "U"]`).
    pub type_params: Vec<String>,
    /// Associated type names.
    pub associated_types: Vec<String>,
}

impl RustTraitDef {
    /// Creates a new empty trait definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            methods: Vec::new(),
            super_traits: Vec::new(),
            type_params: Vec::new(),
            associated_types: Vec::new(),
        }
    }

    /// Adds a method to the trait.
    pub fn add_method(&mut self, method: TraitMethodDef) {
        self.methods.push(method);
    }

    /// Adds a super-trait requirement.
    pub fn add_super_trait(&mut self, name: impl Into<String>) {
        self.super_traits.push(name.into());
    }

    /// Generates Fajar Lang trait definition code.
    pub fn to_fajar_code(&self) -> String {
        let mut code = String::new();

        // trait header with super-traits
        code.push_str(&format!("trait {}", self.name));
        if !self.type_params.is_empty() {
            code.push('<');
            code.push_str(&self.type_params.join(", "));
            code.push('>');
        }
        if !self.super_traits.is_empty() {
            code.push_str(": ");
            code.push_str(&self.super_traits.join(" + "));
        }
        code.push_str(" {\n");

        // associated types
        for assoc in &self.associated_types {
            code.push_str(&format!("    type {assoc}\n"));
        }

        // methods
        for method in &self.methods {
            let async_kw = if method.is_async { "async " } else { "" };
            let self_param = if method.has_mut_self {
                "mut self"
            } else if method.has_self {
                "self"
            } else {
                ""
            };
            let params: Vec<String> = method
                .params
                .iter()
                .map(|(n, t)| format!("{n}: {t}"))
                .collect();
            let all_params = if self_param.is_empty() {
                params.join(", ")
            } else if params.is_empty() {
                self_param.to_string()
            } else {
                format!("{self_param}, {}", params.join(", "))
            };
            code.push_str(&format!(
                "    {async_kw}fn {}({all_params}) -> {}\n",
                method.name, method.return_type
            ));
        }

        code.push_str("}\n");
        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.2: Fajar impl for Rust struct
// ═══════════════════════════════════════════════════════════════════════

/// A function body placeholder for cross-boundary implementations.
/// In simulation, we store a callable name that the Fajar interpreter resolves.
#[derive(Debug, Clone)]
pub struct FajarFnRef {
    /// Fully-qualified Fajar function name.
    pub fn_name: String,
    /// Expected parameter types.
    pub param_types: Vec<String>,
    /// Expected return type.
    pub return_type: String,
}

/// An implementation of a Rust trait by a Fajar struct.
#[derive(Debug, Clone)]
pub struct CrossBoundaryImpl {
    /// The Rust struct name being extended.
    pub struct_name: String,
    /// The trait being implemented.
    pub trait_name: String,
    /// Method implementations: method name → Fajar function reference.
    pub method_impls: HashMap<String, FajarFnRef>,
}

impl CrossBoundaryImpl {
    /// Creates a new cross-boundary impl.
    pub fn new(struct_name: impl Into<String>, trait_name: impl Into<String>) -> Self {
        Self {
            struct_name: struct_name.into(),
            trait_name: trait_name.into(),
            method_impls: HashMap::new(),
        }
    }

    /// Registers a Fajar function as the implementation for a trait method.
    pub fn register_method(&mut self, method_name: impl Into<String>, fn_ref: FajarFnRef) {
        self.method_impls.insert(method_name.into(), fn_ref);
    }

    /// Checks whether all required methods of the trait are implemented.
    pub fn validate(&self, trait_def: &RustTraitDef) -> Result<(), Vec<String>> {
        let missing: Vec<String> = trait_def
            .methods
            .iter()
            .filter(|m| !m.has_default && !self.method_impls.contains_key(&m.name))
            .map(|m| m.name.clone())
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.3: dyn Trait across FFI — VTable + DynTraitObject
// ═══════════════════════════════════════════════════════════════════════

/// A function pointer entry in a vtable.
pub type VTableFn = fn(&[FfiValue]) -> Result<FfiValue, FfiTraitError>;

/// A virtual dispatch table for dyn Trait objects.
#[derive(Clone)]
pub struct VTable {
    /// Trait name this vtable implements.
    pub trait_name: String,
    /// Method name → function pointer.
    pub methods: HashMap<String, VTableFn>,
}

impl fmt::Debug for VTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VTable")
            .field("trait_name", &self.trait_name)
            .field("methods", &self.methods.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl VTable {
    /// Creates a new vtable for the given trait.
    pub fn new(trait_name: impl Into<String>) -> Self {
        Self {
            trait_name: trait_name.into(),
            methods: HashMap::new(),
        }
    }

    /// Adds a method entry to the vtable.
    pub fn add_method(&mut self, name: impl Into<String>, func: VTableFn) {
        self.methods.insert(name.into(), func);
    }

    /// Looks up a method by name.
    pub fn lookup(&self, name: &str) -> Option<&VTableFn> {
        self.methods.get(name)
    }
}

/// A `dyn Trait` object that can cross the FFI boundary.
///
/// Contains a vtable for dynamic dispatch and an opaque data pointer
/// (simulated as an `FfiValue`).
#[derive(Debug, Clone)]
pub struct DynTraitObject {
    /// The vtable for method dispatch.
    pub vtable: VTable,
    /// The data payload (simulated — in production this would be a raw pointer).
    pub data: FfiValue,
}

impl DynTraitObject {
    /// Creates a new dyn trait object.
    pub fn new(vtable: VTable, data: FfiValue) -> Self {
        Self { vtable, data }
    }

    /// Calls a method on this dyn trait object.
    ///
    /// Prepends `self.data` to the argument list before dispatch.
    pub fn call_method(
        &self,
        method_name: &str,
        args: &[FfiValue],
    ) -> Result<FfiValue, FfiTraitError> {
        let func = self
            .vtable
            .lookup(method_name)
            .ok_or_else(|| FfiTraitError::MethodNotFound(method_name.to_string()))?;
        let mut full_args = vec![self.data.clone()];
        full_args.extend_from_slice(args);
        func(&full_args)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.4: Generic function bridge
// ═══════════════════════════════════════════════════════════════════════

/// A monomorphized function entry.
#[derive(Debug, Clone)]
pub struct MonomorphEntry {
    /// Concrete type arguments (e.g., `["i32", "str"]`).
    pub type_args: Vec<String>,
    /// The monomorphized function name/symbol.
    pub symbol: String,
    /// The function pointer (simulated).
    pub func: VTableFn,
}

/// Bridges a generic Rust function to Fajar by storing monomorphized variants.
#[derive(Debug, Clone)]
pub struct GenericFnBridge {
    /// Base generic function name (e.g., `std::convert::Into::into`).
    pub base_name: String,
    /// Type parameter names.
    pub type_params: Vec<String>,
    /// Monomorphization map: serialized type args → entry.
    pub monomorphizations: HashMap<String, MonomorphEntry>,
}

impl GenericFnBridge {
    /// Creates a new generic function bridge.
    pub fn new(base_name: impl Into<String>, type_params: Vec<String>) -> Self {
        Self {
            base_name: base_name.into(),
            type_params,
            monomorphizations: HashMap::new(),
        }
    }

    /// Registers a monomorphized variant.
    pub fn register(&mut self, type_args: Vec<String>, symbol: impl Into<String>, func: VTableFn) {
        let key = type_args.join(",");
        self.monomorphizations.insert(
            key,
            MonomorphEntry {
                type_args,
                symbol: symbol.into(),
                func,
            },
        );
    }

    /// Calls the appropriate monomorphized variant.
    pub fn call(&self, type_args: &[String], args: &[FfiValue]) -> Result<FfiValue, FfiTraitError> {
        let key = type_args.join(",");
        let entry =
            self.monomorphizations
                .get(&key)
                .ok_or_else(|| FfiTraitError::MonomorphNotFound {
                    base_fn: self.base_name.clone(),
                    type_args: type_args.to_vec(),
                })?;
        (entry.func)(args)
    }

    /// Returns the number of registered monomorphizations.
    pub fn variant_count(&self) -> usize {
        self.monomorphizations.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.5: Lifetime handling
// ═══════════════════════════════════════════════════════════════════════

/// Tracks the state of a borrow for lifetime enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowState {
    /// Shared (immutable) borrow active.
    Shared(u32),
    /// Exclusive (mutable) borrow active.
    Exclusive,
    /// No active borrow.
    Free,
}

/// A tracked borrow within a lifetime scope.
#[derive(Debug, Clone)]
pub struct TrackedBorrow {
    /// Variable or handle name.
    pub name: String,
    /// Type of the borrowed value.
    pub value_type: String,
    /// Current borrow state.
    pub state: BorrowState,
}

/// Maps Rust lifetimes to Fajar borrow scopes for FFI safety.
///
/// Ensures that references passed across the FFI boundary remain valid
/// for the duration the Fajar side holds them.
#[derive(Debug, Clone)]
pub struct LifetimeScope {
    /// Scope name (e.g., `'a`, `'static`, `call_42`).
    pub name: String,
    /// Whether this scope is still active.
    pub active: bool,
    /// Tracked borrows within this scope.
    pub borrows: Vec<TrackedBorrow>,
    /// Child scopes (nested lifetimes).
    pub children: Vec<LifetimeScope>,
}

impl LifetimeScope {
    /// Creates a new active lifetime scope.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            active: true,
            borrows: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Adds a shared borrow to this scope.
    pub fn add_shared_borrow(
        &mut self,
        name: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Result<(), FfiTraitError> {
        let name = name.into();
        if !self.active {
            return Err(FfiTraitError::LifetimeViolation {
                scope: self.name.clone(),
                message: format!("scope expired, cannot borrow '{name}'"),
            });
        }
        // Check no exclusive borrow exists for this name
        for b in &self.borrows {
            if b.name == name && b.state == BorrowState::Exclusive {
                return Err(FfiTraitError::LifetimeViolation {
                    scope: self.name.clone(),
                    message: format!("'{name}' already exclusively borrowed"),
                });
            }
        }
        // Increment shared count or add new entry
        if let Some(b) = self.borrows.iter_mut().find(|b| b.name == name) {
            if let BorrowState::Shared(ref mut count) = b.state {
                *count += 1;
            }
        } else {
            self.borrows.push(TrackedBorrow {
                name,
                value_type: value_type.into(),
                state: BorrowState::Shared(1),
            });
        }
        Ok(())
    }

    /// Adds an exclusive (mutable) borrow to this scope.
    pub fn add_exclusive_borrow(
        &mut self,
        name: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Result<(), FfiTraitError> {
        let name = name.into();
        if !self.active {
            return Err(FfiTraitError::LifetimeViolation {
                scope: self.name.clone(),
                message: format!("scope expired, cannot borrow '{name}'"),
            });
        }
        // Check no existing borrow for this name
        for b in &self.borrows {
            if b.name == name && b.state != BorrowState::Free {
                return Err(FfiTraitError::LifetimeViolation {
                    scope: self.name.clone(),
                    message: format!("'{name}' already borrowed"),
                });
            }
        }
        self.borrows.push(TrackedBorrow {
            name,
            value_type: value_type.into(),
            state: BorrowState::Exclusive,
        });
        Ok(())
    }

    /// Releases all borrows and marks the scope as inactive.
    pub fn end_scope(&mut self) {
        self.active = false;
        for b in &mut self.borrows {
            b.state = BorrowState::Free;
        }
        for child in &mut self.children {
            child.end_scope();
        }
    }

    /// Creates a child lifetime scope.
    pub fn child_scope(&mut self, name: impl Into<String>) -> &mut LifetimeScope {
        self.children.push(LifetimeScope::new(name));
        // SAFETY: we just pushed, so last() is guaranteed to exist.
        self.children.last_mut().expect("just pushed child scope")
    }

    /// Returns the number of active borrows.
    pub fn active_borrow_count(&self) -> usize {
        self.borrows
            .iter()
            .filter(|b| b.state != BorrowState::Free)
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.6: Error type bridge — RustResult<T,E> ↔ Fajar Result<T,E>
// ═══════════════════════════════════════════════════════════════════════

/// A Result type that can cross the FFI boundary.
///
/// Maps Rust `Result<T, E>` to Fajar `Result<T, E>` using `FfiValue`
/// for both the success and error payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum FfiResult {
    /// Success value.
    Ok(FfiValue),
    /// Error value.
    Err(FfiValue),
}

impl FfiResult {
    /// Creates an Ok variant.
    pub fn ok(value: FfiValue) -> Self {
        Self::Ok(value)
    }

    /// Creates an Err variant.
    pub fn err(value: FfiValue) -> Self {
        Self::Err(value)
    }

    /// Returns true if this is Ok.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    /// Returns true if this is Err.
    pub fn is_err(&self) -> bool {
        matches!(self, Self::Err(_))
    }

    /// Converts to a standard Rust Result.
    pub fn into_result(self) -> Result<FfiValue, FfiValue> {
        match self {
            Self::Ok(v) => Ok(v),
            Self::Err(e) => Err(e),
        }
    }

    /// Creates from a standard Rust Result.
    pub fn from_result(result: Result<FfiValue, FfiValue>) -> Self {
        match result {
            Ok(v) => Self::Ok(v),
            Err(e) => Self::Err(e),
        }
    }

    /// Converts to Fajar `Result<T,E>` enum representation.
    pub fn to_fajar_enum(&self) -> FfiValue {
        match self {
            Self::Ok(v) => {
                let mut fields = HashMap::new();
                fields.insert("variant".to_string(), FfiValue::Str("Ok".to_string()));
                fields.insert("data".to_string(), v.clone());
                FfiValue::Record(fields)
            }
            Self::Err(e) => {
                let mut fields = HashMap::new();
                fields.insert("variant".to_string(), FfiValue::Str("Err".to_string()));
                fields.insert("data".to_string(), e.clone());
                FfiValue::Record(fields)
            }
        }
    }

    /// Parses from a Fajar `Result<T,E>` enum representation.
    pub fn from_fajar_enum(value: &FfiValue) -> Result<Self, FfiTraitError> {
        if let FfiValue::Record(fields) = value {
            let variant = fields
                .get("variant")
                .ok_or_else(|| FfiTraitError::TypeMismatch {
                    expected: "Record with 'variant' field".to_string(),
                    got: "Record without 'variant'".to_string(),
                })?;
            let data = fields.get("data").cloned().unwrap_or(FfiValue::Null);
            match variant {
                FfiValue::Str(s) if s == "Ok" => Ok(FfiResult::Ok(data)),
                FfiValue::Str(s) if s == "Err" => Ok(FfiResult::Err(data)),
                other => Err(FfiTraitError::TypeMismatch {
                    expected: "\"Ok\" or \"Err\"".to_string(),
                    got: format!("{other:?}"),
                }),
            }
        } else {
            Err(FfiTraitError::TypeMismatch {
                expected: "Record (Fajar enum)".to_string(),
                got: format!("{value:?}"),
            })
        }
    }
}

/// Mapping table for Rust error types to Fajar error types.
#[derive(Debug, Clone)]
pub struct ErrorTypeBridge {
    /// Rust error type name → Fajar error type name.
    pub mappings: HashMap<String, String>,
}

impl ErrorTypeBridge {
    /// Creates a bridge with standard error type mappings.
    pub fn with_defaults() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert("std::io::Error".to_string(), "IoError".to_string());
        mappings.insert("std::fmt::Error".to_string(), "FmtError".to_string());
        mappings.insert(
            "std::num::ParseIntError".to_string(),
            "ParseError".to_string(),
        );
        mappings.insert(
            "std::num::ParseFloatError".to_string(),
            "ParseError".to_string(),
        );
        mappings.insert("anyhow::Error".to_string(), "str".to_string());
        mappings.insert("serde_json::Error".to_string(), "JsonError".to_string());
        Self { mappings }
    }

    /// Looks up the Fajar error type for a Rust error type.
    pub fn resolve(&self, rust_error: &str) -> Option<&str> {
        self.mappings.get(rust_error).map(|s| s.as_str())
    }

    /// Registers a custom error type mapping.
    pub fn add_mapping(&mut self, rust_type: impl Into<String>, fajar_type: impl Into<String>) {
        self.mappings.insert(rust_type.into(), fajar_type.into());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.7: Iterator bridge
// ═══════════════════════════════════════════════════════════════════════

/// The state of a `RustIterator`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IteratorState {
    /// Iterator is ready to produce the next element.
    Ready,
    /// Iterator has been exhausted.
    Exhausted,
}

/// Bridges a Rust iterator to Fajar's `for-in` loop.
///
/// Wraps an iterator as a state machine with `next()` returning `Option<FfiValue>`.
/// The items are pre-collected (simulated — production would use lazy evaluation).
#[derive(Debug, Clone)]
pub struct RustIterator {
    /// Name/type of the underlying iterator.
    pub name: String,
    /// Item type name.
    pub item_type: String,
    /// Pre-collected items (simulated lazy source).
    items: Vec<FfiValue>,
    /// Current position.
    position: usize,
    /// Current state.
    pub state: IteratorState,
}

impl RustIterator {
    /// Creates a new iterator from a vector of items.
    pub fn new(
        name: impl Into<String>,
        item_type: impl Into<String>,
        items: Vec<FfiValue>,
    ) -> Self {
        Self {
            name: name.into(),
            item_type: item_type.into(),
            items,
            position: 0,
            state: IteratorState::Ready,
        }
    }

    /// Returns the next item, or `None` if exhausted.
    pub fn next_item(&mut self) -> Option<FfiValue> {
        if self.position < self.items.len() {
            let item = self.items[self.position].clone();
            self.position += 1;
            if self.position >= self.items.len() {
                self.state = IteratorState::Exhausted;
            }
            Some(item)
        } else {
            self.state = IteratorState::Exhausted;
            None
        }
    }

    /// Resets the iterator to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
        self.state = IteratorState::Ready;
    }

    /// Returns the number of remaining items.
    pub fn remaining(&self) -> usize {
        self.items.len().saturating_sub(self.position)
    }

    /// Collects all remaining items into a Vec.
    pub fn collect_remaining(&mut self) -> Vec<FfiValue> {
        let mut result = Vec::new();
        while let Some(item) = self.next_item() {
            result.push(item);
        }
        result
    }

    /// Maps a function over the remaining items, producing a new iterator.
    pub fn map_values(&mut self, func: impl Fn(&FfiValue) -> FfiValue) -> RustIterator {
        let mapped: Vec<FfiValue> = self
            .items
            .iter()
            .skip(self.position)
            .map(func)
            .collect();
        RustIterator::new(format!("{}.map", self.name), self.item_type.clone(), mapped)
    }

    /// Filters remaining items, producing a new iterator.
    pub fn filter_values(&mut self, predicate: impl Fn(&FfiValue) -> bool) -> RustIterator {
        let filtered: Vec<FfiValue> = self
            .items
            .iter()
            .skip(self.position)
            .filter(|item| predicate(item))
            .cloned()
            .collect();
        RustIterator::new(
            format!("{}.filter", self.name),
            self.item_type.clone(),
            filtered,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.8: Closure bridge
// ═══════════════════════════════════════════════════════════════════════

/// Function type for closures: (captures, args) -> Result.
pub type ClosureFn = fn(&HashMap<String, FfiValue>, &[FfiValue]) -> Result<FfiValue, FfiTraitError>;

/// A Fajar closure that can be passed to Rust as a `FnMut` equivalent.
///
/// Contains captured values and a callable function reference.
#[derive(Clone)]
pub struct FajarClosure {
    /// Name of the closure (for debugging).
    pub name: String,
    /// Captured environment values.
    pub captures: HashMap<String, FfiValue>,
    /// Parameter type names.
    pub param_types: Vec<String>,
    /// Return type name.
    pub return_type: String,
    /// The callable function.
    func: ClosureFn,
}

impl fmt::Debug for FajarClosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FajarClosure")
            .field("name", &self.name)
            .field("captures", &self.captures.keys().collect::<Vec<_>>())
            .field("param_types", &self.param_types)
            .field("return_type", &self.return_type)
            .finish()
    }
}

impl FajarClosure {
    /// Creates a new closure.
    pub fn new(
        name: impl Into<String>,
        captures: HashMap<String, FfiValue>,
        param_types: Vec<String>,
        return_type: impl Into<String>,
        func: ClosureFn,
    ) -> Self {
        Self {
            name: name.into(),
            captures,
            param_types,
            return_type: return_type.into(),
            func,
        }
    }

    /// Calls the closure with the given arguments.
    pub fn call(&self, args: &[FfiValue]) -> Result<FfiValue, FfiTraitError> {
        if args.len() != self.param_types.len() {
            return Err(FfiTraitError::TypeMismatch {
                expected: format!("{} arguments", self.param_types.len()),
                got: format!("{} arguments", args.len()),
            });
        }
        (self.func)(&self.captures, args)
    }

    /// Calls the closure mutably, allowing captures to be updated.
    pub fn call_mut(&mut self, args: &[FfiValue]) -> Result<FfiValue, FfiTraitError> {
        if args.len() != self.param_types.len() {
            return Err(FfiTraitError::TypeMismatch {
                expected: format!("{} arguments", self.param_types.len()),
                got: format!("{} arguments", args.len()),
            });
        }
        let result = (self.func)(&self.captures, args)?;
        // Convention: if the closure returns a Record with a "__captures" key,
        // update the captures.
        if let FfiValue::Record(ref fields) = result {
            if let Some(FfiValue::Record(new_caps)) = fields.get("__captures") {
                for (k, v) in new_caps {
                    self.captures.insert(k.clone(), v.clone());
                }
                // Return the actual value (without __captures metadata)
                if let Some(val) = fields.get("__value") {
                    return Ok(val.clone());
                }
            }
        }
        Ok(result)
    }

    /// Returns the number of captured variables.
    pub fn capture_count(&self) -> usize {
        self.captures.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.9: Async bridge — RustFuture
// ═══════════════════════════════════════════════════════════════════════

/// Poll result for a `RustFuture`.
#[derive(Debug, Clone, PartialEq)]
pub enum FfiPoll {
    /// The future is ready with a value.
    Ready(FfiValue),
    /// The future is not yet ready.
    Pending,
}

/// The state of a `RustFuture`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FutureState {
    /// Not yet started.
    Created,
    /// Currently executing / waiting.
    Pending,
    /// Completed successfully.
    Ready,
    /// Completed with error.
    Failed,
}

/// Wraps a Rust `Future` as a Fajar async/await compatible object.
///
/// Uses a poll-based model: call `poll()` to advance the future. Once
/// it returns `Ready`, the value can be retrieved with `take_value()`.
#[derive(Debug, Clone)]
pub struct RustFuture {
    /// Name of the future (for debugging).
    pub name: String,
    /// Return type name.
    pub return_type: String,
    /// Current state.
    pub state: FutureState,
    /// The resolved value (set when Ready).
    value: Option<FfiValue>,
    /// The error (set when Failed).
    error: Option<FfiTraitError>,
    /// Simulated poll count (for testing progress).
    poll_count: u32,
    /// Number of polls before becoming ready (simulated latency).
    ready_after: u32,
    /// The value to resolve with (simulated).
    resolve_value: Option<FfiValue>,
}

impl RustFuture {
    /// Creates a new future that becomes ready after `ready_after` polls.
    pub fn new(
        name: impl Into<String>,
        return_type: impl Into<String>,
        ready_after: u32,
        resolve_value: FfiValue,
    ) -> Self {
        Self {
            name: name.into(),
            return_type: return_type.into(),
            state: FutureState::Created,
            value: None,
            error: None,
            poll_count: 0,
            ready_after,
            resolve_value: Some(resolve_value),
        }
    }

    /// Creates a future that immediately fails.
    pub fn failed(
        name: impl Into<String>,
        return_type: impl Into<String>,
        error: FfiTraitError,
    ) -> Self {
        Self {
            name: name.into(),
            return_type: return_type.into(),
            state: FutureState::Failed,
            value: None,
            error: Some(error),
            poll_count: 0,
            ready_after: 0,
            resolve_value: None,
        }
    }

    /// Polls the future, advancing its state.
    pub fn poll(&mut self) -> Result<FfiPoll, FfiTraitError> {
        match self.state {
            FutureState::Ready => Err(FfiTraitError::FutureAlreadyCompleted),
            FutureState::Failed => Err(self
                .error
                .clone()
                .unwrap_or(FfiTraitError::Other("unknown error".to_string()))),
            FutureState::Created | FutureState::Pending => {
                self.state = FutureState::Pending;
                self.poll_count += 1;
                if self.poll_count >= self.ready_after {
                    if let Some(val) = self.resolve_value.take() {
                        self.value = Some(val.clone());
                        self.state = FutureState::Ready;
                        Ok(FfiPoll::Ready(val))
                    } else {
                        self.state = FutureState::Failed;
                        Err(FfiTraitError::Other("no resolve value".to_string()))
                    }
                } else {
                    Ok(FfiPoll::Pending)
                }
            }
        }
    }

    /// Takes the resolved value (consumes it).
    pub fn take_value(&mut self) -> Option<FfiValue> {
        self.value.take()
    }

    /// Returns the number of times this future has been polled.
    pub fn poll_count(&self) -> u32 {
        self.poll_count
    }

    /// Returns true if the future is complete (ready or failed).
    pub fn is_complete(&self) -> bool {
        matches!(self.state, FutureState::Ready | FutureState::Failed)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Registry — Manages all trait bridges
// ═══════════════════════════════════════════════════════════════════════

/// Central registry for all trait-level FFI bridges.
#[derive(Debug, Clone)]
pub struct TraitBridgeRegistry {
    /// Registered trait definitions.
    pub traits: HashMap<String, RustTraitDef>,
    /// Cross-boundary implementations: `(struct, trait)` → impl.
    pub impls: HashMap<(String, String), CrossBoundaryImpl>,
    /// Generic function bridges.
    pub generic_fns: HashMap<String, GenericFnBridge>,
    /// Error type bridge.
    pub error_bridge: ErrorTypeBridge,
}

impl TraitBridgeRegistry {
    /// Creates a new empty registry with default error mappings.
    pub fn new() -> Self {
        Self {
            traits: HashMap::new(),
            impls: HashMap::new(),
            generic_fns: HashMap::new(),
            error_bridge: ErrorTypeBridge::with_defaults(),
        }
    }

    /// Registers a trait definition.
    pub fn register_trait(&mut self, def: RustTraitDef) {
        self.traits.insert(def.name.clone(), def);
    }

    /// Registers a cross-boundary implementation.
    pub fn register_impl(&mut self, impl_: CrossBoundaryImpl) {
        let key = (impl_.struct_name.clone(), impl_.trait_name.clone());
        self.impls.insert(key, impl_);
    }

    /// Registers a generic function bridge.
    pub fn register_generic_fn(&mut self, bridge: GenericFnBridge) {
        self.generic_fns.insert(bridge.base_name.clone(), bridge);
    }

    /// Looks up a trait definition by name.
    pub fn get_trait(&self, name: &str) -> Option<&RustTraitDef> {
        self.traits.get(name)
    }

    /// Looks up an implementation for a (struct, trait) pair.
    pub fn get_impl(&self, struct_name: &str, trait_name: &str) -> Option<&CrossBoundaryImpl> {
        self.impls
            .get(&(struct_name.to_string(), trait_name.to_string()))
    }
}

impl Default for TraitBridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E6.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── E6.1: Rust trait → Fajar trait ──

    #[test]
    fn e6_1_rust_trait_def_to_fajar_code() {
        let mut def = RustTraitDef::new("Display");
        def.add_method(TraitMethodDef {
            name: "fmt".to_string(),
            params: vec![("f".to_string(), "Formatter".to_string())],
            return_type: "str".to_string(),
            has_self: true,
            has_mut_self: false,
            is_async: false,
            has_default: false,
        });
        let code = def.to_fajar_code();
        assert!(code.contains("trait Display"));
        assert!(code.contains("fn fmt(self, f: Formatter) -> str"));
    }

    #[test]
    fn e6_1_trait_with_super_traits_and_generics() {
        let mut def = RustTraitDef::new("Serialize");
        def.type_params.push("T".to_string());
        def.add_super_trait("Clone");
        def.add_super_trait("Debug");
        def.associated_types.push("Output".to_string());
        def.add_method(TraitMethodDef {
            name: "serialize".to_string(),
            params: vec![],
            return_type: "Output".to_string(),
            has_self: true,
            has_mut_self: false,
            is_async: false,
            has_default: false,
        });
        let code = def.to_fajar_code();
        assert!(code.contains("trait Serialize<T>: Clone + Debug"));
        assert!(code.contains("type Output"));
        assert!(code.contains("fn serialize(self) -> Output"));
    }

    // ── E6.2: CrossBoundaryImpl ──

    #[test]
    fn e6_2_cross_boundary_impl_validates() {
        let mut def = RustTraitDef::new("Display");
        def.add_method(TraitMethodDef {
            name: "fmt".to_string(),
            params: vec![],
            return_type: "str".to_string(),
            has_self: true,
            has_mut_self: false,
            is_async: false,
            has_default: false,
        });
        def.add_method(TraitMethodDef {
            name: "debug".to_string(),
            params: vec![],
            return_type: "str".to_string(),
            has_self: true,
            has_mut_self: false,
            is_async: false,
            has_default: true, // has default impl
        });

        let mut impl_ = CrossBoundaryImpl::new("MyStruct", "Display");
        // Only implement required method
        impl_.register_method(
            "fmt",
            FajarFnRef {
                fn_name: "my_struct_fmt".to_string(),
                param_types: vec![],
                return_type: "str".to_string(),
            },
        );
        assert!(impl_.validate(&def).is_ok());

        // Remove the required method — validation should fail
        let bad_impl = CrossBoundaryImpl::new("MyStruct", "Display");
        let err = bad_impl.validate(&def).unwrap_err();
        assert_eq!(err, vec!["fmt".to_string()]);
    }

    // ── E6.3: dyn Trait across FFI ──

    #[test]
    fn e6_3_dyn_trait_object_dispatch() {
        fn mock_to_string(args: &[FfiValue]) -> Result<FfiValue, FfiTraitError> {
            if let Some(FfiValue::Int(n)) = args.first() {
                Ok(FfiValue::Str(format!("{n}")))
            } else {
                Err(FfiTraitError::TypeMismatch {
                    expected: "Int".to_string(),
                    got: "other".to_string(),
                })
            }
        }

        let mut vtable = VTable::new("ToString");
        vtable.add_method("to_string", mock_to_string as VTableFn);

        let obj = DynTraitObject::new(vtable, FfiValue::Int(42));
        let result = obj.call_method("to_string", &[]).unwrap();
        assert_eq!(result, FfiValue::Str("42".to_string()));
    }

    #[test]
    fn e6_3_dyn_trait_method_not_found() {
        let vtable = VTable::new("Empty");
        let obj = DynTraitObject::new(vtable, FfiValue::Null);
        let err = obj.call_method("missing", &[]).unwrap_err();
        assert_eq!(err, FfiTraitError::MethodNotFound("missing".to_string()));
    }

    // ── E6.4: Generic function bridge ──

    #[test]
    fn e6_4_generic_fn_bridge_monomorphization() {
        fn identity_i32(args: &[FfiValue]) -> Result<FfiValue, FfiTraitError> {
            args.first()
                .cloned()
                .ok_or(FfiTraitError::Other("no args".to_string()))
        }
        fn identity_str(args: &[FfiValue]) -> Result<FfiValue, FfiTraitError> {
            args.first()
                .cloned()
                .ok_or(FfiTraitError::Other("no args".to_string()))
        }

        let mut bridge = GenericFnBridge::new("identity", vec!["T".to_string()]);
        bridge.register(
            vec!["i32".to_string()],
            "identity_i32",
            identity_i32 as VTableFn,
        );
        bridge.register(
            vec!["str".to_string()],
            "identity_str",
            identity_str as VTableFn,
        );

        assert_eq!(bridge.variant_count(), 2);

        let result = bridge
            .call(&["i32".to_string()], &[FfiValue::Int(99)])
            .unwrap();
        assert_eq!(result, FfiValue::Int(99));

        let err = bridge
            .call(&["f64".to_string()], &[FfiValue::Float(1.0)])
            .unwrap_err();
        assert!(matches!(err, FfiTraitError::MonomorphNotFound { .. }));
    }

    // ── E6.5: Lifetime handling ──

    #[test]
    fn e6_5_lifetime_scope_borrows() {
        let mut scope = LifetimeScope::new("'a");
        scope.add_shared_borrow("x", "i32").unwrap();
        scope.add_shared_borrow("x", "i32").unwrap(); // multiple shared OK
        assert_eq!(scope.active_borrow_count(), 1); // still one entry

        // Exclusive borrow should fail while shared exists
        let err = scope.add_exclusive_borrow("x", "i32").unwrap_err();
        assert!(matches!(err, FfiTraitError::LifetimeViolation { .. }));

        // Exclusive borrow on new name should succeed
        scope.add_exclusive_borrow("y", "str").unwrap();
        assert_eq!(scope.active_borrow_count(), 2);
    }

    #[test]
    fn e6_5_lifetime_scope_end() {
        let mut scope = LifetimeScope::new("'call");
        scope.add_shared_borrow("data", "Vec<u8>").unwrap();
        assert!(scope.active);
        scope.end_scope();
        assert!(!scope.active);
        assert_eq!(scope.active_borrow_count(), 0);

        // Borrowing after scope ends should fail
        let err = scope.add_shared_borrow("x", "i32").unwrap_err();
        assert!(matches!(err, FfiTraitError::LifetimeViolation { .. }));
    }

    // ── E6.6: Error type bridge ──

    #[test]
    fn e6_6_ffi_result_roundtrip() {
        let ok = FfiResult::ok(FfiValue::Int(42));
        assert!(ok.is_ok());
        let fajar_repr = ok.to_fajar_enum();
        let parsed = FfiResult::from_fajar_enum(&fajar_repr).unwrap();
        assert_eq!(parsed, FfiResult::Ok(FfiValue::Int(42)));

        let err = FfiResult::err(FfiValue::Str("not found".to_string()));
        assert!(err.is_err());
        let fajar_repr = err.to_fajar_enum();
        let parsed = FfiResult::from_fajar_enum(&fajar_repr).unwrap();
        assert_eq!(
            parsed,
            FfiResult::Err(FfiValue::Str("not found".to_string()))
        );
    }

    #[test]
    fn e6_6_error_type_bridge_resolve() {
        let bridge = ErrorTypeBridge::with_defaults();
        assert_eq!(bridge.resolve("std::io::Error"), Some("IoError"));
        assert_eq!(bridge.resolve("anyhow::Error"), Some("str"));
        assert_eq!(bridge.resolve("unknown::Error"), None);
    }

    // ── E6.7: Iterator bridge ──

    #[test]
    fn e6_7_rust_iterator_next() {
        let items = vec![FfiValue::Int(1), FfiValue::Int(2), FfiValue::Int(3)];
        let mut iter = RustIterator::new("range", "i32", items);

        assert_eq!(iter.remaining(), 3);
        assert_eq!(iter.next_item(), Some(FfiValue::Int(1)));
        assert_eq!(iter.next_item(), Some(FfiValue::Int(2)));
        assert_eq!(iter.remaining(), 1);
        assert_eq!(iter.next_item(), Some(FfiValue::Int(3)));
        assert_eq!(iter.next_item(), None);
        assert_eq!(iter.state, IteratorState::Exhausted);

        iter.reset();
        assert_eq!(iter.state, IteratorState::Ready);
        assert_eq!(iter.remaining(), 3);
    }

    #[test]
    fn e6_7_iterator_collect_and_map() {
        let items = vec![FfiValue::Int(10), FfiValue::Int(20)];
        let mut iter = RustIterator::new("nums", "i32", items);

        let doubled = iter.map_values(|v| match v {
            FfiValue::Int(n) => FfiValue::Int(n * 2),
            other => other.clone(),
        });
        let mut doubled_iter = doubled;
        assert_eq!(doubled_iter.next_item(), Some(FfiValue::Int(20)));
        assert_eq!(doubled_iter.next_item(), Some(FfiValue::Int(40)));
        assert_eq!(doubled_iter.next_item(), None);
    }

    // ── E6.8: Closure bridge ──

    #[test]
    fn e6_8_fajar_closure_call() {
        fn adder(
            captures: &HashMap<String, FfiValue>,
            args: &[FfiValue],
        ) -> Result<FfiValue, FfiTraitError> {
            let offset = match captures.get("offset") {
                Some(FfiValue::Int(n)) => *n,
                _ => 0,
            };
            match args.first() {
                Some(FfiValue::Int(n)) => Ok(FfiValue::Int(n + offset)),
                _ => Err(FfiTraitError::TypeMismatch {
                    expected: "Int".to_string(),
                    got: "other".to_string(),
                }),
            }
        }

        let mut captures = HashMap::new();
        captures.insert("offset".to_string(), FfiValue::Int(100));
        let closure = FajarClosure::new(
            "add_offset",
            captures,
            vec!["i32".to_string()],
            "i32",
            adder,
        );

        assert_eq!(closure.capture_count(), 1);
        let result = closure.call(&[FfiValue::Int(5)]).unwrap();
        assert_eq!(result, FfiValue::Int(105));

        // Wrong arity
        let err = closure.call(&[]).unwrap_err();
        assert!(matches!(err, FfiTraitError::TypeMismatch { .. }));
    }

    // ── E6.9: Async bridge ──

    #[test]
    fn e6_9_rust_future_poll_to_ready() {
        let mut future =
            RustFuture::new("fetch_data", "str", 3, FfiValue::Str("hello".to_string()));
        assert_eq!(future.state, FutureState::Created);

        // Polls 1 and 2 are Pending
        assert_eq!(future.poll().unwrap(), FfiPoll::Pending);
        assert_eq!(future.state, FutureState::Pending);
        assert_eq!(future.poll().unwrap(), FfiPoll::Pending);

        // Poll 3 is Ready
        let result = future.poll().unwrap();
        assert_eq!(result, FfiPoll::Ready(FfiValue::Str("hello".to_string())));
        assert_eq!(future.state, FutureState::Ready);
        assert_eq!(future.poll_count(), 3);

        // Polling again after Ready is an error
        let err = future.poll().unwrap_err();
        assert_eq!(err, FfiTraitError::FutureAlreadyCompleted);
    }

    #[test]
    fn e6_9_rust_future_failed() {
        let mut future =
            RustFuture::failed("bad_op", "void", FfiTraitError::Other("oops".to_string()));
        assert_eq!(future.state, FutureState::Failed);
        let err = future.poll().unwrap_err();
        assert_eq!(err, FfiTraitError::Other("oops".to_string()));
    }

    // ── Registry integration ──

    #[test]
    fn e6_10_registry_roundtrip() {
        let mut registry = TraitBridgeRegistry::new();

        // Register trait
        let mut def = RustTraitDef::new("Drawable");
        def.add_method(TraitMethodDef {
            name: "draw".to_string(),
            params: vec![("canvas".to_string(), "Canvas".to_string())],
            return_type: "void".to_string(),
            has_self: true,
            has_mut_self: false,
            is_async: false,
            has_default: false,
        });
        registry.register_trait(def);

        // Register impl
        let mut impl_ = CrossBoundaryImpl::new("Circle", "Drawable");
        impl_.register_method(
            "draw",
            FajarFnRef {
                fn_name: "circle_draw".to_string(),
                param_types: vec!["Canvas".to_string()],
                return_type: "void".to_string(),
            },
        );
        registry.register_impl(impl_);

        // Lookup
        let trait_def = registry.get_trait("Drawable").unwrap();
        assert_eq!(trait_def.methods.len(), 1);

        let impl_ = registry.get_impl("Circle", "Drawable").unwrap();
        assert!(impl_.method_impls.contains_key("draw"));

        // Validate via registry
        let validated = impl_.validate(trait_def);
        assert!(validated.is_ok());
    }

    #[test]
    fn e6_10_ffi_value_display() {
        assert_eq!(format!("{}", FfiValue::Null), "null");
        assert_eq!(format!("{}", FfiValue::Int(42)), "42");
        assert_eq!(format!("{}", FfiValue::Bool(true)), "true");
        assert_eq!(format!("{}", FfiValue::Str("hi".to_string())), "\"hi\"");
        assert_eq!(
            format!(
                "{}",
                FfiValue::Array(vec![FfiValue::Int(1), FfiValue::Int(2)])
            ),
            "[1, 2]"
        );
        assert_eq!(format!("{}", FfiValue::Handle(0xFF)), "handle(0xff)");
    }

    #[test]
    fn e6_10_lifetime_child_scopes() {
        let mut parent = LifetimeScope::new("'outer");
        {
            let child = parent.child_scope("'inner");
            child.add_shared_borrow("buf", "Vec<u8>").unwrap();
            assert_eq!(child.active_borrow_count(), 1);
        }
        // End parent ends all children
        parent.end_scope();
        assert!(!parent.active);
        assert!(!parent.children[0].active);
        assert_eq!(parent.children[0].active_borrow_count(), 0);
    }
}
