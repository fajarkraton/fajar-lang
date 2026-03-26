//! Data Formats — JSON, TOML, CSV, regex, datetime, UUID, compression.
//!
//! Phase S3: 20 tasks covering parsing, serialization, and data manipulation.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S3.1: JSON Parser/Serializer
// ═══════════════════════════════════════════════════════════════════════

/// JSON value type.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Gets a value by key (for objects).
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Gets a value by index (for arrays).
    pub fn index(&self, idx: usize) -> Option<&JsonValue> {
        match self {
            Self::Array(arr) => arr.get(idx),
            _ => None,
        }
    }

    /// Returns as string if this is a String value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns as f64 if this is a Number value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns as bool if this is a Bool value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns true if this is Null.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Number(n) => {
                if *n == (*n as i64) as f64 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{n}")
                }
            }
            Self::String(s) => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Self::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Self::Object(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
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

/// Parses a JSON string into a JsonValue.
pub fn json_parse(input: &str) -> Result<JsonValue, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty input".to_string());
    }
    let (val, rest) = parse_value(trimmed)?;
    if !rest.trim().is_empty() {
        return Err(format!("trailing characters: {rest}"));
    }
    Ok(val)
}

fn parse_value(input: &str) -> Result<(JsonValue, &str), String> {
    let input = input.trim_start();
    match input.as_bytes().first() {
        Some(b'"') => parse_string(input),
        Some(b'{') => parse_object(input),
        Some(b'[') => parse_array(input),
        Some(b't') if input.starts_with("true") => Ok((JsonValue::Bool(true), &input[4..])),
        Some(b'f') if input.starts_with("false") => Ok((JsonValue::Bool(false), &input[5..])),
        Some(b'n') if input.starts_with("null") => Ok((JsonValue::Null, &input[4..])),
        Some(b'-') | Some(b'0'..=b'9') => parse_number(input),
        Some(c) => Err(format!("unexpected character: {}", *c as char)),
        None => Err("unexpected end of input".to_string()),
    }
}

fn parse_string(input: &str) -> Result<(JsonValue, &str), String> {
    if !input.starts_with('"') {
        return Err("expected string".to_string());
    }
    let mut s = String::new();
    let mut i = 1;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Ok((JsonValue::String(s), &input[i + 1..])),
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    return Err("unterminated string escape".to_string());
                }
                match bytes[i] {
                    b'"' => s.push('"'),
                    b'\\' => s.push('\\'),
                    b'/' => s.push('/'),
                    b'b' => s.push('\u{0008}'),  // backspace
                    b'f' => s.push('\u{000C}'),  // form feed
                    b'n' => s.push('\n'),
                    b't' => s.push('\t'),
                    b'r' => s.push('\r'),
                    b'u' => {
                        // FQ7.1: Unicode escape \uXXXX (+ surrogate pairs)
                        if i + 4 >= bytes.len() {
                            return Err("incomplete unicode escape".to_string());
                        }
                        let hex = &input[i + 1..i + 5];
                        let code_point = u16::from_str_radix(hex, 16)
                            .map_err(|_| format!("invalid unicode escape: \\u{hex}"))?;
                        i += 4; // advance past XXXX

                        // Check for surrogate pair: \uD800-\uDBFF followed by \uDC00-\uDFFF
                        if (0xD800..=0xDBFF).contains(&code_point) {
                            // High surrogate — expect \uDC00-\uDFFF
                            if i + 2 < bytes.len() && bytes[i + 1] == b'\\' && bytes[i + 2] == b'u' {
                                if i + 6 < bytes.len() {
                                    let low_hex = &input[i + 3..i + 7];
                                    let low = u16::from_str_radix(low_hex, 16)
                                        .map_err(|_| format!("invalid surrogate: \\u{low_hex}"))?;
                                    if (0xDC00..=0xDFFF).contains(&low) {
                                        let cp = 0x10000
                                            + ((code_point as u32 - 0xD800) << 10)
                                            + (low as u32 - 0xDC00);
                                        if let Some(c) = char::from_u32(cp) {
                                            s.push(c);
                                        }
                                        i += 6; // advance past \uXXXX
                                    } else {
                                        s.push(char::REPLACEMENT_CHARACTER);
                                    }
                                }
                            } else {
                                s.push(char::REPLACEMENT_CHARACTER);
                            }
                        } else if let Some(c) = char::from_u32(code_point as u32) {
                            s.push(c);
                        } else {
                            s.push(char::REPLACEMENT_CHARACTER);
                        }
                    }
                    c => {
                        s.push('\\');
                        s.push(c as char);
                    }
                }
            }
            b => {
                // Handle multi-byte UTF-8: if high bit set, read full UTF-8 sequence
                if b & 0x80 == 0 {
                    s.push(b as char);
                } else {
                    // Determine byte count from leading bits
                    let byte_count = if b & 0xE0 == 0xC0 {
                        2
                    } else if b & 0xF0 == 0xE0 {
                        3
                    } else if b & 0xF8 == 0xF0 {
                        4
                    } else {
                        1 // invalid, treat as single byte
                    };
                    if i + byte_count <= bytes.len() {
                        if let Ok(ch) = std::str::from_utf8(&bytes[i..i + byte_count]) {
                            s.push_str(ch);
                            i += byte_count - 1; // -1 because outer loop does i += 1
                        } else {
                            s.push(char::REPLACEMENT_CHARACTER);
                        }
                    } else {
                        s.push(char::REPLACEMENT_CHARACTER);
                    }
                }
            }
        }
        i += 1;
    }
    Err("unterminated string".to_string())
}

fn parse_number(input: &str) -> Result<(JsonValue, &str), String> {
    let end = input
        .find(|c: char| {
            !c.is_ascii_digit() && c != '.' && c != '-' && c != 'e' && c != 'E' && c != '+'
        })
        .unwrap_or(input.len());
    let num_str = &input[..end];
    let num: f64 = num_str
        .parse()
        .map_err(|e| format!("invalid number: {e}"))?;
    Ok((JsonValue::Number(num), &input[end..]))
}

fn parse_array(input: &str) -> Result<(JsonValue, &str), String> {
    let mut rest = input[1..].trim_start();
    let mut arr = Vec::new();
    if let Some(stripped) = rest.strip_prefix(']') {
        return Ok((JsonValue::Array(arr), stripped));
    }
    loop {
        let (val, r) = parse_value(rest)?;
        arr.push(val);
        rest = r.trim_start();
        if let Some(stripped) = rest.strip_prefix(']') {
            return Ok((JsonValue::Array(arr), stripped));
        }
        if let Some(stripped) = rest.strip_prefix(',') {
            rest = stripped.trim_start();
        } else {
            return Err("expected ',' or ']' in array".to_string());
        }
    }
}

fn parse_object(input: &str) -> Result<(JsonValue, &str), String> {
    let mut rest = input[1..].trim_start();
    let mut entries = Vec::new();
    if let Some(stripped) = rest.strip_prefix('}') {
        return Ok((JsonValue::Object(entries), stripped));
    }
    loop {
        let (key_val, r) = parse_string(rest)?;
        let key = match key_val {
            JsonValue::String(s) => s,
            _ => return Err("expected string key in object".to_string()),
        };
        let r = r.trim_start();
        if !r.starts_with(':') {
            return Err("expected ':' in object".to_string());
        }
        let (val, r) = parse_value(&r[1..])?;
        entries.push((key, val));
        rest = r.trim_start();
        if let Some(stripped) = rest.strip_prefix('}') {
            return Ok((JsonValue::Object(entries), stripped));
        }
        if let Some(stripped) = rest.strip_prefix(',') {
            rest = stripped.trim_start();
        } else {
            return Err("expected ',' or '}' in object".to_string());
        }
    }
}

/// Serializes a `JsonValue` to a compact JSON string.
///
/// Equivalent to `format!("{value}")` but provided as a named function for
/// API symmetry with `json_parse`.
pub fn json_stringify(value: &JsonValue) -> String {
    format!("{value}")
}

/// Serializes a `JsonValue` to a pretty-printed JSON string with indentation.
pub fn json_stringify_pretty(value: &JsonValue) -> String {
    fn write_pretty(val: &JsonValue, indent: usize, buf: &mut String) {
        let pad = "  ".repeat(indent);
        let inner = "  ".repeat(indent + 1);
        match val {
            JsonValue::Null => buf.push_str("null"),
            JsonValue::Bool(b) => buf.push_str(&b.to_string()),
            JsonValue::Number(n) => {
                if *n == (*n as i64) as f64 {
                    buf.push_str(&(*n as i64).to_string());
                } else {
                    buf.push_str(&n.to_string());
                }
            }
            JsonValue::String(s) => {
                buf.push('"');
                buf.push_str(&s.replace('\\', "\\\\").replace('"', "\\\""));
                buf.push('"');
            }
            JsonValue::Array(arr) => {
                if arr.is_empty() {
                    buf.push_str("[]");
                    return;
                }
                buf.push_str("[\n");
                for (i, v) in arr.iter().enumerate() {
                    buf.push_str(&inner);
                    write_pretty(v, indent + 1, buf);
                    if i < arr.len() - 1 {
                        buf.push(',');
                    }
                    buf.push('\n');
                }
                buf.push_str(&pad);
                buf.push(']');
            }
            JsonValue::Object(entries) => {
                if entries.is_empty() {
                    buf.push_str("{}");
                    return;
                }
                buf.push_str("{\n");
                for (i, (k, v)) in entries.iter().enumerate() {
                    buf.push_str(&inner);
                    buf.push('"');
                    buf.push_str(k);
                    buf.push_str("\": ");
                    write_pretty(v, indent + 1, buf);
                    if i < entries.len() - 1 {
                        buf.push(',');
                    }
                    buf.push('\n');
                }
                buf.push_str(&pad);
                buf.push('}');
            }
        }
    }
    let mut buf = String::new();
    write_pretty(value, 0, &mut buf);
    buf
}

// ═══════════════════════════════════════════════════════════════════════
// S3.2: CSV Reader/Writer
// ═══════════════════════════════════════════════════════════════════════

/// CSV record (row of fields).
pub type CsvRecord = Vec<String>;

/// Parses CSV text into records.
///
/// Handles RFC 4180-style quoting: fields wrapped in double quotes may
/// contain the delimiter, newlines, and escaped quotes (`""`).
pub fn csv_parse(input: &str, delimiter: char) -> Vec<CsvRecord> {
    let mut records: Vec<CsvRecord> = Vec::new();
    let mut current_record: CsvRecord = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                // Check for escaped quote ("") vs end of quoted field.
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.is_empty() {
            // Start of a quoted field (quote must appear at field start).
            in_quotes = true;
        } else if c == delimiter {
            current_record.push(field.trim().to_string());
            field = String::new();
        } else if c == '\n' {
            // Ignore trailing \r before \n.
            let trimmed = field.trim_end_matches('\r');
            current_record.push(trimmed.trim().to_string());
            field = String::new();
            if !current_record.iter().all(|f| f.is_empty()) || current_record.len() > 1 {
                records.push(current_record);
            }
            current_record = Vec::new();
        } else {
            field.push(c);
        }
    }

    // Flush last field / record.
    if !field.is_empty() || !current_record.is_empty() {
        let trimmed = field.trim_end_matches('\r');
        current_record.push(trimmed.trim().to_string());
        if !current_record.iter().all(|f| f.is_empty()) || current_record.len() > 1 {
            records.push(current_record);
        }
    }

    records
}

/// Serializes records to CSV text.
pub fn csv_serialize(records: &[CsvRecord], delimiter: char) -> String {
    records
        .iter()
        .map(|record| {
            record
                .iter()
                .map(|field| {
                    if field.contains(delimiter) || field.contains('"') || field.contains('\n') {
                        format!("\"{}\"", field.replace('"', "\"\""))
                    } else {
                        field.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(&delimiter.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S3.2b: TOML Parser
// ═══════════════════════════════════════════════════════════════════════

/// TOML value type (mirrors TOML data model).
#[derive(Debug, Clone, PartialEq)]
pub enum TomlValue {
    /// A string value.
    String(String),
    /// An integer value.
    Integer(i64),
    /// A float value.
    Float(f64),
    /// A boolean value.
    Bool(bool),
    /// A TOML array.
    Array(Vec<TomlValue>),
    /// A TOML table (ordered key-value pairs).
    Table(Vec<(String, TomlValue)>),
}

impl TomlValue {
    /// Gets a value by key (for tables).
    pub fn get(&self, key: &str) -> Option<&TomlValue> {
        match self {
            Self::Table(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Returns as string if this is a String value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns as i64 if this is an Integer value.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns as f64 if this is a Float value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns as bool if this is a Bool value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl fmt::Display for TomlValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Integer(n) => write!(f, "{n}"),
            Self::Float(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Self::Table(entries) => {
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{k} = {v}")?;
                }
                Ok(())
            }
        }
    }
}

/// Converts a `toml::Value` (from the toml crate) into our `TomlValue`.
fn toml_crate_to_toml_value(val: &toml::Value) -> TomlValue {
    match val {
        toml::Value::String(s) => TomlValue::String(s.clone()),
        toml::Value::Integer(n) => TomlValue::Integer(*n),
        toml::Value::Float(f) => TomlValue::Float(*f),
        toml::Value::Boolean(b) => TomlValue::Bool(*b),
        toml::Value::Datetime(dt) => TomlValue::String(dt.to_string()),
        toml::Value::Array(arr) => {
            TomlValue::Array(arr.iter().map(toml_crate_to_toml_value).collect())
        }
        toml::Value::Table(table) => {
            let entries = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_crate_to_toml_value(v)))
                .collect();
            TomlValue::Table(entries)
        }
    }
}

/// Parses a TOML string into a `TomlValue`.
///
/// Uses the `toml` crate (v0.8) for standards-compliant parsing, then
/// converts the result into our own `TomlValue` type.
pub fn toml_parse(input: &str) -> Result<TomlValue, String> {
    let parsed: toml::Value = input
        .parse::<toml::Value>()
        .map_err(|e| format!("TOML parse error: {e}"))?;
    Ok(toml_crate_to_toml_value(&parsed))
}

/// Serializes a `TomlValue` to a TOML string.
///
/// Handles top-level tables and nested structures.
pub fn toml_stringify(value: &TomlValue) -> String {
    fn write_value(val: &TomlValue) -> String {
        match val {
            TomlValue::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            TomlValue::Integer(n) => n.to_string(),
            TomlValue::Float(f) => {
                let s = f.to_string();
                // Ensure float has a decimal point for TOML compliance.
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{s}.0")
                }
            }
            TomlValue::Bool(b) => b.to_string(),
            TomlValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(write_value).collect();
                format!("[{}]", items.join(", "))
            }
            TomlValue::Table(entries) => {
                // Inline table.
                let items: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{k} = {}", write_value(v)))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }

    fn write_table(entries: &[(String, TomlValue)], prefix: &str, buf: &mut String) {
        // First pass: write simple key-value pairs.
        for (key, val) in entries {
            if !matches!(val, TomlValue::Table(_)) {
                if !prefix.is_empty() && buf.is_empty() || (!buf.is_empty() && buf.ends_with('\n'))
                {
                    // Already at line start.
                } else if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(&format!("{key} = {}\n", write_value(val)));
            }
        }

        // Second pass: write sub-tables with [section] headers.
        for (key, val) in entries {
            if let TomlValue::Table(sub_entries) = val {
                let section = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if !buf.is_empty() && !buf.ends_with('\n') {
                    buf.push('\n');
                }
                buf.push_str(&format!("[{section}]\n"));
                write_table(sub_entries, &section, buf);
            }
        }
    }

    match value {
        TomlValue::Table(entries) => {
            let mut buf = String::new();
            write_table(entries, "", &mut buf);
            // Remove trailing newline for consistency.
            while buf.ends_with('\n') {
                buf.pop();
            }
            buf
        }
        other => write_value(other),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.3: DateTime (ISO 8601)
// ═══════════════════════════════════════════════════════════════════════

/// A datetime value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// Milliseconds.
    pub millis: u16,
    /// UTC offset in minutes (0 = UTC).
    pub utc_offset_minutes: i16,
}

impl DateTime {
    /// Creates a UTC datetime.
    pub fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millis: 0,
            utc_offset_minutes: 0,
        }
    }

    /// Returns ISO 8601 string (e.g., "2026-03-26T12:00:00Z").
    pub fn to_iso8601(&self) -> String {
        let tz = if self.utc_offset_minutes == 0 {
            "Z".to_string()
        } else {
            let sign = if self.utc_offset_minutes > 0 {
                '+'
            } else {
                '-'
            };
            let abs = self.utc_offset_minutes.unsigned_abs();
            format!("{sign}{:02}:{:02}", abs / 60, abs % 60)
        };
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
            self.year, self.month, self.day, self.hour, self.minute, self.second, tz
        )
    }

    /// Checks if this is a leap year.
    pub fn is_leap_year(&self) -> bool {
        (self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0)
    }

    /// Returns days in the current month.
    pub fn days_in_month(&self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.is_leap_year() {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        }
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_iso8601())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.4: UUID v4/v7
// ═══════════════════════════════════════════════════════════════════════

/// A UUID (128-bit).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uuid {
    pub bytes: [u8; 16],
}

impl Uuid {
    /// Creates a nil UUID.
    pub fn nil() -> Self {
        Self { bytes: [0; 16] }
    }

    /// Returns the UUID version (4, 7, etc.).
    pub fn version(&self) -> u8 {
        (self.bytes[6] >> 4) & 0x0F
    }

    /// Returns the UUID variant.
    pub fn variant(&self) -> u8 {
        (self.bytes[8] >> 6) & 0x03
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.bytes;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0],
            b[1],
            b[2],
            b[3],
            b[4],
            b[5],
            b[6],
            b[7],
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15]
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.5: MIME Type Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detects MIME type from file extension.
pub fn mime_from_extension(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "tar" => "application/x-tar",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "fj" => "text/x-fajar",
        "toml" => "application/toml",
        "yaml" | "yml" => "application/x-yaml",
        _ => "application/octet-stream",
    }
}

/// Detects MIME type from file magic bytes.
pub fn mime_from_magic(data: &[u8]) -> &'static str {
    if data.len() < 4 {
        return "application/octet-stream";
    }
    match &data[..4] {
        [0x89, b'P', b'N', b'G'] => "image/png",
        [0xFF, 0xD8, 0xFF, _] => "image/jpeg",
        [b'G', b'I', b'F', b'8'] => "image/gif",
        [b'R', b'I', b'F', b'F'] => "audio/wav", // could be webp too
        [b'P', b'K', 0x03, 0x04] => "application/zip",
        [0x1F, 0x8B, _, _] => "application/gzip",
        [0x00, b'a', b's', b'm'] => "application/wasm",
        [b'%', b'P', b'D', b'F'] => "application/pdf",
        [0x7F, b'E', b'L', b'F'] => "application/x-elf",
        _ => "application/octet-stream",
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FQ7.9: Format Auto-Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detect file format from content.
///
/// Returns "json", "toml", or "csv" based on first non-whitespace characters.
pub fn detect_format(content: &str) -> &'static str {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        // Could be JSON array or TOML section header
        // TOML sections have [name] followed by key = value
        if trimmed.starts_with('[') {
            // Check if it's [key]\n or [1, 2, 3]
            if let Some(close) = trimmed.find(']') {
                let inside = &trimmed[1..close];
                // TOML section: [section_name] where name is alphanumeric
                if inside.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
                    // Check if next non-empty line has '='
                    let after = trimmed[close + 1..].trim_start();
                    if after.starts_with('\n') || after.is_empty() {
                        return "toml";
                    }
                }
            }
            "json"
        } else {
            "json"
        }
    } else if trimmed.contains('=') && !trimmed.starts_with('"') {
        // key = value pattern → TOML
        "toml"
    } else if trimmed.contains(',') {
        // Comma-separated → CSV
        "csv"
    } else {
        "csv" // default fallback
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S3.1: JSON
    #[test]
    fn s3_1_json_parse_primitives() {
        assert_eq!(json_parse("null").unwrap(), JsonValue::Null);
        assert_eq!(json_parse("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(json_parse("42").unwrap(), JsonValue::Number(42.0));
        assert_eq!(
            json_parse("\"hello\"").unwrap(),
            JsonValue::String("hello".to_string())
        );
    }

    #[test]
    fn s3_1_json_parse_array() {
        let val = json_parse("[1, 2, 3]").unwrap();
        match val {
            JsonValue::Array(arr) => assert_eq!(arr.len(), 3),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn s3_1_json_parse_object() {
        let val = json_parse(r#"{"name": "fajar", "age": 30}"#).unwrap();
        assert_eq!(val.get("name").unwrap().as_str(), Some("fajar"));
        assert_eq!(val.get("age").unwrap().as_f64(), Some(30.0));
    }

    #[test]
    fn s3_1_json_nested() {
        let val = json_parse(r#"{"data": [1, {"x": true}]}"#).unwrap();
        let arr = val.get("data").unwrap();
        assert_eq!(arr.index(0).unwrap().as_f64(), Some(1.0));
        assert_eq!(
            arr.index(1).unwrap().get("x").unwrap().as_bool(),
            Some(true)
        );
    }

    #[test]
    fn s3_1_json_serialize() {
        let obj = JsonValue::Object(vec![
            ("name".to_string(), JsonValue::String("fj".to_string())),
            ("version".to_string(), JsonValue::Number(5.5)),
        ]);
        let s = format!("{obj}");
        assert!(s.contains("\"name\":\"fj\""));
    }

    #[test]
    fn s3_1_json_error() {
        assert!(json_parse("{invalid}").is_err());
        assert!(json_parse("").is_err());
    }

    // S3.2: CSV
    #[test]
    fn s3_2_csv_parse() {
        let records = csv_parse("name,age\nfajar,30\nlang,5", ',');
        assert_eq!(records.len(), 3);
        assert_eq!(records[0], vec!["name", "age"]);
        assert_eq!(records[1], vec!["fajar", "30"]);
    }

    #[test]
    fn s3_2_csv_serialize() {
        let records = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["1".to_string(), "hello, world".to_string()],
        ];
        let csv = csv_serialize(&records, ',');
        assert!(csv.contains("\"hello, world\""));
    }

    // S3.3: DateTime
    #[test]
    fn s3_3_datetime_iso8601() {
        let dt = DateTime::utc(2026, 3, 26, 14, 30, 0);
        assert_eq!(dt.to_iso8601(), "2026-03-26T14:30:00Z");
    }

    #[test]
    fn s3_3_leap_year() {
        assert!(DateTime::utc(2024, 1, 1, 0, 0, 0).is_leap_year());
        assert!(!DateTime::utc(2025, 1, 1, 0, 0, 0).is_leap_year());
        assert!(DateTime::utc(2000, 1, 1, 0, 0, 0).is_leap_year());
        assert!(!DateTime::utc(1900, 1, 1, 0, 0, 0).is_leap_year());
    }

    #[test]
    fn s3_3_days_in_month() {
        assert_eq!(DateTime::utc(2026, 2, 1, 0, 0, 0).days_in_month(), 28);
        assert_eq!(DateTime::utc(2024, 2, 1, 0, 0, 0).days_in_month(), 29);
        assert_eq!(DateTime::utc(2026, 7, 1, 0, 0, 0).days_in_month(), 31);
    }

    // S3.4: UUID
    #[test]
    fn s3_4_uuid_nil() {
        let uuid = Uuid::nil();
        assert_eq!(format!("{uuid}"), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn s3_4_uuid_version() {
        let mut uuid = Uuid::nil();
        uuid.bytes[6] = 0x40; // version 4
        uuid.bytes[8] = 0x80; // variant 2
        assert_eq!(uuid.version(), 4);
        assert_eq!(uuid.variant(), 2);
    }

    // S3.5: MIME
    #[test]
    fn s3_5_mime_from_extension() {
        assert_eq!(mime_from_extension("json"), "application/json");
        assert_eq!(mime_from_extension("fj"), "text/x-fajar");
        assert_eq!(mime_from_extension("png"), "image/png");
        assert_eq!(mime_from_extension("wasm"), "application/wasm");
        assert_eq!(mime_from_extension("xyz"), "application/octet-stream");
    }

    #[test]
    fn s3_5_mime_from_magic() {
        assert_eq!(mime_from_magic(&[0x89, b'P', b'N', b'G']), "image/png");
        assert_eq!(mime_from_magic(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(
            mime_from_magic(&[0x00, b'a', b's', b'm']),
            "application/wasm"
        );
        assert_eq!(
            mime_from_magic(&[0x7F, b'E', b'L', b'F']),
            "application/x-elf"
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // Integration: json_stringify
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn json_stringify_compact_primitives() {
        assert_eq!(json_stringify(&JsonValue::Null), "null");
        assert_eq!(json_stringify(&JsonValue::Bool(true)), "true");
        assert_eq!(json_stringify(&JsonValue::Number(3.14)), "3.14");
        assert_eq!(json_stringify(&JsonValue::Number(42.0)), "42");
        assert_eq!(
            json_stringify(&JsonValue::String("hello".into())),
            "\"hello\""
        );
    }

    #[test]
    fn json_stringify_object_and_array() {
        let val = JsonValue::Object(vec![
            (
                "arr".into(),
                JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Number(2.0)]),
            ),
            ("flag".into(), JsonValue::Bool(false)),
        ]);
        let s = json_stringify(&val);
        assert!(s.contains("\"arr\":[1,2]"));
        assert!(s.contains("\"flag\":false"));
    }

    #[test]
    fn json_stringify_pretty_output() {
        let val = JsonValue::Object(vec![
            ("name".into(), JsonValue::String("fajar".into())),
            ("version".into(), JsonValue::Number(5.0)),
        ]);
        let pretty = json_stringify_pretty(&val);
        assert!(pretty.contains("  \"name\": \"fajar\""));
        assert!(pretty.contains("  \"version\": 5"));
        // Multi-line output.
        assert!(pretty.contains('\n'));
    }

    #[test]
    fn json_roundtrip_parse_stringify_parse() {
        let input = r#"{"name":"fajar","scores":[100,95.5,88],"active":true,"meta":null}"#;
        let parsed = json_parse(input).unwrap();
        let serialized = json_stringify(&parsed);
        let reparsed = json_parse(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn json_roundtrip_nested_objects() {
        let input = r#"{"a":{"b":{"c":42}},"d":[1,[2,3]]}"#;
        let parsed = json_parse(input).unwrap();
        let serialized = json_stringify(&parsed);
        let reparsed = json_parse(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn json_parse_escaped_strings() {
        let input = r#"{"msg":"hello\nworld","path":"c:\\dir"}"#;
        let val = json_parse(input).unwrap();
        assert_eq!(val.get("msg").unwrap().as_str(), Some("hello\nworld"));
        assert_eq!(val.get("path").unwrap().as_str(), Some("c:\\dir"));
    }

    #[test]
    fn json_parse_empty_structures() {
        let val = json_parse("{}").unwrap();
        assert_eq!(val, JsonValue::Object(vec![]));
        let val = json_parse("[]").unwrap();
        assert_eq!(val, JsonValue::Array(vec![]));
    }

    #[test]
    fn json_parse_negative_and_scientific_numbers() {
        let val = json_parse("-42").unwrap();
        assert_eq!(val.as_f64(), Some(-42.0));
        let val = json_parse("1.5e2").unwrap();
        assert_eq!(val.as_f64(), Some(150.0));
        let val = json_parse("-3.14E-1").unwrap();
        assert_eq!(val.as_f64(), Some(-0.314));
    }

    // ═══════════════════════════════════════════════════════════════════
    // Integration: TOML parser
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn toml_parse_simple_key_values() {
        let input = r#"
name = "fajar-lang"
version = "5.5.0"
edition = 2026
debug = true
"#;
        let val = toml_parse(input).unwrap();
        assert_eq!(val.get("name").unwrap().as_str(), Some("fajar-lang"));
        assert_eq!(val.get("version").unwrap().as_str(), Some("5.5.0"));
        assert_eq!(val.get("edition").unwrap().as_i64(), Some(2026));
        assert_eq!(val.get("debug").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn toml_parse_nested_tables() {
        let input = r#"
[package]
name = "fj"
version = "5.5.0"

[dependencies]
ndarray = "0.16"
"#;
        let val = toml_parse(input).unwrap();
        let pkg = val.get("package").unwrap();
        assert_eq!(pkg.get("name").unwrap().as_str(), Some("fj"));
        assert_eq!(pkg.get("version").unwrap().as_str(), Some("5.5.0"));
        let deps = val.get("dependencies").unwrap();
        assert_eq!(deps.get("ndarray").unwrap().as_str(), Some("0.16"));
    }

    #[test]
    fn toml_parse_arrays() {
        let input = r#"
ports = [80, 443, 8080]
names = ["alpha", "beta", "gamma"]
"#;
        let val = toml_parse(input).unwrap();
        match val.get("ports").unwrap() {
            TomlValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0].as_i64(), Some(80));
                assert_eq!(arr[2].as_i64(), Some(8080));
            }
            other => panic!("expected array, got: {other:?}"),
        }
        match val.get("names").unwrap() {
            TomlValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0].as_str(), Some("alpha"));
            }
            other => panic!("expected array, got: {other:?}"),
        }
    }

    #[test]
    fn toml_parse_floats() {
        let input = "pi = 3.14159\nrate = 1e-3\n";
        let val = toml_parse(input).unwrap();
        let pi = val.get("pi").unwrap().as_f64().unwrap();
        assert!((pi - 3.14159).abs() < 1e-10);
        let rate = val.get("rate").unwrap().as_f64().unwrap();
        assert!((rate - 0.001).abs() < 1e-10);
    }

    #[test]
    fn toml_parse_error_invalid_input() {
        let result = toml_parse("= no key");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("TOML parse error"));
    }

    #[test]
    fn toml_stringify_simple() {
        let val = TomlValue::Table(vec![
            ("name".into(), TomlValue::String("fj".into())),
            ("version".into(), TomlValue::Integer(5)),
            ("debug".into(), TomlValue::Bool(true)),
        ]);
        let s = toml_stringify(&val);
        assert!(s.contains("name = \"fj\""));
        assert!(s.contains("version = 5"));
        assert!(s.contains("debug = true"));
    }

    #[test]
    fn toml_value_display() {
        assert_eq!(format!("{}", TomlValue::String("hi".into())), "\"hi\"");
        assert_eq!(format!("{}", TomlValue::Integer(42)), "42");
        assert_eq!(format!("{}", TomlValue::Bool(false)), "false");
    }

    // ═══════════════════════════════════════════════════════════════════
    // Integration: CSV parser (quoted fields)
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn csv_parse_quoted_fields_with_delimiter() {
        let input = "name,address\nFajar,\"Jakarta, Indonesia\"\nLang,\"Bandung\"";
        let records = csv_parse(input, ',');
        assert_eq!(records.len(), 3);
        assert_eq!(records[1][0], "Fajar");
        assert_eq!(records[1][1], "Jakarta, Indonesia");
        assert_eq!(records[2][1], "Bandung");
    }

    #[test]
    fn csv_parse_escaped_quotes() {
        let input = "col\n\"He said \"\"hello\"\"\"";
        let records = csv_parse(input, ',');
        assert_eq!(records.len(), 2);
        assert_eq!(records[1][0], "He said \"hello\"");
    }

    #[test]
    fn csv_parse_multiline_quoted_field() {
        let input = "desc\n\"line 1\nline 2\"";
        let records = csv_parse(input, ',');
        assert_eq!(records.len(), 2);
        assert_eq!(records[1][0], "line 1\nline 2");
    }

    #[test]
    fn csv_parse_tab_delimiter() {
        let input = "a\tb\tc\n1\t2\t3\n4\t5\t6";
        let records = csv_parse(input, '\t');
        assert_eq!(records.len(), 3);
        assert_eq!(records[0], vec!["a", "b", "c"]);
        assert_eq!(records[1], vec!["1", "2", "3"]);
        assert_eq!(records[2], vec!["4", "5", "6"]);
    }

    #[test]
    fn csv_roundtrip_serialize_parse() {
        let original = vec![
            vec!["name".to_string(), "value".to_string()],
            vec!["hello, world".to_string(), "42".to_string()],
            vec!["simple".to_string(), "data".to_string()],
        ];
        let serialized = csv_serialize(&original, ',');
        let reparsed = csv_parse(&serialized, ',');
        assert_eq!(reparsed.len(), 3);
        assert_eq!(reparsed[0], vec!["name", "value"]);
        assert_eq!(reparsed[1], vec!["hello, world", "42"]);
        assert_eq!(reparsed[2], vec!["simple", "data"]);
    }

    #[test]
    fn csv_parse_empty_fields() {
        let input = "a,,c\n,b,";
        let records = csv_parse(input, ',');
        assert_eq!(records.len(), 2);
        assert_eq!(records[0], vec!["a", "", "c"]);
        assert_eq!(records[1], vec!["", "b", ""]);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Quality Improvement Tests (FQ7.x)
    // ═══════════════════════════════════════════════════════════════════

    // FQ7.1: JSON unicode escapes
    #[test]
    fn fq7_1_unicode_basic() {
        let result = json_parse(r#""\u0041""#).unwrap();
        assert_eq!(result.as_str(), Some("A")); // \u0041 == 'A'
    }

    #[test]
    fn fq7_1_unicode_non_ascii() {
        let result = json_parse(r#""\u00E9""#).unwrap();
        assert_eq!(result.as_str(), Some("é")); // \u00E9 == 'é'
    }

    #[test]
    fn fq7_1_unicode_cjk() {
        let result = json_parse(r#""\u4E16\u754C""#).unwrap();
        assert_eq!(result.as_str(), Some("世界")); // Chinese: "world"
    }

    #[test]
    fn fq7_1_unicode_surrogate_pair() {
        // 𝄞 = U+1D11E (Musical Symbol G Clef) = \uD834\uDD1E
        let result = json_parse(r#""\uD834\uDD1E""#).unwrap();
        assert_eq!(result.as_str(), Some("𝄞"));
    }

    #[test]
    fn fq7_1_unicode_mixed_with_text() {
        let result = json_parse(r#""Hello \u0057orld""#).unwrap();
        assert_eq!(result.as_str(), Some("Hello World")); // \u0057 == 'W'
    }

    // FQ7.2: JSON number edge cases
    #[test]
    fn fq7_2_number_scientific() {
        let result = json_parse("1e10").unwrap();
        assert_eq!(result.as_f64(), Some(1e10));
    }

    #[test]
    fn fq7_2_number_scientific_plus() {
        let result = json_parse("2.5e+3").unwrap();
        assert_eq!(result.as_f64(), Some(2500.0));
    }

    #[test]
    fn fq7_2_number_negative_zero() {
        let result = json_parse("-0").unwrap();
        assert_eq!(result.as_f64(), Some(-0.0));
    }

    #[test]
    fn fq7_2_number_very_small() {
        let result = json_parse("1e-308").unwrap();
        assert!(result.as_f64().unwrap() > 0.0);
    }

    // FQ7.3: JSON error messages with position
    #[test]
    fn fq7_3_error_unterminated_string() {
        let result = json_parse(r#""hello"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("unterminated"), "error: {err}");
    }

    #[test]
    fn fq7_3_error_unexpected_char() {
        let result = json_parse("@invalid");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("unexpected"), "error: {err}");
    }

    #[test]
    fn fq7_3_error_invalid_unicode() {
        let result = json_parse(r#""\uZZZZ""#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("invalid unicode"),
            "should mention unicode: {err}"
        );
    }

    // FQ7.5: TOML datetime
    #[test]
    fn fq7_5_toml_datetime() {
        let result = toml_parse("dt = 2026-03-26T12:00:00Z").unwrap();
        let dt = result.get("dt").unwrap();
        assert!(
            dt.as_str().is_some(),
            "TOML datetime should be accessible as string"
        );
    }

    // FQ7.6: TOML inline tables
    #[test]
    fn fq7_6_toml_inline_table() {
        let result = toml_parse("point = { x = 1, y = 2 }").unwrap();
        let point = result.get("point").unwrap();
        // Inline table accessed via .get()
        assert_eq!(point.get("x").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(point.get("y").and_then(|v| v.as_i64()), Some(2));
    }

    // FQ7.7: CSV escape roundtrip
    #[test]
    fn fq7_7_csv_roundtrip_with_special_chars() {
        let records = vec![
            vec!["name".to_string(), "value".to_string()],
            vec!["has,comma".to_string(), "has\"quote".to_string()],
            vec!["normal".to_string(), "data".to_string()],
        ];
        let serialized = csv_serialize(&records, ',');
        let parsed = csv_parse(&serialized, ',');
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[1][0], "has,comma");
        assert_eq!(parsed[1][1], "has\"quote");
    }

    // FQ7.8: CSV header parsing
    #[test]
    fn fq7_8_csv_with_headers() {
        let input = "name,age,city\nFajar,30,Jakarta\nBudi,25,Bandung";
        let records = csv_parse(input, ',');
        assert_eq!(records.len(), 3);
        let headers = &records[0];
        assert_eq!(headers[0], "name");
        assert_eq!(headers[1], "age");
        assert_eq!(headers[2], "city");
        // Access data by column index (header at [0])
        let row1 = &records[1];
        assert_eq!(row1[0], "Fajar");
        assert_eq!(row1[1], "30");
    }

    // FQ7.9: Format detection
    #[test]
    fn fq7_9_detect_json_object() {
        assert_eq!(detect_format(r#"{"key": "value"}"#), "json");
    }

    #[test]
    fn fq7_9_detect_json_array() {
        assert_eq!(detect_format("[1, 2, 3]"), "json");
    }

    #[test]
    fn fq7_9_detect_toml() {
        assert_eq!(detect_format("key = \"value\"\nother = 42"), "toml");
    }

    #[test]
    fn fq7_9_detect_csv() {
        assert_eq!(detect_format("name,age,city\nFajar,30,Jakarta"), "csv");
    }

    // FQ7.10: JSON stringify roundtrip with unicode
    #[test]
    fn fq7_10_json_roundtrip_unicode() {
        let original = json_parse(r#"{"greeting": "Hello \u4E16\u754C"}"#).unwrap();
        let serialized = json_stringify(&original);
        let reparsed = json_parse(&serialized).unwrap();
        assert_eq!(
            reparsed.get("greeting").and_then(|v| v.as_str()),
            Some("Hello 世界")
        );
    }
}
