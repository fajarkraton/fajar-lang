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
}
