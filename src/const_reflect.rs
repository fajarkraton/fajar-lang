//! Compile-time reflection — intrinsics for querying type metadata at compile time.
//!
//! # Intrinsics
//!
//! - `type_name::<T>()` → type name as const string
//! - `size_of::<T>()` → byte size
//! - `align_of::<T>()` → alignment
//! - `field_count::<T>()` → struct field count
//! - `field_names::<T>()` → field names as const array
//! - `field_types::<T>()` → field type names as const array
//! - `variant_count::<T>()` → enum variant count
//! - `variant_names::<T>()` → variant names as const array
//! - `has_trait::<T, Trait>()` → bool

use std::collections::HashMap;

use crate::analyzer::comptime::ComptimeValue;

// ═══════════════════════════════════════════════════════════════════════
// Type Metadata Registry
// ═══════════════════════════════════════════════════════════════════════

/// Metadata for a single type, queryable at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeMeta {
    /// Type name (e.g., `"i64"`, `"Point"`, `"Option"`).
    pub name: String,
    /// Size in bytes (for the default target).
    pub size: usize,
    /// Alignment in bytes.
    pub align: usize,
    /// Kind of type.
    pub kind: TypeMetaKind,
}

/// The kind of type — determines which reflection queries are valid.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeMetaKind {
    /// A primitive type (i8-i128, u8-u128, f32, f64, bool, char, str).
    Primitive,
    /// A struct with named fields.
    Struct {
        fields: Vec<FieldMeta>,
    },
    /// An enum with named variants.
    Enum {
        variants: Vec<VariantMeta>,
    },
    /// A tuple type.
    Tuple {
        elements: Vec<String>,
    },
    /// An array type.
    Array {
        elem_type: String,
        length: Option<usize>,
    },
}

/// Metadata for a struct field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMeta {
    /// Field name.
    pub name: String,
    /// Field type name.
    pub type_name: String,
    /// Byte offset within the struct.
    pub offset: usize,
}

/// Metadata for an enum variant.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantMeta {
    /// Variant name.
    pub name: String,
    /// Payload type (if any).
    pub payload: Option<String>,
    /// Discriminant value.
    pub discriminant: usize,
}

/// Registry of all type metadata, built during compilation.
#[derive(Debug, Clone, Default)]
pub struct TypeMetaRegistry {
    /// Type name → metadata.
    types: HashMap<String, TypeMeta>,
    /// Trait implementations: (type_name, trait_name).
    trait_impls: Vec<(String, String)>,
}

impl TypeMetaRegistry {
    /// Creates a new registry with built-in primitive types.
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.register_builtins();
        reg
    }

    /// Registers metadata for a type.
    pub fn register(&mut self, meta: TypeMeta) {
        self.types.insert(meta.name.clone(), meta);
    }

    /// Records a trait implementation.
    pub fn register_trait_impl(&mut self, type_name: &str, trait_name: &str) {
        self.trait_impls
            .push((type_name.to_string(), trait_name.to_string()));
    }

    /// Gets metadata for a type.
    pub fn get(&self, name: &str) -> Option<&TypeMeta> {
        self.types.get(name)
    }

    fn register_builtins(&mut self) {
        let primitives = [
            ("i8", 1, 1), ("i16", 2, 2), ("i32", 4, 4), ("i64", 8, 8), ("i128", 16, 16),
            ("isize", 8, 8),
            ("u8", 1, 1), ("u16", 2, 2), ("u32", 4, 4), ("u64", 8, 8), ("u128", 16, 16),
            ("usize", 8, 8),
            ("f32", 4, 4), ("f64", 8, 8),
            ("bool", 1, 1), ("char", 4, 4),
        ];
        for (name, size, align) in &primitives {
            self.register(TypeMeta {
                name: name.to_string(),
                size: *size,
                align: *align,
                kind: TypeMetaKind::Primitive,
            });
        }
        // str is a special case — size is dynamic, but we register it with size 0
        self.register(TypeMeta {
            name: "str".to_string(),
            size: 0,
            align: 1,
            kind: TypeMetaKind::Primitive,
        });

        // Register standard trait impls for primitives
        let numeric = ["i8", "i16", "i32", "i64", "i128", "isize",
                       "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64"];
        for ty in &numeric {
            for tr in &["Display", "Debug", "Clone", "Copy", "PartialEq", "PartialOrd"] {
                self.register_trait_impl(ty, tr);
            }
        }
        for tr in &["Display", "Debug", "Clone", "Copy", "PartialEq"] {
            self.register_trait_impl("bool", tr);
            self.register_trait_impl("char", tr);
        }
        self.register_trait_impl("str", "Display");
        self.register_trait_impl("str", "Debug");
        self.register_trait_impl("str", "PartialEq");
    }

    /// Register a struct type from field names and types.
    pub fn register_struct(
        &mut self,
        name: &str,
        fields: &[(&str, &str)],
    ) {
        let mut offset = 0;
        let mut field_metas = Vec::new();
        let mut max_align = 1;

        for (fname, ftype) in fields {
            let (fsize, falign) = self.size_align_of(ftype);
            // Align offset
            let padding = (falign - (offset % falign)) % falign;
            offset += padding;

            field_metas.push(FieldMeta {
                name: fname.to_string(),
                type_name: ftype.to_string(),
                offset,
            });
            offset += fsize;
            if falign > max_align {
                max_align = falign;
            }
        }

        // Final padding for struct alignment
        let padding = (max_align - (offset % max_align)) % max_align;
        offset += padding;

        self.register(TypeMeta {
            name: name.to_string(),
            size: offset,
            align: max_align,
            kind: TypeMetaKind::Struct {
                fields: field_metas,
            },
        });
    }

    /// Register an enum type from variant names and optional payloads.
    pub fn register_enum(
        &mut self,
        name: &str,
        variants: &[(&str, Option<&str>)],
    ) {
        let variant_metas: Vec<VariantMeta> = variants
            .iter()
            .enumerate()
            .map(|(i, (vname, payload))| VariantMeta {
                name: vname.to_string(),
                payload: payload.map(|p| p.to_string()),
                discriminant: i,
            })
            .collect();

        // Size = discriminant (8 bytes) + max payload size
        let max_payload = variants
            .iter()
            .filter_map(|(_, p)| p.map(|pt| self.size_align_of(pt).0))
            .max()
            .unwrap_or(0);

        self.register(TypeMeta {
            name: name.to_string(),
            size: 8 + max_payload, // discriminant + payload
            align: 8,
            kind: TypeMetaKind::Enum {
                variants: variant_metas,
            },
        });
    }

    /// Get size and alignment for a type name.
    fn size_align_of(&self, type_name: &str) -> (usize, usize) {
        self.types
            .get(type_name)
            .map(|m| (m.size, m.align))
            .unwrap_or((8, 8)) // Default to pointer-sized
    }

    // ═══════════════════════════════════════════════════════════════════
    // K5.1-K5.9: Reflection intrinsics
    // ═══════════════════════════════════════════════════════════════════

    /// K5.1: `type_name::<T>()` → const string.
    pub fn type_name(&self, type_name: &str) -> ComptimeValue {
        ComptimeValue::Str(type_name.to_string())
    }

    /// K5.2: `size_of::<T>()` → byte size.
    pub fn size_of(&self, type_name: &str) -> ComptimeValue {
        let size = self.types.get(type_name).map(|m| m.size).unwrap_or(0);
        ComptimeValue::Int(size as i64)
    }

    /// K5.3: `align_of::<T>()` → alignment.
    pub fn align_of(&self, type_name: &str) -> ComptimeValue {
        let align = self.types.get(type_name).map(|m| m.align).unwrap_or(1);
        ComptimeValue::Int(align as i64)
    }

    /// K5.4: `field_count::<T>()` → struct field count.
    pub fn field_count(&self, type_name: &str) -> ComptimeValue {
        let count = match self.types.get(type_name).map(|m| &m.kind) {
            Some(TypeMetaKind::Struct { fields }) => fields.len(),
            _ => 0,
        };
        ComptimeValue::Int(count as i64)
    }

    /// K5.5: `field_names::<T>()` → field names as const array.
    pub fn field_names(&self, type_name: &str) -> ComptimeValue {
        let names = match self.types.get(type_name).map(|m| &m.kind) {
            Some(TypeMetaKind::Struct { fields }) => fields
                .iter()
                .map(|f| ComptimeValue::Str(f.name.clone()))
                .collect(),
            _ => vec![],
        };
        ComptimeValue::Array(names)
    }

    /// K5.6: `field_types::<T>()` → field type names as const array.
    pub fn field_types(&self, type_name: &str) -> ComptimeValue {
        let types = match self.types.get(type_name).map(|m| &m.kind) {
            Some(TypeMetaKind::Struct { fields }) => fields
                .iter()
                .map(|f| ComptimeValue::Str(f.type_name.clone()))
                .collect(),
            _ => vec![],
        };
        ComptimeValue::Array(types)
    }

    /// K5.7: `variant_count::<T>()` → enum variant count.
    pub fn variant_count(&self, type_name: &str) -> ComptimeValue {
        let count = match self.types.get(type_name).map(|m| &m.kind) {
            Some(TypeMetaKind::Enum { variants }) => variants.len(),
            _ => 0,
        };
        ComptimeValue::Int(count as i64)
    }

    /// K5.8: `variant_names::<T>()` → variant names as const array.
    pub fn variant_names(&self, type_name: &str) -> ComptimeValue {
        let names = match self.types.get(type_name).map(|m| &m.kind) {
            Some(TypeMetaKind::Enum { variants }) => variants
                .iter()
                .map(|v| ComptimeValue::Str(v.name.clone()))
                .collect(),
            _ => vec![],
        };
        ComptimeValue::Array(names)
    }

    /// K5.9: `has_trait::<T, Trait>()` → bool.
    pub fn has_trait(&self, type_name: &str, trait_name: &str) -> ComptimeValue {
        let has = self
            .trait_impls
            .iter()
            .any(|(t, tr)| t == type_name && tr == trait_name);
        ComptimeValue::Bool(has)
    }

    /// Dispatch a reflection intrinsic by name.
    ///
    /// Returns `None` if the name is not a known reflection intrinsic.
    pub fn eval_intrinsic(
        &self,
        intrinsic_name: &str,
        type_arg: &str,
        trait_arg: Option<&str>,
    ) -> Option<ComptimeValue> {
        match intrinsic_name {
            "type_name" => Some(self.type_name(type_arg)),
            "size_of" => Some(self.size_of(type_arg)),
            "align_of" => Some(self.align_of(type_arg)),
            "field_count" => Some(self.field_count(type_arg)),
            "field_names" => Some(self.field_names(type_arg)),
            "field_types" => Some(self.field_types(type_arg)),
            "variant_count" => Some(self.variant_count(type_arg)),
            "variant_names" => Some(self.variant_names(type_arg)),
            "has_trait" => {
                let tr = trait_arg.unwrap_or("");
                Some(self.has_trait(type_arg, tr))
            }
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K5.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn reg_with_point() -> TypeMetaRegistry {
        let mut reg = TypeMetaRegistry::new();
        reg.register_struct("Point", &[("x", "f64"), ("y", "f64")]);
        reg.register_trait_impl("Point", "Display");
        reg
    }

    fn reg_with_option() -> TypeMetaRegistry {
        let mut reg = TypeMetaRegistry::new();
        reg.register_enum("Option", &[("Some", Some("i64")), ("None", None)]);
        reg
    }

    // ── K5.1: type_name ──

    #[test]
    fn k5_1_type_name_primitive() {
        let reg = TypeMetaRegistry::new();
        assert_eq!(reg.type_name("i32"), ComptimeValue::Str("i32".into()));
    }

    #[test]
    fn k5_1_type_name_struct() {
        let reg = reg_with_point();
        assert_eq!(reg.type_name("Point"), ComptimeValue::Str("Point".into()));
    }

    // ── K5.2: size_of ──

    #[test]
    fn k5_2_size_of_primitives() {
        let reg = TypeMetaRegistry::new();
        assert_eq!(reg.size_of("i8"), ComptimeValue::Int(1));
        assert_eq!(reg.size_of("i16"), ComptimeValue::Int(2));
        assert_eq!(reg.size_of("i32"), ComptimeValue::Int(4));
        assert_eq!(reg.size_of("i64"), ComptimeValue::Int(8));
        assert_eq!(reg.size_of("f32"), ComptimeValue::Int(4));
        assert_eq!(reg.size_of("f64"), ComptimeValue::Int(8));
        assert_eq!(reg.size_of("bool"), ComptimeValue::Int(1));
        assert_eq!(reg.size_of("char"), ComptimeValue::Int(4));
    }

    #[test]
    fn k5_2_size_of_struct() {
        let reg = reg_with_point();
        // Point { x: f64, y: f64 } = 16 bytes
        assert_eq!(reg.size_of("Point"), ComptimeValue::Int(16));
    }

    // ── K5.3: align_of ──

    #[test]
    fn k5_3_align_of() {
        let reg = TypeMetaRegistry::new();
        assert_eq!(reg.align_of("i8"), ComptimeValue::Int(1));
        assert_eq!(reg.align_of("f64"), ComptimeValue::Int(8));
        assert_eq!(reg.align_of("i128"), ComptimeValue::Int(16));
    }

    // ── K5.4: field_count ──

    #[test]
    fn k5_4_field_count_struct() {
        let reg = reg_with_point();
        assert_eq!(reg.field_count("Point"), ComptimeValue::Int(2));
    }

    #[test]
    fn k5_4_field_count_non_struct() {
        let reg = TypeMetaRegistry::new();
        assert_eq!(reg.field_count("i64"), ComptimeValue::Int(0));
    }

    // ── K5.5: field_names ──

    #[test]
    fn k5_5_field_names() {
        let reg = reg_with_point();
        assert_eq!(
            reg.field_names("Point"),
            ComptimeValue::Array(vec![
                ComptimeValue::Str("x".into()),
                ComptimeValue::Str("y".into()),
            ])
        );
    }

    // ── K5.6: field_types ──

    #[test]
    fn k5_6_field_types() {
        let reg = reg_with_point();
        assert_eq!(
            reg.field_types("Point"),
            ComptimeValue::Array(vec![
                ComptimeValue::Str("f64".into()),
                ComptimeValue::Str("f64".into()),
            ])
        );
    }

    // ── K5.7: variant_count ──

    #[test]
    fn k5_7_variant_count() {
        let reg = reg_with_option();
        assert_eq!(reg.variant_count("Option"), ComptimeValue::Int(2));
    }

    #[test]
    fn k5_7_variant_count_non_enum() {
        let reg = TypeMetaRegistry::new();
        assert_eq!(reg.variant_count("i64"), ComptimeValue::Int(0));
    }

    // ── K5.8: variant_names ──

    #[test]
    fn k5_8_variant_names() {
        let reg = reg_with_option();
        assert_eq!(
            reg.variant_names("Option"),
            ComptimeValue::Array(vec![
                ComptimeValue::Str("Some".into()),
                ComptimeValue::Str("None".into()),
            ])
        );
    }

    // ── K5.9: has_trait ──

    #[test]
    fn k5_9_has_trait_primitive() {
        let reg = TypeMetaRegistry::new();
        assert_eq!(reg.has_trait("i64", "Display"), ComptimeValue::Bool(true));
        assert_eq!(reg.has_trait("i64", "Clone"), ComptimeValue::Bool(true));
        assert_eq!(reg.has_trait("i64", "Iterator"), ComptimeValue::Bool(false));
    }

    #[test]
    fn k5_9_has_trait_struct() {
        let reg = reg_with_point();
        assert_eq!(reg.has_trait("Point", "Display"), ComptimeValue::Bool(true));
        assert_eq!(reg.has_trait("Point", "Clone"), ComptimeValue::Bool(false));
    }

    // ── K5.10: Integration via eval_intrinsic ──

    #[test]
    fn k5_10_eval_intrinsic_dispatch() {
        let reg = reg_with_point();

        assert_eq!(
            reg.eval_intrinsic("type_name", "Point", None),
            Some(ComptimeValue::Str("Point".into()))
        );
        assert_eq!(
            reg.eval_intrinsic("size_of", "Point", None),
            Some(ComptimeValue::Int(16))
        );
        assert_eq!(
            reg.eval_intrinsic("field_count", "Point", None),
            Some(ComptimeValue::Int(2))
        );
        assert_eq!(
            reg.eval_intrinsic("has_trait", "Point", Some("Display")),
            Some(ComptimeValue::Bool(true))
        );
        assert_eq!(reg.eval_intrinsic("unknown_fn", "Point", None), None);
    }

    #[test]
    fn k5_10_struct_field_offsets() {
        let mut reg = TypeMetaRegistry::new();
        // Mixed struct: { a: bool, b: i64, c: i16 }
        reg.register_struct("Mixed", &[("a", "bool"), ("b", "i64"), ("c", "i16")]);

        let meta = reg.get("Mixed").unwrap();
        if let TypeMetaKind::Struct { fields } = &meta.kind {
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[0].offset, 0); // bool at offset 0

            assert_eq!(fields[1].name, "b");
            assert_eq!(fields[1].offset, 8); // i64 at offset 8 (aligned)

            assert_eq!(fields[2].name, "c");
            assert_eq!(fields[2].offset, 16); // i16 at offset 16
        } else {
            panic!("expected struct kind");
        }
        // Size = 16 (a:1 + pad:7 + b:8) + 2 (c) + pad:6 = 24 bytes (aligned to 8)
        assert_eq!(meta.size, 24);
    }

    #[test]
    fn k5_10_enum_size() {
        let reg = reg_with_option();
        let meta = reg.get("Option").unwrap();
        // Option = discriminant(8) + max_payload(i64=8) = 16
        assert_eq!(meta.size, 16);
    }
}
