//! Self-Hosted Standard Library — core types (Option, Result, Array, HashMap, String)
//! implemented in simulated Fajar, math builtins, IO builtins, string operations,
//! iterator protocol, error handling helpers.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S6.1: Core Types — Option<T>
// ═══════════════════════════════════════════════════════════════════════

/// Simulated Fajar Lang value for the self-hosted stdlib.
#[derive(Debug, Clone, PartialEq)]
pub enum FjValue {
    /// Null / unit.
    Null,
    /// Integer.
    Int(i64),
    /// Float.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// Character.
    Char(char),
    /// String.
    Str(String),
    /// Array of values.
    Array(Vec<FjValue>),
    /// HashMap of string keys to values.
    Map(HashMap<String, FjValue>),
    /// Option: Some(value) or None.
    Option(Option<Box<FjValue>>),
    /// Result: Ok(value) or Err(message).
    Result(Result<Box<FjValue>, String>),
}

impl fmt::Display for FjValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FjValue::Null => write!(f, "null"),
            FjValue::Int(n) => write!(f, "{n}"),
            FjValue::Float(n) => write!(f, "{n}"),
            FjValue::Bool(b) => write!(f, "{b}"),
            FjValue::Char(c) => write!(f, "'{c}'"),
            FjValue::Str(s) => write!(f, "{s}"),
            FjValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", items.join(", "))
            }
            FjValue::Map(map) => {
                let items: Vec<String> = map.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", items.join(", "))
            }
            FjValue::Option(Some(v)) => write!(f, "Some({v})"),
            FjValue::Option(None) => write!(f, "None"),
            FjValue::Result(Ok(v)) => write!(f, "Ok({v})"),
            FjValue::Result(Err(e)) => write!(f, "Err({e})"),
        }
    }
}

/// Creates an Option::Some value.
pub fn fj_some(val: FjValue) -> FjValue {
    FjValue::Option(Some(Box::new(val)))
}

/// Creates an Option::None value.
pub fn fj_none() -> FjValue {
    FjValue::Option(None)
}

/// Unwraps an Option, returning the inner value or a default.
pub fn fj_unwrap_or(opt: &FjValue, default: FjValue) -> FjValue {
    match opt {
        FjValue::Option(Some(v)) => *v.clone(),
        FjValue::Option(None) => default,
        _ => default,
    }
}

/// Returns true if the Option is Some.
pub fn fj_is_some(opt: &FjValue) -> bool {
    matches!(opt, FjValue::Option(Some(_)))
}

/// Returns true if the Option is None.
pub fn fj_is_none(opt: &FjValue) -> bool {
    matches!(opt, FjValue::Option(None))
}

// ═══════════════════════════════════════════════════════════════════════
// S6.2: Core Types — Result<T, E>
// ═══════════════════════════════════════════════════════════════════════

/// Creates a Result::Ok value.
pub fn fj_ok(val: FjValue) -> FjValue {
    FjValue::Result(Ok(Box::new(val)))
}

/// Creates a Result::Err value.
pub fn fj_err(msg: &str) -> FjValue {
    FjValue::Result(Err(msg.into()))
}

/// Returns true if the Result is Ok.
pub fn fj_is_ok(res: &FjValue) -> bool {
    matches!(res, FjValue::Result(Ok(_)))
}

/// Returns true if the Result is Err.
pub fn fj_is_err(res: &FjValue) -> bool {
    matches!(res, FjValue::Result(Err(_)))
}

/// Unwraps a Result or returns the error message.
pub fn fj_result_unwrap(res: &FjValue) -> Result<FjValue, String> {
    match res {
        FjValue::Result(Ok(v)) => Ok(*v.clone()),
        FjValue::Result(Err(e)) => Err(e.clone()),
        _ => Err("not a Result value".into()),
    }
}

/// Maps a function over a Result::Ok, leaving Err untouched.
pub fn fj_result_map(res: &FjValue, f: fn(FjValue) -> FjValue) -> FjValue {
    match res {
        FjValue::Result(Ok(v)) => fj_ok(f(*v.clone())),
        FjValue::Result(Err(e)) => fj_err(e),
        other => other.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.3: Array Operations
// ═══════════════════════════════════════════════════════════════════════

/// Creates a new empty array.
pub fn fj_array_new() -> FjValue {
    FjValue::Array(Vec::new())
}

/// Pushes a value onto an array. Returns the modified array.
pub fn fj_array_push(arr: &FjValue, val: FjValue) -> FjValue {
    match arr {
        FjValue::Array(items) => {
            let mut new_items = items.clone();
            new_items.push(val);
            FjValue::Array(new_items)
        }
        _ => arr.clone(),
    }
}

/// Pops the last value from an array. Returns (modified_array, popped_value).
pub fn fj_array_pop(arr: &FjValue) -> (FjValue, FjValue) {
    match arr {
        FjValue::Array(items) => {
            let mut new_items = items.clone();
            let popped = new_items.pop().map(fj_some).unwrap_or_else(fj_none);
            (FjValue::Array(new_items), popped)
        }
        _ => (arr.clone(), fj_none()),
    }
}

/// Returns the length of an array.
pub fn fj_array_len(arr: &FjValue) -> usize {
    match arr {
        FjValue::Array(items) => items.len(),
        _ => 0,
    }
}

/// Gets an element at an index. Returns Option.
pub fn fj_array_get(arr: &FjValue, index: usize) -> FjValue {
    match arr {
        FjValue::Array(items) => {
            if index < items.len() {
                fj_some(items[index].clone())
            } else {
                fj_none()
            }
        }
        _ => fj_none(),
    }
}

/// Sorts an array of integers in ascending order.
pub fn fj_array_sort(arr: &FjValue) -> FjValue {
    match arr {
        FjValue::Array(items) => {
            let mut sorted = items.clone();
            sorted.sort_by(|a, b| match (a, b) {
                (FjValue::Int(x), FjValue::Int(y)) => x.cmp(y),
                (FjValue::Str(x), FjValue::Str(y)) => x.cmp(y),
                _ => std::cmp::Ordering::Equal,
            });
            FjValue::Array(sorted)
        }
        _ => arr.clone(),
    }
}

/// Reverses an array.
pub fn fj_array_reverse(arr: &FjValue) -> FjValue {
    match arr {
        FjValue::Array(items) => {
            let mut reversed = items.clone();
            reversed.reverse();
            FjValue::Array(reversed)
        }
        _ => arr.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.4: HashMap Operations
// ═══════════════════════════════════════════════════════════════════════

/// Creates a new empty HashMap.
pub fn fj_map_new() -> FjValue {
    FjValue::Map(HashMap::new())
}

/// Inserts a key-value pair into a HashMap. Returns the modified map.
pub fn fj_map_insert(map: &FjValue, key: &str, val: FjValue) -> FjValue {
    match map {
        FjValue::Map(entries) => {
            let mut new_map = entries.clone();
            new_map.insert(key.into(), val);
            FjValue::Map(new_map)
        }
        _ => map.clone(),
    }
}

/// Gets a value from a HashMap by key. Returns Option.
pub fn fj_map_get(map: &FjValue, key: &str) -> FjValue {
    match map {
        FjValue::Map(entries) => match entries.get(key) {
            Some(v) => fj_some(v.clone()),
            None => fj_none(),
        },
        _ => fj_none(),
    }
}

/// Checks if a HashMap contains a key.
pub fn fj_map_contains(map: &FjValue, key: &str) -> bool {
    match map {
        FjValue::Map(entries) => entries.contains_key(key),
        _ => false,
    }
}

/// Removes a key from a HashMap. Returns the modified map.
pub fn fj_map_remove(map: &FjValue, key: &str) -> FjValue {
    match map {
        FjValue::Map(entries) => {
            let mut new_map = entries.clone();
            new_map.remove(key);
            FjValue::Map(new_map)
        }
        _ => map.clone(),
    }
}

/// Returns sorted keys of a HashMap.
pub fn fj_map_keys(map: &FjValue) -> FjValue {
    match map {
        FjValue::Map(entries) => {
            let mut keys: Vec<FjValue> = entries.keys().map(|k| FjValue::Str(k.clone())).collect();
            keys.sort_by(|a, b| {
                if let (FjValue::Str(x), FjValue::Str(y)) = (a, b) {
                    x.cmp(y)
                } else {
                    std::cmp::Ordering::Equal
                }
            });
            FjValue::Array(keys)
        }
        _ => fj_array_new(),
    }
}

/// Returns the number of entries in a HashMap.
pub fn fj_map_len(map: &FjValue) -> usize {
    match map {
        FjValue::Map(entries) => entries.len(),
        _ => 0,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.5: String Operations
// ═══════════════════════════════════════════════════════════════════════

/// Returns the length of a string.
pub fn fj_str_len(val: &FjValue) -> usize {
    match val {
        FjValue::Str(s) => s.len(),
        _ => 0,
    }
}

/// Checks if a string contains a substring.
pub fn fj_str_contains(val: &FjValue, needle: &str) -> bool {
    match val {
        FjValue::Str(s) => s.contains(needle),
        _ => false,
    }
}

/// Splits a string by a delimiter.
pub fn fj_str_split(val: &FjValue, delim: &str) -> FjValue {
    match val {
        FjValue::Str(s) => {
            let parts: Vec<FjValue> = s.split(delim).map(|p| FjValue::Str(p.into())).collect();
            FjValue::Array(parts)
        }
        _ => fj_array_new(),
    }
}

/// Trims whitespace from both ends of a string.
pub fn fj_str_trim(val: &FjValue) -> FjValue {
    match val {
        FjValue::Str(s) => FjValue::Str(s.trim().into()),
        _ => val.clone(),
    }
}

/// Replaces all occurrences of `from` with `to` in a string.
pub fn fj_str_replace(val: &FjValue, from: &str, to: &str) -> FjValue {
    match val {
        FjValue::Str(s) => FjValue::Str(s.replace(from, to)),
        _ => val.clone(),
    }
}

/// Checks if a string starts with a prefix.
pub fn fj_str_starts_with(val: &FjValue, prefix: &str) -> bool {
    match val {
        FjValue::Str(s) => s.starts_with(prefix),
        _ => false,
    }
}

/// Checks if a string ends with a suffix.
pub fn fj_str_ends_with(val: &FjValue, suffix: &str) -> bool {
    match val {
        FjValue::Str(s) => s.ends_with(suffix),
        _ => false,
    }
}

/// Converts a string to uppercase.
pub fn fj_str_to_upper(val: &FjValue) -> FjValue {
    match val {
        FjValue::Str(s) => FjValue::Str(s.to_uppercase()),
        _ => val.clone(),
    }
}

/// Converts a string to lowercase.
pub fn fj_str_to_lower(val: &FjValue) -> FjValue {
    match val {
        FjValue::Str(s) => FjValue::Str(s.to_lowercase()),
        _ => val.clone(),
    }
}

/// Returns a substring from `start` to `end` (exclusive).
pub fn fj_str_slice(val: &FjValue, start: usize, end: usize) -> FjValue {
    match val {
        FjValue::Str(s) => {
            let clamped_end = end.min(s.len());
            let clamped_start = start.min(clamped_end);
            FjValue::Str(s[clamped_start..clamped_end].into())
        }
        _ => val.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.6: Math Builtins
// ═══════════════════════════════════════════════════════════════════════

/// Absolute value.
pub fn fj_math_abs(val: &FjValue) -> FjValue {
    match val {
        FjValue::Int(n) => FjValue::Int(n.abs()),
        FjValue::Float(n) => FjValue::Float(n.abs()),
        _ => val.clone(),
    }
}

/// Minimum of two values.
pub fn fj_math_min(a: &FjValue, b: &FjValue) -> FjValue {
    match (a, b) {
        (FjValue::Int(x), FjValue::Int(y)) => FjValue::Int(*x.min(y)),
        (FjValue::Float(x), FjValue::Float(y)) => FjValue::Float(x.min(*y)),
        _ => a.clone(),
    }
}

/// Maximum of two values.
pub fn fj_math_max(a: &FjValue, b: &FjValue) -> FjValue {
    match (a, b) {
        (FjValue::Int(x), FjValue::Int(y)) => FjValue::Int(*x.max(y)),
        (FjValue::Float(x), FjValue::Float(y)) => FjValue::Float(x.max(*y)),
        _ => a.clone(),
    }
}

/// Power (integer exponent).
pub fn fj_math_pow(base: &FjValue, exp: &FjValue) -> FjValue {
    match (base, exp) {
        (FjValue::Int(b), FjValue::Int(e)) => {
            if *e >= 0 && *e <= 63 {
                FjValue::Int(b.wrapping_pow(*e as u32))
            } else {
                FjValue::Int(0)
            }
        }
        (FjValue::Float(b), FjValue::Float(e)) => FjValue::Float(b.powf(*e)),
        (FjValue::Float(b), FjValue::Int(e)) => FjValue::Float(b.powi(*e as i32)),
        _ => base.clone(),
    }
}

/// Square root.
pub fn fj_math_sqrt(val: &FjValue) -> FjValue {
    match val {
        FjValue::Float(n) => FjValue::Float(n.sqrt()),
        FjValue::Int(n) => FjValue::Float((*n as f64).sqrt()),
        _ => val.clone(),
    }
}

/// Clamp a value between min and max.
pub fn fj_math_clamp(val: &FjValue, min: &FjValue, max: &FjValue) -> FjValue {
    match (val, min, max) {
        (FjValue::Int(v), FjValue::Int(lo), FjValue::Int(hi)) => {
            FjValue::Int((*v).max(*lo).min(*hi))
        }
        (FjValue::Float(v), FjValue::Float(lo), FjValue::Float(hi)) => {
            FjValue::Float(v.max(*lo).min(*hi))
        }
        _ => val.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.7: IO Builtins
// ═══════════════════════════════════════════════════════════════════════

/// Simulated IO output buffer (for testing without actual I/O).
#[derive(Debug, Clone, Default)]
pub struct IoBuffer {
    /// Lines written via print/println.
    pub lines: Vec<String>,
}

impl IoBuffer {
    /// Creates a new IO buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Simulates `print(val)`.
    pub fn print(&mut self, val: &FjValue) {
        self.lines.push(val.to_string());
    }

    /// Simulates `println(val)`.
    pub fn println(&mut self, val: &FjValue) {
        self.lines.push(format!("{val}\n"));
    }

    /// Simulates `eprintln(val)`.
    pub fn eprintln(&mut self, val: &FjValue) {
        self.lines.push(format!("[ERR] {val}\n"));
    }

    /// Simulates `dbg(val)` — prints debug representation.
    pub fn dbg(&mut self, val: &FjValue) {
        self.lines.push(format!("[dbg] {val:?}"));
    }

    /// Returns all output as a single string.
    pub fn output(&self) -> String {
        self.lines.join("")
    }

    /// Clears the buffer.
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

/// Simulated file system for read/write testing.
#[derive(Debug, Clone, Default)]
pub struct SimulatedFs {
    /// In-memory file system: path -> contents.
    files: HashMap<String, String>,
}

impl SimulatedFs {
    /// Creates a new simulated file system.
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes content to a file.
    pub fn write_file(&mut self, path: &str, content: &str) -> FjValue {
        self.files.insert(path.into(), content.into());
        fj_ok(FjValue::Null)
    }

    /// Reads content from a file.
    pub fn read_file(&self, path: &str) -> FjValue {
        match self.files.get(path) {
            Some(content) => fj_ok(FjValue::Str(content.clone())),
            None => fj_err(&format!("file not found: {path}")),
        }
    }

    /// Checks if a file exists.
    pub fn file_exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Appends content to a file.
    pub fn append_file(&mut self, path: &str, content: &str) -> FjValue {
        let existing = self.files.entry(path.into()).or_default();
        existing.push_str(content);
        fj_ok(FjValue::Null)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.8: Iterator Protocol
// ═══════════════════════════════════════════════════════════════════════

/// A simulated iterator over FjValues.
#[derive(Debug, Clone)]
pub struct FjIterator {
    /// Items remaining.
    items: Vec<FjValue>,
    /// Current position.
    pos: usize,
}

impl FjIterator {
    /// Creates a new iterator from an array value.
    pub fn from_array(arr: &FjValue) -> Self {
        match arr {
            FjValue::Array(items) => Self {
                items: items.clone(),
                pos: 0,
            },
            _ => Self {
                items: Vec::new(),
                pos: 0,
            },
        }
    }

    /// Returns the next value, or None if exhausted.
    pub fn next_item(&mut self) -> FjValue {
        if self.pos < self.items.len() {
            let val = self.items[self.pos].clone();
            self.pos += 1;
            fj_some(val)
        } else {
            fj_none()
        }
    }

    /// Maps a function over remaining items, returning a new array.
    pub fn map(self, f: fn(&FjValue) -> FjValue) -> FjValue {
        let mapped: Vec<FjValue> = self.items[self.pos..].iter().map(f).collect();
        FjValue::Array(mapped)
    }

    /// Filters remaining items by a predicate, returning a new array.
    pub fn filter(self, pred: fn(&FjValue) -> bool) -> FjValue {
        let filtered: Vec<FjValue> = self.items[self.pos..]
            .iter()
            .filter(|v| pred(v))
            .cloned()
            .collect();
        FjValue::Array(filtered)
    }

    /// Collects remaining items into an array.
    pub fn collect(self) -> FjValue {
        FjValue::Array(self.items[self.pos..].to_vec())
    }

    /// Folds over remaining items.
    pub fn fold(self, init: FjValue, f: fn(FjValue, &FjValue) -> FjValue) -> FjValue {
        let mut acc = init;
        for item in &self.items[self.pos..] {
            acc = f(acc, item);
        }
        acc
    }

    /// Returns the count of remaining items.
    pub fn count(&self) -> usize {
        if self.pos <= self.items.len() {
            self.items.len() - self.pos
        } else {
            0
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.9: Error Handling Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Simulates the `?` operator: if Err, return early; if Ok, unwrap.
pub fn fj_try(res: &FjValue) -> Result<FjValue, String> {
    match res {
        FjValue::Result(Ok(v)) => Ok(*v.clone()),
        FjValue::Result(Err(e)) => Err(e.clone()),
        _ => Ok(res.clone()),
    }
}

/// Chains two Results: if first is Ok, apply function; else propagate Err.
pub fn fj_and_then(res: &FjValue, f: fn(FjValue) -> FjValue) -> FjValue {
    match res {
        FjValue::Result(Ok(v)) => {
            let result = f(*v.clone());
            // Wrap in Result if not already
            match &result {
                FjValue::Result(_) => result,
                _ => fj_ok(result),
            }
        }
        FjValue::Result(Err(e)) => fj_err(e),
        other => other.clone(),
    }
}

/// Converts an Option to a Result with a custom error message.
pub fn fj_option_ok_or(opt: &FjValue, err_msg: &str) -> FjValue {
    match opt {
        FjValue::Option(Some(v)) => fj_ok(*v.clone()),
        FjValue::Option(None) => fj_err(err_msg),
        _ => fj_err("not an Option value"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.10: Formatting / Debug Printing
// ═══════════════════════════════════════════════════════════════════════

/// Simulated `format!` — formats a template with positional args.
pub fn fj_format(template: &str, args: &[FjValue]) -> FjValue {
    let mut result = template.to_string();
    for arg in args {
        if let Some(pos) = result.find("{}") {
            result.replace_range(pos..pos + 2, &arg.to_string());
        }
    }
    FjValue::Str(result)
}

/// Converts any value to its type name string.
pub fn fj_type_of(val: &FjValue) -> FjValue {
    let name = match val {
        FjValue::Null => "null",
        FjValue::Int(_) => "i64",
        FjValue::Float(_) => "f64",
        FjValue::Bool(_) => "bool",
        FjValue::Char(_) => "char",
        FjValue::Str(_) => "str",
        FjValue::Array(_) => "Array",
        FjValue::Map(_) => "HashMap",
        FjValue::Option(_) => "Option",
        FjValue::Result(_) => "Result",
    };
    FjValue::Str(name.into())
}

/// Standard library registry — maps builtin names to their metadata.
#[derive(Debug, Clone)]
pub struct StdlibRegistry {
    /// Builtin name -> (parameter count, description).
    pub builtins: HashMap<String, (usize, String)>,
}

impl StdlibRegistry {
    /// Creates a registry with all standard builtins.
    pub fn new() -> Self {
        let mut builtins = HashMap::new();
        // Math builtins
        builtins.insert("abs".into(), (1, "absolute value".into()));
        builtins.insert("min".into(), (2, "minimum of two values".into()));
        builtins.insert("max".into(), (2, "maximum of two values".into()));
        builtins.insert("pow".into(), (2, "power function".into()));
        builtins.insert("sqrt".into(), (1, "square root".into()));
        builtins.insert("clamp".into(), (3, "clamp between min/max".into()));
        // IO builtins
        builtins.insert("print".into(), (1, "print without newline".into()));
        builtins.insert("println".into(), (1, "print with newline".into()));
        builtins.insert("eprintln".into(), (1, "print to stderr".into()));
        builtins.insert("dbg".into(), (1, "debug print".into()));
        builtins.insert("read_file".into(), (1, "read file contents".into()));
        builtins.insert("write_file".into(), (2, "write file contents".into()));
        // String builtins
        builtins.insert("len".into(), (1, "length of string/array".into()));
        builtins.insert("type_of".into(), (1, "type name of value".into()));
        builtins.insert("to_string".into(), (1, "convert to string".into()));
        builtins.insert("parse_int".into(), (1, "parse string to int".into()));
        builtins.insert("parse_float".into(), (1, "parse string to float".into()));
        Self { builtins }
    }

    /// Returns the number of registered builtins.
    pub fn count(&self) -> usize {
        self.builtins.len()
    }

    /// Checks if a builtin exists.
    pub fn has(&self, name: &str) -> bool {
        self.builtins.contains_key(name)
    }

    /// Returns the parameter count for a builtin.
    pub fn param_count(&self, name: &str) -> Option<usize> {
        self.builtins.get(name).map(|(count, _)| *count)
    }
}

impl Default for StdlibRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses a string to an integer. Returns Result.
pub fn fj_parse_int(val: &FjValue) -> FjValue {
    match val {
        FjValue::Str(s) => match s.trim().parse::<i64>() {
            Ok(n) => fj_ok(FjValue::Int(n)),
            Err(_) => fj_err(&format!("cannot parse '{s}' as integer")),
        },
        _ => fj_err("parse_int requires a string"),
    }
}

/// Parses a string to a float. Returns Result.
pub fn fj_parse_float(val: &FjValue) -> FjValue {
    match val {
        FjValue::Str(s) => match s.trim().parse::<f64>() {
            Ok(n) => fj_ok(FjValue::Float(n)),
            Err(_) => fj_err(&format!("cannot parse '{s}' as float")),
        },
        _ => fj_err("parse_float requires a string"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S6.1 — Option<T>
    #[test]
    fn s6_1_option_some_none() {
        let some = fj_some(FjValue::Int(42));
        assert!(fj_is_some(&some));
        assert!(!fj_is_none(&some));

        let none = fj_none();
        assert!(fj_is_none(&none));
        assert!(!fj_is_some(&none));
    }

    #[test]
    fn s6_1_option_unwrap_or() {
        let some = fj_some(FjValue::Int(42));
        assert_eq!(fj_unwrap_or(&some, FjValue::Int(0)), FjValue::Int(42));

        let none = fj_none();
        assert_eq!(fj_unwrap_or(&none, FjValue::Int(0)), FjValue::Int(0));
    }

    #[test]
    fn s6_1_option_display() {
        assert_eq!(fj_some(FjValue::Int(42)).to_string(), "Some(42)");
        assert_eq!(fj_none().to_string(), "None");
    }

    // S6.2 — Result<T, E>
    #[test]
    fn s6_2_result_ok_err() {
        let ok = fj_ok(FjValue::Int(10));
        assert!(fj_is_ok(&ok));
        assert!(!fj_is_err(&ok));

        let err = fj_err("something failed");
        assert!(fj_is_err(&err));
        assert!(!fj_is_ok(&err));
    }

    #[test]
    fn s6_2_result_unwrap() {
        let ok = fj_ok(FjValue::Str("hello".into()));
        assert_eq!(fj_result_unwrap(&ok).unwrap(), FjValue::Str("hello".into()));

        let err = fj_err("oops");
        assert!(fj_result_unwrap(&err).is_err());
    }

    #[test]
    fn s6_2_result_map() {
        fn double(v: FjValue) -> FjValue {
            match v {
                FjValue::Int(n) => FjValue::Int(n * 2),
                other => other,
            }
        }
        let ok = fj_ok(FjValue::Int(5));
        let mapped = fj_result_map(&ok, double);
        assert_eq!(fj_result_unwrap(&mapped).unwrap(), FjValue::Int(10));

        let err = fj_err("fail");
        let mapped_err = fj_result_map(&err, double);
        assert!(fj_is_err(&mapped_err));
    }

    // S6.3 — Array Operations
    #[test]
    fn s6_3_array_push_pop() {
        let arr = fj_array_new();
        let arr = fj_array_push(&arr, FjValue::Int(1));
        let arr = fj_array_push(&arr, FjValue::Int(2));
        assert_eq!(fj_array_len(&arr), 2);

        let (arr, popped) = fj_array_pop(&arr);
        assert_eq!(fj_array_len(&arr), 1);
        assert_eq!(fj_unwrap_or(&popped, FjValue::Null), FjValue::Int(2));
    }

    #[test]
    fn s6_3_array_get_sort() {
        let arr = FjValue::Array(vec![FjValue::Int(3), FjValue::Int(1), FjValue::Int(2)]);
        assert_eq!(
            fj_unwrap_or(&fj_array_get(&arr, 0), FjValue::Null),
            FjValue::Int(3)
        );
        assert!(fj_is_none(&fj_array_get(&arr, 10)));

        let sorted = fj_array_sort(&arr);
        if let FjValue::Array(items) = &sorted {
            assert_eq!(items[0], FjValue::Int(1));
            assert_eq!(items[2], FjValue::Int(3));
        }
    }

    #[test]
    fn s6_3_array_reverse() {
        let arr = FjValue::Array(vec![FjValue::Int(1), FjValue::Int(2), FjValue::Int(3)]);
        let rev = fj_array_reverse(&arr);
        if let FjValue::Array(items) = &rev {
            assert_eq!(items[0], FjValue::Int(3));
            assert_eq!(items[2], FjValue::Int(1));
        }
    }

    // S6.4 — HashMap Operations
    #[test]
    fn s6_4_map_insert_get_remove() {
        let map = fj_map_new();
        let map = fj_map_insert(&map, "name", FjValue::Str("Fajar".into()));
        assert!(fj_map_contains(&map, "name"));
        assert!(!fj_map_contains(&map, "age"));
        assert_eq!(fj_map_len(&map), 1);

        let val = fj_map_get(&map, "name");
        assert_eq!(
            fj_unwrap_or(&val, FjValue::Null),
            FjValue::Str("Fajar".into())
        );

        let map = fj_map_remove(&map, "name");
        assert!(!fj_map_contains(&map, "name"));
    }

    #[test]
    fn s6_4_map_keys() {
        let map = fj_map_new();
        let map = fj_map_insert(&map, "b", FjValue::Int(2));
        let map = fj_map_insert(&map, "a", FjValue::Int(1));
        let keys = fj_map_keys(&map);
        if let FjValue::Array(items) = &keys {
            assert_eq!(items.len(), 2);
            // Keys are sorted
            assert_eq!(items[0], FjValue::Str("a".into()));
            assert_eq!(items[1], FjValue::Str("b".into()));
        }
    }

    // S6.5 — String Operations
    #[test]
    fn s6_5_string_len_contains() {
        let s = FjValue::Str("hello world".into());
        assert_eq!(fj_str_len(&s), 11);
        assert!(fj_str_contains(&s, "world"));
        assert!(!fj_str_contains(&s, "xyz"));
    }

    #[test]
    fn s6_5_string_split_trim() {
        let s = FjValue::Str("a,b,c".into());
        let parts = fj_str_split(&s, ",");
        if let FjValue::Array(items) = &parts {
            assert_eq!(items.len(), 3);
            assert_eq!(items[1], FjValue::Str("b".into()));
        }

        let padded = FjValue::Str("  hello  ".into());
        assert_eq!(fj_str_trim(&padded), FjValue::Str("hello".into()));
    }

    #[test]
    fn s6_5_string_replace_case() {
        let s = FjValue::Str("Hello World".into());
        let replaced = fj_str_replace(&s, "World", "Fajar");
        assert_eq!(replaced, FjValue::Str("Hello Fajar".into()));
        assert_eq!(fj_str_to_upper(&s), FjValue::Str("HELLO WORLD".into()));
        assert_eq!(fj_str_to_lower(&s), FjValue::Str("hello world".into()));
    }

    #[test]
    fn s6_5_string_starts_ends_slice() {
        let s = FjValue::Str("hello world".into());
        assert!(fj_str_starts_with(&s, "hello"));
        assert!(!fj_str_starts_with(&s, "world"));
        assert!(fj_str_ends_with(&s, "world"));
        assert_eq!(fj_str_slice(&s, 0, 5), FjValue::Str("hello".into()));
    }

    // S6.6 — Math Builtins
    #[test]
    fn s6_6_math_abs_min_max() {
        assert_eq!(fj_math_abs(&FjValue::Int(-42)), FjValue::Int(42));
        assert_eq!(fj_math_abs(&FjValue::Float(-1.25)), FjValue::Float(1.25));
        assert_eq!(
            fj_math_min(&FjValue::Int(3), &FjValue::Int(7)),
            FjValue::Int(3)
        );
        assert_eq!(
            fj_math_max(&FjValue::Int(3), &FjValue::Int(7)),
            FjValue::Int(7)
        );
    }

    #[test]
    fn s6_6_math_pow_sqrt() {
        assert_eq!(
            fj_math_pow(&FjValue::Int(2), &FjValue::Int(10)),
            FjValue::Int(1024)
        );
        let sqrt_result = fj_math_sqrt(&FjValue::Float(16.0));
        if let FjValue::Float(n) = sqrt_result {
            assert!((n - 4.0).abs() < 1e-10);
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn s6_6_math_clamp() {
        assert_eq!(
            fj_math_clamp(&FjValue::Int(15), &FjValue::Int(0), &FjValue::Int(10)),
            FjValue::Int(10)
        );
        assert_eq!(
            fj_math_clamp(&FjValue::Int(-5), &FjValue::Int(0), &FjValue::Int(10)),
            FjValue::Int(0)
        );
        assert_eq!(
            fj_math_clamp(&FjValue::Int(5), &FjValue::Int(0), &FjValue::Int(10)),
            FjValue::Int(5)
        );
    }

    // S6.7 — IO Builtins
    #[test]
    fn s6_7_io_buffer() {
        let mut io = IoBuffer::new();
        io.println(&FjValue::Str("hello".into()));
        io.print(&FjValue::Int(42));
        assert_eq!(io.lines.len(), 2);
        assert!(io.output().contains("hello"));
        assert!(io.output().contains("42"));
    }

    #[test]
    fn s6_7_simulated_fs() {
        let mut fs = SimulatedFs::new();
        fs.write_file("test.fj", "fn main() {}");
        assert!(fs.file_exists("test.fj"));
        assert!(!fs.file_exists("other.fj"));

        let content = fs.read_file("test.fj");
        assert!(fj_is_ok(&content));
        assert_eq!(
            fj_result_unwrap(&content).unwrap(),
            FjValue::Str("fn main() {}".into())
        );

        fs.append_file("test.fj", "\nprintln(42)");
        let content2 = fs.read_file("test.fj");
        let text = fj_result_unwrap(&content2).unwrap();
        if let FjValue::Str(s) = text {
            assert!(s.contains("println(42)"));
        }
    }

    // S6.8 — Iterator Protocol
    #[test]
    fn s6_8_iterator_next() {
        let arr = FjValue::Array(vec![FjValue::Int(1), FjValue::Int(2), FjValue::Int(3)]);
        let mut iter = FjIterator::from_array(&arr);
        assert_eq!(iter.count(), 3);
        assert_eq!(
            fj_unwrap_or(&iter.next_item(), FjValue::Null),
            FjValue::Int(1)
        );
        assert_eq!(
            fj_unwrap_or(&iter.next_item(), FjValue::Null),
            FjValue::Int(2)
        );
        assert_eq!(iter.count(), 1);
    }

    #[test]
    fn s6_8_iterator_map_filter() {
        let arr = FjValue::Array(vec![FjValue::Int(1), FjValue::Int(2), FjValue::Int(3)]);

        let iter = FjIterator::from_array(&arr);
        fn double(v: &FjValue) -> FjValue {
            match v {
                FjValue::Int(n) => FjValue::Int(n * 2),
                other => other.clone(),
            }
        }
        let mapped = iter.map(double);
        assert_eq!(
            mapped,
            FjValue::Array(vec![FjValue::Int(2), FjValue::Int(4), FjValue::Int(6)])
        );

        let iter2 = FjIterator::from_array(&arr);
        fn is_even(v: &FjValue) -> bool {
            matches!(v, FjValue::Int(n) if n % 2 == 0)
        }
        let filtered = iter2.filter(is_even);
        assert_eq!(filtered, FjValue::Array(vec![FjValue::Int(2)]));
    }

    #[test]
    fn s6_8_iterator_fold_collect() {
        let arr = FjValue::Array(vec![FjValue::Int(1), FjValue::Int(2), FjValue::Int(3)]);
        let iter = FjIterator::from_array(&arr);
        fn sum(acc: FjValue, item: &FjValue) -> FjValue {
            match (&acc, item) {
                (FjValue::Int(a), FjValue::Int(b)) => FjValue::Int(a + b),
                _ => acc,
            }
        }
        let total = iter.fold(FjValue::Int(0), sum);
        assert_eq!(total, FjValue::Int(6));

        let iter2 = FjIterator::from_array(&arr);
        let collected = iter2.collect();
        assert_eq!(collected, arr);
    }

    // S6.9 — Error Handling Helpers
    #[test]
    fn s6_9_try_operator() {
        let ok = fj_ok(FjValue::Int(42));
        assert_eq!(fj_try(&ok).unwrap(), FjValue::Int(42));

        let err = fj_err("failure");
        assert!(fj_try(&err).is_err());
    }

    #[test]
    fn s6_9_and_then_chain() {
        fn validate(v: FjValue) -> FjValue {
            match v {
                FjValue::Int(n) if n > 0 => fj_ok(FjValue::Int(n * 2)),
                _ => fj_err("must be positive"),
            }
        }
        let ok = fj_ok(FjValue::Int(5));
        let result = fj_and_then(&ok, validate);
        assert_eq!(fj_result_unwrap(&result).unwrap(), FjValue::Int(10));

        let err = fj_err("initial error");
        let result2 = fj_and_then(&err, validate);
        assert!(fj_is_err(&result2));
    }

    #[test]
    fn s6_9_option_ok_or() {
        let some = fj_some(FjValue::Int(42));
        let result = fj_option_ok_or(&some, "was none");
        assert!(fj_is_ok(&result));

        let none = fj_none();
        let result2 = fj_option_ok_or(&none, "was none");
        assert!(fj_is_err(&result2));
    }

    // S6.10 — Formatting / Debug / Registry
    #[test]
    fn s6_10_format_template() {
        let result = fj_format(
            "Hello, {}! You are {} years old.",
            &[FjValue::Str("Fajar".into()), FjValue::Int(30)],
        );
        assert_eq!(
            result,
            FjValue::Str("Hello, Fajar! You are 30 years old.".into())
        );
    }

    #[test]
    fn s6_10_type_of() {
        assert_eq!(fj_type_of(&FjValue::Int(42)), FjValue::Str("i64".into()));
        assert_eq!(
            fj_type_of(&FjValue::Bool(true)),
            FjValue::Str("bool".into())
        );
        assert_eq!(
            fj_type_of(&FjValue::Str("hi".into())),
            FjValue::Str("str".into())
        );
        assert_eq!(
            fj_type_of(&FjValue::Array(vec![])),
            FjValue::Str("Array".into())
        );
    }

    #[test]
    fn s6_10_stdlib_registry() {
        let registry = StdlibRegistry::new();
        assert!(registry.count() >= 17);
        assert!(registry.has("abs"));
        assert!(registry.has("println"));
        assert!(registry.has("len"));
        assert!(!registry.has("nonexistent"));
        assert_eq!(registry.param_count("min"), Some(2));
        assert_eq!(registry.param_count("print"), Some(1));
    }

    #[test]
    fn s6_10_parse_int_float() {
        let ok_int = fj_parse_int(&FjValue::Str("42".into()));
        assert_eq!(fj_result_unwrap(&ok_int).unwrap(), FjValue::Int(42));

        let bad_int = fj_parse_int(&FjValue::Str("abc".into()));
        assert!(fj_is_err(&bad_int));

        let ok_float = fj_parse_float(&FjValue::Str("1.25".into()));
        if let Ok(FjValue::Float(n)) = fj_result_unwrap(&ok_float) {
            assert!((n - 1.25).abs() < 1e-10);
        }
    }

    #[test]
    fn s6_10_io_dbg_eprintln() {
        let mut io = IoBuffer::new();
        io.dbg(&FjValue::Int(42));
        io.eprintln(&FjValue::Str("error".into()));
        assert!(io.lines[0].contains("[dbg]"));
        assert!(io.lines[1].contains("[ERR]"));
        io.clear();
        assert!(io.lines.is_empty());
    }
}
