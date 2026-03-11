//! Resource handles — linear struct definitions for file handles, GPU buffers,
//! MIG partitions, must-use enforcement, LinearDrop trait, leak detection,
//! transfer semantics, and linearity propagation.

use std::collections::HashMap;
use std::fmt;

use super::checker::Linearity;

// ═══════════════════════════════════════════════════════════════════════
// S6.1 / S6.2 / S6.3: Resource Handle Types
// ═══════════════════════════════════════════════════════════════════════

/// A linear struct definition.
#[derive(Debug, Clone)]
pub struct LinearStruct {
    /// Struct name.
    pub name: String,
    /// Fields: (name, type_name, is_linear).
    pub fields: Vec<LinearField>,
    /// Overall linearity (computed from fields).
    pub linearity: Linearity,
}

/// A field in a linear struct.
#[derive(Debug, Clone)]
pub struct LinearField {
    /// Field name.
    pub name: String,
    /// Field type name.
    pub type_name: String,
    /// Whether this field is itself linear.
    pub is_linear: bool,
}

impl LinearStruct {
    /// Defines the `FileHandle` resource type.
    pub fn file_handle() -> Self {
        Self {
            name: "FileHandle".into(),
            fields: vec![LinearField {
                name: "fd".into(),
                type_name: "i32".into(),
                is_linear: false,
            }],
            linearity: Linearity::Linear,
        }
    }

    /// Defines the `GpuBuffer` resource type.
    pub fn gpu_buffer() -> Self {
        Self {
            name: "GpuBuffer".into(),
            fields: vec![
                LinearField {
                    name: "ptr".into(),
                    type_name: "*mut u8".into(),
                    is_linear: false,
                },
                LinearField {
                    name: "size".into(),
                    type_name: "usize".into(),
                    is_linear: false,
                },
                LinearField {
                    name: "device".into(),
                    type_name: "i32".into(),
                    is_linear: false,
                },
            ],
            linearity: Linearity::Linear,
        }
    }

    /// Defines the `MigPartition` resource type.
    pub fn mig_partition() -> Self {
        Self {
            name: "MigPartition".into(),
            fields: vec![
                LinearField {
                    name: "id".into(),
                    type_name: "u32".into(),
                    is_linear: false,
                },
                LinearField {
                    name: "gpu".into(),
                    type_name: "i32".into(),
                    is_linear: false,
                },
            ],
            linearity: Linearity::Linear,
        }
    }
}

impl fmt::Display for LinearStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "linear struct {} {{ ", self.name)?;
        let fields_str: Vec<String> = self
            .fields
            .iter()
            .map(|fld| format!("{}: {}", fld.name, fld.type_name))
            .collect();
        write!(f, "{}", fields_str.join(", "))?;
        write!(f, " }}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.4: Must-Use Enforcement
// ═══════════════════════════════════════════════════════════════════════

/// Checks whether a function call returning a linear type has its result used.
pub fn check_must_use(
    fn_name: &str,
    return_type: &str,
    linear_types: &HashMap<String, LinearStruct>,
    result_bound: bool,
) -> Option<String> {
    if !result_bound && linear_types.contains_key(return_type) {
        Some(format!(
            "result of `{fn_name}()` returning linear type `{return_type}` must be used"
        ))
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.5: Linear Drop Trait
// ═══════════════════════════════════════════════════════════════════════

/// The `LinearDrop` trait — must be implemented by linear types to define
/// their finalization behavior.
#[derive(Debug, Clone)]
pub struct LinearDropImpl {
    /// Type that implements LinearDrop.
    pub type_name: String,
    /// Finalization method body description.
    pub finalize_desc: String,
}

/// Registry of LinearDrop implementations.
#[derive(Debug, Clone, Default)]
pub struct LinearDropRegistry {
    /// Implementations keyed by type name.
    impls: HashMap<String, LinearDropImpl>,
}

impl LinearDropRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a LinearDrop implementation.
    pub fn register(&mut self, type_name: &str, finalize_desc: &str) {
        self.impls.insert(
            type_name.into(),
            LinearDropImpl {
                type_name: type_name.into(),
                finalize_desc: finalize_desc.into(),
            },
        );
    }

    /// Checks whether a type has a LinearDrop implementation.
    pub fn has_impl(&self, type_name: &str) -> bool {
        self.impls.contains_key(type_name)
    }

    /// Gets the LinearDrop implementation for a type.
    pub fn get_impl(&self, type_name: &str) -> Option<&LinearDropImpl> {
        self.impls.get(type_name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.6: Resource Leak Detection
// ═══════════════════════════════════════════════════════════════════════

/// Result of leak detection at function exit.
#[derive(Debug, Clone)]
pub struct LeakReport {
    /// Names and types of leaked linear values.
    pub leaked: Vec<(String, String)>,
}

impl LeakReport {
    /// Returns `true` if no leaks were detected.
    pub fn is_clean(&self) -> bool {
        self.leaked.is_empty()
    }
}

/// Detects leaked linear values at function exit.
pub fn detect_leaks(bindings: &HashMap<String, super::checker::LinearBinding>) -> LeakReport {
    let leaked: Vec<(String, String)> = bindings
        .values()
        .filter(|b| b.linearity.is_linear() && !b.consumed && !b.returned)
        .map(|b| (b.name.clone(), b.type_name.clone()))
        .collect();
    LeakReport { leaked }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.7: Transfer Semantics
// ═══════════════════════════════════════════════════════════════════════

/// Transfer mode for linear values passed to functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    /// Ownership transferred — caller loses access.
    Move,
    /// Temporarily borrowed — ownership retained.
    Borrow,
    /// Consumed — caller loses access and resource is finalized.
    Consume,
}

/// Determines the transfer mode for a linear value being passed to a function.
pub fn determine_transfer(param_is_consume: bool, param_is_ref: bool) -> TransferMode {
    if param_is_consume {
        TransferMode::Consume
    } else if param_is_ref {
        TransferMode::Borrow
    } else {
        TransferMode::Move
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.8 / S6.9: Linearity Propagation
// ═══════════════════════════════════════════════════════════════════════

/// Computes the linearity of a struct from its fields.
///
/// If any field is linear, the entire struct is linear.
pub fn compute_struct_linearity(
    fields: &[LinearField],
    linear_types: &HashMap<String, LinearStruct>,
) -> Linearity {
    for field in fields {
        if field.is_linear || linear_types.contains_key(&field.type_name) {
            return Linearity::Linear;
        }
    }
    Linearity::Affine
}

/// Computes the linearity of an enum from its variants' data types.
///
/// If any variant carries linear data, the enum is linear.
pub fn compute_enum_linearity(
    variant_types: &[Option<String>],
    linear_types: &HashMap<String, LinearStruct>,
) -> Linearity {
    for vt in variant_types.iter().flatten() {
        if linear_types.contains_key(vt) {
            return Linearity::Linear;
        }
    }
    Linearity::Affine
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S6.1 — FileHandle
    #[test]
    fn s6_1_file_handle_definition() {
        let fh = LinearStruct::file_handle();
        assert_eq!(fh.name, "FileHandle");
        assert_eq!(fh.linearity, Linearity::Linear);
        assert_eq!(fh.fields.len(), 1);
        assert_eq!(fh.fields[0].name, "fd");
    }

    #[test]
    fn s6_1_file_handle_display() {
        let fh = LinearStruct::file_handle();
        let s = fh.to_string();
        assert!(s.contains("linear struct FileHandle"));
        assert!(s.contains("fd: i32"));
    }

    // S6.2 — GpuBuffer
    #[test]
    fn s6_2_gpu_buffer_definition() {
        let gb = LinearStruct::gpu_buffer();
        assert_eq!(gb.name, "GpuBuffer");
        assert_eq!(gb.fields.len(), 3);
        assert_eq!(gb.linearity, Linearity::Linear);
    }

    // S6.3 — MigPartition
    #[test]
    fn s6_3_mig_partition_definition() {
        let mp = LinearStruct::mig_partition();
        assert_eq!(mp.name, "MigPartition");
        assert_eq!(mp.fields.len(), 2);
    }

    // S6.4 — Must-Use
    #[test]
    fn s6_4_must_use_unbound() {
        let mut types = HashMap::new();
        types.insert("FileHandle".into(), LinearStruct::file_handle());
        let result = check_must_use("open", "FileHandle", &types, false);
        assert!(result.is_some());
        assert!(result.unwrap().contains("must be used"));
    }

    #[test]
    fn s6_4_must_use_bound() {
        let mut types = HashMap::new();
        types.insert("FileHandle".into(), LinearStruct::file_handle());
        assert!(check_must_use("open", "FileHandle", &types, true).is_none());
    }

    #[test]
    fn s6_4_must_use_non_linear() {
        let types = HashMap::new();
        assert!(check_must_use("foo", "i32", &types, false).is_none());
    }

    // S6.5 — LinearDrop
    #[test]
    fn s6_5_linear_drop_registry() {
        let mut reg = LinearDropRegistry::new();
        reg.register("FileHandle", "close(self.fd)");
        assert!(reg.has_impl("FileHandle"));
        assert!(!reg.has_impl("GpuBuffer"));
        let imp = reg.get_impl("FileHandle").unwrap();
        assert_eq!(imp.finalize_desc, "close(self.fd)");
    }

    // S6.6 — Leak Detection
    #[test]
    fn s6_6_no_leaks() {
        let mut bindings = HashMap::new();
        let mut b =
            super::super::checker::LinearBinding::new("h", "FileHandle", Linearity::Linear, 0);
        b.mark_consumed();
        bindings.insert("h".into(), b);
        let report = detect_leaks(&bindings);
        assert!(report.is_clean());
    }

    #[test]
    fn s6_6_leak_detected() {
        let mut bindings = HashMap::new();
        bindings.insert(
            "h".into(),
            super::super::checker::LinearBinding::new("h", "FileHandle", Linearity::Linear, 0),
        );
        let report = detect_leaks(&bindings);
        assert!(!report.is_clean());
        assert_eq!(report.leaked.len(), 1);
        assert_eq!(report.leaked[0].0, "h");
    }

    // S6.7 — Transfer
    #[test]
    fn s6_7_transfer_modes() {
        assert_eq!(determine_transfer(true, false), TransferMode::Consume);
        assert_eq!(determine_transfer(false, true), TransferMode::Borrow);
        assert_eq!(determine_transfer(false, false), TransferMode::Move);
    }

    // S6.8 — Struct Propagation
    #[test]
    fn s6_8_struct_with_linear_field() {
        let mut types = HashMap::new();
        types.insert("FileHandle".into(), LinearStruct::file_handle());
        let fields = vec![
            LinearField {
                name: "handle".into(),
                type_name: "FileHandle".into(),
                is_linear: false,
            },
            LinearField {
                name: "name".into(),
                type_name: "String".into(),
                is_linear: false,
            },
        ];
        assert_eq!(compute_struct_linearity(&fields, &types), Linearity::Linear);
    }

    #[test]
    fn s6_8_struct_no_linear_fields() {
        let types = HashMap::new();
        let fields = vec![LinearField {
            name: "x".into(),
            type_name: "i32".into(),
            is_linear: false,
        }];
        assert_eq!(compute_struct_linearity(&fields, &types), Linearity::Affine);
    }

    // S6.9 — Enum Propagation
    #[test]
    fn s6_9_enum_with_linear_variant() {
        let mut types = HashMap::new();
        types.insert("FileHandle".into(), LinearStruct::file_handle());
        let variants = vec![Some("FileHandle".into()), None];
        assert_eq!(compute_enum_linearity(&variants, &types), Linearity::Linear);
    }

    #[test]
    fn s6_9_enum_no_linear_variants() {
        let types = HashMap::new();
        let variants = vec![Some("i32".into()), None];
        assert_eq!(compute_enum_linearity(&variants, &types), Linearity::Affine);
    }

    // S6.10 — Additional
    #[test]
    fn s6_10_linear_struct_explicit_linear_field() {
        let types = HashMap::new();
        let fields = vec![LinearField {
            name: "token".into(),
            type_name: "AuthToken".into(),
            is_linear: true,
        }];
        assert_eq!(compute_struct_linearity(&fields, &types), Linearity::Linear);
    }
}
