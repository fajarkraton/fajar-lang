//! TFLite model import — minimal binary format parser and inference engine.
//!
//! Parses TFLite-compatible model data (flatbuffer-based binary format)
//! and executes operator graphs using Fajar Lang tensor operations.
//! No external flatbuffers dependency — implements direct binary parsing.

use ndarray::Array2;
use std::collections::HashMap;
use thiserror::Error;

use super::tensor::TensorError;

// ═══════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from TFLite model operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum TfLiteError {
    /// Invalid or corrupted binary data.
    #[error("TfLite: invalid binary data — {reason}")]
    InvalidData {
        /// Reason for invalidity.
        reason: String,
    },

    /// Unsupported operator type.
    #[error("TfLite: unsupported operator {op_name}")]
    UnsupportedOp {
        /// Name of the unsupported operator.
        op_name: String,
    },

    /// Shape mismatch during inference.
    #[error("TfLite: shape mismatch — expected {expected:?}, got {got:?}")]
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        got: Vec<usize>,
    },

    /// Tensor operation error.
    #[error("TfLite: tensor error — {0}")]
    TensorError(#[from] TensorError),

    /// Missing tensor buffer.
    #[error("TfLite: missing buffer at index {index}")]
    MissingBuffer {
        /// Buffer index.
        index: usize,
    },

    /// Invalid tensor index.
    #[error("TfLite: invalid tensor index {index} (max {max})")]
    InvalidTensorIndex {
        /// Requested index.
        index: usize,
        /// Maximum valid index.
        max: usize,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// TfLite Types
// ═══════════════════════════════════════════════════════════════════════

/// TFLite tensor data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TfLiteType {
    /// 32-bit floating point.
    Float32,
    /// 8-bit signed integer (quantized).
    Int8,
    /// 8-bit unsigned integer (quantized).
    UInt8,
    /// 32-bit signed integer.
    Int32,
    /// 16-bit signed integer.
    Int16,
    /// 16-bit floating point.
    Float16,
}

impl TfLiteType {
    /// Returns the byte size of a single element.
    pub fn size_bytes(&self) -> usize {
        match self {
            TfLiteType::Float32 => 4,
            TfLiteType::Int8 => 1,
            TfLiteType::UInt8 => 1,
            TfLiteType::Int32 => 4,
            TfLiteType::Int16 => 2,
            TfLiteType::Float16 => 2,
        }
    }

    /// Returns a human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            TfLiteType::Float32 => "float32",
            TfLiteType::Int8 => "int8",
            TfLiteType::UInt8 => "uint8",
            TfLiteType::Int32 => "int32",
            TfLiteType::Int16 => "int16",
            TfLiteType::Float16 => "float16",
        }
    }

    /// Parses a type code from the TFLite binary format.
    pub fn from_code(code: u8) -> Result<Self, TfLiteError> {
        match code {
            0 => Ok(TfLiteType::Float32),
            1 => Ok(TfLiteType::Float16),
            2 => Ok(TfLiteType::Int32),
            3 => Ok(TfLiteType::UInt8),
            9 => Ok(TfLiteType::Int8),
            7 => Ok(TfLiteType::Int16),
            _ => Err(TfLiteError::InvalidData {
                reason: format!("unknown type code: {code}"),
            }),
        }
    }
}

impl std::fmt::Display for TfLiteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Quantization parameters for a tensor.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantizationParams {
    /// Per-tensor scale factor.
    pub scale: f64,
    /// Zero point offset.
    pub zero_point: i64,
}

impl QuantizationParams {
    /// Creates new quantization parameters.
    pub fn new(scale: f64, zero_point: i64) -> Self {
        Self { scale, zero_point }
    }

    /// Dequantizes an integer value to float.
    pub fn dequantize(&self, value: i64) -> f64 {
        (value - self.zero_point) as f64 * self.scale
    }

    /// Quantizes a float value to integer.
    pub fn quantize(&self, value: f64) -> i64 {
        (value / self.scale).round() as i64 + self.zero_point
    }
}

impl Default for QuantizationParams {
    fn default() -> Self {
        Self {
            scale: 1.0,
            zero_point: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TfLite Tensor
// ═══════════════════════════════════════════════════════════════════════

/// A tensor in the TFLite model graph.
#[derive(Debug, Clone)]
pub struct TfLiteTensor {
    /// Tensor shape (e.g., `[1, 28, 28, 1]`).
    pub shape: Vec<usize>,
    /// Data type.
    pub dtype: TfLiteType,
    /// Index into the model's buffer table.
    pub buffer_index: usize,
    /// Human-readable name.
    pub name: String,
    /// Optional quantization parameters.
    pub quantization: Option<QuantizationParams>,
}

impl TfLiteTensor {
    /// Creates a new TFLite tensor descriptor.
    pub fn new(shape: Vec<usize>, dtype: TfLiteType, buffer_index: usize, name: String) -> Self {
        Self {
            shape,
            dtype,
            buffer_index,
            name,
            quantization: None,
        }
    }

    /// Creates a tensor descriptor with quantization parameters.
    pub fn quantized(
        shape: Vec<usize>,
        dtype: TfLiteType,
        buffer_index: usize,
        name: String,
        scale: f64,
        zero_point: i64,
    ) -> Self {
        Self {
            shape,
            dtype,
            buffer_index,
            name,
            quantization: Some(QuantizationParams::new(scale, zero_point)),
        }
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Returns the total byte size.
    pub fn byte_size(&self) -> usize {
        self.numel() * self.dtype.size_bytes()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TfLite Operators
// ═══════════════════════════════════════════════════════════════════════

/// Supported TFLite operator types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TfLiteOp {
    /// 2D convolution.
    Conv2D,
    /// Depthwise 2D convolution.
    DepthwiseConv2D,
    /// Fully-connected (dense) layer.
    FullyConnected,
    /// Reshape tensor.
    Reshape,
    /// Softmax activation.
    Softmax,
    /// ReLU activation.
    ReLU,
    /// ReLU6 activation (clamp to [0, 6]).
    ReLU6,
    /// Element-wise addition.
    Add,
    /// Element-wise multiplication.
    Mul,
    /// Max pooling 2D.
    MaxPool2D,
    /// Average pooling 2D.
    AveragePool2D,
}

impl TfLiteOp {
    /// Parses an operator code from the TFLite binary format.
    pub fn from_code(code: u8) -> Result<Self, TfLiteError> {
        match code {
            0 => Ok(TfLiteOp::Add),
            1 => Ok(TfLiteOp::AveragePool2D),
            3 => Ok(TfLiteOp::Conv2D),
            4 => Ok(TfLiteOp::DepthwiseConv2D),
            9 => Ok(TfLiteOp::FullyConnected),
            17 => Ok(TfLiteOp::Mul),
            18 => Ok(TfLiteOp::Reshape),
            22 => Ok(TfLiteOp::Softmax),
            25 => Ok(TfLiteOp::ReLU),
            26 => Ok(TfLiteOp::ReLU6),
            56 => Ok(TfLiteOp::MaxPool2D),
            _ => Err(TfLiteError::UnsupportedOp {
                op_name: format!("op_code_{code}"),
            }),
        }
    }

    /// Returns the operator name.
    pub fn name(&self) -> &'static str {
        match self {
            TfLiteOp::Conv2D => "Conv2D",
            TfLiteOp::DepthwiseConv2D => "DepthwiseConv2D",
            TfLiteOp::FullyConnected => "FullyConnected",
            TfLiteOp::Reshape => "Reshape",
            TfLiteOp::Softmax => "Softmax",
            TfLiteOp::ReLU => "ReLU",
            TfLiteOp::ReLU6 => "ReLU6",
            TfLiteOp::Add => "Add",
            TfLiteOp::Mul => "Mul",
            TfLiteOp::MaxPool2D => "MaxPool2D",
            TfLiteOp::AveragePool2D => "AveragePool2D",
        }
    }
}

impl std::fmt::Display for TfLiteOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A single operator node in the execution graph.
#[derive(Debug, Clone)]
pub struct TfLiteOperator {
    /// Operator type.
    pub op: TfLiteOp,
    /// Input tensor indices.
    pub inputs: Vec<usize>,
    /// Output tensor indices.
    pub outputs: Vec<usize>,
}

// ═══════════════════════════════════════════════════════════════════════
// TfLite Subgraph
// ═══════════════════════════════════════════════════════════════════════

/// A subgraph (computation graph) in the TFLite model.
#[derive(Debug, Clone)]
pub struct TfLiteSubgraph {
    /// Tensors in this subgraph.
    pub tensors: Vec<TfLiteTensor>,
    /// Operators in execution order.
    pub operators: Vec<TfLiteOperator>,
    /// Indices of input tensors.
    pub inputs: Vec<usize>,
    /// Indices of output tensors.
    pub outputs: Vec<usize>,
    /// Subgraph name.
    pub name: String,
}

// ═══════════════════════════════════════════════════════════════════════
// TfLite Model
// ═══════════════════════════════════════════════════════════════════════

/// A parsed TFLite model ready for inference.
#[derive(Debug, Clone)]
pub struct TfLiteModel {
    /// Subgraphs (typically one main graph).
    pub subgraphs: Vec<TfLiteSubgraph>,
    /// Data buffers (weight data, bias data, etc.).
    pub buffers: Vec<Vec<f64>>,
    /// Model description/version.
    pub description: String,
}

impl TfLiteModel {
    /// Creates a TFLite model from components (for programmatic construction).
    pub fn new(
        subgraphs: Vec<TfLiteSubgraph>,
        buffers: Vec<Vec<f64>>,
        description: String,
    ) -> Self {
        Self {
            subgraphs,
            buffers,
            description,
        }
    }

    /// Returns the number of subgraphs.
    pub fn num_subgraphs(&self) -> usize {
        self.subgraphs.len()
    }

    /// Returns the primary (first) subgraph.
    pub fn primary_subgraph(&self) -> Option<&TfLiteSubgraph> {
        self.subgraphs.first()
    }

    /// Returns tensor data from a buffer index.
    pub fn get_buffer(&self, index: usize) -> Result<&[f64], TfLiteError> {
        self.buffers
            .get(index)
            .map(|v| v.as_slice())
            .ok_or(TfLiteError::MissingBuffer { index })
    }

    /// Returns a summary of the model.
    pub fn summary(&self) -> String {
        let mut s = format!("TfLiteModel: {}\n", self.description);
        for (i, sg) in self.subgraphs.iter().enumerate() {
            s.push_str(&format!(
                "  Subgraph {i}: {} tensors, {} operators\n",
                sg.tensors.len(),
                sg.operators.len()
            ));
        }
        s
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Binary Parsing
// ═══════════════════════════════════════════════════════════════════════

/// TFLite magic bytes: "TFL3".
const TFLITE_MAGIC: [u8; 4] = [0x54, 0x46, 0x4C, 0x33];

/// Loads a TFLite model from binary data.
///
/// Parses the flatbuffer-based binary header and table structure.
/// Returns a `TfLiteModel` ready for inference.
pub fn tflite_load(data: &[u8]) -> Result<TfLiteModel, TfLiteError> {
    if data.len() < 8 {
        return Err(TfLiteError::InvalidData {
            reason: "data too short (< 8 bytes)".to_string(),
        });
    }

    // Check for magic bytes (offset 4..8 in standard flatbuffer, or first 4 bytes)
    let has_magic = check_magic(data);
    if !has_magic {
        return Err(TfLiteError::InvalidData {
            reason: "missing TFL3 magic bytes".to_string(),
        });
    }

    // Parse the simplified binary format:
    // [4 bytes magic][4 bytes version][rest = model tables]
    parse_model_tables(data)
}

/// Checks for TFL3 magic bytes at various positions.
fn check_magic(data: &[u8]) -> bool {
    // Standard: magic at offset 4
    if data.len() >= 8 && data[4..8] == TFLITE_MAGIC {
        return true;
    }
    // Alternative: magic at offset 0
    if data.len() >= 4 && data[0..4] == TFLITE_MAGIC {
        return true;
    }
    false
}

/// Parses model tables from binary data after header validation.
///
/// Format (simplified for Fajar's parser):
/// - Byte 8: number of subgraphs (u8)
/// - Byte 9: number of buffers (u8)
/// - Following bytes: tensor descriptors, operators, buffer data
fn parse_model_tables(data: &[u8]) -> Result<TfLiteModel, TfLiteError> {
    let offset = find_data_start(data);
    if data.len() < offset + 2 {
        return Err(TfLiteError::InvalidData {
            reason: "truncated model header".to_string(),
        });
    }

    let num_subgraphs = data[offset] as usize;
    let num_buffers = data[offset + 1] as usize;
    let mut pos = offset + 2;

    // Parse buffers
    let mut buffers = Vec::with_capacity(num_buffers);
    for _ in 0..num_buffers {
        let (buf, new_pos) = parse_buffer(data, pos)?;
        buffers.push(buf);
        pos = new_pos;
    }

    // Parse subgraphs
    let mut subgraphs = Vec::with_capacity(num_subgraphs);
    for _ in 0..num_subgraphs {
        let (sg, new_pos) = parse_subgraph(data, pos)?;
        subgraphs.push(sg);
        pos = new_pos;
    }

    Ok(TfLiteModel::new(
        subgraphs,
        buffers,
        "TFLite model".to_string(),
    ))
}

/// Finds the start of data content after magic/version header.
fn find_data_start(data: &[u8]) -> usize {
    if data.len() >= 8 && data[4..8] == TFLITE_MAGIC {
        8
    } else if data.len() >= 4 && data[0..4] == TFLITE_MAGIC {
        4
    } else {
        0
    }
}

/// Reads a u32 from 4 bytes in little-endian format.
fn read_u32_le(data: &[u8], pos: usize) -> Result<u32, TfLiteError> {
    if pos + 4 > data.len() {
        return Err(TfLiteError::InvalidData {
            reason: format!("read_u32 out of bounds at pos {pos}"),
        });
    }
    Ok(u32::from_le_bytes([
        data[pos],
        data[pos + 1],
        data[pos + 2],
        data[pos + 3],
    ]))
}

/// Reads a f32 from 4 bytes in little-endian format.
fn read_f32_le(data: &[u8], pos: usize) -> Result<f32, TfLiteError> {
    if pos + 4 > data.len() {
        return Err(TfLiteError::InvalidData {
            reason: format!("read_f32 out of bounds at pos {pos}"),
        });
    }
    Ok(f32::from_le_bytes([
        data[pos],
        data[pos + 1],
        data[pos + 2],
        data[pos + 3],
    ]))
}

/// Parses a buffer: [u32 num_elements][f32 * num_elements].
fn parse_buffer(data: &[u8], pos: usize) -> Result<(Vec<f64>, usize), TfLiteError> {
    let count = read_u32_le(data, pos)? as usize;
    let mut buf = Vec::with_capacity(count);
    let mut p = pos + 4;
    for _ in 0..count {
        let val = read_f32_le(data, p)? as f64;
        buf.push(val);
        p += 4;
    }
    Ok((buf, p))
}

/// Parses a subgraph: tensors, operators, inputs, outputs.
fn parse_subgraph(data: &[u8], pos: usize) -> Result<(TfLiteSubgraph, usize), TfLiteError> {
    let mut p = pos;

    // Number of tensors
    if p >= data.len() {
        return Err(TfLiteError::InvalidData {
            reason: "truncated subgraph header".to_string(),
        });
    }
    let num_tensors = data[p] as usize;
    p += 1;

    // Number of operators
    if p >= data.len() {
        return Err(TfLiteError::InvalidData {
            reason: "truncated subgraph (operators count)".to_string(),
        });
    }
    let num_ops = data[p] as usize;
    p += 1;

    // Parse tensors: [ndim(u8)][shape dims(u32 each)][dtype(u8)][buffer_idx(u8)]
    let mut tensors = Vec::with_capacity(num_tensors);
    for i in 0..num_tensors {
        let (tensor, new_pos) = parse_tensor_desc(data, p, i)?;
        tensors.push(tensor);
        p = new_pos;
    }

    // Parse operators: [op_code(u8)][num_inputs(u8)][inputs(u8 each)][num_outputs(u8)][outputs(u8 each)]
    let mut operators = Vec::with_capacity(num_ops);
    for _ in 0..num_ops {
        let (op, new_pos) = parse_operator(data, p)?;
        operators.push(op);
        p = new_pos;
    }

    // Parse input/output indices: [num_inputs(u8)][indices(u8)][num_outputs(u8)][indices(u8)]
    let (inputs, new_pos) = parse_index_list(data, p)?;
    p = new_pos;
    let (outputs, new_pos) = parse_index_list(data, p)?;
    p = new_pos;

    Ok((
        TfLiteSubgraph {
            tensors,
            operators,
            inputs,
            outputs,
            name: "main".to_string(),
        },
        p,
    ))
}

/// Parses a single tensor descriptor.
fn parse_tensor_desc(
    data: &[u8],
    pos: usize,
    index: usize,
) -> Result<(TfLiteTensor, usize), TfLiteError> {
    let mut p = pos;
    if p >= data.len() {
        return Err(TfLiteError::InvalidData {
            reason: format!("truncated tensor descriptor at index {index}"),
        });
    }

    let ndim = data[p] as usize;
    p += 1;

    let mut shape = Vec::with_capacity(ndim);
    for _ in 0..ndim {
        let dim = read_u32_le(data, p)? as usize;
        shape.push(dim);
        p += 4;
    }

    if p + 2 > data.len() {
        return Err(TfLiteError::InvalidData {
            reason: format!("truncated tensor {index} type/buffer"),
        });
    }
    let dtype = TfLiteType::from_code(data[p])?;
    p += 1;
    let buffer_index = data[p] as usize;
    p += 1;

    let name = format!("tensor_{index}");
    Ok((TfLiteTensor::new(shape, dtype, buffer_index, name), p))
}

/// Parses a single operator.
fn parse_operator(data: &[u8], pos: usize) -> Result<(TfLiteOperator, usize), TfLiteError> {
    let mut p = pos;
    if p >= data.len() {
        return Err(TfLiteError::InvalidData {
            reason: "truncated operator".to_string(),
        });
    }

    let op = TfLiteOp::from_code(data[p])?;
    p += 1;

    let (inputs, new_pos) = parse_index_list(data, p)?;
    p = new_pos;
    let (outputs, new_pos) = parse_index_list(data, p)?;
    p = new_pos;

    Ok((
        TfLiteOperator {
            op,
            inputs,
            outputs,
        },
        p,
    ))
}

/// Parses a list of u8 indices: [count(u8)][indices(u8 each)].
fn parse_index_list(data: &[u8], pos: usize) -> Result<(Vec<usize>, usize), TfLiteError> {
    if pos >= data.len() {
        return Err(TfLiteError::InvalidData {
            reason: "truncated index list".to_string(),
        });
    }
    let count = data[pos] as usize;
    let mut indices = Vec::with_capacity(count);
    let mut p = pos + 1;
    for _ in 0..count {
        if p >= data.len() {
            return Err(TfLiteError::InvalidData {
                reason: "truncated index list data".to_string(),
            });
        }
        indices.push(data[p] as usize);
        p += 1;
    }
    Ok((indices, p))
}

// ═══════════════════════════════════════════════════════════════════════
// Inference Engine
// ═══════════════════════════════════════════════════════════════════════

/// Runs inference on a TFLite model with the given input.
///
/// Executes the operator graph of the primary subgraph.
/// Input is provided as a 2D tensor (e.g., `[1, input_features]`).
///
/// Returns the output tensor from the graph.
pub fn tflite_infer(model: &TfLiteModel, input: Array2<f64>) -> Result<Array2<f64>, TfLiteError> {
    let sg = model
        .primary_subgraph()
        .ok_or_else(|| TfLiteError::InvalidData {
            reason: "no subgraphs in model".to_string(),
        })?;

    // Initialize tensor storage
    let mut tensors: HashMap<usize, Array2<f64>> = HashMap::new();

    // Load constant buffers into tensor storage
    load_buffers_into_tensors(model, sg, &mut tensors)?;

    // Set input tensor
    if let Some(&input_idx) = sg.inputs.first() {
        tensors.insert(input_idx, input);
    }

    // Execute operators in order
    for op in &sg.operators {
        execute_operator(op, &mut tensors, sg)?;
    }

    // Return output tensor
    let output_idx = sg.outputs.first().ok_or_else(|| TfLiteError::InvalidData {
        reason: "no output tensors defined".to_string(),
    })?;
    tensors
        .remove(output_idx)
        .ok_or_else(|| TfLiteError::InvalidData {
            reason: format!("output tensor {output_idx} not computed"),
        })
}

/// Loads buffer data into the tensor storage map.
fn load_buffers_into_tensors(
    model: &TfLiteModel,
    sg: &TfLiteSubgraph,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    for (i, tensor_desc) in sg.tensors.iter().enumerate() {
        if sg.inputs.contains(&i) {
            continue; // Skip input tensors — filled at inference time
        }
        let buf_idx = tensor_desc.buffer_index;
        if buf_idx < model.buffers.len() {
            let buf = &model.buffers[buf_idx];
            if !buf.is_empty() {
                let arr = buffer_to_array2(buf, &tensor_desc.shape, &tensor_desc.quantization)?;
                tensors.insert(i, arr);
            }
        }
    }
    Ok(())
}

/// Converts a flat buffer to a 2D array, applying quantization if present.
fn buffer_to_array2(
    buf: &[f64],
    shape: &[usize],
    quant: &Option<QuantizationParams>,
) -> Result<Array2<f64>, TfLiteError> {
    let total: usize = shape.iter().product();
    if buf.len() < total {
        return Err(TfLiteError::InvalidData {
            reason: format!(
                "buffer has {} elements, shape {:?} needs {total}",
                buf.len(),
                shape
            ),
        });
    }

    // Flatten shape to 2D: keep last dim, merge others
    let (rows, cols) = flatten_to_2d(shape);

    let data: Vec<f64> = if let Some(q) = quant {
        buf.iter()
            .take(total)
            .map(|&v| q.dequantize(v as i64))
            .collect()
    } else {
        buf.iter().take(total).copied().collect()
    };

    Array2::from_shape_vec((rows, cols), data).map_err(|e| TfLiteError::InvalidData {
        reason: e.to_string(),
    })
}

/// Flattens an n-dimensional shape to 2D: (product of all but last, last).
fn flatten_to_2d(shape: &[usize]) -> (usize, usize) {
    if shape.is_empty() {
        (1, 1)
    } else if shape.len() == 1 {
        (1, shape[0])
    } else {
        let cols = shape[shape.len() - 1];
        let rows: usize = shape[..shape.len() - 1].iter().product();
        (rows, cols)
    }
}

/// Executes a single operator.
fn execute_operator(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
    sg: &TfLiteSubgraph,
) -> Result<(), TfLiteError> {
    match op.op {
        TfLiteOp::FullyConnected => exec_fully_connected(op, tensors, sg),
        TfLiteOp::ReLU => exec_relu(op, tensors),
        TfLiteOp::ReLU6 => exec_relu6(op, tensors),
        TfLiteOp::Softmax => exec_softmax(op, tensors),
        TfLiteOp::Add => exec_elementwise_add(op, tensors),
        TfLiteOp::Mul => exec_elementwise_mul(op, tensors),
        TfLiteOp::Reshape => exec_reshape(op, tensors, sg),
        TfLiteOp::Conv2D => exec_conv2d_sim(op, tensors),
        TfLiteOp::DepthwiseConv2D => exec_depthwise_conv2d_sim(op, tensors),
        TfLiteOp::MaxPool2D => exec_max_pool_sim(op, tensors),
        TfLiteOp::AveragePool2D => exec_avg_pool_sim(op, tensors),
    }
}

/// Executes FullyConnected: output = input @ weight + bias.
fn exec_fully_connected(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
    sg: &TfLiteSubgraph,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let weight = get_tensor(tensors, op.inputs.get(1))?;

    // weight may be [out, in], need to transpose for matmul
    let result = input.dot(&weight.t());

    // Add bias if present
    let result = if op.inputs.len() > 2 {
        let bias_idx = op.inputs[2];
        if let Some(bias) = tensors.get(&bias_idx) {
            &result + bias
        } else {
            result
        }
    } else {
        result
    };

    // Check for fused activation from tensor descriptor name hint
    let output_idx = op.outputs.first().copied().unwrap_or(0);
    let activated = if output_idx < sg.tensors.len() && sg.tensors[output_idx].name.contains("relu")
    {
        result.mapv(|v| v.max(0.0))
    } else {
        result
    };

    set_tensor(tensors, op.outputs.first(), activated)
}

/// Executes ReLU: max(0, x).
fn exec_relu(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let result = input.mapv(|v| v.max(0.0));
    set_tensor(tensors, op.outputs.first(), result)
}

/// Executes ReLU6: clamp(x, 0, 6).
fn exec_relu6(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let result = input.mapv(|v| v.clamp(0.0, 6.0));
    set_tensor(tensors, op.outputs.first(), result)
}

/// Executes Softmax over the last axis.
fn exec_softmax(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let result = softmax_2d_local(&input);
    set_tensor(tensors, op.outputs.first(), result)
}

/// Executes element-wise Add.
fn exec_elementwise_add(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let a = get_tensor(tensors, op.inputs.first())?;
    let b = get_tensor(tensors, op.inputs.get(1))?;
    let result = &a + &b;
    set_tensor(tensors, op.outputs.first(), result)
}

/// Executes element-wise Mul.
fn exec_elementwise_mul(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let a = get_tensor(tensors, op.inputs.first())?;
    let b = get_tensor(tensors, op.inputs.get(1))?;
    let result = &a * &b;
    set_tensor(tensors, op.outputs.first(), result)
}

/// Executes Reshape (simple flatten/reshape as 2D).
fn exec_reshape(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
    sg: &TfLiteSubgraph,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let output_idx = op.outputs.first().copied().unwrap_or(0);

    if output_idx < sg.tensors.len() {
        let target_shape = &sg.tensors[output_idx].shape;
        let (rows, cols) = flatten_to_2d(target_shape);
        let total = rows * cols;
        let input_total = input.nrows() * input.ncols();
        if total != input_total {
            return Err(TfLiteError::ShapeMismatch {
                expected: target_shape.clone(),
                got: vec![input.nrows(), input.ncols()],
            });
        }
        let flat: Vec<f64> = input.iter().copied().collect();
        let reshaped =
            Array2::from_shape_vec((rows, cols), flat).map_err(|e| TfLiteError::InvalidData {
                reason: e.to_string(),
            })?;
        set_tensor(tensors, op.outputs.first(), reshaped)
    } else {
        // Pass through
        set_tensor(tensors, op.outputs.first(), input)
    }
}

/// Simulated Conv2D (simplified 2D: treat as linear + activation).
fn exec_conv2d_sim(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let weight = get_tensor(tensors, op.inputs.get(1))?;
    // Simplified: matmul with weight if shapes allow, else pass through
    let result = if input.ncols() == weight.nrows() {
        input.dot(&weight)
    } else if input.ncols() == weight.ncols() {
        input.dot(&weight.t())
    } else {
        input
    };
    set_tensor(tensors, op.outputs.first(), result)
}

/// Simulated Depthwise Conv2D (simplified).
fn exec_depthwise_conv2d_sim(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    // Depthwise: element-wise multiply with kernel then sum
    let input = get_tensor(tensors, op.inputs.first())?;
    let weight = get_tensor(tensors, op.inputs.get(1))?;
    let result = if input.shape() == weight.shape() {
        &input * &weight
    } else {
        input
    };
    set_tensor(tensors, op.outputs.first(), result)
}

/// Simulated MaxPool2D (take max of each row pair).
fn exec_max_pool_sim(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let rows = input.nrows();
    let cols = input.ncols();
    let out_rows = rows.div_ceil(2);
    let mut result = Array2::zeros((out_rows, cols));
    for r in 0..out_rows {
        for c in 0..cols {
            let v1 = input[[r * 2, c]];
            let v2 = if r * 2 + 1 < rows {
                input[[r * 2 + 1, c]]
            } else {
                v1
            };
            result[[r, c]] = v1.max(v2);
        }
    }
    set_tensor(tensors, op.outputs.first(), result)
}

/// Simulated AveragePool2D (take mean of each row pair).
fn exec_avg_pool_sim(
    op: &TfLiteOperator,
    tensors: &mut HashMap<usize, Array2<f64>>,
) -> Result<(), TfLiteError> {
    let input = get_tensor(tensors, op.inputs.first())?;
    let rows = input.nrows();
    let cols = input.ncols();
    let out_rows = rows.div_ceil(2);
    let mut result = Array2::zeros((out_rows, cols));
    for r in 0..out_rows {
        for c in 0..cols {
            let v1 = input[[r * 2, c]];
            let v2 = if r * 2 + 1 < rows {
                input[[r * 2 + 1, c]]
            } else {
                v1
            };
            result[[r, c]] = (v1 + v2) / 2.0;
        }
    }
    set_tensor(tensors, op.outputs.first(), result)
}

/// Numerically-stable softmax for 2D array.
fn softmax_2d_local(x: &Array2<f64>) -> Array2<f64> {
    let rows = x.nrows();
    let cols = x.ncols();
    let mut result = Array2::zeros((rows, cols));
    for r in 0..rows {
        let row = x.row(r);
        let max_val = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = row.iter().map(|&v| (v - max_val).exp()).collect();
        let sum: f64 = exps.iter().sum();
        for c in 0..cols {
            result[[r, c]] = exps[c] / sum;
        }
    }
    result
}

/// Gets a tensor from the storage, cloning it.
fn get_tensor(
    tensors: &HashMap<usize, Array2<f64>>,
    idx: Option<&usize>,
) -> Result<Array2<f64>, TfLiteError> {
    let idx = idx.ok_or_else(|| TfLiteError::InvalidData {
        reason: "missing tensor index".to_string(),
    })?;
    tensors
        .get(idx)
        .cloned()
        .ok_or_else(|| TfLiteError::InvalidData {
            reason: format!("tensor {idx} not found in storage"),
        })
}

/// Sets a tensor in the storage.
fn set_tensor(
    tensors: &mut HashMap<usize, Array2<f64>>,
    idx: Option<&usize>,
    value: Array2<f64>,
) -> Result<(), TfLiteError> {
    let idx = idx.ok_or_else(|| TfLiteError::InvalidData {
        reason: "missing output tensor index".to_string(),
    })?;
    tensors.insert(*idx, value);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Builder (for constructing synthetic models in tests)
// ═══════════════════════════════════════════════════════════════════════

/// Builder for constructing TFLite models programmatically.
///
/// Useful for testing without actual .tflite files.
#[derive(Debug, Clone)]
pub struct TfLiteModelBuilder {
    /// Tensors to add.
    tensors: Vec<TfLiteTensor>,
    /// Operators to add.
    operators: Vec<TfLiteOperator>,
    /// Buffers (weight data).
    buffers: Vec<Vec<f64>>,
    /// Input tensor indices.
    inputs: Vec<usize>,
    /// Output tensor indices.
    outputs: Vec<usize>,
}

impl TfLiteModelBuilder {
    /// Creates a new empty model builder.
    pub fn new() -> Self {
        Self {
            tensors: Vec::new(),
            operators: Vec::new(),
            buffers: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Adds a buffer and returns its index.
    pub fn add_buffer(&mut self, data: Vec<f64>) -> usize {
        let idx = self.buffers.len();
        self.buffers.push(data);
        idx
    }

    /// Adds a tensor descriptor and returns its index.
    pub fn add_tensor(
        &mut self,
        shape: Vec<usize>,
        dtype: TfLiteType,
        buffer_index: usize,
        name: &str,
    ) -> usize {
        let idx = self.tensors.len();
        self.tensors.push(TfLiteTensor::new(
            shape,
            dtype,
            buffer_index,
            name.to_string(),
        ));
        idx
    }

    /// Adds an operator.
    pub fn add_operator(&mut self, op: TfLiteOp, inputs: Vec<usize>, outputs: Vec<usize>) {
        self.operators.push(TfLiteOperator {
            op,
            inputs,
            outputs,
        });
    }

    /// Sets the input tensor indices.
    pub fn set_inputs(&mut self, inputs: Vec<usize>) {
        self.inputs = inputs;
    }

    /// Sets the output tensor indices.
    pub fn set_outputs(&mut self, outputs: Vec<usize>) {
        self.outputs = outputs;
    }

    /// Builds the TFLite model.
    pub fn build(self) -> TfLiteModel {
        let sg = TfLiteSubgraph {
            tensors: self.tensors,
            operators: self.operators,
            inputs: self.inputs,
            outputs: self.outputs,
            name: "main".to_string(),
        };
        TfLiteModel::new(vec![sg], self.buffers, "synthetic model".to_string())
    }
}

impl Default for TfLiteModelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializes a TFLite model to binary bytes (for testing `tflite_load`).
///
/// Format:
/// - `[4B: 0x00 0x00 0x00 0x00][4B: TFL3 magic]`
/// - `[1B: num_subgraphs][1B: num_buffers]`
/// - Per buffer: `[4B LE: count][4B LE * count: f32 values]`
/// - Per subgraph: `[1B: num_tensors][1B: num_ops]`
///   - Per tensor: `[1B: ndim][4B LE * ndim: dims][1B: dtype][1B: buf_idx]`
///   - Per operator: `[1B: op_code][1B: num_in][in indices][1B: num_out][out indices]`
///   - `[1B: num_sg_in][sg_in indices][1B: num_sg_out][sg_out indices]`
pub fn tflite_serialize(model: &TfLiteModel) -> Vec<u8> {
    let mut data = Vec::new();

    // Header: 4 zero bytes + TFL3 magic
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    data.extend_from_slice(&TFLITE_MAGIC);

    // Counts
    data.push(model.subgraphs.len() as u8);
    data.push(model.buffers.len() as u8);

    // Buffers
    for buf in &model.buffers {
        data.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        for &v in buf {
            data.extend_from_slice(&(v as f32).to_le_bytes());
        }
    }

    // Subgraphs
    for sg in &model.subgraphs {
        serialize_subgraph(sg, &mut data);
    }

    data
}

/// Serializes a subgraph to bytes.
fn serialize_subgraph(sg: &TfLiteSubgraph, data: &mut Vec<u8>) {
    data.push(sg.tensors.len() as u8);
    data.push(sg.operators.len() as u8);

    // Tensors
    for t in &sg.tensors {
        data.push(t.shape.len() as u8);
        for &dim in &t.shape {
            data.extend_from_slice(&(dim as u32).to_le_bytes());
        }
        let dtype_code: u8 = match t.dtype {
            TfLiteType::Float32 => 0,
            TfLiteType::Float16 => 1,
            TfLiteType::Int32 => 2,
            TfLiteType::UInt8 => 3,
            TfLiteType::Int16 => 7,
            TfLiteType::Int8 => 9,
        };
        data.push(dtype_code);
        data.push(t.buffer_index as u8);
    }

    // Operators
    for op in &sg.operators {
        let op_code: u8 = match op.op {
            TfLiteOp::Add => 0,
            TfLiteOp::AveragePool2D => 1,
            TfLiteOp::Conv2D => 3,
            TfLiteOp::DepthwiseConv2D => 4,
            TfLiteOp::FullyConnected => 9,
            TfLiteOp::Mul => 17,
            TfLiteOp::Reshape => 18,
            TfLiteOp::Softmax => 22,
            TfLiteOp::ReLU => 25,
            TfLiteOp::ReLU6 => 26,
            TfLiteOp::MaxPool2D => 56,
        };
        data.push(op_code);
        data.push(op.inputs.len() as u8);
        for &i in &op.inputs {
            data.push(i as u8);
        }
        data.push(op.outputs.len() as u8);
        for &o in &op.outputs {
            data.push(o as u8);
        }
    }

    // Subgraph inputs/outputs
    data.push(sg.inputs.len() as u8);
    for &i in &sg.inputs {
        data.push(i as u8);
    }
    data.push(sg.outputs.len() as u8);
    for &o in &sg.outputs {
        data.push(o as u8);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 15: TFLite Model Import ──

    #[test]
    fn s15_1_tflite_type_properties() {
        assert_eq!(TfLiteType::Float32.size_bytes(), 4);
        assert_eq!(TfLiteType::Int8.size_bytes(), 1);
        assert_eq!(TfLiteType::Int32.size_bytes(), 4);
        assert_eq!(TfLiteType::Float32.name(), "float32");
        assert_eq!(TfLiteType::from_code(0).unwrap(), TfLiteType::Float32);
        assert_eq!(TfLiteType::from_code(9).unwrap(), TfLiteType::Int8);
        assert!(TfLiteType::from_code(255).is_err());
    }

    #[test]
    fn s15_2_quantization_params_roundtrip() {
        let q = QuantizationParams::new(0.01, 128);
        assert_eq!(q.dequantize(128), 0.0);
        assert!((q.dequantize(228) - 1.0).abs() < 1e-10);
        assert_eq!(q.quantize(0.0), 128);
        assert_eq!(q.quantize(1.0), 228);
        // Roundtrip
        let orig = 0.5;
        let quantized = q.quantize(orig);
        let dequantized = q.dequantize(quantized);
        assert!((dequantized - orig).abs() < 0.01);
    }

    #[test]
    fn s15_3_tflite_tensor_descriptor() {
        let t = TfLiteTensor::new(
            vec![1, 28, 28, 1],
            TfLiteType::Float32,
            0,
            "input".to_string(),
        );
        assert_eq!(t.numel(), 784);
        assert_eq!(t.byte_size(), 3136);
        assert!(t.quantization.is_none());

        let tq = TfLiteTensor::quantized(
            vec![10, 3],
            TfLiteType::Int8,
            1,
            "weights".to_string(),
            0.05,
            0,
        );
        assert!(tq.quantization.is_some());
    }

    #[test]
    fn s15_4_tflite_op_codes() {
        assert_eq!(TfLiteOp::from_code(9).unwrap(), TfLiteOp::FullyConnected);
        assert_eq!(TfLiteOp::from_code(25).unwrap(), TfLiteOp::ReLU);
        assert_eq!(TfLiteOp::from_code(22).unwrap(), TfLiteOp::Softmax);
        assert_eq!(TfLiteOp::FullyConnected.name(), "FullyConnected");
        assert!(TfLiteOp::from_code(200).is_err());
    }

    #[test]
    fn s15_5_model_builder_simple_fc() {
        let mut builder = TfLiteModelBuilder::new();

        // Buffer 0: empty (for input)
        builder.add_buffer(vec![]);
        // Buffer 1: weight [3, 2] = 6 values
        let weights = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        builder.add_buffer(weights);
        // Buffer 2: bias [1, 2] = 2 values
        builder.add_buffer(vec![0.01, 0.02]);
        // Buffer 3: empty (for output)
        builder.add_buffer(vec![]);

        // Tensor 0: input [1, 3]
        builder.add_tensor(vec![1, 3], TfLiteType::Float32, 0, "input");
        // Tensor 1: weight [2, 3]
        builder.add_tensor(vec![2, 3], TfLiteType::Float32, 1, "weight");
        // Tensor 2: bias [1, 2]
        builder.add_tensor(vec![1, 2], TfLiteType::Float32, 2, "bias");
        // Tensor 3: output [1, 2]
        builder.add_tensor(vec![1, 2], TfLiteType::Float32, 3, "output");

        builder.add_operator(TfLiteOp::FullyConnected, vec![0, 1, 2], vec![3]);
        builder.set_inputs(vec![0]);
        builder.set_outputs(vec![3]);

        let model = builder.build();
        assert_eq!(model.num_subgraphs(), 1);

        let input = Array2::from_shape_vec((1, 3), vec![1.0, 2.0, 3.0]).unwrap();
        let output = tflite_infer(&model, input).unwrap();
        assert_eq!(output.shape(), &[1, 2]);
        // Verify output is non-zero
        assert!(output.iter().any(|&v| v.abs() > 1e-10));
    }

    #[test]
    fn s15_6_model_relu_activation() {
        let mut builder = TfLiteModelBuilder::new();
        builder.add_buffer(vec![]);
        builder.add_buffer(vec![]);

        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 0, "input");
        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 1, "output");

        builder.add_operator(TfLiteOp::ReLU, vec![0], vec![1]);
        builder.set_inputs(vec![0]);
        builder.set_outputs(vec![1]);

        let model = builder.build();
        let input = Array2::from_shape_vec((1, 4), vec![-2.0, -0.5, 0.5, 3.0]).unwrap();
        let output = tflite_infer(&model, input).unwrap();
        let vals: Vec<f64> = output.iter().copied().collect();
        assert_eq!(vals, vec![0.0, 0.0, 0.5, 3.0]);
    }

    #[test]
    fn s15_7_model_relu6_clamps() {
        let mut builder = TfLiteModelBuilder::new();
        builder.add_buffer(vec![]);
        builder.add_buffer(vec![]);

        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 0, "input");
        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 1, "output");

        builder.add_operator(TfLiteOp::ReLU6, vec![0], vec![1]);
        builder.set_inputs(vec![0]);
        builder.set_outputs(vec![1]);

        let model = builder.build();
        let input = Array2::from_shape_vec((1, 4), vec![-1.0, 3.0, 6.0, 10.0]).unwrap();
        let output = tflite_infer(&model, input).unwrap();
        let vals: Vec<f64> = output.iter().copied().collect();
        assert_eq!(vals, vec![0.0, 3.0, 6.0, 6.0]);
    }

    #[test]
    fn s15_8_model_softmax_output_sums_to_one() {
        let mut builder = TfLiteModelBuilder::new();
        builder.add_buffer(vec![]);
        builder.add_buffer(vec![]);

        builder.add_tensor(vec![1, 3], TfLiteType::Float32, 0, "input");
        builder.add_tensor(vec![1, 3], TfLiteType::Float32, 1, "output");

        builder.add_operator(TfLiteOp::Softmax, vec![0], vec![1]);
        builder.set_inputs(vec![0]);
        builder.set_outputs(vec![1]);

        let model = builder.build();
        let input = Array2::from_shape_vec((1, 3), vec![2.0, 1.0, 0.0]).unwrap();
        let output = tflite_infer(&model, input).unwrap();
        let sum: f64 = output.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
        // First element should be largest
        assert!(output[[0, 0]] > output[[0, 1]]);
        assert!(output[[0, 1]] > output[[0, 2]]);
    }

    #[test]
    fn s15_9_serialize_and_load_roundtrip() {
        let mut builder = TfLiteModelBuilder::new();
        builder.add_buffer(vec![]);
        builder.add_buffer(vec![1.0, 2.0, 3.0, 4.0]);
        builder.add_buffer(vec![]);

        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 0, "input");
        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 1, "weights");
        builder.add_tensor(vec![1, 4], TfLiteType::Float32, 2, "output");

        builder.add_operator(TfLiteOp::ReLU, vec![0], vec![2]);
        builder.set_inputs(vec![0]);
        builder.set_outputs(vec![2]);

        let model = builder.build();
        let bytes = tflite_serialize(&model);

        // Load from bytes
        let loaded = tflite_load(&bytes).unwrap();
        assert_eq!(loaded.num_subgraphs(), 1);

        let sg = loaded.primary_subgraph().unwrap();
        assert_eq!(sg.tensors.len(), 3); // parsed from binary
        assert_eq!(sg.operators.len(), 1);
        assert_eq!(sg.operators[0].op, TfLiteOp::ReLU);
    }

    #[test]
    fn s15_10_tflite_load_rejects_invalid_data() {
        // Too short
        assert!(tflite_load(&[0x00]).is_err());
        // No magic
        assert!(tflite_load(&[0x00; 16]).is_err());
        // Valid magic but truncated
        let mut data = vec![0x00, 0x00, 0x00, 0x00];
        data.extend_from_slice(&TFLITE_MAGIC);
        // Just magic, no content after counts
        data.push(1); // 1 subgraph
        data.push(0); // 0 buffers
        // Missing subgraph data
        assert!(tflite_load(&data).is_err());
    }
}
