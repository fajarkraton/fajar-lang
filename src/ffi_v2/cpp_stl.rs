//! Sprint E3: C++ STL Container Bridge.
//!
//! Provides Fajar Lang wrappers around simulated C++ STL containers.
//! Each wrapper stores data internally using Rust types (no real C++ linkage)
//! and exposes conversions to/from Fajar Lang equivalents.
//!
//! Containers: string, vector, map, set, optional, variant, tuple, array, span.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// CppValue — type-erased storage for STL container elements
// ═══════════════════════════════════════════════════════════════════════

/// Type-erased value mirroring C++ runtime types.
///
/// Used as the element type inside STL container wrappers so that a single
/// container can hold heterogeneous data (e.g. `variant`, `tuple`).
#[derive(Debug, Clone, PartialEq)]
pub enum CppValue {
    /// A 64-bit signed integer (`int64_t`).
    Int(i64),
    /// A 64-bit float (`double`).
    Float(f64),
    /// A UTF-8 string (`std::string`).
    String(String),
    /// A boolean (`bool`).
    Bool(bool),
    /// A dynamically-typed array (`std::vector<CppValue>`).
    Array(Vec<CppValue>),
    /// A string-keyed map (`std::map<std::string, CppValue>`).
    Map(BTreeMap<String, CppValue>),
}

impl fmt::Display for CppValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Array(v) => write!(f, "{v:?}"),
            Self::Map(v) => write!(f, "{v:?}"),
        }
    }
}

impl CppValue {
    /// Returns the type name of the contained C++ value.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Int(_) => "int64_t",
            Self::Float(_) => "double",
            Self::String(_) => "std::string",
            Self::Bool(_) => "bool",
            Self::Array(_) => "std::vector",
            Self::Map(_) => "std::map",
        }
    }

    /// Attempts to extract an `i64`.
    pub fn as_int(&self) -> Option<i64> {
        if let Self::Int(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// Attempts to extract an `f64`.
    pub fn as_float(&self) -> Option<f64> {
        if let Self::Float(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// Attempts to extract a `&str`.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v.as_str())
        } else {
            None
        }
    }

    /// Attempts to extract a `bool`.
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Common error type
// ═══════════════════════════════════════════════════════════════════════

/// Errors raised by STL container operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StlError {
    /// Index was out of bounds.
    IndexOutOfBounds {
        /// The index that was requested.
        index: usize,
        /// The length of the container.
        length: usize,
    },
    /// Key was not found in a map.
    KeyNotFound {
        /// The missing key.
        key: String,
    },
    /// An `optional`/`variant` had no value of the requested type.
    EmptyAccess {
        /// Human-readable context.
        context: String,
    },
    /// Type mismatch during conversion.
    TypeMismatch {
        /// Expected type name.
        expected: String,
        /// Actual type name.
        actual: String,
    },
    /// Fixed-size container had a size mismatch.
    SizeMismatch {
        /// Expected size.
        expected: usize,
        /// Actual size.
        actual: usize,
    },
}

impl fmt::Display for StlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfBounds { index, length } => {
                write!(f, "index {index} out of bounds (length {length})")
            }
            Self::KeyNotFound { key } => write!(f, "key not found: {key}"),
            Self::EmptyAccess { context } => write!(f, "empty access: {context}"),
            Self::TypeMismatch { expected, actual } => {
                write!(f, "type mismatch: expected {expected}, got {actual}")
            }
            Self::SizeMismatch { expected, actual } => {
                write!(f, "size mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.1: std::string ↔ str
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::string`.
///
/// Provides conversion between C++ `std::string` and Fajar Lang `str`.
/// Internally stores data as a Rust `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StlString {
    /// The internal UTF-8 data.
    data: String,
}

impl StlString {
    /// Creates a new `StlString` from a Rust `&str` (simulating construction
    /// from a Fajar Lang `str` literal).
    pub fn from_fajar_str(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }

    /// Converts this `StlString` back to a Fajar Lang `str` (as `String`).
    pub fn to_fajar_str(&self) -> String {
        self.data.clone()
    }

    /// Returns the length in bytes (like `std::string::size()`).
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if empty.
    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Appends another `StlString` (like `operator+=`).
    pub fn append(&mut self, other: &StlString) {
        self.data.push_str(&other.data);
    }

    /// Returns a substring by byte range (clamped to bounds).
    pub fn substr(&self, pos: usize, len: usize) -> Self {
        let end = (pos + len).min(self.data.len());
        let start = pos.min(self.data.len());
        Self {
            data: self.data[start..end].to_string(),
        }
    }

    /// Returns the byte at `index`, or an error if out of bounds.
    pub fn at(&self, index: usize) -> Result<u8, StlError> {
        self.data
            .as_bytes()
            .get(index)
            .copied()
            .ok_or(StlError::IndexOutOfBounds {
                index,
                length: self.data.len(),
            })
    }

    /// Finds the first occurrence of `needle`, returning the byte offset.
    pub fn find(&self, needle: &str) -> Option<usize> {
        self.data.find(needle)
    }

    /// Returns the underlying `CppValue::String`.
    pub fn to_cpp_value(&self) -> CppValue {
        CppValue::String(self.data.clone())
    }
}

impl fmt::Display for StlString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.2: vector<T> ↔ Array<T>
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::vector<T>`.
///
/// Elements are stored as `CppValue` for type-erased access.
/// Provides `push_back`, `at`, `size`, and conversion to a Fajar Lang array.
#[derive(Debug, Clone, PartialEq)]
pub struct StlVector {
    /// Element storage.
    data: Vec<CppValue>,
}

impl StlVector {
    /// Creates an empty vector.
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Creates a vector from an existing `Vec<CppValue>`.
    pub fn from_vec(v: Vec<CppValue>) -> Self {
        Self { data: v }
    }

    /// Appends an element (like `push_back`).
    pub fn push_back(&mut self, value: CppValue) {
        self.data.push(value);
    }

    /// Returns the element at `index`, or an error if out of bounds.
    pub fn at(&self, index: usize) -> Result<&CppValue, StlError> {
        self.data.get(index).ok_or(StlError::IndexOutOfBounds {
            index,
            length: self.data.len(),
        })
    }

    /// Returns the number of elements (like `size()`).
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the vector is empty.
    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Removes the last element and returns it, or `None` if empty.
    pub fn pop_back(&mut self) -> Option<CppValue> {
        self.data.pop()
    }

    /// Converts to a Fajar Lang `Array` representation (a `Vec<CppValue>`).
    pub fn to_fajar_array(&self) -> Vec<CppValue> {
        self.data.clone()
    }

    /// Creates an `StlVector` from a Fajar Lang array.
    pub fn from_fajar_array(arr: Vec<CppValue>) -> Self {
        Self { data: arr }
    }

    /// Returns the underlying data as a `CppValue::Array`.
    pub fn to_cpp_value(&self) -> CppValue {
        CppValue::Array(self.data.clone())
    }
}

impl Default for StlVector {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.3: map<K,V> ↔ HashMap
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::map<std::string, CppValue>`.
///
/// Uses `BTreeMap` to provide sorted key order (matching C++ `std::map`).
/// Keys are `String`; values are `CppValue`.
#[derive(Debug, Clone, PartialEq)]
pub struct StlMap {
    /// Sorted key-value storage.
    data: BTreeMap<String, CppValue>,
}

impl StlMap {
    /// Creates an empty map.
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    /// Inserts a key-value pair (like `insert` / `operator[]`).
    pub fn insert(&mut self, key: String, value: CppValue) {
        self.data.insert(key, value);
    }

    /// Returns the value for `key`, or an error if not found.
    pub fn get(&self, key: &str) -> Result<&CppValue, StlError> {
        self.data.get(key).ok_or_else(|| StlError::KeyNotFound {
            key: key.to_string(),
        })
    }

    /// Returns `true` if the map contains `key`.
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Returns all keys in sorted order.
    pub fn keys(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }

    /// Returns the number of entries (like `size()`).
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the map is empty.
    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Removes a key and returns its value, or `None`.
    pub fn erase(&mut self, key: &str) -> Option<CppValue> {
        self.data.remove(key)
    }

    /// Converts to a Fajar Lang `HashMap` representation (`BTreeMap`).
    pub fn to_fajar_map(&self) -> BTreeMap<String, CppValue> {
        self.data.clone()
    }

    /// Creates an `StlMap` from a Fajar Lang map.
    pub fn from_fajar_map(map: BTreeMap<String, CppValue>) -> Self {
        Self { data: map }
    }

    /// Returns the underlying data as a `CppValue::Map`.
    pub fn to_cpp_value(&self) -> CppValue {
        CppValue::Map(self.data.clone())
    }
}

impl Default for StlMap {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.4: set<T> ↔ HashSet<T>
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::set<std::string>`.
///
/// Uses `BTreeSet` for sorted order (matching C++ `std::set`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StlSet {
    /// Sorted unique elements.
    data: BTreeSet<String>,
}

impl StlSet {
    /// Creates an empty set.
    pub fn new() -> Self {
        Self {
            data: BTreeSet::new(),
        }
    }

    /// Inserts an element. Returns `true` if the element was new.
    pub fn insert(&mut self, value: String) -> bool {
        self.data.insert(value)
    }

    /// Returns `true` if the set contains `value`.
    pub fn contains(&self, value: &str) -> bool {
        self.data.contains(value)
    }

    /// Removes an element. Returns `true` if it was present.
    pub fn erase(&mut self, value: &str) -> bool {
        self.data.remove(value)
    }

    /// Returns the number of elements (like `size()`).
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the set is empty.
    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Converts to a Fajar Lang `HashSet` representation (`BTreeSet<String>`).
    pub fn to_fajar_set(&self) -> BTreeSet<String> {
        self.data.clone()
    }

    /// Creates an `StlSet` from a Fajar Lang set.
    pub fn from_fajar_set(set: BTreeSet<String>) -> Self {
        Self { data: set }
    }

    /// Returns all elements as a sorted `Vec<String>`.
    pub fn to_sorted_vec(&self) -> Vec<String> {
        self.data.iter().cloned().collect()
    }
}

impl Default for StlSet {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.5: optional<T> ↔ Option<T>
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::optional<T>`.
///
/// Maps directly to Rust/Fajar Lang `Option<CppValue>`.
#[derive(Debug, Clone, PartialEq)]
pub struct StlOptional {
    /// The contained value, or `None` if `nullopt`.
    inner: Option<CppValue>,
}

impl StlOptional {
    /// Creates an engaged optional holding `value`.
    pub fn some(value: CppValue) -> Self {
        Self { inner: Some(value) }
    }

    /// Creates a disengaged optional (`std::nullopt`).
    pub fn none() -> Self {
        Self { inner: None }
    }

    /// Returns `true` if the optional holds a value (like `has_value()`).
    pub fn has_value(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns the contained value, or an error if disengaged.
    pub fn value(&self) -> Result<&CppValue, StlError> {
        self.inner.as_ref().ok_or_else(|| StlError::EmptyAccess {
            context: "std::optional has no value (nullopt)".to_string(),
        })
    }

    /// Returns the contained value or a default.
    pub fn value_or(&self, default: CppValue) -> CppValue {
        match &self.inner {
            Some(v) => v.clone(),
            None => default,
        }
    }

    /// Converts to a Fajar Lang `Option<CppValue>`.
    pub fn to_fajar_option(&self) -> Option<CppValue> {
        self.inner.clone()
    }

    /// Creates an `StlOptional` from a Fajar Lang `Option<CppValue>`.
    pub fn from_fajar_option(opt: Option<CppValue>) -> Self {
        Self { inner: opt }
    }

    /// Resets the optional to disengaged.
    pub fn reset(&mut self) {
        self.inner = None;
    }
}

impl Default for StlOptional {
    fn default() -> Self {
        Self::none()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.6: variant<T...> ↔ enum
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::variant<Types...>`.
///
/// Tracks the active alternative by index and stores its value.
/// Provides `index()`, `get()`, and a simple `visit()` pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct StlVariant {
    /// Zero-based index of the active alternative.
    active_index: usize,
    /// The active value.
    value: CppValue,
    /// Names of the allowed alternative types (for error messages).
    type_names: Vec<String>,
}

impl StlVariant {
    /// Creates a new variant with the given alternatives and initial value.
    ///
    /// `type_names` describes the allowed types (e.g. `["int", "string", "bool"]`).
    /// The initial value is placed at `index`.
    pub fn new(type_names: Vec<String>, index: usize, value: CppValue) -> Result<Self, StlError> {
        if index >= type_names.len() {
            return Err(StlError::IndexOutOfBounds {
                index,
                length: type_names.len(),
            });
        }
        Ok(Self {
            active_index: index,
            value,
            type_names,
        })
    }

    /// Returns the zero-based index of the active alternative.
    pub fn index(&self) -> usize {
        self.active_index
    }

    /// Returns the active value if `index` matches, otherwise an error
    /// (like `std::get<I>(v)` throwing `bad_variant_access`).
    pub fn get(&self, index: usize) -> Result<&CppValue, StlError> {
        if index == self.active_index {
            Ok(&self.value)
        } else {
            Err(StlError::TypeMismatch {
                expected: self
                    .type_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("index {index}")),
                actual: self
                    .type_names
                    .get(self.active_index)
                    .cloned()
                    .unwrap_or_else(|| format!("index {}", self.active_index)),
            })
        }
    }

    /// Applies a visitor function to the active value and returns the result
    /// (simplified `std::visit`).
    pub fn visit<F, R>(&self, visitor: F) -> R
    where
        F: FnOnce(usize, &CppValue) -> R,
    {
        visitor(self.active_index, &self.value)
    }

    /// Sets a new active alternative.
    pub fn emplace(&mut self, index: usize, value: CppValue) -> Result<(), StlError> {
        if index >= self.type_names.len() {
            return Err(StlError::IndexOutOfBounds {
                index,
                length: self.type_names.len(),
            });
        }
        self.active_index = index;
        self.value = value;
        Ok(())
    }

    /// Returns the number of allowed alternatives.
    pub fn variant_size(&self) -> usize {
        self.type_names.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.7: tuple<T...> ↔ tuple
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::tuple<Types...>`.
///
/// Elements are stored positionally as `CppValue`.
/// Provides index-based access and conversion to a Fajar Lang tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct StlTuple {
    /// Positional elements.
    elements: Vec<CppValue>,
}

impl StlTuple {
    /// Creates a tuple from a list of elements.
    pub fn new(elements: Vec<CppValue>) -> Self {
        Self { elements }
    }

    /// Returns the element at `index`, or an error if out of bounds
    /// (like `std::get<I>(t)`).
    pub fn get_element(&self, index: usize) -> Result<&CppValue, StlError> {
        self.elements.get(index).ok_or(StlError::IndexOutOfBounds {
            index,
            length: self.elements.len(),
        })
    }

    /// Returns the number of elements (like `std::tuple_size`).
    pub fn size(&self) -> usize {
        self.elements.len()
    }

    /// Converts to a Fajar Lang tuple (`Vec<CppValue>`).
    pub fn to_fajar_tuple(&self) -> Vec<CppValue> {
        self.elements.clone()
    }

    /// Creates an `StlTuple` from a Fajar Lang tuple.
    pub fn from_fajar_tuple(elements: Vec<CppValue>) -> Self {
        Self { elements }
    }

    /// Returns all elements as a slice.
    pub fn as_slice(&self) -> &[CppValue] {
        &self.elements
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.8: array<T,N> ↔ [T; N]
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::array<T, N>`.
///
/// The capacity `N` is fixed at construction time. Attempts to create
/// from data of a different length return an error.
#[derive(Debug, Clone, PartialEq)]
pub struct StlArray {
    /// Fixed-size element storage.
    data: Vec<CppValue>,
    /// The compile-time size `N`.
    capacity: usize,
}

impl StlArray {
    /// Creates a fixed-size array from `data`.
    ///
    /// Returns an error if `data.len() != capacity`.
    pub fn new(data: Vec<CppValue>, capacity: usize) -> Result<Self, StlError> {
        if data.len() != capacity {
            return Err(StlError::SizeMismatch {
                expected: capacity,
                actual: data.len(),
            });
        }
        Ok(Self { data, capacity })
    }

    /// Creates a fixed-size array filled with a default value.
    pub fn filled(value: CppValue, capacity: usize) -> Self {
        Self {
            data: vec![value; capacity],
            capacity,
        }
    }

    /// Returns the element at `index`, or an error if out of bounds.
    pub fn at(&self, index: usize) -> Result<&CppValue, StlError> {
        self.data.get(index).ok_or(StlError::IndexOutOfBounds {
            index,
            length: self.capacity,
        })
    }

    /// Sets the element at `index`, or returns an error if out of bounds.
    pub fn set(&mut self, index: usize, value: CppValue) -> Result<(), StlError> {
        if index >= self.capacity {
            return Err(StlError::IndexOutOfBounds {
                index,
                length: self.capacity,
            });
        }
        self.data[index] = value;
        Ok(())
    }

    /// Returns the fixed size `N`.
    pub fn size(&self) -> usize {
        self.capacity
    }

    /// Converts to a Fajar Lang fixed-size array (`Vec<CppValue>`).
    pub fn to_fajar_array(&self) -> Vec<CppValue> {
        self.data.clone()
    }

    /// Creates an `StlArray` from a Fajar Lang fixed-size array.
    pub fn from_fajar_array(data: Vec<CppValue>) -> Self {
        let capacity = data.len();
        Self { data, capacity }
    }

    /// Returns the underlying data as a slice.
    pub fn as_slice(&self) -> &[CppValue] {
        &self.data
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.9: span<T> ↔ slice
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper around a simulated `std::span<T>`.
///
/// A non-owning view into a contiguous range of `CppValue` elements.
/// The span borrows data from another container (vector, array, etc.)
/// and provides no-copy access.
#[derive(Debug, Clone, PartialEq)]
pub struct StlSpan {
    /// Offset into the source container.
    offset: usize,
    /// Number of elements in the span.
    length: usize,
    /// Snapshot of the viewed data (simulating a borrow — real FFI would
    /// use a raw pointer + length).
    data: Vec<CppValue>,
}

impl StlSpan {
    /// Creates a span viewing `source[offset .. offset + length]`.
    ///
    /// Returns an error if the range exceeds the source length.
    pub fn new(source: &[CppValue], offset: usize, length: usize) -> Result<Self, StlError> {
        if offset + length > source.len() {
            return Err(StlError::IndexOutOfBounds {
                index: offset + length,
                length: source.len(),
            });
        }
        Ok(Self {
            offset,
            length,
            data: source[offset..offset + length].to_vec(),
        })
    }

    /// Creates a span viewing the entire source.
    pub fn from_slice(source: &[CppValue]) -> Self {
        Self {
            offset: 0,
            length: source.len(),
            data: source.to_vec(),
        }
    }

    /// Returns the element at `index` within the span.
    pub fn at(&self, index: usize) -> Result<&CppValue, StlError> {
        self.data.get(index).ok_or(StlError::IndexOutOfBounds {
            index,
            length: self.length,
        })
    }

    /// Returns the number of elements in the span.
    pub fn size(&self) -> usize {
        self.length
    }

    /// Returns `true` if the span is empty.
    pub fn empty(&self) -> bool {
        self.length == 0
    }

    /// Returns the span's offset into the original container.
    pub fn source_offset(&self) -> usize {
        self.offset
    }

    /// Returns a sub-span.
    pub fn subspan(&self, offset: usize, count: usize) -> Result<Self, StlError> {
        if offset + count > self.length {
            return Err(StlError::IndexOutOfBounds {
                index: offset + count,
                length: self.length,
            });
        }
        Ok(Self {
            offset: self.offset + offset,
            length: count,
            data: self.data[offset..offset + count].to_vec(),
        })
    }

    /// Converts the span contents to a Fajar Lang slice (`Vec<CppValue>`).
    pub fn to_fajar_slice(&self) -> Vec<CppValue> {
        self.data.clone()
    }

    /// Returns the underlying data as a Rust slice.
    pub fn as_slice(&self) -> &[CppValue] {
        &self.data
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E3.10: Tests (≥ 15)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- E3.1: StlString ---

    #[test]
    fn stl_string_roundtrip() {
        let s = StlString::from_fajar_str("hello world");
        assert_eq!(s.to_fajar_str(), "hello world");
        assert_eq!(s.size(), 11);
        assert!(!s.empty());
    }

    #[test]
    fn stl_string_append_and_substr() {
        let mut a = StlString::from_fajar_str("hello");
        let b = StlString::from_fajar_str(" world");
        a.append(&b);
        assert_eq!(a.to_fajar_str(), "hello world");
        let sub = a.substr(0, 5);
        assert_eq!(sub.to_fajar_str(), "hello");
    }

    #[test]
    fn stl_string_at_and_find() {
        let s = StlString::from_fajar_str("abc");
        assert_eq!(s.at(0).unwrap(), b'a');
        assert_eq!(s.at(2).unwrap(), b'c');
        assert!(s.at(10).is_err());
        assert_eq!(s.find("bc"), Some(1));
        assert_eq!(s.find("xyz"), None);
    }

    // --- E3.2: StlVector ---

    #[test]
    fn stl_vector_push_back_at_size() {
        let mut v = StlVector::new();
        assert!(v.empty());
        v.push_back(CppValue::Int(1));
        v.push_back(CppValue::Int(2));
        v.push_back(CppValue::Int(3));
        assert_eq!(v.size(), 3);
        assert_eq!(v.at(0).unwrap(), &CppValue::Int(1));
        assert_eq!(v.at(2).unwrap(), &CppValue::Int(3));
        assert!(v.at(5).is_err());
    }

    #[test]
    fn stl_vector_fajar_roundtrip() {
        let data = vec![CppValue::String("a".into()), CppValue::String("b".into())];
        let v = StlVector::from_fajar_array(data.clone());
        assert_eq!(v.to_fajar_array(), data);
    }

    // --- E3.3: StlMap ---

    #[test]
    fn stl_map_insert_get_keys() {
        let mut m = StlMap::new();
        m.insert("x".into(), CppValue::Int(10));
        m.insert("y".into(), CppValue::Int(20));
        assert_eq!(m.size(), 2);
        assert_eq!(m.get("x").unwrap(), &CppValue::Int(10));
        assert!(m.get("z").is_err());
        assert!(m.contains("y"));
        assert!(!m.contains("z"));
        assert_eq!(m.keys(), vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn stl_map_fajar_roundtrip() {
        let mut m = StlMap::new();
        m.insert("k".into(), CppValue::Bool(true));
        let fajar_map = m.to_fajar_map();
        let m2 = StlMap::from_fajar_map(fajar_map);
        assert_eq!(m, m2);
    }

    // --- E3.4: StlSet ---

    #[test]
    fn stl_set_insert_contains_erase() {
        let mut s = StlSet::new();
        assert!(s.insert("alpha".into()));
        assert!(s.insert("beta".into()));
        assert!(!s.insert("alpha".into())); // duplicate
        assert_eq!(s.size(), 2);
        assert!(s.contains("alpha"));
        assert!(!s.contains("gamma"));
        assert!(s.erase("alpha"));
        assert!(!s.contains("alpha"));
        assert_eq!(s.size(), 1);
    }

    #[test]
    fn stl_set_fajar_roundtrip() {
        let mut s = StlSet::new();
        s.insert("one".into());
        s.insert("two".into());
        let fajar_set = s.to_fajar_set();
        let s2 = StlSet::from_fajar_set(fajar_set);
        assert_eq!(s, s2);
    }

    // --- E3.5: StlOptional ---

    #[test]
    fn stl_optional_some_and_none() {
        let some = StlOptional::some(CppValue::Int(42));
        assert!(some.has_value());
        assert_eq!(some.value().unwrap(), &CppValue::Int(42));

        let none = StlOptional::none();
        assert!(!none.has_value());
        assert!(none.value().is_err());
    }

    #[test]
    fn stl_optional_value_or_and_reset() {
        let opt = StlOptional::none();
        assert_eq!(opt.value_or(CppValue::Int(99)), CppValue::Int(99));

        let mut opt2 = StlOptional::some(CppValue::Bool(true));
        assert_eq!(opt2.value_or(CppValue::Bool(false)), CppValue::Bool(true));
        opt2.reset();
        assert!(!opt2.has_value());
    }

    #[test]
    fn stl_optional_fajar_roundtrip() {
        let opt = StlOptional::some(CppValue::String("hi".into()));
        let fajar = opt.to_fajar_option();
        let opt2 = StlOptional::from_fajar_option(fajar);
        assert_eq!(opt, opt2);
    }

    // --- E3.6: StlVariant ---

    #[test]
    fn stl_variant_index_and_get() {
        let types = vec!["int".into(), "string".into(), "bool".into()];
        let v = StlVariant::new(types, 1, CppValue::String("hello".into())).unwrap();
        assert_eq!(v.index(), 1);
        assert_eq!(v.get(1).unwrap(), &CppValue::String("hello".into()));
        assert!(v.get(0).is_err()); // wrong alternative
        assert_eq!(v.variant_size(), 3);
    }

    #[test]
    fn stl_variant_visit_and_emplace() {
        let types = vec!["int".into(), "float".into()];
        let mut v = StlVariant::new(types, 0, CppValue::Int(7)).unwrap();

        let label = v.visit(|idx, val| format!("alt{idx}={val}"));
        assert_eq!(label, "alt0=7");

        v.emplace(1, CppValue::Float(3.14)).unwrap();
        assert_eq!(v.index(), 1);
        assert!(v.emplace(5, CppValue::Bool(true)).is_err());
    }

    // --- E3.7: StlTuple ---

    #[test]
    fn stl_tuple_get_element_and_size() {
        let t = StlTuple::new(vec![
            CppValue::Int(1),
            CppValue::String("two".into()),
            CppValue::Bool(false),
        ]);
        assert_eq!(t.size(), 3);
        assert_eq!(t.get_element(0).unwrap(), &CppValue::Int(1));
        assert_eq!(t.get_element(1).unwrap(), &CppValue::String("two".into()));
        assert!(t.get_element(5).is_err());
    }

    #[test]
    fn stl_tuple_fajar_roundtrip() {
        let elems = vec![CppValue::Float(1.5), CppValue::Bool(true)];
        let t = StlTuple::from_fajar_tuple(elems.clone());
        assert_eq!(t.to_fajar_tuple(), elems);
    }

    // --- E3.8: StlArray ---

    #[test]
    fn stl_array_fixed_size() {
        let arr = StlArray::new(
            vec![CppValue::Int(10), CppValue::Int(20), CppValue::Int(30)],
            3,
        )
        .unwrap();
        assert_eq!(arr.size(), 3);
        assert_eq!(arr.at(1).unwrap(), &CppValue::Int(20));
        assert!(arr.at(3).is_err());

        // wrong size
        let err = StlArray::new(vec![CppValue::Int(1)], 5);
        assert!(err.is_err());
    }

    #[test]
    fn stl_array_set_and_filled() {
        let mut arr = StlArray::filled(CppValue::Int(0), 4);
        assert_eq!(arr.size(), 4);
        arr.set(2, CppValue::Int(99)).unwrap();
        assert_eq!(arr.at(2).unwrap(), &CppValue::Int(99));
        assert!(arr.set(10, CppValue::Int(1)).is_err());
    }

    // --- E3.9: StlSpan ---

    #[test]
    fn stl_span_view_no_copy() {
        let source = vec![
            CppValue::Int(1),
            CppValue::Int(2),
            CppValue::Int(3),
            CppValue::Int(4),
        ];
        let span = StlSpan::new(&source, 1, 2).unwrap();
        assert_eq!(span.size(), 2);
        assert_eq!(span.source_offset(), 1);
        assert_eq!(span.at(0).unwrap(), &CppValue::Int(2));
        assert_eq!(span.at(1).unwrap(), &CppValue::Int(3));
        assert!(span.at(5).is_err());
    }

    #[test]
    fn stl_span_subspan_and_full() {
        let source = vec![
            CppValue::Int(10),
            CppValue::Int(20),
            CppValue::Int(30),
            CppValue::Int(40),
        ];
        let full = StlSpan::from_slice(&source);
        assert_eq!(full.size(), 4);

        let sub = full.subspan(1, 2).unwrap();
        assert_eq!(sub.size(), 2);
        assert_eq!(sub.at(0).unwrap(), &CppValue::Int(20));

        assert!(full.subspan(2, 5).is_err());
    }

    #[test]
    fn stl_span_out_of_bounds_construction() {
        let source = vec![CppValue::Int(1)];
        assert!(StlSpan::new(&source, 0, 5).is_err());
        assert!(StlSpan::new(&source, 2, 1).is_err());
    }

    // --- Cross-container ---

    #[test]
    fn cpp_value_type_names_and_accessors() {
        assert_eq!(CppValue::Int(1).type_name(), "int64_t");
        assert_eq!(CppValue::Float(1.0).type_name(), "double");
        assert_eq!(CppValue::String("s".into()).type_name(), "std::string");
        assert_eq!(CppValue::Bool(true).type_name(), "bool");
        assert_eq!(CppValue::Array(vec![]).type_name(), "std::vector");
        assert_eq!(CppValue::Map(BTreeMap::new()).type_name(), "std::map");

        assert_eq!(CppValue::Int(42).as_int(), Some(42));
        assert_eq!(CppValue::Int(42).as_float(), None);
        assert_eq!(CppValue::String("hi".into()).as_str(), Some("hi"));
        assert_eq!(CppValue::Bool(true).as_bool(), Some(true));
    }

    #[test]
    fn stl_string_to_cpp_value() {
        let s = StlString::from_fajar_str("test");
        assert_eq!(s.to_cpp_value(), CppValue::String("test".into()));
    }

    #[test]
    fn stl_vector_pop_back() {
        let mut v = StlVector::new();
        v.push_back(CppValue::Int(1));
        v.push_back(CppValue::Int(2));
        assert_eq!(v.pop_back(), Some(CppValue::Int(2)));
        assert_eq!(v.size(), 1);
        assert_eq!(v.pop_back(), Some(CppValue::Int(1)));
        assert_eq!(v.pop_back(), None);
    }

    #[test]
    fn stl_map_erase_and_empty() {
        let mut m = StlMap::new();
        assert!(m.empty());
        m.insert("a".into(), CppValue::Int(1));
        assert!(!m.empty());
        assert_eq!(m.erase("a"), Some(CppValue::Int(1)));
        assert!(m.empty());
        assert_eq!(m.erase("a"), None);
    }

    #[test]
    fn stl_error_display() {
        let e = StlError::IndexOutOfBounds {
            index: 5,
            length: 3,
        };
        assert_eq!(e.to_string(), "index 5 out of bounds (length 3)");

        let e2 = StlError::KeyNotFound { key: "foo".into() };
        assert_eq!(e2.to_string(), "key not found: foo");
    }

    #[test]
    fn stl_variant_bad_construction() {
        let result = StlVariant::new(vec!["int".into()], 5, CppValue::Int(1));
        assert!(result.is_err());
    }
}
