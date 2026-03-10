//! ABI (Application Binary Interface) definitions for native codegen.
//!
//! Defines calling conventions, value representation, and function
//! signature construction for the Cranelift backend.

use cranelift_codegen::ir::{AbiParam, Signature};
use cranelift_codegen::isa::CallConv;

use super::types as clif_types;
use super::CodegenError;
use crate::parser::ast::{Param, TypeExpr};

/// Builds a Cranelift function signature from Fajar Lang function parameters.
pub fn build_signature(
    call_conv: CallConv,
    params: &[Param],
    has_return: bool,
) -> Result<Signature, CodegenError> {
    build_signature_with_return_type(call_conv, params, has_return, None)
}

/// Builds a Cranelift function signature with an explicit return type.
///
/// If `return_type` is `Some`, it is used for the return value. Otherwise,
/// the default integer type (`i64`) is used.
pub fn build_signature_with_return_type(
    call_conv: CallConv,
    params: &[Param],
    has_return: bool,
    return_type: Option<cranelift_codegen::ir::Type>,
) -> Result<Signature, CodegenError> {
    let mut sig = Signature::new(call_conv);

    for param in params {
        let param_type =
            clif_types::lower_type(&param.ty).unwrap_or(clif_types::default_int_type());
        sig.params.push(AbiParam::new(param_type));
        // String params need a second parameter for the length
        if is_str_type(&param.ty) {
            sig.params
                .push(AbiParam::new(clif_types::default_int_type()));
        }
    }

    if has_return {
        let ret_type = return_type.unwrap_or(clif_types::default_int_type());
        sig.returns.push(AbiParam::new(ret_type));
    }

    Ok(sig)
}

/// Returns true if a type expression refers to the `str` type.
fn is_str_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Simple { name, .. } if name == "str")
}

/// Returns true if a parameter list has any string parameters.
pub fn has_str_params(params: &[Param]) -> bool {
    params.iter().any(|p| is_str_type(&p.ty))
}

/// Builds a signature for the `main()` function (no params, no return).
pub fn main_signature(call_conv: CallConv) -> Signature {
    Signature::new(call_conv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_sig_has_no_params_or_returns() {
        let sig = main_signature(CallConv::SystemV);
        assert!(sig.params.is_empty());
        assert!(sig.returns.is_empty());
    }
}
