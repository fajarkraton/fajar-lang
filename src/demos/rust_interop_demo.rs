//! Sprint W6: Rust serde_json Interop — JSON value model, parser, serializer,
//! serde bridge, schema validator, JSONPath queries, diff, pretty-printer.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// W6.1: JsonValue — Fajar Representation of JSON
// ═══════════════════════════════════════════════════════════════════════

/// Fajar Lang representation of a JSON value, mirroring serde_json::Value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// JSON number (integer).
    Integer(i64),
    /// JSON number (floating point).
    Float(f64),
    /// JSON string.
    String(String),
    /// JSON array.
    Array(Vec<JsonValue>),
    /// JSON object (preserves insertion order via Vec of pairs).
    Object(Vec<(String, JsonValue)>),
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonValue::Null => write!(f, "null"),
            JsonValue::Bool(b) => write!(f, "{b}"),
            JsonValue::Integer(n) => write!(f, "{n}"),
            JsonValue::Float(v) => {
                if v.fract() == 0.0 {
                    write!(f, "{v:.1}")
                } else {
                    write!(f, "{v}")
                }
            }
            JsonValue::String(s) => write!(f, "\"{s}\""),
            JsonValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            JsonValue::Object(pairs) => {
                write!(f, "{{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "\"{k}\":{v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl JsonValue {
    /// Returns true if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }

    /// Returns true if the value is an object.
    pub fn is_object(&self) -> bool {
        matches!(self, JsonValue::Object(_))
    }

    /// Returns true if the value is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, JsonValue::Array(_))
    }

    /// Get a field from an object by key.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Get an element from an array by index.
    pub fn get_index(&self, idx: usize) -> Option<&JsonValue> {
        match self {
            JsonValue::Array(arr) => arr.get(idx),
            _ => None,
        }
    }

    /// Returns the type name of this JSON value.
    pub fn type_name(&self) -> &'static str {
        match self {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Integer(_) => "integer",
            JsonValue::Float(_) => "number",
            JsonValue::String(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.2: JsonParser — Parse JSON String into JsonValue
// ═══════════════════════════════════════════════════════════════════════

/// Error type for JSON parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonParseError {
    /// Error message.
    pub message: String,
    /// Byte position where the error occurred.
    pub position: usize,
}

impl fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON parse error at {}: {}", self.position, self.message)
    }
}

/// JSON parser that converts a JSON string into a `JsonValue`.
pub struct JsonParser {
    /// Input bytes.
    input: Vec<char>,
    /// Current position.
    pos: usize,
}

impl JsonParser {
    /// Parse a JSON string into a `JsonValue`.
    pub fn parse(input: &str) -> Result<JsonValue, JsonParseError> {
        let mut parser = Self {
            input: input.chars().collect(),
            pos: 0,
        };
        let value = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.pos < parser.input.len() {
            return Err(JsonParseError {
                message: "unexpected trailing content".into(),
                position: parser.pos,
            });
        }
        Ok(value)
    }

    /// Skip whitespace characters.
    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// Peek current character.
    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    /// Advance and return current character.
    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    /// Expect a specific character.
    fn expect(&mut self, expected: char) -> Result<(), JsonParseError> {
        match self.advance() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(JsonParseError {
                message: format!("expected '{expected}', found '{ch}'"),
                position: self.pos - 1,
            }),
            None => Err(JsonParseError {
                message: format!("expected '{expected}', found EOF"),
                position: self.pos,
            }),
        }
    }

    /// Parse any JSON value.
    fn parse_value(&mut self) -> Result<JsonValue, JsonParseError> {
        self.skip_whitespace();
        match self.peek() {
            Some('"') => self.parse_string_value(),
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('t') | Some('f') => self.parse_bool(),
            Some('n') => self.parse_null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(JsonParseError {
                message: format!("unexpected character '{c}'"),
                position: self.pos,
            }),
            None => Err(JsonParseError {
                message: "unexpected end of input".into(),
                position: self.pos,
            }),
        }
    }

    /// Parse a JSON string (including surrounding quotes).
    fn parse_string(&mut self) -> Result<String, JsonParseError> {
        self.expect('"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('\\') => match self.advance() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('/') => s.push('/'),
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('b') => s.push('\u{0008}'),
                    Some('f') => s.push('\u{000C}'),
                    Some(c) => {
                        return Err(JsonParseError {
                            message: format!("invalid escape '\\{c}'"),
                            position: self.pos - 1,
                        })
                    }
                    None => {
                        return Err(JsonParseError {
                            message: "unterminated string escape".into(),
                            position: self.pos,
                        })
                    }
                },
                Some('"') => break,
                Some(c) => s.push(c),
                None => {
                    return Err(JsonParseError {
                        message: "unterminated string".into(),
                        position: self.pos,
                    })
                }
            }
        }
        Ok(s)
    }

    /// Parse a JSON string value.
    fn parse_string_value(&mut self) -> Result<JsonValue, JsonParseError> {
        self.parse_string().map(JsonValue::String)
    }

    /// Parse a JSON number.
    fn parse_number(&mut self) -> Result<JsonValue, JsonParseError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let mut is_float = false;
        if self.pos < self.input.len() && self.input[self.pos] == '.' {
            is_float = true;
            self.pos += 1;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < self.input.len()
            && (self.input[self.pos] == 'e' || self.input[self.pos] == 'E')
        {
            is_float = true;
            self.pos += 1;
            if self.pos < self.input.len()
                && (self.input[self.pos] == '+' || self.input[self.pos] == '-')
            {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let text: String = self.input[start..self.pos].iter().collect();
        if is_float {
            text.parse::<f64>().map(JsonValue::Float).map_err(|_| JsonParseError {
                message: format!("invalid number: {text}"),
                position: start,
            })
        } else {
            text.parse::<i64>().map(JsonValue::Integer).map_err(|_| JsonParseError {
                message: format!("invalid integer: {text}"),
                position: start,
            })
        }
    }

    /// Parse a JSON boolean.
    fn parse_bool(&mut self) -> Result<JsonValue, JsonParseError> {
        if self.input[self.pos..].starts_with(&['t', 'r', 'u', 'e']) {
            self.pos += 4;
            Ok(JsonValue::Bool(true))
        } else if self.input[self.pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            self.pos += 5;
            Ok(JsonValue::Bool(false))
        } else {
            Err(JsonParseError {
                message: "expected 'true' or 'false'".into(),
                position: self.pos,
            })
        }
    }

    /// Parse JSON null.
    fn parse_null(&mut self) -> Result<JsonValue, JsonParseError> {
        if self.input[self.pos..].starts_with(&['n', 'u', 'l', 'l']) {
            self.pos += 4;
            Ok(JsonValue::Null)
        } else {
            Err(JsonParseError {
                message: "expected 'null'".into(),
                position: self.pos,
            })
        }
    }

    /// Parse a JSON object.
    fn parse_object(&mut self) -> Result<JsonValue, JsonParseError> {
        self.expect('{')?;
        self.skip_whitespace();
        let mut pairs = Vec::new();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(':')?;
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                _ => {
                    return Err(JsonParseError {
                        message: "expected ',' or '}' in object".into(),
                        position: self.pos,
                    })
                }
            }
        }
        Ok(JsonValue::Object(pairs))
    }

    /// Parse a JSON array.
    fn parse_array(&mut self) -> Result<JsonValue, JsonParseError> {
        self.expect('[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                _ => {
                    return Err(JsonParseError {
                        message: "expected ',' or ']' in array".into(),
                        position: self.pos,
                    })
                }
            }
        }
        Ok(JsonValue::Array(items))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.3: JsonSerializer — Serialize JsonValue Back to String
// ═══════════════════════════════════════════════════════════════════════

/// Serializes a `JsonValue` into a compact JSON string.
pub struct JsonSerializer;

impl JsonSerializer {
    /// Serialize to compact JSON (no extra whitespace).
    pub fn to_string(value: &JsonValue) -> String {
        // Display impl is compact
        format!("{value}")
    }

    /// Serialize to a minified string with escaped special characters.
    pub fn to_escaped_string(value: &JsonValue) -> String {
        match value {
            JsonValue::String(s) => {
                let mut out = String::from('"');
                for ch in s.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c => out.push(c),
                    }
                }
                out.push('"');
                out
            }
            other => Self::to_string(other),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.4: SerdeBridge — Simulated Rust serde_json FFI Bridge
// ═══════════════════════════════════════════════════════════════════════

/// Trait for types that can be serialized to JSON via the serde bridge.
pub trait FjSerialize {
    /// Convert to JsonValue.
    fn to_json(&self) -> JsonValue;
}

/// Trait for types that can be deserialized from JSON via the serde bridge.
pub trait FjDeserialize: Sized {
    /// Convert from JsonValue.
    fn from_json(value: &JsonValue) -> Result<Self, String>;
}

/// Simulated serde_json bridge — demonstrates how Fajar Lang values would
/// cross the FFI boundary to Rust's serde_json.
pub struct SerdeBridge;

impl SerdeBridge {
    /// Simulate serializing a Fajar value through serde_json.
    /// In production, this would call into Rust serde_json via FFI.
    pub fn serialize<T: FjSerialize>(value: &T) -> Result<String, String> {
        let json = value.to_json();
        Ok(JsonSerializer::to_string(&json))
    }

    /// Simulate deserializing a JSON string into a Fajar value via serde_json.
    pub fn deserialize<T: FjDeserialize>(json_str: &str) -> Result<T, String> {
        let json = JsonParser::parse(json_str).map_err(|e| e.to_string())?;
        T::from_json(&json)
    }

    /// Round-trip: serialize then deserialize, verifying fidelity.
    pub fn round_trip<T: FjSerialize + FjDeserialize + PartialEq + fmt::Debug>(
        value: &T,
    ) -> Result<bool, String> {
        let json_str = Self::serialize(value)?;
        let back: T = Self::deserialize(&json_str)?;
        Ok(*value == back)
    }
}

/// Example struct for serde bridge demonstration.
#[derive(Debug, Clone, PartialEq)]
pub struct UserRecord {
    /// User name.
    pub name: String,
    /// User age.
    pub age: i64,
    /// Whether the user is active.
    pub active: bool,
    /// User tags.
    pub tags: Vec<String>,
}

impl FjSerialize for UserRecord {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("name".into(), JsonValue::String(self.name.clone())),
            ("age".into(), JsonValue::Integer(self.age)),
            ("active".into(), JsonValue::Bool(self.active)),
            (
                "tags".into(),
                JsonValue::Array(self.tags.iter().map(|t| JsonValue::String(t.clone())).collect()),
            ),
        ])
    }
}

impl FjDeserialize for UserRecord {
    fn from_json(value: &JsonValue) -> Result<Self, String> {
        let obj = match value {
            JsonValue::Object(pairs) => pairs,
            _ => return Err("expected object".into()),
        };
        let lookup = |key: &str| -> Option<&JsonValue> {
            obj.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        };
        let name = match lookup("name") {
            Some(JsonValue::String(s)) => s.clone(),
            _ => return Err("missing or invalid 'name'".into()),
        };
        let age = match lookup("age") {
            Some(JsonValue::Integer(n)) => *n,
            _ => return Err("missing or invalid 'age'".into()),
        };
        let active = match lookup("active") {
            Some(JsonValue::Bool(b)) => *b,
            _ => return Err("missing or invalid 'active'".into()),
        };
        let tags = match lookup("tags") {
            Some(JsonValue::Array(arr)) => {
                let mut out = Vec::new();
                for v in arr {
                    match v {
                        JsonValue::String(s) => out.push(s.clone()),
                        _ => return Err("tags must be strings".into()),
                    }
                }
                out
            }
            _ => return Err("missing or invalid 'tags'".into()),
        };
        Ok(UserRecord {
            name,
            age,
            active,
            tags,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.5: JsonSchemaValidator — Validate JSON Against Schema
// ═══════════════════════════════════════════════════════════════════════

/// JSON schema type constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaType {
    /// Any type allowed.
    Any,
    /// Must be null.
    Null,
    /// Must be boolean.
    Boolean,
    /// Must be integer.
    Integer,
    /// Must be number (integer or float).
    Number,
    /// Must be string.
    Str,
    /// Must be array with element schema.
    Array(Box<JsonSchema>),
    /// Must be object with required/optional fields.
    Object(Vec<FieldSchema>),
}

/// Schema for an object field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSchema {
    /// Field name.
    pub name: String,
    /// Field schema.
    pub schema: JsonSchema,
    /// Whether the field is required.
    pub required: bool,
}

/// A JSON schema definition.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonSchema {
    /// Type constraint.
    pub schema_type: SchemaType,
    /// Optional description.
    pub description: Option<String>,
}

impl JsonSchema {
    /// Create a schema for a specific type.
    pub fn of_type(schema_type: SchemaType) -> Self {
        Self {
            schema_type,
            description: None,
        }
    }

    /// Create a schema with description.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Validation error from schema checking.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    /// JSONPath to the problematic value.
    pub path: String,
    /// Error message.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Validates a `JsonValue` against a `JsonSchema`.
pub struct JsonSchemaValidator;

impl JsonSchemaValidator {
    /// Validate the value against the schema, returning all errors found.
    pub fn validate(value: &JsonValue, schema: &JsonSchema) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        Self::validate_inner(value, schema, "$", &mut errors);
        errors
    }

    /// Internal recursive validator.
    fn validate_inner(
        value: &JsonValue,
        schema: &JsonSchema,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        match &schema.schema_type {
            SchemaType::Any => {}
            SchemaType::Null => {
                if !value.is_null() {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected null, got {}", value.type_name()),
                    });
                }
            }
            SchemaType::Boolean => {
                if !matches!(value, JsonValue::Bool(_)) {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected boolean, got {}", value.type_name()),
                    });
                }
            }
            SchemaType::Integer => {
                if !matches!(value, JsonValue::Integer(_)) {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected integer, got {}", value.type_name()),
                    });
                }
            }
            SchemaType::Number => {
                if !matches!(value, JsonValue::Integer(_) | JsonValue::Float(_)) {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected number, got {}", value.type_name()),
                    });
                }
            }
            SchemaType::Str => {
                if !matches!(value, JsonValue::String(_)) {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected string, got {}", value.type_name()),
                    });
                }
            }
            SchemaType::Array(elem_schema) => match value {
                JsonValue::Array(arr) => {
                    for (i, item) in arr.iter().enumerate() {
                        let item_path = format!("{path}[{i}]");
                        Self::validate_inner(item, elem_schema, &item_path, errors);
                    }
                }
                _ => {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected array, got {}", value.type_name()),
                    });
                }
            },
            SchemaType::Object(fields) => match value {
                JsonValue::Object(pairs) => {
                    let lookup: HashMap<&str, &JsonValue> =
                        pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();
                    for field in fields {
                        let field_path = format!("{path}.{}", field.name);
                        match lookup.get(field.name.as_str()) {
                            Some(v) => {
                                Self::validate_inner(v, &field.schema, &field_path, errors);
                            }
                            None if field.required => {
                                errors.push(ValidationError {
                                    path: field_path,
                                    message: "required field missing".into(),
                                });
                            }
                            None => {}
                        }
                    }
                }
                _ => {
                    errors.push(ValidationError {
                        path: path.into(),
                        message: format!("expected object, got {}", value.type_name()),
                    });
                }
            },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.6: JsonPath — Query JSON with Path Expressions
// ═══════════════════════════════════════════════════════════════════════

/// A segment in a JSON path expression.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    /// Object key lookup: `.key`
    Key(String),
    /// Array index lookup: `[0]`
    Index(usize),
    /// Wildcard: `.*` or `[*]`
    Wildcard,
}

/// JSON path query engine.
///
/// Supports paths like `$.name`, `$.items[0].value`, `$.items[*].id`.
pub struct JsonPath;

impl JsonPath {
    /// Parse a JSONPath string into segments.
    ///
    /// Format: `$.key1.key2[0].key3[*]`
    pub fn parse(path: &str) -> Result<Vec<PathSegment>, String> {
        let path = path.strip_prefix('$').unwrap_or(path);
        let mut segments = Vec::new();
        let mut chars = path.chars().peekable();

        while let Some(&ch) = chars.peek() {
            match ch {
                '.' => {
                    chars.next();
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        segments.push(PathSegment::Wildcard);
                    } else {
                        let mut key = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '.' || c == '[' {
                                break;
                            }
                            key.push(c);
                            chars.next();
                        }
                        if key.is_empty() {
                            return Err("empty key in path".into());
                        }
                        segments.push(PathSegment::Key(key));
                    }
                }
                '[' => {
                    chars.next();
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        if chars.next() != Some(']') {
                            return Err("expected ']' after '*'".into());
                        }
                        segments.push(PathSegment::Wildcard);
                    } else {
                        let mut num_str = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == ']' {
                                break;
                            }
                            num_str.push(c);
                            chars.next();
                        }
                        if chars.next() != Some(']') {
                            return Err("expected ']'".into());
                        }
                        let idx: usize = num_str
                            .parse()
                            .map_err(|_| format!("invalid index: {num_str}"))?;
                        segments.push(PathSegment::Index(idx));
                    }
                }
                _ => return Err(format!("unexpected character '{ch}' in path")),
            }
        }
        Ok(segments)
    }

    /// Query a JSON value with a path string, returning all matching values.
    pub fn query<'a>(value: &'a JsonValue, path: &str) -> Result<Vec<&'a JsonValue>, String> {
        let segments = Self::parse(path)?;
        let mut results = vec![value];

        for seg in &segments {
            let mut next = Vec::new();
            for v in &results {
                match seg {
                    PathSegment::Key(key) => {
                        if let Some(child) = v.get(key) {
                            next.push(child);
                        }
                    }
                    PathSegment::Index(idx) => {
                        if let Some(child) = v.get_index(*idx) {
                            next.push(child);
                        }
                    }
                    PathSegment::Wildcard => match v {
                        JsonValue::Array(arr) => {
                            for item in arr {
                                next.push(item);
                            }
                        }
                        JsonValue::Object(pairs) => {
                            for (_, child) in pairs {
                                next.push(child);
                            }
                        }
                        _ => {}
                    },
                }
            }
            results = next;
        }
        Ok(results)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.7: JsonDiff — Compare Two JSON Values
// ═══════════════════════════════════════════════════════════════════════

/// A single difference between two JSON values.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffEntry {
    /// Value was added at path.
    Added {
        /// JSONPath location.
        path: String,
        /// The new value.
        value: JsonValue,
    },
    /// Value was removed at path.
    Removed {
        /// JSONPath location.
        path: String,
        /// The old value.
        value: JsonValue,
    },
    /// Value was changed at path.
    Changed {
        /// JSONPath location.
        path: String,
        /// The old value.
        old: JsonValue,
        /// The new value.
        new: JsonValue,
    },
}

impl fmt::Display for DiffEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffEntry::Added { path, value } => write!(f, "+ {path}: {value}"),
            DiffEntry::Removed { path, value } => write!(f, "- {path}: {value}"),
            DiffEntry::Changed { path, old, new } => {
                write!(f, "~ {path}: {old} -> {new}")
            }
        }
    }
}

/// Compares two JSON values and produces a list of differences.
pub struct JsonDiff;

impl JsonDiff {
    /// Compute differences between `left` and `right`.
    pub fn diff(left: &JsonValue, right: &JsonValue) -> Vec<DiffEntry> {
        let mut entries = Vec::new();
        Self::diff_inner(left, right, "$", &mut entries);
        entries
    }

    /// Returns true if the two values are structurally identical.
    pub fn is_equal(left: &JsonValue, right: &JsonValue) -> bool {
        Self::diff(left, right).is_empty()
    }

    /// Internal recursive diff.
    fn diff_inner(
        left: &JsonValue,
        right: &JsonValue,
        path: &str,
        entries: &mut Vec<DiffEntry>,
    ) {
        match (left, right) {
            (JsonValue::Object(l_pairs), JsonValue::Object(r_pairs)) => {
                let l_map: HashMap<&str, &JsonValue> =
                    l_pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();
                let r_map: HashMap<&str, &JsonValue> =
                    r_pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

                // Check keys in left
                for (k, lv) in &l_map {
                    let field_path = format!("{path}.{k}");
                    match r_map.get(k) {
                        Some(rv) => Self::diff_inner(lv, rv, &field_path, entries),
                        None => entries.push(DiffEntry::Removed {
                            path: field_path,
                            value: (*lv).clone(),
                        }),
                    }
                }
                // Check keys only in right
                for (k, rv) in &r_map {
                    if !l_map.contains_key(k) {
                        entries.push(DiffEntry::Added {
                            path: format!("{path}.{k}"),
                            value: (*rv).clone(),
                        });
                    }
                }
            }
            (JsonValue::Array(l_arr), JsonValue::Array(r_arr)) => {
                let max_len = l_arr.len().max(r_arr.len());
                for i in 0..max_len {
                    let item_path = format!("{path}[{i}]");
                    match (l_arr.get(i), r_arr.get(i)) {
                        (Some(lv), Some(rv)) => Self::diff_inner(lv, rv, &item_path, entries),
                        (Some(lv), None) => entries.push(DiffEntry::Removed {
                            path: item_path,
                            value: lv.clone(),
                        }),
                        (None, Some(rv)) => entries.push(DiffEntry::Added {
                            path: item_path,
                            value: rv.clone(),
                        }),
                        (None, None) => {}
                    }
                }
            }
            (l, r) if l == r => {}
            (l, r) => {
                entries.push(DiffEntry::Changed {
                    path: path.into(),
                    old: l.clone(),
                    new: r.clone(),
                });
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W6.8: JsonPrettyPrinter — Indented JSON Output
// ═══════════════════════════════════════════════════════════════════════

/// Pretty-prints JSON with configurable indentation.
pub struct JsonPrettyPrinter {
    /// Indent string (e.g. "  " for 2 spaces).
    indent: String,
}

impl JsonPrettyPrinter {
    /// Create a pretty printer with 2-space indentation.
    pub fn new() -> Self {
        Self {
            indent: "  ".into(),
        }
    }

    /// Create a pretty printer with custom indent string.
    pub fn with_indent(indent: &str) -> Self {
        Self {
            indent: indent.into(),
        }
    }

    /// Pretty-print a JSON value.
    pub fn format(&self, value: &JsonValue) -> String {
        let mut out = String::new();
        self.format_inner(value, 0, &mut out);
        out
    }

    /// Internal recursive formatter.
    fn format_inner(&self, value: &JsonValue, depth: usize, out: &mut String) {
        let indent = self.indent.repeat(depth);
        let inner_indent = self.indent.repeat(depth + 1);

        match value {
            JsonValue::Null => out.push_str("null"),
            JsonValue::Bool(b) => out.push_str(&format!("{b}")),
            JsonValue::Integer(n) => out.push_str(&format!("{n}")),
            JsonValue::Float(v) => {
                if v.fract() == 0.0 {
                    out.push_str(&format!("{v:.1}"));
                } else {
                    out.push_str(&format!("{v}"));
                }
            }
            JsonValue::String(s) => out.push_str(&format!("\"{s}\"")),
            JsonValue::Array(arr) => {
                if arr.is_empty() {
                    out.push_str("[]");
                } else {
                    out.push_str("[\n");
                    for (i, item) in arr.iter().enumerate() {
                        out.push_str(&inner_indent);
                        self.format_inner(item, depth + 1, out);
                        if i < arr.len() - 1 {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    out.push_str(&indent);
                    out.push(']');
                }
            }
            JsonValue::Object(pairs) => {
                if pairs.is_empty() {
                    out.push_str("{}");
                } else {
                    out.push_str("{\n");
                    for (i, (k, v)) in pairs.iter().enumerate() {
                        out.push_str(&inner_indent);
                        out.push_str(&format!("\"{k}\": "));
                        self.format_inner(v, depth + 1, out);
                        if i < pairs.len() - 1 {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    out.push_str(&indent);
                    out.push('}');
                }
            }
        }
    }
}

impl Default for JsonPrettyPrinter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // W6.1: JsonValue
    #[test]
    fn w6_1_json_value_types() {
        assert!(JsonValue::Null.is_null());
        assert!(JsonValue::Object(vec![]).is_object());
        assert!(JsonValue::Array(vec![]).is_array());
        assert_eq!(JsonValue::Bool(true).type_name(), "boolean");
        assert_eq!(JsonValue::Integer(42).type_name(), "integer");
        assert_eq!(JsonValue::Float(3.14).type_name(), "number");
        assert_eq!(JsonValue::String("hi".into()).type_name(), "string");
    }

    #[test]
    fn w6_1_json_value_get() {
        let obj = JsonValue::Object(vec![
            ("name".into(), JsonValue::String("Fajar".into())),
            ("age".into(), JsonValue::Integer(30)),
        ]);
        assert_eq!(obj.get("name"), Some(&JsonValue::String("Fajar".into())));
        assert_eq!(obj.get("age"), Some(&JsonValue::Integer(30)));
        assert_eq!(obj.get("missing"), None);
    }

    #[test]
    fn w6_1_json_value_get_index() {
        let arr = JsonValue::Array(vec![
            JsonValue::Integer(1),
            JsonValue::Integer(2),
            JsonValue::Integer(3),
        ]);
        assert_eq!(arr.get_index(0), Some(&JsonValue::Integer(1)));
        assert_eq!(arr.get_index(2), Some(&JsonValue::Integer(3)));
        assert_eq!(arr.get_index(5), None);
    }

    #[test]
    fn w6_1_json_value_display() {
        assert_eq!(format!("{}", JsonValue::Null), "null");
        assert_eq!(format!("{}", JsonValue::Bool(true)), "true");
        assert_eq!(format!("{}", JsonValue::Integer(42)), "42");
        assert_eq!(format!("{}", JsonValue::String("hi".into())), "\"hi\"");
    }

    // W6.2: JsonParser
    #[test]
    fn w6_2_parse_null() {
        assert_eq!(JsonParser::parse("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn w6_2_parse_bool() {
        assert_eq!(JsonParser::parse("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(JsonParser::parse("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn w6_2_parse_numbers() {
        assert_eq!(JsonParser::parse("42").unwrap(), JsonValue::Integer(42));
        assert_eq!(JsonParser::parse("-7").unwrap(), JsonValue::Integer(-7));
        assert_eq!(JsonParser::parse("3.14").unwrap(), JsonValue::Float(3.14));
        assert_eq!(JsonParser::parse("1e2").unwrap(), JsonValue::Float(100.0));
    }

    #[test]
    fn w6_2_parse_string() {
        assert_eq!(
            JsonParser::parse(r#""hello""#).unwrap(),
            JsonValue::String("hello".into())
        );
        assert_eq!(
            JsonParser::parse(r#""line\nbreak""#).unwrap(),
            JsonValue::String("line\nbreak".into())
        );
    }

    #[test]
    fn w6_2_parse_array() {
        let val = JsonParser::parse("[1, 2, 3]").unwrap();
        assert_eq!(
            val,
            JsonValue::Array(vec![
                JsonValue::Integer(1),
                JsonValue::Integer(2),
                JsonValue::Integer(3),
            ])
        );
    }

    #[test]
    fn w6_2_parse_object() {
        let val = JsonParser::parse(r#"{"a": 1, "b": "two"}"#).unwrap();
        assert_eq!(
            val,
            JsonValue::Object(vec![
                ("a".into(), JsonValue::Integer(1)),
                ("b".into(), JsonValue::String("two".into())),
            ])
        );
    }

    #[test]
    fn w6_2_parse_nested() {
        let val = JsonParser::parse(r#"{"items": [1, {"nested": true}]}"#).unwrap();
        let items = val.get("items").unwrap();
        assert!(items.is_array());
        let nested = items.get_index(1).unwrap();
        assert_eq!(nested.get("nested"), Some(&JsonValue::Bool(true)));
    }

    #[test]
    fn w6_2_parse_error_trailing() {
        let err = JsonParser::parse("42 extra").unwrap_err();
        assert!(err.message.contains("trailing"));
    }

    #[test]
    fn w6_2_parse_empty_object_and_array() {
        assert_eq!(JsonParser::parse("{}").unwrap(), JsonValue::Object(vec![]));
        assert_eq!(JsonParser::parse("[]").unwrap(), JsonValue::Array(vec![]));
    }

    // W6.3: JsonSerializer
    #[test]
    fn w6_3_serialize_compact() {
        let obj = JsonValue::Object(vec![
            ("x".into(), JsonValue::Integer(1)),
            ("y".into(), JsonValue::Bool(false)),
        ]);
        assert_eq!(JsonSerializer::to_string(&obj), r#"{"x":1,"y":false}"#);
    }

    #[test]
    fn w6_3_serialize_escaped() {
        let val = JsonValue::String("line\nnew".into());
        assert_eq!(JsonSerializer::to_escaped_string(&val), r#""line\nnew""#);
    }

    // W6.4: SerdeBridge
    #[test]
    fn w6_4_serde_bridge_serialize() {
        let user = UserRecord {
            name: "Fajar".into(),
            age: 30,
            active: true,
            tags: vec!["rust".into(), "ml".into()],
        };
        let json = SerdeBridge::serialize(&user).unwrap();
        assert!(json.contains("\"name\":\"Fajar\""));
        assert!(json.contains("\"age\":30"));
    }

    #[test]
    fn w6_4_serde_bridge_deserialize() {
        let json = r#"{"name":"Fajar","age":30,"active":true,"tags":["rust"]}"#;
        let user: UserRecord = SerdeBridge::deserialize(json).unwrap();
        assert_eq!(user.name, "Fajar");
        assert_eq!(user.age, 30);
        assert!(user.active);
        assert_eq!(user.tags, vec!["rust".to_string()]);
    }

    #[test]
    fn w6_4_serde_bridge_round_trip() {
        let user = UserRecord {
            name: "Alice".into(),
            age: 25,
            active: false,
            tags: vec!["dev".into()],
        };
        assert!(SerdeBridge::round_trip(&user).unwrap());
    }

    // W6.5: JsonSchemaValidator
    #[test]
    fn w6_5_schema_valid() {
        let schema = JsonSchema::of_type(SchemaType::Object(vec![
            FieldSchema {
                name: "name".into(),
                schema: JsonSchema::of_type(SchemaType::Str),
                required: true,
            },
            FieldSchema {
                name: "age".into(),
                schema: JsonSchema::of_type(SchemaType::Integer),
                required: true,
            },
        ]));
        let value = JsonParser::parse(r#"{"name":"Fajar","age":30}"#).unwrap();
        let errors = JsonSchemaValidator::validate(&value, &schema);
        assert!(errors.is_empty());
    }

    #[test]
    fn w6_5_schema_missing_field() {
        let schema = JsonSchema::of_type(SchemaType::Object(vec![FieldSchema {
            name: "name".into(),
            schema: JsonSchema::of_type(SchemaType::Str),
            required: true,
        }]));
        let value = JsonParser::parse(r#"{}"#).unwrap();
        let errors = JsonSchemaValidator::validate(&value, &schema);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("required"));
    }

    #[test]
    fn w6_5_schema_type_mismatch() {
        let schema = JsonSchema::of_type(SchemaType::Integer);
        let value = JsonValue::String("not a number".into());
        let errors = JsonSchemaValidator::validate(&value, &schema);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("expected integer"));
    }

    #[test]
    fn w6_5_schema_array_elements() {
        let schema = JsonSchema::of_type(SchemaType::Array(Box::new(JsonSchema::of_type(
            SchemaType::Integer,
        ))));
        let value = JsonParser::parse(r#"[1, "two", 3]"#).unwrap();
        let errors = JsonSchemaValidator::validate(&value, &schema);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].path.contains("[1]"));
    }

    // W6.6: JsonPath
    #[test]
    fn w6_6_path_simple_key() {
        let val = JsonParser::parse(r#"{"name":"Fajar"}"#).unwrap();
        let results = JsonPath::query(&val, "$.name").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &JsonValue::String("Fajar".into()));
    }

    #[test]
    fn w6_6_path_nested() {
        let val = JsonParser::parse(r#"{"a":{"b":{"c":42}}}"#).unwrap();
        let results = JsonPath::query(&val, "$.a.b.c").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &JsonValue::Integer(42));
    }

    #[test]
    fn w6_6_path_array_index() {
        let val = JsonParser::parse(r#"{"items":[10,20,30]}"#).unwrap();
        let results = JsonPath::query(&val, "$.items[1]").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &JsonValue::Integer(20));
    }

    #[test]
    fn w6_6_path_wildcard() {
        let val = JsonParser::parse(r#"{"items":[{"id":1},{"id":2}]}"#).unwrap();
        let results = JsonPath::query(&val, "$.items[*].id").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], &JsonValue::Integer(1));
        assert_eq!(results[1], &JsonValue::Integer(2));
    }

    // W6.7: JsonDiff
    #[test]
    fn w6_7_diff_identical() {
        let a = JsonParser::parse(r#"{"x":1}"#).unwrap();
        let b = JsonParser::parse(r#"{"x":1}"#).unwrap();
        assert!(JsonDiff::is_equal(&a, &b));
    }

    #[test]
    fn w6_7_diff_changed_value() {
        let a = JsonParser::parse(r#"{"x":1}"#).unwrap();
        let b = JsonParser::parse(r#"{"x":2}"#).unwrap();
        let diffs = JsonDiff::diff(&a, &b);
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            DiffEntry::Changed { path, old, new } => {
                assert!(path.contains("x"));
                assert_eq!(old, &JsonValue::Integer(1));
                assert_eq!(new, &JsonValue::Integer(2));
            }
            _ => panic!("expected Changed entry"),
        }
    }

    #[test]
    fn w6_7_diff_added_removed() {
        let a = JsonParser::parse(r#"{"a":1}"#).unwrap();
        let b = JsonParser::parse(r#"{"b":2}"#).unwrap();
        let diffs = JsonDiff::diff(&a, &b);
        assert_eq!(diffs.len(), 2);
        let has_removed = diffs.iter().any(|d| matches!(d, DiffEntry::Removed { .. }));
        let has_added = diffs.iter().any(|d| matches!(d, DiffEntry::Added { .. }));
        assert!(has_removed);
        assert!(has_added);
    }

    #[test]
    fn w6_7_diff_array_length() {
        let a = JsonParser::parse("[1,2]").unwrap();
        let b = JsonParser::parse("[1,2,3]").unwrap();
        let diffs = JsonDiff::diff(&a, &b);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(&diffs[0], DiffEntry::Added { .. }));
    }

    // W6.8: JsonPrettyPrinter
    #[test]
    fn w6_8_pretty_print_object() {
        let val = JsonParser::parse(r#"{"name":"Fajar","age":30}"#).unwrap();
        let pp = JsonPrettyPrinter::new();
        let out = pp.format(&val);
        assert!(out.contains("{\n"));
        assert!(out.contains("  \"name\": \"Fajar\""));
        assert!(out.contains("  \"age\": 30"));
    }

    #[test]
    fn w6_8_pretty_print_empty() {
        let pp = JsonPrettyPrinter::new();
        assert_eq!(pp.format(&JsonValue::Object(vec![])), "{}");
        assert_eq!(pp.format(&JsonValue::Array(vec![])), "[]");
    }

    #[test]
    fn w6_8_pretty_print_custom_indent() {
        let val = JsonParser::parse(r#"{"x":1}"#).unwrap();
        let pp = JsonPrettyPrinter::with_indent("\t");
        let out = pp.format(&val);
        assert!(out.contains("\t\"x\": 1"));
    }

    #[test]
    fn w6_8_pretty_print_nested_array() {
        let val = JsonParser::parse(r#"{"items":[1,2,3]}"#).unwrap();
        let pp = JsonPrettyPrinter::new();
        let out = pp.format(&val);
        assert!(out.contains("\"items\": [\n"));
        assert!(out.contains("    1"));
    }

    // Integration: round-trip parse -> pretty -> parse
    #[test]
    fn w6_integration_round_trip_parse_format_parse() {
        let json = r#"{"users":[{"name":"Alice","score":95},{"name":"Bob","score":87}]}"#;
        let val = JsonParser::parse(json).unwrap();
        let pp = JsonPrettyPrinter::new();
        let pretty = pp.format(&val);
        let val2 = JsonParser::parse(&pretty).unwrap();
        assert!(JsonDiff::is_equal(&val, &val2));
    }
}
