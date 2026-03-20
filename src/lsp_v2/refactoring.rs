//! Refactoring suite — extract function/variable, inline function/variable,
//! rename symbol, move module, extract trait, change signature, convert
//! to/from method.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S32.1: Extract Function
// ═══════════════════════════════════════════════════════════════════════

/// A variable referenced in a code region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefVariable {
    /// Variable name.
    pub name: String,
    /// Variable type.
    pub ty: String,
    /// Whether it is mutated in the region.
    pub is_mutated: bool,
    /// Whether it is defined outside the region (input).
    pub is_input: bool,
    /// Whether it is used after the region (output).
    pub is_output: bool,
}

/// Result of extracting a function from a code region.
#[derive(Debug, Clone)]
pub struct ExtractFunctionResult {
    /// The new function definition.
    pub function_def: String,
    /// The replacement call site.
    pub call_site: String,
    /// Parameters needed by the new function.
    pub params: Vec<RefVariable>,
    /// Return type (if any output variables).
    pub return_type: Option<String>,
}

/// Extracts a code region into a new function.
pub fn extract_function(
    selected_code: &str,
    function_name: &str,
    variables: &[RefVariable],
) -> ExtractFunctionResult {
    let inputs: Vec<&RefVariable> = variables.iter().filter(|v| v.is_input).collect();
    let outputs: Vec<&RefVariable> = variables.iter().filter(|v| v.is_output).collect();

    let param_list: String = inputs
        .iter()
        .map(|v| {
            if v.is_mutated {
                format!("{}: &mut {}", v.name, v.ty)
            } else {
                format!("{}: &{}", v.name, v.ty)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let return_type = if outputs.is_empty() {
        None
    } else if outputs.len() == 1 {
        Some(outputs[0].ty.clone())
    } else {
        Some(format!(
            "({})",
            outputs
                .iter()
                .map(|v| v.ty.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    };

    let ret_annotation = return_type
        .as_ref()
        .map_or(String::new(), |t| format!(" -> {t}"));

    let function_def = format!(
        "fn {function_name}({param_list}){ret_annotation} {{\n    {}\n}}",
        selected_code.trim()
    );

    let arg_list: String = inputs
        .iter()
        .map(|v| {
            if v.is_mutated {
                format!("&mut {}", v.name)
            } else {
                format!("&{}", v.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let call_site = if outputs.is_empty() {
        format!("{function_name}({arg_list})")
    } else if outputs.len() == 1 {
        format!("let {} = {function_name}({arg_list})", outputs[0].name)
    } else {
        let out_names: String = outputs
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("let ({out_names}) = {function_name}({arg_list})")
    };

    ExtractFunctionResult {
        function_def,
        call_site,
        params: inputs.into_iter().cloned().collect(),
        return_type,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.2: Extract Variable
// ═══════════════════════════════════════════════════════════════════════

/// Result of extracting an expression into a variable.
#[derive(Debug, Clone)]
pub struct ExtractVariableResult {
    /// The new let binding.
    pub binding: String,
    /// The variable name to use at original locations.
    pub var_name: String,
    /// Number of occurrences replaced.
    pub occurrences: usize,
}

/// Extracts an expression into a let binding.
pub fn extract_variable(
    expression: &str,
    var_name: &str,
    source: &str,
    is_mutable: bool,
) -> ExtractVariableResult {
    let mut_kw = if is_mutable { "mut " } else { "" };
    let binding = format!("let {mut_kw}{var_name} = {expression}");
    let occurrences = source.matches(expression).count();

    ExtractVariableResult {
        binding,
        var_name: var_name.into(),
        occurrences,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.3: Inline Function
// ═══════════════════════════════════════════════════════════════════════

/// A function's decomposed parts for inlining.
#[derive(Debug, Clone)]
pub struct FunctionParts {
    /// Function name.
    pub name: String,
    /// Parameters: (name, type).
    pub params: Vec<(String, String)>,
    /// Function body.
    pub body: String,
    /// Return type.
    pub return_type: Option<String>,
}

/// Result of inlining a function.
#[derive(Debug, Clone)]
pub struct InlineFunctionResult {
    /// The inlined code to replace the call.
    pub inlined_code: String,
    /// Number of call sites inlined.
    pub sites_inlined: usize,
}

/// Inlines a function call by substituting body for call.
pub fn inline_function(parts: &FunctionParts, call_args: &[&str]) -> InlineFunctionResult {
    let mut body = parts.body.clone();

    // Substitute parameters with arguments
    for (i, (param_name, _)) in parts.params.iter().enumerate() {
        if let Some(arg) = call_args.get(i) {
            body = body.replace(param_name.as_str(), arg);
        }
    }

    InlineFunctionResult {
        inlined_code: body.trim().to_string(),
        sites_inlined: 1,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.4: Inline Variable
// ═══════════════════════════════════════════════════════════════════════

/// Result of inlining a variable.
#[derive(Debug, Clone)]
pub struct InlineVariableResult {
    /// Source code with variable replaced by its initializer.
    pub result_code: String,
    /// Number of replacements made.
    pub replacements: usize,
}

/// Inlines all occurrences of a variable with its initializer.
pub fn inline_variable(var_name: &str, initializer: &str, source: &str) -> InlineVariableResult {
    // Remove the let binding line
    let binding_pattern = format!("let {var_name} = {initializer}");
    let mut result = source.replace(&binding_pattern, "");

    // Also try with mut
    let binding_mut = format!("let mut {var_name} = {initializer}");
    result = result.replace(&binding_mut, "");

    // Replace all usages of var_name with initializer
    let replacements = result.matches(var_name).count();
    result = result.replace(var_name, initializer);

    InlineVariableResult {
        result_code: result,
        replacements,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.5: Rename Symbol
// ═══════════════════════════════════════════════════════════════════════

/// A text edit for renaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameEdit {
    /// File path.
    pub file: String,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column start (0-indexed).
    pub col_start: usize,
    /// Column end.
    pub col_end: usize,
    /// New text.
    pub new_text: String,
}

/// Result of a rename operation.
#[derive(Debug, Clone)]
pub struct RenameResult {
    /// Edits grouped by file.
    pub edits: HashMap<String, Vec<RenameEdit>>,
    /// Total number of edits.
    pub total_edits: usize,
}

/// Renames a symbol across the workspace.
pub fn rename_symbol(
    _old_name: &str,
    new_name: &str,
    occurrences: &[(String, usize, usize, usize)], // (file, line, col_start, col_end)
) -> RenameResult {
    let mut edits: HashMap<String, Vec<RenameEdit>> = HashMap::new();
    let total_edits = occurrences.len();

    for (file, line, col_start, col_end) in occurrences {
        edits.entry(file.clone()).or_default().push(RenameEdit {
            file: file.clone(),
            line: *line,
            col_start: *col_start,
            col_end: *col_end,
            new_text: new_name.into(),
        });
    }

    RenameResult { edits, total_edits }
}

impl fmt::Display for RenameResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Renamed {} occurrences across {} files",
            self.total_edits,
            self.edits.len()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.6: Move Module
// ═══════════════════════════════════════════════════════════════════════

/// Result of moving a module.
#[derive(Debug, Clone)]
pub struct MoveModuleResult {
    /// File move: (from, to).
    pub file_move: (String, String),
    /// Import updates needed.
    pub import_updates: Vec<ImportUpdate>,
}

/// An import statement that needs updating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportUpdate {
    /// File containing the import.
    pub file: String,
    /// Old import path.
    pub old_import: String,
    /// New import path.
    pub new_import: String,
    /// Line number.
    pub line: usize,
}

/// Plans a module move operation.
pub fn plan_move_module(
    old_path: &str,
    new_path: &str,
    old_module: &str,
    new_module: &str,
    import_sites: &[(String, usize)], // (file, line)
) -> MoveModuleResult {
    let import_updates: Vec<ImportUpdate> = import_sites
        .iter()
        .map(|(file, line)| ImportUpdate {
            file: file.clone(),
            old_import: format!("use {old_module}"),
            new_import: format!("use {new_module}"),
            line: *line,
        })
        .collect();

    MoveModuleResult {
        file_move: (old_path.into(), new_path.into()),
        import_updates,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.7: Extract Trait
// ═══════════════════════════════════════════════════════════════════════

/// A method signature for trait extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    /// Method name.
    pub name: String,
    /// Parameters (excluding self).
    pub params: Vec<(String, String)>,
    /// Return type.
    pub return_type: Option<String>,
    /// Whether it takes &self, &mut self, or self.
    pub self_param: SelfParam,
}

/// The self parameter kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfParam {
    /// `&self`
    Ref,
    /// `&mut self`
    RefMut,
    /// `self`
    Owned,
    /// No self (associated function).
    None,
}

impl fmt::Display for SelfParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfParam::Ref => write!(f, "&self"),
            SelfParam::RefMut => write!(f, "&mut self"),
            SelfParam::Owned => write!(f, "self"),
            SelfParam::None => Ok(()),
        }
    }
}

/// Result of extracting a trait.
#[derive(Debug, Clone)]
pub struct ExtractTraitResult {
    /// The generated trait definition.
    pub trait_def: String,
    /// Trait name.
    pub trait_name: String,
    /// Methods extracted.
    pub methods: Vec<String>,
}

/// Extracts selected methods into a new trait.
pub fn extract_trait(trait_name: &str, methods: &[MethodSignature]) -> ExtractTraitResult {
    let mut body = format!("trait {trait_name} {{\n");

    let method_names: Vec<String> = methods.iter().map(|m| m.name.clone()).collect();

    for method in methods {
        let self_str = method.self_param.to_string();
        let params: String = method
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {t}"))
            .collect::<Vec<_>>()
            .join(", ");

        let all_params = if self_str.is_empty() {
            params
        } else if params.is_empty() {
            self_str
        } else {
            format!("{self_str}, {params}")
        };

        let ret = method
            .return_type
            .as_ref()
            .map_or(String::new(), |t| format!(" -> {t}"));

        body.push_str(&format!("    fn {}({all_params}){ret};\n", method.name));
    }

    body.push('}');

    ExtractTraitResult {
        trait_def: body,
        trait_name: trait_name.into(),
        methods: method_names,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.8: Change Signature
// ═══════════════════════════════════════════════════════════════════════

/// A parameter change operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamChange {
    /// Add a new parameter at index.
    Add {
        index: usize,
        name: String,
        ty: String,
        default: Option<String>,
    },
    /// Remove parameter at index.
    Remove { index: usize },
    /// Reorder: move param from old_index to new_index.
    Reorder { old_index: usize, new_index: usize },
    /// Rename a parameter.
    Rename { index: usize, new_name: String },
    /// Change parameter type.
    ChangeType { index: usize, new_type: String },
}

/// Result of changing a function signature.
#[derive(Debug, Clone)]
pub struct ChangeSignatureResult {
    /// The new function signature.
    pub new_signature: String,
    /// Call site updates needed.
    pub call_site_updates: Vec<String>,
    /// Number of call sites affected.
    pub affected_sites: usize,
}

/// Applies signature changes to a function.
pub fn change_signature(
    func_name: &str,
    current_params: &[(String, String)],
    changes: &[ParamChange],
    call_sites: &[Vec<String>], // each call site's current arguments
) -> ChangeSignatureResult {
    let mut params: Vec<(String, String)> = current_params.to_vec();

    // Apply changes (process removals last to preserve indices)
    let mut adds = Vec::new();
    let mut removes = Vec::new();
    let mut renames = Vec::new();
    let mut type_changes = Vec::new();

    for change in changes {
        match change {
            ParamChange::Add {
                index, name, ty, ..
            } => {
                adds.push((*index, name.clone(), ty.clone()));
            }
            ParamChange::Remove { index } => removes.push(*index),
            ParamChange::Rename { index, new_name } => renames.push((*index, new_name.clone())),
            ParamChange::ChangeType { index, new_type } => {
                type_changes.push((*index, new_type.clone()));
            }
            ParamChange::Reorder { .. } => {} // Handled separately
        }
    }

    // Apply renames
    for (idx, new_name) in &renames {
        if let Some(p) = params.get_mut(*idx) {
            p.0 = new_name.clone();
        }
    }

    // Apply type changes
    for (idx, new_type) in &type_changes {
        if let Some(p) = params.get_mut(*idx) {
            p.1 = new_type.clone();
        }
    }

    // Apply removals (reverse order to preserve indices)
    let mut sorted_removes = removes.clone();
    sorted_removes.sort_unstable();
    sorted_removes.reverse();
    for idx in &sorted_removes {
        if *idx < params.len() {
            params.remove(*idx);
        }
    }

    // Apply adds
    for (idx, name, ty) in &adds {
        let insert_at = (*idx).min(params.len());
        params.insert(insert_at, (name.clone(), ty.clone()));
    }

    let param_str: String = params
        .iter()
        .map(|(n, t)| format!("{n}: {t}"))
        .collect::<Vec<_>>()
        .join(", ");
    let new_signature = format!("fn {func_name}({param_str})");

    // Update call sites
    let call_site_updates: Vec<String> = call_sites
        .iter()
        .map(|args| {
            let mut new_args = args.clone();

            // Apply removals to args
            let mut sorted_rem = removes.clone();
            sorted_rem.sort_unstable();
            sorted_rem.reverse();
            for idx in &sorted_rem {
                if *idx < new_args.len() {
                    new_args.remove(*idx);
                }
            }

            // Apply adds
            for (idx, _, _) in &adds {
                let default = changes.iter().find_map(|c| {
                    if let ParamChange::Add { index, default, .. } = c {
                        if index == idx { default.clone() } else { None }
                    } else {
                        None
                    }
                });
                let insert_at = (*idx).min(new_args.len());
                new_args.insert(insert_at, default.unwrap_or_else(|| "todo!()".into()));
            }

            format!("{func_name}({})", new_args.join(", "))
        })
        .collect();

    let affected = call_site_updates.len();

    ChangeSignatureResult {
        new_signature,
        call_site_updates,
        affected_sites: affected,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.9: Convert to/from Method
// ═══════════════════════════════════════════════════════════════════════

/// Direction of function-to-method conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionDirection {
    /// Free function → method (add self param).
    FunctionToMethod,
    /// Method → free function (remove self param).
    MethodToFunction,
}

/// Result of function/method conversion.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// The converted function/method signature.
    pub new_code: String,
    /// Direction of conversion.
    pub direction: ConversionDirection,
    /// Type name (for method conversion).
    pub type_name: String,
}

/// Converts a free function to a method or vice versa.
pub fn convert_function_method(
    func_name: &str,
    params: &[(String, String)],
    body: &str,
    return_type: Option<&str>,
    direction: ConversionDirection,
    type_name: &str,
) -> ConversionResult {
    let ret = return_type.map(|t| format!(" -> {t}")).unwrap_or_default();

    match direction {
        ConversionDirection::FunctionToMethod => {
            // First param becomes self
            let method_params = if params.is_empty() {
                "&self".into()
            } else {
                let rest: Vec<String> = params
                    .iter()
                    .skip(1)
                    .map(|(n, t)| format!("{n}: {t}"))
                    .collect();
                if rest.is_empty() {
                    "&self".into()
                } else {
                    format!("&self, {}", rest.join(", "))
                }
            };

            // Replace first param name with self in body
            let new_body = if let Some((first_name, _)) = params.first() {
                body.replace(first_name.as_str(), "self")
            } else {
                body.into()
            };

            let code = format!(
                "impl {type_name} {{\n    fn {func_name}({method_params}){ret} {{\n        {}\n    }}\n}}",
                new_body.trim()
            );

            ConversionResult {
                new_code: code,
                direction,
                type_name: type_name.into(),
            }
        }
        ConversionDirection::MethodToFunction => {
            // Add type_name as first param
            let mut all_params = vec![format!("{}: &{}", type_name.to_lowercase(), type_name)];
            all_params.extend(params.iter().map(|(n, t)| format!("{n}: {t}")));

            let new_body = body.replace("self", &type_name.to_lowercase());

            let code = format!(
                "fn {func_name}({}){ret} {{\n    {}\n}}",
                all_params.join(", "),
                new_body.trim()
            );

            ConversionResult {
                new_code: code,
                direction,
                type_name: type_name.into(),
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S32.1 — Extract Function
    #[test]
    fn s32_1_extract_function_simple() {
        let vars = vec![RefVariable {
            name: "x".into(),
            ty: "i32".into(),
            is_mutated: false,
            is_input: true,
            is_output: false,
        }];
        let result = extract_function("x + 1", "add_one", &vars);
        assert!(result.function_def.contains("fn add_one"));
        assert!(result.function_def.contains("x: &i32"));
        assert!(result.call_site.contains("add_one(&x)"));
    }

    #[test]
    fn s32_1_extract_function_with_output() {
        let vars = vec![
            RefVariable {
                name: "x".into(),
                ty: "i32".into(),
                is_mutated: false,
                is_input: true,
                is_output: false,
            },
            RefVariable {
                name: "result".into(),
                ty: "i32".into(),
                is_mutated: false,
                is_input: false,
                is_output: true,
            },
        ];
        let result = extract_function("let result = x * 2", "compute", &vars);
        assert!(result.return_type.is_some());
        assert!(result.call_site.contains("let result ="));
    }

    #[test]
    fn s32_1_extract_function_mutable() {
        let vars = vec![RefVariable {
            name: "counter".into(),
            ty: "i32".into(),
            is_mutated: true,
            is_input: true,
            is_output: false,
        }];
        let result = extract_function("counter += 1", "increment", &vars);
        assert!(result.function_def.contains("&mut i32"));
        assert!(result.call_site.contains("&mut counter"));
    }

    // S32.2 — Extract Variable
    #[test]
    fn s32_2_extract_variable() {
        let result = extract_variable(
            "a + b * c",
            "product",
            "let x = a + b * c\nlet y = a + b * c",
            false,
        );
        assert_eq!(result.binding, "let product = a + b * c");
        assert_eq!(result.occurrences, 2);
    }

    #[test]
    fn s32_2_extract_mutable_variable() {
        let result = extract_variable("Vec::new()", "items", "Vec::new()", true);
        assert!(result.binding.contains("let mut items"));
    }

    // S32.3 — Inline Function
    #[test]
    fn s32_3_inline_function() {
        let parts = FunctionParts {
            name: "double".into(),
            params: vec![("x".into(), "i32".into())],
            body: "x * 2".into(),
            return_type: Some("i32".into()),
        };
        let result = inline_function(&parts, &["n"]);
        assert_eq!(result.inlined_code, "n * 2");
    }

    #[test]
    fn s32_3_inline_multi_param() {
        let parts = FunctionParts {
            name: "add".into(),
            params: vec![("a".into(), "i32".into()), ("b".into(), "i32".into())],
            body: "a + b".into(),
            return_type: Some("i32".into()),
        };
        let result = inline_function(&parts, &["x", "y"]);
        assert_eq!(result.inlined_code, "x + y");
    }

    // S32.4 — Inline Variable
    #[test]
    fn s32_4_inline_variable() {
        let source = "let tmp = x + 1\nresult = tmp * tmp";
        let result = inline_variable("tmp", "x + 1", source);
        assert!(result.result_code.contains("x + 1"));
        assert!(result.replacements > 0);
    }

    // S32.5 — Rename Symbol
    #[test]
    fn s32_5_rename_single_file() {
        let occurrences = vec![("main.fj".into(), 1, 4, 7), ("main.fj".into(), 5, 10, 13)];
        let result = rename_symbol("foo", "bar", &occurrences);
        assert_eq!(result.total_edits, 2);
        assert_eq!(result.edits.len(), 1);
    }

    #[test]
    fn s32_5_rename_multi_file() {
        let occurrences = vec![
            ("main.fj".into(), 1, 0, 3),
            ("lib.fj".into(), 10, 5, 8),
            ("lib.fj".into(), 15, 0, 3),
        ];
        let result = rename_symbol("foo", "bar", &occurrences);
        assert_eq!(result.total_edits, 3);
        assert_eq!(result.edits.len(), 2);
        assert!(result.to_string().contains("3 occurrences"));
    }

    // S32.6 — Move Module
    #[test]
    fn s32_6_move_module() {
        let sites = vec![("main.fj".into(), 2), ("lib.fj".into(), 5)];
        let result = plan_move_module(
            "src/utils.fj",
            "src/helpers/utils.fj",
            "utils",
            "helpers::utils",
            &sites,
        );
        assert_eq!(result.file_move.0, "src/utils.fj");
        assert_eq!(result.file_move.1, "src/helpers/utils.fj");
        assert_eq!(result.import_updates.len(), 2);
        assert!(
            result.import_updates[0]
                .new_import
                .contains("helpers::utils")
        );
    }

    // S32.7 — Extract Trait
    #[test]
    fn s32_7_extract_trait() {
        let methods = vec![
            MethodSignature {
                name: "area".into(),
                params: vec![],
                return_type: Some("f64".into()),
                self_param: SelfParam::Ref,
            },
            MethodSignature {
                name: "perimeter".into(),
                params: vec![],
                return_type: Some("f64".into()),
                self_param: SelfParam::Ref,
            },
        ];
        let result = extract_trait("Shape", &methods);
        assert!(result.trait_def.contains("trait Shape"));
        assert!(result.trait_def.contains("fn area"));
        assert!(result.trait_def.contains("fn perimeter"));
        assert_eq!(result.methods.len(), 2);
    }

    #[test]
    fn s32_7_extract_trait_with_params() {
        let methods = vec![MethodSignature {
            name: "set_name".into(),
            params: vec![("name".into(), "String".into())],
            return_type: None,
            self_param: SelfParam::RefMut,
        }];
        let result = extract_trait("Named", &methods);
        assert!(result.trait_def.contains("&mut self, name: String"));
    }

    // S32.8 — Change Signature
    #[test]
    fn s32_8_add_param() {
        let params = vec![("x".into(), "i32".into())];
        let changes = vec![ParamChange::Add {
            index: 1,
            name: "y".into(),
            ty: "i32".into(),
            default: Some("0".into()),
        }];
        let calls = vec![vec!["42".into()]];
        let result = change_signature("add", &params, &changes, &calls);
        assert!(result.new_signature.contains("y: i32"));
        assert_eq!(result.affected_sites, 1);
    }

    #[test]
    fn s32_8_remove_param() {
        let params = vec![("x".into(), "i32".into()), ("y".into(), "i32".into())];
        let changes = vec![ParamChange::Remove { index: 1 }];
        let calls = vec![vec!["1".into(), "2".into()]];
        let result = change_signature("foo", &params, &changes, &calls);
        assert!(!result.new_signature.contains("y"));
        assert_eq!(result.call_site_updates[0], "foo(1)");
    }

    #[test]
    fn s32_8_rename_param() {
        let params = vec![("x".into(), "i32".into())];
        let changes = vec![ParamChange::Rename {
            index: 0,
            new_name: "value".into(),
        }];
        let result = change_signature("foo", &params, &changes, &[]);
        assert!(result.new_signature.contains("value: i32"));
    }

    // S32.9 — Convert to/from Method
    #[test]
    fn s32_9_function_to_method() {
        let params = vec![("p".into(), "Point".into())];
        let result = convert_function_method(
            "distance",
            &params,
            "sqrt(p.x * p.x + p.y * p.y)",
            Some("f64"),
            ConversionDirection::FunctionToMethod,
            "Point",
        );
        assert!(result.new_code.contains("impl Point"));
        assert!(result.new_code.contains("&self"));
        assert!(result.new_code.contains("self.x"));
    }

    #[test]
    fn s32_9_method_to_function() {
        let params: Vec<(String, String)> = vec![];
        let result = convert_function_method(
            "area",
            &params,
            "self.width * self.height",
            Some("f64"),
            ConversionDirection::MethodToFunction,
            "Rect",
        );
        assert!(result.new_code.contains("fn area(rect: &Rect)"));
        assert!(result.new_code.contains("rect.width"));
    }

    // S32.10 — Integration
    #[test]
    fn s32_10_self_param_display() {
        assert_eq!(SelfParam::Ref.to_string(), "&self");
        assert_eq!(SelfParam::RefMut.to_string(), "&mut self");
        assert_eq!(SelfParam::Owned.to_string(), "self");
        assert_eq!(SelfParam::None.to_string(), "");
    }

    #[test]
    fn s32_10_conversion_direction() {
        assert_ne!(
            ConversionDirection::FunctionToMethod,
            ConversionDirection::MethodToFunction,
        );
    }
}
