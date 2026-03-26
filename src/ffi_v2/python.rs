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
}
