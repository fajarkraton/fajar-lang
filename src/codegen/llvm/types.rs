//! Fajar Lang type → LLVM type mapping.
//!
//! Maps the Fajar Lang type system to LLVM IR types via inkwell.

use inkwell::context::Context;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};

/// Maps a Fajar Lang type name to an LLVM basic type.
///
/// # Type Mapping
///
/// | Fajar Type | LLVM Type |
/// |------------|-----------|
/// | bool       | i1        |
/// | i8         | i8        |
/// | i16        | i16       |
/// | i32        | i32       |
/// | i64        | i64       |
/// | i128       | i128      |
/// | u8-u128    | iN        |
/// | f32        | float     |
/// | f64        | double    |
/// | str        | { ptr, i64 } |
/// | void       | i64 (sentinel 0) |
pub fn fj_type_to_llvm<'ctx>(ctx: &'ctx Context, type_name: &str) -> BasicTypeEnum<'ctx> {
    match type_name {
        "bool" => ctx.bool_type().into(),
        "i8" | "u8" => ctx.i8_type().into(),
        "i16" | "u16" => ctx.i16_type().into(),
        "i32" | "u32" => ctx.i32_type().into(),
        "i64" | "u64" | "isize" | "usize" => ctx.i64_type().into(),
        "i128" | "u128" => ctx.i128_type().into(),
        "f32" => ctx.f32_type().into(),
        "f64" => ctx.f64_type().into(),
        "str" => {
            // String: { ptr, len }
            ctx.struct_type(
                &[
                    ctx.ptr_type(inkwell::AddressSpace::default()).into(),
                    ctx.i64_type().into(),
                ],
                false,
            )
            .into()
        }
        // Default: treat as i64 (opaque pointer / void sentinel)
        _ => ctx.i64_type().into(),
    }
}

/// Maps a Fajar Lang type name to an LLVM metadata type (for function signatures).
pub fn fj_type_to_metadata<'ctx>(
    ctx: &'ctx Context,
    type_name: &str,
) -> BasicMetadataTypeEnum<'ctx> {
    match type_name {
        "bool" => ctx.bool_type().into(),
        "i8" | "u8" => ctx.i8_type().into(),
        "i16" | "u16" => ctx.i16_type().into(),
        "i32" | "u32" => ctx.i32_type().into(),
        "i64" | "u64" | "isize" | "usize" => ctx.i64_type().into(),
        "i128" | "u128" => ctx.i128_type().into(),
        "f32" => ctx.f32_type().into(),
        "f64" => ctx.f64_type().into(),
        _ => ctx.i64_type().into(),
    }
}

/// Returns the default LLVM type for function return values (i64).
pub fn default_return_type<'ctx>(ctx: &'ctx Context) -> BasicTypeEnum<'ctx> {
    ctx.i64_type().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_mapping_integer_types() {
        let ctx = Context::create();
        assert!(fj_type_to_llvm(&ctx, "i64").is_int_type());
        assert!(fj_type_to_llvm(&ctx, "i32").is_int_type());
        assert!(fj_type_to_llvm(&ctx, "i8").is_int_type());
        assert!(fj_type_to_llvm(&ctx, "bool").is_int_type());
    }

    #[test]
    fn type_mapping_float_types() {
        let ctx = Context::create();
        assert!(fj_type_to_llvm(&ctx, "f64").is_float_type());
        assert!(fj_type_to_llvm(&ctx, "f32").is_float_type());
    }

    #[test]
    fn type_mapping_string_is_struct() {
        let ctx = Context::create();
        assert!(fj_type_to_llvm(&ctx, "str").is_struct_type());
    }

    #[test]
    fn type_mapping_unknown_defaults_to_i64() {
        let ctx = Context::create();
        let ty = fj_type_to_llvm(&ctx, "SomeStruct");
        assert!(ty.is_int_type());
        assert_eq!(ty.into_int_type().get_bit_width(), 64);
    }

    #[test]
    fn type_mapping_bool_is_i1() {
        let ctx = Context::create();
        let ty = fj_type_to_llvm(&ctx, "bool");
        assert_eq!(ty.into_int_type().get_bit_width(), 1);
    }

    #[test]
    fn type_mapping_metadata_matches_basic() {
        let ctx = Context::create();
        // metadata i64 should produce i64 metadata type
        let meta = fj_type_to_metadata(&ctx, "i64");
        assert!(matches!(meta, BasicMetadataTypeEnum::IntType(_)));
    }
}
