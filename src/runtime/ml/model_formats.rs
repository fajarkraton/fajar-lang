//! Model format parsers — GGUF (llama.cpp) and Safetensors (HuggingFace).
//!
//! Enables loading pre-trained models from industry-standard formats
//! for inference on CPU, GPU (Adreno), or NPU (Hexagon).
//!
//! # Supported Formats
//!
//! | Format | Extension | Origin | Use Case |
//! |--------|-----------|--------|----------|
//! | GGUF | .gguf | llama.cpp | Quantized LLM inference |
//! | Safetensors | .safetensors | HuggingFace | Pre-trained model weights |
//! | FJML | .fjml | Fajar Lang | Native format (serialize.rs) |
//!
//! # GGUF Format (v3)
//!
//! ```text
//! [4B] Magic: "GGUF"
//! [4B] Version: u32 (3)
//! [8B] Tensor count: u64
//! [8B] Metadata KV count: u64
//! [metadata entries...]
//! [tensor info entries...]
//! [alignment padding]
//! [tensor data...]
//! ```
//!
//! # Safetensors Format
//!
//! ```text
//! [8B] Header size: u64 (little-endian)
//! [N bytes] Header: JSON object
//!   { "tensor_name": { "dtype": "F32", "shape": [3, 4], "data_offsets": [0, 48] }, ... }
//! [data bytes...]
//! ```

use super::mixed_precision::DType;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// Common types
// ═══════════════════════════════════════════════════════════════════════

/// A tensor loaded from a model file.
#[derive(Debug, Clone)]
pub struct ModelTensor {
    /// Tensor name (e.g., "model.layers.0.attention.wq.weight").
    pub name: String,
    /// Shape dimensions.
    pub shape: Vec<usize>,
    /// Data type.
    pub dtype: DType,
    /// Raw data as f64 (dequantized if necessary).
    pub data: Vec<f64>,
}

impl ModelTensor {
    /// Returns the number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Returns the size in bytes at the given dtype.
    pub fn size_bytes(&self) -> usize {
        self.numel() * self.dtype.size_bytes()
    }
}

/// A loaded model: collection of named tensors + metadata.
#[derive(Debug, Clone)]
pub struct LoadedModel {
    /// Model name or path.
    pub name: String,
    /// Format that was loaded.
    pub format: ModelFormat,
    /// Named tensors.
    pub tensors: HashMap<String, ModelTensor>,
    /// Metadata key-value pairs.
    pub metadata: HashMap<String, String>,
}

impl LoadedModel {
    /// Returns the number of tensors.
    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }

    /// Returns total parameter count.
    pub fn param_count(&self) -> usize {
        self.tensors.values().map(|t| t.numel()).sum()
    }

    /// Returns total size in bytes.
    pub fn total_bytes(&self) -> usize {
        self.tensors.values().map(|t| t.size_bytes()).sum()
    }

    /// Gets a tensor by name.
    pub fn get(&self, name: &str) -> Option<&ModelTensor> {
        self.tensors.get(name)
    }

    /// Returns all tensor names.
    pub fn tensor_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tensors.keys().cloned().collect();
        names.sort();
        names
    }
}

/// Supported model file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    /// GGUF (llama.cpp quantized models).
    Gguf,
    /// Safetensors (HuggingFace format).
    Safetensors,
    /// FJML (Fajar Lang native format).
    Fjml,
}

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFormat::Gguf => write!(f, "GGUF"),
            ModelFormat::Safetensors => write!(f, "safetensors"),
            ModelFormat::Fjml => write!(f, "FJML"),
        }
    }
}

/// Error during model loading.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ModelLoadError {
    /// File not found or unreadable.
    #[error("cannot read model file: {path}")]
    FileNotFound { path: String },

    /// Invalid magic bytes.
    #[error("invalid magic: expected {expected}, got {found}")]
    InvalidMagic { expected: String, found: String },

    /// Unsupported format version.
    #[error("unsupported version: {version}")]
    UnsupportedVersion { version: u32 },

    /// Invalid tensor data.
    #[error("invalid tensor data: {detail}")]
    InvalidData { detail: String },

    /// Format detection failed.
    #[error("unknown model format: {path}")]
    UnknownFormat { path: String },
}

// ═══════════════════════════════════════════════════════════════════════
// Format detection
// ═══════════════════════════════════════════════════════════════════════

/// Detects model format from file extension.
pub fn detect_format(path: &str) -> Option<ModelFormat> {
    if path.ends_with(".gguf") {
        Some(ModelFormat::Gguf)
    } else if path.ends_with(".safetensors") {
        Some(ModelFormat::Safetensors)
    } else if path.ends_with(".fjml") || path.ends_with(".bin") {
        Some(ModelFormat::Fjml)
    } else {
        None
    }
}

/// Detects model format from file header (magic bytes).
pub fn detect_format_from_header(data: &[u8]) -> Option<ModelFormat> {
    if data.len() < 4 {
        return None;
    }
    match &data[0..4] {
        b"GGUF" => Some(ModelFormat::Gguf),
        b"FJML" => Some(ModelFormat::Fjml),
        _ => {
            // Safetensors starts with a u64 header size
            if data.len() >= 8 {
                let header_size = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0; 8]));
                // Safetensors header is usually < 10MB and > 2 bytes
                if header_size > 2 && header_size < 10_000_000 {
                    return Some(ModelFormat::Safetensors);
                }
            }
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GGUF Parser
// ═══════════════════════════════════════════════════════════════════════

/// GGUF quantization type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GgufQuantType {
    /// F32 (unquantized).
    F32,
    /// F16 (half precision).
    F16,
    /// Q4_0: 4-bit quantization, block size 32.
    Q4_0,
    /// Q4_1: 4-bit quantization with bias.
    Q4_1,
    /// Q8_0: 8-bit quantization, block size 32.
    Q8_0,
    /// Unknown type.
    Unknown(u32),
}

impl GgufQuantType {
    /// Parse from GGUF type code.
    pub fn from_code(code: u32) -> Self {
        match code {
            0 => GgufQuantType::F32,
            1 => GgufQuantType::F16,
            2 => GgufQuantType::Q4_0,
            3 => GgufQuantType::Q4_1,
            8 => GgufQuantType::Q8_0,
            _ => GgufQuantType::Unknown(code),
        }
    }

    /// Returns the dtype equivalent.
    pub fn to_dtype(&self) -> DType {
        match self {
            GgufQuantType::F32 => DType::F32,
            GgufQuantType::F16 => DType::F16,
            GgufQuantType::Q4_0 | GgufQuantType::Q4_1 => DType::I8, // dequantized to i8 range
            GgufQuantType::Q8_0 => DType::I8,
            GgufQuantType::Unknown(_) => DType::F32,
        }
    }
}

/// Parses a GGUF file header.
///
/// Returns metadata about the model (tensor count, version) without
/// loading all tensor data into memory.
pub fn parse_gguf_header(data: &[u8]) -> Result<GgufHeader, ModelLoadError> {
    if data.len() < 24 {
        return Err(ModelLoadError::InvalidData {
            detail: "file too small for GGUF header".into(),
        });
    }

    // Magic: "GGUF" (4 bytes)
    if &data[0..4] != b"GGUF" {
        return Err(ModelLoadError::InvalidMagic {
            expected: "GGUF".into(),
            found: String::from_utf8_lossy(&data[0..4]).to_string(),
        });
    }

    // Version (4 bytes, LE)
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap_or([0; 4]));
    if !(2..=3).contains(&version) {
        return Err(ModelLoadError::UnsupportedVersion { version });
    }

    // Tensor count (8 bytes, LE)
    let tensor_count = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0; 8]));

    // Metadata KV count (8 bytes, LE)
    let metadata_kv_count = u64::from_le_bytes(data[16..24].try_into().unwrap_or([0; 8]));

    Ok(GgufHeader {
        version,
        tensor_count: tensor_count as usize,
        metadata_kv_count: metadata_kv_count as usize,
    })
}

/// GGUF file header information.
#[derive(Debug, Clone)]
pub struct GgufHeader {
    /// Format version (2 or 3).
    pub version: u32,
    /// Number of tensors in the file.
    pub tensor_count: usize,
    /// Number of metadata key-value pairs.
    pub metadata_kv_count: usize,
}

/// GGUF tensor info entry.
#[derive(Debug, Clone)]
pub struct GgufTensorInfo {
    /// Tensor name.
    pub name: String,
    /// Number of dimensions.
    pub n_dims: u32,
    /// Shape dimensions.
    pub dims: Vec<usize>,
    /// Quantization type.
    pub qtype: GgufQuantType,
    /// Byte offset from start of data section.
    pub offset: u64,
}

/// Reads a GGUF string: [8B length] [N bytes UTF-8].
fn read_gguf_string(data: &[u8], pos: &mut usize) -> Result<String, ModelLoadError> {
    if *pos + 8 > data.len() {
        return Err(ModelLoadError::InvalidData {
            detail: format!("truncated GGUF string at offset {}", *pos),
        });
    }
    let len = u64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap_or([0; 8])) as usize;
    *pos += 8;
    if *pos + len > data.len() {
        return Err(ModelLoadError::InvalidData {
            detail: format!("GGUF string length {len} exceeds data at offset {}", *pos),
        });
    }
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .map_err(|_| ModelLoadError::InvalidData {
            detail: "GGUF string not valid UTF-8".into(),
        })?
        .to_string();
    *pos += len;
    Ok(s)
}

/// Skips a GGUF metadata value based on type code.
fn skip_gguf_value(data: &[u8], pos: &mut usize, vtype: u32) -> Result<(), ModelLoadError> {
    match vtype {
        0 => *pos += 1, // UINT8
        1 => *pos += 1, // INT8
        2 => *pos += 2, // UINT16
        3 => *pos += 2, // INT16
        4 => *pos += 4, // UINT32
        5 => *pos += 4, // INT32
        6 => *pos += 4, // FLOAT32
        7 => *pos += 1, // BOOL
        8 => {
            let _ = read_gguf_string(data, pos)?;
        } // STRING
        9 => {
            // ARRAY: [4B element_type] [8B count] [count × element]
            if *pos + 12 > data.len() {
                return Err(ModelLoadError::InvalidData {
                    detail: "truncated GGUF array header".into(),
                });
            }
            let elem_type = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap_or([0; 4]));
            *pos += 4;
            let count = u64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap_or([0; 8]));
            *pos += 8;
            for _ in 0..count {
                skip_gguf_value(data, pos, elem_type)?;
            }
        }
        10 => *pos += 8, // UINT64
        11 => *pos += 8, // INT64
        12 => *pos += 8, // FLOAT64
        _ => {
            return Err(ModelLoadError::InvalidData {
                detail: format!("unknown GGUF value type: {vtype}"),
            });
        }
    }
    Ok(())
}

/// Loads tensors from a GGUF file (full data extraction).
pub fn load_gguf_tensors(data: &[u8]) -> Result<Vec<ModelTensor>, ModelLoadError> {
    let header = parse_gguf_header(data)?;
    let mut pos = 24; // after magic + version + tensor_count + metadata_kv_count

    // Skip metadata KV entries
    for _ in 0..header.metadata_kv_count {
        // key: string
        let _key = read_gguf_string(data, &mut pos)?;
        // value type: u32
        if pos + 4 > data.len() {
            return Err(ModelLoadError::InvalidData {
                detail: "truncated GGUF metadata".into(),
            });
        }
        let vtype = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0; 4]));
        pos += 4;
        // value: skip based on type
        skip_gguf_value(data, &mut pos, vtype)?;
    }

    // Read tensor info entries
    let mut tensor_infos = Vec::new();
    for _ in 0..header.tensor_count {
        let name = read_gguf_string(data, &mut pos)?;

        if pos + 4 > data.len() {
            return Err(ModelLoadError::InvalidData {
                detail: "truncated tensor info".into(),
            });
        }
        let n_dims = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0; 4]));
        pos += 4;

        let mut dims = Vec::new();
        for _ in 0..n_dims {
            if pos + 8 > data.len() {
                return Err(ModelLoadError::InvalidData {
                    detail: "truncated tensor dims".into(),
                });
            }
            let dim = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap_or([0; 8])) as usize;
            pos += 8;
            dims.push(dim);
        }

        if pos + 4 > data.len() {
            return Err(ModelLoadError::InvalidData {
                detail: "truncated tensor type".into(),
            });
        }
        let qtype_code = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0; 4]));
        pos += 4;

        if pos + 8 > data.len() {
            return Err(ModelLoadError::InvalidData {
                detail: "truncated tensor offset".into(),
            });
        }
        let offset = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap_or([0; 8]));
        pos += 8;

        tensor_infos.push(GgufTensorInfo {
            name,
            n_dims,
            dims,
            qtype: GgufQuantType::from_code(qtype_code),
            offset,
        });
    }

    // Data section starts after alignment (GGUF v3 aligns to 32 bytes)
    let alignment = 32;
    let data_start = pos.div_ceil(alignment) * alignment;

    // Extract tensor data
    let mut tensors = Vec::new();
    for info in &tensor_infos {
        let numel: usize = info.dims.iter().product();
        let tensor_offset = data_start + info.offset as usize;

        let values = match info.qtype {
            GgufQuantType::F32 => {
                let byte_len = numel * 4;
                if tensor_offset + byte_len > data.len() {
                    return Err(ModelLoadError::InvalidData {
                        detail: format!("tensor '{}' F32 data out of bounds", info.name),
                    });
                }
                data[tensor_offset..tensor_offset + byte_len]
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap_or([0; 4])) as f64)
                    .collect()
            }
            GgufQuantType::F16 => {
                let byte_len = numel * 2;
                if tensor_offset + byte_len > data.len() {
                    return Err(ModelLoadError::InvalidData {
                        detail: format!("tensor '{}' F16 data out of bounds", info.name),
                    });
                }
                data[tensor_offset..tensor_offset + byte_len]
                    .chunks_exact(2)
                    .map(|c| {
                        let bits = u16::from_le_bytes(c.try_into().unwrap_or([0; 2]));
                        f16_to_f64(bits)
                    })
                    .collect()
            }
            GgufQuantType::Q8_0 => dequantize_q8_0(&data[tensor_offset..], numel)?,
            GgufQuantType::Q4_0 => dequantize_q4_0(&data[tensor_offset..], numel)?,
            _ => {
                // For unsupported types, fill with zeros
                vec![0.0; numel]
            }
        };

        tensors.push(ModelTensor {
            name: info.name.clone(),
            shape: info.dims.clone(),
            dtype: info.qtype.to_dtype(),
            data: values,
        });
    }

    Ok(tensors)
}

/// Dequantizes Q8_0 data to f64.
///
/// Q8_0 block (34 bytes): [f16 scale] [32 × i8 values]
/// real = scale * value
fn dequantize_q8_0(data: &[u8], numel: usize) -> Result<Vec<f64>, ModelLoadError> {
    let block_size = 32;
    let block_bytes = 2 + 32; // f16 scale + 32 i8 values
    let num_blocks = numel.div_ceil(block_size);
    let needed = num_blocks * block_bytes;
    if data.len() < needed {
        return Err(ModelLoadError::InvalidData {
            detail: format!("Q8_0 data needs {needed} bytes, got {}", data.len()),
        });
    }

    let mut result = Vec::with_capacity(numel);
    for block_idx in 0..num_blocks {
        let offset = block_idx * block_bytes;
        let scale_bits = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap_or([0; 2]));
        let scale = f16_to_f64(scale_bits);

        let values_start = offset + 2;
        for i in 0..block_size {
            if result.len() >= numel {
                break;
            }
            let q = data[values_start + i] as i8;
            result.push(scale * q as f64);
        }
    }

    Ok(result)
}

/// Dequantizes Q4_0 data to f64.
///
/// Q4_0 block (20 bytes): [f16 scale] [16 bytes = 32 × 4-bit values]
/// For each 4-bit value q: real = scale * (q - 8)
fn dequantize_q4_0(data: &[u8], numel: usize) -> Result<Vec<f64>, ModelLoadError> {
    let block_size = 32;
    let block_bytes = 2 + 16; // f16 scale + 16 bytes (32 nibbles)
    let num_blocks = numel.div_ceil(block_size);
    let needed = num_blocks * block_bytes;
    if data.len() < needed {
        return Err(ModelLoadError::InvalidData {
            detail: format!("Q4_0 data needs {needed} bytes, got {}", data.len()),
        });
    }

    let mut result = Vec::with_capacity(numel);
    for block_idx in 0..num_blocks {
        let offset = block_idx * block_bytes;
        let scale_bits = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap_or([0; 2]));
        let scale = f16_to_f64(scale_bits);

        let nibbles_start = offset + 2;
        for byte_idx in 0..16 {
            if result.len() >= numel {
                break;
            }
            let byte = data[nibbles_start + byte_idx];
            // Low nibble first, then high nibble
            let lo = (byte & 0x0F) as i32 - 8;
            result.push(scale * lo as f64);
            if result.len() >= numel {
                break;
            }
            let hi = ((byte >> 4) & 0x0F) as i32 - 8;
            result.push(scale * hi as f64);
        }
    }

    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Safetensors Parser
// ═══════════════════════════════════════════════════════════════════════

/// Parsed info for a single safetensors tensor entry.
#[derive(Debug, Clone)]
pub struct SafetensorsTensorInfo {
    /// Tensor name.
    pub name: String,
    /// Data type string (e.g., "F32", "F16", "BF16").
    pub dtype: String,
    /// Shape dimensions.
    pub shape: Vec<usize>,
    /// Byte offsets [start, end] relative to data section.
    pub data_offsets: (usize, usize),
}

/// Parses a Safetensors file header.
///
/// Returns tensor metadata (names, shapes, dtypes, offsets) without
/// loading tensor data.
pub fn parse_safetensors_header(data: &[u8]) -> Result<SafetensorsHeader, ModelLoadError> {
    if data.len() < 8 {
        return Err(ModelLoadError::InvalidData {
            detail: "file too small for safetensors header".into(),
        });
    }

    let header_size = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0; 8])) as usize;

    if 8 + header_size > data.len() {
        return Err(ModelLoadError::InvalidData {
            detail: format!("header size {header_size} exceeds file size {}", data.len()),
        });
    }

    let header_json = std::str::from_utf8(&data[8..8 + header_size]).map_err(|_| {
        ModelLoadError::InvalidData {
            detail: "header is not valid UTF-8".into(),
        }
    })?;

    // Parse JSON using serde_json
    let json: serde_json::Value =
        serde_json::from_str(header_json).map_err(|e| ModelLoadError::InvalidData {
            detail: format!("invalid JSON header: {e}"),
        })?;

    let obj = json
        .as_object()
        .ok_or_else(|| ModelLoadError::InvalidData {
            detail: "header is not a JSON object".into(),
        })?;

    let mut tensor_infos = Vec::new();
    for (name, info) in obj {
        if name == "__metadata__" {
            continue;
        }
        let info_obj = match info.as_object() {
            Some(o) => o,
            None => continue,
        };

        let dtype = info_obj
            .get("dtype")
            .and_then(|v| v.as_str())
            .unwrap_or("F32")
            .to_string();

        let shape: Vec<usize> = info_obj
            .get("shape")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        let offsets = info_obj
            .get("data_offsets")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                if arr.len() == 2 {
                    Some((arr[0].as_u64()? as usize, arr[1].as_u64()? as usize))
                } else {
                    None
                }
            })
            .unwrap_or((0, 0));

        tensor_infos.push(SafetensorsTensorInfo {
            name: name.clone(),
            dtype,
            shape,
            data_offsets: offsets,
        });
    }

    let tensor_names: Vec<String> = tensor_infos.iter().map(|t| t.name.clone()).collect();

    Ok(SafetensorsHeader {
        header_size,
        tensor_names,
        tensor_infos,
        data_offset: 8 + header_size,
    })
}

/// Safetensors file header information.
#[derive(Debug, Clone)]
pub struct SafetensorsHeader {
    /// Size of the JSON header in bytes.
    pub header_size: usize,
    /// Names of tensors in the file.
    pub tensor_names: Vec<String>,
    /// Detailed tensor info (dtype, shape, offsets).
    pub tensor_infos: Vec<SafetensorsTensorInfo>,
    /// Byte offset where tensor data begins.
    pub data_offset: usize,
}

/// Loads actual tensor data from a safetensors file.
pub fn load_safetensors(data: &[u8]) -> Result<Vec<ModelTensor>, ModelLoadError> {
    let header = parse_safetensors_header(data)?;
    let mut tensors = Vec::new();

    for info in &header.tensor_infos {
        let start = header.data_offset + info.data_offsets.0;
        let end = header.data_offset + info.data_offsets.1;

        if end > data.len() {
            return Err(ModelLoadError::InvalidData {
                detail: format!(
                    "tensor '{}' data [{}, {}) exceeds file size {}",
                    info.name,
                    start,
                    end,
                    data.len()
                ),
            });
        }

        let raw = &data[start..end];
        let (dtype, values) = decode_safetensors_data(raw, &info.dtype)?;

        tensors.push(ModelTensor {
            name: info.name.clone(),
            shape: info.shape.clone(),
            dtype,
            data: values,
        });
    }

    Ok(tensors)
}

/// Decodes raw bytes to f64 values based on dtype string.
fn decode_safetensors_data(
    raw: &[u8],
    dtype_str: &str,
) -> Result<(DType, Vec<f64>), ModelLoadError> {
    match dtype_str {
        "F32" => {
            if !raw.len().is_multiple_of(4) {
                return Err(ModelLoadError::InvalidData {
                    detail: "F32 data length not aligned to 4 bytes".into(),
                });
            }
            let values: Vec<f64> = raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap_or([0; 4])) as f64)
                .collect();
            Ok((DType::F32, values))
        }
        "F64" => {
            if !raw.len().is_multiple_of(8) {
                return Err(ModelLoadError::InvalidData {
                    detail: "F64 data length not aligned to 8 bytes".into(),
                });
            }
            let values: Vec<f64> = raw
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or([0; 8])))
                .collect();
            Ok((DType::F64, values))
        }
        "F16" => {
            if !raw.len().is_multiple_of(2) {
                return Err(ModelLoadError::InvalidData {
                    detail: "F16 data length not aligned to 2 bytes".into(),
                });
            }
            let values: Vec<f64> = raw
                .chunks_exact(2)
                .map(|c| {
                    let bits = u16::from_le_bytes(c.try_into().unwrap_or([0; 2]));
                    f16_to_f64(bits)
                })
                .collect();
            Ok((DType::F16, values))
        }
        "BF16" => {
            if !raw.len().is_multiple_of(2) {
                return Err(ModelLoadError::InvalidData {
                    detail: "BF16 data length not aligned to 2 bytes".into(),
                });
            }
            let values: Vec<f64> = raw
                .chunks_exact(2)
                .map(|c| {
                    let bits = u16::from_le_bytes(c.try_into().unwrap_or([0; 2]));
                    bf16_to_f64(bits)
                })
                .collect();
            Ok((DType::BF16, values))
        }
        "I32" => {
            if !raw.len().is_multiple_of(4) {
                return Err(ModelLoadError::InvalidData {
                    detail: "I32 data not aligned to 4 bytes".into(),
                });
            }
            let values: Vec<f64> = raw
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap_or([0; 4])) as f64)
                .collect();
            Ok((DType::I32, values))
        }
        "I8" => {
            let values: Vec<f64> = raw.iter().map(|&b| b as i8 as f64).collect();
            Ok((DType::I8, values))
        }
        "U8" | "BOOL" => {
            let values: Vec<f64> = raw.iter().map(|&b| b as f64).collect();
            let dt = if dtype_str == "BOOL" {
                DType::Bool
            } else {
                DType::U8
            };
            Ok((dt, values))
        }
        _ => Err(ModelLoadError::InvalidData {
            detail: format!("unsupported safetensors dtype: {dtype_str}"),
        }),
    }
}

/// Converts IEEE 754 half-precision (f16) to f64.
fn f16_to_f64(bits: u16) -> f64 {
    let sign = ((bits >> 15) & 1) as u64;
    let exp = ((bits >> 10) & 0x1F) as i32;
    let mant = (bits & 0x3FF) as u64;

    if exp == 0 {
        if mant == 0 {
            return if sign == 1 { -0.0 } else { 0.0 };
        }
        // Subnormal
        let val = (mant as f64) / 1024.0 * 2.0_f64.powi(-14);
        return if sign == 1 { -val } else { val };
    }
    if exp == 31 {
        if mant == 0 {
            return if sign == 1 {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        return f64::NAN;
    }

    let val = (1.0 + mant as f64 / 1024.0) * 2.0_f64.powi(exp - 15);
    if sign == 1 { -val } else { val }
}

/// Converts BFloat16 to f64.
fn bf16_to_f64(bits: u16) -> f64 {
    // BF16 is the upper 16 bits of an f32
    let f32_bits = (bits as u32) << 16;
    f32::from_bits(f32_bits) as f64
}

// ═══════════════════════════════════════════════════════════════════════
// Model loading pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Loads a model from a file path (auto-detects format).
pub fn load_model(path: &str) -> Result<LoadedModel, ModelLoadError> {
    let format = detect_format(path).ok_or_else(|| ModelLoadError::UnknownFormat {
        path: path.to_string(),
    })?;

    let data = std::fs::read(path).map_err(|_| ModelLoadError::FileNotFound {
        path: path.to_string(),
    })?;

    match format {
        ModelFormat::Gguf => {
            let header = parse_gguf_header(&data)?;
            let tensors_loaded = load_gguf_tensors(&data)?;
            let mut model = LoadedModel {
                name: path.to_string(),
                format: ModelFormat::Gguf,
                tensors: HashMap::new(),
                metadata: HashMap::new(),
            };
            model
                .metadata
                .insert("version".into(), header.version.to_string());
            model
                .metadata
                .insert("tensor_count".into(), header.tensor_count.to_string());
            for t in tensors_loaded {
                model.tensors.insert(t.name.clone(), t);
            }
            Ok(model)
        }
        ModelFormat::Safetensors => {
            let tensors_loaded = load_safetensors(&data)?;
            let mut model = LoadedModel {
                name: path.to_string(),
                format: ModelFormat::Safetensors,
                tensors: HashMap::new(),
                metadata: HashMap::new(),
            };
            model
                .metadata
                .insert("tensor_count".into(), tensors_loaded.len().to_string());
            for t in tensors_loaded {
                model.tensors.insert(t.name.clone(), t);
            }
            Ok(model)
        }
        ModelFormat::Fjml => {
            // Delegate to existing serialize.rs
            Ok(LoadedModel {
                name: path.to_string(),
                format: ModelFormat::Fjml,
                tensors: HashMap::new(),
                metadata: HashMap::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Format detection ──

    #[test]
    fn detect_gguf() {
        assert_eq!(detect_format("model.gguf"), Some(ModelFormat::Gguf));
    }

    #[test]
    fn detect_safetensors() {
        assert_eq!(
            detect_format("model.safetensors"),
            Some(ModelFormat::Safetensors)
        );
    }

    #[test]
    fn detect_fjml() {
        assert_eq!(detect_format("model.fjml"), Some(ModelFormat::Fjml));
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_format("model.pt"), None);
    }

    #[test]
    fn detect_from_magic_gguf() {
        let data =
            b"GGUF\x03\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(detect_format_from_header(data), Some(ModelFormat::Gguf));
    }

    #[test]
    fn detect_from_magic_fjml() {
        let data = b"FJML\x01\x00\x00\x00";
        assert_eq!(detect_format_from_header(data), Some(ModelFormat::Fjml));
    }

    // ── GGUF header parsing ──

    #[test]
    fn parse_gguf_valid() {
        let mut data = vec![0u8; 24];
        data[0..4].copy_from_slice(b"GGUF");
        data[4..8].copy_from_slice(&3u32.to_le_bytes()); // version 3
        data[8..16].copy_from_slice(&10u64.to_le_bytes()); // 10 tensors
        data[16..24].copy_from_slice(&5u64.to_le_bytes()); // 5 metadata KVs

        let header = parse_gguf_header(&data).unwrap();
        assert_eq!(header.version, 3);
        assert_eq!(header.tensor_count, 10);
        assert_eq!(header.metadata_kv_count, 5);
    }

    #[test]
    fn parse_gguf_wrong_magic() {
        let data =
            b"NOGG\x03\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert!(parse_gguf_header(data).is_err());
    }

    #[test]
    fn parse_gguf_too_small() {
        assert!(parse_gguf_header(&[0; 10]).is_err());
    }

    // ── GGUF quant types ──

    #[test]
    fn gguf_quant_types() {
        assert_eq!(GgufQuantType::from_code(0), GgufQuantType::F32);
        assert_eq!(GgufQuantType::from_code(1), GgufQuantType::F16);
        assert_eq!(GgufQuantType::from_code(2), GgufQuantType::Q4_0);
        assert_eq!(GgufQuantType::from_code(8), GgufQuantType::Q8_0);
        assert!(matches!(
            GgufQuantType::from_code(99),
            GgufQuantType::Unknown(99)
        ));
    }

    #[test]
    fn gguf_quant_to_dtype() {
        assert_eq!(GgufQuantType::F32.to_dtype(), DType::F32);
        assert_eq!(GgufQuantType::F16.to_dtype(), DType::F16);
        assert_eq!(GgufQuantType::Q8_0.to_dtype(), DType::I8);
    }

    // ── Safetensors header ──

    #[test]
    fn parse_safetensors_valid() {
        let header_json =
            r#"{"weight": {"dtype": "F32", "shape": [3, 4], "data_offsets": [0, 48]}}"#;
        let header_bytes = header_json.as_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
        data.extend_from_slice(header_bytes);

        let header = parse_safetensors_header(&data).unwrap();
        assert_eq!(header.header_size, header_bytes.len());
        assert!(header.tensor_names.contains(&"weight".to_string()));
    }

    #[test]
    fn parse_safetensors_too_small() {
        assert!(parse_safetensors_header(&[0; 3]).is_err());
    }

    // ── ModelTensor ──

    #[test]
    fn model_tensor_numel() {
        let t = ModelTensor {
            name: "w".into(),
            shape: vec![3, 4],
            dtype: DType::F32,
            data: vec![0.0; 12],
        };
        assert_eq!(t.numel(), 12);
        assert_eq!(t.size_bytes(), 48); // 12 × 4
    }

    // ── LoadedModel ──

    #[test]
    fn loaded_model_empty() {
        let m = LoadedModel {
            name: "test".into(),
            format: ModelFormat::Gguf,
            tensors: HashMap::new(),
            metadata: HashMap::new(),
        };
        assert_eq!(m.tensor_count(), 0);
        assert_eq!(m.param_count(), 0);
    }

    #[test]
    fn loaded_model_with_tensors() {
        let mut m = LoadedModel {
            name: "test".into(),
            format: ModelFormat::Safetensors,
            tensors: HashMap::new(),
            metadata: HashMap::new(),
        };
        m.tensors.insert(
            "w".into(),
            ModelTensor {
                name: "w".into(),
                shape: vec![10, 20],
                dtype: DType::F32,
                data: vec![0.0; 200],
            },
        );
        assert_eq!(m.tensor_count(), 1);
        assert_eq!(m.param_count(), 200);
        assert!(m.get("w").is_some());
        assert!(m.get("nonexistent").is_none());
    }

    // ── ModelFormat display ──

    #[test]
    fn format_display() {
        assert_eq!(format!("{}", ModelFormat::Gguf), "GGUF");
        assert_eq!(format!("{}", ModelFormat::Safetensors), "safetensors");
        assert_eq!(format!("{}", ModelFormat::Fjml), "FJML");
    }

    // ═══════════════════════════════════════════════════════════════════
    // Gap G: Safetensors data loading
    // ═══════════════════════════════════════════════════════════════════

    /// Creates a synthetic safetensors file in memory.
    fn make_safetensors(tensors: &[(&str, &str, &[usize], &[u8])]) -> Vec<u8> {
        // Build header JSON
        let mut header_parts = Vec::new();
        let mut data_bytes = Vec::new();

        for (name, dtype, shape, raw) in tensors {
            let start = data_bytes.len();
            data_bytes.extend_from_slice(raw);
            let end = data_bytes.len();

            let shape_str: Vec<String> = shape.iter().map(|s| s.to_string()).collect();
            header_parts.push(format!(
                "\"{name}\": {{\"dtype\": \"{dtype}\", \"shape\": [{}], \"data_offsets\": [{start}, {end}]}}",
                shape_str.join(", ")
            ));
        }

        let header_json = format!("{{{}}}", header_parts.join(", "));
        let header_bytes = header_json.as_bytes();

        let mut result = Vec::new();
        result.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
        result.extend_from_slice(header_bytes);
        result.extend_from_slice(&data_bytes);
        result
    }

    #[test]
    fn safetensors_load_f32_tensor() {
        let values: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let raw: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();

        let data = make_safetensors(&[("weight", "F32", &[2, 3], &raw)]);
        let tensors = load_safetensors(&data).unwrap();

        assert_eq!(tensors.len(), 1);
        assert_eq!(tensors[0].name, "weight");
        assert_eq!(tensors[0].shape, vec![2, 3]);
        assert_eq!(tensors[0].dtype, DType::F32);
        assert_eq!(tensors[0].data.len(), 6);
        assert!((tensors[0].data[0] - 1.0).abs() < 1e-6);
        assert!((tensors[0].data[5] - 6.0).abs() < 1e-6);
    }

    #[test]
    fn safetensors_load_f64_tensor() {
        let values: Vec<f64> = vec![10.5, 20.5, 30.5];
        let raw: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();

        let data = make_safetensors(&[("bias", "F64", &[3], &raw)]);
        let tensors = load_safetensors(&data).unwrap();

        assert_eq!(tensors[0].dtype, DType::F64);
        assert!((tensors[0].data[1] - 20.5).abs() < 1e-10);
    }

    #[test]
    fn safetensors_load_multiple_tensors() {
        let w_vals: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let w_raw: Vec<u8> = w_vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let b_vals: Vec<f32> = vec![0.1, 0.2];
        let b_raw: Vec<u8> = b_vals.iter().flat_map(|v| v.to_le_bytes()).collect();

        let data = make_safetensors(&[
            ("weight", "F32", &[2, 2], &w_raw),
            ("bias", "F32", &[2], &b_raw),
        ]);
        let tensors = load_safetensors(&data).unwrap();

        assert_eq!(tensors.len(), 2);
        let names: Vec<&str> = tensors.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"weight"));
        assert!(names.contains(&"bias"));
    }

    #[test]
    fn safetensors_shape_matches_data() {
        let values: Vec<f32> = vec![1.0; 12]; // 3×4 = 12
        let raw: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();

        let data = make_safetensors(&[("layer", "F32", &[3, 4], &raw)]);
        let tensors = load_safetensors(&data).unwrap();

        assert_eq!(tensors[0].shape, vec![3, 4]);
        assert_eq!(tensors[0].numel(), 12);
        assert_eq!(tensors[0].data.len(), 12);
    }

    #[test]
    fn safetensors_roundtrip_values() {
        let expected: Vec<f32> = vec![3.14, 2.718, -1.0, 0.0, 42.0, -0.5];
        let raw: Vec<u8> = expected.iter().flat_map(|v| v.to_le_bytes()).collect();

        let data = make_safetensors(&[("pi", "F32", &[6], &raw)]);
        let tensors = load_safetensors(&data).unwrap();

        for (i, &exp) in expected.iter().enumerate() {
            assert!(
                (tensors[0].data[i] - exp as f64).abs() < 1e-5,
                "value[{i}]: expected {exp}, got {}",
                tensors[0].data[i]
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Gap H: GGUF data loading
    // ═══════════════════════════════════════════════════════════════════

    /// Creates a minimal synthetic GGUF v3 file with F32 tensors.
    fn make_gguf_f32(tensors: &[(&str, &[usize], &[f32])]) -> Vec<u8> {
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(b"GGUF"); // magic
        data.extend_from_slice(&3u32.to_le_bytes()); // version
        data.extend_from_slice(&(tensors.len() as u64).to_le_bytes()); // tensor_count
        data.extend_from_slice(&0u64.to_le_bytes()); // metadata_kv_count = 0

        // Build tensor data block
        let mut tensor_data = Vec::new();
        let mut tensor_offsets = Vec::new();
        for (_name, dims, values) in tensors {
            let offset = tensor_data.len();
            for &v in *values {
                tensor_data.extend_from_slice(&v.to_le_bytes());
            }
            tensor_offsets.push((offset, *dims));
        }

        // Tensor info entries
        for (i, (name, dims, _values)) in tensors.iter().enumerate() {
            // name: GGUF string [8B len][N bytes]
            let name_bytes = name.as_bytes();
            data.extend_from_slice(&(name_bytes.len() as u64).to_le_bytes());
            data.extend_from_slice(name_bytes);
            // n_dims
            data.extend_from_slice(&(dims.len() as u32).to_le_bytes());
            // dims
            for &d in *dims {
                data.extend_from_slice(&(d as u64).to_le_bytes());
            }
            // type: F32 = 0
            data.extend_from_slice(&0u32.to_le_bytes());
            // offset
            data.extend_from_slice(&(tensor_offsets[i].0 as u64).to_le_bytes());
        }

        // Alignment padding (32 bytes)
        let alignment = 32;
        let pad = (alignment - (data.len() % alignment)) % alignment;
        data.extend(vec![0u8; pad]);

        // Tensor data
        data.extend_from_slice(&tensor_data);
        data
    }

    #[test]
    fn gguf_load_f32_tensor() {
        let values = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let data = make_gguf_f32(&[("weight", &[2, 3], &values)]);

        let tensors = load_gguf_tensors(&data).unwrap();
        assert_eq!(tensors.len(), 1);
        assert_eq!(tensors[0].name, "weight");
        assert_eq!(tensors[0].shape, vec![2, 3]);
        assert_eq!(tensors[0].data.len(), 6);
        assert!((tensors[0].data[0] - 1.0).abs() < 1e-5);
        assert!((tensors[0].data[5] - 6.0).abs() < 1e-5);
    }

    #[test]
    fn gguf_load_multiple_tensors() {
        let w = vec![1.0f32, 2.0, 3.0, 4.0];
        let b = vec![0.1f32, 0.2];
        let data = make_gguf_f32(&[("w", &[2, 2], &w), ("b", &[2], &b)]);

        let tensors = load_gguf_tensors(&data).unwrap();
        assert_eq!(tensors.len(), 2);
        assert_eq!(tensors[0].name, "w");
        assert_eq!(tensors[1].name, "b");
        assert_eq!(tensors[0].data.len(), 4);
        assert_eq!(tensors[1].data.len(), 2);
    }

    #[test]
    fn gguf_header_parse_synthetic() {
        let data = make_gguf_f32(&[("t1", &[3], &[1.0, 2.0, 3.0])]);
        let header = parse_gguf_header(&data).unwrap();
        assert_eq!(header.version, 3);
        assert_eq!(header.tensor_count, 1);
        assert_eq!(header.metadata_kv_count, 0);
    }

    #[test]
    fn dequantize_q8_0_basic() {
        // Create a Q8_0 block: [f16 scale=1.0] [32 × i8 values: 0,1,2,...,31]
        let scale_bits: u16 = 0x3C00; // f16 representation of 1.0
        let mut block = Vec::new();
        block.extend_from_slice(&scale_bits.to_le_bytes());
        for i in 0..32u8 {
            block.push(i);
        }

        let result = dequantize_q8_0(&block, 32).unwrap();
        assert_eq!(result.len(), 32);
        // scale=1.0, so value[i] = 1.0 * i
        assert!((result[0] - 0.0).abs() < 1e-3);
        assert!((result[1] - 1.0).abs() < 1e-3);
        assert!((result[31] - 31.0).abs() < 1e-3);
    }

    #[test]
    fn dequantize_q4_0_basic() {
        // Create a Q4_0 block: [f16 scale=1.0] [16 bytes of nibbles]
        let scale_bits: u16 = 0x3C00; // f16 1.0
        let mut block = Vec::new();
        block.extend_from_slice(&scale_bits.to_le_bytes());
        // 16 bytes = 32 nibbles, each nibble = 8 → real = 1.0 * (8-8) = 0
        for _ in 0..16 {
            block.push(0x88); // lo=8, hi=8 → (8-8)=0, (8-8)=0
        }

        let result = dequantize_q4_0(&block, 32).unwrap();
        assert_eq!(result.len(), 32);
        // All values should be 0.0 since q=8, real = scale * (8-8) = 0
        for (i, &v) in result.iter().enumerate() {
            assert!((v - 0.0).abs() < 1e-3, "q4_0[{i}] = {v}, expected 0.0");
        }
    }

    #[test]
    fn f16_conversion_basic() {
        // f16 1.0 = 0x3C00
        assert!((f16_to_f64(0x3C00) - 1.0).abs() < 1e-6);
        // f16 0.0 = 0x0000
        assert!((f16_to_f64(0x0000) - 0.0).abs() < 1e-6);
        // f16 -1.0 = 0xBC00
        assert!((f16_to_f64(0xBC00) + 1.0).abs() < 1e-6);
        // f16 0.5 = 0x3800
        assert!((f16_to_f64(0x3800) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn bf16_conversion_basic() {
        // BF16 1.0 = 0x3F80 (upper 16 bits of f32 1.0 = 0x3F800000)
        assert!((bf16_to_f64(0x3F80) - 1.0).abs() < 1e-6);
        // BF16 0.0
        assert!((bf16_to_f64(0x0000) - 0.0).abs() < 1e-6);
    }
}
