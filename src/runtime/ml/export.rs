//! Model export for embedded targets — compact binary format with C header generation.
//!
//! Exports quantized models in a no-alloc-friendly format:
//! ```text
//! [4 bytes] Magic: "FJMQ" (Fajar ML Quantized)
//! [4 bytes] Version: u32
//! [4 bytes] Num layers: u32
//! For each layer:
//!   [4 bytes]  Name length: u32
//!   [N bytes]  Name: UTF-8 string
//!   [8 bytes]  Scale: f64
//!   [4 bytes]  Rank: u32
//!   [4*rank bytes] Shape: u32 per dimension
//!   [numel bytes]  Data: i8 per element
//! ```

use super::quantize::QuantizedTensor;
use super::tensor::TensorError;

/// Magic bytes for quantized model files.
const MAGIC: &[u8; 4] = b"FJMQ";

/// Current format version.
const FORMAT_VERSION: u32 = 1;

/// A named quantized tensor for export.
#[derive(Debug, Clone)]
pub struct NamedQuantized {
    /// Parameter name.
    pub name: String,
    /// Quantized tensor.
    pub tensor: QuantizedTensor,
}

/// Serializes quantized tensors to compact binary format.
pub fn export_quantized(tensors: &[NamedQuantized]) -> Vec<u8> {
    let mut buf = Vec::new();

    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&(tensors.len() as u32).to_le_bytes());

    for nt in tensors {
        let name_bytes = nt.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        buf.extend_from_slice(&nt.tensor.scale().to_le_bytes());

        let shape = nt.tensor.shape();
        buf.extend_from_slice(&(shape.len() as u32).to_le_bytes());
        for &dim in shape {
            buf.extend_from_slice(&(dim as u32).to_le_bytes());
        }

        // i8 data — 1 byte per element
        for &val in nt.tensor.data() {
            buf.push(val as u8);
        }
    }

    buf
}

/// Loads quantized tensors from compact binary format.
pub fn import_quantized(data: &[u8]) -> Result<Vec<NamedQuantized>, TensorError> {
    let mut pos;

    let read_u32 = |pos: &mut usize| -> Result<u32, TensorError> {
        if *pos + 4 > data.len() {
            return Err(TensorError::InvalidData {
                reason: "unexpected end of quantized model file".into(),
            });
        }
        let bytes: [u8; 4] =
            data[*pos..*pos + 4]
                .try_into()
                .map_err(|_| TensorError::InvalidData {
                    reason: "failed to read u32".into(),
                })?;
        *pos += 4;
        Ok(u32::from_le_bytes(bytes))
    };

    let read_f64 = |pos: &mut usize| -> Result<f64, TensorError> {
        if *pos + 8 > data.len() {
            return Err(TensorError::InvalidData {
                reason: "unexpected end of quantized model file".into(),
            });
        }
        let bytes: [u8; 8] =
            data[*pos..*pos + 8]
                .try_into()
                .map_err(|_| TensorError::InvalidData {
                    reason: "failed to read f64".into(),
                })?;
        *pos += 8;
        Ok(f64::from_le_bytes(bytes))
    };

    if data.len() < 12 {
        return Err(TensorError::InvalidData {
            reason: "file too small for FJMQ header".into(),
        });
    }
    if &data[0..4] != MAGIC {
        return Err(TensorError::InvalidData {
            reason: format!("invalid magic: expected {:?}, got {:?}", MAGIC, &data[0..4]),
        });
    }
    pos = 4;

    let version = read_u32(&mut pos)?;
    if version != FORMAT_VERSION {
        return Err(TensorError::InvalidData {
            reason: format!(
                "unsupported quantized format version: expected {FORMAT_VERSION}, got {version}"
            ),
        });
    }

    let num_layers = read_u32(&mut pos)? as usize;
    let mut tensors = Vec::with_capacity(num_layers);

    for _ in 0..num_layers {
        let name_len = read_u32(&mut pos)? as usize;
        if pos + name_len > data.len() {
            return Err(TensorError::InvalidData {
                reason: "unexpected end of file reading name".into(),
            });
        }
        let name = String::from_utf8(data[pos..pos + name_len].to_vec()).map_err(|_| {
            TensorError::InvalidData {
                reason: "invalid UTF-8 in parameter name".into(),
            }
        })?;
        pos += name_len;

        let scale = read_f64(&mut pos)?;

        let rank = read_u32(&mut pos)? as usize;
        let mut shape = Vec::with_capacity(rank);
        for _ in 0..rank {
            shape.push(read_u32(&mut pos)? as usize);
        }

        let numel: usize = shape.iter().product();
        if pos + numel > data.len() {
            return Err(TensorError::InvalidData {
                reason: "unexpected end of file reading i8 data".into(),
            });
        }
        let i8_data: Vec<i8> = data[pos..pos + numel].iter().map(|&b| b as i8).collect();
        pos += numel;

        tensors.push(NamedQuantized {
            name,
            tensor: QuantizedTensor::from_raw(i8_data, scale, shape),
        });
    }

    Ok(tensors)
}

/// Generates a C header file describing the model structure.
///
/// This header provides array declarations and metadata macros
/// for embedding the model in C/C++ firmware.
pub fn generate_c_header(model_name: &str, tensors: &[NamedQuantized]) -> String {
    let guard = model_name.to_uppercase();
    let mut out = String::new();

    out.push_str(&format!(
        "/* Auto-generated by Fajar Lang ML export */\n\
         #ifndef {guard}_H\n\
         #define {guard}_H\n\
         \n\
         #include <stdint.h>\n\
         \n\
         #define {guard}_NUM_LAYERS {}\n\n",
        tensors.len()
    ));

    for (i, nt) in tensors.iter().enumerate() {
        let safe_name = nt.name.replace('.', "_");
        let shape = nt.tensor.shape();
        let numel = nt.tensor.numel();

        out.push_str(&format!(
            "/* Layer {i}: \"{}\" shape={shape:?} scale={} */\n",
            nt.name,
            nt.tensor.scale()
        ));
        out.push_str(&format!(
            "#define {guard}_{}_NUMEL {numel}\n",
            safe_name.to_uppercase()
        ));
        out.push_str(&format!(
            "static const double {guard}_{}_SCALE = {:.15e};\n",
            safe_name.to_uppercase(),
            nt.tensor.scale()
        ));

        // Shape dims
        for (d, &dim) in shape.iter().enumerate() {
            out.push_str(&format!(
                "#define {guard}_{}_DIM{d} {dim}\n",
                safe_name.to_uppercase()
            ));
        }

        out.push_str(&format!(
            "extern const int8_t {model_name}_{safe_name}[{numel}];\n\n"
        ));
    }

    out.push_str(&format!("#endif /* {guard}_H */\n"));
    out
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::super::quantize::QuantizedTensor;
    use super::super::tensor::TensorValue;
    use super::*;

    #[test]
    fn export_import_roundtrip() {
        let t = TensorValue::from_data(vec![1.0, -0.5, 0.25, 0.0], &[2, 2]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        let named = vec![NamedQuantized {
            name: "layer.weight".into(),
            tensor: qt.clone(),
        }];

        let bytes = export_quantized(&named);
        let loaded = import_quantized(&bytes).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "layer.weight");
        assert_eq!(loaded[0].tensor.shape(), qt.shape());
        assert!((loaded[0].tensor.scale() - qt.scale()).abs() < 1e-15);
        assert_eq!(loaded[0].tensor.data(), qt.data());
    }

    #[test]
    fn export_import_multiple() {
        let w = TensorValue::from_data(vec![0.1, 0.2, 0.3, 0.4], &[2, 2]).unwrap();
        let b = TensorValue::from_data(vec![0.5, 0.6], &[2]).unwrap();
        let named = vec![
            NamedQuantized {
                name: "weight".into(),
                tensor: QuantizedTensor::quantize(&w),
            },
            NamedQuantized {
                name: "bias".into(),
                tensor: QuantizedTensor::quantize(&b),
            },
        ];

        let bytes = export_quantized(&named);
        let loaded = import_quantized(&bytes).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "weight");
        assert_eq!(loaded[1].name, "bias");
    }

    #[test]
    fn import_invalid_magic() {
        let data = b"NOPE\x01\x00\x00\x00\x00\x00\x00\x00";
        assert!(import_quantized(data).is_err());
    }

    #[test]
    fn import_wrong_version() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&99u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        assert!(import_quantized(&data).is_err());
    }

    #[test]
    fn export_import_empty() {
        let named: Vec<NamedQuantized> = vec![];
        let bytes = export_quantized(&named);
        let loaded = import_quantized(&bytes).unwrap();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn exported_file_is_compact() {
        // INT8 format should be ~8x smaller than f64
        let t = TensorValue::from_data(vec![0.5; 1000], &[1000]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        let named = vec![NamedQuantized {
            name: "w".into(),
            tensor: qt,
        }];
        let bytes = export_quantized(&named);
        // Header (12) + name_len(4) + name(1) + scale(8) + rank(4) + shape(4) + data(1000)
        // = ~1033 bytes, vs 8000 bytes for f64
        assert!(
            bytes.len() < 1100,
            "exported size {} too large",
            bytes.len()
        );
    }

    #[test]
    fn generate_c_header_basic() {
        let t = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        let named = vec![NamedQuantized {
            name: "dense.weight".into(),
            tensor: qt,
        }];

        let header = generate_c_header("model", &named);

        assert!(header.contains("#ifndef MODEL_H"));
        assert!(header.contains("#define MODEL_H"));
        assert!(header.contains("#define MODEL_NUM_LAYERS 1"));
        assert!(header.contains("MODEL_DENSE_WEIGHT_NUMEL 6"));
        assert!(header.contains("MODEL_DENSE_WEIGHT_DIM0 2"));
        assert!(header.contains("MODEL_DENSE_WEIGHT_DIM1 3"));
        assert!(header.contains("extern const int8_t model_dense_weight[6]"));
        assert!(header.contains("#endif"));
    }

    #[test]
    fn generate_c_header_multiple_layers() {
        let named = vec![
            NamedQuantized {
                name: "w".into(),
                tensor: QuantizedTensor::quantize(
                    &TensorValue::from_data(vec![1.0; 4], &[2, 2]).unwrap(),
                ),
            },
            NamedQuantized {
                name: "b".into(),
                tensor: QuantizedTensor::quantize(
                    &TensorValue::from_data(vec![0.5; 2], &[2]).unwrap(),
                ),
            },
        ];

        let header = generate_c_header("net", &named);
        assert!(header.contains("#define NET_NUM_LAYERS 2"));
        assert!(header.contains("extern const int8_t net_w[4]"));
        assert!(header.contains("extern const int8_t net_b[2]"));
    }
}
