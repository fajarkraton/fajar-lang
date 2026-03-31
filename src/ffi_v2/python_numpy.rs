//! Python NumPy/PyTorch Bridge — zero-copy tensor interop, model loading, ONNX export.
//!
//! Sprint E5: 10 tasks covering zero-copy NumPy<->Tensor transfer, PyTorch tensor
//! bridging with device tracking, dtype mapping, shape/stride handling, model loading,
//! mixed-framework training, ONNX export, and batch processing.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// E5.4: NumPy dtype Mapping (defined first, used throughout)
// ═══════════════════════════════════════════════════════════════════════

/// NumPy data type with full dtype coverage (12 types).
///
/// Maps between NumPy dtype strings, Fajar type names, and byte sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumpyDtype {
    /// 32-bit float (`numpy.float32`).
    Float32,
    /// 64-bit float (`numpy.float64`).
    Float64,
    /// 16-bit float (`numpy.float16`).
    Float16,
    /// 8-bit signed integer (`numpy.int8`).
    Int8,
    /// 16-bit signed integer (`numpy.int16`).
    Int16,
    /// 32-bit signed integer (`numpy.int32`).
    Int32,
    /// 64-bit signed integer (`numpy.int64`).
    Int64,
    /// 8-bit unsigned integer (`numpy.uint8`).
    Uint8,
    /// 16-bit unsigned integer (`numpy.uint16`).
    Uint16,
    /// 32-bit unsigned integer (`numpy.uint32`).
    Uint32,
    /// 64-bit unsigned integer (`numpy.uint64`).
    Uint64,
    /// Boolean (`numpy.bool_`).
    Bool,
}

impl NumpyDtype {
    /// Returns the byte size per element.
    pub fn element_size(self) -> usize {
        match self {
            Self::Float16 => 2,
            Self::Float32 => 4,
            Self::Float64 => 8,
            Self::Int8 | Self::Uint8 | Self::Bool => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 => 4,
            Self::Int64 | Self::Uint64 => 8,
        }
    }

    /// Maps to the Fajar Lang type name.
    pub fn to_fajar_type(self) -> &'static str {
        match self {
            Self::Float16 => "f16",
            Self::Float32 => "f32",
            Self::Float64 => "f64",
            Self::Int8 => "i8",
            Self::Int16 => "i16",
            Self::Int32 => "i32",
            Self::Int64 => "i64",
            Self::Uint8 => "u8",
            Self::Uint16 => "u16",
            Self::Uint32 => "u32",
            Self::Uint64 => "u64",
            Self::Bool => "bool",
        }
    }

    /// Maps to the NumPy dtype string.
    pub fn to_numpy_str(self) -> &'static str {
        match self {
            Self::Float16 => "float16",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Uint8 => "uint8",
            Self::Uint16 => "uint16",
            Self::Uint32 => "uint32",
            Self::Uint64 => "uint64",
            Self::Bool => "bool",
        }
    }

    /// Parses a NumPy dtype string into a `NumpyDtype`.
    pub fn from_numpy_str(s: &str) -> Option<Self> {
        match s {
            "float16" | "f16" | "<f2" => Some(Self::Float16),
            "float32" | "f32" | "<f4" => Some(Self::Float32),
            "float64" | "f64" | "<f8" => Some(Self::Float64),
            "int8" | "i1" => Some(Self::Int8),
            "int16" | "i16" | "<i2" => Some(Self::Int16),
            "int32" | "i32" | "<i4" => Some(Self::Int32),
            "int64" | "i64" | "<i8" => Some(Self::Int64),
            "uint8" | "u8" => Some(Self::Uint8),
            "uint16" | "u16" | "<u2" => Some(Self::Uint16),
            "uint32" | "u32" | "<u4" => Some(Self::Uint32),
            "uint64" | "u64" | "<u8" => Some(Self::Uint64),
            "bool" | "bool_" => Some(Self::Bool),
            _ => None,
        }
    }

    /// Returns whether this is a floating-point type.
    pub fn is_float(self) -> bool {
        matches!(self, Self::Float16 | Self::Float32 | Self::Float64)
    }

    /// Returns whether this is a signed integer type.
    pub fn is_signed_int(self) -> bool {
        matches!(self, Self::Int8 | Self::Int16 | Self::Int32 | Self::Int64)
    }

    /// Returns whether this is an unsigned integer type.
    pub fn is_unsigned_int(self) -> bool {
        matches!(
            self,
            Self::Uint8 | Self::Uint16 | Self::Uint32 | Self::Uint64
        )
    }
}

impl fmt::Display for NumpyDtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_numpy_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Error Type
// ═══════════════════════════════════════════════════════════════════════

/// Error type for NumPy/PyTorch bridge operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumpyBridgeError {
    /// Shape mismatch between source and destination.
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        actual: Vec<usize>,
    },
    /// Data type is not supported or incompatible.
    DtypeMismatch {
        /// Expected dtype.
        expected: NumpyDtype,
        /// Actual dtype.
        actual: NumpyDtype,
    },
    /// The array is not contiguous in memory.
    NotContiguous {
        /// Description of the layout issue.
        reason: String,
    },
    /// Device mismatch (e.g., trying to share CPU tensor with CUDA).
    DeviceMismatch {
        /// Expected device.
        expected: String,
        /// Actual device.
        actual: String,
    },
    /// The model file could not be loaded.
    ModelLoadError {
        /// Path to the model file.
        path: String,
        /// Error message.
        message: String,
    },
    /// ONNX export error.
    OnnxExportError {
        /// Error message.
        message: String,
    },
    /// Invalid operation.
    InvalidOperation {
        /// Description of what went wrong.
        message: String,
    },
}

impl fmt::Display for NumpyBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShapeMismatch { expected, actual } => {
                write!(f, "shape mismatch: expected {expected:?}, got {actual:?}")
            }
            Self::DtypeMismatch { expected, actual } => {
                write!(f, "dtype mismatch: expected {expected}, got {actual}")
            }
            Self::NotContiguous { reason } => {
                write!(f, "array not contiguous: {reason}")
            }
            Self::DeviceMismatch { expected, actual } => {
                write!(f, "device mismatch: expected {expected}, got {actual}")
            }
            Self::ModelLoadError { path, message } => {
                write!(f, "failed to load model '{path}': {message}")
            }
            Self::OnnxExportError { message } => {
                write!(f, "ONNX export error: {message}")
            }
            Self::InvalidOperation { message } => {
                write!(f, "invalid operation: {message}")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.1: Zero-Copy NumPy -> Tensor
// ═══════════════════════════════════════════════════════════════════════

/// Represents a NumPy array with shared memory buffer.
///
/// In a real implementation, this would hold a pointer to the NumPy
/// array's data buffer (via the buffer protocol). Here the data is
/// stored as `Vec<f64>` simulating the shared memory region.
#[derive(Debug, Clone)]
pub struct NumpyArray {
    /// Array shape (e.g., [3, 224, 224]).
    shape: Vec<usize>,
    /// Element data type.
    dtype: NumpyDtype,
    /// Flattened data buffer (simulated shared memory).
    data: Vec<f64>,
    /// Memory layout.
    layout: ArrayLayout,
    /// Whether this is a view (non-owning) into another array's data.
    is_view: bool,
}

impl NumpyArray {
    /// Creates a new NumPy array from shape, dtype, and data.
    pub fn new(
        shape: Vec<usize>,
        dtype: NumpyDtype,
        data: Vec<f64>,
    ) -> Result<Self, NumpyBridgeError> {
        let expected_numel: usize = shape.iter().product();
        if data.len() != expected_numel {
            return Err(NumpyBridgeError::ShapeMismatch {
                expected: shape,
                actual: vec![data.len()],
            });
        }
        let layout = ArrayLayout::c_contiguous(&shape, dtype);
        Ok(Self {
            shape,
            dtype,
            data,
            layout,
            is_view: false,
        })
    }

    /// Creates a zero-filled array with the given shape and dtype.
    pub fn zeros(shape: Vec<usize>, dtype: NumpyDtype) -> Self {
        let numel: usize = shape.iter().product();
        let layout = ArrayLayout::c_contiguous(&shape, dtype);
        Self {
            shape,
            dtype,
            data: vec![0.0; numel],
            layout,
            is_view: false,
        }
    }

    /// Creates an array filled with ones.
    pub fn ones(shape: Vec<usize>, dtype: NumpyDtype) -> Self {
        let numel: usize = shape.iter().product();
        let layout = ArrayLayout::c_contiguous(&shape, dtype);
        Self {
            shape,
            dtype,
            data: vec![1.0; numel],
            layout,
            is_view: false,
        }
    }

    /// Returns the array shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the number of dimensions.
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Returns the element data type.
    pub fn dtype(&self) -> NumpyDtype {
        self.dtype
    }

    /// Returns the memory layout.
    pub fn layout(&self) -> &ArrayLayout {
        &self.layout
    }

    /// Returns whether this is a view.
    pub fn is_view(&self) -> bool {
        self.is_view
    }

    /// Returns the total size in bytes.
    pub fn nbytes(&self) -> usize {
        self.numel() * self.dtype.element_size()
    }

    /// Returns a read-only reference to the data buffer.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Returns a mutable reference to the data buffer.
    pub fn data_mut(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// Converts to a Fajar Tensor descriptor (zero-copy metadata).
    pub fn to_tensor_descriptor(&self) -> TensorDescriptor {
        TensorDescriptor {
            shape: self.shape.clone(),
            dtype: self.dtype,
            numel: self.numel(),
            nbytes: self.nbytes(),
            strides: self.layout.strides.clone(),
            is_contiguous: self.layout.is_contiguous,
        }
    }

    /// Creates a view of this array (non-owning, shares data).
    pub fn view(&self) -> Self {
        Self {
            shape: self.shape.clone(),
            dtype: self.dtype,
            data: self.data.clone(), // In real impl: shared pointer
            layout: self.layout.clone(),
            is_view: true,
        }
    }

    /// Reshapes the array (must preserve total element count).
    pub fn reshape(&self, new_shape: Vec<usize>) -> Result<Self, NumpyBridgeError> {
        let new_numel: usize = new_shape.iter().product();
        if new_numel != self.numel() {
            return Err(NumpyBridgeError::ShapeMismatch {
                expected: new_shape,
                actual: self.shape.clone(),
            });
        }
        let layout = ArrayLayout::c_contiguous(&new_shape, self.dtype);
        Ok(Self {
            shape: new_shape,
            dtype: self.dtype,
            data: self.data.clone(),
            layout,
            is_view: false,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.2: Zero-Copy Tensor -> NumPy
// ═══════════════════════════════════════════════════════════════════════

/// Descriptor for a Fajar Tensor, used for zero-copy transfer to NumPy.
#[derive(Debug, Clone)]
pub struct TensorDescriptor {
    /// Shape of the tensor.
    pub shape: Vec<usize>,
    /// Data type.
    pub dtype: NumpyDtype,
    /// Total number of elements.
    pub numel: usize,
    /// Total size in bytes.
    pub nbytes: usize,
    /// Strides (in elements).
    pub strides: Vec<usize>,
    /// Whether the tensor data is contiguous in memory.
    pub is_contiguous: bool,
}

/// Converts a Fajar tensor (simulated as shape + data) to a NumPy array.
///
/// Returns a `NumpyArray` view over the data. In a real implementation,
/// this would use the Python buffer protocol to share memory directly.
pub fn to_numpy(
    shape: &[usize],
    dtype: NumpyDtype,
    data: &[f64],
) -> Result<NumpyArray, NumpyBridgeError> {
    let expected: usize = shape.iter().product();
    if data.len() != expected {
        return Err(NumpyBridgeError::ShapeMismatch {
            expected: shape.to_vec(),
            actual: vec![data.len()],
        });
    }
    let layout = ArrayLayout::c_contiguous(shape, dtype);
    Ok(NumpyArray {
        shape: shape.to_vec(),
        dtype,
        data: data.to_vec(),
        layout,
        is_view: true, // returned as a "view" of the Fajar tensor
    })
}

// ═══════════════════════════════════════════════════════════════════════
// E5.3: PyTorch Tensor Bridge
// ═══════════════════════════════════════════════════════════════════════

/// Device on which a PyTorch tensor resides.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TorchDevice {
    /// CPU.
    Cpu,
    /// CUDA GPU with device index.
    Cuda(usize),
    /// MPS (Apple Metal).
    Mps,
}

impl fmt::Display for TorchDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Cuda(idx) => write!(f, "cuda:{idx}"),
            Self::Mps => write!(f, "mps"),
        }
    }
}

impl TorchDevice {
    /// Parses a device string (e.g., "cpu", "cuda:0", "mps").
    pub fn parse_device(s: &str) -> Option<Self> {
        match s {
            "cpu" => Some(Self::Cpu),
            "mps" => Some(Self::Mps),
            _ if s.starts_with("cuda") => {
                if s == "cuda" {
                    Some(Self::Cuda(0))
                } else if let Some(idx_str) = s.strip_prefix("cuda:") {
                    idx_str.parse().ok().map(Self::Cuda)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns whether this is a CUDA device.
    pub fn is_cuda(&self) -> bool {
        matches!(self, Self::Cuda(_))
    }
}

/// A PyTorch tensor with device tracking.
///
/// Bridges between PyTorch tensors and Fajar tensors, tracking the device
/// (CPU/CUDA/MPS) and ensuring correct data transfer.
#[derive(Debug, Clone)]
pub struct TorchTensor {
    /// Tensor shape.
    shape: Vec<usize>,
    /// Element data type.
    dtype: NumpyDtype,
    /// Device where the tensor resides.
    device: TorchDevice,
    /// Flattened data (only valid for CPU tensors; CUDA data is simulated).
    data: Vec<f64>,
    /// Whether this tensor requires gradient computation.
    requires_grad: bool,
    /// Whether this tensor is a leaf in the autograd graph.
    is_leaf: bool,
}

impl TorchTensor {
    /// Creates a new CPU tensor from shape and data.
    pub fn new(
        shape: Vec<usize>,
        dtype: NumpyDtype,
        data: Vec<f64>,
    ) -> Result<Self, NumpyBridgeError> {
        let expected: usize = shape.iter().product();
        if data.len() != expected {
            return Err(NumpyBridgeError::ShapeMismatch {
                expected: shape,
                actual: vec![data.len()],
            });
        }
        Ok(Self {
            shape,
            dtype,
            device: TorchDevice::Cpu,
            data,
            requires_grad: false,
            is_leaf: true,
        })
    }

    /// Creates a zero tensor on the specified device.
    pub fn zeros(shape: Vec<usize>, dtype: NumpyDtype, device: TorchDevice) -> Self {
        let numel: usize = shape.iter().product();
        Self {
            shape,
            dtype,
            device,
            data: vec![0.0; numel],
            requires_grad: false,
            is_leaf: true,
        }
    }

    /// Returns the tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the element data type.
    pub fn dtype(&self) -> NumpyDtype {
        self.dtype
    }

    /// Returns the device.
    pub fn device(&self) -> &TorchDevice {
        &self.device
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Returns whether gradients are tracked.
    pub fn requires_grad(&self) -> bool {
        self.requires_grad
    }

    /// Sets whether gradients should be tracked.
    pub fn set_requires_grad(&mut self, requires: bool) {
        self.requires_grad = requires;
    }

    /// Returns whether this is a leaf tensor.
    pub fn is_leaf(&self) -> bool {
        self.is_leaf
    }

    /// Moves the tensor to a different device (simulated).
    pub fn to_device(&self, target: TorchDevice) -> Self {
        Self {
            shape: self.shape.clone(),
            dtype: self.dtype,
            device: target,
            data: self.data.clone(),
            requires_grad: self.requires_grad,
            is_leaf: self.is_leaf,
        }
    }

    /// Converts to a NumPy array (must be on CPU).
    pub fn to_numpy(&self) -> Result<NumpyArray, NumpyBridgeError> {
        if self.device != TorchDevice::Cpu {
            return Err(NumpyBridgeError::DeviceMismatch {
                expected: "cpu".to_string(),
                actual: format!("{}", self.device),
            });
        }
        NumpyArray::new(self.shape.clone(), self.dtype, self.data.clone())
    }

    /// Creates a `TorchTensor` from a `NumpyArray`.
    pub fn from_numpy(arr: &NumpyArray) -> Self {
        Self {
            shape: arr.shape().to_vec(),
            dtype: arr.dtype(),
            device: TorchDevice::Cpu,
            data: arr.data().to_vec(),
            requires_grad: false,
            is_leaf: true,
        }
    }

    /// Returns a read-only reference to the data.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Detaches the tensor from the autograd graph.
    pub fn detach(&self) -> Self {
        Self {
            shape: self.shape.clone(),
            dtype: self.dtype,
            device: self.device.clone(),
            data: self.data.clone(),
            requires_grad: false,
            is_leaf: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.5: Shape/Stride Handling
// ═══════════════════════════════════════════════════════════════════════

/// Array memory layout: shape, strides, contiguity check.
#[derive(Debug, Clone)]
pub struct ArrayLayout {
    /// Shape of the array.
    pub shape: Vec<usize>,
    /// Strides in elements (not bytes) for each dimension.
    pub strides: Vec<usize>,
    /// Whether the layout is C-contiguous (row-major).
    pub is_contiguous: bool,
    /// Whether the layout is Fortran-contiguous (column-major).
    pub is_fortran_contiguous: bool,
    /// Element size in bytes.
    pub element_size: usize,
}

impl ArrayLayout {
    /// Creates a C-contiguous (row-major) layout for the given shape.
    pub fn c_contiguous(shape: &[usize], dtype: NumpyDtype) -> Self {
        let ndim = shape.len();
        let mut strides = vec![0usize; ndim];
        if ndim > 0 {
            strides[ndim - 1] = 1;
            for i in (0..ndim - 1).rev() {
                strides[i] = strides[i + 1] * shape[i + 1];
            }
        }
        Self {
            shape: shape.to_vec(),
            strides,
            is_contiguous: true,
            is_fortran_contiguous: ndim <= 1,
            element_size: dtype.element_size(),
        }
    }

    /// Creates a Fortran-contiguous (column-major) layout for the given shape.
    pub fn fortran_contiguous(shape: &[usize], dtype: NumpyDtype) -> Self {
        let ndim = shape.len();
        let mut strides = vec![0usize; ndim];
        if ndim > 0 {
            strides[0] = 1;
            for i in 1..ndim {
                strides[i] = strides[i - 1] * shape[i - 1];
            }
        }
        Self {
            shape: shape.to_vec(),
            strides,
            is_contiguous: ndim <= 1,
            is_fortran_contiguous: true,
            element_size: dtype.element_size(),
        }
    }

    /// Creates a custom layout with arbitrary strides.
    pub fn custom(
        shape: &[usize],
        strides: &[usize],
        dtype: NumpyDtype,
    ) -> Result<Self, NumpyBridgeError> {
        if shape.len() != strides.len() {
            return Err(NumpyBridgeError::NotContiguous {
                reason: format!(
                    "shape has {} dims but strides has {} dims",
                    shape.len(),
                    strides.len()
                ),
            });
        }
        let c_layout = Self::c_contiguous(shape, dtype);
        let f_layout = Self::fortran_contiguous(shape, dtype);
        Ok(Self {
            shape: shape.to_vec(),
            strides: strides.to_vec(),
            is_contiguous: strides == c_layout.strides.as_slice(),
            is_fortran_contiguous: strides == f_layout.strides.as_slice(),
            element_size: dtype.element_size(),
        })
    }

    /// Returns the number of dimensions.
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Returns the total size in bytes.
    pub fn nbytes(&self) -> usize {
        self.numel() * self.element_size
    }

    /// Computes the flat index from a multi-dimensional index.
    pub fn flat_index(&self, indices: &[usize]) -> Result<usize, NumpyBridgeError> {
        if indices.len() != self.shape.len() {
            return Err(NumpyBridgeError::InvalidOperation {
                message: format!(
                    "expected {} indices, got {}",
                    self.shape.len(),
                    indices.len()
                ),
            });
        }
        for (i, (&idx, &dim)) in indices.iter().zip(self.shape.iter()).enumerate() {
            if idx >= dim {
                return Err(NumpyBridgeError::InvalidOperation {
                    message: format!("index {idx} out of bounds for dim {i} with size {dim}"),
                });
            }
        }
        let flat: usize = indices
            .iter()
            .zip(self.strides.iter())
            .map(|(&idx, &stride)| idx * stride)
            .sum();
        Ok(flat)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.6: PyTorch Model Loading
// ═══════════════════════════════════════════════════════════════════════

/// A parameter in a PyTorch model.
#[derive(Debug, Clone)]
pub struct ModelParameter {
    /// Parameter name (e.g., "layer1.weight").
    pub name: String,
    /// Shape.
    pub shape: Vec<usize>,
    /// Data type.
    pub dtype: NumpyDtype,
    /// Whether this parameter requires gradient.
    pub requires_grad: bool,
    /// Total number of elements.
    pub numel: usize,
}

/// A loaded PyTorch model.
///
/// Supports loading from a simulated state dict, inspecting parameters,
/// and running forward passes (simulated).
#[derive(Debug)]
pub struct TorchModel {
    /// Model name / architecture identifier.
    name: String,
    /// Path the model was loaded from.
    path: String,
    /// Device the model is on.
    device: TorchDevice,
    /// Model parameters (state dict).
    parameters: Vec<ModelParameter>,
    /// Whether the model is in eval mode (vs training mode).
    eval_mode: bool,
    /// Parameter data (name -> flattened values).
    weights: HashMap<String, Vec<f64>>,
}

impl TorchModel {
    /// Loads a model from the given path (simulated).
    ///
    /// In a real implementation, this would call `torch.load()` via pyo3.
    pub fn load(path: &str, device: TorchDevice) -> Result<Self, NumpyBridgeError> {
        if path.is_empty() {
            return Err(NumpyBridgeError::ModelLoadError {
                path: path.to_string(),
                message: "empty path".to_string(),
            });
        }
        // Simulate a simple model with two layers.
        let params = vec![
            ModelParameter {
                name: "layer1.weight".to_string(),
                shape: vec![128, 784],
                dtype: NumpyDtype::Float32,
                requires_grad: true,
                numel: 128 * 784,
            },
            ModelParameter {
                name: "layer1.bias".to_string(),
                shape: vec![128],
                dtype: NumpyDtype::Float32,
                requires_grad: true,
                numel: 128,
            },
            ModelParameter {
                name: "layer2.weight".to_string(),
                shape: vec![10, 128],
                dtype: NumpyDtype::Float32,
                requires_grad: true,
                numel: 10 * 128,
            },
            ModelParameter {
                name: "layer2.bias".to_string(),
                shape: vec![10],
                dtype: NumpyDtype::Float32,
                requires_grad: true,
                numel: 10,
            },
        ];

        let mut weights = HashMap::new();
        for p in &params {
            weights.insert(p.name.clone(), vec![0.0; p.numel]);
        }

        Ok(Self {
            name: "SimulatedModel".to_string(),
            path: path.to_string(),
            device,
            parameters: params,
            eval_mode: false,
            weights,
        })
    }

    /// Returns the model name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the model path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the device.
    pub fn device(&self) -> &TorchDevice {
        &self.device
    }

    /// Returns whether the model is in eval mode.
    pub fn is_eval(&self) -> bool {
        self.eval_mode
    }

    /// Sets the model to eval mode.
    pub fn eval(&mut self) {
        self.eval_mode = true;
    }

    /// Sets the model to training mode.
    pub fn train(&mut self) {
        self.eval_mode = false;
    }

    /// Returns the list of model parameters.
    pub fn parameters(&self) -> &[ModelParameter] {
        &self.parameters
    }

    /// Returns the total number of parameters.
    pub fn total_parameters(&self) -> usize {
        self.parameters.iter().map(|p| p.numel).sum()
    }

    /// Returns the total number of trainable parameters.
    pub fn trainable_parameters(&self) -> usize {
        self.parameters
            .iter()
            .filter(|p| p.requires_grad)
            .map(|p| p.numel)
            .sum()
    }

    /// Runs a forward pass (simulated).
    ///
    /// Returns a tensor with the output shape determined by the model
    /// architecture. For the simulated model, input shape [B, 784] produces
    /// output shape [B, 10].
    pub fn forward(&self, input: &TorchTensor) -> Result<TorchTensor, NumpyBridgeError> {
        if !self.eval_mode {
            // Training mode would also compute gradients.
        }
        if input.shape().len() != 2 {
            return Err(NumpyBridgeError::ShapeMismatch {
                expected: vec![0, 784],
                actual: input.shape().to_vec(),
            });
        }
        let batch_size = input.shape()[0];
        let output_shape = vec![batch_size, 10];
        let numel: usize = output_shape.iter().product();
        Ok(TorchTensor {
            shape: output_shape,
            dtype: input.dtype(),
            device: self.device.clone(),
            data: vec![0.0; numel],
            requires_grad: false,
            is_leaf: false,
        })
    }

    /// Gets a parameter's weight data by name.
    pub fn get_weights(&self, name: &str) -> Option<&[f64]> {
        self.weights.get(name).map(|v| v.as_slice())
    }

    /// Sets a parameter's weight data by name.
    pub fn set_weights(&mut self, name: &str, data: Vec<f64>) -> Result<(), NumpyBridgeError> {
        let param = self.parameters.iter().find(|p| p.name == name);
        match param {
            Some(p) => {
                if data.len() != p.numel {
                    return Err(NumpyBridgeError::ShapeMismatch {
                        expected: p.shape.clone(),
                        actual: vec![data.len()],
                    });
                }
                self.weights.insert(name.to_string(), data);
                Ok(())
            }
            None => Err(NumpyBridgeError::InvalidOperation {
                message: format!("parameter '{name}' not found"),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.7: Mixed Training (Fajar + PyTorch)
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for mixed Fajar/PyTorch training.
#[derive(Debug, Clone)]
pub struct MixedTrainingConfig {
    /// Learning rate.
    pub learning_rate: f64,
    /// Batch size.
    pub batch_size: usize,
    /// Number of epochs.
    pub epochs: usize,
    /// Which layers run on PyTorch vs Fajar.
    pub pytorch_layers: Vec<String>,
    /// Which layers run on Fajar.
    pub fajar_layers: Vec<String>,
}

/// Manages mixed-framework training where some layers execute in PyTorch
/// and others execute in Fajar's native ML runtime.
#[derive(Debug)]
pub struct MixedTrainer {
    /// Training configuration.
    config: MixedTrainingConfig,
    /// Current epoch.
    current_epoch: usize,
    /// Current step within the epoch.
    current_step: usize,
    /// Training loss history.
    loss_history: Vec<f64>,
    /// Shared weights between frameworks (name -> data).
    shared_weights: HashMap<String, Vec<f64>>,
}

impl MixedTrainer {
    /// Creates a new mixed trainer.
    pub fn new(config: MixedTrainingConfig) -> Self {
        Self {
            config,
            current_epoch: 0,
            current_step: 0,
            loss_history: Vec::new(),
            shared_weights: HashMap::new(),
        }
    }

    /// Returns the training configuration.
    pub fn config(&self) -> &MixedTrainingConfig {
        &self.config
    }

    /// Returns the current epoch.
    pub fn current_epoch(&self) -> usize {
        self.current_epoch
    }

    /// Returns the current step.
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// Returns the loss history.
    pub fn loss_history(&self) -> &[f64] {
        &self.loss_history
    }

    /// Shares weights from a Fajar tensor to the PyTorch side.
    pub fn share_weights(&mut self, name: &str, data: Vec<f64>) {
        self.shared_weights.insert(name.to_string(), data);
    }

    /// Retrieves shared weights by name.
    pub fn get_shared_weights(&self, name: &str) -> Option<&[f64]> {
        self.shared_weights.get(name).map(|v| v.as_slice())
    }

    /// Simulates one training step.
    ///
    /// Returns the loss for this step. In a real implementation, this would
    /// coordinate forward/backward passes between PyTorch and Fajar.
    pub fn step(&mut self, _batch_data: &[f64]) -> Result<f64, NumpyBridgeError> {
        self.current_step += 1;
        // Simulate decreasing loss.
        let total_steps = (self.current_epoch * 100 + self.current_step) as f64;
        let loss = 2.0 / (1.0 + total_steps * 0.01);
        self.loss_history.push(loss);
        Ok(loss)
    }

    /// Advances to the next epoch.
    pub fn next_epoch(&mut self) {
        self.current_epoch += 1;
        self.current_step = 0;
    }

    /// Returns the number of shared weight tensors.
    pub fn shared_weight_count(&self) -> usize {
        self.shared_weights.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.8: ONNX Export
// ═══════════════════════════════════════════════════════════════════════

/// ONNX operator set version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpsetVersion {
    /// Domain (e.g., "" for default ONNX domain).
    pub version: u32,
}

impl Default for OpsetVersion {
    fn default() -> Self {
        Self { version: 17 }
    }
}

/// ONNX tensor info (input/output specification).
#[derive(Debug, Clone)]
pub struct OnnxTensorInfo {
    /// Tensor name.
    pub name: String,
    /// Shape (can contain -1 for dynamic dimensions).
    pub shape: Vec<i64>,
    /// Element type.
    pub dtype: NumpyDtype,
}

/// ONNX model metadata produced by export.
#[derive(Debug, Clone)]
pub struct OnnxMetadata {
    /// Path where the ONNX model was saved.
    pub path: String,
    /// Model inputs.
    pub inputs: Vec<OnnxTensorInfo>,
    /// Model outputs.
    pub outputs: Vec<OnnxTensorInfo>,
    /// Opset version.
    pub opset: OpsetVersion,
    /// Number of nodes in the computation graph.
    pub node_count: usize,
    /// Producer name.
    pub producer: String,
    /// Model size in bytes.
    pub size_bytes: usize,
}

/// Exports a model to ONNX format (simulated).
///
/// Produces `OnnxMetadata` describing the exported model. In a real
/// implementation this would call `torch.onnx.export()` or generate
/// ONNX protobuf directly.
pub fn export_onnx(
    model: &TorchModel,
    path: &str,
    input_shape: &[usize],
) -> Result<OnnxMetadata, NumpyBridgeError> {
    if path.is_empty() {
        return Err(NumpyBridgeError::OnnxExportError {
            message: "empty output path".to_string(),
        });
    }
    if input_shape.is_empty() {
        return Err(NumpyBridgeError::OnnxExportError {
            message: "input shape must not be empty".to_string(),
        });
    }

    let input_shape_i64: Vec<i64> = input_shape.iter().map(|&s| s as i64).collect();

    // Determine output shape from model (simulated: last layer output).
    let output_dim = model.parameters().last().map(|p| p.shape[0]).unwrap_or(1);
    let batch_size = input_shape.first().copied().unwrap_or(1) as i64;

    let inputs = vec![OnnxTensorInfo {
        name: "input".to_string(),
        shape: input_shape_i64,
        dtype: NumpyDtype::Float32,
    }];

    let outputs = vec![OnnxTensorInfo {
        name: "output".to_string(),
        shape: vec![batch_size, output_dim as i64],
        dtype: NumpyDtype::Float32,
    }];

    let total_params: usize = model.total_parameters();

    Ok(OnnxMetadata {
        path: path.to_string(),
        inputs,
        outputs,
        opset: OpsetVersion::default(),
        node_count: model.parameters().len() * 2, // rough estimate: matmul + bias per layer
        producer: "fajar-lang".to_string(),
        size_bytes: total_params * NumpyDtype::Float32.element_size(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// E5.9: Batch Processing
// ═══════════════════════════════════════════════════════════════════════

/// Automatic batch converter for NumPy/Tensor data.
///
/// Handles splitting large datasets into batches and converting between
/// NumPy arrays and Fajar tensors in batched form.
#[derive(Debug)]
pub struct BatchConverter {
    /// Batch size.
    batch_size: usize,
    /// Whether to drop the last incomplete batch.
    drop_last: bool,
    /// Total samples seen.
    total_samples: usize,
    /// Total batches produced.
    total_batches: usize,
}

impl BatchConverter {
    /// Creates a new batch converter.
    pub fn new(batch_size: usize, drop_last: bool) -> Self {
        Self {
            batch_size,
            drop_last,
            total_samples: 0,
            total_batches: 0,
        }
    }

    /// Returns the batch size.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Returns whether incomplete batches are dropped.
    pub fn drop_last(&self) -> bool {
        self.drop_last
    }

    /// Returns the total number of samples processed.
    pub fn total_samples(&self) -> usize {
        self.total_samples
    }

    /// Returns the total number of batches produced.
    pub fn total_batches(&self) -> usize {
        self.total_batches
    }

    /// Computes the number of batches for a dataset of the given size.
    pub fn num_batches(&self, dataset_size: usize) -> usize {
        if self.drop_last {
            dataset_size / self.batch_size
        } else if dataset_size == 0 {
            0
        } else {
            dataset_size.div_ceil(self.batch_size)
        }
    }

    /// Splits a 1D array of data into batches.
    ///
    /// Each batch is a `Vec<f64>` of length `batch_size` (the last batch
    /// may be shorter unless `drop_last` is true).
    pub fn split_batches(&mut self, data: &[f64]) -> Vec<Vec<f64>> {
        let mut batches = Vec::new();
        let mut offset = 0;
        while offset < data.len() {
            let end = (offset + self.batch_size).min(data.len());
            let batch = data[offset..end].to_vec();
            if batch.len() < self.batch_size && self.drop_last {
                break;
            }
            batches.push(batch);
            self.total_batches += 1;
            offset = end;
        }
        self.total_samples += data.len();
        batches
    }

    /// Splits a `NumpyArray` along the first axis into batches.
    ///
    /// The input array must be at least 2D. Returns batched `NumpyArray`s
    /// where each batch has at most `batch_size` rows.
    pub fn split_numpy_batches(
        &mut self,
        array: &NumpyArray,
    ) -> Result<Vec<NumpyArray>, NumpyBridgeError> {
        if array.ndim() < 1 {
            return Err(NumpyBridgeError::InvalidOperation {
                message: "cannot batch a 0-dimensional array".to_string(),
            });
        }
        let total_rows = array.shape()[0];
        let row_size: usize = if array.ndim() > 1 {
            array.shape()[1..].iter().product()
        } else {
            1
        };

        let mut batches = Vec::new();
        let mut row = 0;
        while row < total_rows {
            let batch_rows = (self.batch_size).min(total_rows - row);
            if batch_rows < self.batch_size && self.drop_last {
                break;
            }

            let start = row * row_size;
            let end = start + batch_rows * row_size;
            let batch_data = array.data()[start..end].to_vec();

            let mut batch_shape = array.shape().to_vec();
            batch_shape[0] = batch_rows;

            let batch_arr = NumpyArray::new(batch_shape, array.dtype(), batch_data)?;
            batches.push(batch_arr);
            self.total_batches += 1;
            row += batch_rows;
        }
        self.total_samples += total_rows;
        Ok(batches)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5.10: Tests (15+)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- E5.1: Zero-Copy NumPy -> Tensor ---

    #[test]
    fn e5_1_numpy_array_creation() {
        let arr = NumpyArray::new(
            vec![2, 3],
            NumpyDtype::Float32,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .unwrap();
        assert_eq!(arr.shape(), &[2, 3]);
        assert_eq!(arr.ndim(), 2);
        assert_eq!(arr.numel(), 6);
        assert_eq!(arr.dtype(), NumpyDtype::Float32);
        assert_eq!(arr.nbytes(), 24); // 6 * 4
        assert!(!arr.is_view());
    }

    #[test]
    fn e5_1_numpy_array_shape_mismatch() {
        let result = NumpyArray::new(vec![2, 3], NumpyDtype::Float64, vec![1.0, 2.0]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NumpyBridgeError::ShapeMismatch { .. }
        ));
    }

    #[test]
    fn e5_1_numpy_zeros_ones() {
        let z = NumpyArray::zeros(vec![3, 4], NumpyDtype::Float64);
        assert_eq!(z.numel(), 12);
        assert!(z.data().iter().all(|&x| x == 0.0));

        let o = NumpyArray::ones(vec![2, 2], NumpyDtype::Float32);
        assert!(o.data().iter().all(|&x| x == 1.0));
    }

    #[test]
    fn e5_1_numpy_to_tensor_descriptor() {
        let arr = NumpyArray::zeros(vec![3, 224, 224], NumpyDtype::Float32);
        let desc = arr.to_tensor_descriptor();
        assert_eq!(desc.shape, vec![3, 224, 224]);
        assert_eq!(desc.numel, 3 * 224 * 224);
        assert!(desc.is_contiguous);
    }

    // --- E5.2: Zero-Copy Tensor -> NumPy ---

    #[test]
    fn e5_2_to_numpy_view() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let arr = to_numpy(&[2, 3], NumpyDtype::Float64, &data).unwrap();
        assert!(arr.is_view());
        assert_eq!(arr.data(), &data);
        assert_eq!(arr.shape(), &[2, 3]);
    }

    #[test]
    fn e5_2_to_numpy_shape_mismatch() {
        let result = to_numpy(&[5, 5], NumpyDtype::Float32, &[1.0, 2.0]);
        assert!(result.is_err());
    }

    // --- E5.3: PyTorch Tensor Bridge ---

    #[test]
    fn e5_3_torch_tensor_creation() {
        let t = TorchTensor::new(
            vec![2, 3],
            NumpyDtype::Float32,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .unwrap();
        assert_eq!(t.shape(), &[2, 3]);
        assert_eq!(t.dtype(), NumpyDtype::Float32);
        assert_eq!(t.device(), &TorchDevice::Cpu);
        assert!(!t.requires_grad());
        assert!(t.is_leaf());
    }

    #[test]
    fn e5_3_torch_device_transfer() {
        let t = TorchTensor::zeros(vec![4, 4], NumpyDtype::Float32, TorchDevice::Cpu);
        let gpu = t.to_device(TorchDevice::Cuda(0));
        assert_eq!(gpu.device(), &TorchDevice::Cuda(0));
        assert_eq!(gpu.shape(), t.shape());
    }

    #[test]
    fn e5_3_torch_to_numpy_requires_cpu() {
        let gpu_tensor = TorchTensor::zeros(vec![2, 2], NumpyDtype::Float32, TorchDevice::Cuda(0));
        let result = gpu_tensor.to_numpy();
        assert!(matches!(
            result.unwrap_err(),
            NumpyBridgeError::DeviceMismatch { .. }
        ));

        let cpu_tensor = gpu_tensor.to_device(TorchDevice::Cpu);
        assert!(cpu_tensor.to_numpy().is_ok());
    }

    #[test]
    fn e5_3_torch_from_numpy() {
        let arr = NumpyArray::ones(vec![3, 3], NumpyDtype::Float64);
        let t = TorchTensor::from_numpy(&arr);
        assert_eq!(t.shape(), &[3, 3]);
        assert_eq!(t.dtype(), NumpyDtype::Float64);
        assert_eq!(t.device(), &TorchDevice::Cpu);
    }

    // --- E5.4: dtype Mapping ---

    #[test]
    fn e5_4_dtype_all_12_types() {
        let all = [
            NumpyDtype::Float16,
            NumpyDtype::Float32,
            NumpyDtype::Float64,
            NumpyDtype::Int8,
            NumpyDtype::Int16,
            NumpyDtype::Int32,
            NumpyDtype::Int64,
            NumpyDtype::Uint8,
            NumpyDtype::Uint16,
            NumpyDtype::Uint32,
            NumpyDtype::Uint64,
            NumpyDtype::Bool,
        ];
        assert_eq!(all.len(), 12);

        // Each has a Fajar type, a NumPy string, and a nonzero element size.
        for dtype in &all {
            assert!(!dtype.to_fajar_type().is_empty());
            assert!(!dtype.to_numpy_str().is_empty());
            assert!(dtype.element_size() > 0);
        }
    }

    #[test]
    fn e5_4_dtype_from_numpy_str() {
        assert_eq!(
            NumpyDtype::from_numpy_str("float32"),
            Some(NumpyDtype::Float32)
        );
        assert_eq!(NumpyDtype::from_numpy_str("int64"), Some(NumpyDtype::Int64));
        assert_eq!(NumpyDtype::from_numpy_str("bool_"), Some(NumpyDtype::Bool));
        assert_eq!(
            NumpyDtype::from_numpy_str("uint16"),
            Some(NumpyDtype::Uint16)
        );
        assert_eq!(NumpyDtype::from_numpy_str("unknown"), None);
    }

    #[test]
    fn e5_4_dtype_classification() {
        assert!(NumpyDtype::Float32.is_float());
        assert!(!NumpyDtype::Float32.is_signed_int());
        assert!(NumpyDtype::Int32.is_signed_int());
        assert!(!NumpyDtype::Int32.is_unsigned_int());
        assert!(NumpyDtype::Uint8.is_unsigned_int());
        assert!(!NumpyDtype::Bool.is_float());
    }

    // --- E5.5: Shape/Stride Handling ---

    #[test]
    fn e5_5_c_contiguous_strides() {
        let layout = ArrayLayout::c_contiguous(&[2, 3, 4], NumpyDtype::Float32);
        assert_eq!(layout.strides, vec![12, 4, 1]);
        assert!(layout.is_contiguous);
        assert!(!layout.is_fortran_contiguous);
        assert_eq!(layout.numel(), 24);
        assert_eq!(layout.nbytes(), 96); // 24 * 4
    }

    #[test]
    fn e5_5_fortran_contiguous_strides() {
        let layout = ArrayLayout::fortran_contiguous(&[2, 3, 4], NumpyDtype::Float64);
        assert_eq!(layout.strides, vec![1, 2, 6]);
        assert!(!layout.is_contiguous);
        assert!(layout.is_fortran_contiguous);
    }

    #[test]
    fn e5_5_flat_index() {
        let layout = ArrayLayout::c_contiguous(&[3, 4], NumpyDtype::Float32);
        assert_eq!(layout.flat_index(&[0, 0]).unwrap(), 0);
        assert_eq!(layout.flat_index(&[1, 2]).unwrap(), 6);
        assert_eq!(layout.flat_index(&[2, 3]).unwrap(), 11);

        // Out of bounds.
        assert!(layout.flat_index(&[3, 0]).is_err());
        // Wrong number of indices.
        assert!(layout.flat_index(&[1]).is_err());
    }

    // --- E5.6: PyTorch Model Loading ---

    #[test]
    fn e5_6_model_load() {
        let model = TorchModel::load("model.pt", TorchDevice::Cpu).unwrap();
        assert_eq!(model.name(), "SimulatedModel");
        assert_eq!(model.path(), "model.pt");
        assert_eq!(model.parameters().len(), 4);
        assert!(model.total_parameters() > 0);
    }

    #[test]
    fn e5_6_model_load_empty_path() {
        let result = TorchModel::load("", TorchDevice::Cpu);
        assert!(matches!(
            result.unwrap_err(),
            NumpyBridgeError::ModelLoadError { .. }
        ));
    }

    #[test]
    fn e5_6_model_forward() {
        let mut model = TorchModel::load("net.pt", TorchDevice::Cpu).unwrap();
        model.eval();
        assert!(model.is_eval());

        let input = TorchTensor::zeros(vec![4, 784], NumpyDtype::Float32, TorchDevice::Cpu);
        let output = model.forward(&input).unwrap();
        assert_eq!(output.shape(), &[4, 10]);
    }

    #[test]
    fn e5_6_model_set_weights() {
        let mut model = TorchModel::load("m.pt", TorchDevice::Cpu).unwrap();
        let result = model.set_weights("layer1.bias", vec![1.0; 128]);
        assert!(result.is_ok());

        let bad = model.set_weights("layer1.bias", vec![1.0; 5]);
        assert!(bad.is_err());

        let missing = model.set_weights("nonexistent", vec![]);
        assert!(missing.is_err());
    }

    // --- E5.7: Mixed Training ---

    #[test]
    fn e5_7_mixed_trainer() {
        let config = MixedTrainingConfig {
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 10,
            pytorch_layers: vec!["conv1".to_string()],
            fajar_layers: vec!["fc1".to_string()],
        };
        let mut trainer = MixedTrainer::new(config);
        assert_eq!(trainer.current_epoch(), 0);
        assert_eq!(trainer.current_step(), 0);

        let loss = trainer.step(&[0.0; 32]).unwrap();
        assert!(loss > 0.0);
        assert_eq!(trainer.current_step(), 1);
        assert_eq!(trainer.loss_history().len(), 1);

        trainer.share_weights("fc1.weight", vec![0.1; 100]);
        assert_eq!(trainer.shared_weight_count(), 1);
        assert!(trainer.get_shared_weights("fc1.weight").is_some());
    }

    // --- E5.8: ONNX Export ---

    #[test]
    fn e5_8_onnx_export() {
        let model = TorchModel::load("model.pt", TorchDevice::Cpu).unwrap();
        let meta = export_onnx(&model, "model.onnx", &[1, 784]).unwrap();
        assert_eq!(meta.path, "model.onnx");
        assert_eq!(meta.producer, "fajar-lang");
        assert_eq!(meta.inputs.len(), 1);
        assert_eq!(meta.outputs.len(), 1);
        assert_eq!(meta.opset.version, 17);
        assert!(meta.size_bytes > 0);
        assert!(meta.node_count > 0);
    }

    #[test]
    fn e5_8_onnx_export_empty_path() {
        let model = TorchModel::load("m.pt", TorchDevice::Cpu).unwrap();
        let result = export_onnx(&model, "", &[1, 784]);
        assert!(matches!(
            result.unwrap_err(),
            NumpyBridgeError::OnnxExportError { .. }
        ));
    }

    // --- E5.9: Batch Processing ---

    #[test]
    fn e5_9_batch_split_1d() {
        let mut bc = BatchConverter::new(3, false);
        let data: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let batches = bc.split_batches(&data);
        assert_eq!(batches.len(), 4); // 3, 3, 3, 1
        assert_eq!(batches[0].len(), 3);
        assert_eq!(batches[3].len(), 1);
        assert_eq!(bc.total_samples(), 10);
        assert_eq!(bc.total_batches(), 4);
    }

    #[test]
    fn e5_9_batch_split_drop_last() {
        let mut bc = BatchConverter::new(3, true);
        let data: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let batches = bc.split_batches(&data);
        assert_eq!(batches.len(), 3); // drops last incomplete batch
    }

    #[test]
    fn e5_9_batch_split_numpy() {
        let arr = NumpyArray::new(vec![7, 3], NumpyDtype::Float32, vec![0.0; 21]).unwrap();
        let mut bc = BatchConverter::new(3, false);
        let batches = bc.split_numpy_batches(&arr).unwrap();
        assert_eq!(batches.len(), 3); // 3, 3, 1
        assert_eq!(batches[0].shape(), &[3, 3]);
        assert_eq!(batches[2].shape(), &[1, 3]);
    }

    // --- Additional tests ---

    #[test]
    fn e5_torch_device_parsing() {
        assert_eq!(TorchDevice::parse_device("cpu"), Some(TorchDevice::Cpu));
        assert_eq!(TorchDevice::parse_device("cuda:0"), Some(TorchDevice::Cuda(0)));
        assert_eq!(TorchDevice::parse_device("cuda"), Some(TorchDevice::Cuda(0)));
        assert_eq!(TorchDevice::parse_device("mps"), Some(TorchDevice::Mps));
        assert_eq!(TorchDevice::parse_device("tpu"), None);
        assert!(TorchDevice::Cuda(0).is_cuda());
        assert!(!TorchDevice::Cpu.is_cuda());
    }

    #[test]
    fn e5_error_display() {
        let err = NumpyBridgeError::ShapeMismatch {
            expected: vec![2, 3],
            actual: vec![4, 5],
        };
        let s = format!("{err}");
        assert!(s.contains("shape mismatch"));

        let err2 = NumpyBridgeError::DeviceMismatch {
            expected: "cpu".to_string(),
            actual: "cuda:0".to_string(),
        };
        assert!(format!("{err2}").contains("device mismatch"));
    }

    #[test]
    fn e5_numpy_reshape() {
        let arr = NumpyArray::ones(vec![6], NumpyDtype::Float32);
        let reshaped = arr.reshape(vec![2, 3]).unwrap();
        assert_eq!(reshaped.shape(), &[2, 3]);
        assert_eq!(reshaped.numel(), 6);

        // Bad reshape
        let bad = arr.reshape(vec![2, 2]);
        assert!(bad.is_err());
    }

    #[test]
    fn e5_torch_detach() {
        let mut t =
            TorchTensor::new(vec![2, 2], NumpyDtype::Float32, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        t.set_requires_grad(true);
        assert!(t.requires_grad());

        let detached = t.detach();
        assert!(!detached.requires_grad());
        assert!(detached.is_leaf());
    }
}
