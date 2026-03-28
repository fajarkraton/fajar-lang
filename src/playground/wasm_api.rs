//! Wasm-bindgen entry points for the browser playground.
//!
//! These functions are the public API exported to JavaScript.
//! Only available when compiled with `--features playground-wasm`.
//!
//! # Exports
//!
//! - `eval_source(code)` — lex + parse + analyze + interpret, returns output
//! - `tokenize_source(code)` — lex only, returns JSON token list
//! - `format_source(code)` — lex + parse + format, returns formatted source
//! - `check_source(code)` — lex + parse + analyze, returns error JSON or "OK"

#[cfg(feature = "playground-wasm")]
use wasm_bindgen::prelude::*;

/// Evaluates Fajar Lang source code and returns the captured output.
///
/// Runs the full pipeline: lex → parse → analyze → interpret.
/// Print statements are captured and returned as the result string.
/// Errors are returned as formatted error messages.
#[cfg(feature = "playground-wasm")]
#[wasm_bindgen]
pub fn eval_source(code: &str) -> String {
    let mut interp = crate::interpreter::Interpreter::new_capturing();
    match interp.eval_source(code) {
        Ok(val) => {
            let mut output = interp.get_output().join("\n");
            let val_str = format!("{val}");
            if val_str != "null" && !val_str.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&val_str);
            }
            output
        }
        Err(e) => format!("Error: {e}"),
    }
}

/// Tokenizes source code and returns a JSON array of tokens.
///
/// Each token is `{"kind": "...", "text": "...", "line": N, "col": N}`.
#[cfg(feature = "playground-wasm")]
#[wasm_bindgen]
pub fn tokenize_source(code: &str) -> String {
    match crate::lexer::tokenize(code) {
        Ok(tokens) => {
            let mut json = String::from("[");
            for (i, tok) in tokens.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&format!(
                    "{{\"kind\":\"{:?}\",\"line\":{},\"col\":{}}}",
                    tok.kind, tok.line, tok.col
                ));
            }
            json.push(']');
            json
        }
        Err(errors) => {
            let msgs: Vec<String> = errors.iter().map(|e| format!("{e}")).collect();
            format!(
                "{{\"errors\":[{}]}}",
                msgs.iter()
                    .map(|m| format!("\"{m}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

/// Formats source code and returns the formatted result.
///
/// Returns the original source unchanged if parsing fails.
#[cfg(feature = "playground-wasm")]
#[wasm_bindgen]
pub fn format_source(code: &str) -> String {
    let tokens = match crate::lexer::tokenize(code) {
        Ok(t) => t,
        Err(_) => return code.to_string(),
    };
    let program = match crate::parser::parse(tokens) {
        Ok(p) => p,
        Err(_) => return code.to_string(),
    };
    let mut formatter = crate::formatter::PrettyPrinter::new();
    formatter.format_program(&program);
    formatter.finish()
}

/// Type-checks source code and returns "OK" or error messages.
#[cfg(feature = "playground-wasm")]
#[wasm_bindgen]
pub fn check_source(code: &str) -> String {
    let tokens = match crate::lexer::tokenize(code) {
        Ok(t) => t,
        Err(errors) => {
            return errors
                .iter()
                .map(|e| format!("{e}"))
                .collect::<Vec<_>>()
                .join("\n");
        }
    };
    let program = match crate::parser::parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            return errors
                .iter()
                .map(|e| format!("{e}"))
                .collect::<Vec<_>>()
                .join("\n");
        }
    };
    match crate::analyzer::analyze(&program) {
        Ok(()) => "OK".to_string(),
        Err(errors) => errors
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

// When playground-wasm feature is not enabled, provide non-wasm versions
// that can be used for testing the logic without wasm-bindgen.

/// Non-wasm version of eval_source for testing.
#[cfg(not(feature = "playground-wasm"))]
pub fn eval_source(code: &str) -> String {
    let mut interp = crate::interpreter::Interpreter::new_capturing();
    match interp.eval_source(code) {
        Ok(val) => {
            let mut output = interp.get_output().join("\n");
            let val_str = format!("{val}");
            if val_str != "null" && !val_str.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&val_str);
            }
            output
        }
        Err(e) => format!("Error: {e}"),
    }
}

/// Non-wasm version of check_source for testing.
#[cfg(not(feature = "playground-wasm"))]
pub fn check_source(code: &str) -> String {
    let tokens = match crate::lexer::tokenize(code) {
        Ok(t) => t,
        Err(errors) => {
            return errors
                .iter()
                .map(|e| format!("{e}"))
                .collect::<Vec<_>>()
                .join("\n");
        }
    };
    let program = match crate::parser::parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            return errors
                .iter()
                .map(|e| format!("{e}"))
                .collect::<Vec<_>>()
                .join("\n");
        }
    };
    match crate::analyzer::analyze(&program) {
        Ok(()) => "OK".to_string(),
        Err(errors) => errors
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playground_eval_simple() {
        let result = eval_source("1 + 2");
        assert_eq!(result, "3");
    }

    #[test]
    fn playground_eval_print() {
        let result = eval_source("println(\"hello\")");
        assert_eq!(result, "hello");
    }

    #[test]
    fn playground_eval_error() {
        let result = eval_source("let x: i64 = \"bad\"");
        assert!(
            result.starts_with("Error:"),
            "expected error, got: {result}"
        );
    }

    #[test]
    fn playground_check_ok() {
        let result = check_source("let x: i64 = 42");
        assert_eq!(result, "OK");
    }

    #[test]
    fn playground_check_error() {
        let result = check_source("let x: i64 = true");
        assert!(
            result.contains("SE004") || result.contains("type mismatch"),
            "expected type error, got: {result}"
        );
    }
}
