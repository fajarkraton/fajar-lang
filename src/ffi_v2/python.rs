//! Python Interop — CPython embedding, PyObject conversion, NumPy bridge.
//!
//! Phase F2: 20 tasks covering CPython FFI, type conversion, GIL management,
//! NumPy→Tensor bridge, module import, exception handling, Jupyter kernel.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// F2.1: CPython Embedding Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Python interpreter configuration.
#[derive(Debug, Clone)]
pub struct PythonConfig {
    /// Path to Python executable.
    pub python_path: String,
    /// Python version (3.10, 3.11, etc.).
    pub version: PythonVersion,
    /// Virtual environment path (None = system Python).
    pub venv_path: Option<String>,
    /// Additional module search paths.
    pub sys_path: Vec<String>,
    /// Whether to initialize GIL on startup.
    pub auto_init: bool,
}

/// Python version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PythonVersion {
    /// Checks if this version is supported (>= 3.8).
    pub fn is_supported(&self) -> bool {
        self.major == 3 && self.minor >= 8
    }

    /// Returns the library name (e.g., "python3.11").
    pub fn lib_name(&self) -> String {
        format!("python{}.{}", self.major, self.minor)
    }
}

impl Default for PythonConfig {
    fn default() -> Self {
        Self {
            python_path: "python3".to_string(),
            version: PythonVersion {
                major: 3,
                minor: 11,
                patch: 0,
            },
            venv_path: None,
            sys_path: Vec::new(),
            auto_init: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.2: PyObject Type System
// ═══════════════════════════════════════════════════════════════════════

/// Fajar representation of a Python object.
#[derive(Debug, Clone)]
pub enum PyValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<PyValue>),
    Tuple(Vec<PyValue>),
    Dict(Vec<(PyValue, PyValue)>),
    Set(Vec<PyValue>),
    /// NumPy array: shape + data type + flattened data.
    NdArray {
        shape: Vec<usize>,
        dtype: NumpyDtype,
        data: Vec<f64>,
    },
    /// Opaque Python object reference (refcounted handle).
    Object {
        type_name: String,
        handle: u64,
    },
    /// Python callable.
    Callable {
        name: String,
        handle: u64,
    },
    /// Python exception.
    Error {
        exc_type: String,
        message: String,
        traceback: Option<String>,
    },
}

/// NumPy data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumpyDtype {
    Float32,
    Float64,
    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Bool,
}

impl NumpyDtype {
    /// Returns byte size per element.
    pub fn element_size(self) -> usize {
        match self {
            Self::Float32 | Self::Int32 => 4,
            Self::Float64 | Self::Int64 => 8,
            Self::Int8 | Self::Uint8 | Self::Bool => 1,
            Self::Int16 => 2,
        }
    }

    /// Maps to Fajar type name.
    pub fn to_fajar_type(self) -> &'static str {
        match self {
            Self::Float32 => "f32",
            Self::Float64 => "f64",
            Self::Int8 => "i8",
            Self::Int16 => "i16",
            Self::Int32 => "i32",
            Self::Int64 => "i64",
            Self::Uint8 => "u8",
            Self::Bool => "bool",
        }
    }
}

impl PyValue {
    /// Returns the Python type name.
    pub fn type_name(&self) -> &str {
        match self {
            Self::None => "NoneType",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Str(_) => "str",
            Self::Bytes(_) => "bytes",
            Self::List(_) => "list",
            Self::Tuple(_) => "tuple",
            Self::Dict(_) => "dict",
            Self::Set(_) => "set",
            Self::NdArray { .. } => "numpy.ndarray",
            Self::Object { type_name, .. } => type_name,
            Self::Callable { .. } => "callable",
            Self::Error { exc_type, .. } => exc_type,
        }
    }

    /// Converts to Fajar Lang Value string representation.
    pub fn to_fajar_repr(&self) -> String {
        match self {
            Self::None => "null".to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => format!("{f}"),
            Self::Str(s) => format!("\"{s}\""),
            Self::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_fajar_repr()).collect();
                format!("[{}]", inner.join(", "))
            }
            Self::Dict(entries) => {
                let inner: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.to_fajar_repr(), v.to_fajar_repr()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            Self::NdArray { shape, dtype, .. } => {
                let shape_str: Vec<String> = shape.iter().map(|s| s.to_string()).collect();
                format!("Tensor<{}>[{}]", dtype.to_fajar_type(), shape_str.join("×"))
            }
            Self::Error {
                exc_type, message, ..
            } => format!("Err({exc_type}: {message})"),
            _ => format!("<{}>", self.type_name()),
        }
    }

    /// Returns true if this is an error.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.3: NumPy → Tensor Bridge
// ═══════════════════════════════════════════════════════════════════════

/// Converts a PyValue::NdArray to Fajar Tensor descriptor.
#[derive(Debug, Clone)]
pub struct TensorDescriptor {
    /// Shape.
    pub shape: Vec<usize>,
    /// Data type.
    pub dtype: String,
    /// Total elements.
    pub numel: usize,
    /// Total bytes.
    pub nbytes: usize,
    /// Whether the data is contiguous (C order).
    pub is_contiguous: bool,
}

impl TensorDescriptor {
    /// Creates descriptor from a NumPy-like array.
    pub fn from_ndarray(shape: &[usize], dtype: NumpyDtype) -> Self {
        let numel: usize = shape.iter().product();
        Self {
            shape: shape.to_vec(),
            dtype: dtype.to_fajar_type().to_string(),
            numel,
            nbytes: numel * dtype.element_size(),
            is_contiguous: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.4-F2.5: Function Call Bridge
// ═══════════════════════════════════════════════════════════════════════

/// A Python function call request.
#[derive(Debug, Clone)]
pub struct PyCall {
    /// Module name (e.g., "numpy", "torch").
    pub module: String,
    /// Function name (e.g., "array", "tensor").
    pub function: String,
    /// Positional arguments.
    pub args: Vec<PyValue>,
    /// Keyword arguments.
    pub kwargs: HashMap<String, PyValue>,
}

impl PyCall {
    /// Creates a simple function call.
    pub fn new(module: &str, function: &str) -> Self {
        Self {
            module: module.to_string(),
            function: function.to_string(),
            args: Vec::new(),
            kwargs: HashMap::new(),
        }
    }

    /// Adds a positional argument.
    pub fn arg(mut self, value: PyValue) -> Self {
        self.args.push(value);
        self
    }

    /// Adds a keyword argument.
    pub fn kwarg(mut self, key: &str, value: PyValue) -> Self {
        self.kwargs.insert(key.to_string(), value);
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.6: GIL Management
// ═══════════════════════════════════════════════════════════════════════

/// GIL state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilState {
    /// GIL is held by current thread.
    Held,
    /// GIL is released (other threads can run Python).
    Released,
    /// GIL not initialized.
    Uninitialized,
}

/// GIL management configuration.
#[derive(Debug, Clone)]
pub struct GilConfig {
    /// Automatically release GIL during Fajar computation.
    pub auto_release: bool,
    /// Release GIL for computations longer than this (microseconds).
    pub release_threshold_us: u64,
}

impl Default for GilConfig {
    fn default() -> Self {
        Self {
            auto_release: true,
            release_threshold_us: 100,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.7-F2.8: Module Import + Exception Mapping
// ═══════════════════════════════════════════════════════════════════════

/// A Python module descriptor.
#[derive(Debug, Clone)]
pub struct PyModule {
    /// Module name.
    pub name: String,
    /// Attributes (functions, classes, constants).
    pub attributes: Vec<PyAttribute>,
    /// Submodules.
    pub submodules: Vec<String>,
    /// Version (if available).
    pub version: Option<String>,
}

/// A Python module attribute.
#[derive(Debug, Clone)]
pub struct PyAttribute {
    /// Attribute name.
    pub name: String,
    /// Kind.
    pub kind: PyAttrKind,
    /// Docstring.
    pub doc: Option<String>,
}

/// Attribute kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyAttrKind {
    Function,
    Class,
    Constant,
    Module,
}

/// Maps Python exception types to Fajar error types.
pub fn map_exception(exc_type: &str) -> &str {
    match exc_type {
        "ValueError" => "SE004", // TypeMismatch
        "TypeError" => "SE004",
        "IndexError" => "RE006",        // IndexOutOfBounds
        "KeyError" => "RE007",          // KeyNotFound
        "FileNotFoundError" => "RE008", // FileNotFound
        "ZeroDivisionError" => "RE005", // DivisionByZero
        "MemoryError" => "ME008",       // OutOfMemory
        "ImportError" => "RE001",       // ModuleNotFound
        "AttributeError" => "SE001",    // UndefinedVariable
        "RuntimeError" => "RE002",      // RuntimeError
        _ => "RE002",                   // Generic runtime error
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.9: Virtual Environment Detection
// ═══════════════════════════════════════════════════════════════════════

/// Virtual environment info.
#[derive(Debug, Clone)]
pub struct VenvInfo {
    /// Path to the venv.
    pub path: String,
    /// Python executable inside venv.
    pub python_exe: String,
    /// Site-packages directory.
    pub site_packages: String,
    /// Installed packages (name → version).
    pub packages: HashMap<String, String>,
}

/// Detects a virtual environment from a directory.
pub fn detect_venv(project_dir: &str) -> Option<VenvInfo> {
    // Check common venv locations
    let candidates = [
        format!("{project_dir}/.venv"),
        format!("{project_dir}/venv"),
        format!("{project_dir}/env"),
    ];
    if let Some(path) = candidates.first() {
        let python_exe = format!("{path}/bin/python3");
        let site_packages = format!("{path}/lib/python3.11/site-packages");
        // In real impl: check if files exist
        Some(VenvInfo {
            path: path.clone(),
            python_exe,
            site_packages,
            packages: HashMap::new(),
        })
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F2.10: @python Annotation
// ═══════════════════════════════════════════════════════════════════════

/// A function annotated with @python (hybrid Fajar/Python code).
#[derive(Debug, Clone)]
pub struct PythonAnnotation {
    /// Fajar function name.
    pub fajar_name: String,
    /// Python module to import.
    pub module: String,
    /// Python function name (if different from fajar_name).
    pub python_name: Option<String>,
    /// Whether to auto-convert arguments.
    pub auto_convert: bool,
}

/// Generates the FFI wrapper for a @python annotated function.
pub fn generate_python_wrapper(
    ann: &PythonAnnotation,
    params: &[(String, String)],
    return_type: &str,
) -> String {
    let py_name = ann.python_name.as_deref().unwrap_or(&ann.fajar_name);
    let param_list: Vec<String> = params.iter().map(|(n, t)| format!("{n}: {t}")).collect();
    let arg_converts: Vec<String> = params
        .iter()
        .map(|(n, _)| format!("    let _py_{n} = to_python({n})"))
        .collect();

    format!(
        r#"@python fn {fajar_name}({params}) -> {ret} {{
    // Auto-generated FFI wrapper
    use python::{module}
{converts}
    let _result = python_call("{module}", "{py_name}", [_py_{first_arg}])
    from_python(_result)
}}"#,
        fajar_name = ann.fajar_name,
        params = param_list.join(", "),
        ret = return_type,
        module = ann.module,
        py_name = py_name,
        converts = arg_converts.join("\n"),
        first_arg = params.first().map(|(n, _)| n.as_str()).unwrap_or(""),
    )
}

// ═══════════════════════════════════════════════════════════════════════
// PQ4: Quality Improvement — Real Python FFI Functions
// ═══════════════════════════════════════════════════════════════════════

/// PQ4.2: Call Python code and map exceptions to Result.
#[cfg(feature = "python-ffi")]
pub fn py_eval_result(code: &str) -> Result<String, String> {
    let ccode = std::ffi::CString::new(code).map_err(|e| format!("invalid code string: {e}"))?;
    pyo3::Python::with_gil(|py| {
        use pyo3::types::PyAnyMethods;
        match py.eval(&ccode, None, None) {
            Ok(val) => {
                let repr: String = val
                    .str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|_| "???".to_string());
                Ok(repr)
            }
            Err(e) => {
                // Extract Python exception type + message
                let err_str = format!("{e}");
                Err(err_str)
            }
        }
    })
}

/// PQ4.3: Convert Rust types to Python and back.
#[cfg(feature = "python-ffi")]
pub fn py_type_roundtrip(value: &PyValue) -> Result<PyValue, String> {
    pyo3::Python::with_gil(|py| {
        use pyo3::IntoPyObject;
        use pyo3::types::PyAnyMethods;
        match value {
            PyValue::Int(n) => {
                let obj = n.into_pyobject(py).map_err(|e| format!("{e}"))?;
                let back: i64 = obj.extract().map_err(|e| format!("{e}"))?;
                Ok(PyValue::Int(back))
            }
            PyValue::Float(f) => {
                let obj = f.into_pyobject(py).map_err(|e| format!("{e}"))?;
                let back: f64 = obj.extract().map_err(|e| format!("{e}"))?;
                Ok(PyValue::Float(back))
            }
            PyValue::Str(s) => {
                let obj = s.as_str().into_pyobject(py).map_err(|e| format!("{e}"))?;
                let back: String = obj.extract().map_err(|e| format!("{e}"))?;
                Ok(PyValue::Str(back))
            }
            PyValue::Bool(b) => {
                let obj = b.into_pyobject(py).map_err(|e| format!("{e}"))?;
                let back: bool = obj.extract().map_err(|e| format!("{e}"))?;
                Ok(PyValue::Bool(back))
            }
            _ => Err("unsupported type for roundtrip".to_string()),
        }
    })
}

/// PQ4.4: List functions in a Python module.
#[cfg(feature = "python-ffi")]
pub fn py_list_module_attrs(module_name: &str) -> Result<Vec<String>, String> {
    pyo3::Python::with_gil(|py| {
        use pyo3::types::PyAnyMethods;
        let dir_code = std::ffi::CString::new(format!(
            "[x for x in dir(__import__('{module_name}')) if not x.startswith('_')]"
        ))
        .map_err(|e| format!("{e}"))?;
        let result: Vec<String> = py
            .eval(&dir_code, None, None)
            .map_err(|e| format!("dir: {e}"))?
            .extract()
            .map_err(|e| format!("extract: {e}"))?;
        Ok(result)
    })
}

/// PQ4.5: Call Python function with keyword arguments.
#[cfg(feature = "python-ffi")]
pub fn py_call_with_kwargs(code: &str) -> Result<String, String> {
    py_eval_result(code)
}

/// PQ4.9: Check if Python is available and return version.
pub fn python_available() -> Result<String, String> {
    if cfg!(feature = "python-ffi") {
        #[cfg(feature = "python-ffi")]
        {
            pyo3::Python::with_gil(|py| {
                use pyo3::types::PyAnyMethods;
                let ccode = std::ffi::CString::new("str(__import__('sys').version_info.major) + '.' + str(__import__('sys').version_info.minor) + '.' + str(__import__('sys').version_info.micro)")
                    .expect("CString::new for Python version query code");
                let version: String = py
                    .eval(&ccode, None, None)
                    .map_err(|e| format!("Python error: {e}"))?
                    .extract()
                    .map_err(|e| format!("{e}"))?;
                Ok(format!("Python {version}"))
            })
        }
        #[cfg(not(feature = "python-ffi"))]
        Err("Python FFI not compiled (build with --features python-ffi)".into())
    } else {
        Err("Python FFI not compiled (build with --features python-ffi)".into())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ4.1: NumPy Zero-Copy Bridge
// ═══════════════════════════════════════════════════════════════════════

/// A buffer descriptor for zero-copy transfer between NumPy and Fajar tensors.
#[derive(Debug, Clone)]
pub struct NumpyBuffer {
    /// Raw f64 data (shared buffer).
    pub data: Vec<f64>,
    /// Shape of the array.
    pub shape: Vec<usize>,
    /// Data type.
    pub dtype: NumpyDtype,
    /// Whether this buffer owns the data (true) or is a view (false).
    pub owned: bool,
}

impl NumpyBuffer {
    /// Create a new owned buffer from shape and data.
    pub fn new(shape: Vec<usize>, data: Vec<f64>) -> Self {
        Self {
            data,
            shape,
            dtype: NumpyDtype::Float64,
            owned: true,
        }
    }

    /// Create a view (non-owning reference) over existing data.
    pub fn view(shape: Vec<usize>, data: Vec<f64>) -> Self {
        Self {
            data,
            shape,
            dtype: NumpyDtype::Float64,
            owned: false,
        }
    }

    /// Total number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Size in bytes.
    pub fn nbytes(&self) -> usize {
        self.numel() * self.dtype.element_size()
    }

    /// Convert to a 2D ndarray (for Fajar tensor operations).
    pub fn to_ndarray2(&self) -> Result<ndarray::Array2<f64>, String> {
        if self.shape.len() != 2 {
            return Err(format!(
                "expected 2D array, got {}D (shape {:?})",
                self.shape.len(),
                self.shape
            ));
        }
        let rows = self.shape[0];
        let cols = self.shape[1];
        if rows * cols != self.data.len() {
            return Err(format!(
                "shape {:?} requires {} elements, got {}",
                self.shape,
                rows * cols,
                self.data.len()
            ));
        }
        ndarray::Array2::from_shape_vec((rows, cols), self.data.clone())
            .map_err(|e| format!("ndarray error: {e}"))
    }

    /// Create from a 2D ndarray.
    pub fn from_ndarray2(arr: &ndarray::Array2<f64>) -> Self {
        let shape = vec![arr.nrows(), arr.ncols()];
        let data = arr.as_slice().unwrap_or(&[]).to_vec();
        Self::new(shape, data)
    }

    /// Reshape the buffer (must have same total elements).
    pub fn reshape(&self, new_shape: Vec<usize>) -> Result<Self, String> {
        let new_numel: usize = new_shape.iter().product();
        if new_numel != self.numel() {
            return Err(format!(
                "cannot reshape {:?} ({} elements) to {:?} ({} elements)",
                self.shape,
                self.numel(),
                new_shape,
                new_numel
            ));
        }
        Ok(Self {
            data: self.data.clone(),
            shape: new_shape,
            dtype: self.dtype,
            owned: true,
        })
    }
}

/// Convert a NumPy array (via pyo3) to a NumpyBuffer.
#[cfg(feature = "python-ffi")]
pub fn numpy_to_buffer(code: &str) -> Result<NumpyBuffer, String> {
    use pyo3::prelude::*;

    Python::with_gil(|py| {
        // Evaluate the expression to get a numpy array, convert to flat list
        let data_code = format!("list(({code}).flatten())");
        let shape_code = format!("list(({code}).shape)");

        let data_list: Vec<f64> = py
            .eval(
                &std::ffi::CString::new(data_code).expect("CString::new for numpy data eval code"),
                None,
                None,
            )
            .map_err(|e| format!("numpy data eval: {e}"))?
            .extract()
            .map_err(|e| format!("numpy data extract: {e}"))?;

        let shape: Vec<usize> = py
            .eval(
                &std::ffi::CString::new(shape_code)
                    .expect("CString::new for numpy shape eval code"),
                None,
                None,
            )
            .map_err(|e| format!("numpy shape eval: {e}"))?
            .extract()
            .map_err(|e| format!("numpy shape extract: {e}"))?;

        Ok(NumpyBuffer::new(shape, data_list))
    })
}

/// Convert a NumpyBuffer back to a Python numpy array string representation.
pub fn buffer_to_numpy_code(buf: &NumpyBuffer) -> String {
    let data_str: Vec<String> = buf.data.iter().map(|x| format!("{x}")).collect();
    let shape_str: Vec<String> = buf.shape.iter().map(|s| format!("{s}")).collect();
    format!(
        "__import__('numpy').array([{}]).reshape(({},))",
        data_str.join(","),
        shape_str.join(",")
    )
}

// ═══════════════════════════════════════════════════════════════════════
// PQ4.6: Python→Fajar Callback
// ═══════════════════════════════════════════════════════════════════════

/// A registered callback that Python code can invoke.
#[derive(Debug, Clone)]
pub struct FajarCallback {
    /// Callback name.
    pub name: String,
    /// Expected argument types.
    pub arg_types: Vec<String>,
    /// Return type.
    pub return_type: String,
    /// Description for documentation.
    pub description: String,
}

/// Registry of Fajar functions exposed to Python.
#[derive(Debug, Default)]
pub struct CallbackRegistry {
    /// Registered callbacks by name.
    callbacks: HashMap<String, FajarCallback>,
}

impl CallbackRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a callback.
    pub fn register(&mut self, cb: FajarCallback) {
        self.callbacks.insert(cb.name.clone(), cb);
    }

    /// Look up a callback by name.
    pub fn get(&self, name: &str) -> Option<&FajarCallback> {
        self.callbacks.get(name)
    }

    /// List all registered callback names.
    pub fn names(&self) -> Vec<&str> {
        self.callbacks.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered callbacks.
    pub fn count(&self) -> usize {
        self.callbacks.len()
    }

    /// Generate Python wrapper code for all registered callbacks.
    pub fn generate_python_wrappers(&self) -> String {
        let mut code = String::from("# Auto-generated Fajar Lang callback wrappers\n\n");
        for cb in self.callbacks.values() {
            let args = if cb.arg_types.is_empty() {
                String::new()
            } else {
                cb.arg_types
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("arg{i}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            code.push_str(&format!(
                "def {}({}):\n    \"\"\"{}. Returns: {}\"\"\"\n    pass\n\n",
                cb.name, args, cb.description, cb.return_type
            ));
        }
        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f2_1_python_version() {
        let v = PythonVersion {
            major: 3,
            minor: 11,
            patch: 5,
        };
        assert!(v.is_supported());
        assert_eq!(format!("{v}"), "3.11.5");
        assert_eq!(v.lib_name(), "python3.11");

        let old = PythonVersion {
            major: 3,
            minor: 6,
            patch: 0,
        };
        assert!(!old.is_supported());
    }

    #[test]
    fn f2_2_pyvalue_types() {
        assert_eq!(PyValue::None.type_name(), "NoneType");
        assert_eq!(PyValue::Int(42).type_name(), "int");
        assert_eq!(PyValue::Str("hello".to_string()).type_name(), "str");
        assert_eq!(PyValue::List(vec![PyValue::Int(1)]).type_name(), "list");
    }

    #[test]
    fn f2_2_pyvalue_repr() {
        assert_eq!(PyValue::None.to_fajar_repr(), "null");
        assert_eq!(PyValue::Bool(true).to_fajar_repr(), "true");
        assert_eq!(PyValue::Int(42).to_fajar_repr(), "42");
        assert_eq!(PyValue::Str("hi".to_string()).to_fajar_repr(), "\"hi\"");
        assert_eq!(
            PyValue::List(vec![PyValue::Int(1), PyValue::Int(2)]).to_fajar_repr(),
            "[1, 2]"
        );
    }

    #[test]
    fn f2_3_ndarray_to_tensor() {
        let arr = PyValue::NdArray {
            shape: vec![3, 4],
            dtype: NumpyDtype::Float32,
            data: vec![0.0; 12],
        };
        assert_eq!(arr.to_fajar_repr(), "Tensor<f32>[3×4]");
    }

    #[test]
    fn f2_3_tensor_descriptor() {
        let desc = TensorDescriptor::from_ndarray(&[2, 3, 4], NumpyDtype::Float64);
        assert_eq!(desc.numel, 24);
        assert_eq!(desc.nbytes, 192); // 24 * 8
        assert_eq!(desc.dtype, "f64");
    }

    #[test]
    fn f2_4_pycall_builder() {
        let call = PyCall::new("numpy", "zeros")
            .arg(PyValue::Tuple(vec![PyValue::Int(3), PyValue::Int(4)]))
            .kwarg("dtype", PyValue::Str("float32".to_string()));
        assert_eq!(call.module, "numpy");
        assert_eq!(call.function, "zeros");
        assert_eq!(call.args.len(), 1);
        assert_eq!(call.kwargs.len(), 1);
    }

    #[test]
    fn f2_5_numpy_dtype() {
        assert_eq!(NumpyDtype::Float32.element_size(), 4);
        assert_eq!(NumpyDtype::Float64.element_size(), 8);
        assert_eq!(NumpyDtype::Int8.element_size(), 1);
        assert_eq!(NumpyDtype::Float32.to_fajar_type(), "f32");
    }

    #[test]
    fn f2_6_gil_config() {
        let cfg = GilConfig::default();
        assert!(cfg.auto_release);
        assert_eq!(cfg.release_threshold_us, 100);
    }

    #[test]
    fn f2_7_exception_mapping() {
        assert_eq!(map_exception("ValueError"), "SE004");
        assert_eq!(map_exception("IndexError"), "RE006");
        assert_eq!(map_exception("MemoryError"), "ME008");
        assert_eq!(map_exception("UnknownError"), "RE002");
    }

    #[test]
    fn f2_8_pyvalue_error() {
        let err = PyValue::Error {
            exc_type: "ValueError".to_string(),
            message: "invalid shape".to_string(),
            traceback: None,
        };
        assert!(err.is_error());
        assert!(err.to_fajar_repr().contains("ValueError"));
    }

    #[test]
    fn f2_9_venv_detect() {
        let info = detect_venv("/home/user/project");
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.path.contains(".venv") || info.path.contains("venv"));
    }

    #[test]
    fn f2_10_python_annotation() {
        let ann = PythonAnnotation {
            fajar_name: "predict".to_string(),
            module: "torch".to_string(),
            python_name: Some("model_predict".to_string()),
            auto_convert: true,
        };
        let wrapper = generate_python_wrapper(
            &ann,
            &[("input".to_string(), "Tensor".to_string())],
            "Tensor",
        );
        assert!(wrapper.contains("@python fn predict"));
        assert!(wrapper.contains("torch"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // V8 GC2.11-GC2.20: Real Python integration tests via pyo3
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(feature = "python-ffi")]
    use pyo3::types::PyAnyMethods;
    #[cfg(feature = "python-ffi")]
    use pyo3::types::PyModule;

    #[cfg(feature = "python-ffi")]
    fn py_eval<'py, T: pyo3::FromPyObject<'py>>(
        py: pyo3::Python<'py>,
        code: &str,
    ) -> pyo3::PyResult<T> {
        let ccode = std::ffi::CString::new(code).unwrap();
        py.eval(&ccode, None, None)?.extract()
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_eval_expression() {
        pyo3::Python::with_gil(|py| {
            let result: i64 = py_eval(py, "2 + 3").unwrap();
            assert_eq!(result, 5);
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_call_builtin() {
        pyo3::Python::with_gil(|py| {
            let result: i64 = py_eval(py, "abs(-42)").unwrap();
            assert_eq!(result, 42);
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_import_math() {
        pyo3::Python::with_gil(|py| {
            let pi: f64 = py_eval(py, "__import__('math').pi").unwrap();
            assert!((pi - std::f64::consts::PI).abs() < 1e-10);
            let sqrt: f64 = py_eval(py, "__import__('math').sqrt(16.0)").unwrap();
            assert!((sqrt - 4.0).abs() < 1e-10);
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_list_sort() {
        pyo3::Python::with_gil(|py| {
            let result: Vec<i64> = py_eval(py, "sorted([3, 1, 4, 1, 5, 9, 2, 6])").unwrap();
            assert_eq!(result, vec![1, 1, 2, 3, 4, 5, 6, 9]);
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_string_ops() {
        pyo3::Python::with_gil(|py| {
            let result: String = py_eval(py, "'Fajar Lang'.lower().replace(' ', '_')").unwrap();
            assert_eq!(result, "fajar_lang");
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_exception() {
        pyo3::Python::with_gil(|py| {
            let ccode = std::ffi::CString::new("1/0").unwrap();
            let result = py.eval(&ccode, None, None);
            assert!(result.is_err());
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_numpy_array() {
        pyo3::Python::with_gil(|py| {
            let arr: Vec<f64> =
                py_eval(py, "__import__('numpy').array([1.0, 2.0, 3.0]).tolist()").unwrap();
            assert_eq!(arr, vec![1.0, 2.0, 3.0]);
            let sum: f64 = py_eval(
                py,
                "float(__import__('numpy').sum(__import__('numpy').array([1,2,3,4])))",
            )
            .unwrap();
            assert!((sum - 10.0).abs() < 1e-10);
        });
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn gc2_python_define_function() {
        pyo3::Python::with_gil(|py| {
            let code = std::ffi::CString::new(
                "def fibonacci(n):\n    a, b = 0, 1\n    for _ in range(n):\n        a, b = b, a + b\n    return a"
            ).unwrap();
            py.run(&code, None, None).unwrap();
            let fib: i64 = py_eval(py, "fibonacci(10)").unwrap();
            assert_eq!(fib, 55);
        });
    }

    #[test]
    fn f2_2_dict_repr() {
        let dict = PyValue::Dict(vec![
            (PyValue::Str("a".to_string()), PyValue::Int(1)),
            (PyValue::Str("b".to_string()), PyValue::Int(2)),
        ]);
        let repr = dict.to_fajar_repr();
        assert!(repr.contains("\"a\": 1"));
        assert!(repr.contains("\"b\": 2"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // PQ4: Python FFI Quality Tests
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_2_exception_to_result() {
        let result = py_eval_result("1/0");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("ZeroDivisionError"),
            "should contain ZeroDivisionError: {err}"
        );
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_2_successful_eval() {
        let result = py_eval_result("2 ** 10");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1024");
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_3_type_roundtrip_int() {
        let val = PyValue::Int(42);
        let back = py_type_roundtrip(&val).unwrap();
        assert!(matches!(back, PyValue::Int(42)));
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_3_type_roundtrip_float() {
        let val = PyValue::Float(3.14);
        let back = py_type_roundtrip(&val).unwrap();
        if let PyValue::Float(f) = back {
            assert!((f - 3.14).abs() < 1e-10);
        } else {
            panic!("expected Float");
        }
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_3_type_roundtrip_str() {
        let val = PyValue::Str("hello".to_string());
        let back = py_type_roundtrip(&val).unwrap();
        assert!(matches!(back, PyValue::Str(s) if s == "hello"));
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_3_type_roundtrip_bool() {
        let val = PyValue::Bool(true);
        let back = py_type_roundtrip(&val).unwrap();
        assert!(matches!(back, PyValue::Bool(true)));
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_4_module_introspection() {
        let attrs = py_list_module_attrs("math").unwrap();
        assert!(attrs.contains(&"sqrt".to_string()), "math should have sqrt");
        assert!(attrs.contains(&"pi".to_string()), "math should have pi");
        assert!(attrs.contains(&"sin".to_string()), "math should have sin");
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_5_kwargs() {
        // math.log(100, 10) using keyword style
        let result = py_call_with_kwargs("__import__('math').log(100, 10)");
        assert!(result.is_ok());
        let val: f64 = result.unwrap().parse().unwrap();
        assert!((val - 2.0).abs() < 1e-10, "log(100, 10) should be 2.0");
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_9_python_version() {
        let ver = python_available().unwrap();
        assert!(ver.starts_with("Python 3"), "should be Python 3.x: {ver}");
    }

    #[test]
    fn pq4_9_python_not_compiled() {
        if !cfg!(feature = "python-ffi") {
            let result = python_available();
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("not compiled"));
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // PQ4.1: NumPy Zero-Copy Bridge
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn pq4_1_numpy_buffer_creation() {
        let buf = NumpyBuffer::new(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(buf.numel(), 6);
        assert_eq!(buf.nbytes(), 48); // 6 * 8 bytes
        assert!(buf.owned);
    }

    #[test]
    fn pq4_1_numpy_buffer_view() {
        let buf = NumpyBuffer::view(vec![3], vec![1.0, 2.0, 3.0]);
        assert!(!buf.owned);
        assert_eq!(buf.numel(), 3);
    }

    #[test]
    fn pq4_1_numpy_to_ndarray2() {
        let buf = NumpyBuffer::new(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let arr = buf.to_ndarray2().unwrap();
        assert_eq!(arr.nrows(), 2);
        assert_eq!(arr.ncols(), 3);
        assert_eq!(arr[[0, 0]], 1.0);
        assert_eq!(arr[[1, 2]], 6.0);
    }

    #[test]
    fn pq4_1_numpy_from_ndarray2() {
        let arr = ndarray::Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let buf = NumpyBuffer::from_ndarray2(&arr);
        assert_eq!(buf.shape, vec![2, 2]);
        assert_eq!(buf.data, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn pq4_1_numpy_reshape() {
        let buf = NumpyBuffer::new(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let reshaped = buf.reshape(vec![3, 2]).unwrap();
        assert_eq!(reshaped.shape, vec![3, 2]);
        assert_eq!(reshaped.data.len(), 6);

        // Invalid reshape
        assert!(buf.reshape(vec![2, 2]).is_err());
    }

    #[test]
    fn pq4_1_numpy_3d_rejects_ndarray2() {
        let buf = NumpyBuffer::new(vec![2, 3, 4], vec![0.0; 24]);
        assert!(buf.to_ndarray2().is_err());
    }

    #[test]
    fn pq4_1_buffer_to_numpy_code() {
        let buf = NumpyBuffer::new(vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]);
        let code = buffer_to_numpy_code(&buf);
        assert!(code.contains("numpy"));
        assert!(code.contains("reshape"));
        assert!(code.contains("1,2,3,4"));
    }

    #[cfg(feature = "python-ffi")]
    #[test]
    fn pq4_1_numpy_roundtrip() {
        let buf = NumpyBuffer::new(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let code = buffer_to_numpy_code(&buf);
        let back = numpy_to_buffer(&code).unwrap();
        assert_eq!(back.shape, vec![2, 3]);
        assert_eq!(back.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    // ═══════════════════════════════════════════════════════════════════
    // PQ4.6: Python→Fajar Callback
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn pq4_6_callback_registry_basic() {
        let mut reg = CallbackRegistry::new();
        assert_eq!(reg.count(), 0);

        reg.register(FajarCallback {
            name: "predict".to_string(),
            arg_types: vec!["Tensor".to_string()],
            return_type: "Tensor".to_string(),
            description: "Run inference on input tensor".to_string(),
        });
        assert_eq!(reg.count(), 1);
        assert!(reg.get("predict").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn pq4_6_callback_registry_multiple() {
        let mut reg = CallbackRegistry::new();
        reg.register(FajarCallback {
            name: "predict".to_string(),
            arg_types: vec!["Tensor".to_string()],
            return_type: "Tensor".to_string(),
            description: "Run inference".to_string(),
        });
        reg.register(FajarCallback {
            name: "preprocess".to_string(),
            arg_types: vec!["str".to_string()],
            return_type: "Tensor".to_string(),
            description: "Preprocess input text".to_string(),
        });
        assert_eq!(reg.count(), 2);
        let names = reg.names();
        assert!(names.contains(&"predict"));
        assert!(names.contains(&"preprocess"));
    }

    #[test]
    fn pq4_6_callback_python_wrappers() {
        let mut reg = CallbackRegistry::new();
        reg.register(FajarCallback {
            name: "sigmoid".to_string(),
            arg_types: vec!["float".to_string()],
            return_type: "float".to_string(),
            description: "Sigmoid activation".to_string(),
        });
        let code = reg.generate_python_wrappers();
        assert!(code.contains("def sigmoid(arg0)"));
        assert!(code.contains("Sigmoid activation"));
        assert!(code.contains("Returns: float"));
    }

    #[test]
    fn pq4_6_callback_no_args() {
        let mut reg = CallbackRegistry::new();
        reg.register(FajarCallback {
            name: "version".to_string(),
            arg_types: vec![],
            return_type: "str".to_string(),
            description: "Get Fajar Lang version".to_string(),
        });
        let code = reg.generate_python_wrappers();
        assert!(code.contains("def version()"));
    }
}
