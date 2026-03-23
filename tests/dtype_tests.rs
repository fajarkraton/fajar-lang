//! Multi-DType tensor tests for Fajar Lang.
//!
//! Verifies DType enum, conversions, precision simulation,
//! quantization, and dtype metadata.
//! Sprint 10 of Master Implementation Plan v7.0.

use fajar_lang::runtime::ml::mixed_precision::DType;

// ════════════════════════════════════════════════════════════════════════
// 1. DType enum basics
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dtype_f64() {
    assert_eq!(DType::F64.name(), "f64");
    assert_eq!(DType::F64.size_bytes(), 8);
    assert!(DType::F64.is_float());
    assert!(!DType::F64.is_int());
}

#[test]
fn dtype_f32() {
    assert_eq!(DType::F32.name(), "f32");
    assert_eq!(DType::F32.size_bytes(), 4);
    assert!(DType::F32.is_float());
}

#[test]
fn dtype_f16() {
    assert_eq!(DType::F16.name(), "f16");
    assert_eq!(DType::F16.size_bytes(), 2);
    assert!(DType::F16.is_float());
}

#[test]
fn dtype_bf16() {
    assert_eq!(DType::BF16.name(), "bf16");
    assert_eq!(DType::BF16.size_bytes(), 2);
    assert!(DType::BF16.is_float());
}

#[test]
fn dtype_i8() {
    assert_eq!(DType::I8.name(), "i8");
    assert_eq!(DType::I8.size_bytes(), 1);
    assert!(DType::I8.is_int());
    assert!(DType::I8.is_quantized());
}

#[test]
fn dtype_u8() {
    assert_eq!(DType::U8.name(), "u8");
    assert_eq!(DType::U8.size_bytes(), 1);
    assert!(DType::U8.is_int());
    assert!(DType::U8.is_quantized());
}

#[test]
fn dtype_i32() {
    assert_eq!(DType::I32.name(), "i32");
    assert_eq!(DType::I32.size_bytes(), 4);
    assert!(DType::I32.is_int());
    assert!(!DType::I32.is_quantized());
}

#[test]
fn dtype_i64() {
    assert_eq!(DType::I64.name(), "i64");
    assert_eq!(DType::I64.size_bytes(), 8);
    assert!(DType::I64.is_int());
}

#[test]
fn dtype_bool() {
    assert_eq!(DType::Bool.name(), "bool");
    assert_eq!(DType::Bool.size_bytes(), 1);
    assert!(!DType::Bool.is_float());
    assert!(!DType::Bool.is_int());
}

// ════════════════════════════════════════════════════════════════════════
// 2. DType parsing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_all_dtypes() {
    assert_eq!(DType::from_name("f64"), Some(DType::F64));
    assert_eq!(DType::from_name("f32"), Some(DType::F32));
    assert_eq!(DType::from_name("f16"), Some(DType::F16));
    assert_eq!(DType::from_name("bf16"), Some(DType::BF16));
    assert_eq!(DType::from_name("i8"), Some(DType::I8));
    assert_eq!(DType::from_name("u8"), Some(DType::U8));
    assert_eq!(DType::from_name("i32"), Some(DType::I32));
    assert_eq!(DType::from_name("i64"), Some(DType::I64));
    assert_eq!(DType::from_name("bool"), Some(DType::Bool));
}

#[test]
fn parse_unknown_dtype() {
    assert_eq!(DType::from_name("f128"), None);
    assert_eq!(DType::from_name(""), None);
    assert_eq!(DType::from_name("int"), None);
}

// ════════════════════════════════════════════════════════════════════════
// 3. DType display
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dtype_display() {
    assert_eq!(format!("{}", DType::F64), "f64");
    assert_eq!(format!("{}", DType::F16), "f16");
    assert_eq!(format!("{}", DType::BF16), "bf16");
    assert_eq!(format!("{}", DType::I8), "i8");
    assert_eq!(format!("{}", DType::Bool), "bool");
}

// ════════════════════════════════════════════════════════════════════════
// 4. DType value ranges
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dtype_f16_range() {
    assert!((DType::F16.max_value() - 65504.0).abs() < 1.0);
    assert!((DType::F16.min_value() + 65504.0).abs() < 1.0);
}

#[test]
fn dtype_i8_range() {
    assert_eq!(DType::I8.min_value(), -128.0);
    assert_eq!(DType::I8.max_value(), 127.0);
}

#[test]
fn dtype_u8_range() {
    assert_eq!(DType::U8.min_value(), 0.0);
    assert_eq!(DType::U8.max_value(), 255.0);
}

#[test]
fn dtype_bool_range() {
    assert_eq!(DType::Bool.min_value(), 0.0);
    assert_eq!(DType::Bool.max_value(), 1.0);
}

// ════════════════════════════════════════════════════════════════════════
// 5. DType conversion (using runtime tensor)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn convert_f64_to_f32() {
    use fajar_lang::runtime::ml::mixed_precision::to_dtype;
    use fajar_lang::runtime::ml::tensor::TensorValue;
    use ndarray::ArrayD;

    let data = ArrayD::from_shape_vec(vec![3], vec![1.1, 2.2, 3.3]).unwrap();
    let tensor = TensorValue::new(data, false);
    let converted = to_dtype(&tensor, DType::F32);
    // F32 roundtrip loses precision
    let vals: Vec<f64> = converted.data().iter().cloned().collect();
    assert!((vals[0] - 1.1).abs() < 0.001);
    assert!((vals[1] - 2.2).abs() < 0.001);
}

#[test]
fn convert_f64_to_i8() {
    use fajar_lang::runtime::ml::mixed_precision::to_dtype;
    use fajar_lang::runtime::ml::tensor::TensorValue;
    use ndarray::ArrayD;

    let data = ArrayD::from_shape_vec(vec![4], vec![1.7, -3.2, 200.0, -200.0]).unwrap();
    let tensor = TensorValue::new(data, false);
    let converted = to_dtype(&tensor, DType::I8);
    let vals: Vec<f64> = converted.data().iter().cloned().collect();
    assert_eq!(vals[0], 1.0); // 1.7 → 1 (truncated)
    assert_eq!(vals[1], -3.0); // -3.2 → -3
    assert_eq!(vals[2], 127.0); // 200 → clamped to 127
    assert_eq!(vals[3], -128.0); // -200 → clamped to -128
}

#[test]
fn convert_f64_to_u8() {
    use fajar_lang::runtime::ml::mixed_precision::to_dtype;
    use fajar_lang::runtime::ml::tensor::TensorValue;
    use ndarray::ArrayD;

    let data = ArrayD::from_shape_vec(vec![3], vec![100.5, -10.0, 300.0]).unwrap();
    let tensor = TensorValue::new(data, false);
    let converted = to_dtype(&tensor, DType::U8);
    let vals: Vec<f64> = converted.data().iter().cloned().collect();
    assert_eq!(vals[0], 100.0); // truncated
    assert_eq!(vals[1], 0.0); // clamped to 0
    assert_eq!(vals[2], 255.0); // clamped to 255
}

#[test]
fn convert_f64_to_bool() {
    use fajar_lang::runtime::ml::mixed_precision::to_dtype;
    use fajar_lang::runtime::ml::tensor::TensorValue;
    use ndarray::ArrayD;

    let data = ArrayD::from_shape_vec(vec![4], vec![0.0, 1.0, -1.0, 0.5]).unwrap();
    let tensor = TensorValue::new(data, false);
    let converted = to_dtype(&tensor, DType::Bool);
    let vals: Vec<f64> = converted.data().iter().cloned().collect();
    assert_eq!(vals[0], 0.0); // false
    assert_eq!(vals[1], 1.0); // true
    assert_eq!(vals[2], 1.0); // true (non-zero)
    assert_eq!(vals[3], 1.0); // true (non-zero)
}

#[test]
fn convert_f64_to_f64_identity() {
    use fajar_lang::runtime::ml::mixed_precision::to_dtype;
    use fajar_lang::runtime::ml::tensor::TensorValue;
    use ndarray::ArrayD;

    let data = ArrayD::from_shape_vec(vec![2], vec![3.14159, 2.71828]).unwrap();
    let tensor = TensorValue::new(data, false);
    let converted = to_dtype(&tensor, DType::F64);
    let vals: Vec<f64> = converted.data().iter().cloned().collect();
    assert!((vals[0] - 3.14159).abs() < 1e-10);
    assert!((vals[1] - 2.71828).abs() < 1e-10);
}

// ════════════════════════════════════════════════════════════════════════
// 6. DType classification
// ════════════════════════════════════════════════════════════════════════

#[test]
fn float_types() {
    for dt in &[DType::F64, DType::F32, DType::F16, DType::BF16] {
        assert!(dt.is_float(), "{dt} should be float");
        assert!(!dt.is_int(), "{dt} should not be int");
    }
}

#[test]
fn int_types() {
    for dt in &[DType::I8, DType::U8, DType::I32, DType::I64] {
        assert!(dt.is_int(), "{dt} should be int");
        assert!(!dt.is_float(), "{dt} should not be float");
    }
}

#[test]
fn quantized_types() {
    assert!(DType::I8.is_quantized());
    assert!(DType::U8.is_quantized());
    assert!(!DType::I32.is_quantized());
    assert!(!DType::F32.is_quantized());
}

// ════════════════════════════════════════════════════════════════════════
// 7. Size calculation for tensors
// ════════════════════════════════════════════════════════════════════════

#[test]
fn tensor_memory_size() {
    // 1000 elements × 8 bytes = 8KB (F64)
    assert_eq!(DType::F64.size_bytes() * 1000, 8000);
    // 1000 elements × 2 bytes = 2KB (F16)
    assert_eq!(DType::F16.size_bytes() * 1000, 2000);
    // 1000 elements × 1 byte = 1KB (I8)
    assert_eq!(DType::I8.size_bytes() * 1000, 1000);
}

#[test]
fn dtype_memory_savings() {
    let elements = 1_000_000; // 1M elements
    let f64_bytes = DType::F64.size_bytes() * elements;
    let f16_bytes = DType::F16.size_bytes() * elements;
    let i8_bytes = DType::I8.size_bytes() * elements;
    // F16 saves 4x vs F64
    assert_eq!(f64_bytes / f16_bytes, 4);
    // I8 saves 8x vs F64
    assert_eq!(f64_bytes / i8_bytes, 8);
}

// ════════════════════════════════════════════════════════════════════════
// 8. Equality and hashing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dtype_equality() {
    assert_eq!(DType::F32, DType::F32);
    assert_ne!(DType::F32, DType::F64);
    assert_ne!(DType::I8, DType::U8);
}

#[test]
fn dtype_hashable() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(DType::F32);
    set.insert(DType::F64);
    set.insert(DType::F32); // duplicate
    assert_eq!(set.len(), 2);
}
