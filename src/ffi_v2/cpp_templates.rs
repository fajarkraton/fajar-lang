//! C++ Template Support for FFI v2.
//!
//! Sprint E1: Provides template class detection, instantiation, nested template
//! decomposition, method type substitution, SFINAE/concepts, aliases, variadic
//! templates, partial specialization, and default template arguments.
//!
//! This is a simulated implementation (no real libclang). All C++ types are
//! represented as strings for analysis and binding generation.

use std::collections::HashMap;

use super::cpp::{CppClass, CppType};

// ═══════════════════════════════════════════════════════════════════════
// E1.1: Template Class Detection
// ═══════════════════════════════════════════════════════════════════════

/// The kind of a C++ template parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateParamKind {
    /// A type parameter (`typename T` / `class T`).
    Type,
    /// A non-type parameter (`int N`, `bool B`).
    NonType(String),
    /// A template-template parameter (`template<class> class Container`).
    Template(Vec<TemplateParam>),
}

/// A single C++ template parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParam {
    /// Parameter name (e.g., `T`, `N`, `Alloc`).
    pub name: String,
    /// Parameter kind.
    pub kind: TemplateParamKind,
    /// Default value, if any (e.g., `int` for `class T = int`).
    pub default_value: Option<String>,
}

/// Information about a C++ template class.
#[derive(Debug, Clone)]
pub struct TemplateClassInfo {
    /// Template class name (e.g., `vector`, `map`).
    pub name: String,
    /// Namespace path (e.g., `["std"]`).
    pub namespace: Vec<String>,
    /// Template parameters.
    pub params: Vec<TemplateParam>,
    /// Methods defined on this template.
    pub methods: Vec<TemplateMethodInfo>,
    /// Known specializations (e.g., `vector<bool>`).
    pub specializations: Vec<TemplateSpecialization>,
    /// Known aliases pointing to this template.
    pub aliases: Vec<String>,
}

/// Detects whether a `CppClass` is a template and extracts template info.
///
/// Returns `Some(TemplateClassInfo)` if the class has template parameters,
/// `None` otherwise.
pub fn detect_template_class(class: &CppClass) -> Option<TemplateClassInfo> {
    if class.template_params.is_empty() {
        return None;
    }

    let params: Vec<TemplateParam> = class
        .template_params
        .iter()
        .map(|p| parse_template_param(p))
        .collect();

    let methods = class
        .methods
        .iter()
        .map(|m| TemplateMethodInfo {
            name: m.name.clone(),
            return_type: cpp_type_to_string(&m.return_type),
            param_types: m
                .params
                .iter()
                .map(|p| (p.name.clone(), cpp_type_to_string(&p.param_type)))
                .collect(),
            is_const: m.is_const,
            is_static: m.is_static,
            constraints: Vec::new(),
        })
        .collect();

    Some(TemplateClassInfo {
        name: class.name.clone(),
        namespace: class.namespace.clone(),
        params,
        methods,
        specializations: Vec::new(),
        aliases: Vec::new(),
    })
}

/// Parses a template parameter string into a `TemplateParam`.
///
/// Recognizes patterns like `T`, `class T`, `typename T`, `int N`,
/// `class T = int`, `typename... Args`.
fn parse_template_param(raw: &str) -> TemplateParam {
    let trimmed = raw.trim();

    // Check for default: "T = int", "class T = int"
    let (main_part, default) = if let Some(eq_pos) = trimmed.find('=') {
        let left = trimmed[..eq_pos].trim();
        let right = trimmed[eq_pos + 1..].trim();
        (left, Some(right.to_string()))
    } else {
        (trimmed, None)
    };

    // Check for variadic: "typename... Args"
    let is_variadic = main_part.contains("...");
    let cleaned = main_part.replace("...", "");
    let cleaned = cleaned.trim();

    // Strip "class " or "typename " prefix
    let name_part = cleaned
        .strip_prefix("class ")
        .or_else(|| cleaned.strip_prefix("typename "))
        .unwrap_or(cleaned)
        .trim();

    // Check for non-type parameter: "int N", "size_t N", "bool B"
    let non_type_prefixes = ["int", "size_t", "bool", "unsigned", "long", "char"];
    for prefix in &non_type_prefixes {
        if let Some(rest) = name_part.strip_prefix(prefix) {
            let rest = rest.trim();
            if !rest.is_empty() {
                return TemplateParam {
                    name: if is_variadic {
                        format!("{}...", rest)
                    } else {
                        rest.to_string()
                    },
                    kind: TemplateParamKind::NonType(prefix.to_string()),
                    default_value: default,
                };
            }
        }
    }

    TemplateParam {
        name: if is_variadic {
            format!("{}...", name_part)
        } else {
            name_part.to_string()
        },
        kind: TemplateParamKind::Type,
        default_value: default,
    }
}

/// Converts a `CppType` to its string representation for template processing.
fn cpp_type_to_string(t: &CppType) -> String {
    t.to_fajar_type()
}

// ═══════════════════════════════════════════════════════════════════════
// E1.2: Template Instantiation
// ═══════════════════════════════════════════════════════════════════════

/// A concrete instantiation of a template with specific type arguments.
#[derive(Debug, Clone)]
pub struct TemplateInstantiation {
    /// Original template name (e.g., `vector`).
    pub template_name: String,
    /// Concrete type arguments (e.g., `["i32"]`).
    pub type_args: Vec<String>,
    /// Generated binding name (e.g., `vector_i32`).
    pub binding_name: String,
    /// Methods with types fully substituted.
    pub methods: Vec<TemplateMethodInfo>,
}

/// Instantiates a template class with concrete type arguments.
///
/// For example, `vector<int>` produces a binding named `vector_i32`
/// with all `T` occurrences replaced by `i32`.
///
/// # Errors
///
/// Returns an error string if the number of type arguments does not
/// match the number of template parameters (after applying defaults).
pub fn instantiate_template(
    info: &TemplateClassInfo,
    type_args: &[&str],
) -> Result<TemplateInstantiation, String> {
    // Build substitution map, applying defaults where needed
    let substitution = build_substitution_map(&info.params, type_args)?;

    let binding_name = generate_binding_name(&info.name, type_args);

    let methods = info
        .methods
        .iter()
        .map(|m| substitute_method(m, &substitution))
        .collect();

    Ok(TemplateInstantiation {
        template_name: info.name.clone(),
        type_args: type_args.iter().map(|s| s.to_string()).collect(),
        binding_name,
        methods,
    })
}

/// Builds a substitution map from template param names to concrete types.
///
/// # Errors
///
/// Returns an error if too many arguments are provided or if a required
/// parameter has no argument and no default.
fn build_substitution_map(
    params: &[TemplateParam],
    type_args: &[&str],
) -> Result<HashMap<String, String>, String> {
    let required_count = params.iter().filter(|p| p.default_value.is_none()).count();

    if type_args.len() > params.len() {
        return Err(format!(
            "too many template arguments: expected at most {}, got {}",
            params.len(),
            type_args.len()
        ));
    }

    if type_args.len() < required_count {
        return Err(format!(
            "too few template arguments: expected at least {}, got {}",
            required_count,
            type_args.len()
        ));
    }

    let mut map = HashMap::new();
    for (i, param) in params.iter().enumerate() {
        let value = if i < type_args.len() {
            type_args[i].to_string()
        } else if let Some(ref default) = param.default_value {
            map_cpp_type_name(default)
        } else {
            return Err(format!(
                "missing template argument for parameter '{}'",
                param.name
            ));
        };
        // Strip variadic marker for lookup
        let key = param.name.trim_end_matches("...").to_string();
        map.insert(key, value);
    }

    Ok(map)
}

/// Generates a flattened binding name from a template and its arguments.
///
/// Examples:
/// - `("vector", ["i32"])` -> `"vector_i32"`
/// - `("map", ["str", "i32"])` -> `"map_str_i32"`
fn generate_binding_name(template_name: &str, type_args: &[&str]) -> String {
    let sanitized: Vec<String> = type_args.iter().map(|a| sanitize_type_name(a)).collect();
    if sanitized.is_empty() {
        template_name.to_string()
    } else {
        format!("{}_{}", template_name, sanitized.join("_"))
    }
}

/// Sanitizes a type name for use in identifiers.
///
/// Replaces `<`, `>`, `,`, ` `, `*`, `&` with underscores and colons with `_`.
fn sanitize_type_name(name: &str) -> String {
    name.replace('<', "_")
        .replace('>', "")
        .replace(',', "_")
        .replace(' ', "")
        .replace('*', "ptr")
        .replace('&', "ref")
        .replace("::", "_")
}

/// Maps common C++ type names to Fajar Lang equivalents.
fn map_cpp_type_name(cpp_name: &str) -> String {
    match cpp_name.trim() {
        "int" => "i32".to_string(),
        "long" | "long long" => "i64".to_string(),
        "unsigned int" | "unsigned" => "u32".to_string(),
        "unsigned long" | "unsigned long long" => "u64".to_string(),
        "short" => "i16".to_string(),
        "unsigned short" => "u16".to_string(),
        "char" => "char".to_string(),
        "unsigned char" => "u8".to_string(),
        "signed char" => "i8".to_string(),
        "float" => "f32".to_string(),
        "double" => "f64".to_string(),
        "bool" => "bool".to_string(),
        "size_t" => "usize".to_string(),
        "std::string" | "string" => "str".to_string(),
        "void" => "void".to_string(),
        other => other.to_string(),
    }
}

/// Substitutes template parameters in a method's types.
fn substitute_method(
    method: &TemplateMethodInfo,
    substitution: &HashMap<String, String>,
) -> TemplateMethodInfo {
    TemplateMethodInfo {
        name: method.name.clone(),
        return_type: substitute_type(&method.return_type, substitution),
        param_types: method
            .param_types
            .iter()
            .map(|(name, ty)| (name.clone(), substitute_type(ty, substitution)))
            .collect(),
        is_const: method.is_const,
        is_static: method.is_static,
        constraints: method.constraints.clone(),
    }
}

/// Performs type substitution on a single type string.
///
/// Replaces occurrences of template parameter names with their concrete types.
fn substitute_type(type_str: &str, substitution: &HashMap<String, String>) -> String {
    let mut result = type_str.to_string();
    for (param, concrete) in substitution {
        // Replace whole-word occurrences only
        result = replace_type_param(&result, param, concrete);
    }
    result
}

/// Replaces whole-word occurrences of `param` with `replacement` in `source`.
fn replace_type_param(source: &str, param: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.char_indices().peekable();

    while let Some((i, _)) = chars.peek().copied() {
        if source[i..].starts_with(param) {
            let before_ok = i == 0
                || !source[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let after_pos = i + param.len();
            let after_ok = after_pos >= source.len()
                || !source[after_pos..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');

            if before_ok && after_ok {
                result.push_str(replacement);
                // Skip param.len() chars
                for _ in 0..param.len() {
                    chars.next();
                }
                continue;
            }
        }
        result.push(source.as_bytes()[i] as char);
        chars.next();
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════
// E1.3: Nested Templates
// ═══════════════════════════════════════════════════════════════════════

/// A parsed nested template type (e.g., `map<string, vector<int>>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateType {
    /// A simple non-template type (e.g., `int`, `string`).
    Simple(String),
    /// A template type with arguments (e.g., `vector<int>`).
    Parameterized {
        /// Template name.
        name: String,
        /// Type arguments (may themselves be templates).
        args: Vec<TemplateType>,
    },
}

impl TemplateType {
    /// Returns a flattened binding name for this template type.
    ///
    /// Examples:
    /// - `Simple("i32")` -> `"i32"`
    /// - `Parameterized { name: "vector", args: [Simple("i32")] }` -> `"vector_i32"`
    /// - `Parameterized { name: "map", args: [Simple("str"), Parameterized { name: "vector", args: [Simple("i32")] }] }` -> `"map_str_vector_i32"`
    pub fn flatten_name(&self) -> String {
        match self {
            Self::Simple(name) => sanitize_type_name(name),
            Self::Parameterized { name, args } => {
                let arg_names: Vec<String> = args.iter().map(|a| a.flatten_name()).collect();
                format!("{}_{}", sanitize_type_name(name), arg_names.join("_"))
            }
        }
    }

    /// Returns the C++ source representation.
    ///
    /// Examples:
    /// - `Simple("int")` -> `"int"`
    /// - `Parameterized { name: "vector", args: [Simple("int")] }` -> `"vector<int>"`
    pub fn to_cpp_string(&self) -> String {
        match self {
            Self::Simple(name) => name.clone(),
            Self::Parameterized { name, args } => {
                let arg_strs: Vec<String> = args.iter().map(|a| a.to_cpp_string()).collect();
                format!("{}<{}>", name, arg_strs.join(", "))
            }
        }
    }

    /// Returns the depth of nesting (0 for simple types, 1+ for templates).
    pub fn nesting_depth(&self) -> usize {
        match self {
            Self::Simple(_) => 0,
            Self::Parameterized { args, .. } => {
                1 + args.iter().map(|a| a.nesting_depth()).max().unwrap_or(0)
            }
        }
    }
}

/// Parses a C++ template type string into a `TemplateType` tree.
///
/// Handles nested templates like `map<string, vector<int>>`.
///
/// # Errors
///
/// Returns an error string if the angle brackets are mismatched.
pub fn parse_template_type(input: &str) -> Result<TemplateType, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty type string".to_string());
    }

    // Find the first '<' that is at nesting level 0
    let mut depth = 0i32;
    let mut first_open = None;

    for (i, ch) in trimmed.char_indices() {
        match ch {
            '<' => {
                if depth == 0 {
                    first_open = Some(i);
                }
                depth += 1;
            }
            '>' => {
                depth -= 1;
                if depth < 0 {
                    return Err(format!("unmatched '>' in type: {trimmed}"));
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(format!("unmatched '<' in type: {trimmed}"));
    }

    match first_open {
        None => {
            // Simple type, no angle brackets
            Ok(TemplateType::Simple(trimmed.to_string()))
        }
        Some(open_pos) => {
            let name = trimmed[..open_pos].trim().to_string();
            // The content between the outermost < and >
            let inner = &trimmed[open_pos + 1..trimmed.len() - 1];
            let args = split_template_args(inner)?;
            let parsed_args: Result<Vec<TemplateType>, String> =
                args.iter().map(|a| parse_template_type(a.trim())).collect();
            Ok(TemplateType::Parameterized {
                name,
                args: parsed_args?,
            })
        }
    }
}

/// Splits template arguments at top-level commas, respecting nested `<>`.
///
/// # Errors
///
/// Returns an error if angle brackets are mismatched inside the argument list.
fn split_template_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in input.chars() {
        match ch {
            '<' => {
                depth += 1;
                current.push(ch);
            }
            '>' => {
                depth -= 1;
                if depth < 0 {
                    return Err("unmatched '>' in template arguments".to_string());
                }
                current.push(ch);
            }
            ',' if depth == 0 => {
                let arg = current.trim().to_string();
                if !arg.is_empty() {
                    args.push(arg);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if depth != 0 {
        return Err("unmatched '<' in template arguments".to_string());
    }

    let remaining = current.trim().to_string();
    if !remaining.is_empty() {
        args.push(remaining);
    }

    Ok(args)
}

// ═══════════════════════════════════════════════════════════════════════
// E1.4: Template Methods
// ═══════════════════════════════════════════════════════════════════════

/// A method on a template class, with types that may reference template params.
#[derive(Debug, Clone)]
pub struct TemplateMethodInfo {
    /// Method name (e.g., `push_back`, `at`, `size`).
    pub name: String,
    /// Return type (may contain template param names like `T`).
    pub return_type: String,
    /// Parameter names and types.
    pub param_types: Vec<(String, String)>,
    /// Whether the method is const.
    pub is_const: bool,
    /// Whether the method is static.
    pub is_static: bool,
    /// SFINAE/concept constraints (E1.5).
    pub constraints: Vec<TypeConstraint>,
}

impl TemplateMethodInfo {
    /// Creates a new template method info.
    pub fn new(name: &str, return_type: &str, params: Vec<(&str, &str)>, is_const: bool) -> Self {
        Self {
            name: name.to_string(),
            return_type: return_type.to_string(),
            param_types: params
                .into_iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect(),
            is_const,
            is_static: false,
            constraints: Vec::new(),
        }
    }

    /// Returns a Fajar Lang method signature string.
    pub fn to_fajar_signature(&self) -> String {
        let self_param = if self.is_static {
            String::new()
        } else if self.is_const {
            "self".to_string()
        } else {
            "mut self".to_string()
        };

        let other_params: Vec<String> = self
            .param_types
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, ty))
            .collect();

        let all_params = if self_param.is_empty() {
            other_params.join(", ")
        } else if other_params.is_empty() {
            self_param
        } else {
            format!("{}, {}", self_param, other_params.join(", "))
        };

        let ret = if self.return_type == "void" {
            String::new()
        } else {
            format!(" -> {}", self.return_type)
        };

        format!("fn {}({}){}", self.name, all_params, ret)
    }
}

/// Generates standard STL-style methods for common template containers.
///
/// Returns methods with type parameters that can later be substituted.
pub fn generate_stl_methods(template_name: &str, param_names: &[&str]) -> Vec<TemplateMethodInfo> {
    let t = param_names.first().copied().unwrap_or("T");
    match template_name {
        "vector" => vec![
            TemplateMethodInfo::new("push_back", "void", vec![("value", t)], false),
            TemplateMethodInfo::new("pop_back", "void", vec![], false),
            TemplateMethodInfo::new("at", t, vec![("index", "usize")], true),
            TemplateMethodInfo::new("size", "usize", vec![], true),
            TemplateMethodInfo::new("empty", "bool", vec![], true),
            TemplateMethodInfo::new("clear", "void", vec![], false),
            TemplateMethodInfo::new("front", t, vec![], true),
            TemplateMethodInfo::new("back", t, vec![], true),
        ],
        "map" | "unordered_map" => {
            let k = t;
            let v = param_names.get(1).copied().unwrap_or("V");
            vec![
                TemplateMethodInfo::new("insert", "void", vec![("key", k), ("value", v)], false),
                TemplateMethodInfo::new("at", v, vec![("key", k)], true),
                TemplateMethodInfo::new("size", "usize", vec![], true),
                TemplateMethodInfo::new("empty", "bool", vec![], true),
                TemplateMethodInfo::new("contains", "bool", vec![("key", k)], true),
                TemplateMethodInfo::new("erase", "void", vec![("key", k)], false),
            ]
        }
        "set" | "unordered_set" => vec![
            TemplateMethodInfo::new("insert", "void", vec![("value", t)], false),
            TemplateMethodInfo::new("contains", "bool", vec![("value", t)], true),
            TemplateMethodInfo::new("size", "usize", vec![], true),
            TemplateMethodInfo::new("empty", "bool", vec![], true),
            TemplateMethodInfo::new("erase", "void", vec![("value", t)], false),
        ],
        "optional" => vec![
            TemplateMethodInfo::new("value", t, vec![], true),
            TemplateMethodInfo::new("has_value", "bool", vec![], true),
            TemplateMethodInfo::new("value_or", t, vec![("default_val", t)], true),
        ],
        "shared_ptr" | "unique_ptr" => vec![
            TemplateMethodInfo::new("get", &format!("*mut {t}"), vec![], true),
            TemplateMethodInfo::new("reset", "void", vec![], false),
        ],
        _ => Vec::new(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E1.5: SFINAE / Concepts
// ═══════════════════════════════════════════════════════════════════════

/// A type constraint for conditional method availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    /// Requires a trait/concept (e.g., `Comparable`, `Hashable`).
    RequiresTrait(String),
    /// Requires a specific type match (e.g., `T == bool`).
    TypeEquals(String, String),
    /// Requires the type to be one of a set (e.g., `T in {int, float, double}`).
    OneOf(String, Vec<String>),
    /// Requires the type to be arithmetic.
    IsArithmetic(String),
    /// A C++20 concept requirement.
    Concept(String, Vec<String>),
}

impl TypeConstraint {
    /// Checks whether a concrete type satisfies this constraint.
    pub fn is_satisfied_by(&self, substitution: &HashMap<String, String>) -> bool {
        match self {
            Self::RequiresTrait(_) => {
                // Simulated: always satisfied unless we have specific info
                true
            }
            Self::TypeEquals(param, expected) => substitution.get(param) == Some(expected),
            Self::OneOf(param, allowed) => substitution
                .get(param)
                .is_some_and(|actual| allowed.iter().any(|a| a == actual)),
            Self::IsArithmetic(param) => {
                let arithmetic = [
                    "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "isize",
                    "usize",
                ];
                substitution
                    .get(param)
                    .is_some_and(|actual| arithmetic.contains(&actual.as_str()))
            }
            Self::Concept(_, _) => {
                // Simulated: concepts treated as satisfied by default
                true
            }
        }
    }
}

/// Filters methods based on type constraints after substitution.
///
/// Methods whose constraints are not satisfied by the given type arguments
/// are excluded from the returned list.
pub fn filter_constrained_methods(
    methods: &[TemplateMethodInfo],
    substitution: &HashMap<String, String>,
) -> Vec<TemplateMethodInfo> {
    methods
        .iter()
        .filter(|m| {
            m.constraints
                .iter()
                .all(|c| c.is_satisfied_by(substitution))
        })
        .cloned()
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// E1.6: Template Aliases
// ═══════════════════════════════════════════════════════════════════════

/// A `using` alias that maps an alias name to a template instantiation.
///
/// For example, `using IntVec = vector<int>` becomes an entry mapping
/// `"IntVec"` to the template type `vector<int>`.
#[derive(Debug, Clone)]
pub struct TemplateAlias {
    /// Alias name (e.g., `IntVec`).
    pub alias: String,
    /// The target template type.
    pub target: TemplateType,
}

/// A collection of template aliases for resolution.
#[derive(Debug, Clone, Default)]
pub struct TemplateAliasMap {
    /// Maps alias names to their resolved template types.
    aliases: HashMap<String, TemplateType>,
}

impl TemplateAliasMap {
    /// Creates a new empty alias map.
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }

    /// Registers a `using` alias.
    ///
    /// # Errors
    ///
    /// Returns an error if the target type string cannot be parsed.
    pub fn add_alias(&mut self, alias: &str, target_type: &str) -> Result<(), String> {
        let parsed = parse_template_type(target_type)?;
        self.aliases.insert(alias.to_string(), parsed);
        Ok(())
    }

    /// Resolves an alias to its underlying template type.
    ///
    /// Returns `None` if the name is not a known alias.
    pub fn resolve(&self, name: &str) -> Option<&TemplateType> {
        self.aliases.get(name)
    }

    /// Recursively resolves an alias chain.
    ///
    /// For example, if `A -> B` and `B -> vector<int>`, resolving `A`
    /// returns `vector<int>`.
    pub fn resolve_deep(&self, name: &str) -> Option<&TemplateType> {
        let mut current_name = name;
        let mut visited = Vec::new();

        loop {
            if visited.contains(&current_name) {
                // Cycle detected — break
                return self.aliases.get(current_name);
            }
            visited.push(current_name);

            match self.aliases.get(current_name) {
                Some(TemplateType::Simple(next_name)) => {
                    if self.aliases.contains_key(next_name.as_str()) {
                        current_name = next_name.as_str();
                    } else {
                        return self.aliases.get(visited.last().copied().unwrap_or(name));
                    }
                }
                Some(other) => return Some(other),
                None => return None,
            }
        }
    }

    /// Returns the number of registered aliases.
    pub fn len(&self) -> usize {
        self.aliases.len()
    }

    /// Returns whether the alias map is empty.
    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E1.7: Variadic Templates
// ═══════════════════════════════════════════════════════════════════════

/// Represents a variadic template expansion.
///
/// For `template<typename... Args>`, this tracks the parameter pack
/// and its expansion for tuple-like types.
#[derive(Debug, Clone)]
pub struct VariadicExpansion {
    /// Pack parameter name (e.g., `Args`).
    pub pack_name: String,
    /// Concrete types this pack expands to (e.g., `["i32", "f64", "str"]`).
    pub expanded_types: Vec<String>,
}

/// Expands a variadic template into concrete fields or parameters.
///
/// For a `tuple<int, double, string>`, generates fields like:
/// - `field_0: i32`
/// - `field_1: f64`
/// - `field_2: str`
pub fn expand_variadic(pack_name: &str, types: &[&str]) -> VariadicExpansion {
    VariadicExpansion {
        pack_name: pack_name.to_string(),
        expanded_types: types.iter().map(|t| map_cpp_type_name(t)).collect(),
    }
}

/// Generates a Fajar Lang struct for a variadic tuple-like template.
///
/// For `tuple<int, double, string>` produces:
/// ```text
/// struct tuple_i32_f64_str {
///     field_0: i32,
///     field_1: f64,
///     field_2: str,
/// }
/// ```
pub fn generate_variadic_struct(template_name: &str, expansion: &VariadicExpansion) -> String {
    let type_parts: Vec<&str> = expansion
        .expanded_types
        .iter()
        .map(|s| s.as_str())
        .collect();
    let binding_name = generate_binding_name(template_name, &type_parts);

    let mut code = format!("struct {binding_name} {{\n");
    for (i, ty) in expansion.expanded_types.iter().enumerate() {
        code.push_str(&format!("    field_{i}: {ty},\n"));
    }
    code.push_str("}\n");
    code
}

// ═══════════════════════════════════════════════════════════════════════
// E1.8: Partial Specialization
// ═══════════════════════════════════════════════════════════════════════

/// A partial or full specialization of a template.
#[derive(Debug, Clone)]
pub struct TemplateSpecialization {
    /// The specialized type arguments (e.g., `["bool"]` for `vector<bool>`).
    pub specialized_args: Vec<String>,
    /// Whether this is a full specialization (all params specified).
    pub is_full: bool,
    /// Overridden methods for this specialization.
    pub methods: Vec<TemplateMethodInfo>,
    /// Description of what changes in this specialization.
    pub description: String,
}

/// Checks whether a specialization matches the given type arguments.
///
/// A full specialization matches if all args are equal. A partial
/// specialization matches if the specialized positions match and
/// the rest are wildcards (`"*"`).
pub fn specialization_matches(spec: &TemplateSpecialization, type_args: &[&str]) -> bool {
    if spec.specialized_args.len() != type_args.len() {
        return false;
    }

    spec.specialized_args
        .iter()
        .zip(type_args.iter())
        .all(|(spec_arg, actual)| spec_arg == "*" || spec_arg == *actual)
}

/// Selects the best matching specialization for the given type arguments.
///
/// Prefers full specializations over partial ones. Returns `None` if no
/// specialization matches.
pub fn select_specialization<'a>(
    specializations: &'a [TemplateSpecialization],
    type_args: &[&str],
) -> Option<&'a TemplateSpecialization> {
    // First look for full specializations
    let full_match = specializations
        .iter()
        .find(|s| s.is_full && specialization_matches(s, type_args));

    if full_match.is_some() {
        return full_match;
    }

    // Then partial specializations
    specializations
        .iter()
        .find(|s| !s.is_full && specialization_matches(s, type_args))
}

// ═══════════════════════════════════════════════════════════════════════
// E1.9: Default Template Arguments
// ═══════════════════════════════════════════════════════════════════════

/// Applies default template arguments to fill in missing type args.
///
/// For `template<class T, class Alloc = std::allocator<T>>` called as
/// `vector<int>`, fills in `Alloc = std::allocator<int>`.
///
/// # Errors
///
/// Returns an error if required parameters are missing.
pub fn apply_defaults(
    params: &[TemplateParam],
    provided_args: &[&str],
) -> Result<Vec<String>, String> {
    let required_count = params.iter().filter(|p| p.default_value.is_none()).count();

    if provided_args.len() < required_count {
        return Err(format!(
            "need at least {} template arguments, got {}",
            required_count,
            provided_args.len()
        ));
    }

    if provided_args.len() > params.len() {
        return Err(format!(
            "too many template arguments: max {}, got {}",
            params.len(),
            provided_args.len()
        ));
    }

    let mut result = Vec::with_capacity(params.len());

    // Build partial substitution from provided args for default expansion
    let mut partial_sub = HashMap::new();
    for (i, arg) in provided_args.iter().enumerate() {
        if let Some(param) = params.get(i) {
            partial_sub.insert(param.name.clone(), arg.to_string());
        }
    }

    for (i, param) in params.iter().enumerate() {
        if i < provided_args.len() {
            result.push(provided_args[i].to_string());
        } else if let Some(ref default) = param.default_value {
            // Substitute already-known params in the default
            let resolved = substitute_type(&map_cpp_type_name(default), &partial_sub);
            result.push(resolved);
        } else {
            return Err(format!(
                "missing argument for required template parameter '{}'",
                param.name
            ));
        }
    }

    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// TemplateResolver — unified resolver
// ═══════════════════════════════════════════════════════════════════════

/// High-level resolver that manages templates, aliases, and specializations.
///
/// Provides a unified API for resolving template types to concrete Fajar Lang
/// bindings, handling nested templates, aliases, specializations, and defaults.
#[derive(Debug, Clone, Default)]
pub struct TemplateResolver {
    /// Registered template classes.
    templates: HashMap<String, TemplateClassInfo>,
    /// Template alias map.
    aliases: TemplateAliasMap,
}

impl TemplateResolver {
    /// Creates a new empty resolver.
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            aliases: TemplateAliasMap::new(),
        }
    }

    /// Registers a template class.
    pub fn register_template(&mut self, info: TemplateClassInfo) {
        self.templates.insert(info.name.clone(), info);
    }

    /// Registers a type alias.
    ///
    /// # Errors
    ///
    /// Returns an error if the target type cannot be parsed.
    pub fn register_alias(&mut self, alias: &str, target: &str) -> Result<(), String> {
        self.aliases.add_alias(alias, target)
    }

    /// Looks up a template by name, resolving aliases first.
    pub fn lookup(&self, name: &str) -> Option<&TemplateClassInfo> {
        // Check direct registration first
        if let Some(info) = self.templates.get(name) {
            return Some(info);
        }

        // Check aliases
        if let Some(TemplateType::Parameterized {
            name: tmpl_name, ..
        }) = self.aliases.resolve(name)
        {
            return self.templates.get(tmpl_name);
        }

        None
    }

    /// Resolves a template type string into a concrete `TemplateInstantiation`.
    ///
    /// Handles aliases, nested types, defaults, and specializations.
    ///
    /// # Errors
    ///
    /// Returns an error if the type cannot be parsed or the template is unknown.
    pub fn resolve(&self, type_str: &str) -> Result<TemplateInstantiation, String> {
        // First check if it is an alias
        let resolved_type = if let Some(alias_target) = self.aliases.resolve(type_str) {
            alias_target.to_cpp_string()
        } else {
            type_str.to_string()
        };

        let parsed = parse_template_type(&resolved_type)?;

        match parsed {
            TemplateType::Simple(name) => Err(format!("'{name}' is not a template type")),
            TemplateType::Parameterized { name, args } => {
                let info = self
                    .templates
                    .get(&name)
                    .ok_or_else(|| format!("unknown template: '{name}'"))?;

                let arg_strings: Vec<String> = args.iter().map(|a| a.flatten_name()).collect();
                let arg_refs: Vec<&str> = arg_strings.iter().map(|s| s.as_str()).collect();

                // Apply defaults
                let full_args = apply_defaults(&info.params, &arg_refs)?;
                let full_refs: Vec<&str> = full_args.iter().map(|s| s.as_str()).collect();

                // Check specializations
                if let Some(spec) = select_specialization(&info.specializations, &full_refs) {
                    let binding_name = generate_binding_name(&name, &full_refs);
                    let sub = build_substitution_map(&info.params, &full_refs)?;
                    let methods = if spec.methods.is_empty() {
                        info.methods
                            .iter()
                            .map(|m| substitute_method(m, &sub))
                            .collect()
                    } else {
                        spec.methods
                            .iter()
                            .map(|m| substitute_method(m, &sub))
                            .collect()
                    };
                    return Ok(TemplateInstantiation {
                        template_name: name,
                        type_args: full_args,
                        binding_name,
                        methods,
                    });
                }

                instantiate_template(info, &full_refs)
            }
        }
    }

    /// Generates a complete Fajar Lang binding for a template instantiation.
    pub fn generate_binding(&self, inst: &TemplateInstantiation) -> String {
        let mut code = String::new();

        code.push_str(&format!(
            "// Binding for {}<{}>\n",
            inst.template_name,
            inst.type_args.join(", ")
        ));
        code.push_str(&format!("struct {} {{\n", inst.binding_name));
        code.push_str("    _opaque: *mut u8\n");
        code.push_str("}\n\n");

        code.push_str(&format!("impl {} {{\n", inst.binding_name));
        for method in &inst.methods {
            code.push_str(&format!("    @ffi {}\n", method.to_fajar_signature()));
        }
        code.push_str("}\n");

        code
    }

    /// Returns the number of registered templates.
    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    /// Returns the number of registered aliases.
    pub fn alias_count(&self) -> usize {
        self.aliases.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E1.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_v2::cpp::CppFunction;

    // ── E1.1: Template Class Detection ──────────────────────────────

    #[test]
    fn e1_1_detect_template_class() {
        let class = CppClass {
            name: "Container".to_string(),
            namespace: vec!["std".to_string()],
            bases: vec![],
            fields: vec![],
            methods: vec![CppFunction {
                name: "get".to_string(),
                namespace: vec![],
                return_type: CppType::Custom("T".to_string()),
                params: vec![],
                is_static: false,
                is_const: true,
                is_virtual: false,
                is_noexcept: false,
                template_params: vec![],
            }],
            constructors: vec![],
            has_destructor: false,
            is_abstract: false,
            template_params: vec!["T".to_string()],
            size_bytes: 8,
            align_bytes: 8,
        };

        let info = detect_template_class(&class);
        assert!(info.is_some(), "should detect template class");
        let info = info.unwrap();
        assert_eq!(info.name, "Container");
        assert_eq!(info.params.len(), 1);
        assert_eq!(info.params[0].name, "T");
        assert_eq!(info.params[0].kind, TemplateParamKind::Type);
        assert_eq!(info.methods.len(), 1);
        assert_eq!(info.methods[0].name, "get");
    }

    #[test]
    fn e1_1_non_template_class_returns_none() {
        let class = CppClass {
            name: "Plain".to_string(),
            namespace: vec![],
            bases: vec![],
            fields: vec![],
            methods: vec![],
            constructors: vec![],
            has_destructor: false,
            is_abstract: false,
            template_params: vec![],
            size_bytes: 4,
            align_bytes: 4,
        };
        assert!(detect_template_class(&class).is_none());
    }

    #[test]
    fn e1_1_parse_non_type_param() {
        let param = parse_template_param("int N");
        assert_eq!(param.name, "N");
        assert_eq!(param.kind, TemplateParamKind::NonType("int".to_string()));
    }

    // ── E1.2: Template Instantiation ────────────────────────────────

    #[test]
    fn e1_2_instantiate_vector_int() {
        let info = TemplateClassInfo {
            name: "vector".to_string(),
            namespace: vec!["std".to_string()],
            params: vec![TemplateParam {
                name: "T".to_string(),
                kind: TemplateParamKind::Type,
                default_value: None,
            }],
            methods: vec![
                TemplateMethodInfo::new("push_back", "void", vec![("value", "T")], false),
                TemplateMethodInfo::new("at", "T", vec![("index", "usize")], true),
                TemplateMethodInfo::new("size", "usize", vec![], true),
            ],
            specializations: vec![],
            aliases: vec![],
        };

        let inst = instantiate_template(&info, &["i32"]).unwrap();
        assert_eq!(inst.binding_name, "vector_i32");
        assert_eq!(inst.type_args, vec!["i32"]);
        assert_eq!(inst.methods.len(), 3);

        // Check substitution in push_back
        let push_back = &inst.methods[0];
        assert_eq!(push_back.param_types[0].1, "i32");

        // Check substitution in at() return type
        let at = &inst.methods[1];
        assert_eq!(at.return_type, "i32");
    }

    #[test]
    fn e1_2_instantiate_too_many_args() {
        let info = TemplateClassInfo {
            name: "vector".to_string(),
            namespace: vec![],
            params: vec![TemplateParam {
                name: "T".to_string(),
                kind: TemplateParamKind::Type,
                default_value: None,
            }],
            methods: vec![],
            specializations: vec![],
            aliases: vec![],
        };

        let result = instantiate_template(&info, &["i32", "f64"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too many"));
    }

    // ── E1.3: Nested Templates ──────────────────────────────────────

    #[test]
    fn e1_3_parse_nested_template() {
        let parsed = parse_template_type("map<string, vector<int>>").unwrap();
        match &parsed {
            TemplateType::Parameterized { name, args } => {
                assert_eq!(name, "map");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], TemplateType::Simple("string".to_string()));
                match &args[1] {
                    TemplateType::Parameterized { name, args } => {
                        assert_eq!(name, "vector");
                        assert_eq!(args.len(), 1);
                        assert_eq!(args[0], TemplateType::Simple("int".to_string()));
                    }
                    _ => panic!("expected Parameterized for inner vector"),
                }
            }
            _ => panic!("expected Parameterized"),
        }
    }

    #[test]
    fn e1_3_flatten_nested_name() {
        let parsed = parse_template_type("map<string, vector<int>>").unwrap();
        assert_eq!(parsed.flatten_name(), "map_string_vector_int");
    }

    #[test]
    fn e1_3_nesting_depth() {
        let simple = parse_template_type("int").unwrap();
        assert_eq!(simple.nesting_depth(), 0);

        let one = parse_template_type("vector<int>").unwrap();
        assert_eq!(one.nesting_depth(), 1);

        let two = parse_template_type("map<string, vector<int>>").unwrap();
        assert_eq!(two.nesting_depth(), 2);
    }

    #[test]
    fn e1_3_mismatched_brackets() {
        assert!(parse_template_type("vector<int").is_err());
        assert!(parse_template_type("vector<int>>").is_err());
    }

    // ── E1.4: Template Methods ──────────────────────────────────────

    #[test]
    fn e1_4_stl_vector_methods() {
        let methods = generate_stl_methods("vector", &["T"]);
        assert!(methods.len() >= 6, "vector should have 6+ methods");
        let push_back = methods.iter().find(|m| m.name == "push_back");
        assert!(push_back.is_some());
        let pb = push_back.unwrap();
        assert_eq!(pb.return_type, "void");
        assert_eq!(pb.param_types[0].1, "T");
        assert!(!pb.is_const);
    }

    #[test]
    fn e1_4_method_signature_generation() {
        let method = TemplateMethodInfo::new("at", "i32", vec![("index", "usize")], true);
        let sig = method.to_fajar_signature();
        assert_eq!(sig, "fn at(self, index: usize) -> i32");
    }

    // ── E1.5: SFINAE / Concepts ─────────────────────────────────────

    #[test]
    fn e1_5_arithmetic_constraint() {
        let constraint = TypeConstraint::IsArithmetic("T".to_string());

        let mut sub_int = HashMap::new();
        sub_int.insert("T".to_string(), "i32".to_string());
        assert!(constraint.is_satisfied_by(&sub_int));

        let mut sub_str = HashMap::new();
        sub_str.insert("T".to_string(), "str".to_string());
        assert!(!constraint.is_satisfied_by(&sub_str));
    }

    #[test]
    fn e1_5_filter_constrained_methods() {
        let mut numeric_only = TemplateMethodInfo::new("sum", "T", vec![], true);
        numeric_only
            .constraints
            .push(TypeConstraint::IsArithmetic("T".to_string()));

        let always_avail = TemplateMethodInfo::new("size", "usize", vec![], true);

        let methods = vec![numeric_only, always_avail];

        let mut sub_str = HashMap::new();
        sub_str.insert("T".to_string(), "str".to_string());
        let filtered = filter_constrained_methods(&methods, &sub_str);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "size");

        let mut sub_int = HashMap::new();
        sub_int.insert("T".to_string(), "i32".to_string());
        let filtered = filter_constrained_methods(&methods, &sub_int);
        assert_eq!(filtered.len(), 2);
    }

    // ── E1.6: Template Aliases ──────────────────────────────────────

    #[test]
    fn e1_6_alias_resolution() {
        let mut aliases = TemplateAliasMap::new();
        aliases.add_alias("IntVec", "vector<int>").unwrap();
        aliases.add_alias("StringMap", "map<string, int>").unwrap();

        let resolved = aliases.resolve("IntVec");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().to_cpp_string(), "vector<int>");

        assert_eq!(aliases.len(), 2);
        assert!(aliases.resolve("Unknown").is_none());
    }

    // ── E1.7: Variadic Templates ────────────────────────────────────

    #[test]
    fn e1_7_variadic_expansion() {
        let expansion = expand_variadic("Args", &["int", "double", "string"]);
        assert_eq!(expansion.pack_name, "Args");
        assert_eq!(expansion.expanded_types, vec!["i32", "f64", "str"]);

        let code = generate_variadic_struct("tuple", &expansion);
        assert!(code.contains("struct tuple_i32_f64_str"));
        assert!(code.contains("field_0: i32"));
        assert!(code.contains("field_1: f64"));
        assert!(code.contains("field_2: str"));
    }

    // ── E1.8: Partial Specialization ────────────────────────────────

    #[test]
    fn e1_8_specialization_match() {
        let spec_bool = TemplateSpecialization {
            specialized_args: vec!["bool".to_string()],
            is_full: true,
            methods: vec![TemplateMethodInfo::new("flip", "void", vec![], false)],
            description: "vector<bool> bit-packed specialization".to_string(),
        };

        assert!(specialization_matches(&spec_bool, &["bool"]));
        assert!(!specialization_matches(&spec_bool, &["i32"]));

        let partial_spec = TemplateSpecialization {
            specialized_args: vec!["*".to_string(), "bool".to_string()],
            is_full: false,
            methods: vec![],
            description: "partial: any key with bool value".to_string(),
        };
        assert!(specialization_matches(&partial_spec, &["str", "bool"]));
        assert!(!specialization_matches(&partial_spec, &["str", "i32"]));
    }

    #[test]
    fn e1_8_select_best_specialization() {
        let full = TemplateSpecialization {
            specialized_args: vec!["bool".to_string()],
            is_full: true,
            methods: vec![TemplateMethodInfo::new("flip", "void", vec![], false)],
            description: "full specialization".to_string(),
        };
        let partial = TemplateSpecialization {
            specialized_args: vec!["*".to_string()],
            is_full: false,
            methods: vec![],
            description: "partial catch-all".to_string(),
        };

        let specs = vec![partial.clone(), full.clone()];

        // Full match should be preferred
        let selected = select_specialization(&specs, &["bool"]);
        assert!(selected.is_some());
        assert!(selected.unwrap().is_full);

        // Partial match for non-bool
        let selected = select_specialization(&specs, &["i32"]);
        assert!(selected.is_some());
        assert!(!selected.unwrap().is_full);
    }

    // ── E1.9: Default Template Arguments ────────────────────────────

    #[test]
    fn e1_9_apply_defaults() {
        let params = vec![
            TemplateParam {
                name: "T".to_string(),
                kind: TemplateParamKind::Type,
                default_value: None,
            },
            TemplateParam {
                name: "Alloc".to_string(),
                kind: TemplateParamKind::Type,
                default_value: Some("int".to_string()),
            },
        ];

        // Provide only T, Alloc gets default
        let result = apply_defaults(&params, &["f64"]).unwrap();
        assert_eq!(result, vec!["f64", "i32"]);

        // Provide both
        let result = apply_defaults(&params, &["f64", "u8"]).unwrap();
        assert_eq!(result, vec!["f64", "u8"]);

        // Missing required
        let result = apply_defaults(&params, &[]);
        assert!(result.is_err());
    }

    // ── TemplateResolver integration ────────────────────────────────

    #[test]
    fn e1_10_resolver_full_workflow() {
        let mut resolver = TemplateResolver::new();

        // Register vector template
        let mut vector_info = TemplateClassInfo {
            name: "vector".to_string(),
            namespace: vec!["std".to_string()],
            params: vec![TemplateParam {
                name: "T".to_string(),
                kind: TemplateParamKind::Type,
                default_value: None,
            }],
            methods: generate_stl_methods("vector", &["T"]),
            specializations: vec![TemplateSpecialization {
                specialized_args: vec!["bool".to_string()],
                is_full: true,
                methods: vec![
                    TemplateMethodInfo::new("flip", "void", vec![("index", "usize")], false),
                    TemplateMethodInfo::new("size", "usize", vec![], true),
                ],
                description: "vector<bool> bit-packed".to_string(),
            }],
            aliases: vec!["IntVec".to_string()],
        };
        // Add STL methods if not already present via generate_stl_methods
        if vector_info.methods.is_empty() {
            vector_info.methods = generate_stl_methods("vector", &["T"]);
        }
        resolver.register_template(vector_info);

        // Register alias
        resolver.register_alias("IntVec", "vector<i32>").unwrap();

        // Resolve vector<i32>
        let inst = resolver.resolve("vector<i32>").unwrap();
        assert_eq!(inst.binding_name, "vector_i32");
        assert!(!inst.methods.is_empty());

        // Verify push_back has i32 parameter
        let push_back = inst.methods.iter().find(|m| m.name == "push_back");
        assert!(push_back.is_some());
        assert_eq!(push_back.unwrap().param_types[0].1, "i32");

        // Generate binding code
        let code = resolver.generate_binding(&inst);
        assert!(code.contains("struct vector_i32"));
        assert!(code.contains("@ffi fn push_back"));
        assert!(code.contains("@ffi fn size"));

        // Resolve vector<bool> should use specialization
        let inst_bool = resolver.resolve("vector<bool>").unwrap();
        assert_eq!(inst_bool.binding_name, "vector_bool");
        let has_flip = inst_bool.methods.iter().any(|m| m.name == "flip");
        assert!(
            has_flip,
            "vector<bool> should have flip method from specialization"
        );

        assert_eq!(resolver.template_count(), 1);
        assert_eq!(resolver.alias_count(), 1);
    }

    #[test]
    fn e1_10_resolver_alias_resolution() {
        let mut resolver = TemplateResolver::new();

        resolver.register_template(TemplateClassInfo {
            name: "vector".to_string(),
            namespace: vec![],
            params: vec![TemplateParam {
                name: "T".to_string(),
                kind: TemplateParamKind::Type,
                default_value: None,
            }],
            methods: vec![TemplateMethodInfo::new("size", "usize", vec![], true)],
            specializations: vec![],
            aliases: vec![],
        });

        resolver.register_alias("IntVec", "vector<i32>").unwrap();

        // Resolve via alias
        let inst = resolver.resolve("IntVec").unwrap();
        assert_eq!(inst.template_name, "vector");
        assert_eq!(inst.binding_name, "vector_i32");
    }

    #[test]
    fn e1_10_map_type_names() {
        assert_eq!(map_cpp_type_name("int"), "i32");
        assert_eq!(map_cpp_type_name("double"), "f64");
        assert_eq!(map_cpp_type_name("std::string"), "str");
        assert_eq!(map_cpp_type_name("size_t"), "usize");
        assert_eq!(map_cpp_type_name("bool"), "bool");
        assert_eq!(map_cpp_type_name("MyClass"), "MyClass");
    }

    #[test]
    fn e1_5_type_equals_constraint() {
        let constraint = TypeConstraint::TypeEquals("T".to_string(), "bool".to_string());

        let mut sub_bool = HashMap::new();
        sub_bool.insert("T".to_string(), "bool".to_string());
        assert!(constraint.is_satisfied_by(&sub_bool));

        let mut sub_int = HashMap::new();
        sub_int.insert("T".to_string(), "i32".to_string());
        assert!(!constraint.is_satisfied_by(&sub_int));
    }

    #[test]
    fn e1_5_one_of_constraint() {
        let constraint = TypeConstraint::OneOf(
            "T".to_string(),
            vec!["i32".to_string(), "i64".to_string(), "f64".to_string()],
        );

        let mut sub = HashMap::new();
        sub.insert("T".to_string(), "i64".to_string());
        assert!(constraint.is_satisfied_by(&sub));

        sub.insert("T".to_string(), "str".to_string());
        assert!(!constraint.is_satisfied_by(&sub));
    }

    #[test]
    fn e1_3_to_cpp_string_roundtrip() {
        let input = "map<string, vector<int>>";
        let parsed = parse_template_type(input).unwrap();
        assert_eq!(parsed.to_cpp_string(), input);
    }

    #[test]
    fn e1_6_alias_map_empty() {
        let aliases = TemplateAliasMap::new();
        assert!(aliases.is_empty());
        assert_eq!(aliases.len(), 0);
    }

    #[test]
    fn e1_1_parse_param_with_default() {
        let param = parse_template_param("class T = int");
        assert_eq!(param.name, "T");
        assert_eq!(param.kind, TemplateParamKind::Type);
        assert_eq!(param.default_value, Some("int".to_string()));
    }

    #[test]
    fn e1_7_variadic_param_detection() {
        let param = parse_template_param("typename... Args");
        assert_eq!(param.name, "Args...");
        assert_eq!(param.kind, TemplateParamKind::Type);
    }

    #[test]
    fn e1_4_stl_map_methods() {
        let methods = generate_stl_methods("map", &["K", "V"]);
        assert!(methods.len() >= 4, "map should have 4+ methods");

        let insert = methods.iter().find(|m| m.name == "insert").unwrap();
        assert_eq!(insert.param_types.len(), 2);
        assert_eq!(insert.param_types[0].1, "K");
        assert_eq!(insert.param_types[1].1, "V");
    }
}
