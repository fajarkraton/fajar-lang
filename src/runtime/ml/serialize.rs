//! Model serialization — save and load tensor weights to/from binary format.
//!
//! Format (little-endian):
//! ```text
//! [4 bytes] Magic: "FJML"
//! [4 bytes] Version: u32
//! [4 bytes] Num layers: u32
//! For each layer:
//!   [4 bytes]  Name length: u32
//!   [N bytes]  Name: UTF-8 string
//!   [4 bytes]  Rank (num dims): u32
//!   [4*rank bytes] Shape: u32 per dimension
//!   [8*numel bytes] Data: f64 per element (little-endian)
//! ```

use super::tensor::{TensorError, TensorValue};

/// Magic bytes identifying a Fajar ML model file.
const MAGIC: &[u8; 4] = b"FJML";

/// Current format version.
const FORMAT_VERSION: u32 = 1;

/// A named tensor for serialization.
#[derive(Debug, Clone)]
pub struct NamedTensor {
    /// Parameter name (e.g. "layer0.weight").
    pub name: String,
    /// The tensor data.
    pub tensor: TensorValue,
}

/// Serializes named tensors to the FJML binary format.
///
/// Returns the binary data as a `Vec<u8>`.
pub fn save(tensors: &[NamedTensor]) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&(tensors.len() as u32).to_le_bytes());

    // Each tensor
    for nt in tensors {
        // Name
        let name_bytes = nt.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        // Shape
        let shape = nt.tensor.shape();
        buf.extend_from_slice(&(shape.len() as u32).to_le_bytes());
        for &dim in shape {
            buf.extend_from_slice(&(dim as u32).to_le_bytes());
        }

        // Data
        for &val in nt.tensor.data().iter() {
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    buf
}

/// Deserializes named tensors from the FJML binary format.
///
/// Returns an error if the data is malformed or the version is unsupported.
pub fn load(data: &[u8]) -> Result<Vec<NamedTensor>, TensorError> {
    let mut pos;

    let read_u32 = |pos: &mut usize| -> Result<u32, TensorError> {
        if *pos + 4 > data.len() {
            return Err(TensorError::InvalidData {
                reason: "unexpected end of file".into(),
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
                reason: "unexpected end of file".into(),
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

    // Magic
    if data.len() < 12 {
        return Err(TensorError::InvalidData {
            reason: "file too small for FJML header".into(),
        });
    }
    if &data[0..4] != MAGIC {
        return Err(TensorError::InvalidData {
            reason: format!("invalid magic: expected {:?}, got {:?}", MAGIC, &data[0..4]),
        });
    }
    pos = 4;

    // Version
    let version = read_u32(&mut pos)?;
    if version != FORMAT_VERSION {
        return Err(TensorError::InvalidData {
            reason: format!("unsupported format version: expected {FORMAT_VERSION}, got {version}"),
        });
    }

    // Num layers
    let num_layers = read_u32(&mut pos)? as usize;
    let mut tensors = Vec::with_capacity(num_layers);

    for _ in 0..num_layers {
        // Name
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

        // Shape
        let rank = read_u32(&mut pos)? as usize;
        let mut shape = Vec::with_capacity(rank);
        for _ in 0..rank {
            shape.push(read_u32(&mut pos)? as usize);
        }

        // Data
        let numel: usize = shape.iter().product();
        let mut values = Vec::with_capacity(numel);
        for _ in 0..numel {
            values.push(read_f64(&mut pos)?);
        }

        let tensor = TensorValue::from_data(values, &shape)?;
        tensors.push(NamedTensor { name, tensor });
    }

    Ok(tensors)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip_single() {
        let t = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let named = vec![NamedTensor {
            name: "dense.weight".into(),
            tensor: t,
        }];

        let bytes = save(&named);
        let loaded = load(&bytes).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "dense.weight");
        assert_eq!(loaded[0].tensor.shape(), &[2, 3]);
        assert_eq!(
            loaded[0].tensor.to_vec(),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
    }

    #[test]
    fn save_load_roundtrip_multiple() {
        let w = TensorValue::from_data(vec![0.1, 0.2, 0.3, 0.4], &[2, 2]).unwrap();
        let b = TensorValue::from_data(vec![0.5, 0.6], &[1, 2]).unwrap();
        let named = vec![
            NamedTensor {
                name: "layer0.weight".into(),
                tensor: w,
            },
            NamedTensor {
                name: "layer0.bias".into(),
                tensor: b,
            },
        ];

        let bytes = save(&named);
        let loaded = load(&bytes).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "layer0.weight");
        assert_eq!(loaded[0].tensor.shape(), &[2, 2]);
        assert_eq!(loaded[1].name, "layer0.bias");
        assert_eq!(loaded[1].tensor.shape(), &[1, 2]);
        assert_eq!(loaded[1].tensor.to_vec(), vec![0.5, 0.6]);
    }

    #[test]
    fn save_load_preserves_values_exactly() {
        let vals = vec![
            std::f64::consts::PI,
            std::f64::consts::E,
            -0.0,
            1e-300,
            1e300,
        ];
        let t = TensorValue::from_data(vals.clone(), &[5]).unwrap();
        let named = vec![NamedTensor {
            name: "test".into(),
            tensor: t,
        }];

        let bytes = save(&named);
        let loaded = load(&bytes).unwrap();

        // f64 round-trip through le bytes should be exact
        assert_eq!(loaded[0].tensor.to_vec(), vals);
    }

    #[test]
    fn load_invalid_magic() {
        let data = b"NOPE\x01\x00\x00\x00\x00\x00\x00\x00";
        let result = load(data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{err}").contains("invalid magic"),
            "error should mention invalid magic, got: {err}"
        );
    }

    #[test]
    fn load_wrong_version() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&99u32.to_le_bytes()); // bad version
        data.extend_from_slice(&0u32.to_le_bytes()); // 0 layers
        let result = load(&data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{err}").contains("unsupported format version"),
            "error should mention version, got: {err}"
        );
    }

    #[test]
    fn load_truncated_data() {
        let data = b"FJML";
        let result = load(data);
        assert!(result.is_err());
    }

    #[test]
    fn save_load_empty() {
        let named: Vec<NamedTensor> = vec![];
        let bytes = save(&named);
        let loaded = load(&bytes).unwrap();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn save_load_1d_tensor() {
        let t = TensorValue::from_data(vec![10.0, 20.0, 30.0], &[3]).unwrap();
        let named = vec![NamedTensor {
            name: "bias".into(),
            tensor: t,
        }];

        let bytes = save(&named);
        let loaded = load(&bytes).unwrap();

        assert_eq!(loaded[0].tensor.shape(), &[3]);
        assert_eq!(loaded[0].tensor.to_vec(), vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn save_load_3d_tensor() {
        let t = TensorValue::from_data((0..24).map(|i| i as f64).collect(), &[2, 3, 4]).unwrap();
        let named = vec![NamedTensor {
            name: "conv.weight".into(),
            tensor: t.clone(),
        }];

        let bytes = save(&named);
        let loaded = load(&bytes).unwrap();

        assert_eq!(loaded[0].tensor.shape(), &[2, 3, 4]);
        assert_eq!(loaded[0].tensor.to_vec(), t.to_vec());
    }

    #[test]
    fn dense_layer_save_load_predict_same() {
        use super::super::layers::Dense;
        // Create a Dense layer and make a prediction
        let layer = Dense::new(3, 2);
        let x = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[1, 3]).unwrap();
        let y1 = layer.forward(&x).unwrap();

        // Save weights
        let named = vec![
            NamedTensor {
                name: "weight".into(),
                tensor: layer.weight.clone(),
            },
            NamedTensor {
                name: "bias".into(),
                tensor: layer.bias.clone(),
            },
        ];
        let bytes = save(&named);

        // Load weights into a new layer
        let loaded = load(&bytes).unwrap();
        let mut layer2 = Dense::new(3, 2);
        *layer2.weight.data_mut() = loaded[0].tensor.data().clone();
        *layer2.bias.data_mut() = loaded[1].tensor.data().clone();

        // Predict with loaded layer
        let y2 = layer2.forward(&x).unwrap();

        // Must be exactly equal
        assert_eq!(y1.to_vec(), y2.to_vec());
    }
}
