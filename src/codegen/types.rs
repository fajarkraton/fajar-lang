//! Type lowering: Fajar Lang types → Cranelift IR types.
//!
//! Maps the high-level Fajar type system to Cranelift's low-level
//! type representation for native code generation.

use cranelift_codegen::ir::Type as ClifType;
use cranelift_codegen::ir::types;

use crate::parser::ast::TypeExpr;

/// Lowers a Fajar Lang type expression to a Cranelift IR type.
///
/// Returns `None` for types that cannot be represented as a single
/// Cranelift value (e.g., strings, arrays, tensors, void).
pub fn lower_type(ty: &TypeExpr) -> Option<ClifType> {
    match ty {
        TypeExpr::Simple { name, .. } => lower_simple_type(name),
        TypeExpr::Reference { .. } | TypeExpr::Pointer { .. } => Some(types::I64),
        TypeExpr::Array { .. } => Some(types::I64), // Arrays passed/returned as pointers
        TypeExpr::Fn { .. } => Some(types::I64),    // Function pointers are addresses
        _ => None,
    }
}

/// Lowers a simple type name to a Cranelift type.
pub fn lower_simple_type(name: &str) -> Option<ClifType> {
    match name {
        "u1" | "u2" | "u3" | "u4" | "u5" | "u6" | "u7" => Some(types::I64),
        "i8" | "u8" => Some(types::I8),
        "i16" | "u16" => Some(types::I16),
        "i32" | "u32" => Some(types::I32),
        "i64" | "u64" | "isize" | "usize" | "int" => Some(types::I64),
        "i128" | "u128" => Some(types::I128),
        "f32" => Some(types::F32),
        "f64" | "float" => Some(types::F64),
        "bool" => Some(types::I8),
        "char" => Some(types::I32),
        "ptr" => Some(types::I64),
        "void" => None,
        _ => None,
    }
}

/// Returns the Cranelift type used for the default integer (`i64`).
pub fn default_int_type() -> ClifType {
    types::I64
}

/// Returns the Cranelift type used for the default float (`f64`).
pub fn default_float_type() -> ClifType {
    types::F64
}

/// Returns the Cranelift type used for booleans (`i8`).
pub fn bool_type() -> ClifType {
    types::I8
}

/// Returns the pointer-sized integer type for the target.
pub fn pointer_type() -> ClifType {
    types::I64
}

/// Returns true if the given Cranelift type is a floating-point type.
pub fn is_float(ty: ClifType) -> bool {
    ty == types::F32 || ty == types::F64
}

/// Returns the bit width if the type name is a bitfield type (u1-u7), else None.
pub fn bitfield_width(type_name: &str) -> Option<u8> {
    match type_name {
        "u1" => Some(1),
        "u2" => Some(2),
        "u3" => Some(3),
        "u4" => Some(4),
        "u5" => Some(5),
        "u6" => Some(6),
        "u7" => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;

    fn simple_ty(name: &str) -> TypeExpr {
        TypeExpr::Simple {
            name: name.to_string(),
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn lower_i64() {
        assert_eq!(lower_type(&simple_ty("i64")), Some(types::I64));
    }

    #[test]
    fn lower_f64() {
        assert_eq!(lower_type(&simple_ty("f64")), Some(types::F64));
    }

    #[test]
    fn lower_bool() {
        assert_eq!(lower_type(&simple_ty("bool")), Some(types::I8));
    }

    #[test]
    fn lower_void() {
        assert_eq!(lower_type(&simple_ty("void")), None);
    }

    #[test]
    fn lower_unknown_returns_none() {
        assert_eq!(lower_type(&simple_ty("Tensor")), None);
    }

    #[test]
    fn lower_fn_pointer_is_i64() {
        let ty = TypeExpr::Fn {
            params: vec![simple_ty("i64")],
            return_type: Box::new(simple_ty("i64")),
            span: Span::new(0, 0),
        };
        assert_eq!(lower_type(&ty), Some(types::I64));
    }
}
