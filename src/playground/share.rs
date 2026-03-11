//! Share & embed — URL-encoded sharing, short URLs, oEmbed, iframe embed.

// ═══════════════════════════════════════════════════════════════════════
// URL Encoding (S31.1)
// ═══════════════════════════════════════════════════════════════════════

/// Encodes source code into a URL-safe fragment for sharing.
///
/// Uses a simple base64-like encoding (no external crate dependency).
/// In production, lz-string would be used for better compression.
pub fn encode_for_url(source: &str) -> String {
    // Simple percent-encoding for URL fragment
    let mut encoded = String::new();
    for byte in source.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

/// Decodes a URL fragment back to source code.
pub fn decode_from_url(encoded: &str) -> Result<String, String> {
    let mut decoded = Vec::new();
    let bytes = encoded.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                decoded.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("incomplete percent-encoding".to_string());
                }
                let hex = &encoded[i + 1..i + 3];
                let byte = u8::from_str_radix(hex, 16)
                    .map_err(|_| format!("invalid hex in percent-encoding: {hex}"))?;
                decoded.push(byte);
                i += 3;
            }
            b => {
                decoded.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|e| format!("invalid UTF-8: {e}"))
}

/// Generates a share URL for the playground.
pub fn share_url(base_url: &str, source: &str) -> String {
    let encoded = encode_for_url(source);
    format!("{base_url}#code={encoded}")
}

// ═══════════════════════════════════════════════════════════════════════
// Short URLs (S31.2)
// ═══════════════════════════════════════════════════════════════════════

/// Generates a short URL ID from source code hash.
pub fn short_url_id(source: &str) -> String {
    // FNV-1a hash for better distribution
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in source.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")[..8].to_string()
}

/// Short URL format.
pub fn short_url(base_url: &str, id: &str) -> String {
    format!("{base_url}/s/{id}")
}

// ═══════════════════════════════════════════════════════════════════════
// oEmbed (S31.4)
// ═══════════════════════════════════════════════════════════════════════

/// oEmbed response format.
#[derive(Debug, Clone)]
pub struct OEmbedResponse {
    /// oEmbed version (always "1.0").
    pub version: String,
    /// Content type (always "rich").
    pub oembed_type: String,
    /// Provider name.
    pub provider_name: String,
    /// Provider URL.
    pub provider_url: String,
    /// Title.
    pub title: String,
    /// HTML iframe embed.
    pub html: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl OEmbedResponse {
    /// Creates an oEmbed response for a playground snippet.
    pub fn for_snippet(code: &str, title: &str) -> Self {
        let encoded = encode_for_url(code);
        Self {
            version: "1.0".to_string(),
            oembed_type: "rich".to_string(),
            provider_name: "Fajar Lang Playground".to_string(),
            provider_url: "https://play.fajarlang.dev".to_string(),
            title: title.to_string(),
            html: format!(
                r#"<iframe src="https://play.fajarlang.dev/embed?code={encoded}" width="600" height="400" frameborder="0"></iframe>"#
            ),
            width: 600,
            height: 400,
        }
    }

    /// Serializes to JSON.
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"version":"{}","type":"{}","provider_name":"{}","provider_url":"{}","title":"{}","html":"{}","width":{},"height":{}}}"#,
            self.version,
            self.oembed_type,
            self.provider_name,
            self.provider_url,
            self.title,
            self.html.replace('"', "\\\""),
            self.width,
            self.height,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Embed Options (S31.5, S31.6)
// ═══════════════════════════════════════════════════════════════════════

/// Embed configuration options (passed as URL params).
#[derive(Debug, Clone)]
pub struct EmbedOptions {
    /// Theme: "dark" or "light".
    pub theme: String,
    /// Whether the editor is read-only.
    pub readonly: bool,
    /// Whether to auto-run on load.
    pub autorun: bool,
    /// Whether to show the run button.
    pub show_run_button: bool,
    /// Whether to show line numbers.
    pub line_numbers: bool,
}

impl Default for EmbedOptions {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            readonly: false,
            autorun: false,
            show_run_button: true,
            line_numbers: true,
        }
    }
}

impl EmbedOptions {
    /// Parses embed options from URL query parameters.
    pub fn from_params(params: &[(String, String)]) -> Self {
        let mut opts = Self::default();
        for (key, value) in params {
            match key.as_str() {
                "theme" => opts.theme = value.clone(),
                "readonly" => opts.readonly = value == "true",
                "autorun" => opts.autorun = value == "true",
                "line_numbers" => opts.line_numbers = value != "false",
                "run_button" => opts.show_run_button = value != "false",
                _ => {}
            }
        }
        opts
    }

    /// Serializes to URL query string.
    pub fn to_query_string(&self) -> String {
        let mut params = Vec::new();
        if self.theme != "dark" {
            params.push(format!("theme={}", self.theme));
        }
        if self.readonly {
            params.push("readonly=true".to_string());
        }
        if self.autorun {
            params.push("autorun=true".to_string());
        }
        if !self.line_numbers {
            params.push("line_numbers=false".to_string());
        }
        if !self.show_run_button {
            params.push("run_button=false".to_string());
        }
        params.join("&")
    }
}

/// Generates an iframe embed tag.
pub fn embed_iframe(code: &str, options: &EmbedOptions) -> String {
    let encoded = encode_for_url(code);
    let query = options.to_query_string();
    let url = if query.is_empty() {
        format!("https://play.fajarlang.dev/embed?code={encoded}")
    } else {
        format!("https://play.fajarlang.dev/embed?code={encoded}&{query}")
    };
    format!(
        r#"<iframe src="{url}" width="100%" height="400" frameborder="0" allow="clipboard-write"></iframe>"#
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Social Preview (S31.7, S31.8)
// ═══════════════════════════════════════════════════════════════════════

/// Social preview metadata for shared playground links.
#[derive(Debug, Clone)]
pub struct SocialPreview {
    /// Page title.
    pub title: String,
    /// Description.
    pub description: String,
    /// Preview image URL.
    pub image_url: String,
    /// Canonical URL.
    pub url: String,
}

impl SocialPreview {
    /// Generates OpenGraph meta tags.
    pub fn og_tags(&self) -> String {
        format!(
            r#"<meta property="og:title" content="{}">
<meta property="og:description" content="{}">
<meta property="og:image" content="{}">
<meta property="og:url" content="{}">
<meta property="og:type" content="website">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{}">
<meta name="twitter:description" content="{}">"#,
            self.title, self.description, self.image_url, self.url, self.title, self.description,
        )
    }
}

/// Generates social preview for a playground snippet.
pub fn preview_for_code(code: &str) -> SocialPreview {
    let first_line = code.lines().next().unwrap_or("Fajar Lang code");
    let preview = if code.len() > 100 {
        format!("{}...", &code[..100])
    } else {
        code.to_string()
    };
    let id = short_url_id(code);
    SocialPreview {
        title: format!("Fajar Lang — {first_line}"),
        description: preview,
        image_url: format!("https://play.fajarlang.dev/preview/{id}.png"),
        url: format!("https://play.fajarlang.dev/s/{id}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// mdBook Embed (S31.9)
// ═══════════════════════════════════════════════════════════════════════

/// Generates a "Try it" button for mdBook pages.
pub fn try_it_button(code: &str) -> String {
    let encoded = encode_for_url(code);
    format!(
        r#"<a href="https://play.fajarlang.dev/#code={encoded}" target="_blank" class="try-it-btn">Try it in Playground</a>"#
    )
}

/// CSS for the "Try it" button in mdBook.
pub fn try_it_css() -> &'static str {
    r#"
.try-it-btn {
    display: inline-block;
    margin-top: 8px;
    padding: 4px 12px;
    background: #58a6ff;
    color: #fff;
    border-radius: 6px;
    text-decoration: none;
    font-size: 0.85rem;
    font-weight: 600;
}
.try-it-btn:hover { background: #79c0ff; }
"#
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S31.1: URL encoding
    #[test]
    fn s31_1_encode_decode_roundtrip() {
        let source = r#"fn main() { println("Hello, Fajar!") }"#;
        let encoded = encode_for_url(source);
        let decoded = decode_from_url(&encoded).unwrap();
        assert_eq!(decoded, source);
    }

    #[test]
    fn s31_1_encode_special_chars() {
        let encoded = encode_for_url("a b+c&d=e");
        assert!(encoded.contains('+')); // space -> +
        assert!(encoded.contains("%26")); // & -> %26
    }

    #[test]
    fn s31_1_share_url() {
        let url = share_url("https://play.fajarlang.dev", "let x = 42");
        assert!(url.starts_with("https://play.fajarlang.dev#code="));
    }

    // S31.2: Short URLs
    #[test]
    fn s31_2_short_url_id() {
        let id = short_url_id("fn main() {}");
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn s31_2_short_url_deterministic() {
        let id1 = short_url_id("hello");
        let id2 = short_url_id("hello");
        assert_eq!(id1, id2);

        let id3 = short_url_id("world");
        assert_ne!(id1, id3);
    }

    #[test]
    fn s31_2_short_url_format() {
        let url = short_url("https://play.fajarlang.dev", "abc12345");
        assert_eq!(url, "https://play.fajarlang.dev/s/abc12345");
    }

    // S31.4: oEmbed
    #[test]
    fn s31_4_oembed_response() {
        let resp = OEmbedResponse::for_snippet("let x = 42", "Example");
        assert_eq!(resp.version, "1.0");
        assert_eq!(resp.oembed_type, "rich");
        assert!(resp.html.contains("iframe"));
    }

    #[test]
    fn s31_4_oembed_json() {
        let resp = OEmbedResponse::for_snippet("println(\"hi\")", "Hello");
        let json = resp.to_json();
        assert!(json.contains("\"version\":\"1.0\""));
        assert!(json.contains("\"type\":\"rich\""));
    }

    // S31.5-S31.6: Embed
    #[test]
    fn s31_5_embed_iframe() {
        let opts = EmbedOptions::default();
        let iframe = embed_iframe("let x = 1", &opts);
        assert!(iframe.contains("<iframe"));
        assert!(iframe.contains("play.fajarlang.dev/embed"));
    }

    #[test]
    fn s31_6_embed_options() {
        let params = vec![
            ("theme".to_string(), "light".to_string()),
            ("readonly".to_string(), "true".to_string()),
            ("autorun".to_string(), "true".to_string()),
        ];
        let opts = EmbedOptions::from_params(&params);
        assert_eq!(opts.theme, "light");
        assert!(opts.readonly);
        assert!(opts.autorun);
    }

    #[test]
    fn s31_6_embed_options_query_string() {
        let opts = EmbedOptions {
            theme: "light".to_string(),
            readonly: true,
            ..Default::default()
        };
        let qs = opts.to_query_string();
        assert!(qs.contains("theme=light"));
        assert!(qs.contains("readonly=true"));
    }

    // S31.7-S31.8: Social preview
    #[test]
    fn s31_7_social_preview() {
        let preview = preview_for_code("fn main() { println(\"Hello\") }");
        assert!(preview.title.contains("fn main"));
        assert!(!preview.image_url.is_empty());
    }

    #[test]
    fn s31_8_og_tags() {
        let preview = preview_for_code("let x = 42");
        let tags = preview.og_tags();
        assert!(tags.contains("og:title"));
        assert!(tags.contains("twitter:card"));
        assert!(tags.contains("summary_large_image"));
    }

    // S31.9: mdBook embed
    #[test]
    fn s31_9_try_it_button() {
        let btn = try_it_button("let x = 42");
        assert!(btn.contains("play.fajarlang.dev"));
        assert!(btn.contains("Try it"));
    }

    #[test]
    fn s31_9_try_it_css() {
        let css = try_it_css();
        assert!(css.contains(".try-it-btn"));
    }

    // S31.10: URL decoding error
    #[test]
    fn s31_10_decode_invalid() {
        assert!(decode_from_url("%ZZ").is_err());
        assert!(decode_from_url("%").is_err());
    }
}
