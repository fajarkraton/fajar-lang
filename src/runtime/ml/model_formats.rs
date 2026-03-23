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

// ═══════════════════════════════════════════════════════════════════════
// Safetensors Parser
// ═══════════════════════════════════════════════════════════════════════

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

    // Simple JSON parsing: extract tensor names from keys
    let mut tensors = Vec::new();
    // Look for patterns: "tensor_name": {"dtype": "F32", "shape": [M, N], "data_offsets": [start, end]}
    for segment in header_json.split('"') {
        // Every other segment between quotes could be a tensor name
        // Skip __metadata__ and dtype/shape/data_offsets keys
        if !segment.is_empty()
            && !segment.starts_with('{')
            && !segment.starts_with(':')
            && !segment.starts_with(',')
            && !segment.contains("dtype")
            && !segment.contains("shape")
            && !segment.contains("data_offsets")
            && !segment.contains("__metadata__")
            && segment.starts_with(|c: char| c.is_alphabetic())
        {
            tensors.push(segment.to_string());
        }
    }

    Ok(SafetensorsHeader {
        header_size,
        tensor_names: tensors,
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
    /// Byte offset where tensor data begins.
    pub data_offset: usize,
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
            Ok(model)
        }
        ModelFormat::Safetensors => {
            let header = parse_safetensors_header(&data)?;
            let mut model = LoadedModel {
                name: path.to_string(),
                format: ModelFormat::Safetensors,
                tensors: HashMap::new(),
                metadata: HashMap::new(),
            };
            model
                .metadata
                .insert("tensor_count".into(), header.tensor_names.len().to_string());
            for name in &header.tensor_names {
                model
                    .metadata
                    .insert(format!("tensor.{name}"), "present".into());
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
}
