//! FFI and interoperability infrastructure for Fajar Lang.
//!
//! Generates bindings and type mappings for cross-language integration:
//!
//! - **CBindgen** — C/C++ header generation from Fajar types
//! - **PyBindgen** — Python bindings generator (init.py + .pyi stubs)
//! - **WasmComponent** — WebAssembly Component Model (WIT) generation
//! - **PackageAuditor** — Security auditing for dependencies
//! - **InteropTypeMapper** — Universal type mapping between languages
//!
//! # Architecture
//!
//! ```text
//! Fajar Types ──► CBindgen       → .h headers
//!             ──► PyBindgen       → __init__.py + .pyi stubs
//!             ──► WasmComponent   → .wit interface definitions
//!             ──► TypeMapper      → type strings for any target
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from interop binding generation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum InteropError {
    /// A Fajar type has no equivalent in the target language.
    #[error("IO001: type '{fajar_type}' has no mapping to {target}")]
    UnsupportedTypeMapping {
        /// The Fajar type name.
        fajar_type: String,
        /// The target language.
        target: String,
    },

    /// Duplicate definition in generated bindings.
    #[error("IO002: duplicate definition '{name}' in {context}")]
    DuplicateDefinition {
        /// The duplicated name.
        name: String,
        /// Where the duplicate was found.
        context: String,
    },

    /// Invalid identifier for the target language.
    #[error("IO003: identifier '{name}' is invalid or reserved in {target}")]
    InvalidIdentifier {
        /// The problematic identifier.
        name: String,
        /// The target language.
        target: String,
    },

    /// SBOM generation failure.
    #[error("IO004: SBOM generation failed: {reason}")]
    SbomError {
        /// Description of what went wrong.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// 1. CBindgen — C/C++ header generation
// ═══════════════════════════════════════════════════════════════════════

/// C type representation for FFI header generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    /// C `void`.
    Void,
    /// C `int8_t`.
    Int8,
    /// C `int16_t`.
    Int16,
    /// C `int32_t`.
    Int32,
    /// C `int64_t`.
    Int64,
    /// C `uint8_t`.
    UInt8,
    /// C `uint16_t`.
    UInt16,
    /// C `uint32_t`.
    UInt32,
    /// C `uint64_t`.
    UInt64,
    /// C `float`.
    Float,
    /// C `double`.
    Double,
    /// C `_Bool`.
    Bool,
    /// C `const char*`.
    CharPtr,
    /// C `void*`.
    VoidPtr,
    /// C struct by name.
    Struct(String),
    /// C enum by name.
    Enum(String),
    /// C function pointer.
    FnPtr(Box<CFnSig>),
}

/// C function signature (return type + parameters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFnSig {
    /// Return type.
    pub return_type: CType,
    /// Parameter names and types.
    pub params: Vec<(String, CType)>,
}

/// C struct definition for header generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CStructDef {
    /// Struct name.
    pub name: String,
    /// Fields: (name, type).
    pub fields: Vec<(String, CType)>,
    /// Whether the struct uses `__attribute__((packed))`.
    pub packed: bool,
    /// Optional doc comment.
    pub doc_comment: Option<String>,
}

/// C enum definition for header generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CEnumDef {
    /// Enum name.
    pub name: String,
    /// Variants: (name, optional explicit value).
    pub variants: Vec<(String, Option<i64>)>,
    /// Optional doc comment.
    pub doc_comment: Option<String>,
}

/// C function declaration for header generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFunctionDecl {
    /// Function name.
    pub name: String,
    /// Return type.
    pub return_type: CType,
    /// Parameters: (name, type).
    pub params: Vec<(String, CType)>,
    /// Optional doc comment.
    pub doc_comment: Option<String>,
    /// Whether the function is variadic (`...`).
    pub is_variadic: bool,
}

/// Generates C/C++ header files from Fajar type definitions.
///
/// Produces standards-compliant C11 headers with:
/// - Include guards (`#ifndef` / `#define` / `#endif`)
/// - `extern "C"` for C++ compatibility
/// - `#include <stdint.h>` for fixed-width integer types
/// - Doc comment passthrough
#[derive(Debug, Clone, Default)]
pub struct CHeaderGenerator {
    /// Module name for include guard.
    module_name: String,
    /// Struct definitions to emit.
    structs: Vec<CStructDef>,
    /// Enum definitions to emit.
    enums: Vec<CEnumDef>,
    /// Function declarations to emit.
    functions: Vec<CFunctionDecl>,
}

impl CHeaderGenerator {
    /// Creates a new header generator for the given module name.
    pub fn new(module_name: &str) -> Self {
        Self {
            module_name: module_name.to_string(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Adds a struct definition to the header.
    pub fn add_struct(&mut self, def: CStructDef) {
        self.structs.push(def);
    }

    /// Adds an enum definition to the header.
    pub fn add_enum(&mut self, def: CEnumDef) {
        self.enums.push(def);
    }

    /// Adds a function declaration to the header.
    pub fn add_function(&mut self, decl: CFunctionDecl) {
        self.functions.push(decl);
    }

    /// Generates the complete C header as a string.
    pub fn generate(&self) -> String {
        let mut out = String::new();
        let guard = format!("FJ_{}_H", self.module_name.to_uppercase());

        // Include guard + standard includes
        out.push_str(&format!("#ifndef {guard}\n"));
        out.push_str(&format!("#define {guard}\n\n"));
        out.push_str("#include <stdint.h>\n");
        out.push_str("#include <stdbool.h>\n\n");

        // C++ extern "C" open
        out.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");

        // Forward declarations for structs
        for s in &self.structs {
            out.push_str(&format!("typedef struct {name} {name};\n", name = s.name));
        }
        if !self.structs.is_empty() {
            out.push('\n');
        }

        // Enums
        for e in &self.enums {
            self.emit_enum(&mut out, e);
        }

        // Structs
        for s in &self.structs {
            self.emit_struct(&mut out, s);
        }

        // Functions
        for f in &self.functions {
            self.emit_function(&mut out, f);
        }

        // C++ extern "C" close + include guard end
        out.push_str("#ifdef __cplusplus\n}\n#endif\n\n");
        out.push_str(&format!("#endif /* {guard} */\n"));

        out
    }
}

// -- CHeaderGenerator private helpers --

impl CHeaderGenerator {
    /// Emits a C enum definition.
    fn emit_enum(&self, out: &mut String, def: &CEnumDef) {
        if let Some(doc) = &def.doc_comment {
            out.push_str(&format_c_doc_comment(doc));
        }
        out.push_str(&format!("typedef enum {name} {{\n", name = def.name));
        for (i, (variant, value)) in def.variants.iter().enumerate() {
            let comma = if i + 1 < def.variants.len() { "," } else { "" };
            match value {
                Some(v) => out.push_str(&format!("    {variant} = {v}{comma}\n")),
                None => out.push_str(&format!("    {variant}{comma}\n")),
            }
        }
        out.push_str(&format!("}} {name};\n\n", name = def.name));
    }

    /// Emits a C struct definition.
    fn emit_struct(&self, out: &mut String, def: &CStructDef) {
        if let Some(doc) = &def.doc_comment {
            out.push_str(&format_c_doc_comment(doc));
        }
        out.push_str(&format!("struct {name} {{\n", name = def.name));
        for (field_name, field_type) in &def.fields {
            let type_str = ctype_to_string(field_type, Some(field_name));
            out.push_str(&format!("    {type_str};\n"));
        }
        out.push('}');
        if def.packed {
            out.push_str(" __attribute__((packed))");
        }
        out.push_str(";\n\n");
    }

    /// Emits a C function declaration.
    fn emit_function(&self, out: &mut String, decl: &CFunctionDecl) {
        if let Some(doc) = &decl.doc_comment {
            out.push_str(&format_c_doc_comment(doc));
        }
        let ret = ctype_to_string(&decl.return_type, None);
        let params = build_c_param_list(&decl.params, decl.is_variadic);
        out.push_str(&format!("{ret} {name}({params});\n\n", name = decl.name));
    }
}

/// Formats a doc comment string as a C block comment.
fn format_c_doc_comment(doc: &str) -> String {
    let mut out = String::from("/**\n");
    for line in doc.lines() {
        out.push_str(&format!(" * {line}\n"));
    }
    out.push_str(" */\n");
    out
}

/// Converts a `CType` to its C string representation.
///
/// If `name` is provided, it is appended (for variable/field declarations).
/// For function pointers, the name is embedded in the type syntax.
fn ctype_to_string(ty: &CType, name: Option<&str>) -> String {
    let base = match ty {
        CType::Void => "void".to_string(),
        CType::Int8 => "int8_t".to_string(),
        CType::Int16 => "int16_t".to_string(),
        CType::Int32 => "int32_t".to_string(),
        CType::Int64 => "int64_t".to_string(),
        CType::UInt8 => "uint8_t".to_string(),
        CType::UInt16 => "uint16_t".to_string(),
        CType::UInt32 => "uint32_t".to_string(),
        CType::UInt64 => "uint64_t".to_string(),
        CType::Float => "float".to_string(),
        CType::Double => "double".to_string(),
        CType::Bool => "_Bool".to_string(),
        CType::CharPtr => "const char*".to_string(),
        CType::VoidPtr => "void*".to_string(),
        CType::Struct(s) => format!("struct {s}"),
        CType::Enum(e) => e.clone(),
        CType::FnPtr(sig) => {
            let ret = ctype_to_string(&sig.return_type, None);
            let params = build_c_param_list(&sig.params, false);
            let n = name.unwrap_or("fn_ptr");
            return format!("{ret} (*{n})({params})");
        }
    };
    match name {
        Some(n) => format!("{base} {n}"),
        None => base,
    }
}

/// Builds a C parameter list string from typed parameter pairs.
fn build_c_param_list(params: &[(String, CType)], is_variadic: bool) -> String {
    if params.is_empty() && !is_variadic {
        return "void".to_string();
    }
    let mut parts: Vec<String> = params
        .iter()
        .map(|(name, ty)| ctype_to_string(ty, Some(name)))
        .collect();
    if is_variadic {
        parts.push("...".to_string());
    }
    parts.join(", ")
}

// ═══════════════════════════════════════════════════════════════════════
// 2. PyBindgen — Python bindings generator
// ═══════════════════════════════════════════════════════════════════════

/// Python type representation for binding generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyType {
    /// Python `int`.
    Int,
    /// Python `float`.
    Float,
    /// Python `str`.
    Str,
    /// Python `bool`.
    Bool,
    /// Python `list[T]`.
    List(Box<PyType>),
    /// Python `dict[K, V]`.
    Dict(Box<PyType>, Box<PyType>),
    /// Python `tuple[T, ...]`.
    Tuple(Vec<PyType>),
    /// Python `Optional[T]`.
    Optional(Box<PyType>),
    /// Python `None`.
    None,
    /// NumPy `numpy.ndarray`.
    NdArray,
    /// Custom Python class.
    Custom(String),
}

/// Python class definition for binding generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyClassDef {
    /// Class name.
    pub name: String,
    /// Fields: (name, type).
    pub fields: Vec<(String, PyType)>,
    /// Methods defined on this class.
    pub methods: Vec<PyFunctionDef>,
    /// Optional class docstring.
    pub doc_comment: Option<String>,
    /// Base classes (for inheritance).
    pub bases: Vec<String>,
}

/// Python function definition for binding generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyFunctionDef {
    /// Function name.
    pub name: String,
    /// Parameters: (name, type).
    pub params: Vec<(String, PyType)>,
    /// Return type.
    pub return_type: PyType,
    /// Optional docstring.
    pub doc_comment: Option<String>,
}

/// Python enum definition for binding generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyEnumDef {
    /// Enum name.
    pub name: String,
    /// Variants: (name, optional associated type).
    pub variants: Vec<(String, Option<PyType>)>,
}

/// Python module definition aggregating all bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyModuleDef {
    /// Module name (e.g., `"fj_math"`).
    pub name: String,
    /// Class definitions.
    pub classes: Vec<PyClassDef>,
    /// Top-level function definitions.
    pub functions: Vec<PyFunctionDef>,
    /// Enum definitions.
    pub enums: Vec<PyEnumDef>,
    /// Import statements (e.g., `"import numpy as np"`).
    pub imports: Vec<String>,
}

/// Generates Python bindings (`__init__.py` + `.pyi` stubs) from Fajar types.
///
/// Produces:
/// - `__init__.py` with class wrappers and function stubs
/// - `.pyi` type stub files for IDE support
/// - Error bridge: `Result<T,E>` maps to Python exceptions
/// - Tensor bridge: `Tensor` maps to `numpy.ndarray`
#[derive(Debug, Clone, Default)]
pub struct PyBindgenGenerator {
    /// Module name.
    module_name: String,
    /// Class definitions.
    classes: Vec<PyClassDef>,
    /// Top-level functions.
    functions: Vec<PyFunctionDef>,
    /// Enum definitions.
    enums: Vec<PyEnumDef>,
    /// Additional import lines.
    imports: Vec<String>,
}

impl PyBindgenGenerator {
    /// Creates a new Python bindings generator for the given module.
    pub fn new(module_name: &str) -> Self {
        Self {
            module_name: module_name.to_string(),
            classes: Vec::new(),
            functions: Vec::new(),
            enums: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// Adds a class definition to the module.
    pub fn add_class(&mut self, cls: PyClassDef) {
        self.classes.push(cls);
    }

    /// Adds a top-level function to the module.
    pub fn add_function(&mut self, func: PyFunctionDef) {
        self.functions.push(func);
    }

    /// Adds an enum definition to the module.
    pub fn add_enum(&mut self, enm: PyEnumDef) {
        self.enums.push(enm);
    }

    /// Adds a custom import line.
    pub fn add_import(&mut self, import: &str) {
        self.imports.push(import.to_string());
    }

    /// Generates the `__init__.py` file content.
    pub fn generate_init_py(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "\"\"\"Auto-generated Python bindings for Fajar Lang module '{}'.\"\"\"\n\n",
            self.module_name
        ));

        // Standard imports
        out.push_str("from typing import Optional, List, Dict, Tuple\n");
        if self.has_ndarray_type() {
            out.push_str("import numpy as np\n");
        }
        for imp in &self.imports {
            out.push_str(&format!("{imp}\n"));
        }
        out.push('\n');

        // Enums (as classes with class-level attributes)
        for enm in &self.enums {
            self.emit_py_enum(&mut out, enm);
        }

        // Classes
        for cls in &self.classes {
            self.emit_py_class(&mut out, cls);
        }

        // Functions
        for func in &self.functions {
            self.emit_py_function(&mut out, func, 0);
        }

        out
    }

    /// Generates `.pyi` type stub content for IDE support.
    pub fn generate_pyi_stubs(&self) -> String {
        let mut out = String::new();
        out.push_str("from typing import Optional, List, Dict, Tuple\n");
        if self.has_ndarray_type() {
            out.push_str("import numpy as np\n");
        }
        out.push('\n');

        // Enum stubs
        for enm in &self.enums {
            out.push_str(&format!("class {}:\n", enm.name));
            for (variant, _) in &enm.variants {
                out.push_str(&format!("    {variant}: int\n"));
            }
            out.push_str("    ...\n\n");
        }

        // Class stubs
        for cls in &self.classes {
            self.emit_pyi_class(&mut out, cls);
        }

        // Function stubs
        for func in &self.functions {
            self.emit_pyi_function(&mut out, func, 0);
        }

        out
    }
}

// -- PyBindgenGenerator private helpers --

impl PyBindgenGenerator {
    /// Checks whether any type in the module references `NdArray`.
    fn has_ndarray_type(&self) -> bool {
        let check_type = |t: &PyType| matches!(t, PyType::NdArray);
        self.functions
            .iter()
            .any(|f| check_type(&f.return_type) || f.params.iter().any(|(_, t)| check_type(t)))
            || self.classes.iter().any(|c| {
                c.fields.iter().any(|(_, t)| check_type(t))
                    || c.methods.iter().any(|m| {
                        check_type(&m.return_type) || m.params.iter().any(|(_, t)| check_type(t))
                    })
            })
    }

    /// Emits a Python enum as a class with integer constants.
    fn emit_py_enum(&self, out: &mut String, enm: &PyEnumDef) {
        out.push_str(&format!("class {}:\n", enm.name));
        for (i, (variant, _)) in enm.variants.iter().enumerate() {
            out.push_str(&format!("    {variant} = {i}\n"));
        }
        out.push('\n');
    }

    /// Emits a Python class definition.
    fn emit_py_class(&self, out: &mut String, cls: &PyClassDef) {
        let bases = if cls.bases.is_empty() {
            String::new()
        } else {
            format!("({})", cls.bases.join(", "))
        };
        out.push_str(&format!("class {}{bases}:\n", cls.name));
        if let Some(doc) = &cls.doc_comment {
            out.push_str(&format!("    \"\"\"{doc}\"\"\"\n\n"));
        }

        // __init__ with fields
        self.emit_py_init(out, cls);

        // Methods
        for method in &cls.methods {
            self.emit_py_function(out, method, 1);
        }

        if cls.fields.is_empty() && cls.methods.is_empty() {
            out.push_str("    pass\n");
        }
        out.push('\n');
    }

    /// Emits the `__init__` method for a class.
    fn emit_py_init(&self, out: &mut String, cls: &PyClassDef) {
        if cls.fields.is_empty() {
            return;
        }
        let params: Vec<String> = cls
            .fields
            .iter()
            .map(|(n, t)| format!("{n}: {}", pytype_to_string(t)))
            .collect();
        out.push_str(&format!("    def __init__(self, {}):\n", params.join(", ")));
        for (name, _) in &cls.fields {
            out.push_str(&format!("        self.{name} = {name}\n"));
        }
        out.push('\n');
    }

    /// Emits a Python function definition at the given indentation level.
    fn emit_py_function(&self, out: &mut String, func: &PyFunctionDef, indent: usize) {
        let pad = "    ".repeat(indent);
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {}", pytype_to_string(t)))
            .collect();
        let ret = pytype_to_string(&func.return_type);
        out.push_str(&format!(
            "{pad}def {}({}) -> {ret}:\n",
            func.name,
            params.join(", ")
        ));
        if let Some(doc) = &func.doc_comment {
            out.push_str(&format!("{pad}    \"\"\"{doc}\"\"\"\n"));
        }
        out.push_str(&format!("{pad}    ...\n\n"));
    }

    /// Emits a `.pyi` class stub.
    fn emit_pyi_class(&self, out: &mut String, cls: &PyClassDef) {
        let bases = if cls.bases.is_empty() {
            String::new()
        } else {
            format!("({})", cls.bases.join(", "))
        };
        out.push_str(&format!("class {}{bases}:\n", cls.name));
        for (name, ty) in &cls.fields {
            out.push_str(&format!("    {name}: {}\n", pytype_to_string(ty)));
        }
        for method in &cls.methods {
            self.emit_pyi_function(out, method, 1);
        }
        if cls.fields.is_empty() && cls.methods.is_empty() {
            out.push_str("    ...\n");
        }
        out.push('\n');
    }

    /// Emits a `.pyi` function stub at the given indentation level.
    fn emit_pyi_function(&self, out: &mut String, func: &PyFunctionDef, indent: usize) {
        let pad = "    ".repeat(indent);
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {}", pytype_to_string(t)))
            .collect();
        let ret = pytype_to_string(&func.return_type);
        out.push_str(&format!(
            "{pad}def {}({}) -> {ret}: ...\n",
            func.name,
            params.join(", ")
        ));
    }
}

/// Converts a `PyType` to its Python type annotation string.
fn pytype_to_string(ty: &PyType) -> String {
    match ty {
        PyType::Int => "int".to_string(),
        PyType::Float => "float".to_string(),
        PyType::Str => "str".to_string(),
        PyType::Bool => "bool".to_string(),
        PyType::List(inner) => format!("List[{}]", pytype_to_string(inner)),
        PyType::Dict(k, v) => {
            format!("Dict[{}, {}]", pytype_to_string(k), pytype_to_string(v))
        }
        PyType::Tuple(elems) => {
            let inner: Vec<String> = elems.iter().map(pytype_to_string).collect();
            format!("Tuple[{}]", inner.join(", "))
        }
        PyType::Optional(inner) => format!("Optional[{}]", pytype_to_string(inner)),
        PyType::None => "None".to_string(),
        PyType::NdArray => "np.ndarray".to_string(),
        PyType::Custom(name) => name.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. WasmComponent — WebAssembly Component Model (WIT)
// ═══════════════════════════════════════════════════════════════════════

/// WebAssembly Interface Type (WIT) type representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitType {
    /// WIT `u8`.
    U8,
    /// WIT `u16`.
    U16,
    /// WIT `u32`.
    U32,
    /// WIT `u64`.
    U64,
    /// WIT `s8`.
    S8,
    /// WIT `s16`.
    S16,
    /// WIT `s32`.
    S32,
    /// WIT `s64`.
    S64,
    /// WIT `f32`.
    F32,
    /// WIT `f64`.
    F64,
    /// WIT `bool`.
    Bool,
    /// WIT `char`.
    Char,
    /// WIT `string`.
    WitString,
    /// WIT `list<T>`.
    List(Box<WitType>),
    /// WIT `option<T>`.
    Option(Box<WitType>),
    /// WIT `result<T, E>`.
    Result(Box<WitType>, Box<WitType>),
    /// WIT `record { ... }`.
    Record(Vec<(String, WitType)>),
    /// WIT `variant { ... }`.
    Variant(Vec<(String, Option<WitType>)>),
    /// WIT resource handle.
    Resource(String),
}

/// A named type definition in a WIT interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitTypeDef {
    /// Type name.
    pub name: String,
    /// The WIT type definition.
    pub kind: WitType,
}

/// A function definition in a WIT interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitFunction {
    /// Function name.
    pub name: String,
    /// Parameters: (name, type).
    pub params: Vec<(String, WitType)>,
    /// Result types (WIT supports multiple returns).
    pub results: Vec<WitType>,
}

/// A WIT interface grouping related types and functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitInterface {
    /// Interface name.
    pub name: String,
    /// Functions in this interface.
    pub functions: Vec<WitFunction>,
    /// Type definitions in this interface.
    pub types: Vec<WitTypeDef>,
}

/// A WIT world definition (top-level component description).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitWorld {
    /// World name.
    pub name: String,
    /// Imported interfaces.
    pub imports: Vec<WitInterface>,
    /// Exported interfaces.
    pub exports: Vec<WitInterface>,
}

/// Generates WebAssembly Interface Type (WIT) definitions from Fajar types.
///
/// Produces `.wit` files conforming to the Component Model specification.
#[derive(Debug, Clone, Default)]
pub struct WitGenerator {
    /// Interface name.
    interface_name: String,
    /// Functions to expose.
    functions: Vec<WitFunction>,
    /// Type definitions.
    types: Vec<WitTypeDef>,
}

impl WitGenerator {
    /// Creates a new WIT generator for the given interface name.
    pub fn new(interface_name: &str) -> Self {
        Self {
            interface_name: interface_name.to_string(),
            functions: Vec::new(),
            types: Vec::new(),
        }
    }

    /// Adds a function to the WIT interface.
    pub fn add_function(&mut self, func: WitFunction) {
        self.functions.push(func);
    }

    /// Adds a type definition to the WIT interface.
    pub fn add_type(&mut self, typedef: WitTypeDef) {
        self.types.push(typedef);
    }

    /// Generates the complete `.wit` file content.
    pub fn generate_wit(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("interface {} {{\n", self.interface_name));

        // Type definitions
        for td in &self.types {
            emit_wit_typedef(&mut out, td, 1);
        }
        if !self.types.is_empty() && !self.functions.is_empty() {
            out.push('\n');
        }

        // Functions
        for func in &self.functions {
            emit_wit_function(&mut out, func, 1);
        }

        out.push_str("}\n");
        out
    }

    /// Generates a complete WIT world containing this interface as an export.
    pub fn generate_world(&self, world_name: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!("world {world_name} {{\n"));
        out.push_str(&format!("    export {};\n", self.interface_name));
        out.push_str("}\n\n");
        out.push_str(&self.generate_wit());
        out
    }
}

/// Emits a WIT type definition at the given indent level.
fn emit_wit_typedef(out: &mut String, td: &WitTypeDef, indent: usize) {
    let pad = "    ".repeat(indent);
    match &td.kind {
        WitType::Record(fields) => {
            out.push_str(&format!("{pad}record {} {{\n", td.name));
            for (name, ty) in fields {
                let inner_pad = "    ".repeat(indent + 1);
                out.push_str(&format!("{inner_pad}{name}: {},\n", wittype_to_string(ty)));
            }
            out.push_str(&format!("{pad}}}\n"));
        }
        WitType::Variant(variants) => {
            out.push_str(&format!("{pad}variant {} {{\n", td.name));
            for (name, payload) in variants {
                let inner_pad = "    ".repeat(indent + 1);
                match payload {
                    Some(ty) => {
                        out.push_str(&format!("{inner_pad}{name}({}),\n", wittype_to_string(ty)))
                    }
                    None => out.push_str(&format!("{inner_pad}{name},\n")),
                }
            }
            out.push_str(&format!("{pad}}}\n"));
        }
        other => {
            out.push_str(&format!(
                "{pad}type {} = {};\n",
                td.name,
                wittype_to_string(other)
            ));
        }
    }
}

/// Emits a WIT function definition at the given indent level.
fn emit_wit_function(out: &mut String, func: &WitFunction, indent: usize) {
    let pad = "    ".repeat(indent);
    let params: Vec<String> = func
        .params
        .iter()
        .map(|(n, t)| format!("{n}: {}", wittype_to_string(t)))
        .collect();
    let results = match func.results.len() {
        0 => String::new(),
        1 => format!(" -> {}", wittype_to_string(&func.results[0])),
        _ => {
            let r: Vec<String> = func.results.iter().map(wittype_to_string).collect();
            format!(" -> ({})", r.join(", "))
        }
    };
    out.push_str(&format!(
        "{pad}{name}: func({params}){results};\n",
        name = func.name,
        params = params.join(", ")
    ));
}

/// Converts a `WitType` to its WIT string representation.
fn wittype_to_string(ty: &WitType) -> String {
    match ty {
        WitType::U8 => "u8".to_string(),
        WitType::U16 => "u16".to_string(),
        WitType::U32 => "u32".to_string(),
        WitType::U64 => "u64".to_string(),
        WitType::S8 => "s8".to_string(),
        WitType::S16 => "s16".to_string(),
        WitType::S32 => "s32".to_string(),
        WitType::S64 => "s64".to_string(),
        WitType::F32 => "f32".to_string(),
        WitType::F64 => "f64".to_string(),
        WitType::Bool => "bool".to_string(),
        WitType::Char => "char".to_string(),
        WitType::WitString => "string".to_string(),
        WitType::List(inner) => format!("list<{}>", wittype_to_string(inner)),
        WitType::Option(inner) => format!("option<{}>", wittype_to_string(inner)),
        WitType::Result(ok, err) => {
            format!(
                "result<{}, {}>",
                wittype_to_string(ok),
                wittype_to_string(err)
            )
        }
        WitType::Record(fields) => {
            let f: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{n}: {}", wittype_to_string(t)))
                .collect();
            format!("record {{ {} }}", f.join(", "))
        }
        WitType::Variant(variants) => {
            let v: Vec<String> = variants
                .iter()
                .map(|(n, t)| match t {
                    Some(ty) => format!("{n}({})", wittype_to_string(ty)),
                    None => n.clone(),
                })
                .collect();
            format!("variant {{ {} }}", v.join(", "))
        }
        WitType::Resource(name) => format!("resource {name}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. PackageAuditor — Security auditing
// ═══════════════════════════════════════════════════════════════════════

/// Severity level for security advisories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Informational notice.
    Info,
    /// Low-severity issue.
    Low,
    /// Medium-severity issue.
    Medium,
    /// High-severity issue.
    High,
    /// Critical vulnerability requiring immediate action.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Low => write!(f, "low"),
            Severity::Medium => write!(f, "medium"),
            Severity::High => write!(f, "high"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

/// A security advisory for a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    /// Advisory identifier (e.g., "FJ-2026-001").
    pub id: String,
    /// Affected package name.
    pub package: String,
    /// Affected version range (e.g., "< 1.2.3").
    pub affected_versions: String,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable description.
    pub description: String,
    /// Version that fixes the issue, if available.
    pub fixed_in: Option<String>,
    /// URL for more information.
    pub url: Option<String>,
}

/// A vulnerability report for a specific package version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VulnerabilityReport {
    /// Package name.
    pub package_name: String,
    /// Package version.
    pub version: String,
    /// The advisory that matched.
    pub advisory: Advisory,
    /// Whether a fix is available.
    pub fix_available: bool,
}

/// License type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LicenseType {
    /// MIT License.
    MIT,
    /// Apache License 2.0.
    Apache2,
    /// BSD 2-Clause License.
    BSD2,
    /// BSD 3-Clause License.
    BSD3,
    /// GNU General Public License v2.
    GPL2,
    /// GNU General Public License v3.
    GPL3,
    /// GNU Lesser General Public License.
    LGPL,
    /// Mozilla Public License 2.0.
    MPL2,
    /// ISC License.
    ISC,
    /// The Unlicense.
    Unlicense,
    /// Proprietary / closed-source.
    Proprietary,
    /// License could not be determined.
    Unknown,
}

impl std::fmt::Display for LicenseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseType::MIT => write!(f, "MIT"),
            LicenseType::Apache2 => write!(f, "Apache-2.0"),
            LicenseType::BSD2 => write!(f, "BSD-2-Clause"),
            LicenseType::BSD3 => write!(f, "BSD-3-Clause"),
            LicenseType::GPL2 => write!(f, "GPL-2.0"),
            LicenseType::GPL3 => write!(f, "GPL-3.0"),
            LicenseType::LGPL => write!(f, "LGPL"),
            LicenseType::MPL2 => write!(f, "MPL-2.0"),
            LicenseType::ISC => write!(f, "ISC"),
            LicenseType::Unlicense => write!(f, "Unlicense"),
            LicenseType::Proprietary => write!(f, "Proprietary"),
            LicenseType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// License compatibility report for a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseReport {
    /// Package name.
    pub package_name: String,
    /// Detected license type.
    pub license: LicenseType,
    /// Whether this license is compatible with the project.
    pub compatible: bool,
}

/// Complete audit result for a dependency tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditResult {
    /// Vulnerability reports found.
    pub vulnerabilities: Vec<VulnerabilityReport>,
    /// License reports for all packages.
    pub licenses: Vec<LicenseReport>,
    /// Total number of packages audited.
    pub total_packages: usize,
    /// Number of packages with no issues.
    pub clean_packages: usize,
}

impl AuditResult {
    /// Returns true if no vulnerabilities were found.
    pub fn is_clean(&self) -> bool {
        self.vulnerabilities.is_empty()
    }

    /// Returns the highest severity among all vulnerabilities.
    pub fn max_severity(&self) -> Option<Severity> {
        self.vulnerabilities
            .iter()
            .map(|v| v.advisory.severity)
            .max()
    }

    /// Counts vulnerabilities at or above the given severity.
    pub fn count_at_severity(&self, min: Severity) -> usize {
        self.vulnerabilities
            .iter()
            .filter(|v| v.advisory.severity >= min)
            .count()
    }
}

/// A dependency descriptor for auditing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Package name.
    pub name: String,
    /// Package version (semver string).
    pub version: String,
    /// License identifier (SPDX).
    pub license: Option<String>,
}

/// Audits package dependencies for vulnerabilities and license issues.
///
/// Checks against an advisory database and validates license compatibility.
#[derive(Debug, Clone, Default)]
pub struct PackageAuditor {
    /// Known advisories database.
    advisories: Vec<Advisory>,
    /// Licenses considered compatible.
    allowed_licenses: Vec<LicenseType>,
}

impl PackageAuditor {
    /// Creates a new auditor with default permissive license list.
    pub fn new() -> Self {
        Self {
            advisories: Vec::new(),
            allowed_licenses: vec![
                LicenseType::MIT,
                LicenseType::Apache2,
                LicenseType::BSD2,
                LicenseType::BSD3,
                LicenseType::ISC,
                LicenseType::Unlicense,
            ],
        }
    }

    /// Adds an advisory to the database.
    pub fn add_advisory(&mut self, advisory: Advisory) {
        self.advisories.push(advisory);
    }

    /// Sets the list of allowed license types.
    pub fn set_allowed_licenses(&mut self, licenses: Vec<LicenseType>) {
        self.allowed_licenses = licenses;
    }

    /// Audits a list of dependencies against the advisory database.
    pub fn audit(&self, dependencies: &[Dependency]) -> AuditResult {
        let mut vulns = Vec::new();
        let mut licenses = Vec::new();

        for dep in dependencies {
            // Check advisories
            for adv in &self.advisories {
                if adv.package == dep.name && version_in_range(&dep.version, &adv.affected_versions)
                {
                    vulns.push(VulnerabilityReport {
                        package_name: dep.name.clone(),
                        version: dep.version.clone(),
                        advisory: adv.clone(),
                        fix_available: adv.fixed_in.is_some(),
                    });
                }
            }

            // Check license
            let license_type = parse_license_spdx(dep.license.as_deref());
            let compatible = self.allowed_licenses.contains(&license_type);
            licenses.push(LicenseReport {
                package_name: dep.name.clone(),
                license: license_type,
                compatible,
            });
        }

        let total = dependencies.len();
        let vuln_packages: std::collections::HashSet<&str> =
            vulns.iter().map(|v| v.package_name.as_str()).collect();
        let incompat_packages: std::collections::HashSet<&str> = licenses
            .iter()
            .filter(|l| !l.compatible)
            .map(|l| l.package_name.as_str())
            .collect();
        let problem_count = vuln_packages.len() + incompat_packages.len();
        let clean = total.saturating_sub(problem_count);

        AuditResult {
            vulnerabilities: vulns,
            licenses,
            total_packages: total,
            clean_packages: clean,
        }
    }

    /// Checks only license compatibility for the given dependencies.
    pub fn check_licenses(&self, dependencies: &[Dependency]) -> Vec<LicenseReport> {
        dependencies
            .iter()
            .map(|dep| {
                let license_type = parse_license_spdx(dep.license.as_deref());
                let compatible = self.allowed_licenses.contains(&license_type);
                LicenseReport {
                    package_name: dep.name.clone(),
                    license: license_type,
                    compatible,
                }
            })
            .collect()
    }
}

/// Parses an SPDX license identifier string into a `LicenseType`.
fn parse_license_spdx(spdx: Option<&str>) -> LicenseType {
    match spdx {
        Some("MIT") => LicenseType::MIT,
        Some("Apache-2.0") => LicenseType::Apache2,
        Some("BSD-2-Clause") => LicenseType::BSD2,
        Some("BSD-3-Clause") => LicenseType::BSD3,
        Some("GPL-2.0" | "GPL-2.0-only" | "GPL-2.0-or-later") => LicenseType::GPL2,
        Some("GPL-3.0" | "GPL-3.0-only" | "GPL-3.0-or-later") => LicenseType::GPL3,
        Some("LGPL-2.1" | "LGPL-3.0" | "LGPL-2.1-only" | "LGPL-2.1-or-later") => LicenseType::LGPL,
        Some("MPL-2.0") => LicenseType::MPL2,
        Some("ISC") => LicenseType::ISC,
        Some("Unlicense") => LicenseType::Unlicense,
        Some("Proprietary") => LicenseType::Proprietary,
        _ => LicenseType::Unknown,
    }
}

/// Simple version range check for advisory matching.
///
/// Supports patterns: `"< X.Y.Z"`, `"<= X.Y.Z"`, `">= X.Y.Z"`, `"*"`, exact match.
fn version_in_range(version: &str, range: &str) -> bool {
    let range = range.trim();
    if range == "*" {
        return true;
    }
    if let Some(upper) = range.strip_prefix("< ") {
        return compare_versions(version, upper.trim()) == std::cmp::Ordering::Less;
    }
    if let Some(upper) = range.strip_prefix("<= ") {
        let cmp = compare_versions(version, upper.trim());
        return cmp == std::cmp::Ordering::Less || cmp == std::cmp::Ordering::Equal;
    }
    if let Some(lower) = range.strip_prefix(">= ") {
        let cmp = compare_versions(version, lower.trim());
        return cmp == std::cmp::Ordering::Greater || cmp == std::cmp::Ordering::Equal;
    }
    // Exact match
    version == range
}

/// Compares two semver-like version strings lexicographically by parts.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse =
        |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse::<u64>().ok()).collect() };
    let va = parse(a);
    let vb = parse(b);
    let max_len = va.len().max(vb.len());
    for i in 0..max_len {
        let pa = va.get(i).copied().unwrap_or(0);
        let pb = vb.get(i).copied().unwrap_or(0);
        match pa.cmp(&pb) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

// -- SBOM Generator (CycloneDX JSON format) --

/// Generates a Software Bill of Materials in CycloneDX JSON format.
#[derive(Debug, Clone, Default)]
pub struct SbomGenerator {
    /// Project name.
    project_name: String,
    /// Project version.
    project_version: String,
}

impl SbomGenerator {
    /// Creates a new SBOM generator for the given project.
    pub fn new(project_name: &str, project_version: &str) -> Self {
        Self {
            project_name: project_name.to_string(),
            project_version: project_version.to_string(),
        }
    }

    /// Generates a CycloneDX SBOM JSON string from dependencies.
    pub fn generate(&self, dependencies: &[Dependency]) -> Result<String, InteropError> {
        let components = self.build_components(dependencies);
        let sbom = format!(
            "{{\
\n  \"bomFormat\": \"CycloneDX\",\
\n  \"specVersion\": \"1.5\",\
\n  \"version\": 1,\
\n  \"metadata\": {{\
\n    \"component\": {{\
\n      \"type\": \"application\",\
\n      \"name\": \"{name}\",\
\n      \"version\": \"{version}\"\
\n    }}\
\n  }},\
\n  \"components\": [{components}\
\n  ]\
\n}}",
            name = self.project_name,
            version = self.project_version,
        );
        Ok(sbom)
    }

    /// Builds the JSON components array content.
    fn build_components(&self, deps: &[Dependency]) -> String {
        let entries: Vec<String> = deps
            .iter()
            .map(|dep| {
                let license_str = dep.license.as_deref().unwrap_or("NOASSERTION");
                format!(
                    "\n    {{\
\n      \"type\": \"library\",\
\n      \"name\": \"{name}\",\
\n      \"version\": \"{version}\",\
\n      \"licenses\": [{{ \"license\": {{ \"id\": \"{license}\" }} }}]\
\n    }}",
                    name = dep.name,
                    version = dep.version,
                    license = license_str,
                )
            })
            .collect();
        entries.join(",")
    }
}

// -- Yank Manager --

/// Manages yanked package versions.
///
/// Yanked versions are marked as withdrawn and produce warnings on install.
#[derive(Debug, Clone, Default)]
pub struct YankManager {
    /// Map of package name to list of yanked versions with reason.
    yanked: HashMap<String, Vec<(String, String)>>,
}

impl YankManager {
    /// Creates a new empty yank manager.
    pub fn new() -> Self {
        Self {
            yanked: HashMap::new(),
        }
    }

    /// Marks a package version as yanked with a reason.
    pub fn yank(&mut self, package: &str, version: &str, reason: &str) {
        self.yanked
            .entry(package.to_string())
            .or_default()
            .push((version.to_string(), reason.to_string()));
    }

    /// Checks whether a specific package version is yanked.
    pub fn is_yanked(&self, package: &str, version: &str) -> bool {
        self.yanked
            .get(package)
            .map(|versions| versions.iter().any(|(v, _)| v == version))
            .unwrap_or(false)
    }

    /// Returns the yank reason for a package version, if yanked.
    pub fn yank_reason(&self, package: &str, version: &str) -> Option<&str> {
        self.yanked.get(package).and_then(|versions| {
            versions
                .iter()
                .find(|(v, _)| v == version)
                .map(|(_, reason)| reason.as_str())
        })
    }

    /// Lists all yanked versions for a package.
    pub fn yanked_versions(&self, package: &str) -> Vec<(&str, &str)> {
        self.yanked
            .get(package)
            .map(|versions| {
                versions
                    .iter()
                    .map(|(v, r)| (v.as_str(), r.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Removes a yank (un-yank) for a specific version.
    pub fn unyank(&mut self, package: &str, version: &str) -> bool {
        if let Some(versions) = self.yanked.get_mut(package) {
            let before = versions.len();
            versions.retain(|(v, _)| v != version);
            return versions.len() < before;
        }
        false
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. InteropTypeMapper — Universal type mapping
// ═══════════════════════════════════════════════════════════════════════

/// Fajar Lang type representation for interop mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FajarType {
    /// 8-bit signed integer.
    I8,
    /// 16-bit signed integer.
    I16,
    /// 32-bit signed integer.
    I32,
    /// 64-bit signed integer.
    I64,
    /// 8-bit unsigned integer.
    U8,
    /// 16-bit unsigned integer.
    U16,
    /// 32-bit unsigned integer.
    U32,
    /// 64-bit unsigned integer.
    U64,
    /// 32-bit floating point.
    F32,
    /// 64-bit floating point.
    F64,
    /// Boolean.
    Bool,
    /// String.
    Str,
    /// Homogeneous array.
    Array(Box<FajarType>),
    /// Tensor (ML tensor type).
    Tensor,
    /// Named struct.
    Struct(String),
    /// Named enum.
    Enum(String),
    /// Function pointer.
    FnPtr,
    /// Void pointer (raw).
    VoidPtr,
    /// Void (no value).
    Void,
}

/// Target language for type mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetLanguage {
    /// C language.
    C,
    /// C++ language.
    Cpp,
    /// Python language.
    Python,
    /// JavaScript language.
    JavaScript,
    /// WebAssembly (WIT).
    Wasm,
}

impl std::fmt::Display for TargetLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetLanguage::C => write!(f, "C"),
            TargetLanguage::Cpp => write!(f, "C++"),
            TargetLanguage::Python => write!(f, "Python"),
            TargetLanguage::JavaScript => write!(f, "JavaScript"),
            TargetLanguage::Wasm => write!(f, "WebAssembly"),
        }
    }
}

/// Size and alignment information for a type in a target language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    /// Size in bytes.
    pub size: usize,
    /// Alignment in bytes.
    pub alignment: usize,
}

/// Universal type mapper between Fajar Lang and target languages.
///
/// Provides string representations and layout information for every
/// Fajar type in every supported target language.
#[derive(Debug, Clone, Default)]
pub struct TypeMapper;

impl TypeMapper {
    /// Creates a new type mapper.
    pub fn new() -> Self {
        Self
    }

    /// Maps a Fajar type to its string representation in the target language.
    pub fn map_type(
        &self,
        fajar_type: &FajarType,
        target: TargetLanguage,
    ) -> Result<String, InteropError> {
        match target {
            TargetLanguage::C => self.map_to_c(fajar_type),
            TargetLanguage::Cpp => self.map_to_cpp(fajar_type),
            TargetLanguage::Python => self.map_to_python(fajar_type),
            TargetLanguage::JavaScript => self.map_to_javascript(fajar_type),
            TargetLanguage::Wasm => self.map_to_wasm(fajar_type),
        }
    }

    /// Returns size and alignment for a Fajar type (64-bit target).
    pub fn type_layout(&self, fajar_type: &FajarType) -> TypeLayout {
        match fajar_type {
            FajarType::I8 | FajarType::U8 | FajarType::Bool => TypeLayout {
                size: 1,
                alignment: 1,
            },
            FajarType::I16 | FajarType::U16 => TypeLayout {
                size: 2,
                alignment: 2,
            },
            FajarType::I32 | FajarType::U32 | FajarType::F32 => TypeLayout {
                size: 4,
                alignment: 4,
            },
            FajarType::I64 | FajarType::U64 | FajarType::F64 => TypeLayout {
                size: 8,
                alignment: 8,
            },
            FajarType::Str | FajarType::VoidPtr | FajarType::FnPtr => TypeLayout {
                size: 8,
                alignment: 8,
            },
            FajarType::Array(_) | FajarType::Tensor => TypeLayout {
                size: 16,
                alignment: 8,
            },
            FajarType::Struct(_) | FajarType::Enum(_) => TypeLayout {
                size: 8,
                alignment: 8,
            },
            FajarType::Void => TypeLayout {
                size: 0,
                alignment: 1,
            },
        }
    }
}

// -- TypeMapper private helpers --

impl TypeMapper {
    /// Maps to C type string.
    fn map_to_c(&self, ty: &FajarType) -> Result<String, InteropError> {
        let s = match ty {
            FajarType::I8 => "int8_t",
            FajarType::I16 => "int16_t",
            FajarType::I32 => "int32_t",
            FajarType::I64 => "int64_t",
            FajarType::U8 => "uint8_t",
            FajarType::U16 => "uint16_t",
            FajarType::U32 => "uint32_t",
            FajarType::U64 => "uint64_t",
            FajarType::F32 => "float",
            FajarType::F64 => "double",
            FajarType::Bool => "_Bool",
            FajarType::Str => "const char*",
            FajarType::VoidPtr => "void*",
            FajarType::FnPtr => "void (*)(void)",
            FajarType::Void => "void",
            FajarType::Array(inner) => {
                let inner_s = self.map_to_c(inner)?;
                return Ok(format!("{inner_s}*"));
            }
            FajarType::Tensor => "fj_tensor_t*",
            FajarType::Struct(name) => return Ok(format!("struct {name}")),
            FajarType::Enum(name) => return Ok(name.clone()),
        };
        Ok(s.to_string())
    }

    /// Maps to C++ type string.
    fn map_to_cpp(&self, ty: &FajarType) -> Result<String, InteropError> {
        let s = match ty {
            FajarType::I8 => "std::int8_t",
            FajarType::I16 => "std::int16_t",
            FajarType::I32 => "std::int32_t",
            FajarType::I64 => "std::int64_t",
            FajarType::U8 => "std::uint8_t",
            FajarType::U16 => "std::uint16_t",
            FajarType::U32 => "std::uint32_t",
            FajarType::U64 => "std::uint64_t",
            FajarType::F32 => "float",
            FajarType::F64 => "double",
            FajarType::Bool => "bool",
            FajarType::Str => "std::string",
            FajarType::VoidPtr => "void*",
            FajarType::FnPtr => "std::function<void()>",
            FajarType::Void => "void",
            FajarType::Array(inner) => {
                let inner_s = self.map_to_cpp(inner)?;
                return Ok(format!("std::vector<{inner_s}>"));
            }
            FajarType::Tensor => "fj::Tensor",
            FajarType::Struct(name) => return Ok(name.clone()),
            FajarType::Enum(name) => return Ok(name.clone()),
        };
        Ok(s.to_string())
    }

    /// Maps to Python type string.
    fn map_to_python(&self, ty: &FajarType) -> Result<String, InteropError> {
        let s = match ty {
            FajarType::I8
            | FajarType::I16
            | FajarType::I32
            | FajarType::I64
            | FajarType::U8
            | FajarType::U16
            | FajarType::U32
            | FajarType::U64 => "int",
            FajarType::F32 | FajarType::F64 => "float",
            FajarType::Bool => "bool",
            FajarType::Str => "str",
            FajarType::Void => "None",
            FajarType::Tensor => "numpy.ndarray",
            FajarType::Array(inner) => {
                let inner_s = self.map_to_python(inner)?;
                return Ok(format!("list[{inner_s}]"));
            }
            FajarType::Struct(name) | FajarType::Enum(name) => return Ok(name.clone()),
            FajarType::VoidPtr | FajarType::FnPtr => "int",
        };
        Ok(s.to_string())
    }

    /// Maps to JavaScript type string (JSDoc/TypeScript annotations).
    fn map_to_javascript(&self, ty: &FajarType) -> Result<String, InteropError> {
        let s = match ty {
            FajarType::I8
            | FajarType::I16
            | FajarType::I32
            | FajarType::U8
            | FajarType::U16
            | FajarType::U32
            | FajarType::F32
            | FajarType::F64 => "number",
            FajarType::I64 | FajarType::U64 => "bigint",
            FajarType::Bool => "boolean",
            FajarType::Str => "string",
            FajarType::Void => "void",
            FajarType::Tensor => "Float64Array",
            FajarType::Array(inner) => {
                let inner_s = self.map_to_javascript(inner)?;
                return Ok(format!("Array<{inner_s}>"));
            }
            FajarType::Struct(name) | FajarType::Enum(name) => return Ok(name.clone()),
            FajarType::VoidPtr => "number",
            FajarType::FnPtr => "Function",
        };
        Ok(s.to_string())
    }

    /// Maps to WIT type string.
    fn map_to_wasm(&self, ty: &FajarType) -> Result<String, InteropError> {
        let s = match ty {
            FajarType::I8 => "s8",
            FajarType::I16 => "s16",
            FajarType::I32 => "s32",
            FajarType::I64 => "s64",
            FajarType::U8 => "u8",
            FajarType::U16 => "u16",
            FajarType::U32 => "u32",
            FajarType::U64 => "u64",
            FajarType::F32 => "f32",
            FajarType::F64 => "f64",
            FajarType::Bool => "bool",
            FajarType::Str => "string",
            FajarType::Void => {
                return Err(InteropError::UnsupportedTypeMapping {
                    fajar_type: "void".to_string(),
                    target: "WebAssembly".to_string(),
                });
            }
            FajarType::Array(inner) => {
                let inner_s = self.map_to_wasm(inner)?;
                return Ok(format!("list<{inner_s}>"));
            }
            FajarType::Tensor => {
                return Err(InteropError::UnsupportedTypeMapping {
                    fajar_type: "Tensor".to_string(),
                    target: "WebAssembly".to_string(),
                });
            }
            FajarType::Struct(name) | FajarType::Enum(name) => return Ok(name.clone()),
            FajarType::VoidPtr | FajarType::FnPtr => {
                return Err(InteropError::UnsupportedTypeMapping {
                    fajar_type: format!("{ty:?}"),
                    target: "WebAssembly".to_string(),
                });
            }
        };
        Ok(s.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 21: CBindgen (10 tests) ─────────────────────────────

    #[test]
    fn s21_1_c_header_include_guard() {
        let gen = CHeaderGenerator::new("sensor");
        let header = gen.generate();
        assert!(header.contains("#ifndef FJ_SENSOR_H"));
        assert!(header.contains("#define FJ_SENSOR_H"));
        assert!(header.contains("#endif /* FJ_SENSOR_H */"));
    }

    #[test]
    fn s21_2_c_header_extern_c() {
        let gen = CHeaderGenerator::new("test");
        let header = gen.generate();
        assert!(header.contains("#ifdef __cplusplus"));
        assert!(header.contains("extern \"C\" {"));
    }

    #[test]
    fn s21_3_c_header_stdint_include() {
        let gen = CHeaderGenerator::new("test");
        let header = gen.generate();
        assert!(header.contains("#include <stdint.h>"));
        assert!(header.contains("#include <stdbool.h>"));
    }

    #[test]
    fn s21_4_c_struct_generation() {
        let mut gen = CHeaderGenerator::new("data");
        gen.add_struct(CStructDef {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), CType::Double),
                ("y".to_string(), CType::Double),
            ],
            packed: false,
            doc_comment: Some("A 2D point.".to_string()),
        });
        let header = gen.generate();
        assert!(header.contains("struct Point {"));
        assert!(header.contains("double x;"));
        assert!(header.contains("double y;"));
        assert!(header.contains("/**"));
        assert!(header.contains("A 2D point."));
    }

    #[test]
    fn s21_5_c_packed_struct() {
        let mut gen = CHeaderGenerator::new("hw");
        gen.add_struct(CStructDef {
            name: "Register".to_string(),
            fields: vec![
                ("status".to_string(), CType::UInt8),
                ("value".to_string(), CType::UInt32),
            ],
            packed: true,
            doc_comment: None,
        });
        let header = gen.generate();
        assert!(header.contains("__attribute__((packed))"));
    }

    #[test]
    fn s21_6_c_enum_generation() {
        let mut gen = CHeaderGenerator::new("color");
        gen.add_enum(CEnumDef {
            name: "Color".to_string(),
            variants: vec![
                ("COLOR_RED".to_string(), Some(0)),
                ("COLOR_GREEN".to_string(), Some(1)),
                ("COLOR_BLUE".to_string(), Some(2)),
            ],
            doc_comment: None,
        });
        let header = gen.generate();
        assert!(header.contains("typedef enum Color {"));
        assert!(header.contains("COLOR_RED = 0"));
        assert!(header.contains("COLOR_GREEN = 1"));
        assert!(header.contains("COLOR_BLUE = 2"));
        assert!(header.contains("} Color;"));
    }

    #[test]
    fn s21_7_c_function_declaration() {
        let mut gen = CHeaderGenerator::new("math");
        gen.add_function(CFunctionDecl {
            name: "fj_add".to_string(),
            return_type: CType::Int32,
            params: vec![
                ("a".to_string(), CType::Int32),
                ("b".to_string(), CType::Int32),
            ],
            doc_comment: Some("Adds two integers.".to_string()),
            is_variadic: false,
        });
        let header = gen.generate();
        assert!(header.contains("int32_t fj_add(int32_t a, int32_t b);"));
        assert!(header.contains("Adds two integers."));
    }

    #[test]
    fn s21_8_c_variadic_function() {
        let mut gen = CHeaderGenerator::new("log");
        gen.add_function(CFunctionDecl {
            name: "fj_printf".to_string(),
            return_type: CType::Void,
            params: vec![("fmt".to_string(), CType::CharPtr)],
            doc_comment: None,
            is_variadic: true,
        });
        let header = gen.generate();
        assert!(header.contains("void fj_printf(const char* fmt, ...);"));
    }

    #[test]
    fn s21_9_c_function_ptr_type() {
        let sig = CFnSig {
            return_type: CType::Void,
            params: vec![("data".to_string(), CType::VoidPtr)],
        };
        let result = ctype_to_string(&CType::FnPtr(Box::new(sig)), Some("callback"));
        assert!(result.contains("void (*callback)(void* data)"));
    }

    #[test]
    fn s21_10_c_void_param_list() {
        let params: Vec<(String, CType)> = vec![];
        let result = build_c_param_list(&params, false);
        assert_eq!(result, "void");
    }

    // ── Sprint 22: PyBindgen (10 tests) ─────────────────────────────

    #[test]
    fn s22_1_py_module_header() {
        let gen = PyBindgenGenerator::new("fj_math");
        let init = gen.generate_init_py();
        assert!(init.contains("Auto-generated Python bindings"));
        assert!(init.contains("fj_math"));
        assert!(init.contains("from typing import Optional, List, Dict, Tuple"));
    }

    #[test]
    fn s22_2_py_class_generation() {
        let mut gen = PyBindgenGenerator::new("shapes");
        gen.add_class(PyClassDef {
            name: "Circle".to_string(),
            fields: vec![("radius".to_string(), PyType::Float)],
            methods: vec![PyFunctionDef {
                name: "area".to_string(),
                params: vec![],
                return_type: PyType::Float,
                doc_comment: Some("Returns the area.".to_string()),
            }],
            doc_comment: Some("A circle shape.".to_string()),
            bases: vec![],
        });
        let init = gen.generate_init_py();
        assert!(init.contains("class Circle:"));
        assert!(init.contains("\"\"\"A circle shape.\"\"\""));
        assert!(init.contains("def __init__(self, radius: float):"));
        assert!(init.contains("self.radius = radius"));
        assert!(init.contains("def area() -> float:"));
    }

    #[test]
    fn s22_3_py_enum_generation() {
        let mut gen = PyBindgenGenerator::new("colors");
        gen.add_enum(PyEnumDef {
            name: "Color".to_string(),
            variants: vec![
                ("Red".to_string(), None),
                ("Green".to_string(), None),
                ("Blue".to_string(), None),
            ],
        });
        let init = gen.generate_init_py();
        assert!(init.contains("class Color:"));
        assert!(init.contains("Red = 0"));
        assert!(init.contains("Green = 1"));
        assert!(init.contains("Blue = 2"));
    }

    #[test]
    fn s22_4_py_function_generation() {
        let mut gen = PyBindgenGenerator::new("math");
        gen.add_function(PyFunctionDef {
            name: "add".to_string(),
            params: vec![
                ("a".to_string(), PyType::Int),
                ("b".to_string(), PyType::Int),
            ],
            return_type: PyType::Int,
            doc_comment: Some("Add two integers.".to_string()),
        });
        let init = gen.generate_init_py();
        assert!(init.contains("def add(a: int, b: int) -> int:"));
        assert!(init.contains("\"\"\"Add two integers.\"\"\""));
    }

    #[test]
    fn s22_5_py_ndarray_import() {
        let mut gen = PyBindgenGenerator::new("ml");
        gen.add_function(PyFunctionDef {
            name: "predict".to_string(),
            params: vec![("x".to_string(), PyType::NdArray)],
            return_type: PyType::NdArray,
            doc_comment: None,
        });
        let init = gen.generate_init_py();
        assert!(init.contains("import numpy as np"));
    }

    #[test]
    fn s22_6_py_complex_types() {
        let ty = PyType::Dict(
            Box::new(PyType::Str),
            Box::new(PyType::List(Box::new(PyType::Int))),
        );
        let result = pytype_to_string(&ty);
        assert_eq!(result, "Dict[str, List[int]]");
    }

    #[test]
    fn s22_7_py_optional_type() {
        let ty = PyType::Optional(Box::new(PyType::Str));
        let result = pytype_to_string(&ty);
        assert_eq!(result, "Optional[str]");
    }

    #[test]
    fn s22_8_py_pyi_stubs() {
        let mut gen = PyBindgenGenerator::new("test");
        gen.add_function(PyFunctionDef {
            name: "greet".to_string(),
            params: vec![("name".to_string(), PyType::Str)],
            return_type: PyType::Str,
            doc_comment: None,
        });
        let stubs = gen.generate_pyi_stubs();
        assert!(stubs.contains("def greet(name: str) -> str: ..."));
    }

    #[test]
    fn s22_9_py_class_inheritance() {
        let mut gen = PyBindgenGenerator::new("shapes");
        gen.add_class(PyClassDef {
            name: "Square".to_string(),
            fields: vec![("side".to_string(), PyType::Float)],
            methods: vec![],
            doc_comment: None,
            bases: vec!["Shape".to_string()],
        });
        let init = gen.generate_init_py();
        assert!(init.contains("class Square(Shape):"));
    }

    #[test]
    fn s22_10_py_tuple_type() {
        let ty = PyType::Tuple(vec![PyType::Int, PyType::Float, PyType::Str]);
        let result = pytype_to_string(&ty);
        assert_eq!(result, "Tuple[int, float, str]");
    }

    // ── Sprint 23: WasmComponent + PackageAuditor (10 tests) ────────

    #[test]
    fn s23_1_wit_interface_generation() {
        let mut gen = WitGenerator::new("math-ops");
        gen.add_function(WitFunction {
            name: "add".to_string(),
            params: vec![
                ("a".to_string(), WitType::S32),
                ("b".to_string(), WitType::S32),
            ],
            results: vec![WitType::S32],
        });
        let wit = gen.generate_wit();
        assert!(wit.contains("interface math-ops {"));
        assert!(wit.contains("add: func(a: s32, b: s32) -> s32;"));
        assert!(wit.contains("}"));
    }

    #[test]
    fn s23_2_wit_record_type() {
        let mut gen = WitGenerator::new("geometry");
        gen.add_type(WitTypeDef {
            name: "point".to_string(),
            kind: WitType::Record(vec![
                ("x".to_string(), WitType::F64),
                ("y".to_string(), WitType::F64),
            ]),
        });
        let wit = gen.generate_wit();
        assert!(wit.contains("record point {"));
        assert!(wit.contains("x: f64,"));
        assert!(wit.contains("y: f64,"));
    }

    #[test]
    fn s23_3_wit_variant_type() {
        let mut gen = WitGenerator::new("shapes");
        gen.add_type(WitTypeDef {
            name: "shape".to_string(),
            kind: WitType::Variant(vec![
                ("circle".to_string(), Some(WitType::F64)),
                ("square".to_string(), Some(WitType::F64)),
                ("none".to_string(), None),
            ]),
        });
        let wit = gen.generate_wit();
        assert!(wit.contains("variant shape {"));
        assert!(wit.contains("circle(f64),"));
        assert!(wit.contains("none,"));
    }

    #[test]
    fn s23_4_wit_world_generation() {
        let gen = WitGenerator::new("api");
        let world = gen.generate_world("my-app");
        assert!(world.contains("world my-app {"));
        assert!(world.contains("export api;"));
    }

    #[test]
    fn s23_5_wit_type_to_string() {
        assert_eq!(wittype_to_string(&WitType::WitString), "string");
        assert_eq!(
            wittype_to_string(&WitType::List(Box::new(WitType::U8))),
            "list<u8>"
        );
        assert_eq!(
            wittype_to_string(&WitType::Option(Box::new(WitType::S32))),
            "option<s32>"
        );
        assert_eq!(
            wittype_to_string(&WitType::Result(
                Box::new(WitType::WitString),
                Box::new(WitType::S32)
            )),
            "result<string, s32>"
        );
    }

    #[test]
    fn s23_6_audit_clean_packages() {
        let auditor = PackageAuditor::new();
        let deps = vec![Dependency {
            name: "fj-math".to_string(),
            version: "1.0.0".to_string(),
            license: Some("MIT".to_string()),
        }];
        let result = auditor.audit(&deps);
        assert!(result.is_clean());
        assert_eq!(result.total_packages, 1);
        assert_eq!(result.clean_packages, 1);
    }

    #[test]
    fn s23_7_audit_finds_vulnerability() {
        let mut auditor = PackageAuditor::new();
        auditor.add_advisory(Advisory {
            id: "FJ-2026-001".to_string(),
            package: "fj-crypto".to_string(),
            affected_versions: "< 1.2.0".to_string(),
            severity: Severity::High,
            description: "Buffer overflow in key derivation".to_string(),
            fixed_in: Some("1.2.0".to_string()),
            url: None,
        });
        let deps = vec![Dependency {
            name: "fj-crypto".to_string(),
            version: "1.1.0".to_string(),
            license: Some("MIT".to_string()),
        }];
        let result = auditor.audit(&deps);
        assert!(!result.is_clean());
        assert_eq!(result.vulnerabilities.len(), 1);
        assert_eq!(result.max_severity(), Some(Severity::High));
        assert!(result.vulnerabilities[0].fix_available);
    }

    #[test]
    fn s23_8_audit_license_incompatible() {
        let auditor = PackageAuditor::new();
        let deps = vec![Dependency {
            name: "proprietary-lib".to_string(),
            version: "1.0.0".to_string(),
            license: Some("Proprietary".to_string()),
        }];
        let reports = auditor.check_licenses(&deps);
        assert_eq!(reports.len(), 1);
        assert!(!reports[0].compatible);
        assert_eq!(reports[0].license, LicenseType::Proprietary);
    }

    #[test]
    fn s23_9_sbom_generation() {
        let sbom_gen = SbomGenerator::new("my-project", "0.5.0");
        let deps = vec![Dependency {
            name: "fj-math".to_string(),
            version: "1.0.0".to_string(),
            license: Some("MIT".to_string()),
        }];
        let sbom = sbom_gen.generate(&deps).unwrap();
        assert!(sbom.contains("CycloneDX"));
        assert!(sbom.contains("my-project"));
        assert!(sbom.contains("0.5.0"));
        assert!(sbom.contains("fj-math"));
        assert!(sbom.contains("MIT"));
    }

    #[test]
    fn s23_10_yank_manager() {
        let mut ym = YankManager::new();
        ym.yank("fj-crypto", "1.0.0", "critical vulnerability");
        assert!(ym.is_yanked("fj-crypto", "1.0.0"));
        assert!(!ym.is_yanked("fj-crypto", "1.1.0"));
        assert_eq!(
            ym.yank_reason("fj-crypto", "1.0.0"),
            Some("critical vulnerability")
        );
        let versions = ym.yanked_versions("fj-crypto");
        assert_eq!(versions.len(), 1);
        assert!(ym.unyank("fj-crypto", "1.0.0"));
        assert!(!ym.is_yanked("fj-crypto", "1.0.0"));
    }

    // ── Sprint 24: InteropTypeMapper (10 tests) ─────────────────────

    #[test]
    fn s24_1_map_i32_to_all_targets() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper.map_type(&FajarType::I32, TargetLanguage::C).unwrap(),
            "int32_t"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::I32, TargetLanguage::Cpp)
                .unwrap(),
            "std::int32_t"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::I32, TargetLanguage::Python)
                .unwrap(),
            "int"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::I32, TargetLanguage::JavaScript)
                .unwrap(),
            "number"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::I32, TargetLanguage::Wasm)
                .unwrap(),
            "s32"
        );
    }

    #[test]
    fn s24_2_map_f64_to_all_targets() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper.map_type(&FajarType::F64, TargetLanguage::C).unwrap(),
            "double"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::F64, TargetLanguage::Cpp)
                .unwrap(),
            "double"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::F64, TargetLanguage::Python)
                .unwrap(),
            "float"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::F64, TargetLanguage::JavaScript)
                .unwrap(),
            "number"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::F64, TargetLanguage::Wasm)
                .unwrap(),
            "f64"
        );
    }

    #[test]
    fn s24_3_map_str_to_all_targets() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper.map_type(&FajarType::Str, TargetLanguage::C).unwrap(),
            "const char*"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Str, TargetLanguage::Cpp)
                .unwrap(),
            "std::string"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Str, TargetLanguage::Python)
                .unwrap(),
            "str"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Str, TargetLanguage::JavaScript)
                .unwrap(),
            "string"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Str, TargetLanguage::Wasm)
                .unwrap(),
            "string"
        );
    }

    #[test]
    fn s24_4_map_bool_to_all_targets() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper
                .map_type(&FajarType::Bool, TargetLanguage::C)
                .unwrap(),
            "_Bool"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Bool, TargetLanguage::Cpp)
                .unwrap(),
            "bool"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Bool, TargetLanguage::Python)
                .unwrap(),
            "bool"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Bool, TargetLanguage::JavaScript)
                .unwrap(),
            "boolean"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Bool, TargetLanguage::Wasm)
                .unwrap(),
            "bool"
        );
    }

    #[test]
    fn s24_5_map_array_to_targets() {
        let mapper = TypeMapper::new();
        let arr = FajarType::Array(Box::new(FajarType::I32));
        assert_eq!(
            mapper.map_type(&arr, TargetLanguage::C).unwrap(),
            "int32_t*"
        );
        assert_eq!(
            mapper.map_type(&arr, TargetLanguage::Cpp).unwrap(),
            "std::vector<std::int32_t>"
        );
        assert_eq!(
            mapper.map_type(&arr, TargetLanguage::Python).unwrap(),
            "list[int]"
        );
        assert_eq!(
            mapper.map_type(&arr, TargetLanguage::JavaScript).unwrap(),
            "Array<number>"
        );
        assert_eq!(
            mapper.map_type(&arr, TargetLanguage::Wasm).unwrap(),
            "list<s32>"
        );
    }

    #[test]
    fn s24_6_map_tensor_to_targets() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper
                .map_type(&FajarType::Tensor, TargetLanguage::C)
                .unwrap(),
            "fj_tensor_t*"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Tensor, TargetLanguage::Cpp)
                .unwrap(),
            "fj::Tensor"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Tensor, TargetLanguage::Python)
                .unwrap(),
            "numpy.ndarray"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::Tensor, TargetLanguage::JavaScript)
                .unwrap(),
            "Float64Array"
        );
        // Tensor unsupported in WASM
        assert!(mapper
            .map_type(&FajarType::Tensor, TargetLanguage::Wasm)
            .is_err());
    }

    #[test]
    fn s24_7_map_void_wasm_unsupported() {
        let mapper = TypeMapper::new();
        let result = mapper.map_type(&FajarType::Void, TargetLanguage::Wasm);
        assert!(result.is_err());
        if let Err(InteropError::UnsupportedTypeMapping { fajar_type, target }) = result {
            assert_eq!(fajar_type, "void");
            assert_eq!(target, "WebAssembly");
        } else {
            panic!("Expected UnsupportedTypeMapping error");
        }
    }

    #[test]
    fn s24_8_type_layout_sizes() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper.type_layout(&FajarType::I8),
            TypeLayout {
                size: 1,
                alignment: 1
            }
        );
        assert_eq!(
            mapper.type_layout(&FajarType::I16),
            TypeLayout {
                size: 2,
                alignment: 2
            }
        );
        assert_eq!(
            mapper.type_layout(&FajarType::I32),
            TypeLayout {
                size: 4,
                alignment: 4
            }
        );
        assert_eq!(
            mapper.type_layout(&FajarType::I64),
            TypeLayout {
                size: 8,
                alignment: 8
            }
        );
        assert_eq!(
            mapper.type_layout(&FajarType::Bool),
            TypeLayout {
                size: 1,
                alignment: 1
            }
        );
        assert_eq!(
            mapper.type_layout(&FajarType::Void),
            TypeLayout {
                size: 0,
                alignment: 1
            }
        );
    }

    #[test]
    fn s24_9_map_i64_javascript_bigint() {
        let mapper = TypeMapper::new();
        assert_eq!(
            mapper
                .map_type(&FajarType::I64, TargetLanguage::JavaScript)
                .unwrap(),
            "bigint"
        );
        assert_eq!(
            mapper
                .map_type(&FajarType::U64, TargetLanguage::JavaScript)
                .unwrap(),
            "bigint"
        );
    }

    #[test]
    fn s24_10_version_range_comparison() {
        assert!(version_in_range("1.0.0", "< 1.2.0"));
        assert!(!version_in_range("1.2.0", "< 1.2.0"));
        assert!(version_in_range("1.2.0", "<= 1.2.0"));
        assert!(version_in_range("2.0.0", ">= 1.0.0"));
        assert!(!version_in_range("0.9.0", ">= 1.0.0"));
        assert!(version_in_range("1.0.0", "*"));
        assert!(version_in_range("1.0.0", "1.0.0"));
        assert!(!version_in_range("1.0.1", "1.0.0"));
    }
}
