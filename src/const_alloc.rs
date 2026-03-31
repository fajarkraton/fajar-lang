//! Compile-time allocation — lowers `ComptimeValue` to static `.rodata` representations.
//!
//! # Overview
//!
//! When a `const` declaration or `comptime { ... }` block is evaluated, the resulting
//! `ComptimeValue` needs to be stored in the binary's read-only data section (`.rodata`).
//! This module handles:
//!
//! - **Serialization** of `ComptimeValue` to byte arrays
//! - **Static promotion** of temporary const expressions to static storage
//! - **Size verification** to warn about overly large const data
//! - **Cross-compilation** awareness (target pointer size)
//! - **Arena allocation** for grouping compile-time data
//!
//! # Example
//!
//! ```fajar
//! const PRIMES: [i32; 5] = comptime { [2, 3, 5, 7, 11] }
//! // → serialized as 20 bytes in .rodata: [02 00 00 00, 03 00 00 00, ...]
//! ```

use std::collections::HashMap;

use crate::analyzer::comptime::ComptimeValue;

/// Maximum const allocation size before emitting a warning (1 MB).
const WARN_THRESHOLD_BYTES: usize = 1_048_576;

// ═══════════════════════════════════════════════════════════════════════
// K4.1-K4.3: Const Data Serialization
// ═══════════════════════════════════════════════════════════════════════

/// Target architecture info for cross-compilation const eval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetInfo {
    /// Pointer size in bytes (4 for 32-bit, 8 for 64-bit).
    pub pointer_size: usize,
    /// Whether the target is little-endian.
    pub little_endian: bool,
}

impl TargetInfo {
    /// x86_64 target (default).
    pub fn x86_64() -> Self {
        Self {
            pointer_size: 8,
            little_endian: true,
        }
    }

    /// ARM64 target.
    pub fn aarch64() -> Self {
        Self {
            pointer_size: 8,
            little_endian: true,
        }
    }

    /// 32-bit target (e.g., WASM32, ARM32).
    pub fn wasm32() -> Self {
        Self {
            pointer_size: 4,
            little_endian: true,
        }
    }
}

impl Default for TargetInfo {
    fn default() -> Self {
        Self::x86_64()
    }
}

/// A serialized const value ready for `.rodata` placement.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstAllocation {
    /// Symbolic name (e.g., `"PRIMES"`, `"CONFIG"`).
    pub name: String,
    /// Raw bytes for `.rodata`.
    pub bytes: Vec<u8>,
    /// Alignment requirement in bytes.
    pub align: usize,
    /// Section name (default: `.rodata`).
    pub section: String,
    /// Source type description for debugging.
    pub type_desc: String,
}

impl ConstAllocation {
    /// Size in bytes.
    pub fn size(&self) -> usize {
        self.bytes.len()
    }

    /// Whether this allocation exceeds the warning threshold.
    pub fn is_large(&self) -> bool {
        self.bytes.len() > WARN_THRESHOLD_BYTES
    }
}

/// Serializes a `ComptimeValue` into a `ConstAllocation`.
pub fn serialize_const(name: &str, value: &ComptimeValue, target: &TargetInfo) -> ConstAllocation {
    let mut bytes = Vec::new();
    let type_desc = serialize_value(value, target, &mut bytes);
    let align = alignment_for(value, target);

    ConstAllocation {
        name: name.to_string(),
        bytes,
        align,
        section: ".rodata".to_string(),
        type_desc,
    }
}

/// Recursively serializes a comptime value to bytes. Returns a type description.
fn serialize_value(value: &ComptimeValue, target: &TargetInfo, out: &mut Vec<u8>) -> String {
    match value {
        ComptimeValue::Int(v) => {
            if target.little_endian {
                out.extend_from_slice(&v.to_le_bytes());
            } else {
                out.extend_from_slice(&v.to_be_bytes());
            }
            "i64".to_string()
        }
        ComptimeValue::Float(v) => {
            if target.little_endian {
                out.extend_from_slice(&v.to_le_bytes());
            } else {
                out.extend_from_slice(&v.to_be_bytes());
            }
            "f64".to_string()
        }
        ComptimeValue::Bool(b) => {
            out.push(if *b { 1 } else { 0 });
            "bool".to_string()
        }
        ComptimeValue::Str(s) => {
            // Store as: [length: usize][utf8 bytes...]
            let len_bytes = s.len();
            if target.pointer_size == 8 {
                if target.little_endian {
                    out.extend_from_slice(&(len_bytes as u64).to_le_bytes());
                } else {
                    out.extend_from_slice(&(len_bytes as u64).to_be_bytes());
                }
            } else if target.little_endian {
                out.extend_from_slice(&(len_bytes as u32).to_le_bytes());
            } else {
                out.extend_from_slice(&(len_bytes as u32).to_be_bytes());
            }
            out.extend_from_slice(s.as_bytes());
            format!("str(len={})", len_bytes)
        }
        ComptimeValue::Array(items) => {
            // Store as: [count: u64][element0][element1]...
            let count = items.len() as u64;
            if target.little_endian {
                out.extend_from_slice(&count.to_le_bytes());
            } else {
                out.extend_from_slice(&count.to_be_bytes());
            }
            let elem_type = if let Some(first) = items.first() {
                let desc = serialize_value(first, target, out);
                for item in items.iter().skip(1) {
                    serialize_value(item, target, out);
                }
                desc
            } else {
                "void".to_string()
            };
            format!("[{}; {}]", elem_type, items.len())
        }
        ComptimeValue::Struct { name, fields } => {
            for (_, fval) in fields {
                serialize_value(fval, target, out);
            }
            name.clone()
        }
        ComptimeValue::Tuple(items) => {
            let descs: Vec<String> = items
                .iter()
                .map(|item| serialize_value(item, target, out))
                .collect();
            format!("({})", descs.join(", "))
        }
        ComptimeValue::Null => {
            out.push(0);
            "null".to_string()
        }
    }
}

/// Compute alignment for a comptime value.
fn alignment_for(value: &ComptimeValue, target: &TargetInfo) -> usize {
    match value {
        ComptimeValue::Int(_) => 8,
        ComptimeValue::Float(_) => 8,
        ComptimeValue::Bool(_) => 1,
        ComptimeValue::Str(_) => target.pointer_size,
        ComptimeValue::Array(items) => {
            if let Some(first) = items.first() {
                alignment_for(first, target)
            } else {
                1
            }
        }
        ComptimeValue::Struct { fields, .. } => fields
            .iter()
            .map(|(_, v)| alignment_for(v, target))
            .max()
            .unwrap_or(1),
        ComptimeValue::Tuple(items) => items
            .iter()
            .map(|v| alignment_for(v, target))
            .max()
            .unwrap_or(1),
        ComptimeValue::Null => 1,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K4.4: Const HashMap (precomputed as static array of key-value pairs)
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time HashMap stored as sorted key-value pairs.
///
/// At runtime, lookups use binary search on the sorted keys.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstHashMap {
    /// Name of the const map.
    pub name: String,
    /// Sorted entries: (key, value).
    pub entries: Vec<(ComptimeValue, ComptimeValue)>,
}

impl ConstHashMap {
    /// Creates a new const HashMap from key-value pairs.
    pub fn new(name: &str, mut entries: Vec<(ComptimeValue, ComptimeValue)>) -> Self {
        // Sort by key for binary search at runtime
        entries.sort_by(|a, b| {
            let a_key = comptime_sort_key(&a.0);
            let b_key = comptime_sort_key(&b.0);
            a_key.cmp(&b_key)
        });
        Self {
            name: name.to_string(),
            entries,
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a key at compile time.
    pub fn get(&self, key: &ComptimeValue) -> Option<&ComptimeValue> {
        let target = comptime_sort_key(key);
        self.entries
            .binary_search_by(|(k, _)| comptime_sort_key(k).cmp(&target))
            .ok()
            .map(|idx| &self.entries[idx].1)
    }
}

/// Generates a sort key for a comptime value (for HashMap ordering).
fn comptime_sort_key(val: &ComptimeValue) -> String {
    match val {
        ComptimeValue::Int(v) => format!("i:{v:020}"),
        ComptimeValue::Float(v) => format!("f:{v}"),
        ComptimeValue::Str(s) => format!("s:{s}"),
        ComptimeValue::Bool(b) => format!("b:{}", if *b { 1 } else { 0 }),
        _ => format!("?:{val}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K4.5: Const Slices
// ═══════════════════════════════════════════════════════════════════════

/// A const slice — a (pointer, length) pair pointing into `.rodata`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstSlice {
    /// Name of the source allocation.
    pub source_name: String,
    /// Byte offset into the source allocation.
    pub offset: usize,
    /// Number of elements.
    pub length: usize,
    /// Element type description.
    pub elem_type: String,
}

// ═══════════════════════════════════════════════════════════════════════
// K4.6: Static Promotion
// ═══════════════════════════════════════════════════════════════════════

/// Registry of all compile-time allocations in a compilation unit.
#[derive(Debug, Clone, Default)]
pub struct ConstAllocRegistry {
    /// Named allocations: name → allocation.
    allocations: HashMap<String, ConstAllocation>,
    /// Auto-promoted temporaries (unnamed const expressions).
    promotions: Vec<ConstAllocation>,
    /// Target architecture info.
    target: TargetInfo,
    /// Next promotion ID.
    next_id: usize,
}

impl ConstAllocRegistry {
    /// Creates a new registry for the given target.
    pub fn new(target: TargetInfo) -> Self {
        Self {
            allocations: HashMap::new(),
            promotions: Vec::new(),
            target,
            next_id: 0,
        }
    }

    /// Registers a named const allocation.
    pub fn register(&mut self, name: &str, value: &ComptimeValue) -> &ConstAllocation {
        let alloc = serialize_const(name, value, &self.target);
        self.allocations.insert(name.to_string(), alloc);
        self.allocations.get(name).unwrap()
    }

    /// K4.6: Promotes a temporary const expression to static storage.
    ///
    /// Returns the auto-generated name for the promoted allocation.
    pub fn promote(&mut self, value: &ComptimeValue) -> String {
        let name = format!("__const_promoted_{}", self.next_id);
        self.next_id += 1;
        let alloc = serialize_const(&name, value, &self.target);
        self.promotions.push(alloc);
        name
    }

    /// Gets a named allocation.
    pub fn get(&self, name: &str) -> Option<&ConstAllocation> {
        self.allocations.get(name)
    }

    /// Total number of allocations (named + promoted).
    pub fn total_count(&self) -> usize {
        self.allocations.len() + self.promotions.len()
    }

    /// Total bytes across all allocations.
    pub fn total_bytes(&self) -> usize {
        self.allocations.values().map(|a| a.size()).sum::<usize>()
            + self.promotions.iter().map(|a| a.size()).sum::<usize>()
    }

    /// K4.8: Check for overly large allocations and return warnings.
    pub fn size_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let total = self.total_bytes();

        for alloc in self.allocations.values() {
            if alloc.is_large() {
                warnings.push(format!(
                    "const '{}' is {} bytes ({} — consider reducing size)",
                    alloc.name,
                    alloc.size(),
                    alloc.type_desc
                ));
            }
        }

        if total > WARN_THRESHOLD_BYTES {
            warnings.push(format!(
                "total const data is {} bytes (>{} threshold)",
                total, WARN_THRESHOLD_BYTES
            ));
        }

        warnings
    }

    /// All named allocations as an iterator.
    pub fn named_allocations(&self) -> impl Iterator<Item = (&str, &ConstAllocation)> {
        self.allocations.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// All promoted allocations.
    pub fn promoted_allocations(&self) -> &[ConstAllocation] {
        &self.promotions
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K4.7: Const Arena Allocator
// ═══════════════════════════════════════════════════════════════════════

/// A simple arena for compile-time allocations.
///
/// Groups all const data into a single contiguous buffer for efficient
/// `.rodata` emission. No runtime allocation needed for const data.
#[derive(Debug, Clone)]
pub struct ConstArena {
    /// The arena buffer.
    buffer: Vec<u8>,
    /// Allocation records: (name, offset, size).
    records: Vec<(String, usize, usize)>,
}

impl ConstArena {
    /// Creates a new empty arena.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            records: Vec::new(),
        }
    }

    /// Allocates space in the arena for a const allocation.
    /// Returns the byte offset of the allocation.
    pub fn alloc(&mut self, alloc: &ConstAllocation) -> usize {
        // Align the current position
        let align = alloc.align;
        let padding = (align - (self.buffer.len() % align)) % align;
        self.buffer.extend(std::iter::repeat_n(0u8, padding));

        let offset = self.buffer.len();
        self.buffer.extend_from_slice(&alloc.bytes);
        self.records
            .push((alloc.name.clone(), offset, alloc.bytes.len()));
        offset
    }

    /// Total arena size in bytes.
    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    /// Number of allocations.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// Gets the raw buffer for `.rodata` emission.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Looks up an allocation's offset by name.
    pub fn offset_of(&self, name: &str) -> Option<usize> {
        self.records
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, off, _)| *off)
    }
}

impl Default for ConstArena {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K4.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> TargetInfo {
        TargetInfo::x86_64()
    }

    // ── K4.1: Const array allocation ──

    #[test]
    fn k4_1_serialize_const_int_array() {
        let arr = ComptimeValue::Array(vec![
            ComptimeValue::Int(2),
            ComptimeValue::Int(3),
            ComptimeValue::Int(5),
        ]);
        let alloc = serialize_const("PRIMES", &arr, &target());
        assert_eq!(alloc.name, "PRIMES");
        assert_eq!(alloc.section, ".rodata");
        // 8 bytes count + 3 * 8 bytes = 32 bytes
        assert_eq!(alloc.size(), 8 + 3 * 8);
        assert_eq!(alloc.align, 8);
        assert!(alloc.type_desc.contains("[i64; 3]"));
    }

    #[test]
    fn k4_1_large_array_warning() {
        // Create array > 1MB
        let big = ComptimeValue::Array(vec![ComptimeValue::Int(0); 200_000]);
        let alloc = serialize_const("BIG", &big, &target());
        assert!(alloc.is_large());
    }

    // ── K4.2: Const string allocation ──

    #[test]
    fn k4_2_serialize_const_string() {
        let s = ComptimeValue::Str("hello".to_string());
        let alloc = serialize_const("GREETING", &s, &target());
        // 8 bytes length + 5 bytes utf8 = 13 bytes
        assert_eq!(alloc.size(), 8 + 5);
        assert_eq!(alloc.align, 8); // pointer-size aligned
        // Verify length prefix
        let len = u64::from_le_bytes(alloc.bytes[0..8].try_into().unwrap());
        assert_eq!(len, 5);
        // Verify string content
        assert_eq!(&alloc.bytes[8..13], b"hello");
    }

    // ── K4.3: Const struct allocation ──

    #[test]
    fn k4_3_serialize_const_struct() {
        let s = ComptimeValue::Struct {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), ComptimeValue::Float(1.0)),
                ("y".to_string(), ComptimeValue::Float(2.0)),
            ],
        };
        let alloc = serialize_const("ORIGIN", &s, &target());
        assert_eq!(alloc.size(), 16); // 2 * 8 bytes
        assert_eq!(alloc.type_desc, "Point");
        assert_eq!(alloc.align, 8);
    }

    // ── K4.4: Const HashMap ──

    #[test]
    fn k4_4_const_hashmap_lookup() {
        let map = ConstHashMap::new(
            "COLOR_MAP",
            vec![
                (
                    ComptimeValue::Str("red".into()),
                    ComptimeValue::Int(0xFF0000),
                ),
                (
                    ComptimeValue::Str("green".into()),
                    ComptimeValue::Int(0x00FF00),
                ),
                (
                    ComptimeValue::Str("blue".into()),
                    ComptimeValue::Int(0x0000FF),
                ),
            ],
        );

        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get(&ComptimeValue::Str("red".into())),
            Some(&ComptimeValue::Int(0xFF0000))
        );
        assert_eq!(
            map.get(&ComptimeValue::Str("blue".into())),
            Some(&ComptimeValue::Int(0x0000FF))
        );
        assert_eq!(map.get(&ComptimeValue::Str("yellow".into())), None);
    }

    #[test]
    fn k4_4_const_hashmap_int_keys() {
        let map = ConstHashMap::new(
            "STATUS_CODES",
            vec![
                (ComptimeValue::Int(200), ComptimeValue::Str("OK".into())),
                (
                    ComptimeValue::Int(404),
                    ComptimeValue::Str("Not Found".into()),
                ),
            ],
        );

        assert_eq!(
            map.get(&ComptimeValue::Int(200)),
            Some(&ComptimeValue::Str("OK".into()))
        );
    }

    // ── K4.5: Const slice ──

    #[test]
    fn k4_5_const_slice_repr() {
        let slice = ConstSlice {
            source_name: "PRIMES".to_string(),
            offset: 8, // skip count header
            length: 3,
            elem_type: "i64".to_string(),
        };
        assert_eq!(slice.length, 3);
        assert_eq!(slice.source_name, "PRIMES");
    }

    // ── K4.6: Static promotion ──

    #[test]
    fn k4_6_static_promotion() {
        let mut reg = ConstAllocRegistry::new(target());

        // Named const
        reg.register("X", &ComptimeValue::Int(42));

        // Promote temporary
        let name = reg.promote(&ComptimeValue::Array(vec![
            ComptimeValue::Int(1),
            ComptimeValue::Int(2),
        ]));
        assert!(name.starts_with("__const_promoted_"));

        assert_eq!(reg.total_count(), 2);
        assert!(reg.get("X").is_some());
    }

    // ── K4.7: Const arena allocator ──

    #[test]
    fn k4_7_arena_basic() {
        let mut arena = ConstArena::new();

        let alloc1 = serialize_const("A", &ComptimeValue::Int(42), &target());
        let off1 = arena.alloc(&alloc1);
        assert_eq!(off1, 0);

        let alloc2 = serialize_const("B", &ComptimeValue::Int(99), &target());
        let off2 = arena.alloc(&alloc2);
        assert_eq!(off2, 8); // aligned after first 8-byte int

        assert_eq!(arena.count(), 2);
        assert_eq!(arena.size(), 16);
        assert_eq!(arena.offset_of("A"), Some(0));
        assert_eq!(arena.offset_of("B"), Some(8));
    }

    #[test]
    fn k4_7_arena_alignment() {
        let mut arena = ConstArena::new();

        // Bool = 1 byte, align 1
        let a1 = serialize_const("FLAG", &ComptimeValue::Bool(true), &target());
        arena.alloc(&a1);
        assert_eq!(arena.size(), 1);

        // Int = 8 bytes, align 8 → needs 7 bytes padding
        let a2 = serialize_const("NUM", &ComptimeValue::Int(100), &target());
        let off = arena.alloc(&a2);
        assert_eq!(off, 8); // aligned to 8
        assert_eq!(arena.size(), 16);
    }

    // ── K4.8: Const size verification ──

    #[test]
    fn k4_8_size_warning_for_large_data() {
        let mut reg = ConstAllocRegistry::new(target());
        let big = ComptimeValue::Array(vec![ComptimeValue::Int(0); 200_000]);
        reg.register("HUGE", &big);

        let warnings = reg.size_warnings();
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("HUGE"));
    }

    #[test]
    fn k4_8_no_warning_for_small_data() {
        let mut reg = ConstAllocRegistry::new(target());
        reg.register("SMALL", &ComptimeValue::Int(42));

        let warnings = reg.size_warnings();
        assert!(warnings.is_empty());
    }

    // ── K4.9: Cross-compilation const ──

    #[test]
    fn k4_9_wasm32_pointer_size() {
        let wasm = TargetInfo::wasm32();
        let s = ComptimeValue::Str("hi".to_string());
        let alloc = serialize_const("S", &s, &wasm);
        // 4 bytes length (32-bit pointer) + 2 bytes = 6 bytes
        assert_eq!(alloc.size(), 4 + 2);
        let len = u32::from_le_bytes(alloc.bytes[0..4].try_into().unwrap());
        assert_eq!(len, 2);
    }

    #[test]
    fn k4_9_aarch64_same_as_x86() {
        let x86 = TargetInfo::x86_64();
        let arm = TargetInfo::aarch64();
        let val = ComptimeValue::Int(123);
        let a1 = serialize_const("V", &val, &x86);
        let a2 = serialize_const("V", &val, &arm);
        assert_eq!(a1.bytes, a2.bytes); // Same pointer size, same endianness
    }

    // ── K4.10: Integration test ──

    #[test]
    fn k4_10_full_pipeline_registry_to_arena() {
        let mut reg = ConstAllocRegistry::new(target());

        // Register several consts
        reg.register("PI", &ComptimeValue::Float(std::f64::consts::PI));
        reg.register("GREETING", &ComptimeValue::Str("Hello".into()));
        reg.register(
            "ORIGIN",
            &ComptimeValue::Struct {
                name: "Point".into(),
                fields: vec![
                    ("x".into(), ComptimeValue::Float(0.0)),
                    ("y".into(), ComptimeValue::Float(0.0)),
                ],
            },
        );

        // Promote a temporary
        let _prom = reg.promote(&ComptimeValue::Array(vec![
            ComptimeValue::Int(1),
            ComptimeValue::Int(2),
            ComptimeValue::Int(3),
        ]));

        assert_eq!(reg.total_count(), 4);
        assert!(reg.total_bytes() > 0);
        assert!(reg.size_warnings().is_empty());

        // Pack into arena
        let mut arena = ConstArena::new();
        for (_, alloc) in reg.named_allocations() {
            arena.alloc(alloc);
        }
        for alloc in reg.promoted_allocations() {
            arena.alloc(alloc);
        }

        assert_eq!(arena.count(), 4);
        assert!(arena.size() > 0);
        assert!(arena.as_bytes().len() == arena.size());
    }

    #[test]
    fn k4_10_serialize_tuple() {
        let t = ComptimeValue::Tuple(vec![
            ComptimeValue::Int(1),
            ComptimeValue::Bool(true),
            ComptimeValue::Float(3.14),
        ]);
        let alloc = serialize_const("TPL", &t, &target());
        // 8 (i64) + 1 (bool) + 8 (f64) = 17 bytes
        assert_eq!(alloc.size(), 17);
        assert!(alloc.type_desc.contains("i64"));
    }
}
