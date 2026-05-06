//! Macro system for Fajar Lang.
//!
//! Provides built-in macros, derive macro expansion, and a framework
//! for user-defined `macro_rules!` declarations.
//!
//! # Built-in Macros
//!
//! | Macro | Description |
//! |-------|-------------|
//! | `vec![a, b, c]` | Creates an array from elements |
//! | `stringify!(expr)` | Converts expression to string literal |
//! | `concat!(a, b, c)` | Concatenates string literals |
//! | `dbg!(expr)` | Debug-prints expression and returns it |
//! | `todo!()` | Panics with "not yet implemented" |
//! | `env!("VAR")` | Reads compile-time environment variable |
//!
//! # Derive Macros
//!
//! `@derive(Debug, Clone, PartialEq)` on structs/enums generates
//! trait implementations automatically.

use crate::interpreter::Value;
use std::collections::HashMap;

/// A registered macro (built-in or user-defined).
#[derive(Debug, Clone)]
pub enum MacroDef {
    /// Built-in macro with native implementation.
    Builtin {
        /// Macro name.
        name: String,
        /// Description for documentation.
        description: String,
    },
    /// User-defined macro_rules! macro.
    UserDefined {
        /// Macro name.
        name: String,
        /// Pattern → template arms (raw strings for now).
        arms: Vec<(String, String)>,
    },
}

/// Macro registry — stores all available macros.
#[derive(Debug, Clone)]
pub struct MacroRegistry {
    /// Registered macros by name.
    macros: HashMap<String, MacroDef>,
}

impl MacroRegistry {
    /// Creates a new registry with built-in macros pre-registered.
    pub fn new() -> Self {
        let mut reg = Self {
            macros: HashMap::new(),
        };
        reg.register_builtins();
        reg
    }

    fn register_builtins(&mut self) {
        let builtins = [
            ("vec", "Creates an array from comma-separated elements"),
            ("stringify", "Converts an expression to a string literal"),
            ("concat", "Concatenates string values"),
            ("dbg", "Debug-prints an expression and returns its value"),
            ("todo", "Panics with 'not yet implemented'"),
            ("env", "Reads a compile-time environment variable"),
            ("include", "Includes contents of a file as a string"),
            ("cfg", "Conditional compilation based on configuration"),
            ("line", "Expands to the current line number"),
            ("file", "Expands to the current file name"),
            ("column", "Expands to the current column number"),
        ];
        for (name, desc) in builtins {
            self.macros.insert(
                name.to_string(),
                MacroDef::Builtin {
                    name: name.to_string(),
                    description: desc.to_string(),
                },
            );
        }
    }

    /// Registers a user-defined macro.
    pub fn register(&mut self, name: String, def: MacroDef) {
        self.macros.insert(name, def);
    }

    /// Checks if a macro exists.
    pub fn contains(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// Looks up a macro by name.
    pub fn lookup(&self, name: &str) -> Option<&MacroDef> {
        self.macros.get(name)
    }

    /// Returns all registered macro names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.macros.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for MacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluates a built-in macro invocation.
///
/// Returns the expanded value, or an error message.
pub fn eval_builtin_macro(name: &str, args: &[Value]) -> Result<Value, String> {
    match name {
        "vec" => {
            // vec![a, b, c] → Array([a, b, c])
            Ok(Value::Array(std::sync::Arc::new(args.to_vec())))
        }
        "stringify" => {
            // stringify!(expr) → string representation
            if args.is_empty() {
                Ok(Value::Str(String::new()))
            } else {
                let s = args
                    .iter()
                    .map(|a| format!("{a}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(Value::Str(s))
            }
        }
        "concat" => {
            // concat!(a, b, c) → concatenated string
            let s = args.iter().map(|a| format!("{a}")).collect::<String>();
            Ok(Value::Str(s))
        }
        "dbg" => {
            // dbg!(expr) → prints and returns value
            if let Some(val) = args.first() {
                eprintln!("[dbg] {val}");
                Ok(val.clone())
            } else {
                Ok(Value::Null)
            }
        }
        "todo" => {
            // todo!() → panic with message
            let msg = if let Some(Value::Str(s)) = args.first() {
                format!("not yet implemented: {s}")
            } else {
                "not yet implemented".to_string()
            };
            Err(msg)
        }
        "env" => {
            // env!("VAR") → compile-time env variable
            if let Some(Value::Str(var_name)) = args.first() {
                match std::env::var(var_name) {
                    Ok(val) => Ok(Value::Str(val)),
                    Err(_) => Err(format!("environment variable '{var_name}' not found")),
                }
            } else {
                Err("env! requires a string argument".to_string())
            }
        }
        // V12 Gap Closure: format!, matches!, println!, assert_eq!, cfg!
        "format" => {
            // format!("template {}", value) → formatted string
            if args.is_empty() {
                return Ok(Value::Str(String::new()));
            }
            let template = format!("{}", args[0]);
            let mut result = template.clone();
            for arg in args.iter().skip(1) {
                if let Some(pos) = result.find("{}") {
                    result.replace_range(pos..pos + 2, &format!("{arg}"));
                }
            }
            Ok(Value::Str(result))
        }
        "println" => {
            // println!("template {}", value) → print + newline
            if args.is_empty() {
                println!();
                return Ok(Value::Null);
            }
            let template = format!("{}", args[0]);
            let mut result = template.clone();
            for arg in args.iter().skip(1) {
                if let Some(pos) = result.find("{}") {
                    result.replace_range(pos..pos + 2, &format!("{arg}"));
                }
            }
            println!("{result}");
            Ok(Value::Null)
        }
        "matches" => {
            // matches!(expr, pattern) → bool
            if args.len() < 2 {
                return Err("matches! requires 2 arguments".to_string());
            }
            Ok(Value::Bool(args[0] == args[1]))
        }
        "assert_eq" => {
            // assert_eq!(left, right) → panics if not equal
            if args.len() < 2 {
                return Err("assert_eq! requires 2 arguments".to_string());
            }
            if args[0] == args[1] {
                Ok(Value::Null)
            } else {
                Err(format!(
                    "assertion failed: `left == right`\n  left: {}\n right: {}",
                    args[0], args[1]
                ))
            }
        }
        "assert" => {
            // assert!(condition) → panics if false
            if let Some(Value::Bool(true)) = args.first() {
                Ok(Value::Null)
            } else {
                let msg = args
                    .get(1)
                    .map_or("assertion failed".to_string(), |v| format!("{v}"));
                Err(msg)
            }
        }
        "cfg" => {
            // cfg!(feature = "name") → bool (simplified: always true for "std")
            if let Some(Value::Str(s)) = args.first() {
                let enabled = s.contains("std") || s.contains("default");
                Ok(Value::Bool(enabled))
            } else {
                Ok(Value::Bool(false))
            }
        }
        "include_str" => {
            // include_str!("file.txt") → file contents as string
            if let Some(Value::Str(path)) = args.first() {
                match std::fs::read_to_string(path) {
                    Ok(contents) => Ok(Value::Str(contents)),
                    Err(e) => Err(format!("include_str!: cannot read '{path}': {e}")),
                }
            } else {
                Err("include_str! requires a string path argument".to_string())
            }
        }
        "line" => Ok(Value::Int(0)), // placeholder — would need source location
        "file" => Ok(Value::Str("unknown".to_string())),
        "column" => Ok(Value::Int(0)),
        _ => Err(format!("unknown built-in macro: {name}!")),
    }
}

/// Derive trait names that are supported.
pub const SUPPORTED_DERIVES: &[&str] = &["Debug", "Clone", "PartialEq", "Default", "Hash"];

/// Checks if a derive trait is supported.
pub fn is_supported_derive(name: &str) -> bool {
    SUPPORTED_DERIVES.contains(&name)
}

/// Generates a derive implementation description (for documentation/analysis).
///
/// In a real compiler this would generate AST nodes for the impl block.
/// For now we return a description of what would be generated.
pub fn describe_derive(trait_name: &str, struct_name: &str, fields: &[String]) -> String {
    match trait_name {
        "Debug" => {
            let field_fmts: Vec<String> = fields.iter().map(|f| format!("{f}: {{:?}}")).collect();
            format!(
                "impl Debug for {struct_name} {{ fn fmt(&self) -> str {{ \"{struct_name} {{ {} }}\" }} }}",
                field_fmts.join(", ")
            )
        }
        "Clone" => {
            let field_clones: Vec<String> = fields
                .iter()
                .map(|f| format!("{f}: self.{f}.clone()"))
                .collect();
            format!(
                "impl Clone for {struct_name} {{ fn clone(&self) -> {struct_name} {{ {struct_name} {{ {} }} }} }}",
                field_clones.join(", ")
            )
        }
        "PartialEq" => {
            let field_eqs: Vec<String> = fields
                .iter()
                .map(|f| format!("self.{f} == other.{f}"))
                .collect();
            let eq_expr = if field_eqs.is_empty() {
                "true".to_string()
            } else {
                field_eqs.join(" && ")
            };
            format!(
                "impl PartialEq for {struct_name} {{ fn eq(&self, other: &{struct_name}) -> bool {{ {eq_expr} }} }}"
            )
        }
        "Default" => {
            format!(
                "impl Default for {struct_name} {{ fn default() -> {struct_name} {{ {struct_name} {{ /* default fields */ }} }} }}"
            )
        }
        "Hash" => {
            format!(
                "impl Hash for {struct_name} {{ fn hash(&self) -> i64 {{ /* hash fields */ 0 }} }}"
            )
        }
        _ => format!("// unsupported derive: {trait_name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_builtins() {
        let reg = MacroRegistry::new();
        assert!(reg.contains("vec"));
        assert!(reg.contains("stringify"));
        assert!(reg.contains("concat"));
        assert!(reg.contains("dbg"));
        assert!(reg.contains("todo"));
        assert!(reg.contains("env"));
        assert!(!reg.contains("nonexistent"));
    }

    #[test]
    fn registry_names() {
        let reg = MacroRegistry::new();
        let names = reg.names();
        assert!(names.contains(&"vec".to_string()));
        assert!(names.contains(&"stringify".to_string()));
    }

    #[test]
    fn registry_register_custom() {
        let mut reg = MacroRegistry::new();
        reg.register(
            "my_macro".to_string(),
            MacroDef::UserDefined {
                name: "my_macro".to_string(),
                arms: vec![("()".to_string(), "42".to_string())],
            },
        );
        assert!(reg.contains("my_macro"));
    }

    #[test]
    fn eval_vec_macro() {
        let result = eval_builtin_macro("vec", &[Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(
            result.unwrap(),
            Value::array_from_vec(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }

    #[test]
    fn eval_vec_empty() {
        let result = eval_builtin_macro("vec", &[]);
        assert_eq!(result.unwrap(), Value::array_from_vec(vec![]));
    }

    #[test]
    fn eval_stringify() {
        let result = eval_builtin_macro("stringify", &[Value::Int(42)]);
        assert_eq!(result.unwrap(), Value::Str("42".to_string()));
    }

    #[test]
    fn eval_concat() {
        let result = eval_builtin_macro(
            "concat",
            &[
                Value::Str("hello".into()),
                Value::Str(" ".into()),
                Value::Str("world".into()),
            ],
        );
        assert_eq!(result.unwrap(), Value::Str("hello world".to_string()));
    }

    #[test]
    fn eval_todo_panics() {
        let result = eval_builtin_macro("todo", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet implemented"));
    }

    #[test]
    fn eval_dbg_returns_value() {
        let result = eval_builtin_macro("dbg", &[Value::Int(42)]);
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn derive_debug_description() {
        let desc = describe_derive("Debug", "Point", &["x".into(), "y".into()]);
        assert!(desc.contains("impl Debug for Point"));
        assert!(desc.contains("x:"));
        assert!(desc.contains("y:"));
    }

    #[test]
    fn derive_clone_description() {
        let desc = describe_derive("Clone", "Point", &["x".into(), "y".into()]);
        assert!(desc.contains("impl Clone for Point"));
        assert!(desc.contains("self.x.clone()"));
    }

    #[test]
    fn derive_partial_eq_description() {
        let desc = describe_derive("PartialEq", "Point", &["x".into(), "y".into()]);
        assert!(desc.contains("impl PartialEq for Point"));
        assert!(desc.contains("self.x == other.x"));
    }

    #[test]
    fn derive_empty_struct() {
        let desc = describe_derive("PartialEq", "Unit", &[]);
        assert!(desc.contains("true"));
    }

    #[test]
    fn supported_derives() {
        assert!(is_supported_derive("Debug"));
        assert!(is_supported_derive("Clone"));
        assert!(is_supported_derive("PartialEq"));
        assert!(!is_supported_derive("Serialize"));
    }
}
