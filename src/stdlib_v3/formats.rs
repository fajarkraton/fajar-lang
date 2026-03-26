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
        match self { Self::String(s) => Some(s), _ => None }
    }

    /// Returns as f64 if this is a Number value.
    pub fn as_f64(&self) -> Option<f64> {
        match self { Self::Number(n) => Some(*n), _ => None }
    }

    /// Returns as bool if this is a Bool value.
    pub fn as_bool(&self) -> Option<bool> {
        match self { Self::Bool(b) => Some(*b), _ => None }
    }

    /// Returns true if this is Null.
    pub fn is_null(&self) -> bool { matches!(self, Self::Null) }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Number(n) => {
                if *n == (*n as i64) as f64 { write!(f, "{}", *n as i64) }
                else { write!(f, "{n}") }
            }
            Self::String(s) => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Self::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Self::Object(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
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
    if trimmed.is_empty() { return Err("empty input".to_string()); }
    let (val, rest) = parse_value(trimmed)?;
    if !rest.trim().is_empty() { return Err(format!("trailing characters: {rest}")); }
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
    if !input.starts_with('"') { return Err("expected string".to_string()); }
    let mut s = String::new();
    let mut i = 1;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Ok((JsonValue::String(s), &input[i + 1..])),
            b'\\' => {
                i += 1;
                if i >= bytes.len() { return Err("unterminated string escape".to_string()); }
                match bytes[i] {
                    b'"' => s.push('"'), b'\\' => s.push('\\'), b'/' => s.push('/'),
                    b'n' => s.push('\n'), b't' => s.push('\t'), b'r' => s.push('\r'),
                    c => { s.push('\\'); s.push(c as char); }
                }
            }
            b => s.push(b as char),
        }
        i += 1;
    }
    Err("unterminated string".to_string())
}

fn parse_number(input: &str) -> Result<(JsonValue, &str), String> {
    let end = input.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != 'e' && c != 'E' && c != '+')
        .unwrap_or(input.len());
    let num_str = &input[..end];
    let num: f64 = num_str.parse().map_err(|e| format!("invalid number: {e}"))?;
    Ok((JsonValue::Number(num), &input[end..]))
}

fn parse_array(input: &str) -> Result<(JsonValue, &str), String> {
    let mut rest = input[1..].trim_start();
    let mut arr = Vec::new();
    if let Some(stripped) = rest.strip_prefix(']') { return Ok((JsonValue::Array(arr), stripped)); }
    loop {
        let (val, r) = parse_value(rest)?;
        arr.push(val);
        rest = r.trim_start();
        if let Some(stripped) = rest.strip_prefix(']') { return Ok((JsonValue::Array(arr), stripped)); }
        if let Some(stripped) = rest.strip_prefix(',') { rest = stripped.trim_start(); }
        else { return Err("expected ',' or ']' in array".to_string()); }
    }
}

fn parse_object(input: &str) -> Result<(JsonValue, &str), String> {
    let mut rest = input[1..].trim_start();
    let mut entries = Vec::new();
    if let Some(stripped) = rest.strip_prefix('}') { return Ok((JsonValue::Object(entries), stripped)); }
    loop {
        let (key_val, r) = parse_string(rest)?;
        let key = match key_val { JsonValue::String(s) => s, _ => unreachable!() };
        let r = r.trim_start();
        if !r.starts_with(':') { return Err("expected ':' in object".to_string()); }
        let (val, r) = parse_value(&r[1..])?;
        entries.push((key, val));
        rest = r.trim_start();
        if let Some(stripped) = rest.strip_prefix('}') { return Ok((JsonValue::Object(entries), stripped)); }
        if let Some(stripped) = rest.strip_prefix(',') { rest = stripped.trim_start(); }
        else { return Err("expected ',' or '}' in object".to_string()); }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.2: CSV Reader/Writer
// ═══════════════════════════════════════════════════════════════════════

/// CSV record (row of fields).
pub type CsvRecord = Vec<String>;

/// Parses CSV text into records.
pub fn csv_parse(input: &str, delimiter: char) -> Vec<CsvRecord> {
    input.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split(delimiter)
                .map(|field| field.trim().trim_matches('"').to_string())
                .collect()
        })
        .collect()
}

/// Serializes records to CSV text.
pub fn csv_serialize(records: &[CsvRecord], delimiter: char) -> String {
    records.iter().map(|record| {
        record.iter().map(|field| {
            if field.contains(delimiter) || field.contains('"') || field.contains('\n') {
                format!("\"{}\"", field.replace('"', "\"\""))
            } else {
                field.clone()
            }
        }).collect::<Vec<_>>().join(&delimiter.to_string())
    }).collect::<Vec<_>>().join("\n")
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
        Self { year, month, day, hour, minute, second, millis: 0, utc_offset_minutes: 0 }
    }

    /// Returns ISO 8601 string (e.g., "2026-03-26T12:00:00Z").
    pub fn to_iso8601(&self) -> String {
        let tz = if self.utc_offset_minutes == 0 {
            "Z".to_string()
        } else {
            let sign = if self.utc_offset_minutes > 0 { '+' } else { '-' };
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
            2 => if self.is_leap_year() { 29 } else { 28 },
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
    pub fn nil() -> Self { Self { bytes: [0; 16] } }

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
        write!(f, "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15])
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
    if data.len() < 4 { return "application/octet-stream"; }
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
        assert_eq!(json_parse("\"hello\"").unwrap(), JsonValue::String("hello".to_string()));
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
        assert_eq!(arr.index(1).unwrap().get("x").unwrap().as_bool(), Some(true));
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
        assert_eq!(mime_from_magic(&[0x00, b'a', b's', b'm']), "application/wasm");
        assert_eq!(mime_from_magic(&[0x7F, b'E', b'L', b'F']), "application/x-elf");
    }
}
