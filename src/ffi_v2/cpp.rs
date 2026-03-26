//! C++ Interop — header parsing, name mangling, class bridging, templates.
//!
//! Phase F1: 20 tasks covering libclang parsing, Itanium name mangling,
//! class method dispatch, RAII bridging, STL conversions, exception handling.

// ═══════════════════════════════════════════════════════════════════════
// F1.1: C++ Header Parsing (via libclang model)
// ═══════════════════════════════════════════════════════════════════════

/// A parsed C++ declaration.
#[derive(Debug, Clone)]
pub enum CppDecl {
    Function(CppFunction),
    Class(CppClass),
    Enum(CppEnum),
    Typedef(CppTypedef),
    Namespace(CppNamespace),
    Variable(CppVariable),
}

/// A C++ function declaration.
#[derive(Debug, Clone)]
pub struct CppFunction {
    /// Fully qualified name.
    pub name: String,
    /// Namespace path.
    pub namespace: Vec<String>,
    /// Return type.
    pub return_type: CppType,
    /// Parameters.
    pub params: Vec<CppParam>,
    /// Whether this is a static function.
    pub is_static: bool,
    /// Whether this is const.
    pub is_const: bool,
    /// Whether this is virtual.
    pub is_virtual: bool,
    /// Whether this is noexcept.
    pub is_noexcept: bool,
    /// Template parameters (empty if not template).
    pub template_params: Vec<String>,
}

/// A C++ parameter.
#[derive(Debug, Clone)]
pub struct CppParam {
    pub name: String,
    pub param_type: CppType,
    pub has_default: bool,
}

/// A C++ type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CppType {
    Void,
    Bool,
    Int(CppIntSize),
    Float,
    Double,
    Char,
    String,  // std::string
    WString, // std::wstring
    Pointer(Box<CppType>),
    Reference(Box<CppType>),
    ConstRef(Box<CppType>),
    RValueRef(Box<CppType>),
    SharedPtr(Box<CppType>),
    UniquePtr(Box<CppType>),
    Vector(Box<CppType>),
    Map(Box<CppType>, Box<CppType>),
    Optional(Box<CppType>),
    Custom(String),
    Template(String, Vec<CppType>),
}

/// C++ integer sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CppIntSize {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Size,
}

impl CppType {
    /// Maps a C++ type to a Fajar Lang type.
    pub fn to_fajar_type(&self) -> String {
        match self {
            Self::Void => "void".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Int(CppIntSize::I8) => "i8".to_string(),
            Self::Int(CppIntSize::I16) => "i16".to_string(),
            Self::Int(CppIntSize::I32) => "i32".to_string(),
            Self::Int(CppIntSize::I64) => "i64".to_string(),
            Self::Int(CppIntSize::U8) => "u8".to_string(),
            Self::Int(CppIntSize::U16) => "u16".to_string(),
            Self::Int(CppIntSize::U32) => "u32".to_string(),
            Self::Int(CppIntSize::U64) => "u64".to_string(),
            Self::Int(CppIntSize::Size) => "usize".to_string(),
            Self::Float => "f32".to_string(),
            Self::Double => "f64".to_string(),
            Self::Char => "char".to_string(),
            Self::String | Self::WString => "str".to_string(),
            Self::Pointer(inner) => format!("*mut {}", inner.to_fajar_type()),
            Self::Reference(inner) | Self::ConstRef(inner) => format!("&{}", inner.to_fajar_type()),
            Self::RValueRef(inner) => inner.to_fajar_type(), // move semantics
            Self::SharedPtr(inner) => format!("Rc<{}>", inner.to_fajar_type()),
            Self::UniquePtr(inner) => format!("Box<{}>", inner.to_fajar_type()),
            Self::Vector(inner) => format!("[{}]", inner.to_fajar_type()),
            Self::Map(k, v) => format!("Map<{}, {}>", k.to_fajar_type(), v.to_fajar_type()),
            Self::Optional(inner) => format!("Option<{}>", inner.to_fajar_type()),
            Self::Custom(name) => name.clone(),
            Self::Template(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_fajar_type()).collect();
                format!("{}<{}>", name, args_str.join(", "))
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F1.2: Itanium Name Mangling
// ═══════════════════════════════════════════════════════════════════════

/// Mangles a C++ function name (Itanium ABI).
pub fn mangle_name(namespace: &[String], name: &str, params: &[CppType]) -> String {
    let mut mangled = String::from("_Z");

    // Nested name
    if !namespace.is_empty() {
        mangled.push('N');
        for ns in namespace {
            mangled.push_str(&format!("{}{}", ns.len(), ns));
        }
        mangled.push_str(&format!("{}{}", name.len(), name));
        mangled.push('E');
    } else {
        mangled.push_str(&format!("{}{}", name.len(), name));
    }

    // Parameters
    for param in params {
        mangled.push_str(&mangle_type(param));
    }
    if params.is_empty() {
        mangled.push('v');
    } // void

    mangled
}

fn mangle_type(t: &CppType) -> String {
    match t {
        CppType::Void => "v".to_string(),
        CppType::Bool => "b".to_string(),
        CppType::Int(CppIntSize::I32) => "i".to_string(),
        CppType::Int(CppIntSize::I64) => "l".to_string(),
        CppType::Int(CppIntSize::U32) => "j".to_string(),
        CppType::Float => "f".to_string(),
        CppType::Double => "d".to_string(),
        CppType::Char => "c".to_string(),
        CppType::Pointer(inner) => format!("P{}", mangle_type(inner)),
        CppType::Reference(inner) => format!("R{}", mangle_type(inner)),
        CppType::ConstRef(inner) => format!("RK{}", mangle_type(inner)),
        CppType::Custom(name) => format!("{}{}", name.len(), name),
        _ => "v".to_string(), // simplified
    }
}

/// Demangles an Itanium-mangled name (simplified).
pub fn demangle_name(mangled: &str) -> String {
    if !mangled.starts_with("_Z") {
        return mangled.to_string();
    }
    // Simplified: extract name lengths
    let rest = &mangled[2..];
    if rest.starts_with('N') {
        // Nested name
        let mut parts = Vec::new();
        let mut pos = 1;
        let chars: Vec<char> = rest.chars().collect();
        while pos < chars.len() && chars[pos] != 'E' {
            if chars[pos].is_ascii_digit() {
                let mut num = String::new();
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    num.push(chars[pos]);
                    pos += 1;
                }
                let len: usize = num.parse().unwrap_or(0);
                let name: String = chars[pos..pos + len.min(chars.len() - pos)]
                    .iter()
                    .collect();
                parts.push(name);
                pos += len;
            } else {
                break;
            }
        }
        parts.join("::")
    } else {
        mangled.to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F1.3: C++ Class Bridging
// ═══════════════════════════════════════════════════════════════════════

/// A C++ class declaration.
#[derive(Debug, Clone)]
pub struct CppClass {
    /// Class name.
    pub name: String,
    /// Namespace.
    pub namespace: Vec<String>,
    /// Base classes.
    pub bases: Vec<String>,
    /// Fields.
    pub fields: Vec<CppField>,
    /// Methods.
    pub methods: Vec<CppFunction>,
    /// Constructors.
    pub constructors: Vec<CppFunction>,
    /// Destructor present.
    pub has_destructor: bool,
    /// Whether the class is abstract (has pure virtual methods).
    pub is_abstract: bool,
    /// Template parameters.
    pub template_params: Vec<String>,
    /// Size in bytes (for allocation).
    pub size_bytes: usize,
    /// Alignment.
    pub align_bytes: usize,
}

/// A C++ class field.
#[derive(Debug, Clone)]
pub struct CppField {
    pub name: String,
    pub field_type: CppType,
    pub is_public: bool,
    pub is_static: bool,
    pub offset_bytes: usize,
}

/// Generates a Fajar Lang struct + impl from a C++ class.
pub fn generate_class_binding(class: &CppClass) -> String {
    let mut code = String::new();

    // Struct
    code.push_str(&format!("struct {} {{\n", class.name));
    code.push_str("    _opaque: *mut u8  // opaque C++ pointer\n");
    code.push_str("}\n\n");

    // Impl
    code.push_str(&format!("impl {} {{\n", class.name));

    // Constructor
    for ctor in &class.constructors {
        let params: Vec<String> = ctor
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.param_type.to_fajar_type()))
            .collect();
        code.push_str(&format!(
            "    @ffi fn new({}) -> {} {{\n",
            params.join(", "),
            class.name
        ));
        code.push_str("        // calls C++ constructor via FFI\n");
        code.push_str("    }\n\n");
    }

    // Methods
    for method in &class.methods {
        let self_param = if method.is_static {
            ""
        } else if method.is_const {
            "self"
        } else {
            "mut self"
        };
        let other_params: Vec<String> = method
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.param_type.to_fajar_type()))
            .collect();
        let all_params = if self_param.is_empty() {
            other_params.join(", ")
        } else {
            let mut v = vec![self_param.to_string()];
            v.extend(other_params);
            v.join(", ")
        };
        let ret = if method.return_type == CppType::Void {
            String::new()
        } else {
            format!(" -> {}", method.return_type.to_fajar_type())
        };
        code.push_str(&format!(
            "    @ffi fn {}({}){} {{}}\n",
            method.name, all_params, ret
        ));
    }

    // Destructor
    if class.has_destructor {
        code.push_str("\n    @ffi fn drop(mut self) {}\n");
    }

    code.push_str("}\n");
    code
}

// ═══════════════════════════════════════════════════════════════════════
// F1.4-F1.5: Enums, Typedefs, Namespaces
// ═══════════════════════════════════════════════════════════════════════

/// C++ enum.
#[derive(Debug, Clone)]
pub struct CppEnum {
    pub name: String,
    pub namespace: Vec<String>,
    pub variants: Vec<(String, i64)>,
    pub is_scoped: bool, // enum class
}

/// C++ typedef.
#[derive(Debug, Clone)]
pub struct CppTypedef {
    pub name: String,
    pub target: CppType,
}

/// C++ namespace.
#[derive(Debug, Clone)]
pub struct CppNamespace {
    pub name: String,
    pub declarations: Vec<CppDecl>,
}

/// C++ global variable.
#[derive(Debug, Clone)]
pub struct CppVariable {
    pub name: String,
    pub var_type: CppType,
    pub is_const: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// F1.6-F1.8: Exception Handling, CMake, ABI Check
// ═══════════════════════════════════════════════════════════════════════

/// C++ exception to Fajar Result mapping.
#[derive(Debug, Clone)]
pub struct ExceptionMapping {
    /// C++ exception type.
    pub cpp_type: String,
    /// Fajar error type.
    pub fajar_type: String,
    /// Conversion expression.
    pub conversion: String,
}

/// Generates a try/catch wrapper for a C++ function call.
pub fn generate_exception_wrapper(func: &CppFunction) -> String {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, p.param_type.to_fajar_type()))
        .collect();
    let ret = func.return_type.to_fajar_type();

    format!(
        r#"fn {name}_safe({params}) -> Result<{ret}, str> {{
    // C++ side: try {{ return Ok(func(...)); }} catch (std::exception& e) {{ return Err(e.what()); }}
    extern "C" fn _ffi_{name}({params}) -> i64;
    // ...
}}"#,
        name = func.name,
        params = params.join(", "),
        ret = ret,
    )
}

/// CMake integration: find_package configuration.
#[derive(Debug, Clone)]
pub struct CmakePackage {
    /// Package name.
    pub name: String,
    /// Required components.
    pub components: Vec<String>,
    /// Minimum version.
    pub min_version: Option<String>,
    /// Include directories found.
    pub include_dirs: Vec<String>,
    /// Libraries to link.
    pub libraries: Vec<String>,
}

/// ABI compatibility check result.
#[derive(Debug, Clone)]
pub struct AbiCheck {
    /// Compiler.
    pub compiler: String,
    /// C++ standard.
    pub std_version: String,
    /// ABI version.
    pub abi_version: String,
    /// Compatible with Fajar FFI.
    pub compatible: bool,
    /// Warnings.
    pub warnings: Vec<String>,
}

impl AbiCheck {
    /// Checks if a compiler/ABI combination is supported.
    pub fn check(compiler: &str, std_version: &str) -> Self {
        let compatible = matches!(compiler, "gcc" | "clang" | "g++" | "clang++");
        let warnings = if std_version.starts_with("c++2") {
            vec![]
        } else {
            vec!["C++17 or newer recommended".to_string()]
        };
        Self {
            compiler: compiler.to_string(),
            std_version: std_version.to_string(),
            abi_version: "itanium".to_string(),
            compatible,
            warnings,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC2.1-GC2.9: Real C++ Header Parsing via libclang
// ═══════════════════════════════════════════════════════════════════════

/// Parse a C++ header file and extract declarations using libclang.
///
/// Requires the `cpp-ffi` feature and libclang installed on the system.
#[cfg(feature = "cpp-ffi")]
pub fn parse_header(header_path: &str, include_dirs: &[&str]) -> Result<Vec<CppDecl>, String> {
    use clang_sys::*;
    use std::ffi::CString;
    use std::ptr;

    // Load libclang dynamically
    if let Err(e) = clang_sys::load() {
        // Already loaded is OK
        let msg = format!("{e}");
        if !msg.contains("already") {
            return Err(format!("failed to load libclang: {e}"));
        }
    }

    let index = unsafe { clang_createIndex(0, 0) };
    if index.is_null() {
        return Err("clang_createIndex failed".to_string());
    }

    // Build arguments
    let mut args: Vec<CString> = vec![CString::new("-x").unwrap(), CString::new("c++").unwrap()];
    for dir in include_dirs {
        args.push(CString::new(format!("-I{dir}")).unwrap());
    }
    let c_args: Vec<*const i8> = args.iter().map(|a| a.as_ptr()).collect();

    let c_path = CString::new(header_path).map_err(|e| format!("invalid path: {e}"))?;

    let tu = unsafe {
        clang_parseTranslationUnit(
            index,
            c_path.as_ptr(),
            c_args.as_ptr(),
            c_args.len() as i32,
            ptr::null_mut(),
            0,
            CXTranslationUnit_SkipFunctionBodies,
        )
    };

    if tu.is_null() {
        unsafe { clang_disposeIndex(index) };
        return Err(format!("failed to parse '{header_path}'"));
    }

    let mut decls = Vec::new();
    let cursor = unsafe { clang_getTranslationUnitCursor(tu) };

    // Visit top-level declarations
    unsafe {
        clang_visitChildren(
            cursor,
            visit_decl,
            &mut decls as *mut Vec<CppDecl> as *mut std::ffi::c_void,
        );
    }

    unsafe {
        clang_disposeTranslationUnit(tu);
        clang_disposeIndex(index);
    }

    Ok(decls)
}

#[cfg(feature = "cpp-ffi")]
extern "C" fn visit_decl(
    cursor: clang_sys::CXCursor,
    _parent: clang_sys::CXCursor,
    client_data: *mut std::ffi::c_void,
) -> clang_sys::CXChildVisitResult {
    use clang_sys::*;

    let decls = unsafe { &mut *(client_data as *mut Vec<CppDecl>) };

    let kind = unsafe { clang_getCursorKind(cursor) };
    let location = unsafe { clang_getCursorLocation(cursor) };

    // Skip system headers
    if unsafe { clang_Location_isInSystemHeader(location) } != 0 {
        return CXChildVisit_Continue;
    }

    let name = cursor_name(cursor);

    match kind {
        CXCursor_FunctionDecl => {
            let func = extract_function(cursor, &name);
            decls.push(CppDecl::Function(func));
        }
        CXCursor_ClassDecl | CXCursor_StructDecl | CXCursor_ClassTemplate => {
            let class = extract_class(cursor, &name);
            decls.push(CppDecl::Class(class));
        }
        CXCursor_EnumDecl => {
            let en = extract_enum(cursor, &name);
            decls.push(CppDecl::Enum(en));
        }
        CXCursor_TypedefDecl | CXCursor_TypeAliasDecl => {
            decls.push(CppDecl::Typedef(CppTypedef {
                name: name.clone(),
                target: CppType::Custom(name),
            }));
        }
        CXCursor_Namespace => {
            let mut ns_decls = Vec::new();
            unsafe {
                clang_visitChildren(
                    cursor,
                    visit_decl,
                    &mut ns_decls as *mut Vec<CppDecl> as *mut std::ffi::c_void,
                );
            }
            decls.push(CppDecl::Namespace(CppNamespace {
                name,
                declarations: ns_decls,
            }));
        }
        _ => {}
    }

    CXChildVisit_Continue
}

#[cfg(feature = "cpp-ffi")]
fn cursor_name(cursor: clang_sys::CXCursor) -> String {
    unsafe {
        let cx_str = clang_sys::clang_getCursorSpelling(cursor);
        let c_str = clang_sys::clang_getCString(cx_str);
        let name = if c_str.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(c_str)
                .to_string_lossy()
                .into_owned()
        };
        clang_sys::clang_disposeString(cx_str);
        name
    }
}

#[cfg(feature = "cpp-ffi")]
fn extract_function(cursor: clang_sys::CXCursor, name: &str) -> CppFunction {
    use clang_sys::*;

    let num_args = unsafe { clang_Cursor_getNumArguments(cursor) };
    let mut params = Vec::new();
    for i in 0..num_args {
        let arg = unsafe { clang_Cursor_getArgument(cursor, i as u32) };
        let arg_name = cursor_name(arg);
        let arg_type = unsafe { clang_getCursorType(arg) };
        params.push(CppParam {
            name: if arg_name.is_empty() {
                format!("arg{i}")
            } else {
                arg_name
            },
            param_type: clang_type_to_cpp(arg_type),
            has_default: false,
        });
    }

    let ret_type = unsafe { clang_getCursorResultType(cursor) };

    CppFunction {
        name: name.to_string(),
        namespace: vec![],
        return_type: clang_type_to_cpp(ret_type),
        params,
        is_static: unsafe { clang_CXXMethod_isStatic(cursor) } != 0,
        is_const: unsafe { clang_CXXMethod_isConst(cursor) } != 0,
        is_virtual: unsafe { clang_CXXMethod_isVirtual(cursor) } != 0,
        is_noexcept: false,
        template_params: vec![],
    }
}

#[cfg(feature = "cpp-ffi")]
fn extract_class(cursor: clang_sys::CXCursor, name: &str) -> CppClass {
    let _methods: Vec<CppFunction> = Vec::new();
    let _constructors: Vec<CppFunction> = Vec::new();
    let _fields: Vec<CppField> = Vec::new();

    struct ClassVisitorData {
        methods: Vec<CppFunction>,
        constructors: Vec<CppFunction>,
        fields: Vec<CppField>,
        has_destructor: bool,
        bases: Vec<String>,
        template_params: Vec<String>,
    }

    let mut data = ClassVisitorData {
        methods: Vec::new(),
        constructors: Vec::new(),
        fields: Vec::new(),
        has_destructor: false,
        bases: Vec::new(),
        template_params: Vec::new(),
    };

    extern "C" fn visit_member(
        cursor: clang_sys::CXCursor,
        _parent: clang_sys::CXCursor,
        client_data: *mut std::ffi::c_void,
    ) -> clang_sys::CXChildVisitResult {
        use clang_sys::*;
        let data = unsafe { &mut *(client_data as *mut ClassVisitorData) };
        let kind = unsafe { clang_getCursorKind(cursor) };
        let member_name = cursor_name(cursor);

        match kind {
            CXCursor_CXXMethod => {
                data.methods.push(extract_function(cursor, &member_name));
            }
            CXCursor_Constructor => {
                data.constructors
                    .push(extract_function(cursor, &member_name));
            }
            CXCursor_Destructor => {
                data.has_destructor = true;
            }
            CXCursor_FieldDecl => {
                let field_type = unsafe { clang_getCursorType(cursor) };
                data.fields.push(CppField {
                    name: member_name,
                    field_type: clang_type_to_cpp(field_type),
                    is_public: true,
                    is_static: false,
                    offset_bytes: 0,
                });
            }
            // CQ3.2: Extract base classes
            CXCursor_CXXBaseSpecifier => {
                let base_type = unsafe { clang_getCursorType(cursor) };
                let base_name = clang_type_name(base_type);
                data.bases.push(base_name);
            }
            // CQ3.1: Extract template parameters
            CXCursor_TemplateTypeParameter => {
                data.template_params.push(member_name);
            }
            _ => {}
        }
        CXChildVisit_Continue
    }

    unsafe {
        clang_sys::clang_visitChildren(
            cursor,
            visit_member,
            &mut data as *mut ClassVisitorData as *mut std::ffi::c_void,
        );
    }

    CppClass {
        name: name.to_string(),
        namespace: vec![],
        bases: data.bases,
        fields: data.fields,
        methods: data.methods,
        constructors: data.constructors,
        has_destructor: data.has_destructor,
        is_abstract: false,
        template_params: data.template_params,
        size_bytes: 0,
        align_bytes: 0,
    }
}

#[cfg(feature = "cpp-ffi")]
fn extract_enum(cursor: clang_sys::CXCursor, name: &str) -> CppEnum {
    let mut variants = Vec::new();

    extern "C" fn visit_variant(
        cursor: clang_sys::CXCursor,
        _parent: clang_sys::CXCursor,
        client_data: *mut std::ffi::c_void,
    ) -> clang_sys::CXChildVisitResult {
        use clang_sys::*;
        let variants = unsafe { &mut *(client_data as *mut Vec<(String, i64)>) };
        let kind = unsafe { clang_getCursorKind(cursor) };
        if kind == CXCursor_EnumConstantDecl {
            let vname = cursor_name(cursor);
            let value = unsafe { clang_getEnumConstantDeclValue(cursor) };
            variants.push((vname, value));
        }
        CXChildVisit_Continue
    }

    unsafe {
        clang_sys::clang_visitChildren(
            cursor,
            visit_variant,
            &mut variants as *mut Vec<(String, i64)> as *mut std::ffi::c_void,
        );
    }

    CppEnum {
        name: name.to_string(),
        namespace: vec![],
        variants,
        is_scoped: false,
    }
}

#[cfg(feature = "cpp-ffi")]
fn clang_type_to_cpp(cx_type: clang_sys::CXType) -> CppType {
    use clang_sys::*;
    match cx_type.kind {
        CXType_Void => CppType::Void,
        CXType_Bool => CppType::Bool,
        CXType_Char_S | CXType_SChar => CppType::Char,
        CXType_UChar | CXType_Char_U => CppType::Char,
        CXType_Short => CppType::Int(CppIntSize::I16),
        CXType_UShort => CppType::Int(CppIntSize::I16),
        CXType_Int => CppType::Int(CppIntSize::I32),
        CXType_UInt => CppType::Int(CppIntSize::I32),
        CXType_Long | CXType_LongLong => CppType::Int(CppIntSize::I64),
        CXType_ULong | CXType_ULongLong => CppType::Int(CppIntSize::I64),
        CXType_Float => CppType::Float,
        CXType_Double => CppType::Double,
        CXType_Pointer => {
            let pointee = unsafe { clang_getPointeeType(cx_type) };
            CppType::Pointer(Box::new(clang_type_to_cpp(pointee)))
        }
        CXType_LValueReference => {
            let pointee = unsafe { clang_getPointeeType(cx_type) };
            CppType::Reference(Box::new(clang_type_to_cpp(pointee)))
        }
        _ => {
            let name = unsafe {
                let cx_str = clang_getTypeSpelling(cx_type);
                let c_str = clang_getCString(cx_str);
                let s = if c_str.is_null() {
                    String::from("unknown")
                } else {
                    std::ffi::CStr::from_ptr(c_str)
                        .to_string_lossy()
                        .into_owned()
                };
                clang_disposeString(cx_str);
                s
            };
            CppType::Custom(name)
        }
    }
}

/// Get the type name from a clang CXType.
#[cfg(feature = "cpp-ffi")]
fn clang_type_name(cx_type: clang_sys::CXType) -> String {
    unsafe {
        let cx_str = clang_sys::clang_getTypeSpelling(cx_type);
        let c_str = clang_sys::clang_getCString(cx_str);
        let name = if c_str.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(c_str)
                .to_string_lossy()
                .into_owned()
        };
        clang_sys::clang_disposeString(cx_str);
        name
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CQ3.7: Generate Fajar Lang binding code from parsed C++ declarations
// ═══════════════════════════════════════════════════════════════════════

/// Generate Fajar Lang extern block from a list of parsed C++ declarations.
pub fn generate_fajar_bindings(decls: &[CppDecl]) -> String {
    let mut code = String::new();
    code.push_str("// Auto-generated Fajar Lang bindings\n\n");

    for decl in decls {
        match decl {
            CppDecl::Function(f) => {
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.param_type.to_fajar_type()))
                    .collect();
                let ret = f.return_type.to_fajar_type();
                code.push_str(&format!(
                    "extern fn {}({}) -> {}\n",
                    f.name,
                    params.join(", "),
                    ret
                ));
            }
            CppDecl::Class(c) => {
                // Generate struct + impl
                code.push_str(&format!("struct {} {{\n", c.name));
                for field in &c.fields {
                    code.push_str(&format!(
                        "    {}: {},\n",
                        field.name,
                        field.field_type.to_fajar_type()
                    ));
                }
                code.push_str("}\n\n");
                if !c.methods.is_empty() {
                    code.push_str(&format!("impl {} {{\n", c.name));
                    for m in &c.methods {
                        let params: Vec<String> = m
                            .params
                            .iter()
                            .map(|p| format!("{}: {}", p.name, p.param_type.to_fajar_type()))
                            .collect();
                        code.push_str(&format!(
                            "    extern fn {}(self{}) -> {}\n",
                            m.name,
                            if params.is_empty() {
                                String::new()
                            } else {
                                format!(", {}", params.join(", "))
                            },
                            m.return_type.to_fajar_type()
                        ));
                    }
                    code.push_str("}\n\n");
                }
            }
            CppDecl::Enum(e) => {
                code.push_str(&format!("enum {} {{\n", e.name));
                for (name, value) in &e.variants {
                    code.push_str(&format!("    {} = {},\n", name, value));
                }
                code.push_str("}\n\n");
            }
            _ => {}
        }
    }

    code
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f1_1_cpp_type_to_fajar() {
        assert_eq!(CppType::Int(CppIntSize::I32).to_fajar_type(), "i32");
        assert_eq!(CppType::Double.to_fajar_type(), "f64");
        assert_eq!(CppType::String.to_fajar_type(), "str");
        assert_eq!(
            CppType::Vector(Box::new(CppType::Float)).to_fajar_type(),
            "[f32]"
        );
        assert_eq!(
            CppType::SharedPtr(Box::new(CppType::Custom("Widget".to_string()))).to_fajar_type(),
            "Rc<Widget>"
        );
        assert_eq!(
            CppType::UniquePtr(Box::new(CppType::Custom("Buffer".to_string()))).to_fajar_type(),
            "Box<Buffer>"
        );
        assert_eq!(
            CppType::Optional(Box::new(CppType::Int(CppIntSize::I64))).to_fajar_type(),
            "Option<i64>"
        );
    }

    #[test]
    fn f1_2_name_mangling() {
        // _Z3fooi = foo(int)
        let mangled = mangle_name(&[], "foo", &[CppType::Int(CppIntSize::I32)]);
        assert_eq!(mangled, "_Z3fooi");

        // _ZN3std6vectorIiEE = std::vector<int> (simplified)
        let mangled2 = mangle_name(&["std".to_string()], "sort", &[]);
        assert_eq!(mangled2, "_ZN3std4sortEv");
    }

    #[test]
    fn f1_2_name_demangling() {
        let demangled = demangle_name("_ZN5MyLib7MyClass6methodEi");
        assert!(demangled.contains("MyLib"));
        assert!(demangled.contains("MyClass"));
    }

    #[test]
    fn f1_3_class_binding() {
        let class = CppClass {
            name: "Mat".to_string(),
            namespace: vec!["cv".to_string()],
            bases: vec![],
            fields: vec![],
            methods: vec![CppFunction {
                name: "rows".to_string(),
                namespace: vec![],
                return_type: CppType::Int(CppIntSize::I32),
                params: vec![],
                is_static: false,
                is_const: true,
                is_virtual: false,
                is_noexcept: false,
                template_params: vec![],
            }],
            constructors: vec![CppFunction {
                name: "Mat".to_string(),
                namespace: vec![],
                return_type: CppType::Void,
                params: vec![
                    CppParam {
                        name: "rows".to_string(),
                        param_type: CppType::Int(CppIntSize::I32),
                        has_default: false,
                    },
                    CppParam {
                        name: "cols".to_string(),
                        param_type: CppType::Int(CppIntSize::I32),
                        has_default: false,
                    },
                ],
                is_static: false,
                is_const: false,
                is_virtual: false,
                is_noexcept: false,
                template_params: vec![],
            }],
            has_destructor: true,
            is_abstract: false,
            template_params: vec![],
            size_bytes: 96,
            align_bytes: 8,
        };
        let code = generate_class_binding(&class);
        assert!(code.contains("struct Mat"));
        assert!(code.contains("fn new("));
        assert!(code.contains("fn rows("));
        assert!(code.contains("fn drop("));
    }

    #[test]
    fn f1_6_abi_check() {
        let check = AbiCheck::check("clang++", "c++20");
        assert!(check.compatible);
        assert!(check.warnings.is_empty());

        let check2 = AbiCheck::check("msvc", "c++14");
        assert!(!check2.compatible);
        assert!(!check2.warnings.is_empty());
    }

    #[test]
    fn f1_1_template_type() {
        let t = CppType::Template("vector".to_string(), vec![CppType::Int(CppIntSize::I32)]);
        assert_eq!(t.to_fajar_type(), "vector<i32>");
    }

    #[test]
    fn f1_1_map_type() {
        let t = CppType::Map(
            Box::new(CppType::String),
            Box::new(CppType::Int(CppIntSize::I64)),
        );
        assert_eq!(t.to_fajar_type(), "Map<str, i64>");
    }

    #[test]
    fn f1_2_mangle_void() {
        let mangled = mangle_name(&[], "init", &[]);
        assert_eq!(mangled, "_Z4initv");
    }

    // ═══════════════════════════════════════════════════════════════════
    // V8 GC2: Real libclang integration tests
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn gc2_parse_simple_header() {
        use std::io::Write;
        // Create a temporary C++ header
        let dir = std::env::temp_dir().join("fj_cpp_test");
        let _ = std::fs::create_dir_all(&dir);
        let header = dir.join("test.h");
        let mut f = std::fs::File::create(&header).unwrap();
        writeln!(
            f,
            r#"
int add(int a, int b);
double multiply(double x, double y);

struct Point {{
    double x;
    double y;
}};

enum Color {{
    Red = 0,
    Green = 1,
    Blue = 2
}};

class Calculator {{
public:
    Calculator();
    int compute(int input);
    static double pi();
}};
"#
        )
        .unwrap();

        let decls = parse_header(header.to_str().unwrap(), &[]).unwrap();

        // Should find functions, struct, enum, class
        let func_count = decls
            .iter()
            .filter(|d| matches!(d, CppDecl::Function(_)))
            .count();
        let class_count = decls
            .iter()
            .filter(|d| matches!(d, CppDecl::Class(_)))
            .count();
        let enum_count = decls
            .iter()
            .filter(|d| matches!(d, CppDecl::Enum(_)))
            .count();

        assert!(func_count >= 2, "expected >=2 functions, got {func_count}");
        assert!(
            class_count >= 2,
            "expected >=2 classes/structs, got {class_count}"
        );
        assert!(enum_count >= 1, "expected >=1 enum, got {enum_count}");

        // Verify function details
        let add_fn = decls.iter().find_map(|d| match d {
            CppDecl::Function(f) if f.name == "add" => Some(f),
            _ => None,
        });
        assert!(add_fn.is_some(), "should find 'add' function");
        let add_fn = add_fn.unwrap();
        assert_eq!(add_fn.params.len(), 2);
        assert_eq!(add_fn.return_type, CppType::Int(CppIntSize::I32));

        // Verify enum details
        let color_enum = decls.iter().find_map(|d| match d {
            CppDecl::Enum(e) if e.name == "Color" => Some(e),
            _ => None,
        });
        assert!(color_enum.is_some(), "should find 'Color' enum");
        assert_eq!(color_enum.unwrap().variants.len(), 3);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn gc2_parse_cpp_with_namespace() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("fj_cpp_ns_test");
        let _ = std::fs::create_dir_all(&dir);
        let header = dir.join("ns_test.h");
        let mut f = std::fs::File::create(&header).unwrap();
        writeln!(
            f,
            r#"
namespace math {{
    int abs(int x);
    double sqrt(double x);
}}

namespace io {{
    void print(const char* msg);
}}
"#
        )
        .unwrap();

        let decls = parse_header(header.to_str().unwrap(), &[]).unwrap();
        let ns_count = decls
            .iter()
            .filter(|d| matches!(d, CppDecl::Namespace(_)))
            .count();
        assert!(ns_count >= 2, "expected 2 namespaces, got {ns_count}");

        // Verify nested declarations
        if let Some(CppDecl::Namespace(ns)) = decls
            .iter()
            .find(|d| matches!(d, CppDecl::Namespace(n) if n.name == "math"))
        {
            assert!(
                ns.declarations.len() >= 2,
                "math namespace should have 2+ functions"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ═══════════════════════════════════════════════════════════════════
    // CQ3: C++ FFI Quality Tests
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn cq3_1_template_detection() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("fj_cq3_tmpl");
        let _ = std::fs::create_dir_all(&dir);
        let header = dir.join("tmpl.h");
        let mut f = std::fs::File::create(&header).unwrap();
        writeln!(f, r#"
template<typename T>
class Container {{
public:
    T value;
    T get() const {{ return value; }}
}};
"#).unwrap();

        let decls = parse_header(header.to_str().unwrap(), &[]).unwrap();
        let class = decls.iter().find_map(|d| match d {
            CppDecl::Class(c) if c.name == "Container" => Some(c),
            _ => None,
        });
        assert!(class.is_some(), "should find Container class");
        let cls = class.unwrap();
        assert!(
            !cls.template_params.is_empty(),
            "should detect template parameter T"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn cq3_2_inheritance() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("fj_cq3_inherit");
        let _ = std::fs::create_dir_all(&dir);
        let header = dir.join("inherit.h");
        let mut f = std::fs::File::create(&header).unwrap();
        writeln!(f, r#"
class Base {{
public:
    int x;
}};

class Derived : public Base {{
public:
    int y;
}};
"#).unwrap();

        let decls = parse_header(header.to_str().unwrap(), &[]).unwrap();
        let derived = decls.iter().find_map(|d| match d {
            CppDecl::Class(c) if c.name == "Derived" => Some(c),
            _ => None,
        });
        assert!(derived.is_some(), "should find Derived class");
        let cls = derived.unwrap();
        assert!(
            !cls.bases.is_empty(),
            "Derived should have base class: {:?}",
            cls.bases
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn cq3_3_method_qualifiers() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("fj_cq3_qual");
        let _ = std::fs::create_dir_all(&dir);
        let header = dir.join("qual.h");
        let mut f = std::fs::File::create(&header).unwrap();
        writeln!(f, r#"
class Widget {{
public:
    int value() const;
    static Widget create();
    virtual void update();
}};
"#).unwrap();

        let decls = parse_header(header.to_str().unwrap(), &[]).unwrap();
        let widget = decls.iter().find_map(|d| match d {
            CppDecl::Class(c) if c.name == "Widget" => Some(c),
            _ => None,
        });
        assert!(widget.is_some());
        let cls = widget.unwrap();

        let value_fn = cls.methods.iter().find(|m| m.name == "value");
        assert!(value_fn.is_some(), "should find value method");
        assert!(value_fn.unwrap().is_const, "value should be const");

        let create_fn = cls.methods.iter().find(|m| m.name == "create");
        assert!(create_fn.is_some(), "should find create method");
        assert!(create_fn.unwrap().is_static, "create should be static");

        let update_fn = cls.methods.iter().find(|m| m.name == "update");
        assert!(update_fn.is_some(), "should find update method");
        assert!(update_fn.unwrap().is_virtual, "update should be virtual");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn cq3_6_error_handling() {
        let result = parse_header("/nonexistent/path/header.h", &[]);
        assert!(result.is_err(), "nonexistent file should return error");
    }

    #[cfg(feature = "cpp-ffi")]
    #[test]
    fn cq3_7_binding_generation() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("fj_cq3_bind");
        let _ = std::fs::create_dir_all(&dir);
        let header = dir.join("api.h");
        let mut f = std::fs::File::create(&header).unwrap();
        writeln!(f, r#"
int add(int a, int b);
double pi();

enum Color {{ Red = 0, Green = 1, Blue = 2 }};

class Point {{
public:
    double x;
    double y;
    double distance() const;
}};
"#).unwrap();

        let decls = parse_header(header.to_str().unwrap(), &[]).unwrap();
        let code = generate_fajar_bindings(&decls);

        assert!(code.contains("extern fn add("), "should generate add binding");
        assert!(code.contains("extern fn pi("), "should generate pi binding");
        assert!(code.contains("enum Color"), "should generate Color enum");
        assert!(code.contains("struct Point"), "should generate Point struct");
        assert!(code.contains("x: f64"), "Point should have x field");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cq3_7_binding_from_manual_decls() {
        let decls = vec![
            CppDecl::Function(CppFunction {
                name: "sqrt".to_string(),
                namespace: vec![],
                return_type: CppType::Double,
                params: vec![CppParam {
                    name: "x".to_string(),
                    param_type: CppType::Double,
                    has_default: false,
                }],
                is_static: false,
                is_const: false,
                is_virtual: false,
                is_noexcept: false,
                template_params: vec![],
            }),
            CppDecl::Enum(CppEnum {
                name: "Status".to_string(),
                namespace: vec![],
                variants: vec![
                    ("OK".to_string(), 0),
                    ("Error".to_string(), 1),
                ],
                is_scoped: false,
            }),
        ];
        let code = generate_fajar_bindings(&decls);
        assert!(code.contains("extern fn sqrt(x: f64) -> f64"));
        assert!(code.contains("enum Status"));
        assert!(code.contains("OK = 0"));
    }
}
